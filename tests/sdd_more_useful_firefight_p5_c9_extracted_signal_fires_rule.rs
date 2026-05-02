//! SDD more-useful-firefight Phase 5, C9.
//! GIVEN dmesg.oom_count is extracted with value 2 AND a rule "dmesg.oom_count > 0" severity Crit
//! WHEN the rule engine runs
//! THEN a Crit finding fires for dmesg.oom_count.

use std::collections::HashMap;
use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::collector::CollectCtx;
use usereport::extract::extract_signals;
use usereport::finding::Severity;
use usereport::rule::{Op, Predicate, Rhs, Rule, RuleEngine, Value};
use usereport::signal::Unit;

#[test]
fn extracted_signal_fires_rule() {
    let extracts = vec![CommandExtract {
        pattern: "Out of memory:".to_string(),
        signal_id: "dmesg.oom_count".to_string(),
        unit: Unit::Count,
        aggregate: Aggregate::Count,
    }];
    let stdout = "Out of memory: kill 1\nOut of memory: kill 2\n";
    let signals = extract_signals("dmesg", stdout, &extracts);
    assert_eq!(signals.len(), 1);

    let rule = Rule {
        id: "dmesg.oom".to_string(),
        severity: Severity::Crit,
        summary: "OOM events detected".to_string(),
        when: Predicate::Cmp {
            path: vec!["dmesg".to_string(), "oom_count".to_string()],
            op: Op::Gt,
            rhs: Rhs::Value(Value::Number(0.0)),
        },
        evidence_ids: vec!["dmesg.oom_count".to_string()],
        suggest: vec![],
        description: None,
        links: vec![],
    };
    let engine = RuleEngine::new(vec![rule]);
    let ctx = CollectCtx::default();
    let (findings, _) = engine.run(&signals, &ctx, &HashMap::new());
    assert_eq!(findings.len(), 1, "expected one finding");
    assert_eq!(findings[0].severity, Severity::Crit);
}
