//! SDD more-useful-firefight Phase 4, C6.
//! GIVEN a fired finding with id = "cpu.iowait_elevated" and a followup_recommendation
//!   matching that finding with recommend = "mem" and reason = "iowait often driven by memory pressure"
//! WHEN the HTML report renders
//! THEN a section appears containing "mem" and "iowait often driven by memory pressure".

use usereport::analysis::{AnalysisReport, Context, ProfileFollowup};
use usereport::finding::{Finding, FindingKind, Severity};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn followup_recommendation_rendered_in_html() {
    let finding = Finding {
        id: "cpu.iowait_elevated".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "iowait elevated".to_string(),
        evidence: vec![],
        suggest: vec![],
    };
    let mut report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![],
        1,
        64,
        vec![],
        vec![finding],
        vec![],
    );
    report.followup_recommendations = vec![ProfileFollowup {
        finding: "cpu.iowait_elevated".to_string(),
        recommend: "mem".to_string(),
        reason: "iowait often driven by memory pressure".to_string(),
    }];
    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("mem"),
        "HTML must contain 'mem' from followup recommendation:\n{}",
        &s[..s.len().min(3000)]
    );
    assert!(
        s.contains("iowait often driven by memory pressure"),
        "HTML must contain reason string:\n{}",
        &s[..s.len().min(3000)]
    );
}
