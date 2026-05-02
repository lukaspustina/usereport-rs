//! SDD more-useful-firefight Phase 5, C1.
//! GIVEN stdout "Out of memory: Killed process 123\n" and extract = [{pattern: "Out of memory:",
//!   signal_id: "dmesg.oom_count", unit: "count", aggregate: "count"}]
//! WHEN extract_signals is called
//! THEN it returns exactly one Signal with id = "dmesg.oom_count" and SignalValue::I64(1).

use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::signal::{SignalValue, Unit};

#[test]
fn count_oom_lines_returns_i64_1() {
    let stdout = "Out of memory: Killed process 123\n";
    let extracts = vec![CommandExtract {
        pattern: "Out of memory:".to_string(),
        signal_id: "dmesg.oom_count".to_string(),
        unit: Unit::Count,
        aggregate: Aggregate::Count,
    }];
    let signals = extract_signals("dmesg", stdout, &extracts);
    assert_eq!(signals.len(), 1, "expected exactly one signal");
    assert_eq!(signals[0].id, "dmesg.oom_count");
    assert_eq!(signals[0].value, SignalValue::I64(1));
}
