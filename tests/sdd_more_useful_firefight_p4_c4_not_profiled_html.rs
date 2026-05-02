//! SDD more-useful-firefight Phase 4, C4.
//! GIVEN all network signal fields are None
//! WHEN the HTML report renders the Network vital signs line
//! THEN the line contains "[not profiled]".

use usereport::Renderer;
use usereport::analysis::{AnalysisReport, Context};
use usereport::renderer::TemplateRenderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn html_network_vital_signs_not_profiled_when_no_data() {
    let report = AnalysisReport::new_with_diagnostics(Context::new(), vec![], vec![], 1, 64, vec![], vec![], vec![]);
    // vital_signs defaults to all-None, so network shows [not profiled]
    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("[not profiled]"),
        "HTML must contain '[not profiled]' for unprofiled resources:\n{}",
        &s[..s.len().min(3000)]
    );
}
