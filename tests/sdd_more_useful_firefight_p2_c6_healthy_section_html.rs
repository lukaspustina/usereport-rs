//! SDD more-useful-firefight Phase 2, C6.
//! GIVEN checked_ok = ["cpu.iowait_pct"] in AnalysisReport
//! WHEN the HTML report renders
//! THEN the output contains "Healthy" and "cpu.iowait_pct".

use usereport::analysis::{AnalysisReport, Context};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn healthy_section_in_html_when_checked_ok_non_empty() {
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

    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("Healthy"),
        "HTML must contain 'Healthy' section:\n{}",
        &s[..s.len().min(3000)]
    );
    assert!(
        s.contains("cpu.iowait_pct"),
        "HTML Healthy section must list 'cpu.iowait_pct':\n{}",
        &s[..s.len().min(3000)]
    );
}
