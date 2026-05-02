//! SDD product-review Phase 4, C3.
//! GIVEN no baseline files exist in the baseline store directory WHEN
//! `usereport baseline list` runs THEN exit code is 0 AND stdout contains
//! `No` and `baseline`.
//!
//! This test FAILS today because baseline list just prints nothing when empty.
#![cfg(feature = "bin")]

#[test]
fn baseline_list_empty_state_message() {
    let tmp = tempfile::tempdir().expect("create tempdir");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["baseline", "list"])
        .env("XDG_DATA_HOME", tmp.path())
        .output()
        .expect("run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0; stderr: {stderr}\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("No") && stdout.contains("baseline"),
        "expected empty state message; got: '{stdout}'"
    );
}
