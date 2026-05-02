//! SDD more-useful-firefight Phase 1, C6.
//! GIVEN linux.conf WHEN Config::from_str parses it THEN it returns Ok.
#![cfg(feature = "bin")]

use std::str::FromStr;
use usereport::cli::config::Config;

#[test]
fn linux_conf_parses_ok() {
    let toml_src = include_str!("../contrib/linux.conf");
    Config::from_str(toml_src).expect("linux.conf must parse without error");
}
