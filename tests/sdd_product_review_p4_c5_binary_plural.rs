//! SDD product-review Phase 4, C5.
//! GIVEN one missing binary WHEN the not-found message is emitted THEN it
//! reads `1 binary not found`. GIVEN two missing binaries THEN it reads
//! `2 binaries not found`.
//!
//! This test FAILS today because the message uses "binary/binaries" always.
#![cfg(feature = "bin")]

use std::io::Write as _;

fn make_config_with_commands(commands: &[&str]) -> String {
    let mut toml = String::from("[defaults]\nmax_parallel_commands = 1\nrepetitions = 1\n\n[[profile]]\nname = \"default\"\ncommands = [");
    toml.push_str(&commands.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", "));
    toml.push_str("]\n\n");
    for cmd in commands {
        toml.push_str(&format!(
            "[[command]]\nname = \"{cmd}\"\ncommand = \"/nonexistent-binary-{cmd}\"\ntimeout = 1\n\n"
        ));
    }
    toml
}

#[test]
fn one_missing_binary_singular() {
    let config = make_config_with_commands(&["no-such-cmd-a"]);
    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("config.toml");
    {
        let mut f = std::fs::File::create(&config_path).expect("create config");
        f.write_all(config.as_bytes()).expect("write config");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--config", config_path.to_str().unwrap(), "check"])
        .output()
        .expect("run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("1 binary not found"),
        "expected '1 binary not found' but got: {stderr}"
    );
    assert!(
        !stderr.contains("1 binary/binaries"),
        "should not use 'binary/binaries'; got: {stderr}"
    );
}

#[test]
fn two_missing_binaries_plural() {
    let config = make_config_with_commands(&["no-such-cmd-a", "no-such-cmd-b"]);
    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("config.toml");
    {
        let mut f = std::fs::File::create(&config_path).expect("create config");
        f.write_all(config.as_bytes()).expect("write config");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--config", config_path.to_str().unwrap(), "check"])
        .output()
        .expect("run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("2 binaries not found"),
        "expected '2 binaries not found' but got: {stderr}"
    );
}
