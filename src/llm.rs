//! Schema-versioned LLM-friendly output (SDD Req 21).
//!
//! `LlmOutput` is a stable, self-describing JSON document that a Claude Code
//! skill (or any LLM) can consume without needing to understand internal
//! representation details. The `schema_version` field lets consumers refuse
//! analysis on unknown versions.

use serde::{Deserialize, Serialize};

use crate::analysis::AnalysisReport;
use crate::command::CommandResult;
use crate::finding::Finding;
use crate::redact::Redactor;
use crate::signal::{Signal, SignalValue};

/// One command's stdout, truncated to `MAX_EXCERPT_CHARS`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExcerpt {
    pub command: String,
    pub output: String,
}

/// Top-level schema-versioned document. `schema_version` is always `"1"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOutput {
    pub schema_version: String,
    pub date_time: String,
    pub host: LlmHost,
    pub signals: Vec<Signal>,
    pub findings: Vec<Finding>,
    pub checked_ok: Vec<String>,
    pub raw_excerpts: Vec<LlmExcerpt>,
}

/// Host metadata populated from `Context` and collector-emitted host signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmHost {
    pub hostname: String,
    // NOTE: field name "kernel" matches context.uname() — kept for schema stability
    pub kernel: String,
    pub cpu_count: usize,
    pub mem_total_bytes: u64,
    pub load_avg_1m: f64,
}

impl LlmOutput {
    pub const SCHEMA_VERSION: &'static str = "1";
    pub const MAX_EXCERPT_CHARS: usize = 1_000;

    /// Build an `LlmOutput` from an `AnalysisReport`.
    ///
    /// `hostname` and `kernel` come from `Context`. `cpu_count`,
    /// `mem_total_bytes`, and `load_avg_1m` are read from collector-emitted
    /// signals `host.cpu_count`, `host.mem_total_bytes`, `host.load_avg_1m`
    /// respectively (falling back to 0 if absent). If `redact` is true the
    /// output is HMAC-redacted before returning.
    pub fn from_report(report: &AnalysisReport, redact: bool) -> Self {
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

        let raw_excerpts: Vec<LlmExcerpt> = report
            .command_results()
            .first()
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .iter()
            .filter_map(|cr| match cr {
                CommandResult::Success { command, stdout, .. } => Some(LlmExcerpt {
                    command: command.name().to_string(),
                    output: stdout.chars().take(Self::MAX_EXCERPT_CHARS).collect(),
                }),
                _ => None,
            })
            .collect();

        let mut out = LlmOutput {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            date_time: report.context().date_time().to_rfc3339(),
            host,
            signals,
            findings: report.findings().to_vec(),
            checked_ok: report.checked_ok().to_vec(),
            raw_excerpts,
        };
        if redact {
            out = Redactor::from_env().redact_output(out);
        }
        out
    }
}
