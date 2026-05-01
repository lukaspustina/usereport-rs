//! Network collector — `/proc/net/dev`, `/proc/net/snmp`, `/proc/net/sockstat`
//! delta engine (SDD Req 9).
//!
//! Two snapshots ≥ 1 s apart yield per-second rates. On hosts without `/proc`
//! (e.g. macOS) returns an empty Vec gracefully.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::Local;

use super::{CollectCtx, Collector, Result};
use crate::signal::{Signal, SignalValue, Unit};

const MIN_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct NetworkCollector;

impl NetworkCollector {
    pub fn new() -> Self {
        NetworkCollector
    }

    /// Delta-engine entry point for tests: compute signals from two
    /// `/proc/net/dev` + `/proc/net/snmp` snapshot pairs and elapsed time.
    pub fn from_snapshots(dev1: &str, dev2: &str, snmp1: &str, snmp2: &str, elapsed_secs: f64) -> Vec<Signal> {
        let now = Local::now();
        let mut signals = Vec::new();
        if elapsed_secs <= 0.0 {
            return signals;
        }

        // rx_drops: sum across non-loopback interfaces
        let drops1 = parse_rx_drops(dev1);
        let drops2 = parse_rx_drops(dev2);
        let total_drops: u64 = drops2
            .iter()
            .map(|(iface, d2)| d2.saturating_sub(*drops1.get(iface).unwrap_or(d2)))
            .sum();
        push(&mut signals, "net.rx_drops", total_drops as f64, Unit::Count, now);

        // retrans_pct from /proc/net/snmp TCP counters
        if let (Some(s1), Some(s2)) = (parse_tcp_snmp(snmp1), parse_tcp_snmp(snmp2)) {
            let out_delta = s2.out_segs.saturating_sub(s1.out_segs) as f64;
            let ret_delta = s2.retrans_segs.saturating_sub(s1.retrans_segs) as f64;
            let retrans_pct = if out_delta > 0.0 {
                (ret_delta / out_delta) * 100.0
            } else {
                0.0
            };
            push(&mut signals, "net.retrans_pct", retrans_pct, Unit::Pct, now);
        }

        signals
    }
}

impl Collector for NetworkCollector {
    fn id(&self) -> &str {
        "network"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let dev1 = match std::fs::read_to_string("/proc/net/dev") {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };
        let snmp1 = std::fs::read_to_string("/proc/net/snmp").unwrap_or_default();

        let start = Instant::now();
        let elapsed = start.elapsed();
        if elapsed < MIN_WINDOW {
            std::thread::sleep(MIN_WINDOW - elapsed);
        }

        let dev2 = std::fs::read_to_string("/proc/net/dev").unwrap_or_default();
        let snmp2 = std::fs::read_to_string("/proc/net/snmp").unwrap_or_default();
        let elapsed_secs = start.elapsed().as_secs_f64().max(0.001);

        let mut signals = Self::from_snapshots(&dev1, &dev2, &snmp1, &snmp2, elapsed_secs);

        // tw_count from /proc/net/sockstat (not a delta — point-in-time count)
        if let Ok(sockstat) = std::fs::read_to_string("/proc/net/sockstat") {
            if let Some(tw) = parse_tw_count(&sockstat) {
                let now = Local::now();
                push(&mut signals, "net.tw_count", tw as f64, Unit::Count, now);
            }
        }

        Ok(signals)
    }
}

#[derive(Default)]
struct TcpSnmp {
    out_segs: u64,
    retrans_segs: u64,
}

fn parse_tcp_snmp(s: &str) -> Option<TcpSnmp> {
    let mut header: Vec<&str> = Vec::new();
    let mut values: Vec<&str> = Vec::new();
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("Tcp:") {
            let fields: Vec<&str> = rest.split_whitespace().collect();
            if header.is_empty() {
                header = fields;
            } else {
                values = fields;
            }
        }
    }
    if header.is_empty() || values.len() != header.len() {
        return None;
    }
    let idx = |name: &str| header.iter().position(|h| *h == name);
    let get = |name: &str| -> u64 {
        idx(name)
            .and_then(|i| values.get(i))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    Some(TcpSnmp {
        out_segs: get("OutSegs"),
        retrans_segs: get("RetransSegs"),
    })
}

fn parse_rx_drops(s: &str) -> HashMap<String, u64> {
    let mut map = HashMap::new();
    for line in s.lines() {
        let line = line.trim();
        let Some(colon) = line.find(':') else { continue };
        let iface = line[..colon].trim();
        if iface == "lo" || iface == "face" || iface == "Inter" {
            continue;
        }
        let fields: Vec<&str> = line[colon + 1..].split_whitespace().collect();
        // /proc/net/dev columns: bytes packets errs drop ... (rx) | bytes packets errs drop ... (tx)
        // drop is index 3 (0-based)
        if let Some(drop_str) = fields.get(3) {
            if let Ok(drops) = drop_str.parse::<u64>() {
                map.insert(iface.to_string(), drops);
            }
        }
    }
    map
}

fn parse_tw_count(s: &str) -> Option<u64> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("TCP:") {
            for part in rest.split_whitespace().collect::<Vec<_>>().windows(2) {
                if part[0] == "tw" {
                    return part[1].parse().ok();
                }
            }
        }
    }
    None
}

fn push(signals: &mut Vec<Signal>, id: &str, val: f64, unit: Unit, now: chrono::DateTime<Local>) {
    signals.push(Signal {
        id: id.to_string(),
        value: SignalValue::F64(val),
        unit,
        at: now,
        samples: None,
        stats: None,
        baseline: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV1: &str = r#"Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo:    100      1    0    0    0     0          0         0     100      1    0    0    0     0       0          0
  eth0: 100000   1000    0    5    0     0          0         0   50000    500    0    0    0     0       0          0"#;

    const DEV2: &str = r#"Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo:    200      2    0    0    0     0          0         0     200      2    0    0    0     0       0          0
  eth0: 200000   2000    0    8    0     0          0         0  100000   1000    0    0    0     0       0          0"#;

    const SNMP1: &str = "Tcp: RtoAlgorithm RtoMin RtoMax MaxConn ActiveOpens PassiveOpens AttemptFails EstabResets CurrEstab InSegs OutSegs RetransSegs InErrs OutRsts InCsumErrors\nTcp: 1 200 120000 -1 100 50 5 10 20 1000 900 9 0 5 0";
    const SNMP2: &str = "Tcp: RtoAlgorithm RtoMin RtoMax MaxConn ActiveOpens PassiveOpens AttemptFails EstabResets CurrEstab InSegs OutSegs RetransSegs InErrs OutRsts InCsumErrors\nTcp: 1 200 120000 -1 110 55 5 10 20 1100 1000 15 0 5 0";

    #[test]
    fn parse_rx_drops_skips_loopback() {
        let drops = parse_rx_drops(DEV1);
        assert!(!drops.contains_key("lo"));
        assert_eq!(drops.get("eth0"), Some(&5));
    }

    #[test]
    fn network_snapshot_emits_drops_and_retrans() {
        let signals = NetworkCollector::from_snapshots(DEV1, DEV2, SNMP1, SNMP2, 1.0);
        let ids: Vec<_> = signals.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"net.rx_drops"), "missing rx_drops: {:?}", ids);
        assert!(ids.contains(&"net.retrans_pct"), "missing retrans_pct: {:?}", ids);
        // 3 new drops on eth0 over 1s
        let drops = signals.iter().find(|s| s.id == "net.rx_drops").unwrap();
        assert_eq!(drops.value, SignalValue::F64(3.0));
        // RetransSegs delta = 15-9=6, OutSegs delta = 1000-900=100 → 6%
        let retrans = signals.iter().find(|s| s.id == "net.retrans_pct").unwrap();
        if let SignalValue::F64(v) = retrans.value {
            assert!((v - 6.0).abs() < 0.01, "retrans_pct = {}", v);
        }
    }

    #[test]
    fn parse_tw_count_extracts_value() {
        let sockstat = "sockets: used 292\nTCP: inuse 115 orphan 0 tw 2345 alloc 117 mem 8\n";
        assert_eq!(parse_tw_count(sockstat), Some(2345));
    }
}
