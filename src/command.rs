use chrono::Local;
use log::{debug, trace};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Seek, SeekFrom},
    rc::Rc,
    time::Duration,
};
use subprocess::{Popen, PopenConfig, Redirection};
use tempfile;

/// Run a CLI command and store its stdout.
///
/// # Example
/// ```
/// # use usereport::command::{Command, CommandResult};
/// #[cfg(target_os = "macos")]
/// let command = Command::new("uname", r#"/usr/bin/uname -a"#)
///     .with_title("Host OS")
///     .with_timeout(5);
/// #[cfg(target_os = "linux")]
/// let command = Command::new("true", r#"/bin/true"#)
///     .with_title("Just a successful command")
///     .with_timeout(5);
/// match command.exec() {
///     CommandResult::Success {
///         command: _,
///         run_time_ms: _,
///         stdout: stdout,
///     } => println!("Command output '{}'", stdout),
///     CommandResult::Failed {
///         command: _,
///         run_time_ms: _,
///         stdout: stdout,
///     } => println!("Command failed with output '{}'", stdout),
///     CommandResult::Timeout {
///         command: _,
///         run_time_ms: _,
///     } => println!("Command timed out"),
///     CommandResult::Error {
///         command: _,
///         reason: reason,
///     } => println!("Command errored because {}", reason),
/// };
/// ```
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize, Clone)]
pub struct Command {
    pub(crate) name:        String,
    pub(crate) title:       Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) command:     String,
    #[serde(rename = "timeout")]
    /// Timeout for command execution, defaults to 1 sec if not set
    pub(crate) timeout_sec: Option<u64>,
    pub(crate) links:       Option<Vec<Link>>,
}

impl Command {
    /// Create new command with default values
    pub fn new<T: Into<String>>(name: T, command: T) -> Command {
        Command {
            name:        name.into(),
            title:       None,
            description: None,
            command:     command.into(),
            timeout_sec: None,
            links:       None,
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
    pub fn with_title<T: Into<String>>(self, title: T) -> Command {
        Command {
            title: Some(title.into()),
            ..self
        }
    }

    /// Set title of command
    pub fn with_timeout<T: Into<Option<u64>>>(self, timeout_sec: T) -> Command {
        Command {
            timeout_sec: timeout_sec.into(),
            ..self
        }
    }

    /// Set description of command
    pub fn with_description<T: Into<String>, S: Into<Option<T>>>(self, description: S) -> Command {
        Command {
            description: description.into().map(Into::into),
            ..self
        }
    }

    /// Set Links of command
    pub fn with_links<T: Into<Option<Vec<Link>>>>(self, links: T) -> Command {
        Command {
            links: links.into(),
            ..self
        }
    }

    /// Execute this command; may panic
    ///
    /// Standard output and error will be written to a temporary file, because a pipe may only
    /// contain up 64 KB data; cf. http://man7.org/linux/man-pages/man7/pipe.7.html.
    pub fn exec(self) -> CommandResult {
        let args = match self.args() {
            Ok(args) => args,
            Err(_) => return self.fail("failed to split command into arguments"),
        };
        let mut tmpfile = match tempfile::tempfile() {
            Ok(f) => Rc::new(f),
            Err(err) => return self.fail(err),
        };
        let popen_config = PopenConfig {
            stdout: Redirection::RcFile(Rc::clone(&tmpfile)),
            stderr: Redirection::Merge,
            ..Default::default()
        };
        let start_time = Local::now();
        let popen = Popen::create(&args, popen_config);

        let mut p = match popen {
            Ok(p) => p,
            Err(err) => return self.fail(err),
        };
        debug!("Running '{:?}' as '{:?}'", args, p);

        let wait = p.wait_timeout(Duration::new(self.timeout_sec.unwrap_or(1), 0));
        let run_time_ms = (Local::now() - start_time).num_milliseconds() as u64;

        if let Err(err) = Rc::get_mut(&mut tmpfile).unwrap().seek(SeekFrom::Start(0)) {
            // TODO: unwrap is unsafe
            return self.fail(err);
        };
        match wait {
            Ok(Some(status)) if status.success() => {
                debug!(
                    "{:?} process successfully finished as {:?} with {} bytes output",
                    args,
                    status,
                    tmpfile.metadata().unwrap().len()
                );
                let mut stdout = String::new();
                if let Err(err) = Rc::get_mut(&mut tmpfile).unwrap().read_to_string(&mut stdout) {
                    // TODO: unwrap is unsafe
                    return self.fail(err);
                };
                trace!("stdout '{}'", stdout);

                CommandResult::Success {
                    command: self,
                    run_time_ms,
                    stdout,
                }
            }
            Ok(Some(status)) => {
                debug!("{:?} process finished as {:?}", args, status);
                let mut stdout = String::new();
                if let Err(err) = Rc::get_mut(&mut tmpfile).unwrap().read_to_string(&mut stdout) {
                    // TODO: unwrap is unsafe
                    return self.fail(err);
                };
                trace!("stdout '{}'", stdout);
                CommandResult::Failed {
                    command: self,
                    run_time_ms,
                    stdout,
                }
            }
            Ok(None) => {
                debug!("{:?} process timed out and will be killed", args);
                self.terminate(&mut p);
                CommandResult::Timeout {
                    command: self,
                    run_time_ms,
                }
            }
            Err(err) => {
                debug!("{:?} process failed '{:?}'", args, err);
                self.terminate(&mut p);
                CommandResult::Error {
                    command: self,
                    reason:  err.to_string(),
                }
            }
        }
    }

    /// Fail with error message
    fn fail<T: ToString>(self, reason: T) -> CommandResult {
        CommandResult::Error {
            command: self,
            reason:  reason.to_string(),
        }
    }

    fn args(&self) -> Result<Vec<String>, shellwords::MismatchedQuotes> { shellwords::split(&self.command) }

    /// Panics
    fn terminate(&self, p: &mut Popen) {
        p.kill().expect("failed to kill command");
        p.wait().expect("failed to wait for command to finish");
        trace!("process killed");
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct Link {
    pub(crate) name: String,
    pub(crate) url:  String,
}

impl Link {
    pub fn new<T: Into<String>>(name: T, url: T) -> Link {
        Link {
            name: name.into(),
            url:  url.into(),
        }
    }

    pub fn name(&self) -> &str { &self.name }

    pub fn url(&self) -> &str { &self.url }
}

/// Encapsulates a command execution result
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub enum CommandResult {
    /// `Command` has been executed successfully and `String` contains stdout.
    Success {
        command:     Command,
        run_time_ms: u64,
        stdout:      String,
    },
    /// `Command` failed to execute
    Failed {
        command:     Command,
        run_time_ms: u64,
        stdout:      String,
    },
    /// `Command` execution exceeded specified timeout
    Timeout { command: Command, run_time_ms: u64 },
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

        asserting("executing command successfully").that(&res).is_failed();
    }

    #[test]
    fn execution_timeout() {
        init();

        let command = Command::new("sleep", r#"/bin/sleep 5"#).with_timeout(1);

        let res = command.exec();

        asserting("executing command successfully").that(&res).is_timeout();
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

    #[test]
    fn command_split() {
        init();

        let command = Command::new("no_such_command", r#"/bin/sleep 5"#);
        let args = command.args();

        asserting("splitting command into args")
            .that(&args)
            .is_ok()
            .has_length(2);
    }

    #[test]
    fn command_split_single_quotes() {
        init();

        let command = Command::new("no_such_command", r#"sh -c 'dmesg -T | grep "failed"'"#);
        let args = command.args();

        asserting("splitting command into args")
            .that(&args)
            .is_ok()
            .has_length(3);
    }

    #[test]
    fn command_split_double_quotes() {
        init();

        let command = Command::new("no_such_command", r#"sh -c "dmesg -T | tail""#);
        let args = command.args();

        asserting("splitting command into args")
            .that(&args)
            .is_ok()
            .has_length(3);
    }
}
