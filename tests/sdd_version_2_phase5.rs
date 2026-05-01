//! Integration tests for SDD `specs/sdd/version-2.md` Phase 5 (dmesg miner + pattern catalog).
#![cfg(feature = "bin")]

use usereport::collector::dmesg::DmesgCollector;
use usereport::finding::FindingKind;
use usereport::pattern::PatternEngine;
use usereport::signal::{Signal, SignalValue, Unit};
use usereport::collector::CollectCtx;

fn ctx() -> CollectCtx {
    CollectCtx {
        duration: None,
        interval: None,
        cgroup_path: None,
        baseline: None,
        cpu_count: 4,
    }
}

fn make_signal(id: &str, value: f64) -> Signal {
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

const DMESG_OOM: &str = "\
[12345.678] Out of memory: Killed process 1234 (stress) total-vm:102400kB, anon-rss:98304kB
[12346.000] oom_kill_process+0x0/0x1f0
";

const DMESG_BLOCKED: &str = "\
[12345.678] INFO: task kworker/0:1:42 blocked for more than 120 seconds.
[12346.000] INFO: task jbd2/sda1-8:31 blocked for more than 120 seconds.
";

const DMESG_FS_ERROR: &str = "\
[12345.678] EXT4-fs error (device sda1): ext4_journal_check_start:61: Detected aborted journal
[12346.000] XFS (sdb1): xfs_do_force_shutdown: IO error detected. Shutting down filesystem
";

const DMESG_SEGFAULT: &str = "\
[12345.678] myapp[1234]: segfault at 0 ip 00007f1234560000 sp 00007fff1234 error 4 in libc-2.31.so
";

const DMESG_MCE: &str = "\
[12345.678] mce: [Hardware Error]: Machine check events logged
[12346.000] EDAC MC0: 1 CE memory read error on CPU_SrcID#0_Ha#0_Chan#0_DIMM#0
";

const DMESG_NIC_FLAP: &str = "\
[12345.678] eth0: Link is Down
[12346.000] eth0: Link is Up - 1Gbps/Full - flow control rx/tx
";

const DMESG_IO_ERROR: &str = "\
[12345.678] blk_update_request: I/O error, dev sda, sector 12345678
[12346.000] blk_update_request: I/O error, dev sda, sector 12345679
";

// ---------------------------------------------------------------------------
// Criterion 1 — dmesg OOM parsing
// ---------------------------------------------------------------------------

#[test]
fn ac_phase5_1_dmesg_oom_count_from_oom_lines() {
    let signals = DmesgCollector::parse(DMESG_OOM);
    let sig = signals.iter().find(|s| s.id == "dmesg.oom_count").expect("dmesg.oom_count");
    match sig.value {
        SignalValue::F64(v) => assert!(v > 0.0, "oom_count should be > 0, got {}", v),
        SignalValue::I64(v) => assert!(v > 0, "oom_count should be > 0, got {}", v),
        _ => panic!("unexpected value type"),
    }
}

#[test]
fn ac_phase5_1_dmesg_oom_count_zero_when_no_oom() {
    let signals = DmesgCollector::parse(DMESG_BLOCKED);
    let sig = signals.iter().find(|s| s.id == "dmesg.oom_count").expect("dmesg.oom_count");
    match sig.value {
        SignalValue::F64(v) => assert_eq!(v as i64, 0, "oom_count should be 0"),
        SignalValue::I64(v) => assert_eq!(v, 0, "oom_count should be 0"),
        _ => panic!("unexpected value type"),
    }
}

// ---------------------------------------------------------------------------
// Criterion 2 — dmesg blocked-task parsing
// ---------------------------------------------------------------------------

#[test]
fn ac_phase5_2_dmesg_blocked_task_count_from_blocked_lines() {
    let signals = DmesgCollector::parse(DMESG_BLOCKED);
    let sig = signals
        .iter()
        .find(|s| s.id == "dmesg.blocked_task_count")
        .expect("dmesg.blocked_task_count");
    match sig.value {
        SignalValue::F64(v) => assert!(v > 0.0, "blocked_task_count should be > 0, got {}", v),
        SignalValue::I64(v) => assert!(v > 0, "blocked_task_count should be > 0, got {}", v),
        _ => panic!("unexpected value type"),
    }
}

// ---------------------------------------------------------------------------
// Criterion 3 — dmesg filesystem error parsing
// ---------------------------------------------------------------------------

#[test]
fn ac_phase5_3_dmesg_fs_error_count_from_ext4_xfs_lines() {
    let signals = DmesgCollector::parse(DMESG_FS_ERROR);
    let sig = signals
        .iter()
        .find(|s| s.id == "dmesg.fs_error_count")
        .expect("dmesg.fs_error_count");
    match sig.value {
        SignalValue::F64(v) => assert!(v > 0.0, "fs_error_count should be > 0, got {}", v),
        SignalValue::I64(v) => assert!(v > 0, "fs_error_count should be > 0, got {}", v),
        _ => panic!("unexpected value type"),
    }
}

// ---------------------------------------------------------------------------
// Criterion 4 — all 7 event types detected
// ---------------------------------------------------------------------------

#[test]
fn ac_phase5_4_all_7_signal_ids_emitted() {
    let all = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        DMESG_OOM, DMESG_BLOCKED, DMESG_FS_ERROR, DMESG_SEGFAULT, DMESG_MCE, DMESG_NIC_FLAP, DMESG_IO_ERROR
    );
    let signals = DmesgCollector::parse(&all);
    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();

    let expected = [
        "dmesg.oom_count",
        "dmesg.blocked_task_count",
        "dmesg.fs_error_count",
        "dmesg.segfault_count",
        "dmesg.mce_count",
        "dmesg.nic_flap_count",
        "dmesg.io_error_count",
    ];
    for e in expected {
        assert!(ids.contains(&e), "missing signal '{}'; got {:?}", e, ids);
    }
}

// ---------------------------------------------------------------------------
// Criterion 5 — TIME_WAIT pattern fires with both signals
// ---------------------------------------------------------------------------

const TIME_WAIT_TOML: &str = r#"
[[pattern]]
id = "time_wait_exhaustion"
when = "net.tw_count > 28000 AND net.connect_failures > 0"
severity = "crit"
summary = "TIME_WAIT exhaustion likely"
suggest = ["sysctl net.ipv4.tcp_tw_reuse", "sysctl net.ipv4.ip_local_port_range"]
"#;

#[test]
fn ac_phase5_5_pattern_fires_with_both_signals() {
    let engine = PatternEngine::from_toml(TIME_WAIT_TOML).expect("parse");
    let signals = vec![
        make_signal("net.tw_count", 30000.0),
        make_signal("net.connect_failures", 5.0),
    ];
    let findings = engine.run(&signals, &ctx());
    assert_eq!(findings.len(), 1, "expected exactly one finding; got {:?}", findings);
    assert_eq!(findings[0].id, "time_wait_exhaustion");
}

// ---------------------------------------------------------------------------
// Criterion 6 — AC-8: partial signal set produces no pattern finding
// ---------------------------------------------------------------------------

#[test]
fn ac_phase5_6_pattern_does_not_fire_with_one_signal_only() {
    let engine = PatternEngine::from_toml(TIME_WAIT_TOML).expect("parse");
    let signals = vec![make_signal("net.tw_count", 30000.0)]; // net.connect_failures absent
    let findings = engine.run(&signals, &ctx());
    assert!(
        findings.is_empty(),
        "pattern must not fire when a required signal is absent; got {:?}",
        findings
    );
}

// ---------------------------------------------------------------------------
// Criterion 7 — pattern findings have kind = FindingKind::Pattern
// ---------------------------------------------------------------------------

#[test]
fn ac_phase5_7_pattern_finding_kind_is_pattern() {
    let engine = PatternEngine::from_toml(TIME_WAIT_TOML).expect("parse");
    let signals = vec![
        make_signal("net.tw_count", 30000.0),
        make_signal("net.connect_failures", 5.0),
    ];
    let findings = engine.run(&signals, &ctx());
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].kind,
        FindingKind::Pattern,
        "finding kind should be Pattern, got {:?}",
        findings[0].kind
    );
}

// ---------------------------------------------------------------------------
// Criterion 8 — pattern catalog TOML files all load without error
// ---------------------------------------------------------------------------

const LOCK_CONTENTION_TOML: &str = include_str!("../contrib/patterns/lock_contention.toml");
const NFS_STALL_TOML: &str = include_str!("../contrib/patterns/nfs_stall.toml");
const TIME_WAIT_FILE_TOML: &str = include_str!("../contrib/patterns/time_wait.toml");
const SLAB_LEAK_TOML: &str = include_str!("../contrib/patterns/slab_leak.toml");
const THUNDERING_HERD_TOML: &str = include_str!("../contrib/patterns/thundering_herd.toml");
const SOCKET_LEAK_TOML: &str = include_str!("../contrib/patterns/socket_leak.toml");

#[test]
fn ac_phase5_8_pattern_catalog_all_files_load() {
    for (name, content) in [
        ("lock_contention", LOCK_CONTENTION_TOML),
        ("nfs_stall", NFS_STALL_TOML),
        ("time_wait", TIME_WAIT_FILE_TOML),
        ("slab_leak", SLAB_LEAK_TOML),
        ("thundering_herd", THUNDERING_HERD_TOML),
        ("socket_leak", SOCKET_LEAK_TOML),
    ] {
        PatternEngine::from_toml(content)
            .unwrap_or_else(|e| panic!("pattern file '{}' failed to load: {}", name, e));
    }
}
