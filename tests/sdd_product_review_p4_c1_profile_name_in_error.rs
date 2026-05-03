//! SDD product-review Phase 4, C1.
//! GIVEN a config that defines no profile named `nonexistent` WHEN
//! `usereport --profile nonexistent` runs THEN exit code is non-zero AND
//! stderr contains both `profile` and `nonexistent`.
//!
//! This test FAILS today because the error just says "no such profile"
//! without the profile name. It will pass once NoSuchProfile { name } is used.
#![cfg(feature = "bin")]

#[test]
fn profile_name_appears_in_error() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--profile", "nonexistent"])
        .output()
        .expect("run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected non-zero exit; stderr: {stderr}");
    assert!(
        stderr.to_lowercase().contains("profile"),
        "expected stderr to contain 'profile'; got: {stderr}"
    );
    assert!(
        stderr.contains("nonexistent"),
        "expected stderr to contain 'nonexistent'; got: {stderr}"
    );
}
