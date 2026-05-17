use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_forge");

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

    fn exports_root(&self) -> PathBuf {
        self.path.join(".skill-forge").join("exports")
    }

    fn install_user_skill(&self, name: &str, description: &str) {
        let dir = self.path.join(".skill-forge").join("skills").join(name);
        fs::create_dir_all(&dir).expect("create user skill dir");
        // Minimal skill that just echoes input, no LLM required for schema fetch.
        let skill_js = "defineTool(async (input) => input);\n";
        let schema_js = "defineSchema(\
            { type: 'object', properties: { value: { type: 'string', description: 'echo' } }, required: ['value'], additionalProperties: false },\
            { type: 'object', properties: { value: { type: 'string', description: 'echo' } }, required: ['value'], additionalProperties: false }\
        );\n";
        fs::write(dir.join("skill.js"), skill_js).expect("write skill.js");
        fs::write(dir.join("schema.js"), schema_js).expect("write schema.js");
        fs::write(dir.join("DESCRIPTION.md"), description).expect("write DESCRIPTION.md");
    }

    fn remove_user_skill(&self, name: &str) {
        let dir = self.path.join(".skill-forge").join("skills").join(name);
        fs::remove_dir_all(&dir).expect("remove user skill dir");
    }
}

impl Drop for FakeHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_export(home: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["export"];
    args.extend_from_slice(extra);
    Command::new(BIN)
        .args(&args)
        .env("HOME", home)
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .output()
        .expect("failed to run forge export")
}

#[test]
fn writes_skill_md_and_description_md_for_each_builtin() {
    let home = FakeHome::new("export-builtins");

    let out = run_export(home.path(), &[]);
    assert!(
        out.status.success(),
        "expected zero exit\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let exports = home.exports_root();
    for name in ["echo-task", "implementation-check", "issue-checkout", "pr-create"] {
        let skill_md = exports.join(name).join("SKILL.md");
        let desc_md = exports.join(name).join("DESCRIPTION.md");
        assert!(skill_md.is_file(), "missing {}", skill_md.display());
        assert!(desc_md.is_file(), "missing {}", desc_md.display());
        let body = fs::read_to_string(&skill_md).expect("read SKILL.md");
        assert!(body.starts_with("---\n"), "{name} SKILL.md missing frontmatter");
        assert!(
            body.contains(&format!("name: {name}")),
            "{name} SKILL.md missing name field"
        );
        assert!(
            body.contains(&format!("forge run {name} ")),
            "{name} SKILL.md missing forge run invocation"
        );
    }
}

#[test]
fn writes_manifest_with_skill_set() {
    let home = FakeHome::new("export-manifest");

    let out = run_export(home.path(), &[]);
    assert!(out.status.success(), "stderr:\n{}", String::from_utf8_lossy(&out.stderr));

    let manifest_path = home.exports_root().join(".manifest.json");
    assert!(manifest_path.is_file(), "manifest missing");

    let body = fs::read_to_string(&manifest_path).expect("read manifest");
    let v: serde_json::Value = serde_json::from_str(&body).expect("parse manifest JSON");
    assert_eq!(v["version"], 1);
    let skills = v["skills"].as_array().expect("skills array");
    let names: Vec<&str> = skills.iter().filter_map(|s| s.as_str()).collect();
    for expected in ["echo-task", "implementation-check", "issue-checkout", "pr-create"] {
        assert!(names.contains(&expected), "manifest missing {expected}: {names:?}");
    }
}

#[test]
fn idempotent_when_rerun_against_clean_state() {
    let home = FakeHome::new("export-idempotent");

    let first = run_export(home.path(), &[]);
    assert!(first.status.success(), "first run failed");
    let skill_md = home.exports_root().join("echo-task").join("SKILL.md");
    let first_bytes = fs::read(&skill_md).expect("read after first run");

    let second = run_export(home.path(), &[]);
    assert!(second.status.success(), "second run failed");
    let second_bytes = fs::read(&skill_md).expect("read after second run");

    assert_eq!(first_bytes, second_bytes, "SKILL.md changed across runs");
}

#[test]
fn places_symlinks_under_claude_skills_pointing_at_exports() {
    let home = FakeHome::new("export-symlinks");
    let out = run_export(home.path(), &[]);
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let claude_skills = home.path().join(".claude").join("skills");
    for name in ["echo-task", "implementation-check", "issue-checkout", "pr-create"] {
        let link = claude_skills.join(name);
        let meta = fs::symlink_metadata(&link)
            .unwrap_or_else(|e| panic!("missing link {}: {e}", link.display()));
        assert!(meta.file_type().is_symlink(), "{} is not a symlink", link.display());
        let target = fs::read_link(&link).expect("readlink");
        let expected = home.exports_root().join(name);
        assert_eq!(target, expected, "symlink target mismatch for {name}");
    }
}

#[test]
fn rerun_is_idempotent_for_symlinks() {
    let home = FakeHome::new("export-symlinks-idempotent");
    run_export(home.path(), &[]);
    run_export(home.path(), &[]); // must not error on existing forge-owned link

    let link = home.path().join(".claude").join("skills").join("echo-task");
    assert!(link.is_symlink() || fs::symlink_metadata(&link).map(|m| m.file_type().is_symlink()).unwrap_or(false));
}

#[test]
fn refuses_to_overwrite_existing_user_dir_without_force() {
    let home = FakeHome::new("export-conflict");
    let manual_dir = home.path().join(".claude").join("skills").join("echo-task");
    fs::create_dir_all(&manual_dir).expect("create manual dir");
    fs::write(manual_dir.join("SKILL.md"), "hand-written\n").expect("write manual SKILL.md");

    let out = run_export(home.path(), &[]);
    assert!(
        !out.status.success(),
        "expected non-zero exit due to conflict\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("already exists") || stderr.contains("forge-owned"),
        "expected conflict message in stderr, got:\n{stderr}"
    );
    let body = fs::read_to_string(manual_dir.join("SKILL.md")).expect("manual SKILL.md still there");
    assert_eq!(body, "hand-written\n", "manual SKILL.md must not be overwritten");
}

#[test]
fn removes_orphan_symlink_and_canonical_when_user_skill_deleted() {
    let home = FakeHome::new("export-orphan-cleanup");
    home.install_user_skill("my-skill", "User-defined skill for orphan test.");

    let first = run_export(home.path(), &[]);
    assert!(first.status.success(), "stderr:\n{}", String::from_utf8_lossy(&first.stderr));

    let canonical = home.exports_root().join("my-skill");
    let symlink = home.path().join(".claude").join("skills").join("my-skill");
    assert!(canonical.is_dir(), "canonical missing after first export");
    assert!(
        fs::symlink_metadata(&symlink)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "symlink missing after first export"
    );

    home.remove_user_skill("my-skill");

    let second = run_export(home.path(), &[]);
    assert!(second.status.success(), "stderr:\n{}", String::from_utf8_lossy(&second.stderr));

    assert!(!canonical.exists(), "canonical should be GC'd: {}", canonical.display());
    let sym_meta = fs::symlink_metadata(&symlink);
    assert!(
        sym_meta.is_err(),
        "orphan symlink should be removed: {}",
        symlink.display()
    );

    // Builtin canonical dirs must remain untouched.
    assert!(home.exports_root().join("echo-task").is_dir());
}

#[test]
fn force_backs_up_existing_user_dir_then_installs_symlink() {
    let home = FakeHome::new("export-force-backup");
    let manual_dir = home.path().join(".claude").join("skills").join("echo-task");
    fs::create_dir_all(&manual_dir).expect("create manual dir");
    fs::write(manual_dir.join("MARKER"), "hand-written").expect("write marker");

    let out = run_export(home.path(), &["--force"]);
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parent = manual_dir.parent().unwrap();
    let entries: Vec<_> = fs::read_dir(parent)
        .expect("read .claude/skills")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let bak = entries
        .iter()
        .find(|n| n.starts_with("echo-task.bak."))
        .unwrap_or_else(|| panic!("no .bak.* sibling found in {entries:?}"));
    let bak_dir = parent.join(bak);
    assert!(
        bak_dir.join("MARKER").is_file(),
        "marker should be preserved inside {}",
        bak_dir.display()
    );
    assert!(manual_dir.is_symlink() || fs::symlink_metadata(&manual_dir).map(|m| m.file_type().is_symlink()).unwrap_or(false));
}
