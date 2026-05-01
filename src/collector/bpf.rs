//! eBPF opt-in collectors (feature-gated: `--features bpf`).
//!
//! Each tool wrapper calls `which::which()` at collect time. If the tool is
//! absent from PATH it emits `bpf.<tool>.available = false` and skips
//! execution. When present, the tool is invoked for a 2-second window; its
//! histogram output is parsed into latency-percentile signals with `samples`
//! populated so that `.p50`/`.p95`/`.p99` predicates work in rules.

use chrono::Local;

use crate::baseline::stats::sample_stats;
use crate::collector::{CollectCtx, Result};
use crate::signal::{Signal, SignalValue, Unit};

/// Histogram-producing tools and event-tracing tools bundled as a single collector.
pub const TOOLS: &[&str] = &["runqlat", "biolatency", "tcpretrans", "execsnoop", "cachestat"];

#[derive(Debug, Default)]
pub struct BpfCollector;

impl BpfCollector {
    pub fn new() -> Self {
        Self
    }
}

impl super::Collector for BpfCollector {
    fn id(&self) -> &str {
        "bpf"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let now = Local::now();
        let mut signals = Vec::new();

        for tool in TOOLS {
            let available = which::which(tool).is_ok();
            signals.push(Signal {
                id: format!("bpf.{}.available", tool),
                value: SignalValue::Bool(available),
                unit: Unit::None,
                at: now,
                samples: None,
                stats: None,
                baseline: None,
            });

            if available {
                // Invoke the tool for a short window and parse its output.
                if let Some(sig) = run_histogram_tool(tool, now) {
                    signals.push(sig);
                }
            }
        }

        Ok(signals)
    }
}

/// Run a bcc histogram tool for a 2-second window and produce a latency signal.
/// Returns `None` if the tool fails or produces no parseable histogram output.
fn run_histogram_tool(tool: &str, now: chrono::DateTime<Local>) -> Option<Signal> {
    // Histogram tools: runqlat, biolatency. Others produce event-based output.
    let args: &[&str] = match tool {
        "runqlat" | "biolatency" | "cachestat" => &["2", "1"],
        _ => return None, // tcpretrans, execsnoop — event-based, no histogram
    };

    let output = std::process::Command::new(tool).args(args).output().ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let samples = parse_histogram_usecs(&stdout)?;
    if samples.is_empty() {
        return None;
    }

    let stats = sample_stats(&samples);
    let p50 = stats.as_ref().map(|s| s.p50).unwrap_or(0.0);

    Some(Signal {
        id: format!("bpf.{}.usecs", tool),
        value: SignalValue::F64(p50),
        unit: Unit::Microseconds,
        at: now,
        samples: Some(samples),
        stats,
        baseline: None,
    })
}

/// Parse standard bcc-tools histogram output into individual samples.
///
/// Expected line format: `   <lo> -> <hi>   : <count>   |...|`
/// Each bucket contributes `count` samples at the bucket midpoint.
pub fn parse_histogram_usecs(output: &str) -> Option<Vec<f64>> {
    let mut samples: Vec<f64> = Vec::new();
    for line in output.lines() {
        // Skip non-bucket lines (header, blank, distribution bar-only).
        let Some(arrow) = line.find("->") else { continue };
        let Some(colon_off) = line[arrow..].find(':') else {
            continue;
        };
        let colon = arrow + colon_off;

        let Ok(lo) = line[..arrow].trim().parse::<f64>() else {
            continue;
        };
        let Ok(hi) = line[arrow + 2..colon].trim().parse::<f64>() else {
            continue;
        };
        let count_str = line[colon + 1..].split('|').next().map(str::trim).unwrap_or("");
        let Ok(count) = count_str.parse::<u64>() else { continue };

        if count > 0 {
            let midpoint = (lo + hi) / 2.0;
            for _ in 0..count {
                samples.push(midpoint);
            }
        }
    }
    if samples.is_empty() { None } else { Some(samples) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HISTOGRAM: &str = r#"
     usecs               : count     distribution
         0 -> 1          : 0        |                        |
         2 -> 3          : 0        |                        |
         4 -> 7          : 2        |                        |
         8 -> 15         : 0        |                        |
        16 -> 31         : 0        |                        |
        32 -> 63         : 1        |                        |
        64 -> 127        : 4        |**                      |
       128 -> 255        : 12       |*****                   |
       256 -> 511        : 40       |********************    |
       512 -> 1023       : 48       |************************|
      1024 -> 2047       : 30       |***************         |
      2048 -> 4095       : 2        |                        |
    "#;

    #[test]
    fn parse_histogram_produces_samples() {
        let samples = parse_histogram_usecs(SAMPLE_HISTOGRAM).expect("parse ok");
        assert!(!samples.is_empty(), "should produce samples");
        // 2 + 1 + 4 + 12 + 40 + 48 + 30 + 2 = 139 samples
        assert_eq!(samples.len(), 139);
    }

    #[test]
    fn parse_histogram_p50_in_expected_range() {
        let samples = parse_histogram_usecs(SAMPLE_HISTOGRAM).expect("parse ok");
        let stats = sample_stats(&samples).expect("stats ok");
        // Bulk of samples are in 256-1023 and 1024-2047 range; p50 should be ~640 (midpoint of 512-1023)
        assert!(stats.p50 > 256.0 && stats.p50 < 2048.0, "p50={}", stats.p50);
        // p99 should be in the 1024-2047 bucket range
        assert!(stats.p99 > 512.0 && stats.p99 < 4096.0, "p99={}", stats.p99);
    }

    #[test]
    fn parse_histogram_empty_for_no_counts() {
        let output = "     usecs : count distribution\n 0 -> 1 : 0 |  |";
        assert!(parse_histogram_usecs(output).is_none());
    }

    /// Requires actual bpftrace/bcc tools in PATH. Run manually on a Linux host
    /// with bpfcc-tools installed: `cargo test --features bpf -- --ignored`
    #[test]
    #[ignore = "requires runqlat in PATH (bpfcc-tools on Linux)"]
    fn integration_runqlat_produces_histogram_signal() {
        use super::super::Collector as _;
        let collector = BpfCollector::new();
        let ctx = CollectCtx::default();
        let signals = collector.collect(&ctx).expect("collect ok");
        let hist = signals.iter().find(|s| s.id == "bpf.runqlat.usecs");
        assert!(hist.is_some(), "expected bpf.runqlat.usecs signal");
        let h = hist.unwrap();
        assert!(h.samples.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
        assert!(h.stats.is_some());
    }
}
