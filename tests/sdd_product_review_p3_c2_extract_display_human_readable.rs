//! SDD product-review Phase 3, C2.
//! GIVEN a config command with an extract where aggregate="last" and
//! unit="percent" WHEN `usereport explain <that-command-name>` runs THEN
//! stdout contains `last` AND contains `percent` AND does NOT contain `Last`
//! in Debug notation AND does NOT contain `Percent`.
//!
//! This test FAILS today because run_explain_command uses {:?} formatting
//! for aggregate and unit (Debug format), producing "Last" and "Percent".
#![cfg(feature = "bin")]

use std::io::Write as _;

#[test]
fn extract_display_human_readable() {
    let config_toml = r#"
[defaults]
max_parallel_commands = 1
repetitions = 1

[[profile]]
name = "default"
commands = ["my-cmd"]

[[command]]
name = "my-cmd"
command = "echo 42"
timeout = 1
[[command.extract]]
signal_id = "my.signal"
aggregate = "last"
unit = "pct"
pattern = "(?P<val>\\d+)"
"#;

    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("config.toml");
    {
        let mut f = std::fs::File::create(&config_path).expect("create config");
        f.write_all(config_toml.as_bytes()).expect("write config");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--config", config_path.to_str().unwrap(), "explain", "my-cmd"])
        .output()
        .expect("run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0; stderr: {stderr}\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("last"),
        "expected stdout to contain lowercase 'last'; got: {stdout}"
    );
    assert!(
        stdout.contains("percent"),
        "expected stdout to contain lowercase 'percent'; got: {stdout}"
    );
    assert!(
        !stdout.contains("Last"),
        "stdout must not contain Debug-format 'Last'; got: {stdout}"
    );
    assert!(
        !stdout.contains("Percent"),
        "stdout must not contain Debug-format 'Percent'; got: {stdout}"
    );
}
