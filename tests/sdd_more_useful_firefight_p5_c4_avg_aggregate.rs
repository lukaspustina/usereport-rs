//! SDD more-useful-firefight Phase 5, C4.
//! GIVEN three matching lines with val captures "4", "6", "8" and aggregate = "avg"
//! WHEN extract_signals is called
//! THEN the returned signal has SignalValue::F64(6.0).

use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::signal::{SignalValue, Unit};

#[test]
fn avg_aggregate_returns_f64_6() {
    let stdout = "value=4\nvalue=6\nvalue=8\n";
    let extracts = vec![CommandExtract {
        pattern: r"value=(?P<val>\d+)".to_string(),
        signal_id: "test.avg_signal".to_string(),
        unit: Unit::None,
        aggregate: Aggregate::Avg,
    }];
    let signals = extract_signals("test_cmd", stdout, &extracts);
    assert_eq!(signals.len(), 1, "expected exactly one signal");
    assert_eq!(signals[0].value, SignalValue::F64(6.0));
}
