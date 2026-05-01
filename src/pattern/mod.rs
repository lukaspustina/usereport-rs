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
            let when = Predicate::parse(&def.when).map_err(|e| Error::Predicate { id: def.id.clone(), source: e })?;
            let severity = match def.severity.as_str() {
                "crit" => Severity::Crit,
                "warn" => Severity::Warn,
                _ => Severity::Info,
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
                let evidence = collect_evidence(signals);
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

fn collect_evidence(signals: &[Signal]) -> Vec<Evidence> {
    signals
        .iter()
        .map(|s| Evidence {
            signal_id: s.id.clone(),
            observed: s.value.clone(),
        })
        .collect()
}
