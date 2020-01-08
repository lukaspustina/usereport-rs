use log::{debug, trace};
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};
use subprocess::{Popen, PopenConfig, Redirection};

/// Run a CLI command and store its stdout.
///
/// # Example
/// ```
/// # use usereport::command::{Command, CommandResult};
/// #[cfg(target_os = "macos")]
/// let command = Command::new("uname", r#"/usr/bin/uname -a"#)
///     .set_title("Host OS")
///     .set_timeout(5);
/// #[cfg(target_os = "linux")]
/// let command = Command::new("true", r#"/bin/true"#)
///     .set_title("Just a successful command")
///     .set_timeout(5);
/// match command.exec() {
///     CommandResult::Success { command: _, stdout: stdout } => println!("Command output '{}'", stdout),
///     CommandResult::Failed { command: _ } => println!("Command failed"),
///     CommandResult::Timeout { command: _ } => println!("Command timed out"),
///     CommandResult::Error{ command: _, reason: reason } => println!("Command errored because {}", reason)
/// };
/// ```
#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
pub struct Command {
    pub(crate) name: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) command: String,
    #[serde(rename = "timeout")]
    /// Timeout for command execution, defaults to 1 sec if not set
    pub(crate) timeout_sec: Option<u64>,
}

impl Command {
    /// Create new command with default values
    pub fn new<T: Into<String>>(name: T, command: T) -> Command {
        Command {
            name: name.into(),
            title: None,
            description: None,
            command: command.into(),
            timeout_sec: None,
        }
    }

    /// Get name of command
    pub fn name(&self) -> &str { &self.name }

    /// Get command args
    pub fn command(&self) -> &str { &self.command }

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

    /// Set title of command
    pub fn set_timeout(self, timeout_sec: u64) -> Command {
        Command {
            timeout_sec: Some(timeout_sec),
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

    /// Execute this command; may panic
    pub fn exec(self) -> CommandResult {
        let args: Vec<_> = self.command.split(' ').collect();
        let popen_config = PopenConfig { stdout: Redirection::Pipe, ..Default::default() };
        let popen = Popen::create(&args, popen_config);

        let mut p = match popen {
            Ok(p) => p,
            Err(err) => return CommandResult::Error { command: self, reason: err.to_string() }
        };
        debug!("Running '{:?}' as '{:?}'", args, p);

        match p.wait_timeout(Duration::new(self.timeout_sec.unwrap_or(1), 0)) {
            Ok(Some(status)) if status.success() => {
                trace!("process successfully finished as {:?}", status);
                let mut stdout = String::new();
                let _ = p.stdout.as_ref().unwrap().read_to_string(&mut stdout); // TODO: unwrap is unsafe
                debug!("stdout '{}'", stdout);

                CommandResult::Success { command: self, stdout }
            }
            Ok(Some(status)) => {
                trace!("process successfully finished as {:?}", status);
                CommandResult::Failed { command: self }
            }
            Ok(None) => {
                trace!("process timed out and will be killed");
                self.terminate(&mut p);
                CommandResult::Timeout { command: self }
            }
            Err(err) => {
                trace!("process failed '{:?}'", err);
                self.terminate(&mut p);
                CommandResult::Error { command: self, reason: err.to_string() }
            }
        }
    }

    /// Panics
    fn terminate(&self, p: &mut Popen) -> () {
        p.kill().expect("failed to kill command");
        p.wait().expect("failed to wait for command to finish");
        trace!("process killed");
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
    Error { command: Command, reason: String },
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
            let command = Command::new("true", r#"/usr/bin/true"#);
        #[cfg(target_os = "linux")]
            let command = Command::new("true", r#"/bin/true"#);

        let res = command.exec();

        asserting("executing command successfully")
            .that(&res)
            .is_success_contains("");
    }

    #[test]
    fn execution_failed() {
        init();

        #[cfg(target_os = "macos")]
            let command = Command::new("false", r#"/usr/bin/false"#);
        #[cfg(target_os = "linux")]
            let command = Command::new("false", r#"/bin/false"#);

        let res = command.exec();

        asserting("executing command successfully")
            .that(&res)
            .is_failed();
    }

    #[test]
    fn execution_timeout() {
        init();

        let command = Command::new("sleep", r#"/bin/sleep 5"#).set_timeout(1);

        let res = command.exec();

        asserting("executing command successfully")
            .that(&res)
            .is_timeout();
    }

    #[test]
    fn execution_error() {
        init();

        let command = Command::new("no_such_command", r#"/no_such_command"#);

        let res = command.exec();

        asserting("executing command errors")
            .that(&res)
            .is_error_contains("No such file or directory")
    }
}
