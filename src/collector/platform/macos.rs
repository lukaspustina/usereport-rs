//! macOS platform functions — reads via sysctl, netstat, vm_stat.

use std::collections::HashMap;

use super::{CpuFreqSnapshot, CpuSnapshot, DiskDevSnapshot, HostSnapshot, MemSnapshot, NetSnapshot};

fn run(bin: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

// ---------------------------------------------------------------------------
// Host
// ---------------------------------------------------------------------------

pub fn read_host_snapshot() -> Option<HostSnapshot> {
    let cpu_count: u64 = run("sysctl", &["-n", "hw.logicalcpu"])?.trim().parse().ok()?;
    let mem_total_bytes: u64 = run("sysctl", &["-n", "hw.memsize"])?.trim().parse().ok()?;
    let load_avg_1m = parse_loadavg(&run("sysctl", &["-n", "vm.loadavg"])?)?;
    Some(HostSnapshot {
        cpu_count,
        mem_total_bytes,
        load_avg_1m,
    })
}

/// Parse `{ 0.52 0.58 0.57 }` → first float.
fn parse_loadavg(s: &str) -> Option<f64> {
    s.split_whitespace()
        .find(|t| !t.starts_with('{') && !t.starts_with('}'))?
        .parse()
        .ok()
}

// ---------------------------------------------------------------------------
// CPU
// ---------------------------------------------------------------------------

/// Returns cumulative-tick CPU snapshot derived from `iostat 1 1` and uptime.
///
/// `kern.cp_time` (the BSD sysctl) does not exist on Apple Silicon macOS.
/// Instead, `/usr/sbin/iostat 1 1` provides one-second CPU percentages.
/// We convert those to absolute tick estimates (uptime_secs * HZ * ncpus *
/// pct/100) so that two snapshots taken seconds apart yield a non-zero delta.
pub fn read_cpu_snapshot() -> Option<CpuSnapshot> {
    let out = std::process::Command::new("/usr/sbin/iostat")
        .args(["1", "1"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let (us, sy, id) = parse_iostat_cpu(&stdout)?;

    // Derive absolute tick estimates so two snapshots have a non-zero delta.
    let uptime_secs = read_uptime_secs().unwrap_or(1);
    let ncpus = run("sysctl", &["-n", "hw.logicalcpu"])
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(1);
    let hz: u64 = 100; // macOS HZ
    let total = uptime_secs * hz * ncpus;

    Some(CpuSnapshot {
        user: total * us / 100,
        nice: 0,
        system: total * sy / 100,
        idle: total * id / 100,
        iowait: None,
        irq: 0,
        softirq: 0,
        steal: 0,
        procs_running: None,
        ctxt: None,
    })
}

/// Parse `iostat 1 1` output: returns (user_pct, sys_pct, idle_pct).
///
/// The number of disk columns varies by machine (disk0 only vs disk0+disk1+…),
/// so we locate `us`/`sy`/`id` from the header line rather than using fixed
/// column indices.
fn parse_iostat_cpu(s: &str) -> Option<(u64, u64, u64)> {
    let mut us_col: Option<usize> = None;
    let mut sy_col: Option<usize> = None;
    let mut id_col: Option<usize> = None;

    for line in s.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        // Header line: locate the us/sy/id column positions.
        if us_col.is_none() {
            if let (Some(u), Some(s), Some(i)) = (
                parts.iter().position(|&p| p == "us"),
                parts.iter().position(|&p| p == "sy"),
                parts.iter().position(|&p| p == "id"),
            ) {
                us_col = Some(u);
                sy_col = Some(s);
                id_col = Some(i);
            }
            continue;
        }
        // First data line after the header.
        let (u, s, i) = (us_col?, sy_col?, id_col?);
        if parts.len() <= i {
            continue;
        }
        if let (Ok(us), Ok(sy), Ok(id)) = (
            parts[u].parse::<u64>(),
            parts[s].parse::<u64>(),
            parts[i].parse::<u64>(),
        ) {
            return Some((us, sy, id));
        }
    }
    None
}

fn read_uptime_secs() -> Option<u64> {
    // kern.boottime: { sec = 1775331334, usec = 473175 } ...
    let s = run("sysctl", &["-n", "kern.boottime"])?;
    // Output: { sec = NNNN, usec = NNNN }
    for w in s.split_whitespace().collect::<Vec<_>>().windows(3) {
        if w[0] == "sec" && w[1] == "=" {
            let boot_sec: u64 = w[2].trim_end_matches(',').parse().ok()?;
            let now_sec = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs();
            return Some(now_sec.saturating_sub(boot_sec).max(1));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

pub fn read_net_snapshot() -> Option<NetSnapshot> {
    let netstat_i = run("netstat", &["-i", "-b", "-n"])?;
    let rx_drops = parse_netstat_drops(&netstat_i);

    let netstat_s = run("netstat", &["-s", "-p", "tcp"]).unwrap_or_default();
    let (tcp_out_segs, tcp_retrans_segs, tcp_attempt_fails, tcp_tw_count) = parse_netstat_tcp_stats(&netstat_s);

    Some(NetSnapshot {
        rx_drops,
        tcp_out_segs,
        tcp_retrans_segs,
        tcp_attempt_fails,
        tcp_estab_resets: 0, // no equivalent counter in netstat -s -p tcp output
        tcp_tw_count,
    })
}

/// Parse `netstat -i -b -n`: skip header lines, skip lo0, take last field as Idrop.
fn parse_netstat_drops(s: &str) -> HashMap<String, u64> {
    let mut map = HashMap::new();
    for line in s.lines() {
        let line = line.trim();
        if line.starts_with("Name") {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        let name = fields[0];
        // Strip trailing asterisk (interface flag on some systems)
        let name = name.trim_end_matches('*');
        if name == "lo0" {
            continue;
        }
        // Last field is Idrop
        if let Some(last) = fields.last() {
            if let Ok(drops) = last.parse::<u64>() {
                map.insert(name.to_string(), drops);
            }
        }
    }
    map
}

/// Parse `netstat -s -p tcp` for out_segs, retrans_segs, attempt_fails, tw_count.
fn parse_netstat_tcp_stats(s: &str) -> (u64, u64, u64, Option<u64>) {
    let mut out_segs = 0u64;
    let mut retrans_segs = 0u64;
    let mut attempt_fails = 0u64;
    let mut tw_count = None;

    for line in s.lines() {
        let trimmed = line.trim();
        // "12345 packets sent"
        if trimmed.contains("packets sent") {
            if let Some(n) = first_int(trimmed) {
                out_segs = n;
            }
        }
        // "678 data packets (9012 bytes) retransmitted"
        if trimmed.contains("retransmitted") {
            if let Some(n) = first_int(trimmed) {
                retrans_segs = n;
            }
        }
        // "42 bad connection attempt"
        if trimmed.contains("bad connection attempt") {
            if let Some(n) = first_int(trimmed) {
                attempt_fails = n;
            }
        }
        // "99 connections in TIME_WAIT"
        if trimmed.contains("connections in TIME_WAIT") {
            tw_count = first_int(trimmed);
        }
    }

    (out_segs, retrans_segs, attempt_fails, tw_count)
}

fn first_int(s: &str) -> Option<u64> {
    s.split_whitespace().next()?.parse().ok()
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

pub fn read_mem_snapshot() -> Option<MemSnapshot> {
    let vm_stat_out = run("vm_stat", &[])?;
    let (page_size, pages) = parse_vm_stat(&vm_stat_out)?;

    let free_p = *pages.get("Pages free").unwrap_or(&0);
    let active_p = *pages.get("Pages active").unwrap_or(&0);
    let inactive_p = *pages.get("Pages inactive").unwrap_or(&0);
    let speculative_p = *pages.get("Pages speculative").unwrap_or(&0);
    let wired_p = *pages.get("Pages wired down").unwrap_or(&0);
    let purgeable_p = *pages.get("Pages purgeable").unwrap_or(&0);

    let total_pages = active_p + inactive_p + speculative_p + wired_p + purgeable_p + free_p;
    let bytes_per_mb = 1_048_576u64;
    let total_mb = (total_pages * page_size) as f64 / bytes_per_mb as f64;
    let free_mb = ((free_p + speculative_p) * page_size) as f64 / bytes_per_mb as f64;
    let used_mb = total_mb - free_mb;

    let (swap_total_mb, swap_used_mb, swap_free_mb) = run("sysctl", &["-n", "vm.swapusage"])
        .and_then(|s| parse_swapusage(&s))
        .unwrap_or((0.0, 0.0, 0.0));

    Some(MemSnapshot {
        total_mb,
        used_mb,
        free_mb,
        available_mb: None,
        swap_total_mb,
        swap_used_mb,
        swap_free_mb,
        swap_in_pages: pages.get("Swapins").copied(),
    })
}

/// Parse `vm_stat` output: returns (page_size_bytes, key→page_count map).
fn parse_vm_stat(s: &str) -> Option<(u64, HashMap<String, u64>)> {
    let mut page_size = None::<u64>;
    let mut pages = HashMap::new();

    for line in s.lines() {
        if page_size.is_none() {
            // Header: "Mach Virtual Memory Statistics: (page size of N bytes)"
            if let Some(rest) = line.find("page size of ").map(|i| &line[i + 13..]) {
                page_size = rest.split_whitespace().next()?.parse::<u64>().ok();
            }
            continue;
        }
        // Key-value lines: "Pages free:                12345."
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim().to_string();
            let val_str = line[colon + 1..].trim().trim_end_matches('.');
            if let Ok(n) = val_str.parse::<u64>() {
                pages.insert(key, n);
            }
        }
    }

    Some((page_size?, pages))
}

/// Parse `total = 2048.00M  used = 512.00M  free = 1536.00M`.
fn parse_swapusage(s: &str) -> Option<(f64, f64, f64)> {
    let get = |key: &str| -> Option<f64> {
        let idx = s.find(&format!("{} = ", key))?;
        let after = &s[idx + key.len() + 3..];
        let token = after.split_whitespace().next()?;
        let stripped = token.trim_end_matches(|c: char| c.is_alphabetic());
        let val: f64 = stripped.parse().ok()?;
        // Convert G → MB
        if token.ends_with('G') || token.ends_with('g') {
            Some(val * 1024.0)
        } else {
            Some(val)
        }
    };
    Some((get("total")?, get("used")?, get("free")?))
}

// ---------------------------------------------------------------------------
// CPU frequency — not available on macOS without root / chip-type detection
// ---------------------------------------------------------------------------

pub fn read_cpufreq_snapshot() -> CpuFreqSnapshot {
    CpuFreqSnapshot {
        freq_ratio: None,
        temp_celsius: None,
    }
}

// ---------------------------------------------------------------------------
// Disk — not available without root on macOS
// ---------------------------------------------------------------------------

pub fn read_disk_snapshots() -> Vec<DiskDevSnapshot> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// Tests (fixture-based, no real OS calls)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // T5 — vm_stat fixture
    const VM_STAT_FIXTURE: &str = "\
Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                            12345.
Pages active:                          56789.
Pages inactive:                        11111.
Pages speculative:                      2222.
Pages wired down:                       8888.
Pages purgeable:                        3333.
Swapins:                                 500.
Swapouts:                                200.
";

    #[test]
    fn parse_vm_stat_extracts_page_size_and_counts() {
        let (ps, pages) = parse_vm_stat(VM_STAT_FIXTURE).unwrap();
        assert_eq!(ps, 16384);
        assert_eq!(pages.get("Pages free"), Some(&12345));
        assert_eq!(pages.get("Pages active"), Some(&56789));
        assert_eq!(pages.get("Pages wired down"), Some(&8888));
        assert_eq!(pages.get("Swapins"), Some(&500));
    }

    #[test]
    fn vm_stat_total_mb_computed_correctly() {
        // total_pages = 12345+56789+11111+2222+8888+3333 = 94688
        // total_mb = 94688 * 16384 / 1_048_576 = 94688 * 16384 / 1048576
        let (ps, pages) = parse_vm_stat(VM_STAT_FIXTURE).unwrap();
        let free_p = *pages.get("Pages free").unwrap_or(&0);
        let active_p = *pages.get("Pages active").unwrap_or(&0);
        let inactive_p = *pages.get("Pages inactive").unwrap_or(&0);
        let speculative_p = *pages.get("Pages speculative").unwrap_or(&0);
        let wired_p = *pages.get("Pages wired down").unwrap_or(&0);
        let purgeable_p = *pages.get("Pages purgeable").unwrap_or(&0);
        let total_pages = active_p + inactive_p + speculative_p + wired_p + purgeable_p + free_p;
        assert_eq!(total_pages, 94688);
        let total_mb = (total_pages * ps) as f64 / 1_048_576.0;
        assert!((total_mb - 1479.5).abs() < 1.0, "total_mb = {}", total_mb);
    }

    // T6 — netstat tcp stats fixture
    const NETSTAT_TCP_FIXTURE: &str = "\
tcp:
\t12345 packets sent
\t\t678 data packets (9012 bytes) retransmitted
\t42 bad connection attempt
\t99 connections in TIME_WAIT
";

    #[test]
    fn parse_netstat_tcp_stats_parses_fixture() {
        let (out_segs, retrans, fails, tw) = parse_netstat_tcp_stats(NETSTAT_TCP_FIXTURE);
        assert_eq!(out_segs, 12345, "tcp_out_segs");
        assert_eq!(retrans, 678, "tcp_retrans_segs");
        assert_eq!(fails, 42, "tcp_attempt_fails");
        assert_eq!(tw, Some(99), "tcp_tw_count");
    }

    // T11 — netstat drops fixture skips lo0
    const NETSTAT_I_FIXTURE: &str = "\
Name  Mtu   Network       Address            Ipkts Ierrs Ibytes    Idrop Opkts  Oerrs Obytes    Coll Drop
lo0   16384 <Link#1>      127.0.0.1          1000    0   65536        0   1000    0   65536        0    0
en0   1500  <Link#2>      10.0.0.1           5000    0   2048000     3   4000    0   1024000      0    0
";

    #[test]
    fn parse_netstat_drops_skips_lo0() {
        let drops = parse_netstat_drops(NETSTAT_I_FIXTURE);
        assert!(!drops.contains_key("lo0"), "lo0 must be excluded; got: {:?}", drops);
        assert!(drops.contains_key("en0"), "en0 must be included; got: {:?}", drops);
    }

    #[test]
    fn parse_swapusage_parses_known_input() {
        let s = "total = 2048.00M  used = 512.00M  free = 1536.00M";
        let (total, used, free) = parse_swapusage(s).unwrap();
        assert_eq!(total, 2048.0);
        assert_eq!(used, 512.0);
        assert_eq!(free, 1536.0);
    }

    #[test]
    fn parse_loadavg_parses_brace_format() {
        let s = "{ 0.52 0.58 0.57 }";
        let v = parse_loadavg(s).unwrap();
        assert!((v - 0.52).abs() < 0.001, "got {}", v);
    }

    #[test]
    fn parse_iostat_cpu_parses_known_line() {
        let s = "              disk0       cpu    load average\n    KB/t  tps  MB/s  us sy id   1m   5m   15m\n   17.81  240  4.18  10  9 81  9.34 7.44 5.46\n";
        let (us, sy, id) = parse_iostat_cpu(s).unwrap();
        assert_eq!(us, 10);
        assert_eq!(sy, 9);
        assert_eq!(id, 81);
        assert_eq!(us + sy + id, 100);
    }
}
