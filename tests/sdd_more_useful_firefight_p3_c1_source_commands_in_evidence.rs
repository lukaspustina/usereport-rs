//! SDD more-useful-firefight Phase 3, C1.
//! GIVEN source_map = {"cpu.iowait_pct": ["sar_cpu"]} and a rule fires with evidence signal cpu.iowait_pct
//! WHEN RuleEngine::run builds the Finding
//! THEN finding.evidence[0].source_commands equals ["sar_cpu"].

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
fn source_commands_populated_in_evidence() {
    let rule = Rule {
        id: "cpu.iowait_test".to_string(),
        severity: Severity::Warn,
        summary: "iowait high".to_string(),
        when: Predicate::Cmp {
            path: vec!["cpu".to_string(), "iowait_pct".to_string()],
            op: Op::Gt,
            rhs: Rhs::Value(Value::Number(5.0)),
        },
        evidence_ids: vec!["cpu.iowait_pct".to_string()],
        suggest: vec![],
        description: None,
        links: vec![],
    };
    let engine = RuleEngine::new(vec![rule]);
    let signals = vec![make_signal("cpu.iowait_pct", 25.0)];
    let ctx = CollectCtx::default();
    let mut source_map: HashMap<String, Vec<String>> = HashMap::new();
    source_map.insert("cpu.iowait_pct".to_string(), vec!["sar_cpu".to_string()]);
    let (findings, _) = engine.run(&signals, &ctx, &source_map);
    assert_eq!(findings.len(), 1, "expected one finding");
    let ev = &findings[0].evidence[0];
    assert_eq!(
        ev.source_commands,
        vec!["sar_cpu".to_string()],
        "source_commands must be [\"sar_cpu\"]: {ev:?}"
    );
}
