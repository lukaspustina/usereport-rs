//! Integration tests for SDD `specs/sdd/version-2.md` Phase 4 (time-sampled collection).
#![cfg(feature = "bin")]

use usereport::baseline::stats::sample_stats;
use usereport::collector::CollectCtx;
use usereport::rule::{Predicate, SignalIndex};
use usereport::signal::{Signal, SignalValue, Trend, Unit};

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

fn make_sampled_signal(id: &str, samples: Vec<f64>) -> Signal {
    Signal {
        id: id.to_string(),
        value: SignalValue::F64(samples[0]),
        unit: Unit::None,
        at: chrono::Local::now(),
        samples: Some(samples),
        baseline: None,
    }
}

fn ctx() -> CollectCtx {
    CollectCtx {
        duration: None,
        interval: None,
        cgroup_path: None,
        baseline: None,
        cpu_count: 4,
    }
}

// ---------------------------------------------------------------------------
// Criterion 1 — sample_stats computation
// ---------------------------------------------------------------------------

#[test]
fn ac_phase4_1_sample_stats_min_max() {
    let vals = [3.0, 1.0, 5.0, 2.0, 4.0];
    let stats = sample_stats(&vals).expect("non-empty");
    assert!((stats.min - 1.0).abs() < f64::EPSILON, "min = {}", stats.min);
    assert!((stats.max - 5.0).abs() < f64::EPSILON, "max = {}", stats.max);
}

#[test]
fn ac_phase4_1_sample_stats_p50() {
    let vals = [1.0, 2.0, 3.0, 4.0, 5.0];
    let stats = sample_stats(&vals).expect("non-empty");
    assert!((stats.p50 - 3.0).abs() < f64::EPSILON, "p50 = {}", stats.p50);
}

#[test]
fn ac_phase4_1_sample_stats_p95() {
    // 11 values [1..=11]; p95 rank = 0.95 * 10 = 9.5 → between 10.0 and 11.0 → 10.5
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect();
    let stats = sample_stats(&vals).expect("non-empty");
    assert!((stats.p95 - 10.5).abs() < 0.01, "p95 = {}", stats.p95);
}

#[test]
fn ac_phase4_1_sample_stats_empty_returns_none() {
    assert!(sample_stats(&[]).is_none());
}

// ---------------------------------------------------------------------------
// Criterion 2 — trend via linear regression
// ---------------------------------------------------------------------------

#[test]
fn ac_phase4_2_trend_rising_from_linearly_increasing() {
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect();
    let stats = sample_stats(&vals).expect("non-empty");
    assert_eq!(stats.trend, Trend::Rising, "expected Rising, got {:?}", stats.trend);
}

#[test]
fn ac_phase4_2_trend_falling_from_linearly_decreasing() {
    let vals: Vec<f64> = (1..=11).rev().map(|i| i as f64).collect();
    let stats = sample_stats(&vals).expect("non-empty");
    assert_eq!(stats.trend, Trend::Falling, "expected Falling, got {:?}", stats.trend);
}

#[test]
fn ac_phase4_2_trend_flat_from_constant() {
    let vals = vec![5.0f64; 11];
    let stats = sample_stats(&vals).expect("non-empty");
    assert_eq!(stats.trend, Trend::Flat, "expected Flat, got {:?}", stats.trend);
}

// ---------------------------------------------------------------------------
// Criterion 3 — predicate evaluator resolves .p50 / .p95 / .min / .max
// ---------------------------------------------------------------------------

#[test]
fn ac_phase4_3_predicate_p95_suffix_fires_when_above_threshold() {
    let p = Predicate::parse("cpu.iowait_pct.p95 > 8.0").expect("parse");
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect();
    let signals = vec![make_sampled_signal("cpu.iowait_pct", vals)];
    let idx = SignalIndex::build(&signals);
    assert!(p.evaluate(&idx, &ctx()), "expected predicate to fire");
}

#[test]
fn ac_phase4_3_predicate_p95_suffix_does_not_fire_below_threshold() {
    let p = Predicate::parse("cpu.iowait_pct.p95 > 20.0").expect("parse");
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect();
    let signals = vec![make_sampled_signal("cpu.iowait_pct", vals)];
    let idx = SignalIndex::build(&signals);
    assert!(!p.evaluate(&idx, &ctx()), "expected predicate not to fire");
}

#[test]
fn ac_phase4_3_predicate_p50_suffix_evaluates() {
    let p = Predicate::parse("cpu.iowait_pct.p50 > 5.0").expect("parse");
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect(); // p50 = 6.0
    let signals = vec![make_sampled_signal("cpu.iowait_pct", vals)];
    let idx = SignalIndex::build(&signals);
    assert!(p.evaluate(&idx, &ctx()), "p50 should be 6.0 > 5.0");
}

#[test]
fn ac_phase4_3_predicate_min_suffix_evaluates() {
    let p = Predicate::parse("cpu.iowait_pct.min < 2.0").expect("parse");
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect(); // min = 1.0
    let signals = vec![make_sampled_signal("cpu.iowait_pct", vals)];
    let idx = SignalIndex::build(&signals);
    assert!(p.evaluate(&idx, &ctx()), "min should be 1.0 < 2.0");
}

#[test]
fn ac_phase4_3_predicate_max_suffix_evaluates() {
    let p = Predicate::parse("cpu.iowait_pct.max > 10.0").expect("parse");
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect(); // max = 11.0
    let signals = vec![make_sampled_signal("cpu.iowait_pct", vals)];
    let idx = SignalIndex::build(&signals);
    assert!(p.evaluate(&idx, &ctx()), "max should be 11.0 > 10.0");
}

// ---------------------------------------------------------------------------
// Criterion 4 — predicate evaluator resolves .trend suffix
// ---------------------------------------------------------------------------

#[test]
fn ac_phase4_4_predicate_trend_rising_fires() {
    let p = Predicate::parse("cpu.load.trend == \"rising\"").expect("parse");
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect();
    let signals = vec![make_sampled_signal("cpu.load", vals)];
    let idx = SignalIndex::build(&signals);
    assert!(p.evaluate(&idx, &ctx()), "rising trend predicate should fire");
}

#[test]
fn ac_phase4_4_predicate_trend_falling_fires() {
    let p = Predicate::parse("cpu.load.trend == \"falling\"").expect("parse");
    let vals: Vec<f64> = (1..=11).rev().map(|i| i as f64).collect();
    let signals = vec![make_sampled_signal("cpu.load", vals)];
    let idx = SignalIndex::build(&signals);
    assert!(p.evaluate(&idx, &ctx()), "falling trend predicate should fire");
}

#[test]
fn ac_phase4_4_predicate_trend_does_not_fire_for_wrong_direction() {
    let p = Predicate::parse("cpu.load.trend == \"falling\"").expect("parse");
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect(); // rising, not falling
    let signals = vec![make_sampled_signal("cpu.load", vals)];
    let idx = SignalIndex::build(&signals);
    assert!(!p.evaluate(&idx, &ctx()), "falling predicate must not fire on rising trend");
}

// ---------------------------------------------------------------------------
// Criterion 5 — bare signal with samples resolves to p50
// ---------------------------------------------------------------------------

#[test]
fn ac_phase4_5_bare_signal_with_samples_uses_p50() {
    // Signal::value = 1.0 (intentionally low); samples p50 = 6.0
    // Predicate should use p50, not value.
    let p = Predicate::parse("cpu.load > 5.0").expect("parse");
    let vals: Vec<f64> = (1..=11).map(|i| i as f64).collect(); // p50 = 6.0
    let mut signal = make_sampled_signal("cpu.load", vals);
    signal.value = SignalValue::F64(1.0); // intentionally low to distinguish
    let signals = vec![signal];
    let idx = SignalIndex::build(&signals);
    assert!(p.evaluate(&idx, &ctx()), "bare signal with samples should use p50 (6.0 > 5.0)");
}

#[test]
fn ac_phase4_5_bare_signal_without_samples_uses_value() {
    let p = Predicate::parse("cpu.load > 5.0").expect("parse");
    let signals = vec![make_signal("cpu.load", 7.0)];
    let idx = SignalIndex::build(&signals);
    assert!(p.evaluate(&idx, &ctx()), "bare signal without samples should use value (7.0 > 5.0)");
}

// ---------------------------------------------------------------------------
// Criterion 6 — CLI rejects --interval without --duration
// ---------------------------------------------------------------------------

#[test]
fn ac_phase4_6_interval_without_duration_rejected() {
    let bin = env!("CARGO_BIN_EXE_usereport");
    let output = std::process::Command::new(bin)
        .args(["--interval", "5s"])
        .output()
        .expect("failed to run binary");
    assert!(
        !output.status.success(),
        "expected non-zero exit when --interval given without --duration"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("duration") || stderr.contains("--duration"),
        "error message should reference --duration; got: {}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// Criterion 7 — --duration and --repetitions are mutually exclusive
// ---------------------------------------------------------------------------

#[test]
fn ac_phase4_7_duration_and_repetitions_are_mutually_exclusive() {
    let bin = env!("CARGO_BIN_EXE_usereport");
    let output = std::process::Command::new(bin)
        .args(["--duration", "10s", "--repetitions", "3"])
        .output()
        .expect("failed to run binary");
    assert!(
        !output.status.success(),
        "expected non-zero exit when --duration and --repetitions are combined"
    );
}
