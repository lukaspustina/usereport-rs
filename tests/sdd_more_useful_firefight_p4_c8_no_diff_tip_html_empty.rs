//! SDD more-useful-firefight Phase 4, C8.
//! GIVEN findings is empty
//! WHEN the HTML report renders
//! THEN the output does NOT contain the string "usereport diff".

use usereport::Renderer;
use usereport::analysis::{AnalysisReport, Context};
use usereport::renderer::TemplateRenderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn no_diff_tip_in_html_when_findings_empty() {
    let report = AnalysisReport::new_with_diagnostics(Context::new(), vec![], vec![], 1, 64, vec![], vec![], vec![]);
    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        !s.contains("usereport diff"),
        "HTML must NOT contain 'usereport diff' when findings is empty:\n{}",
        &s[..s.len().min(2000)]
    );
}
