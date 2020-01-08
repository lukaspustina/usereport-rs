use core::fmt;
use log::{debug, trace};
use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use snafu::{ResultExt, Snafu};
use std::{io::Read, time::Duration};
use subprocess::{Popen, PopenConfig, PopenError, Redirection};

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Process creation or execution failed
    #[snafu(display("failed to run command {}: {}", name, source))]
    ProcessFailed { name: String, source: PopenError },
    /// Process could not be killed
    #[snafu(display("failed to kill command {}: {}", name, source))]
    KillFailed { name: String, source: std::io::Error },
    /// Waiting for process termination failed
    #[snafu(display("failed to wait for command {}: {}", name, source))]
    WaitFailed { name: String, source: PopenError },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Run a CLI command and store its stdout.
///
/// # Example
/// ```
/// # use usereport::command::{Command, CommandResult};
/// let command = Command::new("uname", r#"/usr/bin/uname -a"#, 5).set_title("Host OS");
/// match command.exec() {
///     Ok(CommandResult::Success {
///         command: _,
///         stdout: stdout,
///     }) => println!("Command output '{}'", stdout),
///     Ok(CommandResult::Failed { command: _ }) => println!("Command failed"),
///     Ok(CommandResult::Timeout { command: _ }) => println!("Command timed out"),
///     _ => println!("Command execution failed"),
/// };
/// ```
#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
pub struct Command {
    pub(crate) name:        String,
    pub(crate) title:       Option<String>,
    pub(crate) description: Option<String>,
    #[serde(rename = "command", deserialize_with = "de_ser_args", serialize_with = "ser_args")]
    pub(crate) args:        Vec<String>,
    #[serde(rename = "timeout")]
    pub(crate) timeout_sec: u64,
}

fn de_ser_args<'de, D>(deserializer: D) -> ::std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct ArgsVisitor;

    impl<'a> Visitor<'a> for ArgsVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("valid command string")
        }

        fn visit_str<E>(self, s: &str) -> ::std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            let args: Vec<_> = s.split(' ').map(|x| x.into()).collect();
            Ok(args)
        }
    }

    deserializer.deserialize_string(ArgsVisitor)
}

pub fn ser_args<S>(args: &[String], serializer: S) -> ::std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let command: String = args.join(" ");
    serializer.serialize_str(&command)
}

impl Command {
    /// Create new command with default values
    pub fn new<T: Into<String>>(name: T, command: T, timeout_sec: u64) -> Command {
        let args: Vec<_> = command.into().split(' ').map(|x| x.into()).collect();
        assert!(!args.is_empty());

        Command {
            name: name.into(),
            title: None,
            description: None,
            args,
            timeout_sec,
        }
    }

    /// Get name of command
    pub fn name(&self) -> &str { &self.name }

    /// Get command args
    pub fn args(&self) -> &[String] { self.args.as_slice() }

    /// Get title of command
    pub fn title(&self) -> Option<&str> { self.title.as_ref().map(|x| x.as_str()) }

    /// Get description of command
    pub fn description(&self) -> Option<&str> { self.description.as_ref().map(|x| x.as_str()) }

    /// Set title of command
    pub fn set_title<T: Into<String>>(self, title: T) -> Command {
        Command {
            title: Some(title.into()),
            ..self
        }
    }

    /// Set description of command
    pub fn set_description<T: Into<String>>(self, description: T) -> Command {
        Command {
            description: Some(description.into()),
            ..self
        }
    }

    /// Execute this command
    pub fn exec(self) -> Result<CommandResult> {
        let mut p = Popen::create(
            &self.args,
            PopenConfig {
                stdout: Redirection::Pipe,
                ..Default::default()
            },
        )
        .context(ProcessFailed {
            name: self.name.clone(),
        })?;
        debug!("Running '{:?}' as '{:?}'", self.args, p);

        match p.wait_timeout(Duration::new(self.timeout_sec, 0)) {
            Ok(Some(status)) if status.success() => {
                trace!("process successfully finished as {:?}", status);
                let mut stdout = String::new();
                let _ = p.stdout.as_ref().unwrap().read_to_string(&mut stdout); // TODO: unwrap is unsafe
                debug!("stdout '{}'", stdout);

                Ok(CommandResult::Success { command: self, stdout })
            }
            Ok(Some(status)) => {
                trace!("process successfully finished as {:?}", status);
                Ok(CommandResult::Failed { command: self })
            }
            Ok(None) => {
                trace!("process timed out and will be killed");
                self.terminate(&mut p)?;
                Ok(CommandResult::Timeout { command: self })
            }
            err => {
                trace!("process failed '{:?}'", err);
                self.terminate(&mut p)?;
                Ok(CommandResult::Error { command: self })
            }
        }
    }

    fn terminate(&self, p: &mut Popen) -> Result<()> {
        p.kill().context(KillFailed {
            name: self.name.clone(),
        })?;
        p.wait().context(WaitFailed {
            name: self.name.clone(),
        })?;
        trace!("process killed");
        Ok(())
    }
}

/// Encapsulates a command execution result
#[derive(Debug, PartialEq, Serialize)]
pub enum CommandResult {
    /// `Command` has been executed successfully and `String` contains stdout.
    Success { command: Command, stdout: String },
    /// `Command` failed to execute
    Failed { command: Command },
    /// `Command` execution exceeded specified timeout
    Timeout { command: Command },
    /// `Command` could not be executed
    Error { command: Command },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;

    use spectral::prelude::*;

    #[test]
    fn execution_ok() {
        init();

        #[cfg(target_os = "macos")]
        let command = Command::new("uname", r#"/usr/bin/uname -a"#, 5);
        #[cfg(target_os = "macos")]
        let expected = "Darwin";
        #[cfg(target_os = "linux")]
        let command = Command::new("uname", r#"/bin/uname -a"#, 5);
        #[cfg(target_os = "linux")]
        let expected = "Linux";

        let res = command.exec();

        asserting("executing command successfully")
            .that(&res)
            .is_ok()
            .is_success_contains(expected)
    }

    #[test]
    fn execution_failed() {
        init();

        #[cfg(target_os = "macos")]
        let command = Command::new("false", r#"/usr/bin/false"#, 1);
        #[cfg(target_os = "linux")]
        let command = Command::new("false", r#"/bin/false"#, 1);

        let res = command.exec();

        asserting("executing command successfully")
            .that(&res)
            .is_ok()
            .is_failed();
    }

    #[test]
    fn execution_timeout() {
        init();

        let command = Command::new("sleep", r#"/bin/sleep 5"#, 1);

        let res = command.exec();

        asserting("executing command successfully")
            .that(&res)
            .is_ok()
            .is_timeout();
    }

    #[test]
    fn execution_error() {
        init();

        let command = Command::new("no_such_command", r#"/no_such_command"#, 1);

        let res = command.exec();

        asserting("executing command errors").that(&res).is_err();
    }
}
