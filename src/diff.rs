//! `usereport diff <a.json> <b.json>` — compare two `AnalysisReport`s.
//!
//! SDD §117–§118: per-signal deltas plus three finding sections (only-in-a,
//! only-in-b, severity-changed). Default text output; `--output json`
//! re-uses `serde_json` over the `DiffReport` struct.

use std::collections::HashMap;

use serde::Serialize;

use crate::analysis::AnalysisReport;
use crate::finding::{Finding, Severity};
use crate::signal::SignalValue;

#[derive(Debug, Serialize)]
pub struct SignalDelta {
    pub signal_id: String,
    pub value_in_a: f64,
    pub value_in_b: f64,
    pub delta: f64,
}

#[derive(Debug, Serialize)]
pub struct SeverityChange {
    pub finding_id: String,
    pub severity_in_a: Severity,
    pub severity_in_b: Severity,
}

#[derive(Debug, Default, Serialize)]
pub struct DiffReport {
    pub signals_only_in_a: Vec<String>,
    pub signals_only_in_b: Vec<String>,
    pub signal_deltas: Vec<SignalDelta>,
    pub findings_only_in_a: Vec<Finding>,
    pub findings_only_in_b: Vec<Finding>,
    pub findings_severity_changed: Vec<SeverityChange>,
}

/// Compute the diff between two `AnalysisReport`s.
pub fn diff(a: &AnalysisReport, b: &AnalysisReport) -> DiffReport {
    let mut out = DiffReport::default();

    let map_a: HashMap<&str, f64> = a
        .signals()
        .iter()
        .filter_map(|s| signal_to_f64(&s.value).map(|v| (s.id.as_str(), v)))
        .collect();
    let map_b: HashMap<&str, f64> = b
        .signals()
        .iter()
        .filter_map(|s| signal_to_f64(&s.value).map(|v| (s.id.as_str(), v)))
        .collect();

    for (id, va) in &map_a {
        match map_b.get(id) {
            Some(vb) => {
                if (va - vb).abs() > f64::EPSILON {
                    out.signal_deltas.push(SignalDelta {
                        signal_id: id.to_string(),
                        value_in_a: *va,
                        value_in_b: *vb,
                        delta: vb - va,
                    });
                }
            }
            None => out.signals_only_in_a.push(id.to_string()),
        }
    }
    for id in map_b.keys() {
        if !map_a.contains_key(id) {
            out.signals_only_in_b.push(id.to_string());
        }
    }
    out.signals_only_in_a.sort();
    out.signals_only_in_b.sort();
    out.signal_deltas.sort_by(|x, y| x.signal_id.cmp(&y.signal_id));

    let findings_a: HashMap<&str, &Finding> = a.findings().iter().map(|f| (f.id.as_str(), f)).collect();
    let findings_b: HashMap<&str, &Finding> = b.findings().iter().map(|f| (f.id.as_str(), f)).collect();

    for (id, fa) in &findings_a {
        match findings_b.get(id) {
            Some(fb) if fa.severity != fb.severity => {
                out.findings_severity_changed.push(SeverityChange {
                    finding_id: id.to_string(),
                    severity_in_a: fa.severity,
                    severity_in_b: fb.severity,
                });
            }
            Some(_) => {}
            None => out.findings_only_in_a.push((*fa).clone()),
        }
    }
    for (id, fb) in &findings_b {
        if !findings_a.contains_key(id) {
            out.findings_only_in_b.push((*fb).clone());
        }
    }
    out.findings_only_in_a.sort_by(|x, y| x.id.cmp(&y.id));
    out.findings_only_in_b.sort_by(|x, y| x.id.cmp(&y.id));
    out.findings_severity_changed
        .sort_by(|x, y| x.finding_id.cmp(&y.finding_id));

    out
}

fn signal_to_f64(v: &SignalValue) -> Option<f64> {
    match v {
        SignalValue::F64(x) => Some(*x),
        SignalValue::I64(x) => Some(*x as f64),
        _ => None,
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{:.2}", v)
    }
}

fn fmt_num_signed(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{:+}", v as i64)
    } else {
        format!("{:+.2}", v)
    }
}

fn basename(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// Render the diff in plain-text form.
pub fn render_text<W: std::io::Write>(d: &DiffReport, label_a: &str, label_b: &str, mut w: W) -> std::io::Result<()> {
    let name_a = basename(label_a);
    let name_b = basename(label_b);

    writeln!(w, "Signals changed:")?;
    if d.signal_deltas.is_empty() {
        writeln!(w, "  (none)")?;
    } else {
        for sd in &d.signal_deltas {
            let before = fmt_num(sd.value_in_a);
            let after = fmt_num(sd.value_in_b);
            let delta = fmt_num_signed(sd.delta);
            writeln!(w, "  {:<30} {:>10}  \u{2192}  {:>10}   ({})", sd.signal_id, before, after, delta)?;
        }
    }
    writeln!(w)?;
    writeln!(w, "Signals only in {}:", name_a)?;
    if d.signals_only_in_a.is_empty() {
        writeln!(w, "  (none)")?;
    } else {
        for sig in &d.signals_only_in_a {
            writeln!(w, "  {}", sig)?;
        }
    }
    writeln!(w)?;
    writeln!(w, "Signals only in {}:", name_b)?;
    if d.signals_only_in_b.is_empty() {
        writeln!(w, "  (none)")?;
    } else {
        for sig in &d.signals_only_in_b {
            writeln!(w, "  {}", sig)?;
        }
    }
    writeln!(w)?;
    writeln!(w, "Findings only in {}:", name_a)?;
    if d.findings_only_in_a.is_empty() {
        writeln!(w, "  (none)")?;
    } else {
        for f in &d.findings_only_in_a {
            writeln!(w, "  [{}] {}", f.severity, f.id)?;
        }
    }
    writeln!(w)?;
    writeln!(w, "Findings only in {}:", name_b)?;
    if d.findings_only_in_b.is_empty() {
        writeln!(w, "  (none)")?;
    } else {
        for f in &d.findings_only_in_b {
            writeln!(w, "  [{}] {}", f.severity, f.id)?;
        }
    }
    writeln!(w)?;
    writeln!(w, "Findings with changed severity:")?;
    if d.findings_severity_changed.is_empty() {
        writeln!(w, "  (none)")?;
    } else {
        for sc in &d.findings_severity_changed {
            writeln!(w, "  [{} \u{2192} {}] {}", sc.severity_in_a, sc.severity_in_b, sc.finding_id)?;
        }
    }
    Ok(())
}
