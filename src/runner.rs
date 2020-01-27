use crate::command::{Command, CommandResult};

use snafu::{ResultExt, Snafu};
use std::fmt::Debug;

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Command execution failed
    #[snafu(display("failed to run command {}: {}", name, source))]
    ExecuteCommandFailed { name: String, source: std::io::Error },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Runner Interface
pub trait Runner<'a, I: IntoIterator<Item = &'a Command>>: Debug {
    /// Execute all commands and wait until all commands return
    fn run(&self, commands: I, max_parallel_commands: usize) -> Result<Vec<CommandResult>>;
}

pub use thread::ThreadRunner;

/// Thread based runner
pub mod thread {
    use super::*;

    use std::{
        sync::mpsc::{self, Receiver, Sender},
        thread,
        thread::JoinHandle,
    };

    /// Ensures that results are in same order as commands
    #[derive(Default, Debug, Clone)]
    pub struct ThreadRunner {
        progress_tx: Option<Sender<usize>>,
    }

    impl<'a, I: IntoIterator<Item = &'a Command>> super::Runner<'a, I> for ThreadRunner {
        fn run(&self, commands: I, max_parallel_commands: usize) -> Result<Vec<CommandResult>> {
            let mut results = Vec::new();

            let commands: Vec<&Command> = commands.into_iter().collect();
            for chunk in commands.chunks(max_parallel_commands).map(|x| x.to_vec()) {
                // Create child threads and run commands
                let (children, rx) = ThreadRunner::create_children(chunk, &self.progress_tx)?;
                // Wait for results
                let mut chunk_results = ThreadRunner::wait_for_results(children, rx);
                results.append(&mut chunk_results);
            }

            Ok(results)
        }
    }

    type ChildResult = (usize, CommandResult);
    type ChildrenSupervision = (Vec<JoinHandle<()>>, Receiver<ChildResult>);

    impl ThreadRunner {
        pub fn new() -> Self { ThreadRunner::default() }

        pub fn with_progress<T: Into<Option<Sender<usize>>>>(self, progress_tx: T) -> Self {
            ThreadRunner {
                progress_tx: progress_tx.into(),
            }
        }

        fn create_children<'a, I: IntoIterator<Item = &'a Command>>(
            commands: I,
            progress_tx: &Option<Sender<usize>>,
        ) -> Result<ChildrenSupervision> {
            let (tx, rx): (Sender<ChildResult>, Receiver<ChildResult>) = mpsc::channel();
            let mut children = Vec::new();

            for (seq, command) in commands.into_iter().enumerate() {
                let command = command.clone();
                let child = ThreadRunner::create_child(seq, command, tx.clone(), progress_tx.clone())?;
                children.push(child);
            }

            Ok((children, rx))
        }

        fn create_child(
            seq: usize,
            command: Command,
            tx: Sender<ChildResult>,
            progress_tx: Option<Sender<usize>>,
        ) -> Result<JoinHandle<()>> {
            let name = command.name.clone();
            thread::Builder::new()
                .name(command.name.clone())
                .spawn(move || {
                    let res = command.exec();
                    // This should not happen as long as the parent is alive; if it happens, this is a valid reason to
                    // panic
                    tx.send((seq, res)).expect("Thread failed to send result via channel");
                    if let Some(progress_tx) = progress_tx {
                        progress_tx.send(1).expect("Thread failed to send progress via channel");
                    }
                })
                .context(ExecuteCommandFailed { name })
        }

        fn wait_for_results(children: Vec<JoinHandle<()>>, rx: Receiver<ChildResult>) -> Vec<CommandResult> {
            let mut results = Vec::with_capacity(children.len());
            // Get results
            for _ in 0..children.len() {
                // This should not happen as long as the child's tx is alive; if it happens, this is a valid reason
                // to panic
                let (seq, result) = rx.recv().expect("Failed to receive from child");
                results.push((seq, result));
            }

            // Ensure all child threads have completed execution
            for child in children {
                // This should not happen as long as the parent is alive; if it happens, this is a valid reason to
                // panic
                child.join().expect("Parent failed to wait for child");
            }

            results.sort_by_key(|(seq, _)| *seq);
            results.into_iter().map(|(_, result)| result).collect()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{runner::Runner, tests::*};

        use spectral::prelude::*;

        #[test]
        fn run_ok() {
            let mut commands = Vec::new();
            #[cfg(target_os = "macos")]
            commands.push(Command::new("uname", r#"/usr/bin/uname -a"#));
            #[cfg(target_os = "macos")]
            commands.push(Command::new("uname", r#"/usr/bin/uname -a"#));
            #[cfg(target_os = "macos")]
            let expected = "Darwin";
            #[cfg(target_os = "linux")]
            commands.push(Command::new("cat-uname", r#"/bin/cat /proc/version"#));
            #[cfg(target_os = "linux")]
            commands.push(Command::new("cat-uname", r#"/bin/cat /proc/version"#));
            #[cfg(target_os = "linux")]
            let expected = "Linux";

            let r = ThreadRunner::new();
            let results = r.run(&commands, 64);

            asserting("Command run").that(&results).is_ok().has_length(2);

            let results = results.unwrap();
            asserting("First command result is success")
                .that(&results[0])
                .is_success_contains(expected);
        }
    }
}
