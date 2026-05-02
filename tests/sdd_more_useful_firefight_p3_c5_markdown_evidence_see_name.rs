//! SDD more-useful-firefight Phase 3, C5.
//! GIVEN Markdown output and evidence[0].source_commands = ["iostat"]
//! WHEN the template renders
//! THEN the evidence line contains the substring "(see: iostat)".

use usereport::analysis::{AnalysisReport, Context};
use usereport::finding::{Evidence, Finding, FindingKind, Severity};
use usereport::renderer::TemplateRenderer;
use usereport::signal::SignalValue;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

#[test]
fn markdown_evidence_appends_see_source_command_name() {
    let finding = Finding {
        id: "cpu.iowait_high".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "iowait elevated".to_string(),
        evidence: vec![Evidence {
            signal_id: "cpu.iowait_pct".to_string(),
            observed: SignalValue::F64(25.0),
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
    let renderer = TemplateRenderer::new(MARKDOWN);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("(see: iostat)"),
        "Markdown evidence must contain '(see: iostat)':\n{}",
        s
    );
}
