//! Schema-versioned LLM-friendly output (SDD Req 21).
//!
//! `LlmOutput` is a stable, self-describing JSON document that a Claude Code
//! skill (or any LLM) can consume without needing to understand internal
//! representation details. The `schema_version` field lets consumers refuse
//! analysis on unknown versions.

use serde::{Deserialize, Serialize};

use crate::analysis::AnalysisReport;
use crate::finding::Finding;
use crate::signal::{Signal, SignalValue};

/// Top-level schema-versioned document. `schema_version` is always `"1"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOutput {
    pub schema_version: String,
    pub host: LlmHost,
    pub signals: Vec<Signal>,
    pub findings: Vec<Finding>,
    pub checked_ok: Vec<String>,
    pub raw_excerpts: Vec<String>,
}

/// Host metadata populated from `Context` and collector-emitted host signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmHost {
    pub hostname: String,
    pub kernel: String,
    pub cpu_count: usize,
    pub mem_total_bytes: u64,
    pub load_avg_1m: f64,
}

impl LlmOutput {
    pub const SCHEMA_VERSION: &'static str = "1";

    /// Build an `LlmOutput` from an `AnalysisReport`.
    ///
    /// `hostname` and `kernel` come from `Context`. `cpu_count`,
    /// `mem_total_bytes`, and `load_avg_1m` are read from collector-emitted
    /// signals `host.cpu_count`, `host.mem_total_bytes`, `host.load_avg_1m`
    /// respectively (falling back to 0 if absent).
    pub fn from_report(report: &AnalysisReport) -> Self {
        fn find_f64(signals: &[Signal], id: &str) -> f64 {
            signals
                .iter()
                .find(|s| s.id == id)
                .and_then(|s| match s.value {
                    SignalValue::F64(v) => Some(v),
                    SignalValue::I64(v) => Some(v as f64),
                    _ => None,
                })
                .unwrap_or(0.0)
        }

        let signals = report.signals().to_vec();
        let host = LlmHost {
            hostname: report.context().hostname().to_string(),
            kernel: report.context().uname().to_string(),
            cpu_count: find_f64(&signals, "host.cpu_count") as usize,
            mem_total_bytes: find_f64(&signals, "host.mem_total_bytes") as u64,
            load_avg_1m: find_f64(&signals, "host.load_avg_1m"),
        };

        LlmOutput {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            host,
            signals,
            findings: report.findings().to_vec(),
            checked_ok: report.checked_ok().to_vec(),
            raw_excerpts: Vec::new(),
        }
    }
}
