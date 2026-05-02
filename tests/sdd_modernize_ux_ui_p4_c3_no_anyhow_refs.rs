//! Tests for SDD `specs/done/sdd/modernize-ux-ui.md` Phase 4, criterion 3.
//! GIVEN Phase 4 is complete WHEN grep for 'anyhow' in src/cli/mod.rs and
//! src/bin/usereport.rs THEN no output.
#![cfg(feature = "bin")]

use std::path::Path;

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read_file(rel: &str) -> String {
    std::fs::read_to_string(Path::new(REPO_ROOT).join(rel)).unwrap_or_else(|e| panic!("failed to read {}: {}", rel, e))
}

#[test]
fn cli_mod_has_no_anyhow_references() {
    let content = read_file("src/cli/mod.rs");
    let lines: Vec<&str> = content.lines().filter(|l| l.contains("anyhow")).collect();
    assert!(
        lines.is_empty(),
        "src/cli/mod.rs must not reference anyhow; found:\n{}",
        lines.join("\n")
    );
}

#[test]
fn bin_usereport_has_no_anyhow_references() {
    let content = read_file("src/bin/usereport.rs");
    let lines: Vec<&str> = content.lines().filter(|l| l.contains("anyhow")).collect();
    assert!(
        lines.is_empty(),
        "src/bin/usereport.rs must not reference anyhow; found:\n{}",
        lines.join("\n")
    );
}
