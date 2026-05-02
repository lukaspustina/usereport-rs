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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::baseline::stats::sample_stats;
use crate::collector::CollectCtx;
use crate::finding::{Evidence, Finding, FindingKind, Severity, ThresholdInfo, sort_findings};
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
        expr.parse(input.trim()).map_err(|e| Error::Predicate(e.to_string()))
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
    if path[0] == "host" && path.len() == 2 && path[1] == "cpu_count" {
        return Some(LhsValue::Number(ctx.cpu_count as f64));
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

// --- Parser (winnow) ---------------------------------------------------------

use winnow::{
    ModalResult, Parser as _,
    ascii::{float, multispace0, multispace1},
    combinator::{alt, delimited, repeat, separated},
    token::take_while,
};

fn ident<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    take_while(1.., |c: char| c.is_alphanumeric() || c == '_')
        .verify(|s: &&str| s.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_'))
        .parse_next(input)
}

fn bare_ident<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    ident
        .verify(|s: &&str| !matches!(s.to_ascii_uppercase().as_str(), "AND" | "OR" | "TRUE" | "FALSE"))
        .parse_next(input)
}

fn path(input: &mut &str) -> ModalResult<Vec<String>> {
    separated(1.., bare_ident.map(|s: &str| s.to_string()), '.').parse_next(input)
}

fn cmp_op(input: &mut &str) -> ModalResult<Op> {
    alt((
        ">=".value(Op::Ge),
        "<=".value(Op::Le),
        "==".value(Op::Eq),
        "!=".value(Op::Ne),
        ">".value(Op::Gt),
        "<".value(Op::Lt),
    ))
    .parse_next(input)
}

fn bool_literal(input: &mut &str) -> ModalResult<bool> {
    ident
        .verify_map(|s: &str| match s.to_ascii_uppercase().as_str() {
            "TRUE" => Some(true),
            "FALSE" => Some(false),
            _ => None,
        })
        .parse_next(input)
}

fn quoted_string(input: &mut &str) -> ModalResult<String> {
    alt((
        delimited('"', take_while(0.., |c: char| c != '"'), '"'),
        delimited('\'', take_while(0.., |c: char| c != '\''), '\''),
    ))
    .map(|s: &str| s.to_string())
    .parse_next(input)
}

fn number(input: &mut &str) -> ModalResult<f64> {
    float(input)
}

fn rhs(input: &mut &str) -> ModalResult<Rhs> {
    alt((
        bool_literal.map(|b| Rhs::Value(Value::Bool(b))),
        number.map(|n| Rhs::Value(Value::Number(n))),
        quoted_string.map(|s| Rhs::Value(Value::Str(s))),
        path.map(Rhs::Path),
    ))
    .parse_next(input)
}

fn term(input: &mut &str) -> ModalResult<Predicate> {
    let (lhs, _, op, _, rhs) = (path, multispace0, cmp_op, multispace0, rhs).parse_next(input)?;
    Ok(Predicate::Cmp { path: lhs, op, rhs })
}

fn infix_op(input: &mut &str) -> ModalResult<bool> {
    delimited(
        multispace1,
        ident.verify_map(|s: &str| match s.to_ascii_uppercase().as_str() {
            "AND" => Some(true),
            "OR" => Some(false),
            _ => None,
        }),
        multispace1,
    )
    .parse_next(input)
}

fn expr(input: &mut &str) -> ModalResult<Predicate> {
    let first = term.parse_next(input)?;
    let rest: Vec<(bool, Predicate)> = repeat(0.., (infix_op, term)).parse_next(input)?;
    Ok(rest.into_iter().fold(first, |left, (is_and, right)| {
        if is_and {
            Predicate::And(Box::new(left), Box::new(right))
        } else {
            Predicate::Or(Box::new(left), Box::new(right))
        }
    }))
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

    pub fn run(
        &self,
        signals: &[Signal],
        ctx: &CollectCtx,
        source_map: &HashMap<String, Vec<String>>,
    ) -> (Vec<Finding>, Vec<String>) {
        let index = SignalIndex::build(signals);
        let mut findings = Vec::new();
        let mut checked_ok: Vec<String> = Vec::new();
        for rule in &self.rules {
            if rule.when.evaluate(&index, ctx) {
                let evidence = rule
                    .evidence_ids
                    .iter()
                    .filter_map(|sid| evidence_for(sid, &index, ctx, source_map))
                    .collect();
                findings.push(Finding {
                    id: rule.id.clone(),
                    kind: FindingKind::Rule,
                    severity: rule.severity,
                    summary: rule.summary.clone(),
                    evidence,
                    suggest: rule.suggest.clone(),
                });
            } else {
                for sid in &rule.evidence_ids {
                    if !checked_ok.contains(sid) {
                        checked_ok.push(sid.clone());
                    }
                }
            }
        }
        sort_findings(&mut findings);
        checked_ok.sort();
        (findings, checked_ok)
    }

    pub fn signal_thresholds(&self) -> HashMap<String, ThresholdInfo> {
        let mut map = HashMap::new();
        for rule in &self.rules {
            extract_cmp_thresholds(&rule.when, rule.severity, &mut map);
        }
        map
    }
}

fn extract_cmp_thresholds(pred: &Predicate, severity: Severity, map: &mut HashMap<String, ThresholdInfo>) {
    match pred {
        Predicate::Cmp { path, op, rhs } => {
            if let Rhs::Value(Value::Number(v)) = rhs {
                let signal_id = path.join(".");
                map.entry(signal_id).or_insert(ThresholdInfo {
                    severity,
                    op: op_to_str(*op).to_string(),
                    value: *v,
                });
            }
        }
        Predicate::And(a, b) | Predicate::Or(a, b) => {
            extract_cmp_thresholds(a, severity, map);
            extract_cmp_thresholds(b, severity, map);
        }
    }
}

fn op_to_str(op: Op) -> &'static str {
    match op {
        Op::Gt => ">",
        Op::Lt => "<",
        Op::Ge => ">=",
        Op::Le => "<=",
        Op::Eq => "==",
        Op::Ne => "!=",
    }
}

/// Look up the observed value for an `evidence_ids` entry. Resolves both
/// signal IDs and `host.*` paths (which come from `CollectCtx`). Entries that
/// resolve to nothing are silently skipped.
fn evidence_for(
    id: &str,
    index: &SignalIndex<'_>,
    ctx: &CollectCtx,
    source_map: &HashMap<String, Vec<String>>,
) -> Option<Evidence> {
    let source_commands = source_map.get(id).cloned().unwrap_or_default();
    if let Some(signal) = index.get(id) {
        return Some(Evidence {
            signal_id: id.to_string(),
            observed: signal.value.clone(),
            source_commands,
        });
    }
    if id == "host.cpu_count" {
        return Some(Evidence {
            signal_id: id.to_string(),
            observed: SignalValue::I64(ctx.cpu_count as i64),
            source_commands,
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
        let (findings, _) = engine.run(&signals, &ctx(), &HashMap::new());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "compound.test");
    }
}
