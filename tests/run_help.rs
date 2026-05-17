use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_forge");

const SKILL_JS: &str = "defineTool(async (input) => input);\n";

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

    fn install_skill(&self, name: &str, schema_js: &str) -> PathBuf {
        let dir = self.path.join(".skill-forge").join("skills").join(name);
        fs::create_dir_all(&dir).expect("create skill dir");
        fs::write(dir.join("skill.js"), SKILL_JS).expect("write skill.js");
        fs::write(dir.join("schema.js"), schema_js).expect("write schema.js");
        dir
    }
}

impl Drop for FakeHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_help(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(args)
        .env("HOME", home)
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .output()
        .expect("failed to run forge")
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn help_without_skill_shows_only_forge_options() {
    let home = FakeHome::new("help-no-skill");

    let out = run_help(home.path(), &["run", "--help"]);
    assert!(out.status.success(), "expected zero exit");

    let s = stdout(&out);
    assert!(s.contains("Usage: forge run"), "missing Usage line:\n{s}");
    assert!(s.contains("Options:"), "missing Options section:\n{s}");
    assert!(
        !s.contains("Skill arguments:"),
        "Skill arguments must not appear without skill:\n{s}"
    );
}

#[test]
fn help_with_builtin_skill_shows_skill_arguments_section() {
    let home = FakeHome::new("help-builtin");

    let out = run_help(home.path(), &["run", "echo-task", "--help"]);
    assert!(out.status.success(), "expected zero exit");

    let s = stdout(&out);
    assert!(s.contains("Options:"), "missing Options section:\n{s}");
    assert!(
        s.contains("Skill arguments:"),
        "missing Skill arguments section:\n{s}"
    );
    assert!(
        s.contains("<message>"),
        "positional <message> not shown:\n{s}"
    );
    assert!(
        s.contains("(positional"),
        "positional marker missing:\n{s}"
    );
}

#[test]
fn help_with_user_skill_via_name_renders_schema() {
    let home = FakeHome::new("help-user-name");
    let schema = "defineSchema({ \
        type: 'object', \
        properties: { \
            userName: { type: 'string', description: 'User name' }, \
            color: { type: 'string', enum: ['red', 'green', 'blue'] }, \
            note: { type: 'string', default: 'unused' } \
        }, \
        required: ['userName'], \
        additionalProperties: false \
    });\n";
    home.install_skill("hello", schema);

    let out = run_help(home.path(), &["run", "hello", "--help"]);
    assert!(out.status.success(), "expected zero exit");

    let s = stdout(&out);
    assert!(s.contains("--user-name <string>"), "kebab-case flag missing:\n{s}");
    assert!(s.contains("(required)"), "required marker missing:\n{s}");
    assert!(s.contains("(optional)"), "optional marker missing:\n{s}");
    assert!(s.contains("User name"), "description missing:\n{s}");
    assert!(
        s.contains("[enum: red|green|blue]"),
        "enum block missing:\n{s}"
    );
    assert!(s.contains("[default: unused]"), "default block missing:\n{s}");
}

#[test]
fn help_via_skill_flag_loads_schema_from_path() {
    let home = FakeHome::new("help-skill-flag");
    let schema = "defineSchema({ \
        type: 'object', \
        properties: { count: { type: 'integer' } }, \
        required: ['count'] \
    });\n";
    let dir = home.install_skill("via-flag", schema);
    let skill_path = dir.join("skill.js");

    let out = run_help(
        home.path(),
        &["run", "--skill", skill_path.to_str().unwrap(), "--help"],
    );
    assert!(out.status.success(), "expected zero exit");

    let s = stdout(&out);
    assert!(s.contains("--count <integer>"), "flag/type missing:\n{s}");
    assert!(s.contains("(required)"), "required marker missing:\n{s}");
}

#[test]
fn help_omits_skill_arguments_section_when_schema_has_no_properties() {
    let home = FakeHome::new("help-empty-schema");
    let schema = "defineSchema({ type: 'object', additionalProperties: true });\n";
    home.install_skill("noargs", schema);

    let out = run_help(home.path(), &["run", "noargs", "--help"]);
    assert!(out.status.success(), "expected zero exit");

    let s = stdout(&out);
    assert!(s.contains("Options:"), "missing Options section:\n{s}");
    assert!(
        !s.contains("Skill arguments:"),
        "Skill arguments header should be hidden when schema has no properties:\n{s}"
    );
}

#[test]
fn short_help_flag_is_equivalent() {
    let home = FakeHome::new("help-short-flag");

    let out = run_help(home.path(), &["run", "echo-task", "-h"]);
    assert!(out.status.success(), "expected zero exit");

    let s = stdout(&out);
    assert!(s.contains("Skill arguments:"), "missing section for -h:\n{s}");
    assert!(s.contains("<message>"), "missing positional for -h:\n{s}");
}
