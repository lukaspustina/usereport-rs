use crate::command::CommandResult;
use handlebars::Handlebars;
use serde::Serialize;
use snafu::{ResultExt, Snafu};
use std::io::Write;

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Rendering of report to Json failed
    #[snafu(display("Failed to render report to Json: {}", source))]
    JsonRenderingFailed { source: serde_json::Error },
    /// Rendering of report to Markdown failed
    #[snafu(display("Failed to render report to Markdown: {}", source))]
    MdRenderingFailed { source: handlebars::TemplateRenderError},
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

pub use json::JsonRenderer;
pub use markdown::MdRenderer;

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
            serde_json::to_writer(w, self.report).context(JsonRenderingFailed {})
        }
    }
}

pub mod markdown {
    use super::*;

    pub struct MdRenderer<'a> {
        report: &'a Report<'a>,
        template: &'a str,
    }

    impl<'a> MdRenderer<'a> {
        pub fn new<'r: 'a>(report: &'a Report<'r>, template: &'a str) -> Self { MdRenderer { report, template } }
    }

    impl<'a> Renderer for MdRenderer<'a> {
        fn render<W: Write>(&self, w: W) -> Result<()> {
            let mut reg = Handlebars::new();
            reg.render_template_to_write(self.template, self.report, w)
                .context(MdRenderingFailed {})
        }
    }
}
