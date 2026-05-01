//! Integration tests for SDD `specs/sdd/version-2.md` Phase 1
//! (diagnostic foundation: Signal, Finding, Collector, Rule engine, exit-on).
//!
//! Each test maps to a numbered acceptance criterion in
//! `specs/sdd/version-2.md` Test Scenarios section.
#![cfg(feature = "bin")]

use usereport::collector::memory::MemoryCollector;
use usereport::collector::{CollectCtx, Collector};
use usereport::finding::{Evidence, Finding, FindingKind, Severity};
use usereport::renderer::JsonRenderer;
use usereport::rule::{Predicate, Rule, RuleEngine};
use usereport::signal::{Signal, SignalValue, Unit};
use usereport::Renderer;

const FREE_OUTPUT_LINUX: &str = "\
              total        used        free      shared  buff/cache   available
Mem:           7977         511        4983          15        2483        7191
Swap:          7977         512        7465
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
        baseline: None,
    }
}

// ---------------------------------------------------------------------------
// AC-1 — Signal identity stability
// ---------------------------------------------------------------------------

#[test]
fn ac_1_memory_collector_signal_identity_stable() {
    let collector = MemoryCollector::from_stdout(FREE_OUTPUT_LINUX.to_string());
    let c = ctx(4);
    let first = collector.collect(&c).expect("first collect");
    let second = collector.collect(&c).expect("second collect");

    assert_eq!(first.len(), second.len(), "signal counts must match");

    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.id, b.id, "signal id mismatch between runs");
        assert_eq!(a.unit, b.unit, "unit mismatch for {}", a.id);
        match (&a.value, &b.value) {
            (SignalValue::F64(x), SignalValue::F64(y)) => {
                assert!((x - y).abs() < f64::EPSILON, "value drift for {}: {} vs {}", a.id, x, y);
            }
            (SignalValue::I64(x), SignalValue::I64(y)) => assert_eq!(x, y),
            (SignalValue::Bool(x), SignalValue::Bool(y)) => assert_eq!(x, y),
            (SignalValue::Text(x), SignalValue::Text(y)) => assert_eq!(x, y),
            _ => panic!("variant mismatch for {}", a.id),
        }
    }

    let mut ids: Vec<&str> = first.iter().map(|s| s.id.as_str()).collect();
    ids.sort();
    let unique_count = {
        let mut sorted = ids.clone();
        sorted.dedup();
        sorted.len()
    };
    assert_eq!(unique_count, ids.len(), "duplicate signal IDs: {:?}", ids);
}

// ---------------------------------------------------------------------------
// AC-2 — Rule predicate match
// ---------------------------------------------------------------------------

#[test]
fn ac_2_rule_predicate_match_emits_finding() {
    let rule = Rule {
        id: "cpu.runqueue_saturation".to_string(),
        when: Predicate::parse("vmstat.r > host.cpu_count").expect("parse"),
        severity: Severity::Warn,
        summary: "Run queue exceeds core count".to_string(),
        evidence_ids: vec!["vmstat.r".to_string(), "host.cpu_count".to_string()],
        suggest: vec!["pidstat 1 5".to_string()],
    };

    let signals = vec![make_signal("vmstat.r", 8.0)];
    let engine = RuleEngine::new(vec![rule]);
    let findings = engine.run(&signals, &ctx(4));

    assert_eq!(findings.len(), 1, "exactly one finding expected");
    let f = &findings[0];
    assert_eq!(f.id, "cpu.runqueue_saturation");
    assert_eq!(f.severity, Severity::Warn);
    let evidence_ids: Vec<&str> = f.evidence.iter().map(|e| e.signal_id.as_str()).collect();
    assert!(
        evidence_ids.contains(&"vmstat.r"),
        "evidence missing vmstat.r: {:?}",
        evidence_ids
    );
    assert!(
        evidence_ids.contains(&"host.cpu_count"),
        "evidence missing host.cpu_count: {:?}",
        evidence_ids
    );
}

// ---------------------------------------------------------------------------
// AC-3 — Rule predicate non-match
// ---------------------------------------------------------------------------

#[test]
fn ac_3_rule_predicate_non_match_emits_no_finding() {
    let rule = Rule {
        id: "cpu.runqueue_saturation".to_string(),
        when: Predicate::parse("vmstat.r > host.cpu_count").expect("parse"),
        severity: Severity::Warn,
        summary: "Run queue exceeds core count".to_string(),
        evidence_ids: vec!["vmstat.r".to_string()],
        suggest: vec![],
    };

    let signals = vec![make_signal("vmstat.r", 2.0)];
    let engine = RuleEngine::new(vec![rule]);
    let findings = engine.run(&signals, &ctx(4));

    assert!(findings.is_empty(), "no finding expected, got: {:?}", findings);
}

#[test]
fn ac_3_rule_with_absent_signal_emits_no_finding() {
    // Per Data Models §449: predicate evaluator returns Ok(false) when a referenced
    // signal is absent.
    let rule = Rule {
        id: "absent.signal_test".to_string(),
        when: Predicate::parse("does.not.exist > 0").expect("parse"),
        severity: Severity::Warn,
        summary: "won't fire".to_string(),
        evidence_ids: vec!["does.not.exist".to_string()],
        suggest: vec![],
    };

    let signals: Vec<Signal> = vec![];
    let engine = RuleEngine::new(vec![rule]);
    let findings = engine.run(&signals, &ctx(4));

    assert!(findings.is_empty(), "absent signal must not produce finding");
}

// ---------------------------------------------------------------------------
// AC-4 — Exit-code behaviour
// ---------------------------------------------------------------------------

#[test]
fn ac_4_exit_code_never_default_returns_zero() {
    use usereport::cli::{compute_exit_code, ExitOn};
    let warn = Finding {
        id: "x".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "".to_string(),
        evidence: vec![],
        suggest: vec![],
    };
    assert_eq!(compute_exit_code(ExitOn::Never, std::slice::from_ref(&warn)), 0);
    assert_eq!(compute_exit_code(ExitOn::Never, &[]), 0);
}

#[test]
fn ac_4_exit_code_warn_with_warn_finding_returns_one() {
    use usereport::cli::{compute_exit_code, ExitOn};
    let warn = Finding {
        id: "x".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "".to_string(),
        evidence: vec![],
        suggest: vec![],
    };
    assert_eq!(compute_exit_code(ExitOn::Warn, &[warn]), 1);
}

#[test]
fn ac_4_exit_code_crit_with_warn_finding_returns_zero() {
    use usereport::cli::{compute_exit_code, ExitOn};
    let warn = Finding {
        id: "x".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "".to_string(),
        evidence: vec![],
        suggest: vec![],
    };
    assert_eq!(compute_exit_code(ExitOn::Crit, &[warn]), 0);
}

#[test]
fn ac_4_exit_code_crit_with_crit_finding_returns_two() {
    use usereport::cli::{compute_exit_code, ExitOn};
    let crit = Finding {
        id: "x".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Crit,
        summary: "".to_string(),
        evidence: vec![],
        suggest: vec![],
    };
    assert_eq!(compute_exit_code(ExitOn::Crit, &[crit]), 2);
}

// ---------------------------------------------------------------------------
// AC-5 — Malformed rule file isolation
// ---------------------------------------------------------------------------

#[test]
fn ac_5_malformed_user_rule_does_not_block_builtins() {
    use usereport::rule::RulesLoader;

    let tmp = tempfile::tempdir().expect("tempdir");
    let bad_path = tmp.path().join("bad.toml");
    std::fs::write(&bad_path, "this is not valid TOML !!!@@@").unwrap();

    let builtins = vec![Rule {
        id: "builtin.always_true".to_string(),
        when: Predicate::parse("vmstat.r > 0").expect("parse"),
        severity: Severity::Warn,
        summary: "always fires when r > 0".to_string(),
        evidence_ids: vec!["vmstat.r".to_string()],
        suggest: vec![],
    }];

    let load_result = RulesLoader::new()
        .with_builtins(builtins)
        .with_user_dir(tmp.path())
        .load();

    // The malformed file must not poison built-ins.
    assert!(
        load_result.rules.iter().any(|r| r.id == "builtin.always_true"),
        "built-in rule must still load"
    );

    // Plus a warn finding referencing the bad path is produced.
    let path_str = bad_path.to_string_lossy().into_owned();
    let warn_finding_about_bad = load_result
        .load_findings
        .iter()
        .find(|f| f.severity == Severity::Warn && f.summary.contains(&*path_str));
    assert!(
        warn_finding_about_bad.is_some(),
        "expected warn finding mentioning {}, got: {:?}",
        path_str,
        load_result.load_findings
    );
}

// ---------------------------------------------------------------------------
// AC-7 — Rule engine determinism
// ---------------------------------------------------------------------------

#[test]
fn ac_7_rule_engine_is_deterministic_across_runs() {
    let rules = vec![
        Rule {
            id: "a.rule".to_string(),
            when: Predicate::parse("vmstat.r > 1").expect("parse"),
            severity: Severity::Warn,
            summary: "a".to_string(),
            evidence_ids: vec!["vmstat.r".to_string()],
            suggest: vec![],
        },
        Rule {
            id: "b.rule".to_string(),
            when: Predicate::parse("mem.free_pct < 20").expect("parse"),
            severity: Severity::Crit,
            summary: "b".to_string(),
            evidence_ids: vec!["mem.free_pct".to_string()],
            suggest: vec![],
        },
    ];

    let signals = vec![make_signal("vmstat.r", 8.0), make_signal("mem.free_pct", 5.0)];

    let engine = RuleEngine::new(rules);
    let first = engine.run(&signals, &ctx(4));
    let second = engine.run(&signals, &ctx(4));

    assert_eq!(first.len(), second.len(), "finding count differs");
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.id, b.id, "id order differs across runs");
        assert_eq!(a.severity, b.severity);
    }
}

// ---------------------------------------------------------------------------
// AC-10 — JSON renderer extension
// ---------------------------------------------------------------------------

#[test]
fn ac_10_json_renderer_includes_signals_findings_checked_ok() {
    use usereport::analysis::{AnalysisReport, Context};

    let signal = make_signal("cpu.iowait_pct", 25.0);
    let finding = Finding {
        id: "cpu.iowait_high".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "iowait elevated".to_string(),
        evidence: vec![Evidence {
            signal_id: "cpu.iowait_pct".to_string(),
            observed: SignalValue::F64(25.0),
        }],
        suggest: vec!["iotop -ao".to_string()],
    };

    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![],
        1,
        64,
        vec![signal],
        vec![finding],
        vec!["mem.free_pct".to_string()],
    );

    let renderer = JsonRenderer::new();
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("valid json");

    assert!(
        v.get("signals").and_then(|x| x.as_array()).is_some(),
        "signals array missing: {}",
        s
    );
    assert!(
        v.get("findings").and_then(|x| x.as_array()).is_some(),
        "findings array missing"
    );
    assert!(
        v.get("checked_ok").and_then(|x| x.as_array()).is_some(),
        "checked_ok array missing"
    );
    assert!(v.get("command_results").is_some(), "command_results still present");

    let f0 = &v["findings"][0];
    assert!(f0.get("id").is_some());
    assert!(f0.get("severity").is_some());
    assert!(f0.get("summary").is_some());
    assert!(f0.get("evidence").is_some());
    assert!(f0.get("suggest").is_some());
}

// ---------------------------------------------------------------------------
// Severity-ordered findings (no AC#, but required by Phase 1 completion criteria)
// ---------------------------------------------------------------------------

#[test]
fn severity_ordered_findings_crit_first_then_warn_then_info_lex_within() {
    use usereport::finding::sort_findings;

    let mut findings = vec![
        Finding {
            id: "b".to_string(),
            kind: FindingKind::Rule,
            severity: Severity::Warn,
            summary: "b".to_string(),
            evidence: vec![],
            suggest: vec![],
        },
        Finding {
            id: "a".to_string(),
            kind: FindingKind::Rule,
            severity: Severity::Crit,
            summary: "a".to_string(),
            evidence: vec![],
            suggest: vec![],
        },
        Finding {
            id: "c".to_string(),
            kind: FindingKind::Rule,
            severity: Severity::Info,
            summary: "c".to_string(),
            evidence: vec![],
            suggest: vec![],
        },
        Finding {
            id: "a".to_string(),
            kind: FindingKind::Rule,
            severity: Severity::Warn,
            summary: "a".to_string(),
            evidence: vec![],
            suggest: vec![],
        },
    ];

    sort_findings(&mut findings);

    assert_eq!(findings[0].severity, Severity::Crit);
    assert_eq!(findings[0].id, "a");
    assert_eq!(findings[1].severity, Severity::Warn);
    assert_eq!(findings[1].id, "a");
    assert_eq!(findings[2].severity, Severity::Warn);
    assert_eq!(findings[2].id, "b");
    assert_eq!(findings[3].severity, Severity::Info);
    assert_eq!(findings[3].id, "c");
}
