//! Tests for SDD `specs/done/sdd/modernize-ux-ui.md` Phase 4, criterion 1.
//! GIVEN --config /nonexistent/path.toml WHEN the binary exits THEN exit code is
//! non-zero and stderr contains the error message text.
#![cfg(feature = "bin")]

#[test]
fn bad_config_path_exits_nonzero_with_error_message() {
    let bin = env!("CARGO_BIN_EXE_usereport");
    let output = std::process::Command::new(bin)
        .args(["--config", "/nonexistent/path/that/does/not/exist.toml"])
        .output()
        .expect("run binary");

    assert!(
        !output.status.success(),
        "expected non-zero exit code for missing config"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "expected error message in stderr, got empty"
    );
    // miette fancy format includes rich box-drawing (×, ├─▶, ╰─▶) not a bare "Error: msg" line
    let has_rich_format = stderr.contains('×') || stderr.contains("├─▶") || stderr.contains("╰─▶");
    assert!(
        has_rich_format,
        "expected miette rich-format error (× or ├─▶ or ╰─▶) in stderr, got: {:?}",
        stderr
    );
}
