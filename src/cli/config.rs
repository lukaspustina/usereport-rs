use crate::command::Command;

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    iter::FromIterator,
    path::{Path, PathBuf},
    str::FromStr,
};

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Failed to parse Config
    #[snafu(display("failed to parse config: {}", source))]
    ParseConfigFailed { source: toml::de::Error },
    /// Failed to read file
    #[snafu(display("failed to read file config '{}': {}", path.display(), source))]
    ReadConfigFileFailed { path: PathBuf, source: std::io::Error },
    /// Configuration is invalid
    #[snafu(display("configuration is invalid because {}", reason))]
    InvalidConfig { reason: &'static str },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
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
        let config: Config = toml::from_str(toml).context(ParseConfigFailed {})?;
        let config = config.populate_defaults();
        Ok(config)
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let mut file = File::open(path.as_ref()).context(ReadConfigFileFailed {
            path: path.as_ref().to_path_buf(),
        })?;
        let mut toml = String::new();
        file.read_to_string(&mut toml).context(ReadConfigFileFailed {
            path: path.as_ref().to_path_buf(),
        })?;
        Config::from_str(&toml)
    }

    pub fn profile(&self, profile_name: &str) -> Result<&Profile> {
        self.profiles.iter().find(|x| x.name == profile_name).ok_or_else(|| {
            Error::InvalidConfig {
                reason: "no such profile",
            }
        })
    }

    pub fn commands_for_hostinfo(&self) -> Vec<Command> {
        if let Some(hostinfo) = &self.hostinfo {
            self.commands
                .clone()
                .into_iter()
                .filter(|c| hostinfo.commands.contains(&c.name))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn commands_for_profile(&self, profile: &Profile) -> Vec<Command> {
        self.commands
            .clone()
            .into_iter()
            .filter(|c| profile.commands.contains(&c.name))
            .collect()
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_host_info()?;
        self.validate_default_profile()?;
        self.validate_profiles_commands()?;

        Ok(())
    }

    fn validate_host_info(&self) -> Result<()> {
        let command_names: HashSet<&String> = HashSet::from_iter(self.commands.iter().map(|x| &x.name));

        if let Some(ref hostinfo) = self.hostinfo {
            for c in &hostinfo.commands {
                command_names.get(c).ok_or_else(|| {
                    Error::InvalidConfig {
                        reason: "hostinfo command not found",
                    }
                })?;
            }
        }

        Ok(())
    }

    fn validate_default_profile(&self) -> Result<()> {
        let p_name = self.defaults.profile.as_ref();
        self.profile(p_name).map(|_| ())
    }

    fn validate_profiles_commands(&self) -> Result<()> {
        let command_names: HashSet<&String> = HashSet::from_iter(self.commands.iter().map(|x| &x.name));

        for p in &self.profiles {
            for c in &p.commands {
                command_names.get(c).ok_or_else(|| {
                    Error::InvalidConfig {
                        reason: "profile command not found",
                    }
                })?;
            }
        }

        Ok(())
    }

    fn populate_defaults(self) -> Self {
        let timeout = self.defaults.timeout;
        self.populate_commands_timeout(timeout)
    }

    fn populate_commands_timeout(self, timeout: u64) -> Self {
        let Config {
            defaults,
            hostinfo,
            profiles,
            commands,
        } = self;
        let commands = commands
            .into_iter()
            .map(|x| {
                if x.timeout_sec.is_none() {
                    x.with_timeout(timeout)
                } else {
                    x
                }
            })
            .collect();

        Config {
            defaults,
            hostinfo,
            profiles,
            commands,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Defaults {
    #[serde(default = "default_profile")]
    pub profile:               String,
    #[serde(default = "default_timeout")]
    pub timeout:               u64,
    #[serde(default = "default_repetitions")]
    pub repetitions:           usize,
    #[serde(default = "default_max_parallel_commands")]
    pub max_parallel_commands: usize,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            profile:               "default".to_string(),
            timeout:               5,
            repetitions:           1,
            max_parallel_commands: 64,
        }
    }
}

fn default_profile() -> String { Defaults::default().profile }

fn default_timeout() -> u64 { Defaults::default().timeout }

fn default_repetitions() -> usize { Defaults::default().repetitions }

fn default_max_parallel_commands() -> usize { Defaults::default().max_parallel_commands }

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Hostinfo {
    pub commands: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Profile {
    pub name:        String,
    pub commands:    Vec<String>,
    pub description: Option<String>,
}

impl Profile {
    pub fn new<T: Into<String> + Clone>(name: T, commands: &[T]) -> Profile {
        Self::new_with_description(name, commands, None)
    }

    pub fn new_with_description<T: Into<String> + Clone, S: Into<Option<T>>>(
        name: T,
        commands: &[T],
        description: S,
    ) -> Profile {
        let name = name.into();
        let commands = commands.to_vec().into_iter().map(Into::into).collect();
        let description = description.into().map(Into::into);

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
    fn config_read_from_str_ok() {
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
        let defaults = Defaults {
            timeout: 5,
            ..Defaults::default()
        };
        let mut profiles = Vec::new();
        profiles.push(Profile::new("default", &["uname"]));
        let mut commands = Vec::new();
        commands.push(
            Command::new("uname", "/usr/bin/uname -a")
                .with_title("Host OS")
                .with_description("Basic host OS information")
                .with_timeout(1),
        );
        let expected = Config {
            defaults,
            hostinfo: None,
            profiles,
            commands,
        };

        let config = Config::from_str(config_txt);

        asserting("Reading config from toml")
            .that(&config)
            .is_ok()
            .is_equal_to(&expected);
    }

    #[test]
    fn config_read_from_file_ok() {
        #[cfg(target_os = "macos")]
        let path = "contrib/osx.conf";
        #[cfg(target_os = "linux")]
        let path = "contrib/linux.conf";

        let config = Config::from_file(path);

        asserting("Reading config from file")
            .that(&config)
            .is_ok()
            .map(|x| &x.commands)
            .has_length(4)
    }

    #[test]
    fn config_invalid_default_profile() {
        let config_txt = r#"
[defaults]
timeout = 5

[[profile]]
name = "not-the-default"
commands = ["uname"]

[[command]]
name = "uname"
title = "Host OS"
description = "Basic host OS information"
command = "/usr/bin/uname -a"
timeout = 1

"#;
        let config = Config::from_str(config_txt).expect("syntax ok");
        let validation = config.validate();

        asserting("validating config").that(&validation).is_err();
    }

    #[test]
    fn config_invalid_profile_commands() {
        let config_txt = r#"
[defaults]
timeout = 5

[[profile]]
name = "default"
commands = ["unam"]

[[command]]
name = "uname"
title = "Host OS"
description = "Basic host OS information"
command = "/usr/bin/uname -a"
timeout = 1

"#;
        let config = Config::from_str(config_txt).expect("syntax ok");
        let validation = config.validate();

        asserting("validating config").that(&validation).is_err();
    }

    #[test]
    fn config_invalid_hostinfo_commands() {
        let config_txt = r#"
[defaults]
timeout = 5

[hostinfo]
commands = ["unam"]

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
        let config = Config::from_str(config_txt).expect("syntax ok");
        let validation = config.validate();

        asserting("validating config").that(&validation).is_err();
    }

    #[test]
    fn config_populate_defaults_ok() {
        let config_txt = r#"
[defaults]
timeout = 5

[[profile]]
name = "not-the-default"
commands = ["uname"]

[[command]]
name = "uname"
title = "Host OS"
description = "Basic host OS information"
command = "/usr/bin/uname -a"
"#;
        let config = Config::from_str(config_txt).expect("syntax ok");

        asserting("default timeout set")
            .that(&config.commands.first())
            .is_some()
            .map(|x| &x.timeout_sec)
            .is_some()
            .is_equal_to(5);
    }
}
