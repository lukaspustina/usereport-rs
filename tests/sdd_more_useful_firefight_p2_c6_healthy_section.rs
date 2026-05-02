//! SDD more-useful-firefight Phase 2, C6.
//! GIVEN checked_ok = ["cpu.iowait_pct"] in AnalysisReport
//! WHEN the report renders
//! THEN the output contains "Healthy" and "cpu.iowait_pct".

use usereport::analysis::{AnalysisReport, Context};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

#[test]
fn healthy_section_appears_when_checked_ok_nonempty() {
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![],
        1,
        64,
        vec![],
        vec![],
        vec!["cpu.iowait_pct".to_string()],
    );
    let renderer = TemplateRenderer::new(MARKDOWN);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("Healthy"),
        "rendered output must contain 'Healthy':\n{}",
        s
    );
    assert!(
        s.contains("cpu.iowait_pct"),
        "rendered output must contain 'cpu.iowait_pct' in Healthy section:\n{}",
        s
    );
}
