use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_skill-forge");

const SKILL_JS: &str = "defineSkill(async (input) => input);\n";
const SCHEMA_JS: &str = "defineSchema({ type: 'object', additionalProperties: true });\n";

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
            "skill-forge-{label}-{}-{nanos}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create fake home");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn install_skill(&self, name: &str) -> PathBuf {
        let dir = self.path.join(".skill-forge").join("skills").join(name);
        fs::create_dir_all(&dir).expect("create skill dir");
        fs::write(dir.join("skill.js"), SKILL_JS).expect("write skill.js");
        fs::write(dir.join("schema.js"), SCHEMA_JS).expect("write schema.js");
        dir
    }
}

impl Drop for FakeHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_with_home(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(args)
        .env("HOME", home)
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .output()
        .expect("failed to run skill-forge")
}

#[test]
fn positional_name_resolves_to_skill_dir() {
    let home = FakeHome::new("positional");
    home.install_skill("hello");

    let out = run_with_home(
        home.path(),
        &[
            "run",
            "hello",
            "--model",
            "claude-haiku-4-5-20251001",
            "--key",
            "value",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected zero exit\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("output is not valid JSON: {e}\nstdout:\n{stdout}"));
    assert_eq!(v, serde_json::json!({ "key": "value" }));
}

#[test]
fn skill_flag_still_works() {
    let home = FakeHome::new("via-flag");
    let dir = home.install_skill("via-flag");
    let skill_path = dir.join("skill.js");

    let out = run_with_home(
        home.path(),
        &[
            "run",
            "--skill",
            skill_path.to_str().unwrap(),
            "--model",
            "claude-haiku-4-5-20251001",
            "--echoed",
            "hi",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected zero exit\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("output is not valid JSON: {e}\nstdout:\n{stdout}"));
    assert_eq!(v, serde_json::json!({ "echoed": "hi" }));
}

#[test]
fn positional_and_skill_are_mutually_exclusive() {
    let home = FakeHome::new("clash");
    let dir = home.install_skill("clash");
    let skill_path = dir.join("skill.js");

    let out = run_with_home(
        home.path(),
        &[
            "run",
            "clash",
            "--skill",
            skill_path.to_str().unwrap(),
            "--model",
            "claude-haiku-4-5-20251001",
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected non-zero exit when both positional and --skill are given"
    );
    assert!(
        stderr.contains("mutually exclusive"),
        "stderr should mention mutual exclusion; got:\n{stderr}"
    );
}

#[test]
fn omitting_model_uses_haiku_default() {
    let home = FakeHome::new("default-model");
    home.install_skill("hello");

    let out = run_with_home(
        home.path(),
        &["run", "hello", "--key", "value"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected zero exit when --model is omitted\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("output is not valid JSON: {e}\nstdout:\n{stdout}"));
    assert_eq!(v, serde_json::json!({ "key": "value" }));
}

#[test]
fn missing_both_name_and_skill_errors() {
    let home = FakeHome::new("missing");

    let out = run_with_home(
        home.path(),
        &["run", "--model", "claude-haiku-4-5-20251001"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected non-zero exit when neither positional nor --skill is given"
    );
    assert!(
        stderr.contains("required"),
        "stderr should mention required; got:\n{stderr}"
    );
}
