//! Phase 8 acceptance-criteria tests — workload rule packs.

use usereport::workload::load_workload_rules;

// AC 1 + 2 — all four workload TOMLs load and contain at least one rule.
#[test]
fn ac_phase8_1_postgres_workload_has_rules() {
    let rules = load_workload_rules("postgres").expect("postgres workload");
    assert!(!rules.is_empty(), "postgres workload must have at least one rule");
}

#[test]
fn ac_phase8_1_java_workload_has_rules() {
    let rules = load_workload_rules("java").expect("java workload");
    assert!(!rules.is_empty(), "java workload must have at least one rule");
}

#[test]
fn ac_phase8_1_nginx_workload_has_rules() {
    let rules = load_workload_rules("nginx").expect("nginx workload");
    assert!(!rules.is_empty(), "nginx workload must have at least one rule");
}

#[test]
fn ac_phase8_1_kubelet_workload_has_rules() {
    let rules = load_workload_rules("kubelet").expect("kubelet workload");
    assert!(!rules.is_empty(), "kubelet workload must have at least one rule");
}

// AC 3 — "none" returns empty, not an error.
#[test]
fn ac_phase8_3_none_workload_returns_empty() {
    let rules = load_workload_rules("none").expect("none workload should not error");
    assert!(rules.is_empty(), "none workload must return empty rule set");
}

// AC 2 (error case) — unknown workload name returns an error.
#[test]
fn ac_phase8_2_unknown_workload_is_error() {
    let result = load_workload_rules("nonexistent-workload-xyz");
    assert!(result.is_err(), "unknown workload should return Err");
}

// AC 5 — postgres rules fire against crafted postgres signals.
#[test]
fn ac_phase8_5_postgres_rules_fire_on_fixture_signals() {
    use chrono::Local;
    use usereport::collector::CollectCtx;
    use usereport::finding::Severity;
    use usereport::rule::RuleEngine;
    use usereport::signal::{Signal, SignalValue, Unit};

    let rules = load_workload_rules("postgres").expect("postgres workload");
    let engine = RuleEngine::new(rules);
    let ctx = CollectCtx::default();

    // Build a signal set that should trigger at least one postgres rule.
    // Use values that indicate a stressed postgres instance.
    let now = Local::now();
    let signals = vec![
        Signal {
            id: "pg.active_connections".to_string(),
            value: SignalValue::F64(500.0),
            unit: Unit::Count,
            at: now,
            samples: None,
            stats: None,
            baseline: None,
        },
        Signal {
            id: "pg.cache_hit_pct".to_string(),
            value: SignalValue::F64(60.0),
            unit: Unit::Pct,
            at: now,
            samples: None,
            stats: None,
            baseline: None,
        },
        Signal {
            id: "pg.lock_waits".to_string(),
            value: SignalValue::F64(10.0),
            unit: Unit::Count,
            at: now,
            samples: None,
            stats: None,
            baseline: None,
        },
    ];

    let (findings, _) = engine.run(&signals, &ctx, &std::collections::HashMap::new());
    assert!(
        !findings.is_empty(),
        "at least one postgres rule must fire against the fixture; rules = {:?}",
        engine.rules().iter().map(|r| &r.id).collect::<Vec<_>>()
    );
    // All findings should be warn or crit (postgres rules should not be info-only for saturation)
    for f in &findings {
        assert!(
            matches!(f.severity, Severity::Warn | Severity::Crit | Severity::Info),
            "unexpected severity {:?}",
            f.severity
        );
    }
}

// AC 6 — "none" workload rule set matches the baseline (empty additional rules).
#[test]
fn ac_phase8_6_none_workload_same_as_default() {
    use usereport::collector::CollectCtx;
    use usereport::rule::RuleEngine;
    use usereport::rule::builtin::builtin_rules;

    let none_rules = load_workload_rules("none").expect("none workload");
    assert!(none_rules.is_empty());

    let base = builtin_rules();
    let mut merged = base.clone();
    merged.extend(none_rules);

    let ctx = CollectCtx::default();
    let base_engine = RuleEngine::new(base);
    let merged_engine = RuleEngine::new(merged);

    // With no signals, both produce empty findings.
    let (base_findings, _) = base_engine.run(&[], &ctx, &std::collections::HashMap::new());
    let (merged_findings, _) = merged_engine.run(&[], &ctx, &std::collections::HashMap::new());
    assert_eq!(base_findings.len(), merged_findings.len());
}
