//! SDD more-useful-firefight Phase 1, C2.
//! GIVEN a [[command]] block with install_hint and what_to_look_for
//! WHEN Config::from_str parses it THEN both fields deserialize to their string values.
#![cfg(feature = "bin")]

use std::str::FromStr;
use usereport::cli::config::Config;

const TOML_SRC: &str = r#"
[defaults]
timeout = 5

[[profile]]
name = "default"
commands = ["sar_cpu"]

[[command]]
name = "sar_cpu"
command = "sar -u 1 5"
install_hint = "apt-get install sysstat"
what_to_look_for = "look for high wa"
"#;

#[test]
fn command_install_hint_deserializes() {
    let config = Config::from_str(TOML_SRC).expect("must parse");
    let cmd = config
        .commands
        .iter()
        .find(|c| c.name() == "sar_cpu")
        .expect("sar_cpu must be present");
    assert_eq!(cmd.install_hint(), Some("apt-get install sysstat"));
}

#[test]
fn command_what_to_look_for_deserializes() {
    let config = Config::from_str(TOML_SRC).expect("must parse");
    let cmd = config
        .commands
        .iter()
        .find(|c| c.name() == "sar_cpu")
        .expect("sar_cpu must be present");
    assert_eq!(cmd.what_to_look_for(), Some("look for high wa"));
}
