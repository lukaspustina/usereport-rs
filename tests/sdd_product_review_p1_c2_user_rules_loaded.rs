//! Tests for SDD `specs/sdd/product-review.md` Phase 1, criterion 2.
//! GIVEN `XDG_CONFIG_HOME` points to a temp dir containing
//! `usereport/rules.d/test.toml` with rule `id = "test.user_rule"` and an
//! always-true predicate WHEN `usereport --output json` runs THEN the JSON
//! output contains a finding with `id == "test.user_rule"`.
//!
//! This test FAILS today because `generate_report` calls `builtin_rules()`
//! directly and never loads the user rules directory. It will pass once Phase
//! 1c of the SDD is implemented.
#![cfg(feature = "bin")]

use std::fs;

#[test]
fn user_rules_dir_findings_appear_in_json_output() {
    // Build a temp config home with the user rules directory.
    let tmp = tempfile::tempdir().expect("create tempdir");
    let rules_dir = tmp.path().join("usereport").join("rules.d");
    fs::create_dir_all(&rules_dir).expect("create rules.d");

    // Write a rule whose predicate is always true: load average is always >= 0.
    let rule_toml = r#"
[[rule]]
id = "test.user_rule"
summary = "always fires"
severity = "Warn"
when = "host.load_avg_1m >= 0"
"#;
    fs::write(rules_dir.join("test.toml"), rule_toml).expect("write rule file");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--output", "json"])
        .env("XDG_CONFIG_HOME", tmp.path())
        .output()
        .expect("run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0; stderr: {stderr}\nstdout: {stdout}"
    );

    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    let findings = report
        .get("findings")
        .and_then(|f| f.as_array())
        .expect("JSON must have a 'findings' array");

    let has_user_rule = findings
        .iter()
        .any(|f| f.get("id").and_then(|id| id.as_str()) == Some("test.user_rule"));

    assert!(
        has_user_rule,
        "expected a finding with id 'test.user_rule' in findings array; got: {findings:#?}"
    );
}
