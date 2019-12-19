use crate::command::CommandResult;
use serde::Serialize;
use snafu::{ResultExt, Snafu};
use std::io::Write;

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Rendering of report failed
    #[snafu(display("Failed to render report: {}", source))]
    RenderingFailed { source: serde_json::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum OutputType {
    HTML,
    JSON,
    Markdown,
}

#[derive(Debug, Serialize)]
pub struct Report<'a> {
    command_results: &'a [CommandResult],
}

impl<'a> Report<'a> {
    pub fn new(command_results: &'a [CommandResult]) -> Self { Report { command_results } }
}

pub trait Renderer {
    fn render<W: Write>(&self, w: W) -> Result<()>;
}

pub mod json {
    use super::*;

    pub struct JsonRenderer<'a> {
        report: &'a Report<'a>,
    }

    impl<'a> JsonRenderer<'a> {
        pub fn new<'r: 'a>(report: &'a Report<'r>) -> Self { JsonRenderer { report } }
    }

    impl<'a> Renderer for JsonRenderer<'a> {
        fn render<W: Write>(&self, w: W) -> Result<()> {
            serde_json::to_writer(w, self.report).context(RenderingFailed {})
        }
    }
}
