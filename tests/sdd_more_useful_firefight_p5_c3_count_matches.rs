//! SDD more-useful-firefight Phase 5, C3.
//! GIVEN aggregate = "count" and a pattern that matches 3 lines
//! WHEN extract_signals is called
//! THEN the returned signal has SignalValue::I64(3).

use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::signal::{SignalValue, Unit};

#[test]
fn count_aggregate_returns_i64_3() {
    let stdout = "value=5\nvalue=18\nvalue=12\n";
    let extracts = vec![CommandExtract {
        pattern: r"value=(?P<val>\d+)".to_string(),
        signal_id: "test.count_signal".to_string(),
        unit: Unit::Count,
        aggregate: Aggregate::Count,
    }];
    let signals = extract_signals("test_cmd", stdout, &extracts);
    assert_eq!(signals.len(), 1, "expected exactly one signal");
    assert_eq!(signals[0].value, SignalValue::I64(3));
}
