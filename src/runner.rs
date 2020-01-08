use crate::command::{self, Command, CommandResult};

use snafu::{ResultExt, Snafu};
use std::sync::mpsc::Sender;

/// Runner Interface
pub trait Runner<'a> {
    /// Create Runner with commands
    fn new<I: IntoIterator<Item = &'a Command>>(commands: I) -> Self;
    /// Create Runner with commands with progress indication channel
    fn with_progress<I: IntoIterator<Item = &'a Command>>(commands: I, progress_tx: Sender<usize>) -> Self;
    /// Execute all commands and wait until all commands return
    fn run(self) -> Result<Vec<command::Result<CommandResult>>>;
}

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Command execution failed
    #[snafu(display("failed to run command {}: {}", name, source))]
    CommandFailed { name: String, source: std::io::Error },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub use thread::ThreadRunner;

/// Thread based runner
pub mod thread {
    use super::*;

    use std::{
        sync::mpsc::{self, Receiver, Sender},
        thread,
        thread::JoinHandle,
    };

    pub struct ThreadRunner<'a> {
        commands:    Vec<&'a Command>,
        progress_tx: Option<Sender<usize>>,
    }

    impl<'a> super::Runner<'a> for ThreadRunner<'a> {
        fn new<I: IntoIterator<Item = &'a Command>>(commands: I) -> Self {
            let commands = commands.into_iter().collect();
            ThreadRunner {
                commands,
                progress_tx: None,
            }
        }

        fn with_progress<I: IntoIterator<Item = &'a Command>>(commands: I, progress_tx: Sender<usize>) -> Self {
            let commands = commands.into_iter().collect();
            ThreadRunner {
                commands,
                progress_tx: Some(progress_tx),
            }
        }

        fn run(self) -> Result<Vec<command::Result<CommandResult>>> {
            // Create child threads and run commands
            let (children, rx) = ThreadRunner::create_children(self.commands.as_slice(), self.progress_tx)?;
            // Wait for results
            let results = ThreadRunner::wait_for_results(children, rx);

            Ok(results)
        }
    }

    type ChildrenSupervision = (Vec<JoinHandle<()>>, Receiver<command::Result<CommandResult>>);

    impl<'a> ThreadRunner<'a> {
        fn create_children(
            commands: &'a [&Command],
            progress_tx: Option<Sender<usize>>,
        ) -> Result<ChildrenSupervision> {
            let (tx, rx): (
                Sender<command::Result<CommandResult>>,
                Receiver<command::Result<CommandResult>>,
            ) = mpsc::channel();
            let mut children = Vec::new();

            for c in commands {
                let child = ThreadRunner::create_child(c, tx.clone(), progress_tx.clone())?;
                children.push(child);
            }

            Ok((children, rx))
        }

        fn create_child(
            command: &Command,
            tx: Sender<command::Result<CommandResult>>,
            progress_tx: Option<Sender<usize>>,
        ) -> Result<JoinHandle<()>> {
            let command = command.clone();
            let name = command.name.clone();
            thread::Builder::new()
                .name(command.name.clone())
                .spawn(move || {
                    let res = command.exec();
                    // This should not happen as long as the parent is alive; if it happens, this is a valid reason to
                    // panic
                    tx.send(res).expect("Thread failed to send result via channel");
                    if let Some(progress_tx) = progress_tx {
                        progress_tx.send(1).expect("Thread failed to send progress via channel");
                    }
                })
                .context(CommandFailed { name })
        }

        fn wait_for_results(
            children: Vec<JoinHandle<()>>,
            rx: Receiver<command::Result<CommandResult>>,
        ) -> Vec<command::Result<CommandResult>> {
            let mut results = Vec::with_capacity(children.len());
            // Get results
            for _ in 0..children.len() {
                // This should not happen as long as the child's tx is alive; if it happens, this is a valid reason
                // to panic
                let result = rx.recv().expect("Failed to receive from child");
                results.push(result);
            }

            // Ensure all child threads have completed execution
            for child in children {
                // This should not happen as long as the parent is alive; if it happens, this is a valid reason to
                // panic
                child.join().expect("Parent failed to wait for child");
            }

            results
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::runner::Runner;

        use crate::tests::CommandResultSuccess;
        use spectral::prelude::*;

        #[test]
        fn run_ok() {
            let mut commands = Vec::new();
            #[cfg(target_os = "macos")]
            commands.push(Command::new("uname", r#"/usr/bin/uname -a"#, 1));
            #[cfg(target_os = "macos")]
            commands.push(Command::new("uname", r#"/usr/bin/uname -a"#, 1));
            #[cfg(target_os = "macos")]
            let expected = "Darwin";
            #[cfg(target_os = "linux")]
            commands.push(Command::new("cat-uname", r#"/bin/cat /proc/version"#, 1));
            #[cfg(target_os = "linux")]
            commands.push(Command::new("cat-uname", r#"/bin/cat /proc/version"#, 1));
            #[cfg(target_os = "linux")]
            let expected = "Linux";

            let r = ThreadRunner::new(&commands);
            let results = r.run();

            asserting("Command run").that(&results).is_ok().has_length(2);

            let results = results.unwrap();
            asserting("First command result is success")
                .that(&results[0])
                .is_ok()
                .is_success_contains(expected);
        }
    }
}
