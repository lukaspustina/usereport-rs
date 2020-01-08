use crate::command::CommandResult;

use chrono::{DateTime, Local};
use handlebars::Handlebars;
use serde::Serialize;
use snafu::{ResultExt, Snafu};
use std::io::Write;
use uname;

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Failed to parse output type
    #[snafu(display("failed to parse output type"))]
    OutputTypeParseError,
    /// Failed to create a new report
    #[snafu(display("failed to create a new report: {}", source))]
    CreateFailed { source: std::io::Error },
    /// Rendering of report to Json failed
    #[snafu(display("failed to render report to Json: {}", source))]
    JsonRenderingFailed { source: serde_json::Error },
    /// Handlebars template for Markdown is invalid
    #[snafu(display("Handlebars template for Markdown is invalid: {}", source))]
    MdTemplateFailed { source: handlebars::TemplateError },
    /// Rendering of report to Markdown failed
    #[snafu(display("failed to render report to Markdown: {}", source))]
    MdRenderingFailed { source: handlebars::RenderError },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum OutputType {
    JSON,
    Markdown,
}

impl FromStr for OutputType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "json" => Ok(OutputType::JSON),
            "markdown" => Ok(OutputType::Markdown),
            "md" => Ok(OutputType::Markdown),
            _ => Err(Error::OutputTypeParseError),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Report<'a> {
    hostinfo_results: Option<&'a [CommandResult]>,
    command_results:  &'a [CommandResult],
    hostname:         String,
    uname:            String,
    date_time:        DateTime<Local>,
}

impl<'a> Report<'a> {
    pub fn new(command_results: &'a [CommandResult]) -> Result<Self> {
        let uname = uname::uname().context(CreateFailed {})?;
        let hostname = uname.nodename.to_string();
        let uname = format!(
            "{} {} {} {} {}",
            uname.sysname, uname.nodename, uname.release, uname.version, uname.machine
        );
        let date_time = Local::now();

        Ok(Report {
            hostinfo_results: None,
            command_results,
            hostname,
            uname,
            date_time,
        })
    }

    pub fn set_hostinfo_results(&mut self, command_results: &'a [CommandResult]) {
        self.hostinfo_results = Some(command_results);
    }
}

pub trait Renderer {
    fn render<'a, W: Write>(&self, report: &Report<'a>, w: W) -> Result<()>;
}

pub use json::JsonRenderer;
pub use markdown::MdRenderer;
use std::str::FromStr;

pub mod json {
    use super::*;

    #[derive(Default)]
    pub struct JsonRenderer {}

    impl JsonRenderer {
        pub fn new() -> Self { JsonRenderer {} }
    }

    impl Renderer for JsonRenderer {
        fn render<'a, W: Write>(&self, report: &Report<'a>, w: W) -> Result<()> {
            serde_json::to_writer(w, report).context(JsonRenderingFailed {})
        }
    }
}

pub mod markdown {
    use super::*;

    pub struct MdRenderer<'a> {
        template: &'a str,
    }

    impl<'a> MdRenderer<'a> {
        pub fn new(template: &'a str) -> Self { MdRenderer { template } }
    }

    impl<'a> Renderer for MdRenderer<'a> {
        fn render<'r, W: Write>(&self, report: &Report<'r>, w: W) -> Result<()> {
            let mut handlebars = Handlebars::new();
            handlebars.register_helper("rfc2822", Box::new(handlebars_helper::date_time_2822));
            handlebars.register_helper("rfc3339", Box::new(handlebars_helper::date_time_3339));
            handlebars
                .register_template_string("markdown", self.template)
                .context(MdTemplateFailed {})?;
            handlebars
                .render_to_write("markdown", report, w)
                .context(MdRenderingFailed {})
        }
    }
}

mod handlebars_helper {
    use chrono::{DateTime, Local};
    use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderError};

    pub(crate) fn date_time_2822(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> HelperResult {
        let dt = date_param(h)?;
        out.write(&dt.to_rfc2822()).map_err(RenderError::with)
    }

    pub(crate) fn date_time_3339(
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> HelperResult {
        let dt = date_param(h)?;
        out.write(&dt.to_rfc3339()).map_err(RenderError::with)
    }

    fn date_param(h: &Helper) -> ::std::result::Result<DateTime<Local>, RenderError> {
        let dt_str = h
            .param(0)
            .ok_or_else(|| RenderError::new("no such parameter"))?
            .value()
            .as_str()
            .ok_or_else(|| RenderError::new("parameter is not a string"))?;
        dt_str.parse::<DateTime<Local>>().map_err(RenderError::with)
    }
}
