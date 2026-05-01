//! Phase 7 acceptance-criteria tests — eBPF opt-in collectors.

use usereport::baseline::stats::sample_stats;
use usereport::signal::{SampleStats, Signal, SignalValue, Unit};

fn dummy_signal_with_samples(id: &str, samples: Vec<f64>) -> Signal {
    use chrono::Local;
    let stats = sample_stats(&samples);
    Signal {
        id: id.to_string(),
        value: SignalValue::F64(samples[0]),
        unit: Unit::Microseconds,
        at: Local::now(),
        samples: Some(samples),
        stats,
        baseline: None,
    }
}

// AC 4a — SampleStats has a p99 field.
#[test]
fn ac_phase7_4a_sample_stats_has_p99() {
    let values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
    let stats: SampleStats = sample_stats(&values).expect("non-empty");
    // p99 for 1..=100 with linear interpolation: rank = 0.99 * 99 = 98.01
    // sorted[98] = 99.0, sorted[99] = 100.0; result = 99 + 0.01 = 99.01
    assert!((stats.p99 - 99.01).abs() < 0.1, "p99 = {}", stats.p99);
}

// AC 4b — sample_stats() p99 is consistent with standalone percentile().
#[test]
fn ac_phase7_4b_sample_stats_p99_consistent() {
    use usereport::baseline::stats::percentile;
    let values: Vec<f64> = (1..=20).map(|i| i as f64).collect();
    let stats = sample_stats(&values).expect("non-empty");
    let expected = percentile(&values, 99.0).expect("non-empty");
    assert!((stats.p99 - expected).abs() < 1e-9);
}

// AC 4c — rule engine resolves .p99 suffix via SampleStats.
#[test]
fn ac_phase7_4c_rule_p99_suffix_fires() {
    use usereport::collector::CollectCtx;
    use usereport::finding::Severity;
    use usereport::rule::{RuleEngine, parse_rules_toml};

    let toml = r#"
[[rule]]
id = "test.high_p99_latency"
when = "lat.p99 > 90"
severity = "warn"
summary = "p99 latency elevated"
evidence = ["lat"]
suggest = []
"#;
    let engine = RuleEngine::new(parse_rules_toml(toml).expect("parse rules"));
    let ctx = CollectCtx::default();

    // p99 of 1..=100 is ≈ 99.01 > 90 → should fire
    let sig = dummy_signal_with_samples("lat", (1..=100).map(|i| i as f64).collect());
    let findings = engine.run(&[sig], &ctx);
    assert_eq!(findings.len(), 1, "expected 1 finding, got {}", findings.len());
    assert_eq!(findings[0].id, "test.high_p99_latency");
    assert_eq!(findings[0].severity, Severity::Warn);
}

// AC 4d — rule engine .p99 suffix does NOT fire when p99 is below threshold.
#[test]
fn ac_phase7_4d_rule_p99_suffix_no_fire_when_below() {
    use usereport::collector::CollectCtx;
    use usereport::rule::{RuleEngine, parse_rules_toml};

    let toml = r#"
[[rule]]
id = "test.high_p99_latency"
when = "lat.p99 > 200"
severity = "warn"
summary = "p99 latency elevated"
evidence = ["lat"]
suggest = []
"#;
    let engine = RuleEngine::new(parse_rules_toml(toml).expect("parse rules"));
    let ctx = CollectCtx::default();

    let sig = dummy_signal_with_samples("lat", (1..=100).map(|i| i as f64).collect());
    let findings = engine.run(&[sig], &ctx);
    assert!(findings.is_empty(), "expected no findings, got {}", findings.len());
}

// AC 2 + 3 — BpfCollector emits availability signals; bpf rules produce Info
// findings for missing tools. These tests only compile with feature "bpf".
#[cfg(feature = "bpf")]
mod bpf_tests {
    use usereport::collector::{CollectCtx, bpf::BpfCollector};
    use usereport::finding::Severity;
    use usereport::rule::{RuleEngine, builtin::bpf_rules};
    use usereport::signal::SignalValue;

    // AC 2 — BpfCollector produces bpf.<tool>.available = false for all tools
    // not found in PATH (macOS CI has none of the bpftrace family).
    #[test]
    fn ac_phase7_2_missing_tools_emit_availability_signals() {
        use usereport::collector::Collector as _;
        let collector = BpfCollector::new();
        let ctx = CollectCtx::default();
        let signals = collector.collect(&ctx).expect("collect ok");

        // At least some signals — may be mix of true/false depending on host.
        // On CI (macOS/Linux without bpftrace) all should be false.
        let availability: Vec<_> = signals.iter().filter(|s| s.id.ends_with(".available")).collect();
        assert_eq!(
            availability.len(),
            5,
            "expected 5 availability signals, got {}: {:?}",
            availability.len(),
            availability.iter().map(|s| &s.id).collect::<Vec<_>>()
        );
        for sig in &availability {
            assert!(
                matches!(sig.value, SignalValue::Bool(_)),
                "availability signal {} should be Bool, got {:?}",
                sig.id,
                sig.value
            );
        }
    }

    // AC 3 — bpf_rules() + missing availability signals → one Info finding per tool.
    #[test]
    fn ac_phase7_3_bpf_rules_produce_info_findings_for_missing_tools() {
        use chrono::Local;
        use usereport::collector::CollectCtx;

        let tools = ["runqlat", "biolatency", "tcpretrans", "execsnoop", "cachestat"];

        // Build signals with all tools missing.
        let signals: Vec<_> = tools
            .iter()
            .map(|t| usereport::signal::Signal {
                id: format!("bpf.{}.available", t),
                value: SignalValue::Bool(false),
                unit: usereport::signal::Unit::None,
                at: Local::now(),
                samples: None,
                stats: None,
                baseline: None,
            })
            .collect();

        let engine = RuleEngine::new(bpf_rules());
        let ctx = CollectCtx::default();
        let findings = engine.run(&signals, &ctx);

        assert_eq!(
            findings.len(),
            5,
            "expected 5 Info findings (one per tool), got {}: {:?}",
            findings.len(),
            findings.iter().map(|f| &f.id).collect::<Vec<_>>()
        );
        for f in &findings {
            assert_eq!(f.severity, Severity::Info, "finding {} should be Info", f.id);
        }
    }
}
