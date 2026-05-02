//! SDD more-useful-firefight Phase 5, C3.
//! GIVEN aggregate = "count" and a pattern that matches 3 lines
//! WHEN extract_signals is called
//! THEN the returned signal has SignalValue::I64(3).

use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::signal::{SignalValue, Unit};

#[test]
fn count_aggregate_counts_matching_lines() {
    let extracts = vec![CommandExtract {
        pattern: "ERROR".to_string(),
        signal_id: "test.error_count".to_string(),
        unit: Unit::Count,
        aggregate: Aggregate::Count,
    }];
    let stdout = "INFO: ok\nERROR: bad\nERROR: worse\nERROR: bad again\n";
    let signals = extract_signals("test_cmd", stdout, &extracts);
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].value, SignalValue::I64(3));
}
