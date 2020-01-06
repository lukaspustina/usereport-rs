use crate::command::Command;

use serde::Deserialize;
use snafu::{ResultExt, Snafu};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Failed to parse Config
    #[snafu(display("failed to parse config: {}", source))]
    ParsingFailed { source: toml::de::Error },
    /// Failed to read file
    #[snafu(display("failed to read file config '{}': {}", path.display(), source))]
    ReadFileFailed { path: PathBuf, source: std::io::Error },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Config {
    pub defaults: Defaults,
    pub hostinfo: Option<Hostinfo>,
    #[serde(rename = "profile")]
    pub profiles: Vec<Profile>,
    #[serde(rename = "command")]
    pub commands: Vec<Command>,
}

impl FromStr for Config {
    type Err = Error;

    fn from_str(toml: &str) -> Result<Config> {
        let config: Config = toml::from_str(toml).context(ParsingFailed {})?;
        Ok(config)
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let mut file = File::open(path.as_ref()).context(ReadFileFailed {
            path: path.as_ref().to_path_buf(),
        })?;
        let mut toml = String::new();
        file.read_to_string(&mut toml).context(ReadFileFailed {
            path: path.as_ref().to_path_buf(),
        })?;
        Config::from_str(&toml)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Defaults {
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_repetitions")]
    pub repetitions: u64,
    #[serde(default = "default_max_parallel_commands")]
    pub max_parallel_commands: u64,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            profile: "default".to_string(),
            timeout: 5,
            repetitions: 1,
            max_parallel_commands: 64,
        }
    }
}

fn default_profile() -> String { Defaults::default().profile }

fn default_timeout() -> u64 { Defaults::default().timeout }

fn default_repetitions() -> u64 { Defaults::default().repetitions }

fn default_max_parallel_commands() -> u64 { Defaults::default().max_parallel_commands }

#[derive(Debug, Deserialize, PartialEq)]
pub struct Hostinfo {
    pub commands: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Profile {
    pub name: String,
    pub commands: Vec<String>,
    pub description: Option<String>,
}

impl Profile{
    pub fn new<T: Into<String> + Clone>(name: T, commands: &[T]) -> Profile {
        Self::with_description(name, commands, None)
    }

    pub fn with_description<T: Into<String> + Clone>(name: T, commands: &[T], description: Option<T>) -> Profile {
        let name = name.into();
        let commands = commands.iter().map(|x| x.clone().into()).collect();
        let description = description.map(Into::into);

        Profile {
            name,
            commands,
            description,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use spectral::prelude::*;

    #[test]
    fn config_read_ok() {
        let config_txt = r#"
[defaults]
timeout = 5

[[profile]]
name = "default"
commands = ["uname"]

[[command]]
name = "uname"
title = "Host OS"
description = "Basic host OS information"
command = "/usr/bin/uname -a"
timeout = 1

"#;
        let defaults = Defaults { timeout: 5, ..Defaults::default() };
        let mut profiles = Vec::new();
        profiles.push(
            Profile::new("default", &["uname"])
        );
        let mut commands = Vec::new();
        commands.push(
            Command::new("uname", "/usr/bin/uname -a", 1)
                .title("Host OS")
                .description("Basic host OS information")
        );
        let expected = Config { defaults, hostinfo: None, profiles, commands };

        let config = Config::from_str(config_txt);

        asserting("Reading config from toml")
            .that(&config)
            .is_ok()
            .is_equal_to(&expected);
    }

    #[test]
    fn config_file_ok() {
        #[cfg(target_os = "macos")]
        let path = "contrib/osx.conf";
        #[cfg(target_os = "linux")]
        let path = "contrib/linux.conf";

        let config = Config::from_file(path);

        asserting("Reading config from file")
            .that(&config)
            .is_ok()
            .map(|x| &x.commands)
            .has_length(3)
    }
}
