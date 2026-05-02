//! SDD product-review Phase 2, C4.
//! GIVEN `usereport --output xyz` WHEN the command runs THEN exit code is
//! non-zero AND stderr contains each of `template`, `html`, `json`,
//! `markdown`, `llm` AND does NOT contain the word `text`.
//!
//! This test FAILS today because OutputType has no Text variant so clap's
//! error message for --output currently lists the existing variants; once
//! Text is added the test verifies it is excluded from the main command.
#![cfg(feature = "bin")]

#[test]
fn root_output_xyz_excludes_text_from_suggestions() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["--output", "xyz"])
        .output()
        .expect("run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr: {stderr}"
    );
    for expected in ["template", "html", "json", "markdown", "llm"] {
        assert!(
            stderr.contains(expected),
            "expected stderr to list '{expected}' as valid value; stderr: {stderr}"
        );
    }
    assert!(
        !stderr.contains("text"),
        "expected 'text' NOT to appear in main --output suggestions (text is diff-only); stderr: {stderr}"
    );
}
