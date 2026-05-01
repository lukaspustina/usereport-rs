//! Tests for SDD `specs/sdd/macos-compatibility.md` Phase 2
//! (Linux platform functions — regression and correctness).
//! These tests only run on Linux; they compile vacuously on macOS.
#![cfg(feature = "bin")]

use usereport::collector::platform;

/// T7 — Linux read_host_snapshot returns Some with nonzero cpu_count.
#[cfg(target_os = "linux")]
#[test]
fn p2_read_host_snapshot_returns_some_on_linux() {
    let snap = platform::read_host_snapshot();
    assert!(snap.is_some(), "read_host_snapshot() returned None on Linux");
    let s = snap.unwrap();
    assert!(s.cpu_count > 0, "cpu_count should be > 0");
    assert!(s.mem_total_bytes > 0, "mem_total_bytes should be > 0");
}

/// T7 — Linux read_cpu_snapshot returns Some.
#[cfg(target_os = "linux")]
#[test]
fn p2_read_cpu_snapshot_returns_some_on_linux() {
    let snap = platform::read_cpu_snapshot();
    assert!(snap.is_some(), "read_cpu_snapshot() returned None on Linux (needs /proc/stat)");
    let s = snap.unwrap();
    // Linux should always have iowait
    assert!(s.iowait.is_some(), "iowait should be Some on Linux");
}

/// T7 — Linux read_disk_snapshots returns non-empty (assuming at least one disk).
#[cfg(target_os = "linux")]
#[test]
fn p2_read_disk_snapshots_returns_items_on_linux() {
    let snaps = platform::read_disk_snapshots();
    // At minimum the loop device should be present.
    assert!(!snaps.is_empty(), "read_disk_snapshots() returned empty on Linux");
    let first = &snaps[0];
    // Linux disk snapshots should have read_time_ms Some.
    assert!(first.read_time_ms.is_some(), "read_time_ms should be Some on Linux");
    assert!(first.io_time_ms.is_some(), "io_time_ms should be Some on Linux");
}

/// Regression: existing CpuCollector::collect() still works on Linux.
#[cfg(target_os = "linux")]
#[test]
fn p2_cpu_collector_collect_returns_cpu_usr_pct_on_linux() {
    use usereport::collector::{CollectCtx, Collector};
    use usereport::collector::cpu::CpuCollector;
    let c = CpuCollector::new();
    let ctx = CollectCtx::default();
    let result = c.collect(&ctx).expect("collect should not error");
    let ids: Vec<&str> = result.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"cpu.usr_pct"), "missing cpu.usr_pct; got: {:?}", ids);
}

// Ensure this file compiles even on non-Linux platforms.
#[test]
fn p2_platform_module_accessible() {
    // Just verify the import works on all platforms.
    let _ = platform::read_cpufreq_snapshot();
    let _ = platform::read_disk_snapshots();
}
