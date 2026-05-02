//! SDD more-useful-firefight Phase 5, C6.
//! GIVEN extract.pattern = "[" (invalid regex) on command vmstat
//! WHEN Config::validate runs
//! THEN it returns Err containing a message with "vmstat" and "[".

use std::str::FromStr;
use usereport::cli::config::Config;

#[test]
fn invalid_regex_in_extract_fails_validate() {
    let toml = r#"
[defaults]

[[command]]
name = "vmstat"
command = "vmstat -s"

[[command.extract]]
pattern = "["
signal_id = "vmstat.wa_pct"
unit = "pct"
aggregate = "max"

[[profile]]
name = "default"
commands = ["vmstat"]
"#;
    let config = Config::from_str(toml).expect("TOML parses");
    let result = config.validate();
    assert!(result.is_err(), "validate must fail for invalid regex");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("vmstat"), "error must mention command 'vmstat': {err}");
    assert!(err.contains('['), "error must mention pattern '[': {err}");
}
