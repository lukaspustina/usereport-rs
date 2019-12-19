#![warn(missing_docs)]

//! USE report

/// External CLI commands used to collect USE report information.
pub mod command;

/// Trait and default implementation to run commands and collect their output
pub mod runner;

/// Test helper
#[cfg(test)]
pub(crate) mod tests;