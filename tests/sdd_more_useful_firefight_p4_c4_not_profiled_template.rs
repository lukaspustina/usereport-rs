//! SDD more-useful-firefight Phase 4, C4.
//! GIVEN all network vital sign fields are None
//! WHEN the report renders the Network vital signs line
//! THEN the line contains "[not profiled]".

use usereport::analysis::{AnalysisReport, Context, VitalSigns};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

#[test]
fn network_vital_signs_shows_not_profiled() {
    let mut report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![],
        1,
        64,
        vec![],
        vec![],
        vec![],
    );
    // VitalSigns with all defaults (all None)
    report.vital_signs = VitalSigns::default();
    let renderer = TemplateRenderer::new(MARKDOWN);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("[not profiled]"),
        "rendered output must contain '[not profiled]' for empty network vital signs:\n{}",
        s
    );
}
