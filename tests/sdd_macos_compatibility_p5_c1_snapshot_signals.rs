//! Tests for SDD `specs/sdd/macos-compatibility.md` Phase 5
//! (cpu/network/disk snapshot-based signal emission).
//!
//! T2 — iowait absent on macOS (from_cpu_snapshots with iowait:None).
//! T3 — iowait present on Linux (from_cpu_snapshots with iowait:Some).
//! T4 — disk util/await absent when time fields are None.
#![cfg(feature = "bin")]

use usereport::collector::cpu::CpuCollector;
use usereport::collector::disk::DiskCollector;
use usereport::collector::network::NetworkCollector;
use usereport::collector::platform::{CpuSnapshot, DiskDevSnapshot, NetSnapshot};

fn make_cpu_snap(iowait: Option<u64>, procs_running: Option<u64>, ctxt: Option<u64>) -> CpuSnapshot {
    CpuSnapshot {
        user: 1000,
        nice: 0,
        system: 200,
        idle: 8000,
        iowait,
        irq: 10,
        softirq: 5,
        steal: 0,
        procs_running,
        ctxt,
    }
}

// ---------------------------------------------------------------------------
// T2 — iowait absent when None
// ---------------------------------------------------------------------------

#[test]
fn t2_from_cpu_snapshots_no_iowait_pct_when_iowait_none() {
    let a = make_cpu_snap(None, None, None);
    let b = CpuSnapshot { user: 1100, ..make_cpu_snap(None, None, None) };
    let signals = CpuCollector::from_cpu_snapshots(&a, &b, 1.0);
    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();
    assert!(
        !ids.contains(&"cpu.iowait_pct"),
        "cpu.iowait_pct must not appear when iowait is None; got: {:?}",
        ids
    );
    // usr_pct should still be present
    assert!(ids.contains(&"cpu.usr_pct"), "cpu.usr_pct should be present; got: {:?}", ids);
}

// ---------------------------------------------------------------------------
// T3 — iowait present when Some
// ---------------------------------------------------------------------------

#[test]
fn t3_from_cpu_snapshots_has_iowait_pct_when_iowait_some() {
    let a = make_cpu_snap(Some(500), None, None);
    let b = CpuSnapshot { user: 1100, ..make_cpu_snap(Some(600), None, None) };
    let signals = CpuCollector::from_cpu_snapshots(&a, &b, 1.0);
    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();
    assert!(
        ids.contains(&"cpu.iowait_pct"),
        "cpu.iowait_pct must be present when iowait is Some; got: {:?}",
        ids
    );
}

// ---------------------------------------------------------------------------
// T3 variant — procs_running emitted only when Some
// ---------------------------------------------------------------------------

#[test]
fn t3_from_cpu_snapshots_vmstat_r_only_when_procs_running_some() {
    // With None — no vmstat.r
    let a_none = make_cpu_snap(None, None, None);
    let b_none = CpuSnapshot { user: 1100, ..make_cpu_snap(None, None, None) };
    let sigs = CpuCollector::from_cpu_snapshots(&a_none, &b_none, 1.0);
    let ids: Vec<&str> = sigs.iter().map(|s| s.id.as_str()).collect();
    assert!(!ids.contains(&"vmstat.r"), "vmstat.r must not appear when procs_running=None");

    // With Some — vmstat.r present
    let a_some = make_cpu_snap(None, Some(3), None);
    let b_some = CpuSnapshot { user: 1100, ..make_cpu_snap(None, Some(4), None) };
    let sigs2 = CpuCollector::from_cpu_snapshots(&a_some, &b_some, 1.0);
    let ids2: Vec<&str> = sigs2.iter().map(|s| s.id.as_str()).collect();
    assert!(ids2.contains(&"vmstat.r"), "vmstat.r must appear when procs_running=Some");
}

// ---------------------------------------------------------------------------
// T4 — disk util_pct and await_ms absent when io_time_ms is None
// ---------------------------------------------------------------------------

fn make_disk_snap(name: &str, read_ios: u64, write_ios: u64, io_time: Option<u64>) -> DiskDevSnapshot {
    DiskDevSnapshot {
        name: name.to_string(),
        read_ios,
        write_ios,
        read_time_ms: io_time,
        write_time_ms: io_time,
        io_time_ms: io_time,
    }
}

#[test]
fn t4_from_disk_snapshots_no_util_or_await_when_time_none() {
    let a = vec![make_disk_snap("sda", 100, 50, None)];
    let b = vec![make_disk_snap("sda", 200, 100, None)];
    let signals = DiskCollector::from_disk_snapshots(&a, &b, 1.0);
    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();

    assert!(
        !ids.iter().any(|id| id.ends_with(".util_pct")),
        "util_pct must not appear when io_time_ms=None; got: {:?}",
        ids
    );
    assert!(
        !ids.iter().any(|id| id.ends_with(".await_ms")),
        "await_ms must not appear when io_time_ms=None; got: {:?}",
        ids
    );
    // read_iops and write_iops should still be present
    assert!(ids.contains(&"disk.sda.read_iops"), "read_iops should be present; got: {:?}", ids);
    assert!(ids.contains(&"disk.sda.write_iops"), "write_iops should be present; got: {:?}", ids);
}

#[test]
fn t4_from_disk_snapshots_has_util_and_await_when_time_some() {
    let a = vec![make_disk_snap("sda", 100, 50, Some(500))];
    let b = vec![make_disk_snap("sda", 200, 100, Some(600))];
    let signals = DiskCollector::from_disk_snapshots(&a, &b, 1.0);
    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();

    assert!(
        ids.iter().any(|id| id.ends_with(".util_pct")),
        "util_pct should appear when io_time_ms=Some; got: {:?}",
        ids
    );
    assert!(
        ids.iter().any(|id| id.ends_with(".await_ms")),
        "await_ms should appear when io_time_ms=Some; got: {:?}",
        ids
    );
}

// ---------------------------------------------------------------------------
// Network from_net_snapshots — basic smoke
// ---------------------------------------------------------------------------

#[test]
fn p5_from_net_snapshots_emits_retrans_and_drops() {
    use std::collections::HashMap;
    let mut drops1 = HashMap::new();
    drops1.insert("en0".to_string(), 5u64);
    let mut drops2 = HashMap::new();
    drops2.insert("en0".to_string(), 8u64);

    let a = NetSnapshot {
        rx_drops: drops1,
        tcp_out_segs: 1000,
        tcp_retrans_segs: 10,
        tcp_tw_count: None,
    };
    let b = NetSnapshot {
        rx_drops: drops2,
        tcp_out_segs: 1100,
        tcp_retrans_segs: 16,
        tcp_tw_count: Some(50),
    };

    let signals = NetworkCollector::from_net_snapshots(&a, &b, 1.0);
    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"net.rx_drops"), "net.rx_drops missing; got: {:?}", ids);
    assert!(ids.contains(&"net.retrans_pct"), "net.retrans_pct missing; got: {:?}", ids);
    assert!(ids.contains(&"net.tw_count"), "net.tw_count missing; got: {:?}", ids);
}
