//! SDD more-useful-firefight Phase 5, C10.
//! GIVEN vmstat is a command in the loaded config with title, description, what_to_look_for set,
//!   and links populated
//! WHEN run_explain("vmstat", &config) runs
//! THEN the output contains the title, description, what_to_look_for, and the link URL.

use std::str::FromStr;
use usereport::cli::config::Config;
use usereport::cli::run_explain_command;

#[test]
fn run_explain_renders_command_title_description_wtlf_and_links() {
    let toml = r#"
[defaults]

[[command]]
name = "vmstat"
command = "vmstat -s"
title = "Virtual Memory Statistics"
description = "Shows virtual memory, CPU, IO, and system activity"
what_to_look_for = "Look for high wa (iowait) column"

[[command.links]]
name = "vmstat manpage"
url = "https://example.com/vmstat"

[[profile]]
name = "default"
commands = ["vmstat"]
"#;
    let config = Config::from_str(toml).expect("TOML parses");
    let cmd = config
        .commands
        .iter()
        .find(|c| c.name() == "vmstat")
        .expect("vmstat must be in config");
    let mut buf: Vec<u8> = Vec::new();
    run_explain_command(cmd, false, &mut buf).expect("run_explain_command must succeed");
    let output = String::from_utf8(buf).unwrap();
    assert!(
        output.contains("Virtual Memory Statistics"),
        "output must contain title: {output}"
    );
    assert!(
        output.contains("Shows virtual memory"),
        "output must contain description: {output}"
    );
    assert!(
        output.contains("Look for high wa"),
        "output must contain what_to_look_for: {output}"
    );
    assert!(
        output.contains("https://example.com/vmstat"),
        "output must contain link URL: {output}"
    );
}
