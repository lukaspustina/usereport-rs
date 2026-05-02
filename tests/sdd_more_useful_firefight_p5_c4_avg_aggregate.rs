//! SDD more-useful-firefight Phase 5, C4.
//! GIVEN three matching lines with val captures "4", "6", "8" and aggregate = "avg"
//! WHEN extract_signals is called
//! THEN the returned signal has SignalValue::F64(6.0).

use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::signal::Unit;

#[test]
fn avg_aggregate_computes_mean() {
    let extracts = vec![CommandExtract {
        pattern: r"val=(?P<val>\d+)".to_string(),
        signal_id: "test.metric".to_string(),
        unit: Unit::None,
        aggregate: Aggregate::Avg,
    }];
    let stdout = "val=4\nval=6\nval=8\n";
    let signals = extract_signals("test_cmd", stdout, &extracts);
    assert_eq!(signals.len(), 1);
    match &signals[0].value {
        usereport::signal::SignalValue::F64(v) => assert!((*v - 6.0).abs() < 0.001, "expected 6.0, got {v}"),
        other => panic!("expected F64(6.0), got {other:?}"),
    }
}
