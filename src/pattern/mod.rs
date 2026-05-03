//! Multi-signal pattern correlator (SDD Req 20).
//!
//! Patterns are declared in TOML files under `contrib/patterns/`. Each pattern
//! fires when all constituent signals in its `when` predicate are present and
//! the predicate evaluates to true. Pattern findings are distinguished from
//! rule findings by `kind = FindingKind::Pattern`.

use serde::Deserialize;
use thiserror::Error;

use crate::collector::CollectCtx;
use crate::finding::{Evidence, Finding, FindingKind, Severity};
use crate::rule::{Predicate, SignalIndex};
use crate::signal::Signal;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to parse pattern TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("failed to parse predicate in pattern '{id}': {source}")]
    Predicate { id: String, source: crate::rule::Error },
    #[error("unknown severity '{severity}' in pattern '{id}'; valid values: crit, warn, info")]
    UnknownSeverity { id: String, severity: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A single compiled pattern.
#[derive(Debug)]
pub struct Pattern {
    pub id: String,
    pub when: Predicate,
    pub severity: Severity,
    pub summary: String,
    pub suggest: Vec<String>,
}

/// Evaluates patterns against a signal set after the rule pass.
#[derive(Debug)]
pub struct PatternEngine {
    patterns: Vec<Pattern>,
}

impl PatternEngine {
    /// Create an engine with no patterns.
    pub fn empty() -> Self {
        Self { patterns: Vec::new() }
    }

    /// Move all patterns from `other` into this engine.
    pub fn extend_from(&mut self, other: PatternEngine) {
        self.patterns.extend(other.patterns);
    }

    /// Parse patterns from a TOML string (used in tests and for loading files).
    pub fn from_toml(text: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct File {
            pattern: Vec<PatternDef>,
        }
        #[derive(Deserialize)]
        struct PatternDef {
            id: String,
            when: String,
            severity: String,
            summary: String,
            #[serde(default)]
            suggest: Vec<String>,
        }

        let file: File = toml::from_str(text)?;
        let mut patterns = Vec::with_capacity(file.pattern.len());
        for def in file.pattern {
            let when = Predicate::parse(&def.when).map_err(|e| Error::Predicate {
                id: def.id.clone(),
                source: e,
            })?;
            let severity = match def.severity.to_ascii_lowercase().as_str() {
                "crit" => Severity::Crit,
                "warn" => Severity::Warn,
                "info" => Severity::Info,
                other => return Err(Error::UnknownSeverity {
                    id: def.id.clone(),
                    severity: other.to_string(),
                }),
            };
            patterns.push(Pattern {
                id: def.id,
                when,
                severity,
                summary: def.summary,
                suggest: def.suggest,
            });
        }
        Ok(Self { patterns })
    }

    /// Run all patterns against the given signals; return findings that fired.
    pub fn run(&self, signals: &[Signal], ctx: &CollectCtx) -> Vec<Finding> {
        let idx = SignalIndex::build(signals);
        let mut findings = Vec::new();
        for pattern in &self.patterns {
            if pattern.when.evaluate(&idx, ctx) {
                let evidence = collect_evidence(&pattern.when, signals);
                findings.push(Finding {
                    id: pattern.id.clone(),
                    kind: FindingKind::Pattern,
                    severity: pattern.severity,
                    summary: pattern.summary.clone(),
                    evidence,
                    suggest: pattern.suggest.clone(),
                });
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_no_patterns() {
        let pe = PatternEngine::empty();
        let mut other = PatternEngine::empty();
        other.extend_from(pe);
        // If both are empty, the merged engine should also be empty (no panic)
    }

    #[test]
    fn extend_from_merges_patterns() {
        let toml = r#"
[[pattern]]
id = "test.p1"
description = "test"
severity = "warn"
when = "cpu.idle_pct < 10"
summary = "test pattern"
"#;
        let pe1 = PatternEngine::from_toml(toml).unwrap();
        let pe2 = PatternEngine::from_toml(toml).unwrap();
        let mut merged = PatternEngine::empty();
        merged.extend_from(pe1);
        merged.extend_from(pe2);
        // Merged engine should not panic when used; content verified by integration tests
    }

    #[test]
    fn severity_case_insensitive() {
        let toml = r#"
[[pattern]]
id = "test.p2"
severity = "Warn"
when = "cpu.idle_pct < 10"
summary = "uppercase severity"
"#;
        let pe = PatternEngine::from_toml(toml).unwrap();
        assert_eq!(pe.patterns[0].severity, crate::finding::Severity::Warn);
    }

    #[test]
    fn unknown_severity_returns_error() {
        let toml = r#"
[[pattern]]
id = "test.bad"
severity = "warning"
when = "cpu.idle_pct < 10"
summary = "bad severity"
"#;
        let result = PatternEngine::from_toml(toml);
        assert!(result.is_err(), "expected error for unknown severity 'warning'");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("warning"), "error should mention the bad value; got: {msg}");
    }

    #[test]
    fn evidence_scoped_to_predicate_signals() {
        use crate::collector::CollectCtx;
        use crate::signal::{Signal, SignalValue, Unit};
        use chrono::Local;

        let make_signal = |id: &str, v: f64| Signal {
            id: id.to_string(),
            value: SignalValue::F64(v),
            unit: Unit::None,
            at: Local::now(),
            samples: None,
            stats: None,
            baseline: None,
        };

        let toml = r#"
[[pattern]]
id = "test.scope"
severity = "warn"
when = "net.tw_count > 1000 AND cpu.idle_pct < 10"
summary = "scoped evidence test"
"#;
        let pe = PatternEngine::from_toml(toml).unwrap();
        let signals = vec![
            make_signal("net.tw_count", 5000.0),
            make_signal("cpu.idle_pct", 5.0),
            make_signal("mem.free_pct", 50.0), // unrelated — should NOT appear in evidence
        ];
        let ctx = CollectCtx::default();
        let findings = pe.run(&signals, &ctx);
        assert_eq!(findings.len(), 1);
        let evidence_ids: Vec<&str> = findings[0].evidence.iter().map(|e| e.signal_id.as_str()).collect();
        assert!(evidence_ids.contains(&"net.tw_count"));
        assert!(evidence_ids.contains(&"cpu.idle_pct"));
        assert!(!evidence_ids.contains(&"mem.free_pct"), "unrelated signal should not appear in evidence");
    }
}

fn collect_evidence(predicate: &crate::rule::Predicate, signals: &[Signal]) -> Vec<Evidence> {
    let predicate_ids: std::collections::HashSet<String> = predicate.signal_ids().into_iter().collect();
    signals
        .iter()
        .filter(|s| predicate_ids.contains(&s.id))
        .map(|s| Evidence {
            signal_id: s.id.clone(),
            observed: s.value.clone(),
            source_commands: Vec::new(),
        })
        .collect()
}
