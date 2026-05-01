//#![warn(missing_docs)]

//! USE report

/// Analysis encapsulates the whole process of running USE report commands
pub mod analysis;

/// Baseline persistence + outlier detection (Phase 2+).
pub mod baseline;

/// External CLI commands used to collect USE report information.
pub mod command;

/// Compare two `AnalysisReport`s (Phase 2+).
pub mod diff;

/// CLI
#[cfg(feature = "bin")]
pub mod cli;

/// Typed signal collectors (Phase 1+).
pub mod collector;

/// Findings produced by the rule engine.
pub mod finding;

/// Reports in HTML, JSON, and Markdown format
pub mod renderer;

/// Multi-signal pattern correlator (Phase 5+).
pub mod pattern;

/// Declarative rules + predicate DSL (Phase 1+).
pub mod rule;

/// Trait and default implementation to run commands and collect their output
pub mod runner;

/// Typed metric values (Phase 1+).
pub mod signal;

pub use analysis::{Analysis, AnalysisReport, Context};
#[cfg(feature = "bin")]
pub use cli::config::Config;
pub use command::{Command, CommandResult};
pub use finding::{Evidence, Finding, FindingKind, Severity};
pub use renderer::{JsonRenderer, Renderer, TemplateRenderer};
pub use runner::{Runner, ThreadRunner};
pub use signal::{Signal, SignalValue, Unit};

/// Test helper
#[cfg(test)]
pub(crate) mod tests;
