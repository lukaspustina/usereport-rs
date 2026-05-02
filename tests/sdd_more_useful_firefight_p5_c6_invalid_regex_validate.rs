//! SDD more-useful-firefight Phase 5, C6.
//! GIVEN extract.pattern = "[" (invalid regex) on command "vmstat"
//! WHEN Config::validate runs
//! THEN it returns Err containing a message with "vmstat" and "[".

use std::str::FromStr;
use usereport::cli::config::Config;

#[test]
fn invalid_regex_pattern_fails_validate_with_command_and_pattern_in_message() {
    let toml_src = r#"
[defaults]
timeout = 5

[[profile]]
name = "default"
commands = ["vmstat"]

[[command]]
name = "vmstat"
command = "/usr/bin/vmstat"

[[command.extract]]
pattern = "["
signal_id = "vmstat.wa_pct"
unit = "pct"
aggregate = "max"
"#;

    let config = Config::from_str(toml_src).expect("TOML parses");
    let result = config.validate();

    assert!(result.is_err(), "validate must return Err for invalid regex");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("vmstat"),
        "error message must mention the command name 'vmstat', got: {msg}"
    );
    assert!(
        msg.contains('['),
        "error message must mention the bad pattern '[', got: {msg}"
    );
}
