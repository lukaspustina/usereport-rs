//! Baselines + outlier detection (SDD §113–§116).
//!
//! Two flavours of baseline:
//!   - **Named** (`record(name)` → `<dir>/<name>.json`): a snapshot the user
//!     captures explicitly with `usereport baseline record --name green`.
//!   - **Rolling** (`<dir>/_rolling.jsonl`): a sliding window of the last
//!     `baseline_rolling_n` runs, appended automatically.
//!
//! `annotate(&mut signals, &records)` populates `Signal.baseline` with p50,
//! p95, MAD, and z_score derived from the records. `outlier_findings(&signals)`
//! turns annotated z-scores into auto findings (`|z|>Z_WARN_THRESHOLD` → warn,
//! `|z|>Z_CRIT_THRESHOLD` → crit).

// Modified z-score thresholds per Iglewicz & Hoaglin (1993)
const Z_WARN_THRESHOLD: f64 = 3.5;
const Z_CRIT_THRESHOLD: f64 = 7.0;

use crate::finding::{Evidence, Finding, FindingKind, Severity};
use crate::signal::{BaselineStats, Signal, SignalValue};

pub mod stats;
pub mod store;

pub use stats::{mad, median, percentile, z_score};
pub use store::{BaselineRecord, BaselineStore};

/// Annotate each signal in-place with `BaselineStats` derived from the given
/// rolling/named records. Signals not present in any record are left
/// unannotated.
pub fn annotate(signals: &mut [Signal], records: &[BaselineRecord]) {
    if records.is_empty() {
        return;
    }
    for sig in signals.iter_mut() {
        let mut history: Vec<f64> = Vec::with_capacity(records.len());
        for r in records {
            if let Some(v) = r.signals.get(&sig.id) {
                history.push(*v);
            }
        }
        if history.is_empty() {
            continue;
        }
        let p50 = median(&history).unwrap_or(0.0);
        let p95 = percentile(&history, 95.0).unwrap_or(p50);
        let m = mad(&history).unwrap_or(0.0);
        let observed = match &sig.value {
            SignalValue::F64(v) => *v,
            SignalValue::I64(v) => *v as f64,
            _ => continue,
        };
        let z = z_score(observed, p50, m);
        sig.baseline = Some(BaselineStats {
            p50,
            p95,
            mad: m,
            z_score: z,
        });
    }
}

/// Per SDD §116: signals whose `|z_score| > Z_WARN_THRESHOLD` produce automatic
/// warn findings; `|z_score| > Z_CRIT_THRESHOLD` produces crit findings.
/// Findings cite the signal id.
pub fn outlier_findings(signals: &[Signal]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for sig in signals {
        let baseline = match &sig.baseline {
            Some(b) => b,
            None => continue,
        };
        let abs_z = baseline.z_score.abs();
        let severity = if abs_z > Z_CRIT_THRESHOLD {
            Severity::Crit
        } else if abs_z > Z_WARN_THRESHOLD {
            Severity::Warn
        } else {
            continue;
        };
        let evidence = vec![Evidence {
            signal_id: sig.id.clone(),
            observed: sig.value.clone(),
            source_commands: Vec::new(),
        }];
        findings.push(Finding {
            id: format!("baseline.outlier.{}", sig.id),
            kind: FindingKind::Rule,
            severity,
            summary: format!(
                "{} is significantly outside its normal range (z-score: {:.2}, baseline median: {:.3})",
                sig.id,
                baseline.z_score.abs(),
                baseline.p50
            ),
            evidence,
            suggest: vec![
                format!(
                    "Check whether recent changes, deployments, or configuration changes explain the anomaly in '{}'.",
                    sig.id
                ),
                "Run 'usereport baseline record' after verifying the system is healthy to update the baseline."
                    .to_string(),
            ],
        });
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{BaselineStats, Signal, SignalValue, Unit};
    use chrono::Local;

    fn signal_with_z(id: &str, z: f64) -> Signal {
        let mut s = Signal {
            id: id.to_string(),
            value: SignalValue::F64(1.0),
            unit: Unit::None,
            at: Local::now(),
            samples: None,
            stats: None,
            baseline: None,
        };
        s.baseline = Some(BaselineStats { p50: 1.0, p95: 2.0, mad: 0.1, z_score: z });
        s
    }

    #[test]
    fn z_below_warn_threshold_no_finding() {
        assert!(outlier_findings(&[signal_with_z("x", 3.49)]).is_empty());
    }

    #[test]
    fn z_exactly_warn_threshold_no_finding() {
        // boundary: > Z_WARN_THRESHOLD (3.5), not >=
        assert!(outlier_findings(&[signal_with_z("x", 3.5)]).is_empty());
    }

    #[test]
    fn z_above_warn_threshold_fires_warn() {
        let findings = outlier_findings(&[signal_with_z("x", 3.51)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warn);
    }

    #[test]
    fn z_exactly_crit_threshold_fires_warn_not_crit() {
        // boundary: > Z_CRIT_THRESHOLD (7.0), not >=
        let findings = outlier_findings(&[signal_with_z("x", 7.0)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warn, "z=7.0 should be Warn, not Crit");
    }

    #[test]
    fn z_above_crit_threshold_fires_crit() {
        let findings = outlier_findings(&[signal_with_z("x", 7.01)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Crit);
    }

    #[test]
    fn negative_z_above_warn_threshold_fires_warn() {
        let findings = outlier_findings(&[signal_with_z("x", -4.0)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warn);
    }

    #[test]
    fn finding_has_suggest_and_natural_summary() {
        let findings = outlier_findings(&[signal_with_z("my.signal", 4.0)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].suggest.len(), 2);
        assert!(findings[0].suggest[0].contains("my.signal"));
        assert!(findings[0].summary.contains("my.signal"));
        assert!(findings[0].summary.contains("z-score"));
        assert!(findings[0].summary.contains("baseline median"));
    }
}
