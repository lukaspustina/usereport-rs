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

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::analysis::{AnalysisReport, Context};
        use googletest::prelude::*;

        fn empty_report() -> AnalysisReport {
            AnalysisReport::new(Context::new(), vec![], vec![], 1, 64)
        }

        #[test]
        fn json_renderer_produces_valid_json() {
            let r = JsonRenderer::new();
            let mut out = Vec::new();
            let res = r.render(&empty_report(), &mut out);
            assert_that!(res, ok(anything()));
            let s = String::from_utf8(out).unwrap();
            assert_that!(serde_json::from_str::<serde_json::Value>(&s), ok(anything()));
        }

        #[test]
        fn json_renderer_contains_hostname() {
            let r = JsonRenderer::new();
            let mut out = Vec::new();
            r.render(&empty_report(), &mut out).unwrap();
            let s = String::from_utf8(out).unwrap();
            assert_that!(s, contains_substring("hostname"));
        }
    }
}

pub mod jinja {
    use super::*;

    use std::{fs::File, io::Read, path::Path};

    #[derive(Default, Debug, Eq, PartialEq, Clone)]
    pub struct TemplateRenderer {
        template:    String,
        html_escape: bool,
    }

    impl TemplateRenderer {
        pub fn new<T: Into<String>>(template: T) -> Self {
            TemplateRenderer { template: template.into(), html_escape: false }
        }

        pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
            let mut file = File::open(path.as_ref())
                .map_err(|e| Error::ReadTemplateFailed { path: path.as_ref().to_path_buf(), source: e })?;
            let mut template = String::new();
            file.read_to_string(&mut template)
                .map_err(|e| Error::ReadTemplateFailed { path: path.as_ref().to_path_buf(), source: e })?;
            Ok(TemplateRenderer { template, html_escape: false })
        }

        pub fn with_html_escape(self) -> Self {
            TemplateRenderer { html_escape: true, ..self }
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
            if self.html_escape {
                env.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
            } else {
                env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
            }
            env.add_filter("rfc2822", rfc2822);
            let output = env.render_str(&self.template, report)?;
            w.write_all(output.as_bytes())
                .map_err(|e| Error::WriteOutputFailed { source: e })?;
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::analysis::{AnalysisReport, Context};
        use googletest::prelude::*;

        fn empty_report() -> AnalysisReport {
            AnalysisReport::new(Context::new(), vec![], vec![], 1, 64)
        }

        #[test]
        fn template_renderer_new_renders_literal() {
            let r = TemplateRenderer::new("hello world");
            let mut out = Vec::new();
            let res = r.render(&empty_report(), &mut out);
            assert_that!(res, ok(anything()));
            assert_that!(String::from_utf8(out).unwrap(), eq("hello world"));
        }

        #[test]
        fn template_renderer_new_renders_field() {
            let r = TemplateRenderer::new("{{ repetitions }}");
            let mut out = Vec::new();
            r.render(&empty_report(), &mut out).unwrap();
            assert_that!(String::from_utf8(out).unwrap(), eq("1"));
        }

        #[test]
        fn template_renderer_from_file_missing_returns_err() {
            let res = TemplateRenderer::from_file("/no/such/file.j2");
            assert_that!(res, err(anything()));
        }

        #[test]
        fn template_renderer_invalid_template_returns_err() {
            let r = TemplateRenderer::new("{{ unclosed");
            let mut out = Vec::new();
            let res = r.render(&empty_report(), &mut out);
            assert_that!(res, err(anything()));
        }
    }
}

