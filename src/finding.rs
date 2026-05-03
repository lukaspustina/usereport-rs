//! Findings produced by the rule engine.
//!
//! A `Finding` is what the user actually reads in the `FINDINGS` section of
//! the report. Each carries the rule (or pattern) that fired, severity, the
//! supporting `Evidence`, and ordered next-step `suggest` commands.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::signal::SignalValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub kind: FindingKind,
    pub severity: Severity,
    pub summary: String,
    pub evidence: Vec<Evidence>,
    pub suggest: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingKind {
    Rule,
    Pattern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warn,
    Crit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub signal_id: String,
    pub observed: SignalValue,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_commands: Vec<String>,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Crit => write!(f, "CRIT"),
            Severity::Warn => write!(f, "WARN"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

impl Severity {
    pub fn rank(self) -> u8 {
        match self {
            Severity::Crit => 0,
            Severity::Warn => 1,
            Severity::Info => 2,
        }
    }
}

/// Threshold info for one signal, extracted from rule predicates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdInfo {
    pub severity: Severity,
    pub op: String,
    pub value: f64,
}

/// Sort findings in-place per SDD §101: severity Crit → Warn → Info, then
/// lexicographically by `Finding::id` within a severity. The sort is stable.
pub fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        let order = a.severity.rank().cmp(&b.severity.rank());
        if order == std::cmp::Ordering::Equal {
            a.id.cmp(&b.id)
        } else {
            order
        }
    });
}
