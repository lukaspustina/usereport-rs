//! SDD more-useful-firefight Phase 5, C2.
//! GIVEN three stdout lines with val captures "5", "18", "12" and aggregate = "max"
//! WHEN extract_signals is called
//! THEN the returned signal has SignalValue::F64(18.0).

use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::signal::{SignalValue, Unit};

#[test]
fn max_aggregate_returns_f64_18() {
    let stdout = "value=5\nvalue=18\nvalue=12\n";
    let extracts = vec![CommandExtract {
        pattern: r"value=(?P<val>\d+)".to_string(),
        signal_id: "test.max_signal".to_string(),
        unit: Unit::None,
        aggregate: Aggregate::Max,
    }];
    let signals = extract_signals("test_cmd", stdout, &extracts);
    assert_eq!(signals.len(), 1, "expected exactly one signal");
    assert_eq!(signals[0].value, SignalValue::F64(18.0));
}
