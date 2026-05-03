//! Firing and boundary tests for built-in rules.
//!
//! Uses `builtin_rules()` so tests exercise the real TOML definitions, not
//! hand-constructed predicates. All rules use strict `>` or `<`, so the exact
//! threshold value must NOT fire.
#![cfg(feature = "bin")]

use std::collections::HashMap;

use usereport::collector::CollectCtx;
use usereport::rule::{RuleEngine, builtin::builtin_rules};
use usereport::signal::{Signal, SignalValue, Unit};

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

fn run_rules(signals: &[Signal]) -> Vec<usereport::Finding> {
    let engine = RuleEngine::new(builtin_rules());
    let ctx = CollectCtx::default();
    let (findings, _) = engine.run(signals, &ctx, &HashMap::new());
    findings
}

fn run_rules_with_ctx(signals: &[Signal], ctx: CollectCtx) -> Vec<usereport::Finding> {
    let engine = RuleEngine::new(builtin_rules());
    let (findings, _) = engine.run(signals, &ctx, &HashMap::new());
    findings
}

fn fires(rule_id: &str, signals: Vec<Signal>) -> bool {
    run_rules(&signals).into_iter().any(|f| f.id == rule_id)
}

// =============================================================================
// cpu.iowait_elevated (threshold > 20)
// =============================================================================

#[test]
fn cpu_iowait_elevated_fires_above_threshold() {
    assert!(fires("cpu.iowait_elevated", vec![signal("cpu.iowait_pct", 25.0)]));
}

#[test]
fn cpu_iowait_elevated_does_not_fire_below_threshold() {
    assert!(!fires("cpu.iowait_elevated", vec![signal("cpu.iowait_pct", 15.0)]));
}

#[test]
fn cpu_iowait_at_threshold_does_not_fire() {
    assert!(!fires("cpu.iowait_elevated", vec![signal("cpu.iowait_pct", 20.0)]));
}

#[test]
fn cpu_iowait_just_above_threshold_fires() {
    assert!(fires("cpu.iowait_elevated", vec![signal("cpu.iowait_pct", 20.001)]));
}

// =============================================================================
// cpu.saturation (threshold > 80)
// =============================================================================

#[test]
fn cpu_saturation_fires_above_threshold() {
    assert!(fires("cpu.saturation", vec![signal("cpu.usr_pct", 85.0)]));
}

#[test]
fn cpu_saturation_does_not_fire_below_threshold() {
    assert!(!fires("cpu.saturation", vec![signal("cpu.usr_pct", 70.0)]));
}

#[test]
fn cpu_saturation_at_threshold_does_not_fire() {
    assert!(!fires("cpu.saturation", vec![signal("cpu.usr_pct", 80.0)]));
}

#[test]
fn cpu_saturation_just_above_threshold_fires() {
    assert!(fires("cpu.saturation", vec![signal("cpu.usr_pct", 80.001)]));
}

// =============================================================================
// cpu.frequency_throttling (threshold < 0.8)
// =============================================================================

#[test]
fn cpu_frequency_throttling_fires_below_threshold() {
    assert!(fires("cpu.frequency_throttling", vec![signal("cpu.freq_ratio", 0.5)]));
}

#[test]
fn cpu_frequency_throttling_does_not_fire_above_threshold() {
    assert!(!fires("cpu.frequency_throttling", vec![signal("cpu.freq_ratio", 0.9)]));
}

#[test]
fn cpu_frequency_throttling_at_threshold_does_not_fire() {
    assert!(!fires("cpu.frequency_throttling", vec![signal("cpu.freq_ratio", 0.8)]));
}

#[test]
fn cpu_frequency_throttling_just_below_threshold_fires() {
    assert!(fires("cpu.frequency_throttling", vec![signal("cpu.freq_ratio", 0.799)]));
}

// =============================================================================
// mem.pressure (threshold < 10)
// =============================================================================

#[test]
fn mem_pressure_fires_below_threshold() {
    assert!(fires("mem.pressure", vec![signal("mem.free_pct", 5.0)]));
}

#[test]
fn mem_pressure_does_not_fire_above_threshold() {
    assert!(!fires("mem.pressure", vec![signal("mem.free_pct", 20.0)]));
}

#[test]
fn mem_pressure_at_threshold_does_not_fire() {
    assert!(!fires("mem.pressure", vec![signal("mem.free_pct", 10.0)]));
}

#[test]
fn mem_pressure_just_below_threshold_fires() {
    assert!(fires("mem.pressure", vec![signal("mem.free_pct", 9.999)]));
}

// =============================================================================
// mem.swap_in_active (threshold > 0)
// =============================================================================

#[test]
fn mem_swap_in_active_fires_above_zero() {
    assert!(fires("mem.swap_in_active", vec![signal("vmstat.swap_in", 1.0)]));
}

#[test]
fn mem_swap_in_active_does_not_fire_at_zero() {
    assert!(!fires("mem.swap_in_active", vec![signal("vmstat.swap_in", 0.0)]));
}

#[test]
fn mem_swap_in_at_zero_does_not_fire() {
    assert!(!fires("mem.swap_in_active", vec![signal("vmstat.swap_in", 0.0)]));
}

#[test]
fn mem_swap_in_above_zero_fires() {
    assert!(fires("mem.swap_in_active", vec![signal("vmstat.swap_in", 1.0)]));
}

// =============================================================================
// disk.utilization_saturated (threshold > 90)
// =============================================================================

#[test]
fn disk_utilization_saturated_fires_above_threshold() {
    assert!(fires(
        "disk.utilization_saturated",
        vec![signal("disk.max_util_pct", 95.0)]
    ));
}

#[test]
fn disk_utilization_saturated_does_not_fire_below_threshold() {
    assert!(!fires(
        "disk.utilization_saturated",
        vec![signal("disk.max_util_pct", 80.0)]
    ));
}

#[test]
fn disk_utilization_at_threshold_does_not_fire() {
    assert!(!fires(
        "disk.utilization_saturated",
        vec![signal("disk.max_util_pct", 90.0)]
    ));
}

#[test]
fn disk_utilization_just_above_threshold_fires() {
    assert!(fires(
        "disk.utilization_saturated",
        vec![signal("disk.max_util_pct", 90.001)]
    ));
}

// =============================================================================
// disk.await_elevated (threshold > 100)
// =============================================================================

#[test]
fn disk_await_elevated_fires_above_threshold() {
    assert!(fires("disk.await_elevated", vec![signal("disk.max_await_ms", 150.0)]));
}

#[test]
fn disk_await_elevated_does_not_fire_below_threshold() {
    assert!(!fires("disk.await_elevated", vec![signal("disk.max_await_ms", 50.0)]));
}

#[test]
fn disk_await_at_threshold_does_not_fire() {
    assert!(!fires("disk.await_elevated", vec![signal("disk.max_await_ms", 100.0)]));
}

#[test]
fn disk_await_just_above_threshold_fires() {
    assert!(fires("disk.await_elevated", vec![signal("disk.max_await_ms", 100.001)]));
}

// =============================================================================
// net.retransmit_elevated (threshold > 1)
// =============================================================================

#[test]
fn net_retransmit_elevated_fires_above_threshold() {
    assert!(fires("net.retransmit_elevated", vec![signal("net.retrans_pct", 2.5)]));
}

#[test]
fn net_retransmit_elevated_does_not_fire_below_threshold() {
    assert!(!fires("net.retransmit_elevated", vec![signal("net.retrans_pct", 0.5)]));
}

#[test]
fn net_retransmit_at_threshold_does_not_fire() {
    assert!(!fires("net.retransmit_elevated", vec![signal("net.retrans_pct", 1.0)]));
}

#[test]
fn net_retransmit_just_above_threshold_fires() {
    assert!(fires("net.retransmit_elevated", vec![signal("net.retrans_pct", 1.001)]));
}

// =============================================================================
// net.rx_drops (threshold > 0)
// =============================================================================

#[test]
fn net_rx_drops_at_zero_does_not_fire() {
    assert!(!fires("net.rx_drops", vec![signal("net.rx_drops", 0.0)]));
}

#[test]
fn net_rx_drops_above_zero_fires() {
    assert!(fires("net.rx_drops", vec![signal("net.rx_drops", 1.0)]));
}

// =============================================================================
// net.time_wait_high (threshold > 28000)
// =============================================================================

#[test]
fn net_time_wait_high_fires_above_threshold() {
    assert!(fires("net.time_wait_high", vec![signal("net.tw_count", 30000.0)]));
}

#[test]
fn net_time_wait_high_does_not_fire_below_threshold() {
    assert!(!fires("net.time_wait_high", vec![signal("net.tw_count", 10000.0)]));
}

#[test]
fn net_time_wait_at_threshold_does_not_fire() {
    assert!(!fires("net.time_wait_high", vec![signal("net.tw_count", 28000.0)]));
}

#[test]
fn net_time_wait_just_above_threshold_fires() {
    assert!(fires("net.time_wait_high", vec![signal("net.tw_count", 28001.0)]));
}

// =============================================================================
// net.irq_imbalance (threshold > 80)
// =============================================================================

#[test]
fn net_irq_imbalance_fires_above_threshold() {
    assert!(fires("net.irq_imbalance", vec![signal("net.max_cpu_irq_pct", 90.0)]));
}

#[test]
fn net_irq_imbalance_does_not_fire_below_threshold() {
    assert!(!fires("net.irq_imbalance", vec![signal("net.max_cpu_irq_pct", 50.0)]));
}

#[test]
fn net_irq_imbalance_at_threshold_does_not_fire() {
    assert!(!fires("net.irq_imbalance", vec![signal("net.max_cpu_irq_pct", 80.0)]));
}

#[test]
fn net_irq_imbalance_just_above_threshold_fires() {
    assert!(fires("net.irq_imbalance", vec![signal("net.max_cpu_irq_pct", 80.001)]));
}

// =============================================================================
// dmesg.oom_kill (threshold > 0)
// =============================================================================

#[test]
fn dmesg_oom_kill_at_zero_does_not_fire() {
    assert!(!fires("dmesg.oom_kill", vec![signal("dmesg.oom_count", 0.0)]));
}

#[test]
fn dmesg_oom_kill_above_zero_fires() {
    assert!(fires("dmesg.oom_kill", vec![signal("dmesg.oom_count", 1.0)]));
}

// =============================================================================
// dmesg.blocked_tasks (threshold > 0)
// =============================================================================

#[test]
fn dmesg_blocked_tasks_at_zero_does_not_fire() {
    assert!(!fires(
        "dmesg.blocked_tasks",
        vec![signal("dmesg.blocked_task_count", 0.0)]
    ));
}

#[test]
fn dmesg_blocked_tasks_above_zero_fires() {
    assert!(fires(
        "dmesg.blocked_tasks",
        vec![signal("dmesg.blocked_task_count", 1.0)]
    ));
}

// =============================================================================
// dmesg.fs_errors (threshold > 0)
// =============================================================================

#[test]
fn dmesg_fs_errors_at_zero_does_not_fire() {
    assert!(!fires("dmesg.fs_errors", vec![signal("dmesg.fs_error_count", 0.0)]));
}

#[test]
fn dmesg_fs_errors_above_zero_fires() {
    assert!(fires("dmesg.fs_errors", vec![signal("dmesg.fs_error_count", 1.0)]));
}

// =============================================================================
// cpu.runqueue_saturation (threshold: cpu.run_queue > host.cpu_count)
// =============================================================================

fn ctx_with_cpu_count(cpu_count: usize) -> CollectCtx {
    CollectCtx {
        cpu_count,
        ..CollectCtx::default()
    }
}

fn fires_with_ctx(rule_id: &str, signals: Vec<Signal>, ctx: CollectCtx) -> bool {
    run_rules_with_ctx(&signals, ctx).into_iter().any(|f| f.id == rule_id)
}

#[test]
fn cpu_runqueue_saturation_fires_above_cpu_count() {
    // run_queue=5 > cpu_count=4 → fires
    assert!(fires_with_ctx(
        "cpu.runqueue_saturation",
        vec![signal("cpu.run_queue", 5.0)],
        ctx_with_cpu_count(4)
    ));
}

#[test]
fn cpu_runqueue_saturation_does_not_fire_at_cpu_count() {
    // run_queue=4 == cpu_count=4 → does not fire (strict >)
    assert!(!fires_with_ctx(
        "cpu.runqueue_saturation",
        vec![signal("cpu.run_queue", 4.0)],
        ctx_with_cpu_count(4)
    ));
}

#[test]
fn cpu_runqueue_saturation_does_not_fire_below_cpu_count() {
    // run_queue=3 < cpu_count=4 → does not fire
    assert!(!fires_with_ctx(
        "cpu.runqueue_saturation",
        vec![signal("cpu.run_queue", 3.0)],
        ctx_with_cpu_count(4)
    ));
}

#[test]
fn cpu_runqueue_saturation_fires_just_above_cpu_count() {
    // run_queue=4.001 > cpu_count=4 → fires
    assert!(fires_with_ctx(
        "cpu.runqueue_saturation",
        vec![signal("cpu.run_queue", 4.001)],
        ctx_with_cpu_count(4)
    ));
}
