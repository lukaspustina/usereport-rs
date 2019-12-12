use log::{debug,trace};
use std::time::Duration;
use std::io::Read;
use snafu::{ResultExt, Snafu};
use subprocess::{Popen, PopenConfig, PopenError, Redirection};

#[derive(Debug, Snafu)]
pub enum Error<'a> {
    #[snafu(display("Failed to run command {}: {}", name, source))]
    ProcessFailed { name: &'a str, source: PopenError },
    #[snafu(display("Failed to kill command {}: {}", name, source))]
    KillFailed { name: &'a str, source: std::io::Error },
    #[snafu(display("Failed to wait for command {}: {}", name, source))]
    WaitFailed { name: &'a str, source: PopenError },
}

type Result<'a, T, E = Error<'a>> = std::result::Result<T, E>;

pub struct Command<'a> {
    name: &'a str,
    title: &'a str,
    args: Vec<&'a str>,
    timeout_sec: u64,
    default_run: bool,
}

impl<'a> Command<'a> {
    pub fn new(name: &'a str, command: &'a str, timeout_sec: u64) -> Command<'a> {
        let args: Vec<_> = command.split(' ').collect();
        assert!(args.len() > 0);

        Command {
            name,
            title: name,
            args,
            timeout_sec,
            default_run: true,
        }
    }

    pub fn exec(&self) -> Result<CommandResult> {
        let mut p = Popen::create(&self.args, PopenConfig {
                stdout: Redirection::Pipe, ..Default::default()
            }).context(ProcessFailed {name: self.name})?;
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
                self.terminate(&mut p);
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
        p.kill().context(KillFailed {name: self.name})?;
        p.wait().context(WaitFailed {name: self.name})?;
        trace!("process killed");
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CommandResult {
    Success(String),
    Failed,
    Timeout,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    use env_logger;
    use spectral::{AssertionFailure, Spec};
    use spectral::prelude::*;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

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

        let command = Command::new("uname", r#"/usr/bin/false"#, 1);
        #[cfg(target_os = "macos")]
            let expected = "Darwin";
        #[cfg(target_os = "linux")]
            let expected = "Linux";

        let res = command.exec();

        asserting("executing command successfully").that(&res).is_ok().is_equal_to(CommandResult::Failed)
    }

    #[test]
    fn execution_timeout() {
        init();

        let command = Command::new("uname", r#"/bin/sleep 5"#, 1);
        #[cfg(target_os = "macos")]
        let expected = "Darwin";
        #[cfg(target_os = "linux")]
        let expected = "Linux";

        let res = command.exec();

        asserting("executing command successfully").that(&res).is_ok().is_equal_to(CommandResult::Timeout)
    }

    #[test]
    fn execution_error() {
        init();

        let command = Command::new("uname", r#"/no_such_command"#, 1);
        #[cfg(target_os = "macos")]
        let expected = "Darwin";
        #[cfg(target_os = "linux")]
        let expected = "Linux";

        let res = command.exec();

        asserting("executing command errors").that(&res).is_err();
    }

    trait CommandResultSuccess {
        fn is_success_contains(&mut self, expected: &str);
    }

    impl<'s> CommandResultSuccess for Spec<'s, CommandResult> {
        fn is_success_contains(&mut self, expected: &str) {
            let subject = self.subject;
            match subject {
                CommandResult::Success(x) if x.contains(expected) => {},
                _ => AssertionFailure::from_spec(self)
                    .with_expected(format!("command result is success and contains '{}'", expected))
                    .with_actual(format!("'{:?}'", subject))
                    .fail()
            }
        }
    }

}
