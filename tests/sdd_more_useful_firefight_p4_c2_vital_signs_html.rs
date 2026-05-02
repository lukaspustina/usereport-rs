//! SDD more-useful-firefight Phase 4, C2.
//! GIVEN VitalSigns.cpu.iowait_pct = Some(23.4) and cpu.severity = Some(Warn)
//! WHEN the HTML report renders
//! THEN the CPU vital signs line contains "23" and "WARN".

use usereport::Renderer;
use usereport::analysis::{AnalysisReport, Context, CpuVitalSigns, VitalSigns};
use usereport::finding::Severity;
use usereport::renderer::TemplateRenderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn vital_signs_cpu_rendered_in_html() {
    let mut report =
        AnalysisReport::new_with_diagnostics(Context::new(), vec![], vec![], 1, 64, vec![], vec![], vec![]);
    report.vital_signs = VitalSigns {
        cpu: CpuVitalSigns {
            iowait_pct: Some(23.4),
            severity: Some(Severity::Warn),
            trend: None,
        },
        ..Default::default()
    };
    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("23"),
        "HTML must contain '23' from iowait_pct:\n{}",
        &s[..s.len().min(3000)]
    );
    assert!(
        s.contains("WARN") || s.contains("Warn"),
        "HTML must contain CPU severity 'WARN':\n{}",
        &s[..s.len().min(3000)]
    );
}
