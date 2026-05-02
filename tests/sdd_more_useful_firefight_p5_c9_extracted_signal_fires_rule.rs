//! SDD more-useful-firefight Phase 5, C9.
//! GIVEN dmesg.oom_count is extracted with value 2 AND a rule `dmesg.oom_count > 0` severity Crit
//! WHEN the rule engine runs
//! THEN a Crit finding fires for dmesg.oom_count.

use std::collections::HashMap;
use usereport::collector::CollectCtx;
use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::finding::Severity;
use usereport::rule::{Predicate, Rule, RuleEngine};
use usereport::signal::Unit;

fn ctx() -> CollectCtx {
    CollectCtx {
        duration: None,
        interval: None,
        cgroup_path: None,
        baseline: None,
        cpu_count: 4,
    }
}

#[test]
fn extracted_oom_count_fires_crit_rule() {
    // Build two OOM lines so the count = 2.
    let stdout = "Out of memory: Killed process 123\nOut of memory: Killed process 456\n";
    let extracts = vec![CommandExtract {
        pattern: "Out of memory:".to_string(),
        signal_id: "dmesg.oom_count".to_string(),
        unit: Unit::Count,
        aggregate: Aggregate::Count,
    }];

    let signals = extract_signals("dmesg", stdout, &extracts);
    assert_eq!(signals.len(), 1, "extract_signals must emit exactly one signal");

    let rule = Rule {
        id: "dmesg.oom_fired".to_string(),
        when: Predicate::parse("dmesg.oom_count > 0").expect("parse"),
        severity: Severity::Crit,
        summary: "OOM events detected".to_string(),
        evidence_ids: vec!["dmesg.oom_count".to_string()],
        suggest: vec![],
        description: None,
        links: vec![],
    };

    let engine = RuleEngine::new(vec![rule]);
    let (findings, _checked_ok) = engine.run(&signals, &ctx(), &HashMap::new());

    assert_eq!(findings.len(), 1, "exactly one finding must fire: {findings:?}");
    assert_eq!(
        findings[0].severity,
        Severity::Crit,
        "finding must be Crit, got {:?}",
        findings[0].severity
    );
    assert_eq!(
        findings[0].id, "dmesg.oom_fired",
        "unexpected finding id: {}",
        findings[0].id
    );
}
