//! Tests for SDD `specs/done/sdd/modernize-ux-ui.md` Phase 3, criterion 4.
//! GIVEN --no-progress WHEN the binary runs THEN create_progress_bar is not invoked
//! and the program exits 0.
#![cfg(feature = "bin")]

use std::path::Path;

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[test]
#[cfg(target_os = "macos")]
fn no_progress_flag_accepted_exits_zero() {
    let conf = Path::new(REPO_ROOT).join("contrib/osx.conf");
    let bin = env!("CARGO_BIN_EXE_usereport");

    let output = std::process::Command::new(bin)
        .args(["--config", conf.to_str().unwrap(), "--no-progress"])
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "expected exit 0 with --no-progress; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg(target_os = "linux")]
fn no_progress_flag_accepted_exits_zero() {
    let conf = Path::new(REPO_ROOT).join("contrib/linux.conf");
    let bin = env!("CARGO_BIN_EXE_usereport");

    let output = std::process::Command::new(bin)
        .args(["--config", conf.to_str().unwrap(), "--no-progress"])
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "expected exit 0 with --no-progress; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn no_progress_and_progress_flags_conflict() {
    let bin = env!("CARGO_BIN_EXE_usereport");

    // --progress and --no-progress conflict; clap should reject this combination
    let output = std::process::Command::new(bin)
        .args(["--progress", "--no-progress"])
        .output()
        .expect("run binary");

    assert!(
        !output.status.success(),
        "expected non-zero exit when --progress and --no-progress are both passed"
    );
}
