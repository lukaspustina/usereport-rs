//! SDD more-useful-firefight Phase 2, C5.
//! GIVEN cpu.iowait_pct = 25.0 and a rule that fires when cpu.iowait_pct > 20
//! WHEN RuleEngine::run is called
//! THEN findings contains one entry and checked_ok does NOT contain "cpu.iowait_pct".

use std::collections::HashMap;
use usereport::collector::CollectCtx;
use usereport::finding::Severity;
use usereport::rule::{Op, Predicate, Rhs, Rule, RuleEngine, Value};
use usereport::signal::{Signal, SignalValue, Unit};

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
fn rule_engine_does_not_add_fired_signal_to_checked_ok() {
    let rule = Rule {
        id: "cpu.iowait_test".to_string(),
        severity: Severity::Warn,
        summary: "iowait high".to_string(),
        when: Predicate::Cmp {
            path: vec!["cpu".to_string(), "iowait_pct".to_string()],
            op: Op::Gt,
            rhs: Rhs::Value(Value::Number(20.0)),
        },
        evidence_ids: vec!["cpu.iowait_pct".to_string()],
        suggest: vec![],
        description: None,
        links: vec![],
    };
    let engine = RuleEngine::new(vec![rule]);
    let signals = vec![make_signal("cpu.iowait_pct", 25.0)];
    let ctx = CollectCtx::default();
    let (findings, checked_ok) = engine.run(&signals, &ctx, &HashMap::new());
    assert_eq!(findings.len(), 1, "expected one finding");
    assert!(
        !checked_ok.contains(&"cpu.iowait_pct".to_string()),
        "fired signal must not appear in checked_ok: {checked_ok:?}"
    );
}
