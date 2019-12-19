use log::{debug,trace};
use serde::{Deserialize, Deserializer, de};
use std::time::Duration;
use std::io::Read;
use snafu::{ResultExt, Snafu};
use subprocess::{Popen, PopenConfig, PopenError, Redirection};
use serde::de::Visitor;
use core::fmt;

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Process creation or execution failed
    #[snafu(display("Failed to run command {}: {}", name, source))]
    ProcessFailed { name: String, source: PopenError },
    /// Process could not be killed
    #[snafu(display("Failed to kill command {}: {}", name, source))]
    KillFailed { name: String, source: std::io::Error },
    /// Waiting for process termination failed
    #[snafu(display("Failed to wait for command {}: {}", name, source))]
    WaitFailed { name: String, source: PopenError },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Run a CLI command and store its stdout.
///
/// # Example
/// ```
/// # use usereport::command::{Command, CommandResult};
/// let command = Command::new("uname", r#"/usr/bin/uname -a"#, 5)
///     .title("Host OS")
///     .run_by_default(false);
/// match command.exec() {
///     Ok(CommandResult::Success(stdout)) => println!("Command output '{}'", stdout),
///     Ok(CommandResult::Failed) => println!("Command failed"),
///     Ok(CommandResult::Timeout) => println!("Command timed out"),
///     _ => println!("Command execution failed"),
/// };
/// ```
#[derive(Debug, Deserialize, PartialEq)]
pub struct Command {
    pub(crate) name: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    #[serde(rename = "command", deserialize_with = "de_ser_args")]
    pub(crate) args: Vec<String>,
    #[serde(rename = "timeout")]
    pub(crate) timeout_sec: u64,
    pub(crate) default_run: bool,
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

        fn visit_str<E>(self, s: &str) -> ::std::result::Result<Self::Value, E> where E: de::Error {
            let args: Vec<_> = s.split(' ').map(|x| x.into()).collect();
            Ok(args)
        }
    }

    deserializer.deserialize_string(ArgsVisitor)
}

impl Command {
    /// Create new command with default values
    pub fn new<T: Into<String>>(name: T, command: T, timeout_sec: u64) -> Command {
        let args: Vec<_> = command.into().split(' ').map(|x| x.into()).collect();
        assert!(args.len() > 0);

        Command {
            name: name.into(),
            title: None,
            description: None,
            args,
            timeout_sec,
            default_run: true,
        }
    }

    /// Set title of command
    pub fn title<T: Into<String>>(self, title: T) -> Command {
        Command {
            title: Some(title.into()),
            ..self
        }
    }

    /// Set description of command
    pub fn description<T: Into<String>>(self, description: T) -> Command {
        Command {
            description: Some(description.into()),
            ..self
        }
    }

    /// Set whether to run this command by default
    pub fn run_by_default(self, value: bool) -> Command {
        Command {
            default_run: value,
            ..self
        }
    }

    /// Execute this command
    pub fn exec(&self) -> Result<CommandResult> {
        let mut p = Popen::create(&self.args, PopenConfig {
                stdout: Redirection::Pipe, ..Default::default()
            }).context(ProcessFailed {name: self.name.clone()})?;
        debug!("Running '{:?}' as '{:?}'", self.args, p);


        match p.wait_timeout(Duration::new(self.timeout_sec, 0)) {
            Ok(Some(status)) if status.success() => {
                trace!("process successfully finished as {:?}", status);
                let mut stdout = String::new();
                let _ = p.stdout.as_ref().unwrap().read_to_string(&mut stdout); // TODO: unwrap is unsafe
                debug!("stdout '{}'", stdout);

                Ok(CommandResult::Success(stdout))
            }
            Ok(Some(status)) => {
                trace!("process successfully finished as {:?}", status);
                Ok(CommandResult::Failed)
            }
            Ok(None) => {
                trace!("process timed out and will be killed");
                self.terminate(&mut p)?;
                Ok(CommandResult::Timeout)
            }
            err => {
                trace!("process failed '{:?}'", err);
                self.terminate(&mut p)?;
                Ok(CommandResult::Error)
            }
        }
    }

    fn terminate(&self, p: &mut Popen) -> Result<()> {
        p.kill().context(KillFailed {name: self.name.clone()})?;
        p.wait().context(WaitFailed {name: self.name.clone()})?;
        trace!("process killed");
        Ok(())
    }
}

/// Encapsulates an command execution result
#[derive(Debug, Eq, PartialEq)]
pub enum CommandResult {
    /// `Command` has been executed successfully and `String` contains stdout.
    Success(String),
    /// `Command` failed to execute
    Failed,
    /// `Command` execution exceeded specified timeout
    Timeout,
    /// `Command` could not be executed
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;

    use env_logger;
    use spectral::prelude::*;

    #[test]
    fn execution_ok() {
        init();

        let command = Command::new("uname", r#"/usr/bin/uname -a"#, 5);
        #[cfg(target_os = "macos")]
        let expected = "Darwin";
        #[cfg(target_os = "linux")]
        let expected = "Linux";

        let res = command.exec();

        asserting("executing command successfully").that(&res).is_ok()
            .is_success_contains(expected)
    }

    #[test]
    fn execution_failed() {
        init();

        let command = Command::new("false", r#"/usr/bin/false"#, 1);

        let res = command.exec();

        asserting("executing command successfully").that(&res).is_ok().is_equal_to(CommandResult::Failed)
    }

    #[test]
    fn execution_timeout() {
        init();

        let command = Command::new("sleep", r#"/bin/sleep 5"#, 1);

        let res = command.exec();

        asserting("executing command successfully").that(&res).is_ok().is_equal_to(CommandResult::Timeout)
    }

    #[test]
    fn execution_error() {
        init();

        let command = Command::new("no_such_command", r#"/no_such_command"#, 1);

        let res = command.exec();

        asserting("executing command errors").that(&res).is_err();
    }
}
