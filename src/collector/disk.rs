//! Disk collector — `/proc/diskstats` delta engine.
//!
//! Two snapshots ≥ 1 s apart yield per-device read/write IOPS, util%, and
//! avg request latency (await). On hosts without `/proc/diskstats` the
//! runtime collector returns an empty `Vec<Signal>`.

use std::time::{Duration, Instant};

use chrono::Local;

use super::{CollectCtx, Collector, Result};
use crate::signal::{Signal, SignalValue, Unit};

const MIN_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct DiskCollector;

impl DiskCollector {
    pub fn new() -> Self {
        DiskCollector
    }

    /// Pure delta-engine entry point: parse two `/proc/diskstats` snapshots
    /// and emit per-device rate signals over `elapsed_secs`.
    pub fn from_proc_diskstats_snapshots(s1: &str, s2: &str, elapsed_secs: f64) -> Vec<Signal> {
        let now = Local::now();
        let mut signals = Vec::new();
        if elapsed_secs <= 0.0 {
            return signals;
        }
        let m1 = parse_diskstats(s1);
        let m2 = parse_diskstats(s2);
        for (dev, b) in &m2 {
            // Skip partitions: kernels expose both whole-disk and partitions
            // (e.g. sda + sda1). For Phase 3 we keep both — rules can filter
            // later. Excluding partitions is a stylistic choice deferred.
            if let Some(a) = m1.get(dev) {
                let read_delta = b.read_ios.saturating_sub(a.read_ios) as f64;
                let write_delta = b.write_ios.saturating_sub(a.write_ios) as f64;
                let read_t_delta = b.read_time_ms.saturating_sub(a.read_time_ms) as f64;
                let write_t_delta = b.write_time_ms.saturating_sub(a.write_time_ms) as f64;
                let io_t_delta = b.io_time_ms.saturating_sub(a.io_time_ms) as f64;

                push(
                    &mut signals,
                    &format!("disk.{}.read_iops", dev),
                    read_delta / elapsed_secs,
                    Unit::Iops,
                    now,
                );
                push(
                    &mut signals,
                    &format!("disk.{}.write_iops", dev),
                    write_delta / elapsed_secs,
                    Unit::Iops,
                    now,
                );
                let util = (io_t_delta / (elapsed_secs * 1000.0)) * 100.0;
                push(
                    &mut signals,
                    &format!("disk.{}.util_pct", dev),
                    util.min(100.0),
                    Unit::Pct,
                    now,
                );
                let total_ios = read_delta + write_delta;
                let await_ms = if total_ios > 0.0 {
                    (read_t_delta + write_t_delta) / total_ios
                } else {
                    0.0
                };
                push(
                    &mut signals,
                    &format!("disk.{}.await_ms", dev),
                    await_ms,
                    Unit::MillisPerOp,
                    now,
                );
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
        let s1 = match std::fs::read_to_string("/proc/diskstats") {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };
        let started = Instant::now();
        std::thread::sleep(MIN_WINDOW);
        let elapsed_secs = started.elapsed().as_secs_f64().max(1.0);
        let s2 = match std::fs::read_to_string("/proc/diskstats") {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };
        Ok(Self::from_proc_diskstats_snapshots(&s1, &s2, elapsed_secs))
    }
}

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
        // Standard fields: 0=major 1=minor 2=name 3=reads 4=reads_merged
        // 5=sectors_read 6=time_reading 7=writes 8=writes_merged 9=sectors_written
        // 10=time_writing 11=ios_in_progress 12=time_doing_io 13=weighted_time_io
        let dev = toks[2].to_string();
        let read_ios: u64 = toks[3].parse().unwrap_or(0);
        let read_time_ms: u64 = toks[6].parse().unwrap_or(0);
        let write_ios: u64 = toks[7].parse().unwrap_or(0);
        let write_time_ms: u64 = toks[10].parse().unwrap_or(0);
        let io_time_ms: u64 = toks[12].parse().unwrap_or(0);
        out.insert(
            dev,
            DiskStats {
                read_ios,
                read_time_ms,
                write_ios,
                write_time_ms,
                io_time_ms,
            },
        );
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
        baseline: None,
    });
}
