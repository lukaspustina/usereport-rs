//! SDD more-useful-firefight Phase 3, C2.
//! GIVEN Evidence { signal_id: "disk.util_pct", source_commands: ["iostat"] }
//! WHEN the HTML template renders the finding
//! THEN the evidence line contains href="#cmd-iostat".

use usereport::analysis::{AnalysisReport, Context};
use usereport::finding::{Evidence, Finding, FindingKind, Severity};
use usereport::renderer::TemplateRenderer;
use usereport::signal::SignalValue;
use usereport::Renderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn html_evidence_has_anchor_href() {
    let finding = Finding {
        id: "disk.util_high".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "disk utilization high".to_string(),
        evidence: vec![Evidence {
            signal_id: "disk.util_pct".to_string(),
            observed: SignalValue::F64(95.0),
            source_commands: vec!["iostat".to_string()],
        }],
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
    let renderer = TemplateRenderer::new(HTML).with_html_escape();
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("href=\"#cmd-iostat\""),
        "HTML evidence must contain href=\"#cmd-iostat\":\n{}",
        &s[..s.len().min(2000)]
    );
}
