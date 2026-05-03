//! Firing tests for all built-in patterns.
//!
//! Each pattern has two tests:
//!   - fires when both predicates are satisfied
//!   - does NOT fire when only one predicate is satisfied
#![cfg(feature = "bin")]

use usereport::collector::CollectCtx;
use usereport::pattern::PatternEngine;
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

fn run_patterns(signals: &[Signal], ctx: &CollectCtx) -> Vec<usereport::Finding> {
    let engine = PatternEngine::from_toml(concat!(
        include_str!("../contrib/patterns/lock_contention.toml"),
        include_str!("../contrib/patterns/nfs_stall.toml"),
        include_str!("../contrib/patterns/slab_leak.toml"),
        include_str!("../contrib/patterns/socket_leak.toml"),
        include_str!("../contrib/patterns/thundering_herd.toml"),
    ))
    .expect("parse patterns");
    engine.run(signals, ctx)
}

fn fires(pattern_id: &str, signals: Vec<Signal>, ctx: &CollectCtx) -> bool {
    run_patterns(&signals, ctx).into_iter().any(|f| f.id == pattern_id)
}

fn default_ctx() -> CollectCtx {
    CollectCtx::default()
}

// =============================================================================
// lock_contention: dmesg.blocked_task_count > 0 AND cpu.iowait_pct > 10
// =============================================================================

#[test]
fn lock_contention_fires_when_both_predicates_true() {
    let ctx = default_ctx();
    assert!(fires(
        "lock_contention",
        vec![signal("dmesg.blocked_task_count", 1.0), signal("cpu.iowait_pct", 15.0)],
        &ctx
    ));
}

#[test]
fn lock_contention_does_not_fire_when_only_blocked_tasks() {
    let ctx = default_ctx();
    assert!(!fires(
        "lock_contention",
        vec![signal("dmesg.blocked_task_count", 1.0), signal("cpu.iowait_pct", 5.0)],
        &ctx
    ));
}

// =============================================================================
// nfs_stall: dmesg.blocked_task_count > 0 AND cpu.iowait_pct > 20
// =============================================================================

#[test]
fn nfs_stall_fires_when_both_predicates_true() {
    let ctx = default_ctx();
    assert!(fires(
        "nfs_stall",
        vec![signal("dmesg.blocked_task_count", 2.0), signal("cpu.iowait_pct", 25.0)],
        &ctx
    ));
}

#[test]
fn nfs_stall_does_not_fire_when_only_high_iowait() {
    let ctx = default_ctx();
    assert!(!fires(
        "nfs_stall",
        vec![signal("dmesg.blocked_task_count", 0.0), signal("cpu.iowait_pct", 25.0)],
        &ctx
    ));
}

// =============================================================================
// slab_leak: mem.free_pct < 10 AND dmesg.oom_count == 0
// =============================================================================

#[test]
fn slab_leak_fires_when_both_predicates_true() {
    let ctx = default_ctx();
    assert!(fires(
        "slab_leak",
        vec![signal("mem.free_pct", 5.0), signal("dmesg.oom_count", 0.0)],
        &ctx
    ));
}

#[test]
fn slab_leak_does_not_fire_when_oom_count_nonzero() {
    let ctx = default_ctx();
    assert!(!fires(
        "slab_leak",
        vec![signal("mem.free_pct", 5.0), signal("dmesg.oom_count", 1.0)],
        &ctx
    ));
}

// =============================================================================
// socket_leak: net.tw_count > 10000 AND net.rx_drops > 0
// =============================================================================

#[test]
fn socket_leak_fires_when_both_predicates_true() {
    let ctx = default_ctx();
    assert!(fires(
        "socket_leak",
        vec![signal("net.tw_count", 15000.0), signal("net.rx_drops", 1.0)],
        &ctx
    ));
}

#[test]
fn socket_leak_does_not_fire_when_only_high_tw_count() {
    let ctx = default_ctx();
    assert!(!fires(
        "socket_leak",
        vec![signal("net.tw_count", 15000.0), signal("net.rx_drops", 0.0)],
        &ctx
    ));
}

// =============================================================================
// thundering_herd: cpu.run_queue > host.cpu_count AND cpu.sys_pct > 30
// =============================================================================

#[test]
fn thundering_herd_fires_when_both_predicates_true() {
    let ctx = CollectCtx {
        cpu_count: 4,
        ..CollectCtx::default()
    };
    assert!(fires(
        "thundering_herd",
        vec![signal("cpu.run_queue", 5.0), signal("cpu.sys_pct", 35.0)],
        &ctx
    ));
}

#[test]
fn thundering_herd_does_not_fire_when_only_high_sys_pct() {
    let ctx = CollectCtx {
        cpu_count: 4,
        ..CollectCtx::default()
    };
    assert!(!fires(
        "thundering_herd",
        vec![signal("cpu.run_queue", 2.0), signal("cpu.sys_pct", 35.0)],
        &ctx
    ));
}
