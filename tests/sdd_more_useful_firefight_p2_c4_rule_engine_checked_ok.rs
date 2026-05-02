//! SDD more-useful-firefight Phase 2, C4.
//! GIVEN signal cpu.iowait_pct = 5.0 and rule fires only when > 20
//! WHEN RuleEngine::run is called with source_map = &HashMap::new()
//! THEN findings is empty and checked_ok contains "cpu.iowait_pct".

use std::collections::HashMap;
use usereport::collector::CollectCtx;
use usereport::rule::{Predicate, Rule, RuleEngine};
use usereport::signal::{Signal, SignalValue, Unit};
use usereport::finding::Severity;

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

#[test]
fn rule_not_fired_signal_in_checked_ok() {
    let rule = Rule {
        id: "cpu.iowait_high".to_string(),
        when: Predicate::parse("cpu.iowait_pct > 20").expect("parse"),
        severity: Severity::Warn,
        summary: "iowait high".to_string(),
        evidence_ids: vec!["cpu.iowait_pct".to_string()],
        suggest: vec![],
        description: None,
        links: vec![],
    };
    let signals = vec![make_signal("cpu.iowait_pct", 5.0)];
    let engine = RuleEngine::new(vec![rule]);
    let (findings, checked_ok) = engine.run(&signals, &ctx(), &HashMap::new());
    assert!(findings.is_empty(), "no finding should fire: {:?}", findings);
    assert!(
        checked_ok.contains(&"cpu.iowait_pct".to_string()),
        "checked_ok must contain cpu.iowait_pct: {:?}",
        checked_ok
    );
}
