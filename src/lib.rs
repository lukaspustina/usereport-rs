//#![warn(missing_docs)]

//! USE report

/// Analysis encapsulates the whole process of running USE report commands
pub mod analysis;

/// External CLI commands used to collect USE report information.
pub mod command;

/// Configuration
pub mod config;

/// Reports in HTML, JSON, and Markdown format
pub mod report;

/// Trait and default implementation to run commands and collect their output
pub mod runner;

pub use analysis::Analysis;
pub use command::{Command, CommandResult};
pub use config::Config;
pub use report::{Renderer, Report};
pub use runner::Runner;

/// Test helper
#[cfg(test)]
pub(crate) mod tests;
