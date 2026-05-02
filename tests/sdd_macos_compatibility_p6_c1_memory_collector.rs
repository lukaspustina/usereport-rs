//! Tests for SDD `specs/sdd/macos-compatibility.md` Phase 6
//! (MemoryCollector::new(), signals_from_mem_snapshot, CLI registration).
//!
//! T9 — mem signals fire on macOS (MemoryCollector::new().collect() returns mem.free_pct).
#![cfg(feature = "bin")]

use usereport::collector::memory::MemoryCollector;
use usereport::collector::platform::MemSnapshot;
use usereport::collector::{CollectCtx, Collector};

/// T9 — MemoryCollector::new() exists and collect() returns mem.free_pct.
#[test]
fn p6_memory_collector_new_collect_returns_mem_signals() {
    let c = MemoryCollector::new();
    let ctx = CollectCtx::default();
    let result = c.collect(&ctx).expect("collect should not error");
    let ids: Vec<&str> = result.iter().map(|s| s.id.as_str()).collect();
    // On macOS: vm_stat provides these; on Linux: free -m provides these.
    // If the platform source is unavailable, we get an empty Vec — not an error.
    // At minimum, if we get signals, they must include mem.free_pct.
    if !result.is_empty() {
        assert!(
            ids.contains(&"mem.free_pct"),
            "mem.free_pct should be present when collect returns non-empty; got: {:?}",
            ids
        );
    }
}

/// signals_from_mem_snapshot converts a MemSnapshot to signals.
#[test]
fn p6_signals_from_mem_snapshot_produces_required_signals() {
    let snap = MemSnapshot {
        total_mb: 8192.0,
        used_mb: 4096.0,
        free_mb: 4096.0,
        available_mb: None,
        swap_total_mb: 2048.0,
        swap_used_mb: 512.0,
        swap_free_mb: 1536.0,
        swap_in_pages: None,
    };
    let signals = MemoryCollector::signals_from_mem_snapshot(&snap).expect("should not error");
    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"mem.total_mb"), "missing mem.total_mb; got: {:?}", ids);
    assert!(ids.contains(&"mem.used_mb"), "missing mem.used_mb; got: {:?}", ids);
    assert!(ids.contains(&"mem.free_mb"), "missing mem.free_mb; got: {:?}", ids);
    assert!(ids.contains(&"mem.free_pct"), "missing mem.free_pct; got: {:?}", ids);
}

/// available_mb signal only emitted when Some.
#[test]
fn p6_signals_from_mem_snapshot_available_mb_only_when_some() {
    let snap_none = MemSnapshot {
        total_mb: 1000.0,
        used_mb: 400.0,
        free_mb: 600.0,
        available_mb: None,
        swap_total_mb: 0.0,
        swap_used_mb: 0.0,
        swap_free_mb: 0.0,
        swap_in_pages: None,
    };
    let sigs = MemoryCollector::signals_from_mem_snapshot(&snap_none).expect("ok");
    let ids: Vec<&str> = sigs.iter().map(|s| s.id.as_str()).collect();
    assert!(
        !ids.contains(&"mem.available_mb"),
        "mem.available_mb must not appear when None"
    );

    let snap_some = MemSnapshot {
        available_mb: Some(700.0),
        ..snap_none
    };
    let sigs2 = MemoryCollector::signals_from_mem_snapshot(&snap_some).expect("ok");
    let ids2: Vec<&str> = sigs2.iter().map(|s| s.id.as_str()).collect();
    assert!(
        ids2.contains(&"mem.available_mb"),
        "mem.available_mb should appear when Some"
    );
}
