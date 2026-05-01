//! Disk collector — platform snapshot delta engine.
//!
//! Two snapshots ≥ 1 s apart yield per-device IOPS, util%, and await.
//! `util_pct` and `await_ms` are only emitted when `io_time_ms` is `Some`
//! (always on Linux; always None on macOS).

use std::time::{Duration, Instant};

use chrono::Local;

use super::{CollectCtx, Collector, Result};
use crate::collector::platform::{DiskDevSnapshot, read_disk_snapshots};
use crate::signal::{Signal, SignalValue, Unit};

const MIN_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct DiskCollector;

impl DiskCollector {
    pub fn new() -> Self {
        DiskCollector
    }

    /// Pure delta-engine entry point: parse two `/proc/diskstats` snapshots.
    pub fn from_proc_diskstats_snapshots(s1: &str, s2: &str, elapsed_secs: f64) -> Vec<Signal> {
        let now = Local::now();
        let mut signals = Vec::new();
        if elapsed_secs <= 0.0 {
            return signals;
        }
        let m1 = parse_diskstats(s1);
        let m2 = parse_diskstats(s2);
        for (dev, b) in &m2 {
            if let Some(a) = m1.get(dev) {
                let read_delta = b.read_ios.saturating_sub(a.read_ios) as f64;
                let write_delta = b.write_ios.saturating_sub(a.write_ios) as f64;
                let read_t_delta = b.read_time_ms.saturating_sub(a.read_time_ms) as f64;
                let write_t_delta = b.write_time_ms.saturating_sub(a.write_time_ms) as f64;
                let io_t_delta = b.io_time_ms.saturating_sub(a.io_time_ms) as f64;

                push(&mut signals, &format!("disk.{}.read_iops", dev), read_delta / elapsed_secs, Unit::Iops, now);
                push(&mut signals, &format!("disk.{}.write_iops", dev), write_delta / elapsed_secs, Unit::Iops, now);
                let util = (io_t_delta / (elapsed_secs * 1000.0)) * 100.0;
                push(&mut signals, &format!("disk.{}.util_pct", dev), util.min(100.0), Unit::Pct, now);
                let total_ios = read_delta + write_delta;
                let await_ms = if total_ios > 0.0 {
                    (read_t_delta + write_t_delta) / total_ios
                } else {
                    0.0
                };
                push(&mut signals, &format!("disk.{}.await_ms", dev), await_ms, Unit::MillisPerOp, now);
            }
        }
        signals
    }

    /// Snapshot-based delta engine for both Linux and macOS.
    /// `util_pct` and `await_ms` are omitted when `io_time_ms` is `None`.
    pub fn from_disk_snapshots(a: &[DiskDevSnapshot], b: &[DiskDevSnapshot], elapsed_secs: f64) -> Vec<Signal> {
        let now = Local::now();
        let mut signals = Vec::new();
        if elapsed_secs <= 0.0 {
            return signals;
        }

        // Build lookup by device name for snapshot a
        let a_by_name: std::collections::HashMap<&str, &DiskDevSnapshot> =
            a.iter().map(|d| (d.name.as_str(), d)).collect();

        for b_dev in b {
            let Some(a_dev) = a_by_name.get(b_dev.name.as_str()) else { continue };

            let read_delta = b_dev.read_ios.saturating_sub(a_dev.read_ios) as f64;
            let write_delta = b_dev.write_ios.saturating_sub(a_dev.write_ios) as f64;

            push(&mut signals, &format!("disk.{}.read_iops", b_dev.name), read_delta / elapsed_secs, Unit::Iops, now);
            push(&mut signals, &format!("disk.{}.write_iops", b_dev.name), write_delta / elapsed_secs, Unit::Iops, now);

            if let (Some(a_io), Some(b_io)) = (a_dev.io_time_ms, b_dev.io_time_ms) {
                let io_t_delta = b_io.saturating_sub(a_io) as f64;
                let util = (io_t_delta / (elapsed_secs * 1000.0)) * 100.0;
                push(&mut signals, &format!("disk.{}.util_pct", b_dev.name), util.min(100.0), Unit::Pct, now);

                let read_t_delta = b_dev.read_time_ms.unwrap_or(0).saturating_sub(a_dev.read_time_ms.unwrap_or(0)) as f64;
                let write_t_delta = b_dev.write_time_ms.unwrap_or(0).saturating_sub(a_dev.write_time_ms.unwrap_or(0)) as f64;
                let total_ios = read_delta + write_delta;
                let await_ms = if total_ios > 0.0 {
                    (read_t_delta + write_t_delta) / total_ios
                } else {
                    0.0
                };
                push(&mut signals, &format!("disk.{}.await_ms", b_dev.name), await_ms, Unit::MillisPerOp, now);
            }
        }
        signals
    }
}

impl Collector for DiskCollector {
    fn id(&self) -> &str {
        "disk"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let s1 = read_disk_snapshots();
        if s1.is_empty() {
            return Ok(Vec::new());
        }
        let started = Instant::now();
        std::thread::sleep(MIN_WINDOW);
        let elapsed_secs = started.elapsed().as_secs_f64().max(1.0);
        let s2 = read_disk_snapshots();
        Ok(Self::from_disk_snapshots(&s1, &s2, elapsed_secs))
    }
}

// ---------------------------------------------------------------------------
// Legacy /proc/diskstats helpers — preserved for from_proc_diskstats_snapshots()
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy)]
struct DiskStats {
    read_ios: u64,
    read_time_ms: u64,
    write_ios: u64,
    write_time_ms: u64,
    io_time_ms: u64,
}

fn parse_diskstats(s: &str) -> std::collections::HashMap<String, DiskStats> {
    let mut out = std::collections::HashMap::new();
    for line in s.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.len() < 14 {
            continue;
        }
        let dev = toks[2].to_string();
        out.insert(dev, DiskStats {
            read_ios: toks[3].parse().unwrap_or(0),
            read_time_ms: toks[6].parse().unwrap_or(0),
            write_ios: toks[7].parse().unwrap_or(0),
            write_time_ms: toks[10].parse().unwrap_or(0),
            io_time_ms: toks[12].parse().unwrap_or(0),
        });
    }
    out
}

fn push(signals: &mut Vec<Signal>, id: &str, v: f64, unit: Unit, at: chrono::DateTime<Local>) {
    signals.push(Signal {
        id: id.to_string(),
        value: SignalValue::F64(v),
        unit,
        at,
        samples: None,
        stats: None,
        baseline: None,
    });
}
