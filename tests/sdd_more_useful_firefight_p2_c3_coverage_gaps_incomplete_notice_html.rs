//! SDD more-useful-firefight Phase 2, C3.
//! GIVEN sar_cpu is skipped
//! WHEN the HTML Coverage Gaps section renders
//! THEN the output contains the string "findings may be incomplete".

use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn coverage_gaps_incomplete_notice_in_html() {
    let result = CommandResult::SkippedMissing {
        command: Command::new("sar_cpu", "sar"),
        binary: "sar".to_string(),
    };
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![result]],
        1,
        64,
        vec![],
        vec![],
        vec![],
    );
    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("findings may be incomplete"),
        "HTML Coverage Gaps must contain 'findings may be incomplete':\n{}",
        &s[..s.len().min(2000)]
    );
}
