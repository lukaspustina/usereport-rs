//! SDD more-useful-firefight Phase 5, C5.
//! GIVEN three matching lines and aggregate = "last"
//! WHEN extract_signals is called
//! THEN the returned signal value equals SignalValue::F64 of the last parsed capture.

use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::signal::{SignalValue, Unit};

#[test]
fn last_aggregate_returns_f64_of_last_capture() {
    let stdout = "value=10\nvalue=20\nvalue=30\n";
    let extracts = vec![CommandExtract {
        pattern: r"value=(?P<val>\d+)".to_string(),
        signal_id: "test.last_signal".to_string(),
        unit: Unit::None,
        aggregate: Aggregate::Last,
    }];
    let signals = extract_signals("test_cmd", stdout, &extracts);
    assert_eq!(signals.len(), 1, "expected exactly one signal");
    assert_eq!(signals[0].value, SignalValue::F64(30.0));
}
