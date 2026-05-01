//! Typed metric values produced by `Collector` implementations.
//!
//! A `Signal` is the unit of evidence the rule engine evaluates. Every signal
//! has a stable dotted ID (e.g. `cpu.iowait_pct`), a typed value, a unit, and
//! a timestamp. Sampled signals additionally carry per-sample values; baseline
//! annotation is added by the baseline subsystem (Phase 2).

use chrono::{DateTime, Local};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Signal {
    pub id: String,
    pub value: SignalValue,
    pub unit: Unit,
    pub at: DateTime<Local>,
    pub samples: Option<Vec<f64>>,
    pub baseline: Option<BaselineStats>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SignalValue {
    F64(f64),
    I64(i64),
    Bool(bool),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Unit {
    Pct,
    MillisPerOp,
    BytesPerSec,
    Count,
    Iops,
    Microseconds,
    Hz,
    Celsius,
    None,
}

#[derive(Debug, Clone, Serialize)]
pub struct BaselineStats {
    pub p50: f64,
    pub p95: f64,
    pub mad: f64,
    pub z_score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SampleStats {
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p95: f64,
    pub trend: Trend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Trend {
    Rising,
    Falling,
    Flat,
}

impl SignalValue {
    /// Coerce a numeric value to f64 for predicate comparisons. Bool and Text
    /// return `None`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            SignalValue::F64(v) => Some(*v),
            SignalValue::I64(v) => Some(*v as f64),
            _ => None,
        }
    }
}
