use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_forge");

const SKILL_JS: &str = "defineTool(async (input) => input);\n";
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

    fn install_skill(&self, name: &str) {
        let dir = self.path.join(".skill-forge").join("skills").join(name);
        fs::create_dir_all(&dir).expect("create skill dir");
        fs::write(dir.join("skill.js"), SKILL_JS).expect("write skill.js");
        fs::write(dir.join("schema.js"), SCHEMA_JS).expect("write schema.js");
    }
}

impl Drop for FakeHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_list(home: &Path) -> std::process::Output {
    Command::new(BIN)
        .args(["list"])
        .env("HOME", home)
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .output()
        .expect("failed to run forge list")
}

fn stdout_lines(out: &std::process::Output) -> Vec<String> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect()
}

fn parse_label_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("• ")?;
    let space = rest.find(' ')?;
    Some(&rest[..space])
}

#[test]
fn lists_builtin_skills_when_no_user_skills_dir() {
    let home = FakeHome::new("list-no-user");

    let out = run_list(home.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected zero exit\nstderr:\n{stderr}"
    );

    let lines = stdout_lines(&out);
    assert!(!lines.is_empty(), "expected at least one builtin skill");
    for line in &lines {
        assert!(
            line.starts_with("• "),
            "every line should start with bullet; got:\n{line}"
        );
        assert!(
            line.contains(" (builtin)"),
            "every line should be a builtin entry with space before (kind); got:\n{line}"
        );
    }
}

#[test]
fn includes_user_skills_with_user_kind() {
    let home = FakeHome::new("list-with-user");
    home.install_skill("my-user-skill");

    let out = run_list(home.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected zero exit\nstderr:\n{stderr}"
    );

    let lines = stdout_lines(&out);
    let user_line = lines
        .iter()
        .find(|l| l.contains("my-user-skill (user)"))
        .unwrap_or_else(|| panic!("user skill missing in output:\n{}", lines.join("\n")));
    assert!(
        user_line.starts_with("• "),
        "user line should start with bullet; got:\n{user_line}"
    );
    assert!(
        !user_line.contains("  - "),
        "user skill should have no description; got:\n{user_line}"
    );
}

#[test]
fn entries_are_sorted_alphabetically_across_kinds() {
    let home = FakeHome::new("list-sorted");
    home.install_skill("aaa-user");
    home.install_skill("zzz-user");

    let out = run_list(home.path());
    assert!(out.status.success());

    let lines = stdout_lines(&out);
    let names: Vec<&str> = lines.iter().filter_map(|l| parse_label_name(l)).collect();
    assert_eq!(
        names.len(),
        lines.len(),
        "every line should parse a name; got:\n{}",
        lines.join("\n")
    );
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "entries are not sorted by name");

    let first = names.first().expect("at least one entry");
    let last = names.last().expect("at least one entry");
    assert_eq!(*first, "aaa-user");
    assert_eq!(*last, "zzz-user");
}

#[test]
fn ignores_user_dirs_without_skill_js() {
    let home = FakeHome::new("list-incomplete");
    let dir = home
        .path()
        .join(".skill-forge")
        .join("skills")
        .join("incomplete");
    fs::create_dir_all(&dir).expect("create incomplete dir");

    let out = run_list(home.path());
    assert!(out.status.success());

    let lines = stdout_lines(&out);
    assert!(
        !lines.iter().any(|l| l.contains("incomplete (")),
        "user dirs without skill.js should be skipped; got:\n{}",
        lines.join("\n")
    );
}

#[test]
fn builtin_entries_show_first_line_of_description() {
    let home = FakeHome::new("list-desc");

    let out = run_list(home.path());
    assert!(out.status.success());

    let lines = stdout_lines(&out);
    let echo_task = lines
        .iter()
        .find(|l| l.contains("• echo-task (builtin)"))
        .unwrap_or_else(|| panic!("echo-task entry not found:\n{}", lines.join("\n")));
    assert!(
        echo_task.contains("  - "),
        "builtin entry should include description separator; got:\n{echo_task}"
    );
}

#[test]
fn descriptions_are_not_truncated_when_stdout_is_not_tty() {
    let home = FakeHome::new("list-pipe");

    let out = run_list(home.path());
    assert!(out.status.success());

    let lines = stdout_lines(&out);
    assert!(
        !lines.iter().any(|l| l.ends_with("...")),
        "no line should be truncated when stdout is piped; got:\n{}",
        lines.join("\n")
    );
}
