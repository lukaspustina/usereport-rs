use crate::command::{Command, CommandResult};

use std::fmt::Debug;
use thiserror::Error;

/// Error type
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    /// Command execution failed
    #[error("failed to run command {name}: {source}")]
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

    #[derive(Debug, Clone)]
    pub enum EventKind {
        Started,
        Finished,
    }

    #[derive(Debug, Clone)]
    pub struct ProgressEvent {
        pub seq: usize,
        pub name: String,
        pub kind: EventKind,
    }

    /// Ensures that results are in same order as commands
    #[derive(Default, Debug, Clone)]
    pub struct ThreadRunner {
        progress_tx: Option<Sender<ProgressEvent>>,
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
        pub fn new() -> Self {
            ThreadRunner::default()
        }

        pub fn with_progress<T: Into<Option<Sender<ProgressEvent>>>>(self, progress_tx: T) -> Self {
            ThreadRunner {
                progress_tx: progress_tx.into(),
            }
        }

        fn create_children<'a, I: IntoIterator<Item = &'a Command>>(
            commands: I,
            progress_tx: &Option<Sender<ProgressEvent>>,
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
            progress_tx: Option<Sender<ProgressEvent>>,
        ) -> Result<JoinHandle<()>> {
            let name = command.name.clone();
            let name_for_err = name.clone();
            thread::Builder::new()
                .name(command.name.clone())
                .spawn(move || {
                    if let Some(ref progress_tx) = progress_tx {
                        progress_tx
                            .send(ProgressEvent {
                                seq,
                                name: name.clone(),
                                kind: EventKind::Started,
                            })
                            .expect("Thread failed to send progress via channel");
                    }
                    let res = command.exec();
                    tx.send((seq, res)).expect("Thread failed to send result via channel");
                    if let Some(progress_tx) = progress_tx {
                        progress_tx
                            .send(ProgressEvent {
                                seq,
                                name: name.clone(),
                                kind: EventKind::Finished,
                            })
                            .expect("Thread failed to send progress via channel");
                    }
                })
                .map_err(|e| Error::ExecuteCommandFailed {
                    name: name_for_err,
                    source: e,
                })
        }

        fn wait_for_results(children: Vec<JoinHandle<()>>, rx: Receiver<ChildResult>) -> Vec<CommandResult> {
            let mut results = Vec::with_capacity(children.len());
            // Get results
            for _ in 0..children.len() {
                // This should not happen as long as the child's tx is alive; if it happens, this is a valid reason
                // to panic
                let (seq, result) = rx.recv().expect("Failed to receive from child");
                match &result {
                    CommandResult::Timeout { command, .. } => {
                        log::warn!("command '{}' timed out", command.name());
                    }
                    CommandResult::Error { command, reason } => {
                        log::warn!("command '{}' errored: {}", command.name(), reason);
                    }
                    _ => {}
                }
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
        use crate::runner::Runner;

        #[test]
        fn progress_event_started_fields() {
            let ev = ProgressEvent {
                seq: 0,
                name: "cmd_a".to_string(),
                kind: EventKind::Started,
            };
            assert_eq!(ev.seq, 0);
            assert_eq!(ev.name, "cmd_a");
            assert!(matches!(ev.kind, EventKind::Started));
        }

        #[test]
        fn progress_event_finished_fields() {
            let ev = ProgressEvent {
                seq: 1,
                name: "cmd_b".to_string(),
                kind: EventKind::Finished,
            };
            assert_eq!(ev.seq, 1);
            assert!(matches!(ev.kind, EventKind::Finished));
        }

        use googletest::prelude::*;

        #[test]
        fn run_ok() {
            #[cfg(target_os = "macos")]
            let commands = vec![
                Command::new("uname", r#"/usr/bin/uname -a"#),
                Command::new("uname", r#"/usr/bin/uname -a"#),
            ];
            #[cfg(target_os = "macos")]
            let expected = "Darwin";
            #[cfg(target_os = "linux")]
            let commands = vec![
                Command::new("cat-uname", r#"/bin/cat /proc/version"#),
                Command::new("cat-uname", r#"/bin/cat /proc/version"#),
            ];
            #[cfg(target_os = "linux")]
            let expected = "Linux";

            let r = ThreadRunner::new();
            let results = r.run(&commands, 64).expect("Command run");

            assert_eq!(results.len(), 2);
            assert_that!(
                results[0],
                matches_pattern!(CommandResult::Success {
                    stdout: contains_substring(expected),
                    ..
                })
            );
        }

        /// Results must come back in the same order as the commands, regardless of which thread
        /// finishes first. We use a sleep command as the first entry so it finishes last; the
        /// parallel chunk runs both at once, so the order guarantee comes entirely from the sort.
        #[test]
        fn results_are_in_command_order() {
            let commands = vec![
                Command::new("slow", r#"/bin/sleep 0.1"#).with_timeout(5u64),
                #[cfg(target_os = "macos")]
                Command::new("fast", r#"/usr/bin/true"#).with_timeout(5u64),
                #[cfg(target_os = "linux")]
                Command::new("fast", r#"/bin/true"#).with_timeout(5u64),
            ];

            let r = ThreadRunner::new();
            let results = r.run(&commands, 64).expect("run ok");

            assert_eq!(results.len(), 2);
            // First result must correspond to the slow command
            assert_that!(
                results[0],
                matches_pattern!(CommandResult::Success {
                    command: matches_pattern!(Command {
                        name: eq(&"slow".to_string()),
                        ..
                    }),
                    ..
                })
            );
            // Second result must correspond to the fast command
            assert_that!(
                results[1],
                matches_pattern!(CommandResult::Success {
                    command: matches_pattern!(Command {
                        name: eq(&"fast".to_string()),
                        ..
                    }),
                    ..
                })
            );
        }
    }
}
