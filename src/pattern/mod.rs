//! Multi-signal pattern correlator (SDD Req 20).
//!
//! Patterns are declared in TOML files under `contrib/patterns/`. A pattern
//! fires when its `when` predicate evaluates to true. AND predicates require
//! all constituent signals to be present (a missing signal makes the predicate
//! false); OR predicates can fire on a subset of signals — if any branch
//! evaluates to true the pattern fires. Pattern findings are distinguished from
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
                other => {
                    return Err(Error::Predicate {
                        id: def.id.clone(),
                        source: crate::rule::Error::Predicate(format!("unknown severity: {}", other)),
                    });
                }
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
                let referenced: std::collections::HashSet<String> =
                    pattern.when.signal_ids().into_iter().collect();
                let evidence = collect_evidence(signals, &referenced);
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
        let signals = vec![Signal {
            id: "cpu.idle_pct".to_string(),
            value: crate::signal::SignalValue::F64(5.0),
            unit: crate::signal::Unit::Pct,
            at: chrono::Local::now(),
            samples: None,
            stats: None,
            baseline: None,
        }];
        let ctx = crate::collector::CollectCtx::default();
        let findings = merged.run(&signals, &ctx);
        assert_eq!(findings.len(), 2, "extend_from should merge all patterns from both engines");
    }
}

fn collect_evidence(signals: &[Signal], referenced: &std::collections::HashSet<String>) -> Vec<Evidence> {
    signals
        .iter()
        .filter(|s| referenced.contains(&s.id))
        .map(|s| Evidence {
            signal_id: s.id.clone(),
            observed: s.value.clone(),
            source_commands: Vec::new(),
        })
        .collect()
}
