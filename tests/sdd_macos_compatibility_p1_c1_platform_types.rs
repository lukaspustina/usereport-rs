//! Tests for SDD `specs/sdd/macos-compatibility.md` Phase 1
//! (platform module skeleton — snapshot types compile and have correct shape).
#![cfg(feature = "bin")]

use usereport::collector::platform::{
    CpuFreqSnapshot, CpuSnapshot, DiskDevSnapshot, HostSnapshot, MemSnapshot, NetSnapshot,
};

#[test]
fn platform_cpu_snapshot_total_excludes_iowait_when_none() {
    let snap = CpuSnapshot {
        user: 100,
        nice: 0,
        system: 50,
        idle: 800,
        iowait: None,
        irq: 10,
        softirq: 5,
        steal: 0,
        procs_running: None,
        ctxt: None,
    };
    // total = 100+0+50+800+0(iowait None)+10+5+0 = 965
    assert_eq!(snap.total(), 965);
}

#[test]
fn platform_cpu_snapshot_total_includes_iowait_when_some() {
    let snap = CpuSnapshot {
        user: 100,
        nice: 0,
        system: 50,
        idle: 800,
        iowait: Some(35),
        irq: 10,
        softirq: 5,
        steal: 0,
        procs_running: None,
        ctxt: None,
    };
    // total = 100+0+50+800+35+10+5+0 = 1000
    assert_eq!(snap.total(), 1000);
}

#[test]
fn platform_snapshot_types_clone_and_debug() {
    let host = HostSnapshot {
        cpu_count: 4,
        mem_total_bytes: 8 * 1024 * 1024 * 1024,
        load_avg_1m: 1.5,
    };
    let _cloned = host.clone();
    let _dbg = format!("{:?}", _cloned);

    let mem = MemSnapshot {
        total_mb: 8192.0,
        used_mb: 4096.0,
        free_mb: 4096.0,
        available_mb: None,
        swap_total_mb: 2048.0,
        swap_used_mb: 0.0,
        swap_free_mb: 2048.0,
    };
    let _cloned_mem = mem.clone();

    let freq = CpuFreqSnapshot {
        freq_ratio: None,
        temp_celsius: None,
    };
    let _cloned_freq = freq.clone();

    let disk = DiskDevSnapshot {
        name: "sda".to_string(),
        read_ios: 100,
        write_ios: 50,
        read_time_ms: Some(10),
        write_time_ms: Some(5),
        io_time_ms: Some(15),
    };
    let _cloned_disk = disk.clone();

    let net = NetSnapshot {
        rx_drops: std::collections::HashMap::new(),
        tcp_out_segs: 1000,
        tcp_retrans_segs: 10,
        tcp_tw_count: Some(50),
    };
    let _cloned_net = net.clone();
}
