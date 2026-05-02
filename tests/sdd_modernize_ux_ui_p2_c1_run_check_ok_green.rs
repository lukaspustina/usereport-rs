//! Tests for SDD `specs/done/sdd/modernize-ux-ui.md` Phase 2, Criterion 1
//! GIVEN at least one binary is present (status="ok") and is_tty=true
//! WHEN run_check renders the table
//! THEN the "ok" cell value is preceded by \x1b[32m in the captured output.
#![cfg(feature = "bin")]

use usereport::cli::run_check_inner;

#[test]
fn run_check_ok_cell_emits_green_ansi_when_is_tty() {
    // /usr/bin/env is universally present on macOS and Linux.
    let checks = vec![(
        "test-category".to_string(),
        "env".to_string(),
        "/usr/bin/env".to_string(),
    )];

    let mut out: Vec<u8> = Vec::new();
    let missing = run_check_inner(&checks, true, &mut out).expect("run_check_inner should succeed");

    assert_eq!(missing, 0, "expected no missing binaries");

    let rendered = String::from_utf8(out).expect("output is valid UTF-8");
    assert!(
        rendered.contains("\x1b[32m"),
        "expected green ANSI escape \\x1b[32m in output, got: {:?}",
        rendered
    );
}
