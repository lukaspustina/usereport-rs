//! SDD more-useful-firefight Phase 1, C5.
//! GIVEN osx.conf WHEN Config::from_str parses it THEN it returns Ok.
#![cfg(feature = "bin")]

use std::str::FromStr;
use usereport::cli::config::Config;

#[test]
fn osx_conf_parses_ok() {
    let toml_src = include_str!("../contrib/osx.conf");
    Config::from_str(toml_src).expect("osx.conf must parse without error");
}
