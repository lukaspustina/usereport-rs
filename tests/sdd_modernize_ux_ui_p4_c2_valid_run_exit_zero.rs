//! Tests for SDD `specs/done/sdd/modernize-ux-ui.md` Phase 4, criterion 2.
//! GIVEN a valid config and valid profile WHEN the binary runs to completion THEN
//! exit code is 0.
#![cfg(feature = "bin")]

use std::path::Path;

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[test]
#[cfg(target_os = "macos")]
fn valid_osx_config_exits_zero() {
    let conf = Path::new(REPO_ROOT).join("contrib/osx.conf");
    let bin = env!("CARGO_BIN_EXE_usereport");
    let output = std::process::Command::new(bin)
        .args(["--config", conf.to_str().unwrap()])
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "expected exit 0 for valid config; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg(target_os = "linux")]
fn valid_linux_config_exits_zero() {
    let conf = Path::new(REPO_ROOT).join("contrib/linux.conf");
    let bin = env!("CARGO_BIN_EXE_usereport");
    let output = std::process::Command::new(bin)
        .args(["--config", conf.to_str().unwrap()])
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "expected exit 0 for valid config; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
