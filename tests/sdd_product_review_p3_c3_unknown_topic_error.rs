//! SDD product-review Phase 3, C3.
//! GIVEN `usereport explain totally_unknown_id` WHEN the command runs
//! THEN exit code is non-zero AND error output contains `unknown topic`
//! AND contains `totally_unknown_id`.
//!
//! This test FAILS today because run_explain calls std::process::exit(1)
//! instead of returning Err(miette!(...)).
#![cfg(feature = "bin")]

#[test]
fn unknown_topic_returns_error() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["explain", "totally_unknown_id"])
        .output()
        .expect("run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "expected non-zero exit; stdout: {stdout}\nstderr: {stderr}"
    );
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.to_lowercase().contains("unknown topic"),
        "expected error output to contain 'unknown topic'; got: {combined}"
    );
    assert!(
        combined.contains("totally_unknown_id"),
        "expected error output to contain 'totally_unknown_id'; got: {combined}"
    );
}
