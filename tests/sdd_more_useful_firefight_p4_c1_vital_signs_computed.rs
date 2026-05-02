//! SDD more-useful-firefight Phase 4, C1.
//! GIVEN cpu.iowait_pct = 23.4 in signals AND a finding with evidence signal_id = "cpu.iowait_pct" severity Warn
//! WHEN VitalSigns is computed
//! THEN cpu.iowait_pct == 23.4 and cpu.severity == Some(Warn).

use usereport::analysis::compute_vital_signs;
use usereport::finding::{Evidence, Finding, FindingKind, Severity};
use usereport::signal::{Signal, SignalValue, Unit};

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
fn vital_signs_cpu_iowait_and_severity_computed() {
    let signals = vec![make_signal("cpu.iowait_pct", 23.4)];
    let findings = vec![Finding {
        id: "cpu.iowait_high".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "iowait high".to_string(),
        evidence: vec![Evidence {
            signal_id: "cpu.iowait_pct".to_string(),
            observed: SignalValue::F64(23.4),
            source_commands: Vec::new(),
        }],
        suggest: vec![],
    }];
    let vs = compute_vital_signs(&signals, &findings);
    let iowait = vs.cpu.iowait_pct.expect("cpu.iowait_pct must be present");
    assert!(
        (iowait - 23.4).abs() < 0.001,
        "cpu.iowait_pct must be 23.4, got {}",
        iowait
    );
    assert_eq!(
        vs.cpu.severity,
        Some(Severity::Warn),
        "cpu.severity must be Some(Warn)"
    );
}
