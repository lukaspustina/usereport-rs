//! SDD product-review Phase 4, C4.
//! GIVEN `usereport --output xyz` WHEN the command runs THEN exit code is
//! non-zero AND stderr contains at least one of `markdown`, `html`, `json`,
//! `template`, `llm`.
#![cfg(feature = "bin")]

#[test]
fn output_invalid_lists_valid_values() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--output", "xyz"])
        .output()
        .expect("run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected non-zero exit; stderr: {stderr}");
    let has_valid_value = ["markdown", "html", "json", "template", "llm"]
        .iter()
        .any(|v| stderr.contains(v));
    assert!(
        has_valid_value,
        "expected stderr to list valid output types; got: {stderr}"
    );
}
