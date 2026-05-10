use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_forge");

const ECHO_SKILL_JS: &str = "defineSkill(async (input) => input);\n";
const ECHO_SCHEMA_JS: &str = "defineSchema({ type: 'object', additionalProperties: true, properties: { msg: { type: 'string' } } });\n";
const ECHO_DESCRIPTION_MD: &str = "Echo input back as JSON. Use to verify MCP tool wiring.\n";

const NOOP_SKILL_JS: &str = "defineSkill(async () => ({}));\n";
const NOOP_SCHEMA_JS: &str = "defineSchema({ type: 'object' });\n";

struct FakeHome {
    path: PathBuf,
}

impl FakeHome {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "skill-forge-mcp-{label}-{}-{nanos}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create fake home");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn install_skill(
        &self,
        name: &str,
        skill_js: &str,
        schema_js: &str,
        description: Option<&str>,
    ) -> PathBuf {
        let dir = self.path.join(".skill-forge").join("skills").join(name);
        fs::create_dir_all(&dir).expect("create skill dir");
        fs::write(dir.join("skill.js"), skill_js).expect("write skill.js");
        fs::write(dir.join("schema.js"), schema_js).expect("write schema.js");
        if let Some(d) = description {
            fs::write(dir.join("DESCRIPTION.md"), d).expect("write DESCRIPTION.md");
        }
        dir
    }
}

impl Drop for FakeHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct McpSession {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    stderr_reader: BufReader<std::process::ChildStderr>,
}

impl McpSession {
    fn spawn_skills(home: &Path) -> Self {
        let mut child = Command::new(BIN)
            .args(["mcp-server", "--mode", "skills"])
            .env("HOME", home)
            .env_remove("ANTHROPIC_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn mcp-server");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let stderr_reader = BufReader::new(child.stderr.take().expect("stderr"));
        Self {
            child,
            stdin,
            stdout,
            stderr_reader,
        }
    }

    fn send(&mut self, req: serde_json::Value) {
        let line = req.to_string();
        writeln!(self.stdin, "{line}").expect("write request");
        self.stdin.flush().expect("flush stdin");
    }

    fn recv(&mut self) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let mut line = String::new();
            match self.stdout.read_line(&mut line) {
                Ok(0) => panic!("eof before response"),
                Ok(_) => {
                    if line.trim().is_empty() {
                        if Instant::now() >= deadline {
                            panic!("timeout waiting for response");
                        }
                        continue;
                    }
                    return serde_json::from_str(line.trim())
                        .unwrap_or_else(|e| panic!("parse response: {e}\nline: {line}"));
                }
                Err(e) => panic!("read response: {e}"),
            }
        }
    }

    fn close_and_collect_stderr(mut self) -> String {
        drop(self.stdin);
        let _ = self.child.wait();
        let mut stderr_buf = String::new();
        let _ = std::io::Read::read_to_string(&mut self.stderr_reader, &mut stderr_buf);
        stderr_buf
    }
}

fn req_initialize(id: u64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {}
    })
}

fn req_tools_list(id: u64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/list"
    })
}

fn req_tools_call(id: u64, name: &str, args: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": args }
    })
}

#[test]
fn skills_mode_lists_user_skills_with_description() {
    let home = FakeHome::new("list");
    home.install_skill(
        "echo-poc",
        ECHO_SKILL_JS,
        ECHO_SCHEMA_JS,
        Some(ECHO_DESCRIPTION_MD),
    );

    let mut s = McpSession::spawn_skills(home.path());
    s.send(req_initialize(1));
    let init = s.recv();
    assert_eq!(init["id"], serde_json::json!(1));
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");

    s.send(req_tools_list(2));
    let list = s.recv();
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1, "expected one tool, got {tools:?}");
    let tool = &tools[0];
    assert_eq!(tool["name"], "echo-poc");
    assert!(
        tool["description"]
            .as_str()
            .unwrap_or("")
            .contains("Echo input back"),
        "description should be raw DESCRIPTION.md content; got {tool:?}"
    );
    assert_eq!(tool["inputSchema"]["type"], "object");

    let _ = s.close_and_collect_stderr();
}

#[test]
fn skills_mode_invokes_skill_in_process() {
    let home = FakeHome::new("call");
    home.install_skill(
        "echo-poc",
        ECHO_SKILL_JS,
        ECHO_SCHEMA_JS,
        Some(ECHO_DESCRIPTION_MD),
    );

    let mut s = McpSession::spawn_skills(home.path());
    s.send(req_initialize(1));
    let _ = s.recv();
    s.send(req_tools_call(
        2,
        "echo-poc",
        serde_json::json!({ "msg": "hi" }),
    ));
    let resp = s.recv();
    assert_eq!(resp["result"]["isError"], serde_json::json!(false));
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("text is JSON");
    assert_eq!(parsed, serde_json::json!({ "msg": "hi" }));

    let _ = s.close_and_collect_stderr();
}

#[test]
fn skills_mode_skips_skill_missing_description_and_warns() {
    let home = FakeHome::new("skip-desc");
    home.install_skill(
        "echo-poc",
        ECHO_SKILL_JS,
        ECHO_SCHEMA_JS,
        Some(ECHO_DESCRIPTION_MD),
    );
    home.install_skill("missing-desc", NOOP_SKILL_JS, NOOP_SCHEMA_JS, None);

    let mut s = McpSession::spawn_skills(home.path());
    s.send(req_initialize(1));
    let _ = s.recv();
    s.send(req_tools_list(2));
    let list = s.recv();
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(names, vec!["echo-poc"]);

    let stderr = s.close_and_collect_stderr();
    assert!(
        stderr.contains("skipping skill 'missing-desc'"),
        "expected skip warning for missing-desc; stderr was:\n{stderr}"
    );
    assert!(
        stderr.contains("DESCRIPTION.md"),
        "expected mention of DESCRIPTION.md in skip warning; stderr was:\n{stderr}"
    );
}

#[test]
fn skills_mode_skips_skill_with_invalid_schema() {
    let home = FakeHome::new("skip-schema");
    home.install_skill(
        "echo-poc",
        ECHO_SKILL_JS,
        ECHO_SCHEMA_JS,
        Some(ECHO_DESCRIPTION_MD),
    );
    // schema.js without defineSchema call -> evaluation should fail
    home.install_skill(
        "bad-schema",
        NOOP_SKILL_JS,
        "// no defineSchema call\n",
        Some("desc\n"),
    );

    let mut s = McpSession::spawn_skills(home.path());
    s.send(req_initialize(1));
    let _ = s.recv();
    s.send(req_tools_list(2));
    let list = s.recv();
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(names, vec!["echo-poc"]);

    let stderr = s.close_and_collect_stderr();
    assert!(
        stderr.contains("skipping skill 'bad-schema'"),
        "expected skip warning for bad-schema; stderr was:\n{stderr}"
    );
}

#[test]
fn skills_mode_unknown_tool_returns_error() {
    let home = FakeHome::new("unknown");
    home.install_skill(
        "echo-poc",
        ECHO_SKILL_JS,
        ECHO_SCHEMA_JS,
        Some(ECHO_DESCRIPTION_MD),
    );

    let mut s = McpSession::spawn_skills(home.path());
    s.send(req_initialize(1));
    let _ = s.recv();
    s.send(req_tools_call(2, "no-such-skill", serde_json::json!({})));
    let resp = s.recv();
    assert_eq!(resp["error"]["code"], serde_json::json!(-32602));
    assert!(
        resp["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("unknown skill"),
        "expected 'unknown skill' in error message; got {resp:?}"
    );

    let _ = s.close_and_collect_stderr();
}

#[test]
fn codegen_mode_still_exposes_legacy_tools() {
    let mut child = Command::new(BIN)
        .args(["mcp-server", "--mode", "codegen"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn codegen mode");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    writeln!(stdin, "{}", req_initialize(1)).unwrap();
    writeln!(stdin, "{}", req_tools_list(2)).unwrap();
    stdin.flush().unwrap();
    drop(stdin);

    let mut init_line = String::new();
    stdout.read_line(&mut init_line).unwrap();
    let mut list_line = String::new();
    stdout.read_line(&mut list_line).unwrap();

    let _ = child.wait();

    let list: serde_json::Value =
        serde_json::from_str(list_line.trim()).expect("parse list response");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().unwrap_or(""))
        .collect();
    assert!(names.contains(&"callLlm"), "names: {names:?}");
    assert!(names.contains(&"execCmd"), "names: {names:?}");
    assert!(names.contains(&"submit_generated_code"), "names: {names:?}");
}

#[test]
fn mcp_server_default_mode_is_codegen() {
    let mut child = Command::new(BIN)
        .args(["mcp-server"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn default mode");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    writeln!(stdin, "{}", req_initialize(1)).unwrap();
    writeln!(stdin, "{}", req_tools_list(2)).unwrap();
    stdin.flush().unwrap();
    drop(stdin);

    let mut init_line = String::new();
    stdout.read_line(&mut init_line).unwrap();
    let mut list_line = String::new();
    stdout.read_line(&mut list_line).unwrap();

    let _ = child.wait();

    let list: serde_json::Value =
        serde_json::from_str(list_line.trim()).expect("parse list response");
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().unwrap_or(""))
        .collect();
    assert!(
        names.contains(&"callLlm")
            && names.contains(&"execCmd")
            && names.contains(&"submit_generated_code"),
        "default mode should be codegen; names: {names:?}"
    );
}
