//! SDD more-useful-firefight Phase 1, C1.
//! GIVEN linux.conf WHEN Config::from_str parses it THEN it returns Ok
//! and every command with install_hint set has a non-empty string value.
#![cfg(feature = "bin")]

use std::str::FromStr;
use usereport::cli::config::Config;

#[test]
fn linux_conf_parses_ok_and_install_hints_nonempty() {
    let toml_src = include_str!("../contrib/linux.conf");
    let config = Config::from_str(toml_src).expect("linux.conf must parse");
    for cmd in &config.commands {
        if let Some(hint) = cmd.install_hint() {
            assert!(
                !hint.is_empty(),
                "install_hint for command '{}' must not be empty",
                cmd.name()
            );
        }
    }
}
