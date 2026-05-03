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
//! turns annotated z-scores into auto findings (`|z|>3` → warn, `|z|>6` → crit).

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

/// Per SDD §116: signals whose `|z_score| > 3` produce automatic warn findings;
/// `|z_score| > 6` produces crit findings. Findings cite the signal id.
pub fn outlier_findings(signals: &[Signal]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for sig in signals {
        let baseline = match &sig.baseline {
            Some(b) => b,
            None => continue,
        };
        let abs_z = baseline.z_score.abs();
        let severity = if abs_z > 6.0 {
            Severity::Crit
        } else if abs_z > 3.0 {
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
                "{} is {:.2} standard deviations from baseline (p50={:.3}, mad={:.3})",
                sig.id, baseline.z_score.abs(), baseline.p50, baseline.mad
            ),
            evidence,
            suggest: vec![],
        });
    }
    findings
}
