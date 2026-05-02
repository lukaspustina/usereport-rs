//! SDD more-useful-firefight Phase 4, C8.
//! GIVEN findings is empty
//! WHEN the report renders
//! THEN the output does NOT contain the string "usereport diff".

use usereport::analysis::{AnalysisReport, Context};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

#[test]
fn diff_tip_absent_when_no_findings() {
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![],
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
        !s.contains("usereport diff"),
        "output must NOT contain 'usereport diff' when findings empty:\n{}",
        s
    );
}
