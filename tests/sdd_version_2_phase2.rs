//! Integration tests for SDD `specs/sdd/version-2.md` Phase 2 (baselines + diff).
#![cfg(feature = "bin")]

use std::collections::HashMap;

use usereport::analysis::{AnalysisReport, Context};
use usereport::baseline::stats::{mad, median, z_score};
use usereport::baseline::store::{BaselineRecord, BaselineStore};
use usereport::baseline::{annotate, outlier_findings};
use usereport::diff::{diff, DiffReport};
use usereport::finding::{Finding, FindingKind, Severity};
use usereport::signal::{Signal, SignalValue, Unit};

fn make_signal(id: &str, value: f64) -> Signal {
    Signal {
        id: id.to_string(),
        value: SignalValue::F64(value),
        unit: Unit::None,
        at: chrono::Local::now(),
        samples: None,
        baseline: None,
    }
}

// ---------------------------------------------------------------------------
// Criterion 1 — stats::median
// ---------------------------------------------------------------------------

#[test]
fn ac_phase2_1_median_odd_length() {
    assert_eq!(median(&[1.0, 2.0, 3.0, 4.0, 5.0]), Some(3.0));
}

#[test]
fn ac_phase2_1_median_even_length() {
    assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
}

#[test]
fn ac_phase2_1_median_empty_returns_none() {
    assert_eq!(median(&[]), None);
}

// ---------------------------------------------------------------------------
// Criterion 2 — stats::mad
// ---------------------------------------------------------------------------

#[test]
fn ac_phase2_2_mad_known_input() {
    // values = [1, 1, 2, 2, 4]; median = 2; deviations = [1, 1, 0, 0, 2];
    // sorted deviations = [0, 0, 1, 1, 2]; median(deviations) = 1.0
    assert_eq!(mad(&[1.0, 1.0, 2.0, 2.0, 4.0]), Some(1.0));
}

#[test]
fn ac_phase2_2_mad_empty_returns_none() {
    assert_eq!(mad(&[]), None);
}

// ---------------------------------------------------------------------------
// Criterion 3 — stats::z_score
// ---------------------------------------------------------------------------

#[test]
fn ac_phase2_3_z_score_known_input() {
    // modified z-score = 0.6745 * (value - p50) / mad
    // (10 - 3) / 1 = 7; 7 * 0.6745 = 4.7215
    let z = z_score(10.0, 3.0, 1.0);
    assert!((z - 4.7215).abs() < 0.001, "z = {}", z);
}

#[test]
fn ac_phase2_3_z_score_zero_mad_returns_zero() {
    assert_eq!(z_score(10.0, 3.0, 0.0), 0.0);
}

// ---------------------------------------------------------------------------
// Criterion 4 — BaselineStore record + load round-trip
// ---------------------------------------------------------------------------

#[test]
fn ac_phase2_4_baseline_record_load_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BaselineStore::at(tmp.path().to_path_buf());

    let signals = vec![make_signal("cpu.iowait_pct", 3.0), make_signal("mem.free_pct", 60.0)];
    store.record("green", &signals).expect("record");

    let loaded = store.load("green").expect("load").expect("present");
    assert_eq!(loaded.signals.get("cpu.iowait_pct"), Some(&3.0));
    assert_eq!(loaded.signals.get("mem.free_pct"), Some(&60.0));
}

#[test]
fn ac_phase2_4_baseline_load_missing_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BaselineStore::at(tmp.path().to_path_buf());
    let loaded = store.load("nonexistent").expect("load ok");
    assert!(loaded.is_none());
}

// ---------------------------------------------------------------------------
// Criterion 5 — Rolling JSONL pruning at N=24
// ---------------------------------------------------------------------------

#[test]
fn ac_phase2_5_rolling_jsonl_prunes_at_n() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BaselineStore::at(tmp.path().to_path_buf());
    let n = 24;

    // Append 30 records; oldest 6 should be pruned.
    for i in 0..30 {
        let signals = vec![make_signal("test.signal", i as f64)];
        store.append_rolling(&signals, n).expect("append");
    }

    let records = store.load_rolling().expect("load_rolling");
    assert_eq!(
        records.len(),
        n,
        "expected exactly {} records, got {}",
        n,
        records.len()
    );

    // The retained records should be the last 24 (values 6..30).
    let first_value = records.first().and_then(|r| r.signals.get("test.signal")).copied();
    let last_value = records.last().and_then(|r| r.signals.get("test.signal")).copied();
    assert_eq!(first_value, Some(6.0), "oldest retained value should be 6");
    assert_eq!(last_value, Some(29.0), "newest retained value should be 29");
}

// ---------------------------------------------------------------------------
// Criterion 6 — annotate populates Signal.baseline
// ---------------------------------------------------------------------------

#[test]
fn ac_phase2_6_annotate_populates_baseline_stats() {
    // Build a synthetic rolling baseline where cpu.iowait_pct varies around 3.0.
    let records: Vec<BaselineRecord> = (0..10)
        .map(|i| {
            let mut sigs = HashMap::new();
            // Values: 2.5, 2.6, 2.7, ..., 3.4 — median ≈ 3.0, mad ≈ 0.25
            sigs.insert("cpu.iowait_pct".to_string(), 2.5 + (i as f64) * 0.1);
            BaselineRecord {
                captured_at: chrono::Local::now(),
                signals: sigs,
            }
        })
        .collect();

    let mut signals = vec![make_signal("cpu.iowait_pct", 42.0)];
    annotate(&mut signals, &records);

    let baseline = signals[0].baseline.as_ref().expect("baseline annotated");
    // p50 should be close to 3.0 (within rounding). MAD ≈ 0.25.
    assert!((baseline.p50 - 2.95).abs() < 0.1, "p50 = {}", baseline.p50);
    assert!(baseline.mad > 0.0 && baseline.mad < 1.0, "mad = {}", baseline.mad);
    assert!(
        baseline.z_score.abs() > 10.0,
        "z_score should be huge for value=42 vs baseline median 3.0; got {}",
        baseline.z_score
    );
}

// ---------------------------------------------------------------------------
// Criterion 7 — Auto-outlier findings from rolling baseline
// ---------------------------------------------------------------------------

#[test]
fn ac_phase2_7_outlier_findings_warn_and_crit_thresholds() {
    use usereport::signal::BaselineStats;

    // Signal with z-score = 4.0 → warn (|z|>3)
    let mut warn_signal = make_signal("cpu.iowait_pct", 5.0);
    warn_signal.baseline = Some(BaselineStats {
        p50: 3.0,
        p95: 4.0,
        mad: 0.5,
        z_score: 4.0,
    });

    // Signal with z-score = 7.0 → crit (|z|>6)
    let mut crit_signal = make_signal("mem.free_pct", 1.0);
    crit_signal.baseline = Some(BaselineStats {
        p50: 50.0,
        p95: 60.0,
        mad: 5.0,
        z_score: -7.0,
    });

    // Signal with z-score = 1.0 → no finding (within ±3)
    let mut quiet_signal = make_signal("disk.util_pct", 30.0);
    quiet_signal.baseline = Some(BaselineStats {
        p50: 25.0,
        p95: 40.0,
        mad: 5.0,
        z_score: 1.0,
    });

    let findings = outlier_findings(&[warn_signal, crit_signal, quiet_signal]);

    let warn = findings.iter().find(|f| f.severity == Severity::Warn);
    let crit = findings.iter().find(|f| f.severity == Severity::Crit);
    assert!(warn.is_some(), "expected warn finding for |z|=4; got {:?}", findings);
    assert!(crit.is_some(), "expected crit finding for |z|=7; got {:?}", findings);

    let crit_unwrapped = crit.unwrap();
    assert!(
        crit_unwrapped.evidence.iter().any(|e| e.signal_id == "mem.free_pct"),
        "crit finding should cite mem.free_pct"
    );
    // Quiet signal must not produce a finding.
    assert!(
        !findings
            .iter()
            .any(|f| f.evidence.iter().any(|e| e.signal_id == "disk.util_pct")),
        "quiet signal must not emit a finding"
    );
}

// ---------------------------------------------------------------------------
// Criterion 8 — diff of two AnalysisReports
// ---------------------------------------------------------------------------

fn make_report(signals: Vec<Signal>, findings: Vec<Finding>) -> AnalysisReport {
    AnalysisReport::new_with_diagnostics(Context::new(), vec![], vec![], 1, 64, signals, findings, vec![])
}

fn make_finding(id: &str, severity: Severity) -> Finding {
    Finding {
        id: id.to_string(),
        kind: FindingKind::Rule,
        severity,
        summary: id.to_string(),
        evidence: vec![],
        suggest: vec![],
    }
}

#[test]
fn ac_phase2_8_diff_identical_reports_is_empty() {
    let a = make_report(
        vec![make_signal("x", 1.0)],
        vec![make_finding("rule.a", Severity::Warn)],
    );
    let b = make_report(
        vec![make_signal("x", 1.0)],
        vec![make_finding("rule.a", Severity::Warn)],
    );
    let d: DiffReport = diff(&a, &b);
    assert!(d.signals_only_in_a.is_empty());
    assert!(d.signals_only_in_b.is_empty());
    assert!(d.signal_deltas.is_empty());
    assert!(d.findings_only_in_a.is_empty());
    assert!(d.findings_only_in_b.is_empty());
    assert!(d.findings_severity_changed.is_empty());
}

#[test]
fn ac_phase2_8_diff_finds_signal_value_delta() {
    let a = make_report(vec![make_signal("x", 1.0)], vec![]);
    let b = make_report(vec![make_signal("x", 5.0)], vec![]);
    let d = diff(&a, &b);
    assert_eq!(d.signal_deltas.len(), 1);
    let delta = &d.signal_deltas[0];
    assert_eq!(delta.signal_id, "x");
    assert!((delta.delta - 4.0).abs() < f64::EPSILON);
}

#[test]
fn ac_phase2_8_diff_finds_only_in_a_and_only_in_b() {
    let a = make_report(
        vec![make_signal("only_a", 1.0)],
        vec![make_finding("f.a", Severity::Warn)],
    );
    let b = make_report(
        vec![make_signal("only_b", 2.0)],
        vec![make_finding("f.b", Severity::Crit)],
    );
    let d = diff(&a, &b);
    assert!(d.signals_only_in_a.iter().any(|s| s == "only_a"));
    assert!(d.signals_only_in_b.iter().any(|s| s == "only_b"));
    assert!(d.findings_only_in_a.iter().any(|f| f.id == "f.a"));
    assert!(d.findings_only_in_b.iter().any(|f| f.id == "f.b"));
}

#[test]
fn ac_phase2_8_diff_detects_severity_change() {
    let a = make_report(vec![], vec![make_finding("rule.x", Severity::Warn)]);
    let b = make_report(vec![], vec![make_finding("rule.x", Severity::Crit)]);
    let d = diff(&a, &b);
    assert_eq!(d.findings_severity_changed.len(), 1);
    let change = &d.findings_severity_changed[0];
    assert_eq!(change.finding_id, "rule.x");
    assert_eq!(change.severity_in_a, Severity::Warn);
    assert_eq!(change.severity_in_b, Severity::Crit);
}
