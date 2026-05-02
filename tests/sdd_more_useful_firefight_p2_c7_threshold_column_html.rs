//! SDD more-useful-firefight Phase 2, C7.
//! GIVEN a rule predicate cpu.iowait_pct > 20 labelled warn and signal value 15.0
//! WHEN the HTML Signals table renders
//! THEN the threshold column row for cpu.iowait_pct contains "20" and "warn".

use std::collections::HashMap;
use usereport::analysis::{AnalysisReport, Context, ThresholdInfo};
use usereport::finding::Severity;
use usereport::renderer::TemplateRenderer;
use usereport::signal::{Signal, SignalValue, Unit};
use usereport::Renderer;

const HTML: &str = include_str!("../contrib/html.j2");

fn make_signal(id: &str, value: f64) -> Signal {
    Signal {
        id: id.to_string(),
        value: SignalValue::F64(value),
        unit: Unit::None,
        at: chrono::Local::now(),
        samples: None,
        stats: None,
        baseline: None,
    }
}

#[test]
fn threshold_column_in_html_shows_value_and_severity() {
    let mut thresholds = HashMap::new();
    thresholds.insert(
        "cpu.iowait_pct".to_string(),
        ThresholdInfo {
            severity: Severity::Warn,
            op: ">".to_string(),
            value: 20.0,
        },
    );
    let mut report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![],
        1,
        64,
        vec![make_signal("cpu.iowait_pct", 15.0)],
        vec![],
        vec![],
    );
    report.signal_thresholds = thresholds;

    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("20"),
        "HTML threshold column must contain '20':\n{}",
        &s[..s.len().min(3000)]
    );
    assert!(
        s.contains("warn") || s.contains("Warn"),
        "HTML threshold column must contain severity 'warn':\n{}",
        &s[..s.len().min(3000)]
    );
}
