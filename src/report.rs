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
    /// Failed to create a new report
    #[snafu(display("Failed to create a new report: {}", source))]
    CreateFailed{ source: std::io::Error },
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
    hostname: String,
    uname: String,
    date_time: DateTime<Local>,
}

impl<'a> Report<'a> {
    pub fn new(command_results: &'a [CommandResult]) -> Result<Self> {
        let uname = uname::uname().context(CreateFailed {})?;
        let hostname = uname.nodename.to_string();
        let uname = format!(
            "{} {} {} {} {}", uname.sysname, uname.nodename, uname.release, uname.version, uname.machine
        );
        let date_time = Local::now();

        Ok(Report {
            command_results,
            hostname,
            uname,
            date_time,
        })
    }
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
            let mut handlebars = Handlebars::new();
            handlebars.register_helper("rfc2822", Box::new(handlebars_helper::date_time_2822));
            handlebars.register_helper("rfc3339", Box::new(handlebars_helper::date_time_3339));
            handlebars.render_template_to_write(self.template, self.report, w)
                .context(MdRenderingFailed {})
        }
    }
}

mod handlebars_helper {
    use chrono::{DateTime, Local};
    use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderError};


    pub(crate) fn date_time_2822(h: &Helper, _: &Handlebars, _: &Context, rc: &mut RenderContext, out: &mut dyn Output) -> HelperResult {
        let dt = date_param(h)?;
        out.write(&dt.to_rfc2822()).map_err(|e| RenderError::with(e))
    }

    pub(crate) fn date_time_3339(h: &Helper, _: &Handlebars, _: &Context, rc: &mut RenderContext, out: &mut dyn Output) -> HelperResult {
        let dt = date_param(h)?;
        out.write(&dt.to_rfc3339()).map_err(|e| RenderError::with(e))
    }

    fn date_param(h: &Helper) -> ::std::result::Result<DateTime<Local>, RenderError> {
        let dt_str = h.param(0)
            .ok_or(RenderError::new("no such parameter"))?
            .value()
            .as_str()
            .ok_or(RenderError::new("parameter is not a string"))?;
        dt_str.parse::<DateTime<Local>>().map_err(|e| RenderError::with(e))
    }
}