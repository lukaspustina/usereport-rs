//! SDD more-useful-firefight Phase 5, C7.
//! GIVEN extract.pattern = "\\d+" with no (?P<val>...) group and aggregate = "max"
//! WHEN Config::validate runs
//! THEN it returns Err containing the command name and the pattern.

use std::str::FromStr;
use usereport::cli::config::Config;

#[test]
fn missing_val_group_fails_validate_with_command_and_pattern_in_message() {
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
pattern = "\\d+"
signal_id = "vmstat.wa_pct"
unit = "pct"
aggregate = "max"
"#;

    let config = Config::from_str(toml_src).expect("TOML parses");
    let result = config.validate();

    assert!(
        result.is_err(),
        "validate must return Err when (?P<val>...) is absent for non-Count aggregate"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("vmstat"),
        "error message must mention the command name 'vmstat', got: {msg}"
    );
    assert!(
        msg.contains(r"\d+"),
        "error message must mention the pattern '\\d+', got: {msg}"
    );
}
