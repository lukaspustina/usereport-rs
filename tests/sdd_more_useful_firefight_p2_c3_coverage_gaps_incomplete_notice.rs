//! SDD more-useful-firefight Phase 2, C3.
//! GIVEN sar_cpu is skipped
//! WHEN the Coverage Gaps section renders
//! THEN the output contains the exact string "findings may be incomplete".

use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

#[test]
fn coverage_gaps_shows_findings_may_be_incomplete() {
    let skipped = CommandResult::SkippedMissing {
        command: Command::new("sar_cpu", "sar -u 1 5"),
        binary: "sar".to_string(),
    };
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![skipped]],
        1,
        64,
        vec![],
        vec![],
        vec![],
    );
    let renderer = TemplateRenderer::new(MARKDOWN);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("findings may be incomplete"),
        "Coverage Gaps must contain exact string 'findings may be incomplete':\n{}",
        s
    );
}
