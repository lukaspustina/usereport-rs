//! Declarative rules and predicate DSL.
//!
//! The predicate grammar (per SDD §333):
//!
//! ```text
//! expr   ::= term (("AND" | "OR") term)*
//! term   ::= path op value
//! path   ::= IDENT ("." IDENT)*
//! op     ::= ">" | "<" | ">=" | "<=" | "==" | "!="
//! value  ::= NUMBER | BOOL | STRING
//! ```
//!
//! Phase 1 supports bare signal paths and `host.*` paths. SampleStats suffixes
//! (`.p50`, `.p95`, ...) and z-score paths are deferred to Phase 4.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::baseline::stats::sample_stats;
use crate::collector::CollectCtx;
use crate::finding::{sort_findings, Evidence, Finding, FindingKind, Severity};
use crate::signal::{Signal, SignalValue, Trend};

pub mod builtin;

#[derive(Debug, Error)]
pub enum Error {
    #[error("predicate parse error: {0}")]
    Predicate(String),
    #[error("failed to read rules directory {path}: {source}")]
    ReadDir { path: PathBuf, source: std::io::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub when: Predicate,
    pub severity: Severity,
    pub summary: String,
    pub evidence_ids: Vec<String>,
    pub suggest: Vec<String>,
    pub description: Option<String>,
    pub links: Vec<String>,
}

// --- Predicate AST -----------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    Cmp { path: Vec<String>, op: Op, rhs: Rhs },
    And(Box<Predicate>, Box<Predicate>),
    Or(Box<Predicate>, Box<Predicate>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Rhs {
    Value(Value),
    Path(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Str(String),
}

impl Predicate {
    pub fn parse(input: &str) -> Result<Self> {
        let tokens = tokenize(input).map_err(Error::Predicate)?;
        let mut parser = Parser { tokens, pos: 0 };
        let p = parser.parse_expr()?;
        if parser.pos != parser.tokens.len() {
            return Err(Error::Predicate(format!(
                "trailing tokens at position {}: {:?}",
                parser.pos,
                &parser.tokens[parser.pos..]
            )));
        }
        Ok(p)
    }

    pub fn evaluate(&self, signals_index: &SignalIndex<'_>, ctx: &CollectCtx) -> bool {
        match self {
            Predicate::And(a, b) => a.evaluate(signals_index, ctx) && b.evaluate(signals_index, ctx),
            Predicate::Or(a, b) => a.evaluate(signals_index, ctx) || b.evaluate(signals_index, ctx),
            Predicate::Cmp { path, op, rhs } => evaluate_cmp(path, *op, rhs, signals_index, ctx),
        }
    }
}

fn evaluate_cmp(path: &[String], op: Op, rhs: &Rhs, signals_index: &SignalIndex<'_>, ctx: &CollectCtx) -> bool {
    let lhs = resolve_path(path, signals_index, ctx);
    let rhs_resolved = match rhs {
        Rhs::Value(v) => match v {
            Value::Number(n) => Some(LhsValue::Number(*n)),
            Value::Bool(b) => Some(LhsValue::Bool(*b)),
            Value::Str(s) => Some(LhsValue::Text(s.clone())),
        },
        Rhs::Path(p) => resolve_path(p, signals_index, ctx),
    };
    match (lhs, rhs_resolved) {
        (Some(LhsValue::Number(n)), Some(LhsValue::Number(m))) => match op {
            Op::Gt => n > m,
            Op::Lt => n < m,
            Op::Ge => n >= m,
            Op::Le => n <= m,
            Op::Eq => (n - m).abs() < f64::EPSILON,
            Op::Ne => (n - m).abs() >= f64::EPSILON,
        },
        (Some(LhsValue::Bool(b)), Some(LhsValue::Bool(c))) => match op {
            Op::Eq => b == c,
            Op::Ne => b != c,
            _ => false,
        },
        (Some(LhsValue::Text(t)), Some(LhsValue::Text(s))) => match op {
            Op::Eq => t == s,
            Op::Ne => t != s,
            _ => false,
        },
        // Absent or type-mismatched signals never match (SDD §453).
        _ => false,
    }
}

enum LhsValue {
    Number(f64),
    Bool(bool),
    Text(String),
}

fn resolve_path(path: &[String], signals_index: &SignalIndex<'_>, ctx: &CollectCtx) -> Option<LhsValue> {
    if path.is_empty() {
        return None;
    }
    if path[0] == "host" {
        if path.len() == 2 && path[1] == "cpu_count" {
            return Some(LhsValue::Number(ctx.cpu_count as f64));
        }
        return None;
    }

    // Check for SampleStats suffixes (.p50, .p95, .p99, .min, .max, .trend).
    // Only intercept when: (a) the last segment is a known suffix, (b) the
    // prefix path resolves to an actual signal that has samples. If either
    // check fails we fall through to a bare signal lookup, avoiding collisions
    // with signals whose IDs happen to end in a reserved word (e.g. "disk.max").
    let sample_suffix = path.last().map(|s| s.as_str());
    let is_sample_suffix = matches!(sample_suffix, Some("p50" | "p95" | "p99" | "min" | "max" | "trend"));
    if is_sample_suffix && path.len() >= 2 {
        let signal_id = path[..path.len() - 1].join(".");
        if let Some(signal) = signals_index.get(&signal_id) {
            if let Some(samples) = signal.samples.as_deref() {
                if let Some(stats) = sample_stats(samples) {
                    return match sample_suffix.unwrap() {
                        "p50" => Some(LhsValue::Number(stats.p50)),
                        "p95" => Some(LhsValue::Number(stats.p95)),
                        "p99" => Some(LhsValue::Number(stats.p99)),
                        "min" => Some(LhsValue::Number(stats.min)),
                        "max" => Some(LhsValue::Number(stats.max)),
                        "trend" => {
                            let s = match stats.trend {
                                Trend::Rising => "rising",
                                Trend::Falling => "falling",
                                Trend::Flat => "flat",
                            };
                            Some(LhsValue::Text(s.to_string()))
                        }
                        _ => None,
                    };
                }
            }
        }
        // prefix signal not found or has no samples — fall through to bare lookup.
    }

    let id = path.join(".");
    let signal = signals_index.get(&id)?;

    // Bare signal ID with samples: use SampleStats::p50 instead of Signal::value
    // so that rules comparing a sampled signal see the representative value.
    if let Some(samples) = signal.samples.as_deref() {
        if let Some(stats) = sample_stats(samples) {
            return Some(LhsValue::Number(stats.p50));
        }
    }

    match &signal.value {
        SignalValue::F64(v) => Some(LhsValue::Number(*v)),
        SignalValue::I64(v) => Some(LhsValue::Number(*v as f64)),
        SignalValue::Bool(v) => Some(LhsValue::Bool(*v)),
        SignalValue::Text(v) => Some(LhsValue::Text(v.clone())),
    }
}

pub struct SignalIndex<'a> {
    by_id: std::collections::HashMap<&'a str, &'a Signal>,
}

impl<'a> SignalIndex<'a> {
    pub fn build(signals: &'a [Signal]) -> Self {
        SignalIndex {
            by_id: signals.iter().map(|s| (s.id.as_str(), s)).collect(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&&'a Signal> {
        self.by_id.get(id)
    }
}

// --- Tokenizer + Parser ------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Number(f64),
    Str(String),
    Bool(bool),
    Op(Op),
    And,
    Or,
    Dot,
    LParen,
    RParen,
}

fn tokenize(s: &str) -> std::result::Result<Vec<Tok>, String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut tokens = Vec::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '.' {
            tokens.push(Tok::Dot);
            i += 1;
            continue;
        }
        if c == '(' {
            tokens.push(Tok::LParen);
            i += 1;
            continue;
        }
        if c == ')' {
            tokens.push(Tok::RParen);
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            let start = i + 1;
            i += 1;
            while i < bytes.len() && bytes[i] as char != quote {
                i += 1;
            }
            if i >= bytes.len() {
                return Err("unterminated string literal".into());
            }
            let lit = std::str::from_utf8(&bytes[start..i]).map_err(|e| e.to_string())?;
            tokens.push(Tok::Str(lit.to_string()));
            i += 1; // skip closing quote
            continue;
        }
        if c.is_ascii_digit() || (c == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit()) {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let cc = bytes[i] as char;
                if cc.is_ascii_digit() || cc == '.' {
                    i += 1;
                } else {
                    break;
                }
            }
            let lit = std::str::from_utf8(&bytes[start..i]).map_err(|e| e.to_string())?;
            let n: f64 = lit.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
            tokens.push(Tok::Number(n));
            continue;
        }
        if c == '>' || c == '<' || c == '=' || c == '!' {
            let next = bytes.get(i + 1).map(|&b| b as char);
            let op = match (c, next) {
                ('>', Some('=')) => {
                    i += 2;
                    Op::Ge
                }
                ('<', Some('=')) => {
                    i += 2;
                    Op::Le
                }
                ('=', Some('=')) => {
                    i += 2;
                    Op::Eq
                }
                ('!', Some('=')) => {
                    i += 2;
                    Op::Ne
                }
                ('>', _) => {
                    i += 1;
                    Op::Gt
                }
                ('<', _) => {
                    i += 1;
                    Op::Lt
                }
                _ => return Err(format!("unexpected character at {}: {:?}", i, c)),
            };
            tokens.push(Tok::Op(op));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() {
                let cc = bytes[i] as char;
                if cc.is_alphanumeric() || cc == '_' {
                    i += 1;
                } else {
                    break;
                }
            }
            let lit = std::str::from_utf8(&bytes[start..i]).map_err(|e| e.to_string())?;
            let upper = lit.to_ascii_uppercase();
            let tok = match upper.as_str() {
                "AND" => Tok::And,
                "OR" => Tok::Or,
                "TRUE" => Tok::Bool(true),
                "FALSE" => Tok::Bool(false),
                _ => Tok::Ident(lit.to_string()),
            };
            tokens.push(tok);
            continue;
        }
        return Err(format!("unexpected character {:?} at position {}", c, i));
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn parse_expr(&mut self) -> Result<Predicate> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.tokens.get(self.pos) {
                Some(Tok::And) => "AND",
                Some(Tok::Or) => "OR",
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_term()?;
            left = if op == "AND" {
                Predicate::And(Box::new(left), Box::new(right))
            } else {
                Predicate::Or(Box::new(left), Box::new(right))
            };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Predicate> {
        let path = self.parse_path()?;
        let op = match self.next_owned() {
            Some(Tok::Op(o)) => o,
            other => {
                return Err(Error::Predicate(format!(
                    "expected comparison operator, got {:?}",
                    other
                )))
            }
        };
        let rhs = match self.tokens.get(self.pos).cloned() {
            Some(Tok::Number(n)) => {
                self.pos += 1;
                Rhs::Value(Value::Number(n))
            }
            Some(Tok::Bool(b)) => {
                self.pos += 1;
                Rhs::Value(Value::Bool(b))
            }
            Some(Tok::Str(s)) => {
                self.pos += 1;
                Rhs::Value(Value::Str(s))
            }
            Some(Tok::Ident(_)) => {
                // Path-on-RHS: e.g. `vmstat.r > host.cpu_count`.
                let path = self.parse_path()?;
                Rhs::Path(path)
            }
            other => return Err(Error::Predicate(format!("expected value or path, got {:?}", other))),
        };
        Ok(Predicate::Cmp { path, op, rhs })
    }

    fn parse_path(&mut self) -> Result<Vec<String>> {
        let mut segments = Vec::new();
        match self.next_owned() {
            Some(Tok::Ident(s)) => segments.push(s),
            other => {
                return Err(Error::Predicate(format!(
                    "expected identifier at start of path, got {:?}",
                    other
                )))
            }
        }
        while matches!(self.tokens.get(self.pos), Some(Tok::Dot)) {
            self.pos += 1;
            match self.next_owned() {
                Some(Tok::Ident(s)) => segments.push(s),
                other => {
                    return Err(Error::Predicate(format!(
                        "expected identifier after '.', got {:?}",
                        other
                    )))
                }
            }
        }
        Ok(segments)
    }

    fn next_owned(&mut self) -> Option<Tok> {
        let t = self.tokens.get(self.pos)?.clone();
        self.pos += 1;
        Some(t)
    }
}

// --- Rule engine -------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        RuleEngine { rules }
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn run(&self, signals: &[Signal], ctx: &CollectCtx) -> Vec<Finding> {
        let index = SignalIndex::build(signals);
        let mut findings = Vec::new();
        for rule in &self.rules {
            if rule.when.evaluate(&index, ctx) {
                let evidence = rule
                    .evidence_ids
                    .iter()
                    .filter_map(|sid| evidence_for(sid, &index, ctx))
                    .collect();
                findings.push(Finding {
                    id: rule.id.clone(),
                    kind: FindingKind::Rule,
                    severity: rule.severity,
                    summary: rule.summary.clone(),
                    evidence,
                    suggest: rule.suggest.clone(),
                });
            }
        }
        sort_findings(&mut findings);
        findings
    }
}

/// Look up the observed value for an `evidence_ids` entry. Resolves both
/// signal IDs and `host.*` paths (which come from `CollectCtx`). Entries that
/// resolve to nothing are silently skipped.
fn evidence_for(id: &str, index: &SignalIndex<'_>, ctx: &CollectCtx) -> Option<Evidence> {
    if let Some(signal) = index.get(id) {
        return Some(Evidence {
            signal_id: id.to_string(),
            observed: signal.value.clone(),
        });
    }
    if id == "host.cpu_count" {
        return Some(Evidence {
            signal_id: id.to_string(),
            observed: SignalValue::I64(ctx.cpu_count as i64),
        });
    }
    None
}

// --- TOML loader -------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RuleFile {
    #[serde(default)]
    rule: Vec<RuleToml>,
}

#[derive(Debug, Deserialize)]
struct RuleToml {
    id: String,
    when: String,
    severity: String,
    summary: String,
    #[serde(default)]
    evidence: Vec<String>,
    #[serde(default)]
    suggest: Vec<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    links: Vec<String>,
}

impl RuleToml {
    fn into_rule(self) -> Result<Rule> {
        let when = Predicate::parse(&self.when).map_err(|e| Error::Predicate(format!("rule '{}': {}", self.id, e)))?;
        let severity =
            parse_severity(&self.severity).map_err(|e| Error::Predicate(format!("rule '{}': {}", self.id, e)))?;
        Ok(Rule {
            id: self.id,
            when,
            severity,
            summary: self.summary,
            evidence_ids: self.evidence,
            suggest: self.suggest,
            description: self.description,
            links: self.links,
        })
    }
}

fn parse_severity(s: &str) -> std::result::Result<Severity, String> {
    match s.to_ascii_lowercase().as_str() {
        "info" => Ok(Severity::Info),
        "warn" => Ok(Severity::Warn),
        "crit" => Ok(Severity::Crit),
        other => Err(format!("unknown severity: {}", other)),
    }
}

/// Parse a TOML rule-file string into a `Vec<Rule>`. Per-rule predicate or
/// severity errors are returned as the first failure; the caller (see
/// `RulesLoader`) is responsible for converting them into `warn` findings so
/// one bad file does not poison the rest.
pub fn parse_rules_toml(s: &str) -> Result<Vec<Rule>> {
    let parsed: RuleFile = toml::from_str(s).map_err(|e| Error::Predicate(e.to_string()))?;
    parsed.rule.into_iter().map(|r| r.into_rule()).collect()
}

#[derive(Debug, Default)]
pub struct RulesLoader {
    builtins: Vec<Rule>,
    user_dir: Option<PathBuf>,
}

#[derive(Debug)]
pub struct RulesLoadResult {
    pub rules: Vec<Rule>,
    pub load_findings: Vec<Finding>,
}

impl RulesLoader {
    pub fn new() -> Self {
        RulesLoader::default()
    }

    pub fn with_builtins(mut self, rules: Vec<Rule>) -> Self {
        self.builtins = rules;
        self
    }

    pub fn with_user_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.user_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Load built-in rules first, then user rules from the configured dir
    /// (if any). Per SDD §89: malformed user files do NOT poison built-ins;
    /// each malformed file becomes a `warn` finding.
    pub fn load(self) -> RulesLoadResult {
        let mut rules = self.builtins;
        let mut load_findings = Vec::new();
        if let Some(dir) = self.user_dir {
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut paths: Vec<PathBuf> = entries
                        .filter_map(|e| e.ok().map(|e| e.path()))
                        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("toml"))
                        .collect();
                    paths.sort();
                    for path in paths {
                        match std::fs::read_to_string(&path) {
                            Ok(content) => match parse_rules_toml(&content) {
                                Ok(mut more) => rules.append(&mut more),
                                Err(e) => load_findings.push(Finding {
                                    id: "rules.malformed_user_file".to_string(),
                                    kind: FindingKind::Rule,
                                    severity: Severity::Warn,
                                    summary: format!("skipped malformed rule file {}: {}", path.display(), e),
                                    evidence: vec![],
                                    suggest: vec![],
                                }),
                            },
                            Err(e) => load_findings.push(Finding {
                                id: "rules.user_file_unreadable".to_string(),
                                kind: FindingKind::Rule,
                                severity: Severity::Warn,
                                summary: format!("skipped unreadable rule file {}: {}", path.display(), e),
                                evidence: vec![],
                                suggest: vec![],
                            }),
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Absent directory is not an error.
                }
                Err(e) => load_findings.push(Finding {
                    id: "rules.user_dir_unreadable".to_string(),
                    kind: FindingKind::Rule,
                    severity: Severity::Warn,
                    summary: format!("could not read rules directory {}: {}", dir.display(), e),
                    evidence: vec![],
                    suggest: vec![],
                }),
            }
        }
        RulesLoadResult { rules, load_findings }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;
    use crate::signal::{Signal, SignalValue, Unit};

    fn ctx() -> CollectCtx {
        CollectCtx {
            duration: None,
            interval: None,
            cgroup_path: None,
            baseline: None,
            cpu_count: 4,
        }
    }

    fn signal(id: &str, v: f64) -> Signal {
        Signal {
            id: id.to_string(),
            value: SignalValue::F64(v),
            unit: Unit::None,
            at: Local::now(),
            samples: None,
            stats: None,
            baseline: None,
        }
    }

    #[test]
    fn predicate_and_both_terms_true_evaluates_true() {
        let p = Predicate::parse("a > 1 AND b > 2").expect("parse");
        let signals = vec![signal("a", 5.0), signal("b", 5.0)];
        let idx = SignalIndex::build(&signals);
        assert!(p.evaluate(&idx, &ctx()));
    }

    #[test]
    fn predicate_and_one_false_evaluates_false() {
        let p = Predicate::parse("a > 1 AND b > 2").expect("parse");
        let signals = vec![signal("a", 5.0), signal("b", 1.0)];
        let idx = SignalIndex::build(&signals);
        assert!(!p.evaluate(&idx, &ctx()));
    }

    #[test]
    fn predicate_or_one_true_evaluates_true() {
        let p = Predicate::parse("a > 100 OR b > 2").expect("parse");
        let signals = vec![signal("a", 5.0), signal("b", 5.0)];
        let idx = SignalIndex::build(&signals);
        assert!(p.evaluate(&idx, &ctx()));
    }

    #[test]
    fn predicate_or_both_false_evaluates_false() {
        let p = Predicate::parse("a > 100 OR b > 100").expect("parse");
        let signals = vec![signal("a", 5.0), signal("b", 5.0)];
        let idx = SignalIndex::build(&signals);
        assert!(!p.evaluate(&idx, &ctx()));
    }

    #[test]
    fn predicate_and_or_chained_left_associative() {
        // Per SDD §333: expr ::= term (("AND" | "OR") term)* — left-associative.
        // `a > 1 AND b > 1 OR c > 100` parses as `(a > 1 AND b > 1) OR c > 100`.
        // a=5, b=5, c=5 → (true AND true) OR false = true.
        let p = Predicate::parse("a > 1 AND b > 1 OR c > 100").expect("parse");
        let signals = vec![signal("a", 5.0), signal("b", 5.0), signal("c", 5.0)];
        let idx = SignalIndex::build(&signals);
        assert!(p.evaluate(&idx, &ctx()));
    }

    #[test]
    fn rule_engine_fires_compound_and_predicate() {
        // End-to-end: a Rule with an AND predicate produces a Finding when
        // both terms are true.
        let rule = Rule {
            id: "compound.test".to_string(),
            when: Predicate::parse("a > 1 AND b > 1").expect("parse"),
            severity: Severity::Warn,
            summary: "compound rule fired".to_string(),
            evidence_ids: vec!["a".to_string(), "b".to_string()],
            suggest: vec![],
            description: None,
            links: vec![],
        };
        let signals = vec![signal("a", 5.0), signal("b", 5.0)];
        let engine = RuleEngine::new(vec![rule]);
        let findings = engine.run(&signals, &ctx());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "compound.test");
    }
}
