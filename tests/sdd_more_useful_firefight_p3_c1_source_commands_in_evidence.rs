//! SDD more-useful-firefight Phase 3, C1.
//! GIVEN source_map = {"cpu.iowait_pct": ["sar_cpu"]} and a rule fires with evidence signal cpu.iowait_pct
//! WHEN RuleEngine::run builds the Finding
//! THEN finding.evidence[0].source_commands equals ["sar_cpu"].

use std::collections::HashMap;
use usereport::collector::CollectCtx;
use usereport::finding::Severity;
use usereport::rule::{Predicate, Rule, RuleEngine};
use usereport::signal::{Signal, SignalValue, Unit};

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
fn source_commands_populated_from_source_map() {
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
    let signals = vec![make_signal("cpu.iowait_pct", 25.0)];
    let mut source_map = HashMap::new();
    source_map.insert("cpu.iowait_pct".to_string(), vec!["sar_cpu".to_string()]);
    let engine = RuleEngine::new(vec![rule]);
    let (findings, _) = engine.run(&signals, &ctx(), &source_map);
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].evidence[0].source_commands,
        vec!["sar_cpu".to_string()],
        "source_commands must be populated from source_map: {:?}",
        findings[0].evidence[0].source_commands
    );
}
