use std::path::PathBuf;
use std::process::Command;

fn run_fixture(name: &str) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_skill-forge");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}/skill.js"));

    Command::new(bin)
        .args(["run", "--skill"])
        .arg(&fixture)
        .args(["--args", "{}", "--model", "claude-haiku-4-5-20251001"])
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .output()
        .expect("failed to run skill-forge")
}

#[test]
fn invoke_skill_covers_all_evaluation_axes() {
    let out = run_fixture("invoke-multiple-skills");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected zero exit\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("output is not valid JSON: {e}\nstdout:\n{stdout}"));

    assert_eq!(
        v["echoed"],
        serde_json::json!({ "hello": "world", "n": 42 }),
        "echo should round-trip input verbatim"
    );

    assert_eq!(
        v["caught"],
        serde_json::json!({ "code": "user-error", "message": "boom" }),
        "errors thrown by builtin skill should propagate with code/message"
    );

    assert_eq!(
        v["sequential"],
        serde_json::json!({ "a": { "tag": "A" }, "b": { "tag": "B" } }),
        "sequential invokes should be independent"
    );

    assert_eq!(
        v["composed"],
        serde_json::json!({ "wrapped": { "hello": "nested" } }),
        "builtin->builtin nested invoke should pass values through"
    );

    let denied = &v["denied"];
    assert_eq!(
        denied["code"], "capability-denied",
        "unknown skill name should be capability-denied"
    );
    assert!(
        denied["message"]
            .as_str()
            .unwrap_or("")
            .contains("does-not-exist"),
        "capability-denied message should mention the unknown skill name; got: {denied}"
    );
}
