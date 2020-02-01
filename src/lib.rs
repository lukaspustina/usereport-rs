#![deny(missing_docs)]

//! USE report

/// Analysis encapsulates the whole process of running USE report commands
pub mod analysis;

/// External CLI commands used to collect USE report information.
pub mod command;

/// CLI
#[cfg(feature = "bin")]
pub mod cli;

/// Reports in HTML, JSON, and Markdown format
pub mod renderer;

/// Trait and default implementation to run commands and collect their output
pub mod runner;

pub use analysis::{Analysis, AnalysisReport, Context};
#[cfg(feature = "bin")]
pub use cli::config::Config;
pub use command::{Command, CommandResult};
pub use renderer::{HbsRenderer, JsonRenderer, Renderer};
pub use runner::{Runner, ThreadRunner};

/// Test helper
#[cfg(test)]
pub(crate) mod tests;
