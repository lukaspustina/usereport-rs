//! SDD product-review Phase 3, C1.
//! GIVEN a config where command `mem-check` has `[[command.extract]]` with
//! `signal_id = "mem.free_pct"` WHEN `usereport explain mem.free_pct` runs
//! THEN exit code is 0 AND stdout contains `mem.free_pct` AND stdout
//! contains `mem-check`.
//!
//! This test FAILS today because run_explain only checks command names and
//! rule IDs, not signal IDs.
#![cfg(feature = "bin")]

use std::io::Write as _;

#[test]
fn signal_id_resolved_in_explain() {
    let config_toml = r#"
[defaults]
max_parallel_commands = 1
repetitions = 1

[[profile]]
name = "default"
commands = ["mem-check"]

[[command]]
name = "mem-check"
command = "cat /proc/meminfo"
timeout = 1
[[command.extract]]
signal_id = "mem.free_pct"
aggregate = "last"
unit = "pct"
pattern = "MemFree:\\s+(?P<val>\\d+)"
"#;

    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("config.toml");
    {
        let mut f = std::fs::File::create(&config_path).expect("create config");
        f.write_all(config_toml.as_bytes()).expect("write config");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--config", config_path.to_str().unwrap(), "explain", "mem.free_pct"])
        .output()
        .expect("run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0; stderr: {stderr}\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("mem.free_pct"),
        "expected stdout to contain 'mem.free_pct'; got: {stdout}"
    );
    assert!(
        stdout.contains("mem-check"),
        "expected stdout to contain 'mem-check'; got: {stdout}"
    );
}
