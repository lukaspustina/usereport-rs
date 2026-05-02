//! SDD more-useful-firefight Phase 5, C7.
//! GIVEN extract.pattern = "\\d+" with no (?P<val>...) group and aggregate = "max"
//! WHEN Config::validate runs
//! THEN it returns Err containing the command name and the pattern.

use std::str::FromStr;
use usereport::cli::config::Config;

#[test]
fn missing_val_group_in_non_count_pattern_fails_validate() {
    let toml = r#"
[defaults]

[[command]]
name = "vmstat"
command = "vmstat -s"

[[command.extract]]
pattern = "\\d+"
signal_id = "vmstat.wa_pct"
unit = "pct"
aggregate = "max"

[[profile]]
name = "default"
commands = ["vmstat"]
"#;
    let config = Config::from_str(toml).expect("TOML parses");
    let result = config.validate();
    assert!(result.is_err(), "validate must fail for missing (?P<val>...)");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("vmstat"),
        "error must mention command 'vmstat': {err}"
    );
}
