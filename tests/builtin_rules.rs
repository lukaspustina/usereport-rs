//! Firing tests for built-in rules that previously had no coverage (Fix 28).
//!
//! Each test constructs a minimal signal set, runs it through the rule engine,
//! and asserts the specific rule finding fires (or does not fire).
#![cfg(feature = "bin")]

use usereport::collector::CollectCtx;
use usereport::rule::{Predicate, Rule, RuleEngine, builtin::builtin_rules};
use usereport::signal::{Signal, SignalValue, Unit};

fn ctx() -> CollectCtx {
    CollectCtx {
        duration: None,
        interval: None,
        cgroup_path: None,
        baseline: None,
        cpu_count: 4,
    }
}

fn signal(id: &str, value: f64) -> Signal {
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

fn engine() -> RuleEngine {
    RuleEngine::new(builtin_rules())
}

fn fires(rule_id: &str, signals: Vec<Signal>) -> bool {
    let ctx = ctx();
    let (findings, _) = engine().run(&signals, &ctx, &std::collections::HashMap::new());
    findings.iter().any(|f| f.id == rule_id)
}

// cpu.iowait_elevated: fires when cpu.iowait_pct > 20
#[test]
fn cpu_iowait_elevated_fires_above_threshold() {
    assert!(fires("cpu.iowait_elevated", vec![signal("cpu.iowait_pct", 25.0)]));
}

#[test]
fn cpu_iowait_elevated_does_not_fire_below_threshold() {
    assert!(!fires("cpu.iowait_elevated", vec![signal("cpu.iowait_pct", 15.0)]));
}

// cpu.saturation: fires when cpu.usr_pct > 80
#[test]
fn cpu_saturation_fires_above_threshold() {
    assert!(fires("cpu.saturation", vec![signal("cpu.usr_pct", 85.0)]));
}

#[test]
fn cpu_saturation_does_not_fire_below_threshold() {
    assert!(!fires("cpu.saturation", vec![signal("cpu.usr_pct", 70.0)]));
}

// cpu.frequency_throttling: fires when cpu.freq_ratio < 0.8
#[test]
fn cpu_frequency_throttling_fires_below_threshold() {
    assert!(fires("cpu.frequency_throttling", vec![signal("cpu.freq_ratio", 0.5)]));
}

#[test]
fn cpu_frequency_throttling_does_not_fire_above_threshold() {
    assert!(!fires("cpu.frequency_throttling", vec![signal("cpu.freq_ratio", 0.9)]));
}

// mem.pressure: fires when mem.free_pct < 10
#[test]
fn mem_pressure_fires_below_threshold() {
    assert!(fires("mem.pressure", vec![signal("mem.free_pct", 5.0)]));
}

#[test]
fn mem_pressure_does_not_fire_above_threshold() {
    assert!(!fires("mem.pressure", vec![signal("mem.free_pct", 20.0)]));
}

// mem.swap_in_active: fires when vmstat.swap_in > 0
#[test]
fn mem_swap_in_active_fires_above_zero() {
    assert!(fires("mem.swap_in_active", vec![signal("vmstat.swap_in", 1.0)]));
}

#[test]
fn mem_swap_in_active_does_not_fire_at_zero() {
    assert!(!fires("mem.swap_in_active", vec![signal("vmstat.swap_in", 0.0)]));
}

// disk.utilization_saturated: fires when disk.max_util_pct > 90
#[test]
fn disk_utilization_saturated_fires_above_threshold() {
    assert!(fires("disk.utilization_saturated", vec![signal("disk.max_util_pct", 95.0)]));
}

#[test]
fn disk_utilization_saturated_does_not_fire_below_threshold() {
    assert!(!fires("disk.utilization_saturated", vec![signal("disk.max_util_pct", 80.0)]));
}

// disk.await_elevated: fires when disk.max_await_ms > 100
#[test]
fn disk_await_elevated_fires_above_threshold() {
    assert!(fires("disk.await_elevated", vec![signal("disk.max_await_ms", 150.0)]));
}

#[test]
fn disk_await_elevated_does_not_fire_below_threshold() {
    assert!(!fires("disk.await_elevated", vec![signal("disk.max_await_ms", 50.0)]));
}

// net.retransmit_elevated: fires when net.retrans_pct > 1
#[test]
fn net_retransmit_elevated_fires_above_threshold() {
    assert!(fires("net.retransmit_elevated", vec![signal("net.retrans_pct", 2.5)]));
}

#[test]
fn net_retransmit_elevated_does_not_fire_below_threshold() {
    assert!(!fires("net.retransmit_elevated", vec![signal("net.retrans_pct", 0.5)]));
}

// net.time_wait_high: fires when net.tw_count > 28000
#[test]
fn net_time_wait_high_fires_above_threshold() {
    assert!(fires("net.time_wait_high", vec![signal("net.tw_count", 30000.0)]));
}

#[test]
fn net_time_wait_high_does_not_fire_below_threshold() {
    assert!(!fires("net.time_wait_high", vec![signal("net.tw_count", 10000.0)]));
}

// net.irq_imbalance: fires when net.max_cpu_irq_pct > 80
#[test]
fn net_irq_imbalance_fires_above_threshold() {
    assert!(fires("net.irq_imbalance", vec![signal("net.max_cpu_irq_pct", 90.0)]));
}

#[test]
fn net_irq_imbalance_does_not_fire_below_threshold() {
    assert!(!fires("net.irq_imbalance", vec![signal("net.max_cpu_irq_pct", 50.0)]));
}
