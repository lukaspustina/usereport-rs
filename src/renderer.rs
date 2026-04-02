use crate::analysis::AnalysisReport;

use std::{fmt::Debug, io::Write, path::PathBuf};
use thiserror::Error;

/// Error type
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    /// Rendering of report to Json failed
    #[error("failed to render report to Json: {source}")]
    RenderJsonFailed { #[from] source: serde_json::Error },
    /// Failed to read template from file
    #[error("failed to read template from file '{path}': {source}")]
    ReadTemplateFailed { path: PathBuf, source: std::io::Error },
    /// Template rendering failed (parse or render error)
    #[error("failed to render template: {source}")]
    RenderTemplateFailed { #[from] source: minijinja::Error },
    /// Failed to write rendered output
    #[error("failed to write output: {source}")]
    WriteOutputFailed { source: std::io::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub trait Renderer<W: Write>: Debug {
    fn render(&self, report: &AnalysisReport, w: W) -> Result<()>;
}

pub use jinja::TemplateRenderer;
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
            Ok(serde_json::to_writer(w, report)?)
        }
    }
}

pub mod jinja {
    use super::*;

    use std::{fs::File, io::Read, path::Path};

    #[derive(Default, Debug, Eq, PartialEq, Clone)]
    pub struct TemplateRenderer {
        template: String,
    }

    impl TemplateRenderer {
        pub fn new<T: Into<String>>(template: T) -> Self {
            TemplateRenderer { template: template.into() }
        }

        pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
            let mut file = File::open(path.as_ref())
                .map_err(|e| Error::ReadTemplateFailed { path: path.as_ref().to_path_buf(), source: e })?;
            let mut template = String::new();
            file.read_to_string(&mut template)
                .map_err(|e| Error::ReadTemplateFailed { path: path.as_ref().to_path_buf(), source: e })?;
            Ok(TemplateRenderer { template })
        }
    }

    fn rfc2822(s: String) -> String {
        chrono::DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.to_rfc2822())
            .unwrap_or(s)
    }

    impl<W: Write> Renderer<W> for TemplateRenderer {
        fn render(&self, report: &AnalysisReport, mut w: W) -> Result<()> {
            let mut env = minijinja::Environment::new();
            env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
            env.add_filter("rfc2822", rfc2822);
            let output = env.render_str(&self.template, report)?;
            w.write_all(output.as_bytes())
                .map_err(|e| Error::WriteOutputFailed { source: e })?;
            Ok(())
        }
    }
}
