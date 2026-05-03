//! SDD product-review Phase 1, C1.
//! GIVEN contrib/patterns/time_wait.toml whose predicate matches when
//!   net.tw_count > 28000 AND net.connect_failures > 0
//! WHEN Analysis::with_pattern_engine runs against a signal set satisfying
//!   those conditions (net.tw_count = 30000, net.connect_failures = 1)
//! THEN report.findings() contains at least one entry whose id is
//!   "time_wait_exhaustion".
//!
//! This test fails to compile today because PatternEngine::empty() and
//! extend_from() do not yet exist. That is the correct RED state.

use usereport::collector::CollectCtx;
use usereport::pattern::PatternEngine;
use usereport::signal::{Signal, SignalValue, Unit};

#[test]
fn pattern_engine_fires_time_wait_exhaustion() {
    // Build a PatternEngine from the embedded TOML, using the two new methods.
    // PatternEngine::empty() and extend_from() do not exist yet — this
    // causes a compile error, which is the intended RED state.
    let mut engine = PatternEngine::empty();
    let loaded = PatternEngine::from_toml(include_str!("../contrib/patterns/time_wait.toml")).unwrap();
    engine.extend_from(loaded);

    // Synthetic signals that satisfy: net.tw_count > 28000 AND net.connect_failures > 0
    let now = chrono::Local::now();
    let signals = vec![
        Signal {
            id: "net.tw_count".to_string(),
            value: SignalValue::F64(30_000.0),
            unit: Unit::Count,
            at: now,
            samples: None,
            stats: None,
            baseline: None,
        },
        Signal {
            id: "net.connect_failures".to_string(),
            value: SignalValue::F64(1.0),
            unit: Unit::Count,
            at: now,
            samples: None,
            stats: None,
            baseline: None,
        },
    ];

    // Run Analysis with the pattern engine and the synthetic signals injected
    // via a custom Runner that returns no command results (the pattern engine
    // runs against collector-produced signals, but here we use the extract
    // path: we drive it through with_pattern_engine and empty commands, then
    // push signals in via a stub collector).
    //
    // The cleanest way to inject synthetic signals without a subprocess is to
    // call PatternEngine::run directly and assert on its output.
    let ctx = CollectCtx::default();
    let findings = engine.run(&signals, &ctx);

    assert!(
        findings.iter().any(|f| f.id == "time_wait_exhaustion"),
        "expected finding 'time_wait_exhaustion' but got: {:?}",
        findings.iter().map(|f| &f.id).collect::<Vec<_>>()
    );
}
