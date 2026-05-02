//! SDD product-review Phase 4, C6.
//! GIVEN `usereport --output markdown --redact` WHEN the command runs THEN
//! stderr contains a warning referencing `--redact` and stating it has no
//! effect without `--output llm`.
//!
//! This test FAILS today because no warning is emitted.
#![cfg(feature = "bin")]

#[test]
fn redact_warning_when_output_not_llm() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--output", "markdown", "--redact"])
        .output()
        .expect("run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--redact"),
        "expected stderr to mention '--redact'; got: {stderr}"
    );
    assert!(
        stderr.contains("no effect") || stderr.contains("llm"),
        "expected warning to mention 'no effect' or 'llm'; got: {stderr}"
    );
}
