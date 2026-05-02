//! SDD more-useful-firefight Phase 4, C2.
//! GIVEN VitalSigns.cpu.iowait_pct = Some(23.4) and cpu.severity = Some(Warn)
//! WHEN the report renders
//! THEN the CPU vital signs line contains "23" and "WARN".

use usereport::analysis::{AnalysisReport, Context, VitalSigns, CpuVitalSigns};
use usereport::finding::Severity;
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

#[test]
fn vital_signs_cpu_rendered_in_template() {
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
    report.vital_signs = VitalSigns {
        cpu: CpuVitalSigns {
            iowait_pct: Some(23.4),
            severity: Some(Severity::Warn),
            trend: None,
        },
        ..Default::default()
    };
    let renderer = TemplateRenderer::new(MARKDOWN);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("23"),
        "rendered output must contain '23' from iowait_pct:\n{}",
        s
    );
    assert!(
        s.contains("WARN") || s.contains("Warn"),
        "rendered output must contain CPU severity 'WARN':\n{}",
        s
    );
}
