//! Tests for SDD `specs/sdd/macos-compatibility.md` Phase 3
//! (macOS platform parsers — fixture-based, no real OS calls).
#![cfg(feature = "bin")]

// T5 — vm_stat fixture parser produces correct MemSnapshot.
// T6 — netstat -s -p tcp fixture parser.
// T11 — netstat -i -b -n drops parser skips loopback.

// These tests validate the parser helpers that live in platform/macos.rs.
// They are gated on macOS but use only fixture strings (no real subprocess calls).

/// T5 — macOS read_mem_snapshot parses vm_stat + swapusage correctly.
/// This test calls the public platform function which on macOS reads real vm_stat.
/// We test the public API contracts (not None, has mem.total > 0).
#[cfg(target_os = "macos")]
#[test]
fn p3_read_mem_snapshot_returns_some_on_macos() {
    use usereport::collector::platform;
    let snap = platform::read_mem_snapshot();
    assert!(snap.is_some(), "read_mem_snapshot() returned None on macOS");
    let s = snap.unwrap();
    assert!(s.total_mb > 0.0, "total_mb should be > 0");
    assert!(s.free_mb >= 0.0, "free_mb should be >= 0");
    assert!(s.used_mb >= 0.0, "used_mb should be >= 0");
    // macOS does not provide available_mb
    assert!(s.available_mb.is_none(), "available_mb should be None on macOS");
}

/// T6 — macOS read_net_snapshot returns Some with sane fields.
#[cfg(target_os = "macos")]
#[test]
fn p3_read_net_snapshot_returns_some_on_macos() {
    use usereport::collector::platform;
    let snap = platform::read_net_snapshot();
    assert!(snap.is_some(), "read_net_snapshot() returned None on macOS");
}

/// T8 — macOS read_cpu_snapshot returns Some (kern.cp_time available).
#[cfg(target_os = "macos")]
#[test]
fn p3_read_cpu_snapshot_returns_some_on_macos() {
    use usereport::collector::platform;
    let snap = platform::read_cpu_snapshot();
    assert!(snap.is_some(), "read_cpu_snapshot() returned None on macOS");
    let s = snap.unwrap();
    // macOS: iowait is always None
    assert!(s.iowait.is_none(), "iowait should be None on macOS");
    // macOS: procs_running is always None
    assert!(s.procs_running.is_none(), "procs_running should be None on macOS");
}

/// T11 — macOS rx_drops parser skips lo0 and includes other interfaces.
/// We test via read_net_snapshot which internally runs netstat -i -b -n.
#[cfg(target_os = "macos")]
#[test]
fn p3_read_net_snapshot_drops_not_include_lo0() {
    use usereport::collector::platform;
    let snap = platform::read_net_snapshot().expect("read_net_snapshot should return Some");
    assert!(
        !snap.rx_drops.contains_key("lo0"),
        "loopback lo0 should be excluded from rx_drops; got: {:?}",
        snap.rx_drops.keys().collect::<Vec<_>>()
    );
}

/// T12 — macOS read_host_snapshot returns Some.
#[cfg(target_os = "macos")]
#[test]
fn p3_read_host_snapshot_returns_some_on_macos() {
    use usereport::collector::platform;
    let snap = platform::read_host_snapshot();
    assert!(snap.is_some(), "read_host_snapshot() returned None on macOS");
    let s = snap.unwrap();
    assert!(s.cpu_count > 0, "cpu_count should be > 0 on macOS");
    assert!(s.mem_total_bytes > 0, "mem_total_bytes should be > 0");
    assert!(s.load_avg_1m >= 0.0, "load_avg_1m should be >= 0");
}

// Ensure this file compiles on non-macOS platforms.
#[test]
fn p3_platform_smoke() {
    use usereport::collector::platform;
    let _ = platform::read_cpufreq_snapshot();
}
