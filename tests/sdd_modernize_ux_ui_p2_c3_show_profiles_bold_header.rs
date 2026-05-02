//! Tests for SDD `specs/done/sdd/modernize-ux-ui.md` Phase 2, criterion 3:
//! GIVEN any config and is_tty=true WHEN show_profiles_inner renders the table
//! THEN the header row bytes contain \x1b[1m (Bold ANSI attribute).
#![cfg(feature = "bin")]

use std::str::FromStr;
use usereport::cli::config::Config;

const MINIMAL_CONFIG: &str = r#"
[defaults]
timeout = 5

[[profile]]
name = "default"
description = "Standard overview"
commands = ["uname"]

[[command]]
name = "uname"
command = "uname -a"
"#;

#[test]
fn show_profiles_inner_emits_bold_header_when_is_tty_true() {
    let config = Config::from_str(MINIMAL_CONFIG).expect("parse config");
    let mut out: Vec<u8> = Vec::new();
    usereport::cli::show_profiles_inner(&config, true, &mut out);
    let output = String::from_utf8(out).expect("valid utf8");
    assert!(
        output.contains("\x1b[1m"),
        "expected bold ANSI code \\x1b[1m in output, got: {:?}",
        output
    );
}
