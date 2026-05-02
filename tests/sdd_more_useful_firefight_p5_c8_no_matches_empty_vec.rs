//! SDD more-useful-firefight Phase 5, C8.
//! GIVEN stdout with zero matching lines and aggregate = "max"
//! WHEN extract_signals is called
//! THEN it returns an empty vec and does not panic.

use usereport::cli::config::{Aggregate, CommandExtract};
use usereport::extract::extract_signals;
use usereport::signal::Unit;

#[test]
fn no_matching_lines_returns_empty_vec() {
    let extracts = vec![CommandExtract {
        pattern: r"(?P<val>\d+)\s+ms".to_string(),
        signal_id: "vmstat.wa_pct".to_string(),
        unit: Unit::Pct,
        aggregate: Aggregate::Max,
    }];

    let stdout = "this line has no numbers in the expected format\nanother line without match\n";
    let signals = extract_signals("vmstat", stdout, &extracts);

    assert!(
        signals.is_empty(),
        "expected empty vec when no lines match, got: {signals:?}"
    );
}
