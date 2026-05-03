//! SDD more-useful-firefight Phase 4, C1.
//! GIVEN cpu.iowait_pct = 23.4 in signals (no usr/sys available) AND a finding with evidence signal_id "cpu.iowait_pct" severity Warn
//! WHEN VitalSigns is computed
//! THEN cpu.active_pct == 23.4 (iowait fallback) and cpu.severity == Some(Warn).

use usereport::analysis::compute_vital_signs;
use usereport::finding::{Evidence, Finding, FindingKind, Severity};
use usereport::signal::{Signal, SignalValue, Unit};

fn make_signal(id: &str, value: f64) -> Signal {
    Signal {
        id: id.to_string(),
        value: SignalValue::F64(value),
        unit: Unit::Pct,
        at: chrono::Local::now(),
        samples: None,
        stats: None,
        baseline: None,
    }
}

#[test]
fn vital_signs_computed_from_signals_and_findings() {
    let signals = vec![make_signal("cpu.iowait_pct", 23.4)];
    let finding = Finding {
        id: "cpu.iowait_high".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "iowait".to_string(),
        evidence: vec![Evidence {
            signal_id: "cpu.iowait_pct".to_string(),
            observed: SignalValue::F64(23.4),
            source_commands: vec![],
        }],
        suggest: vec![],
    };
    let vs = compute_vital_signs(&signals, &[finding]);
    let got = vs.cpu.active_pct.expect("active_pct must be set (iowait fallback)");
    assert!((got - 23.4).abs() < 0.001, "active_pct must be 23.4 (iowait fallback), got {got}");
    assert_eq!(vs.cpu.severity, Some(Severity::Warn), "cpu severity must be Warn");
}
