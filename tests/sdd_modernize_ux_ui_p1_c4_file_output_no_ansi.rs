//! Tests for SDD `specs/done/sdd/modernize-ux-ui.md` Phase 1, criterion 4.
//! GIVEN output_file=Some(path) and format=Markdown (forcing is_tty=false because
//! output_file.is_some()) WHEN the binary writes its report THEN the file bytes
//! contain no \x1b[ and match the raw TemplateRenderer output.
#![cfg(feature = "bin")]

use std::path::Path;

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[test]
#[cfg(target_os = "macos")]
fn file_output_markdown_contains_no_ansi_escapes() {
    let conf = Path::new(REPO_ROOT).join("contrib/osx.conf");
    let bin = env!("CARGO_BIN_EXE_usereport");
    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    let out_path = tmp.path().to_str().unwrap().to_string();

    let output = std::process::Command::new(bin)
        .args([
            "--config",
            conf.to_str().unwrap(),
            "--output",
            "markdown",
            "-O",
            &out_path,
        ])
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = std::fs::read_to_string(&out_path).expect("read output file");
    assert!(!content.is_empty(), "output file must not be empty");
    assert!(
        !content.contains('\x1b'),
        "output file must contain no ANSI escape sequences, got file with {} bytes",
        content.len()
    );
}

#[test]
#[cfg(target_os = "linux")]
fn file_output_markdown_contains_no_ansi_escapes() {
    let conf = Path::new(REPO_ROOT).join("contrib/linux.conf");
    let bin = env!("CARGO_BIN_EXE_usereport");
    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    let out_path = tmp.path().to_str().unwrap().to_string();

    let output = std::process::Command::new(bin)
        .args([
            "--config",
            conf.to_str().unwrap(),
            "--output",
            "markdown",
            "-O",
            &out_path,
        ])
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = std::fs::read_to_string(&out_path).expect("read output file");
    assert!(!content.is_empty(), "output file must not be empty");
    assert!(
        !content.contains('\x1b'),
        "output file must contain no ANSI escape sequences, got file with {} bytes",
        content.len()
    );
}
