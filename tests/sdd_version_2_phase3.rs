//! Integration tests for SDD `specs/sdd/version-2.md` Phase 3 (direct
//! collectors + delta engine + pipeline wiring).
#![cfg(feature = "bin")]

use usereport::analysis::{Analysis, AnalysisReport, Context};
use usereport::cli::{ExitOn, compute_exit_code};
use usereport::collector::cpu::CpuCollector;
use usereport::collector::disk::DiskCollector;
use usereport::collector::{CollectCtx, Collector};
use usereport::finding::Severity;
use usereport::rule::{Predicate, Rule, RuleEngine};
use usereport::signal::{Signal, SignalValue, Unit};
use usereport::{Command, ThreadRunner};

const PROC_STAT_T0: &str = "\
cpu  1000 0 500 10000 50 0 0 0 0 0
cpu0 500 0 250 5000 25 0 0 0 0 0
cpu1 500 0 250 5000 25 0 0 0 0 0
intr 100000
ctxt 200000
btime 1620000000
processes 5000
procs_running 1
procs_blocked 0
";

const PROC_STAT_T1: &str = "\
cpu  1050 0 525 10500 55 0 0 0 0 0
cpu0 525 0 262 5250 27 0 0 0 0 0
cpu1 525 0 263 5250 28 0 0 0 0 0
intr 100100
ctxt 200200
btime 1620000000
processes 5005
procs_running 8
procs_blocked 0
";

const PROC_DISKSTATS_T0: &str = "\
   8       0 sda 100 0 1000 50 200 0 5000 100 0 50 150 0 0 0 0
   8       1 sda1 50 0 500 25 100 0 2500 50 0 25 75 0 0 0 0
";

const PROC_DISKSTATS_T1: &str = "\
   8       0 sda 200 0 2000 100 250 0 7500 150 0 100 250 0 0 0 0
   8       1 sda1 100 0 1000 50 125 0 3750 75 0 50 125 0 0 0 0
";

fn ctx(cpu_count: usize) -> CollectCtx {
    CollectCtx {
        duration: None,
        interval: None,
        cgroup_path: None,
        baseline: None,
        cpu_count,
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

#[derive(Debug)]
struct MockCollector {
    name: String,
    signals: Vec<Signal>,
}

impl Collector for MockCollector {
    fn id(&self) -> &str {
        &self.name
    }

    fn collect(&self, _ctx: &CollectCtx) -> usereport::collector::Result<Vec<Signal>> {
        Ok(self.signals.clone())
    }
}

// ---------------------------------------------------------------------------
// Criterion 1 — CPU delta engine
// ---------------------------------------------------------------------------

#[test]
fn ac_phase3_1_cpu_delta_emits_pct_signals_summing_to_100() {
    let signals = CpuCollector::from_proc_stat_snapshots(PROC_STAT_T0, PROC_STAT_T1, 1.0);

    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();
    for needed in ["cpu.usr_pct", "cpu.sys_pct", "cpu.iowait_pct", "cpu.idle_pct"] {
        assert!(ids.contains(&needed), "missing signal {}; got {:?}", needed, ids);
    }

    let pct_sum: f64 = signals
        .iter()
        .filter(|s| matches!(s.unit, Unit::Pct))
        .filter_map(|s| match s.value {
            SignalValue::F64(v) => Some(v),
            _ => None,
        })
        .sum();
    assert!(
        (pct_sum - 100.0).abs() < 1.0,
        "cpu pct signals should sum to ~100, got {}",
        pct_sum
    );
}

#[test]
fn ac_phase3_1_cpu_delta_emits_runqueue_from_procs_running() {
    let signals = CpuCollector::from_proc_stat_snapshots(PROC_STAT_T0, PROC_STAT_T1, 1.0);
    let r = signals.iter().find(|s| s.id == "vmstat.r").expect("vmstat.r present");
    match r.value {
        SignalValue::F64(v) => assert_eq!(v as i64, 8),
        SignalValue::I64(v) => assert_eq!(v, 8),
        _ => panic!("vmstat.r must be numeric"),
    }
}

// ---------------------------------------------------------------------------
// Criterion 2 — Disk delta engine
// ---------------------------------------------------------------------------

#[test]
fn ac_phase3_2_disk_delta_emits_per_device_signals() {
    let signals = DiskCollector::from_proc_diskstats_snapshots(PROC_DISKSTATS_T0, PROC_DISKSTATS_T1, 1.0);

    let ids: Vec<&str> = signals.iter().map(|s| s.id.as_str()).collect();
    let needed = [
        "disk.sda.read_iops",
        "disk.sda.write_iops",
        "disk.sda.util_pct",
        "disk.sda.await_ms",
    ];
    for n in needed {
        assert!(ids.contains(&n), "missing signal {}; got {:?}", n, ids);
    }
}

// ---------------------------------------------------------------------------
// Criterion 3 — CPU collector graceful fallback
// ---------------------------------------------------------------------------

#[test]
fn ac_phase3_3_cpu_collector_returns_ok_when_proc_stat_missing() {
    // Default collector reads /proc/stat at runtime. On macOS it's absent;
    // collect() must return Ok (empty signals or fallback) rather than Err.
    let collector = CpuCollector::new();
    let res = collector.collect(&ctx(4));
    assert!(
        res.is_ok(),
        "CpuCollector::collect must not error when /proc/stat is missing; got {:?}",
        res
    );
}

// ---------------------------------------------------------------------------
// Criterion 4 — Disk collector graceful fallback
// ---------------------------------------------------------------------------

#[test]
fn ac_phase3_4_disk_collector_returns_ok_when_proc_diskstats_missing() {
    let collector = DiskCollector::new();
    let res = collector.collect(&ctx(4));
    assert!(
        res.is_ok(),
        "DiskCollector::collect must not error when /proc/diskstats is missing; got {:?}",
        res
    );
}

// ---------------------------------------------------------------------------
// Criterion 5 — Pipeline wiring
// ---------------------------------------------------------------------------

#[test]
fn ac_phase3_5_analysis_run_populates_signals_and_findings() {
    // 9999 chosen to exceed any plausible cpu_count from
    // std::thread::available_parallelism() on the test host.
    let mock_signal = make_signal("vmstat.r", 9999.0);
    let collectors: Vec<Box<dyn Collector>> = vec![Box::new(MockCollector {
        name: "mock".to_string(),
        signals: vec![mock_signal],
    })];

    let rule = Rule {
        id: "cpu.runqueue_saturation".to_string(),
        when: Predicate::parse("vmstat.r > host.cpu_count").expect("parse"),
        severity: Severity::Warn,
        summary: "rq sat".to_string(),
        evidence_ids: vec!["vmstat.r".to_string(), "host.cpu_count".to_string()],
        suggest: vec![],
        description: None,
        links: vec![],
    };
    let engine = RuleEngine::new(vec![rule]);

    let hostinfos: Vec<Command> = vec![];
    let commands: Vec<Command> = vec![];
    let runner = ThreadRunner::new();
    let analysis = Analysis::new(Box::new(runner), &hostinfos, &commands).with_diagnostics(collectors, engine);
    let report: AnalysisReport = analysis.run(Context::new()).expect("run ok");

    assert!(
        report.signals().iter().any(|s| s.id == "vmstat.r"),
        "report.signals() must include vmstat.r; got {:?}",
        report.signals().iter().map(|s| s.id.as_str()).collect::<Vec<_>>()
    );
    assert!(
        report.findings().iter().any(|f| f.id == "cpu.runqueue_saturation"),
        "report.findings() must include cpu.runqueue_saturation; got {:?}",
        report.findings().iter().map(|f| f.id.as_str()).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Criterion 6 — Findings severity-ordered after pipeline run
// ---------------------------------------------------------------------------

#[test]
fn ac_phase3_6_pipeline_findings_are_severity_ordered() {
    let signals = vec![make_signal("vmstat.r", 100.0), make_signal("mem.free_pct", 1.0)];
    let collectors: Vec<Box<dyn Collector>> = vec![Box::new(MockCollector {
        name: "mock".to_string(),
        signals,
    })];
    let rules = vec![
        Rule {
            id: "warn.x".to_string(),
            when: Predicate::parse("vmstat.r > 0").expect("parse"),
            severity: Severity::Warn,
            summary: "warn".to_string(),
            evidence_ids: vec!["vmstat.r".to_string()],
            suggest: vec![],
            description: None,
            links: vec![],
        },
        Rule {
            id: "crit.x".to_string(),
            when: Predicate::parse("mem.free_pct < 5").expect("parse"),
            severity: Severity::Crit,
            summary: "crit".to_string(),
            evidence_ids: vec!["mem.free_pct".to_string()],
            suggest: vec![],
            description: None,
            links: vec![],
        },
    ];
    let engine = RuleEngine::new(rules);
    let hostinfos: Vec<Command> = vec![];
    let commands: Vec<Command> = vec![];
    let runner = ThreadRunner::new();
    let analysis = Analysis::new(Box::new(runner), &hostinfos, &commands).with_diagnostics(collectors, engine);
    let report = analysis.run(Context::new()).expect("run ok");

    assert!(
        report.findings().len() >= 2,
        "expected ≥2 findings; got {:?}",
        report.findings()
    );
    assert_eq!(
        report.findings()[0].severity,
        Severity::Crit,
        "Crit finding must come first"
    );
}

// ---------------------------------------------------------------------------
// Criterion 7 — `--cgroup` CLI flag exists
// (covered by inline tests in src/cli/mod.rs since `Opt` is private)
// Sanity test here: the Opt struct's cgroup field is observable via clap's
// CommandFactory (inline test in cli/mod.rs is the authoritative test).
// ---------------------------------------------------------------------------

#[test]
fn ac_phase3_7_cli_help_lists_cgroup_flag() {
    use clap::CommandFactory;
    use usereport::cli::Opt;

    let mut cmd = Opt::command();
    let help = format!("{}", cmd.render_help());
    assert!(
        help.contains("--cgroup"),
        "--help output must mention --cgroup; got:\n{}",
        help
    );
}

// ---------------------------------------------------------------------------
// Criterion 8 — End-to-end exit code
// ---------------------------------------------------------------------------

#[test]
fn ac_phase3_8_compute_exit_code_returns_one_when_pipeline_emits_warn() {
    let mock_signal = make_signal("vmstat.r", 9999.0);
    let collectors: Vec<Box<dyn Collector>> = vec![Box::new(MockCollector {
        name: "mock".to_string(),
        signals: vec![mock_signal],
    })];
    let rule = Rule {
        id: "cpu.runqueue_saturation".to_string(),
        when: Predicate::parse("vmstat.r > host.cpu_count").expect("parse"),
        severity: Severity::Warn,
        summary: "rq sat".to_string(),
        evidence_ids: vec!["vmstat.r".to_string()],
        suggest: vec![],
        description: None,
        links: vec![],
    };
    let engine = RuleEngine::new(vec![rule]);
    let hostinfos: Vec<Command> = vec![];
    let commands: Vec<Command> = vec![];
    let runner = ThreadRunner::new();
    let analysis = Analysis::new(Box::new(runner), &hostinfos, &commands).with_diagnostics(collectors, engine);
    let report = analysis.run(Context::new()).expect("run ok");

    let code = compute_exit_code(ExitOn::Warn, report.findings());
    assert_eq!(code, 1, "exit-on=warn with a warn finding must yield 1");
}
