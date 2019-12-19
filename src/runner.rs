use crate::command::{self, Command, CommandResult};

use snafu::{ResultExt, Snafu};

/// Runner Interface
pub trait Runner {
    /// Create Runner with commands
    fn new(commands: Vec<Command>) -> Self;
    /// Execute all commands and wait until all commands return
    fn run(self) -> Result<Vec<command::Result<CommandResult>>>;
}

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Command execution failed
    #[snafu(display("Failed to run command {}: {}", name, source))]
    CommandFailed{ name: String, source: std::io::Error },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Thread based runner
pub mod thread {
    use super::*;

    use std::sync::mpsc::{self, Sender, Receiver};
    use std::thread;
    use std::thread::JoinHandle;

    pub struct ThreadRunner {
        commands: Vec<Command>,
    }

    impl super::Runner for ThreadRunner {
        fn new(commands: Vec<Command>) -> Self {
            ThreadRunner {
                commands,
            }
        }

        fn run(self) -> Result<Vec<command::Result<CommandResult>>> {
            // Create child threads and run commands
            let (children, rx) = ThreadRunner::create_children(self.commands)?;
            // Wait for results
            let results = ThreadRunner::wait_for_results(children, rx);

            Ok(results)
        }
    }

    impl ThreadRunner {
        fn create_children(commands: Vec<Command>) -> Result<(Vec<JoinHandle<()>>, Receiver<command::Result<CommandResult>>)> {
            let (tx, rx): (Sender<command::Result<CommandResult>>, Receiver<command::Result<CommandResult>>) = mpsc::channel();
            let mut children = Vec::new();

            for c in commands {
                let tx = tx.clone();
                let child = ThreadRunner::create_child(c, tx)?;
                children.push(child);
            }

            Ok((children, rx))
        }

        fn create_child(command: Command, tx: Sender<command::Result<CommandResult>>) -> Result<JoinHandle<()>> {
            let name = command.name.clone();
            thread::Builder::new().name(name.clone()).spawn(move || {
                let res = command.exec();
                // This should not happen as long as the parent is alive; if it happens, this is a valid reason to panic
                tx.send(res).expect("Thread failed to send result via channel");
            }).context(CommandFailed { name })
        }

        fn wait_for_results(children: Vec<JoinHandle<()>>, rx: Receiver<command::Result<CommandResult>>) -> Vec<command::Result<CommandResult>> {
            let mut results = Vec::with_capacity(children.len());
            // Get results
            for _ in 0..children.len() {
                // This should not happen as long as the child's tx is alive; if it happens, this is a valid reason to panic
                let result = rx.recv().expect("Failed to receive from child");
                results.push(result);
            }

            // Ensure all child threads have completed execution
            for child in children {
                // This should not happen as long as the parent is alive; if it happens, this is a valid reason to panic
                child.join().expect("Parent failed to wait for child");
            }

            results
        }

    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::runner::Runner;

        use crate::tests::*;
        use spectral::prelude::*;
        use crate::tests::CommandResultSuccess;

        #[test]
        fn run_ok() {
            let mut commands = Vec::new();
            commands.push(Command::new("uname", r#"/usr/bin/uname -a"#, 1));
            commands.push(Command::new("uname", r#"/usr/bin/uname -a"#, 1));
            #[cfg(target_os = "macos")]
            let expected = "Darwin";
            #[cfg(target_os = "linux")]
            let expected = "Linux";

            let r = ThreadRunner::new(commands);
            let results = r.run();

            asserting("Command run").that(&results)
                .is_ok()
                .has_length(2);

            let results = results.unwrap();
            asserting("First command result is success").that(&results[0])
                .is_ok()
                .is_success_contains(expected);
        }
    }
}