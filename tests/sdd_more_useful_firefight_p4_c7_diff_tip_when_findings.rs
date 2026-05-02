//! SDD more-useful-firefight Phase 4, C7.
//! GIVEN findings is non-empty
//! WHEN the report renders
//! THEN the output contains the string "usereport diff".

use usereport::analysis::{AnalysisReport, Context};
use usereport::finding::{Finding, FindingKind, Severity};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

#[test]
fn diff_tip_present_when_findings_non_empty() {
    let finding = Finding {
        id: "cpu.iowait_elevated".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "iowait elevated".to_string(),
        evidence: vec![],
        suggest: vec![],
    };
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![],
        1,
        64,
        vec![],
        vec![finding],
        vec![],
    );
    let renderer = TemplateRenderer::new(MARKDOWN);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("usereport diff"),
        "output must contain 'usereport diff' when findings non-empty:\n{}",
        s
    );
}
