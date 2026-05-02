//! Linux platform functions — reads from `/proc` and `/sys`.

use std::collections::HashMap;

use super::{CpuFreqSnapshot, CpuSnapshot, DiskDevSnapshot, HostSnapshot, MemSnapshot, NetSnapshot};

// ---------------------------------------------------------------------------
// CPU
// ---------------------------------------------------------------------------

pub fn read_cpu_snapshot() -> Option<CpuSnapshot> {
    let s = std::fs::read_to_string("/proc/stat").ok()?;
    let mut snap = parse_cpu_line(&s)?;
    snap.procs_running = parse_procs_running(&s);
    snap.ctxt = parse_ctxt(&s);
    Some(snap)
}

fn parse_cpu_line(s: &str) -> Option<CpuSnapshot> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("cpu ").or_else(|| line.strip_prefix("cpu  ")) {
            let n: Vec<u64> = rest.split_whitespace().filter_map(|t| t.parse::<u64>().ok()).collect();
            if n.len() < 4 {
                return None;
            }
            return Some(CpuSnapshot {
                user: n[0],
                nice: n[1],
                system: n[2],
                idle: n[3],
                iowait: Some(*n.get(4).unwrap_or(&0)),
                irq: *n.get(5).unwrap_or(&0),
                softirq: *n.get(6).unwrap_or(&0),
                steal: *n.get(7).unwrap_or(&0),
                procs_running: None,
                ctxt: None,
            });
        }
    }
    None
}

fn parse_procs_running(s: &str) -> Option<u64> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("procs_running ") {
            return rest.trim().parse::<u64>().ok();
        }
    }
    None
}

fn parse_ctxt(s: &str) -> Option<u64> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("ctxt ") {
            return rest.trim().parse::<u64>().ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

pub fn read_net_snapshot() -> Option<NetSnapshot> {
    let dev = std::fs::read_to_string("/proc/net/dev").ok()?;
    let rx_drops = parse_rx_drops(&dev);

    let snmp = std::fs::read_to_string("/proc/net/snmp").unwrap_or_default();
    let (tcp_out_segs, tcp_retrans_segs, tcp_attempt_fails, tcp_estab_resets) = parse_tcp_snmp(&snmp);

    let sockstat = std::fs::read_to_string("/proc/net/sockstat").unwrap_or_default();
    let tcp_tw_count = parse_tw_count(&sockstat);

    Some(NetSnapshot {
        rx_drops,
        tcp_out_segs,
        tcp_retrans_segs,
        tcp_attempt_fails,
        tcp_estab_resets,
        tcp_tw_count,
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
        // /proc/net/dev: bytes packets errs drop … (rx side), drop is index 3
        if let Some(drop_str) = fields.get(3) {
            if let Ok(drops) = drop_str.parse::<u64>() {
                map.insert(iface.to_string(), drops);
            }
        }
    }
    map
}

fn parse_tcp_snmp(s: &str) -> (u64, u64, u64, u64) {
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
        return (0, 0, 0, 0);
    }
    let get = |name: &str| -> u64 {
        header
            .iter()
            .position(|h| *h == name)
            .and_then(|i| values.get(i))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    (
        get("OutSegs"),
        get("RetransSegs"),
        get("AttemptFails"),
        get("EstabResets"),
    )
}

fn parse_tw_count(s: &str) -> Option<u64> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("TCP:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            for w in parts.windows(2) {
                if w[0] == "tw" {
                    return w[1].parse().ok();
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Host
// ---------------------------------------------------------------------------

pub fn read_host_snapshot() -> Option<HostSnapshot> {
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get() as u64)
        .unwrap_or(1);
    let mem_total_bytes = read_mem_total_bytes()?;
    let load_avg_1m = read_load_avg_1m()?;
    Some(HostSnapshot {
        cpu_count,
        mem_total_bytes,
        load_avg_1m,
    })
}

fn read_mem_total_bytes() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

fn read_load_avg_1m() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/loadavg").ok()?;
    s.split_whitespace().next()?.parse().ok()
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

pub fn read_mem_snapshot() -> Option<MemSnapshot> {
    let out = std::process::Command::new("free").arg("-m").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut snap = parse_free_m_output(&stdout)?;
    snap.swap_in_pages = std::fs::read_to_string("/proc/vmstat")
        .ok()
        .as_deref()
        .and_then(parse_pswpin);
    Some(snap)
}

fn parse_pswpin(s: &str) -> Option<u64> {
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("pswpin ") {
            return rest.trim().parse().ok();
        }
    }
    None
}

fn parse_free_m_output(s: &str) -> Option<MemSnapshot> {
    let mut total_mb = None::<f64>;
    let mut used_mb = None::<f64>;
    let mut free_mb = None::<f64>;
    let mut available_mb = None::<f64>;
    let mut swap_total = 0.0f64;
    let mut swap_used = 0.0f64;
    let mut swap_free = 0.0f64;

    for line in s.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Mem:") {
            let n: Vec<f64> = rest.split_whitespace().filter_map(|x| x.parse::<f64>().ok()).collect();
            if n.len() < 3 {
                return None;
            }
            total_mb = Some(n[0]);
            used_mb = Some(n[1]);
            free_mb = Some(n[2]);
            available_mb = n.get(5).copied();
        } else if let Some(rest) = t.strip_prefix("Swap:") {
            let n: Vec<f64> = rest.split_whitespace().filter_map(|x| x.parse::<f64>().ok()).collect();
            if n.len() >= 3 {
                swap_total = n[0];
                swap_used = n[1];
                swap_free = n[2];
            }
        }
    }

    Some(MemSnapshot {
        total_mb: total_mb?,
        used_mb: used_mb?,
        free_mb: free_mb?,
        available_mb,
        swap_total_mb: swap_total,
        swap_used_mb: swap_used,
        swap_free_mb: swap_free,
        swap_in_pages: None, // populated by caller from /proc/vmstat
    })
}

// ---------------------------------------------------------------------------
// CPU frequency
// ---------------------------------------------------------------------------

pub fn read_cpufreq_snapshot() -> CpuFreqSnapshot {
    let freq_ratio = read_freq_ratio();
    let temp_celsius = read_max_temp_celsius();
    CpuFreqSnapshot {
        freq_ratio,
        temp_celsius,
    }
}

fn read_freq_ratio() -> Option<f64> {
    let base = std::path::Path::new("/sys/devices/system/cpu");
    let entries = std::fs::read_dir(base).ok()?;
    let mut cur_sum = 0.0f64;
    let mut max_sum = 0.0f64;
    let mut count = 0usize;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let n = name.to_string_lossy();
        if !n.starts_with("cpu") || !n[3..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let cur: f64 = std::fs::read_to_string(entry.path().join("cpufreq/scaling_cur_freq"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0);
        let max: f64 = std::fs::read_to_string(entry.path().join("cpufreq/scaling_max_freq"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0);
        if max > 0.0 {
            cur_sum += cur;
            max_sum += max;
            count += 1;
        }
    }
    if count == 0 || max_sum == 0.0 {
        None
    } else {
        Some(cur_sum / max_sum)
    }
}

fn read_max_temp_celsius() -> Option<f64> {
    let base = std::path::Path::new("/sys/class/thermal");
    let entries = std::fs::read_dir(base).ok()?;
    let mut max_mc: i64 = i64::MIN;
    let mut found = false;
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        if let Ok(s) = std::fs::read_to_string(entry.path().join("temp")) {
            if let Ok(mc) = s.trim().parse::<i64>() {
                if mc > max_mc {
                    max_mc = mc;
                    found = true;
                }
            }
        }
    }
    if found { Some(max_mc as f64 / 1000.0) } else { None }
}

// ---------------------------------------------------------------------------
// Disk
// ---------------------------------------------------------------------------

pub fn read_disk_snapshots() -> Vec<DiskDevSnapshot> {
    let s = match std::fs::read_to_string("/proc/diskstats") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    parse_diskstats(&s)
}

fn parse_diskstats(s: &str) -> Vec<DiskDevSnapshot> {
    let mut out = Vec::new();
    for line in s.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.len() < 14 {
            continue;
        }
        // fields: 0=major 1=minor 2=name 3=reads_completed 4=reads_merged
        // 5=sectors_read 6=time_reading_ms 7=writes_completed … 10=time_writing_ms
        // 11=ios_in_progress 12=time_doing_io_ms 13=weighted_time_io
        out.push(DiskDevSnapshot {
            name: toks[2].to_string(),
            read_ios: toks[3].parse().unwrap_or(0),
            write_ios: toks[7].parse().unwrap_or(0),
            read_time_ms: Some(toks[6].parse().unwrap_or(0)),
            write_time_ms: Some(toks[10].parse().unwrap_or(0)),
            io_time_ms: Some(toks[12].parse().unwrap_or(0)),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cpu_line_parses_known_input() {
        let s = "cpu  1000 0 200 8000 500 10 5 0 0 0\n";
        let snap = parse_cpu_line(s).unwrap();
        assert_eq!(snap.user, 1000);
        assert_eq!(snap.iowait, Some(500));
        assert_eq!(snap.idle, 8000);
    }

    #[test]
    fn parse_rx_drops_skips_loopback() {
        let s = "Inter-|   Receive\n face |bytes\n    lo:  100  1 0 0\n  eth0:200000 2000 0 5\n";
        let drops = parse_rx_drops(s);
        assert!(!drops.contains_key("lo"));
        assert_eq!(drops.get("eth0"), Some(&5));
    }

    #[test]
    fn parse_free_m_output_parses_known_input() {
        let s = "              total        used        free      shared  buff/cache   available\nMem:           7977         511        4983          15        2483        7191\nSwap:          7977         512        7465\n";
        let snap = parse_free_m_output(s).unwrap();
        assert_eq!(snap.total_mb, 7977.0);
        assert_eq!(snap.used_mb, 511.0);
        assert_eq!(snap.free_mb, 4983.0);
        assert_eq!(snap.available_mb, Some(7191.0));
        assert_eq!(snap.swap_total_mb, 7977.0);
    }

    #[test]
    fn parse_tcp_snmp_extracts_attempt_fails_and_estab_resets() {
        let s = "Tcp: RtoAlgorithm RtoMin RtoMax MaxConn ActiveOpens PassiveOpens AttemptFails EstabResets CurrEstab InSegs OutSegs RetransSegs InErrs OutRsts InCsumErrors\nTcp: 1 200 120000 -1 161783 8 161547 9 21 2397185 7073859 446 0 161597 0\n";
        let (out, retrans, fails, resets) = parse_tcp_snmp(s);
        assert_eq!(out, 7073859);
        assert_eq!(retrans, 446);
        assert_eq!(fails, 161547);
        assert_eq!(resets, 9);
    }

    #[test]
    fn parse_pswpin_extracts_value() {
        let s = "pswpout 0\npswpin 42\nnr_free_pages 12345\n";
        assert_eq!(parse_pswpin(s), Some(42));
    }

    #[test]
    fn parse_pswpin_returns_none_when_absent() {
        assert_eq!(parse_pswpin("nr_free_pages 1\n"), None);
    }

    #[test]
    fn parse_diskstats_parses_known_line() {
        let s = "   8       0 sda 100 0 800 200 50 0 400 100 0 300 300 0 0 0 0\n";
        let snaps = parse_diskstats(s);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].name, "sda");
        assert_eq!(snaps[0].read_ios, 100);
        assert_eq!(snaps[0].read_time_ms, Some(200));
        assert_eq!(snaps[0].io_time_ms, Some(300));
    }
}
