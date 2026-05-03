/// Tests for SDD v2 requirements not covered by phase-specific test files:
/// R9 (network collector), R10 (cgroup collector), R11 (cpufreq collector),
/// R12 (interrupts collector), R33 (--profile-cpu), R34 (explain subcommand).

// R9: network collector -------------------------------------------------------

#[test]
fn ac_r9_network_collector_returns_ok() {
    use usereport::collector::{CollectCtx, Collector as _, network::NetworkCollector};
    let c = NetworkCollector::new();
    let ctx = CollectCtx::default();
    let result = c.collect(&ctx);
    assert!(result.is_ok(), "collect failed: {:?}", result);
}

#[test]
fn ac_r9_network_snapshot_emits_rx_drops_and_retrans() {
    use usereport::collector::network::NetworkCollector;

    const DEV1: &str = "Inter-|   Receive                                                |  Transmit\n face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n    lo:    100      1    0    0    0     0          0         0     100      1    0    0    0     0       0          0\n  eth0: 100000   1000    0   10    0     0          0         0   50000    500    0    0    0     0       0          0";
    const DEV2: &str = "Inter-|   Receive                                                |  Transmit\n face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n    lo:    200      2    0    0    0     0          0         0     200      2    0    0    0     0       0          0\n  eth0: 200000   2000    0   15    0     0          0         0  100000   1000    0    0    0     0       0          0";
    const SNMP1: &str = "Tcp: RtoAlgorithm RtoMin RtoMax MaxConn ActiveOpens PassiveOpens AttemptFails EstabResets CurrEstab InSegs OutSegs RetransSegs InErrs OutRsts InCsumErrors\nTcp: 1 200 120000 -1 100 50 5 10 20 1000 900 9 0 5 0";
    const SNMP2: &str = "Tcp: RtoAlgorithm RtoMin RtoMax MaxConn ActiveOpens PassiveOpens AttemptFails EstabResets CurrEstab InSegs OutSegs RetransSegs InErrs OutRsts InCsumErrors\nTcp: 1 200 120000 -1 110 55 5 10 20 1100 1000 15 0 5 0";

    let signals = NetworkCollector::from_snapshots(DEV1, DEV2, SNMP1, SNMP2, 1.0);
    let ids: Vec<_> = signals.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"net.rx_drops"), "missing rx_drops: {:?}", ids);
    assert!(ids.contains(&"net.retrans_pct"), "missing retrans_pct: {:?}", ids);
}

// R10: cgroup collector -------------------------------------------------------

#[test]
fn ac_r10_cgroup_collector_returns_ok_without_cgroup() {
    use usereport::collector::{CollectCtx, Collector as _, cgroup::CgroupCollector};
    let c = CgroupCollector::new();
    let ctx = CollectCtx::default(); // no cgroup_path set
    let result = c.collect(&ctx);
    assert!(result.is_ok(), "collect failed: {:?}", result);
}

#[test]
fn ac_r10_cgroup_v2_reads_memory_and_pids() {
    use usereport::collector::{CollectCtx, Collector as _, cgroup::CgroupCollector};

    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();

    std::fs::write(base.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
    std::fs::write(base.join("memory.current"), "10485760\n").unwrap();
    std::fs::write(base.join("memory.max"), "max\n").unwrap();
    std::fs::write(
        base.join("memory.events"),
        "low 0\nhigh 0\nmax 0\noom 0\noom_kill 2\noom_group_kill 0\n",
    )
    .unwrap();
    std::fs::write(base.join("pids.current"), "7\n").unwrap();
    std::fs::write(base.join("cpu.stat"), "usage_usec 0\nthrottled_usec 0\n").unwrap();

    let c = CgroupCollector::new();
    let ctx = CollectCtx {
        cgroup_path: Some(base.to_path_buf()),
        ..Default::default()
    };
    let signals = c.collect(&ctx).expect("collect");
    let ids: Vec<_> = signals.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"cgroup.memory_bytes"), "missing memory_bytes: {:?}", ids);
    assert!(
        ids.contains(&"cgroup.memory_limit_bytes"),
        "missing memory_limit_bytes: {:?}",
        ids
    );
    assert!(ids.contains(&"cgroup.oom_kills"), "missing oom_kills: {:?}", ids);
    assert!(ids.contains(&"cgroup.pids_current"), "missing pids_current: {:?}", ids);

    // "max" limit → 0.0
    let limit = signals.iter().find(|s| s.id == "cgroup.memory_limit_bytes").unwrap();
    assert_eq!(limit.value, usereport::signal::SignalValue::F64(0.0));

    // oom_kills = 2
    let oom = signals.iter().find(|s| s.id == "cgroup.oom_kills").unwrap();
    assert_eq!(oom.value, usereport::signal::SignalValue::F64(2.0));
}

// R11: cpufreq collector ------------------------------------------------------

#[test]
fn ac_r11_cpufreq_collector_returns_ok() {
    use usereport::collector::{CollectCtx, Collector as _, cpufreq::CpuFreqCollector};
    let c = CpuFreqCollector::new();
    let ctx = CollectCtx::default();
    let result = c.collect(&ctx);
    assert!(result.is_ok(), "collect failed: {:?}", result);
}

// R12: interrupts collector ---------------------------------------------------

#[test]
fn ac_r12_interrupts_collector_returns_ok() {
    use usereport::collector::{CollectCtx, Collector as _, interrupts::InterruptsCollector};
    let c = InterruptsCollector::new();
    let ctx = CollectCtx::default();
    let result = c.collect(&ctx);
    assert!(result.is_ok(), "collect failed: {:?}", result);
}

#[test]
fn ac_r12_max_cpu_irq_pct_computed() {
    use usereport::collector::interrupts::max_cpu_irq_pct;
    const SAMPLE: &str = "           CPU0       CPU1\n  0:       1000          0  IR-IO-APIC    2-edge      timer\n 26:     100000      10000  PCI-MSI 524288-edge      eth0-TxRx-0\n";
    let pct = max_cpu_irq_pct(SAMPLE).expect("should find nic irqs");
    // CPU0 handles 100000 out of 110000 total → ~90.9%
    assert!(pct > 80.0, "expected >80%, got {}", pct);
    assert!(pct <= 100.0);
}

// R33: --profile-cpu info finding when tools absent ---------------------------

#[cfg(feature = "bin")]
#[test]
fn ac_r33_profile_cpu_flag_parses() {
    use clap::Parser;
    let opt = usereport::cli::Opt::try_parse_from(["usereport", "--profile-cpu", "5s"]).expect("parse");
    // We can't easily inspect private fields, but parsing succeeding is enough.
    let _ = opt;
}

// R34: explain subcommand -----------------------------------------------------

#[cfg(feature = "bin")]
#[test]
fn ac_r34_explain_subcommand_parses() {
    use clap::Parser;
    let opt = usereport::cli::Opt::try_parse_from(["usereport", "explain", "net.retransmit_elevated"]).expect("parse");
    let _ = opt;
}

// Rule description/links fields -----------------------------------------------

#[test]
fn ac_r34_rule_description_and_links_round_trip_toml() {
    use usereport::rule::parse_rules_toml;
    let toml = r#"
[[rule]]
id = "test.rule"
when = "cpu.usr_pct > 90"
severity = "warn"
summary = "CPU is saturated"
evidence = ["cpu.usr_pct"]
suggest = ["top"]
description = "Fraction of time spent in user space. Sustained values near 100% indicate compute saturation."
links = ["https://example.com/cpu-perf"]
"#;
    let rules = parse_rules_toml(toml).expect("parse");
    assert_eq!(rules.len(), 1);
    let r = &rules[0];
    assert_eq!(r.id, "test.rule");
    assert!(r.description.is_some(), "description should be Some");
    assert!(r.description.as_deref().unwrap().contains("user space"));
    assert_eq!(r.links.len(), 1);
    assert!(r.links[0].contains("example.com"));
}

#[test]
fn ac_r34_rule_without_description_links_parses() {
    use usereport::rule::parse_rules_toml;
    let toml = r#"
[[rule]]
id = "test.simple"
when = "cpu.usr_pct > 50"
severity = "info"
summary = "CPU moderate"
evidence = []
suggest = []
"#;
    let rules = parse_rules_toml(toml).expect("parse");
    assert_eq!(rules.len(), 1);
    assert!(rules[0].description.is_none());
    assert!(rules[0].links.is_empty());
}
