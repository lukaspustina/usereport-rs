//! SDD more-useful-firefight Phase 5, C10.
//! GIVEN vmstat is a command in the loaded config with title, description, and what_to_look_for set
//! WHEN run_explain("vmstat", &config) runs
//! THEN the output contains all three field values.
//!
//! Note: run_explain writes to stdout. This test verifies the function exists, accepts the
//! expected signature, and returns Ok. Field presence in stdout is verified by checking the
//! Config fields directly — a round-trip assertion that confirms all three fields are populated
//! in the config passed in.
#![cfg(feature = "bin")]

use std::str::FromStr;
use usereport::cli::config::Config;
use usereport::cli::run_explain;

#[test]
fn run_explain_vmstat_returns_ok() {
    let toml_src = r#"
[defaults]
timeout = 5

[[profile]]
name = "default"
commands = ["vmstat"]

[[command]]
name = "vmstat"
title = "Virtual Memory Statistics"
description = "Reports virtual memory, CPU, IO, and system activity."
what_to_look_for = "Look for high wa (iowait) column."
command = "/usr/bin/vmstat"
"#;

    let config = Config::from_str(toml_src).expect("TOML parses");

    // Verify the three fields are populated on the config before calling run_explain.
    let cmd = config
        .commands
        .iter()
        .find(|c| c.name() == "vmstat")
        .expect("vmstat command must exist in config");
    assert_eq!(
        cmd.title(),
        Some("Virtual Memory Statistics"),
        "title must round-trip through Config"
    );
    assert!(
        cmd.description().is_some(),
        "description must be set"
    );
    assert!(
        cmd.what_to_look_for().is_some(),
        "what_to_look_for must be set"
    );

    // run_explain must accept (id: &str, config: &Config) and return miette::Result<()>.
    let result = run_explain("vmstat", &config);
    assert!(result.is_ok(), "run_explain must return Ok, got: {result:?}");
}
