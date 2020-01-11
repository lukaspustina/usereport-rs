use crate::analysis::AnalysisReport;

use snafu::{ResultExt, Snafu};
use std::{io::Write, path::PathBuf};

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Rendering of report to Json failed
    #[snafu(display("failed to render report to Json: {}", source))]
    RenderJsonFailed { source: serde_json::Error },
    /// Failed to read handlebars template from file
    #[snafu(display("failed to read handlebars template from file '{}': {}", path.display(), source))]
    ReadHbsTemplateFileFailed { path: PathBuf, source: std::io::Error },
    /// Handlebars template for Markdown is invalid
    #[snafu(display("Handlebars template is invalid: {}", source))]
    InvalidHbsTemplate { source: ::handlebars::TemplateError },
    /// Rendering of report to Markdown failed
    #[snafu(display("failed to render handlebars template: {}", source))]
    RenderHbsTemplateFailed { source: ::handlebars::RenderError },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub trait Renderer<W: Write> {
    fn render(&self, report: &AnalysisReport, w: W) -> Result<()>;
}

pub use crate::renderer::handlebars::HbsRenderer;
pub use json::JsonRenderer;

pub mod json {
    use super::*;

    #[derive(Default, Debug, Eq, PartialEq, Clone)]
    pub struct JsonRenderer {}

    impl JsonRenderer {
        pub fn new() -> Self { JsonRenderer {} }
    }

    impl<W: Write> Renderer<W> for JsonRenderer {
        fn render(&self, report: &AnalysisReport, w: W) -> Result<()> {
            serde_json::to_writer(w, report).context(RenderJsonFailed {})
        }
    }
}

pub mod handlebars {
    use super::*;

    use ::handlebars::Handlebars;
    use std::{fs::File, io::Read, path::Path};

    #[derive(Default, Debug, Eq, PartialEq, Clone)]
    pub struct HbsRenderer {
        template: String,
    }

    impl HbsRenderer {
        pub fn new<T: Into<String>>(template: T) -> Self {
            HbsRenderer {
                template: template.into(),
            }
        }

        pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
            let mut file = File::open(path.as_ref()).context(ReadHbsTemplateFileFailed {
                path: path.as_ref().to_path_buf(),
            })?;
            let mut template = String::new();
            file.read_to_string(&mut template).context(ReadHbsTemplateFileFailed {
                path: path.as_ref().to_path_buf(),
            })?;

            Ok(HbsRenderer { template })
        }
    }

    impl<W: Write> Renderer<W> for HbsRenderer {
        fn render(&self, report: &AnalysisReport, w: W) -> Result<()> {
            let mut handlebars = Handlebars::new();
            handlebars.register_helper("inc", Box::new(helpers::inc));
            handlebars.register_helper("rfc2822", Box::new(helpers::date_time_2822));
            handlebars.register_helper("rfc3339", Box::new(helpers::date_time_3339));
            handlebars
                .register_template_string("markdown", &self.template)
                .context(InvalidHbsTemplate {})?;
            handlebars
                .render_to_write("markdown", report, w)
                .context(RenderHbsTemplateFailed {})
        }
    }

    mod helpers {
        use chrono::{DateTime, Local};
        use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderError};

        pub(crate) fn inc(
            h: &Helper,
            _: &Handlebars,
            _: &Context,
            _: &mut RenderContext,
            out: &mut dyn Output,
        ) -> HelperResult {
            let value = h
                .param(0)
                .ok_or_else(|| RenderError::new("no such parameter"))?
                .value()
                .as_i64()
                .ok_or_else(|| RenderError::new("parameter is not a number"))?;
            let inc = format!("{}", value + 1);
            out.write(&inc).map_err(RenderError::with)
        }

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
}
