//! Tests for SDD modernize-ux-ui Phase 2, criterion 2:
//! GIVEN at least one binary is absent and is_tty=true
//! WHEN run_check renders the table
//! THEN the non-ok cell value is preceded by \x1b[31m in the captured output.
#![cfg(feature = "bin")]

use usereport::cli::run_check_inner;

#[test]
fn run_check_missing_binary_emits_red_ansi_when_tty() {
    let checks = vec![(
        "test-category".to_string(),
        "nonexistent-tool".to_string(),
        "/nonexistent/binary/path".to_string(),
    )];

    let mut out: Vec<u8> = Vec::new();
    let missing = run_check_inner(&checks, true, &mut out)
        .expect("run_check_inner should succeed");

    let output = String::from_utf8(out).expect("output is valid UTF-8");

    assert_eq!(missing, 1, "expected 1 missing binary, got {}", missing);
    assert!(
        output.contains("\x1b[31m"),
        "expected red ANSI escape \\x1b[31m in output, got: {:?}",
        output
    );
}
