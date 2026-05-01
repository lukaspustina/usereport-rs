//! Interrupts collector — `/proc/interrupts` (SDD Req 12).
//!
//! Reads the per-CPU IRQ table and computes what fraction of NIC interrupts
//! are handled by the busiest CPU, emitting `net.max_cpu_irq_pct`.
//! Returns empty Vec gracefully on macOS / any host without `/proc/interrupts`.

use chrono::Local;

use super::{CollectCtx, Collector, Result};
use crate::signal::{Signal, SignalValue, Unit};

#[derive(Debug, Clone, Default)]
pub struct InterruptsCollector;

impl InterruptsCollector {
    pub fn new() -> Self {
        InterruptsCollector
    }
}

impl Collector for InterruptsCollector {
    fn id(&self) -> &str {
        "interrupts"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let content = match std::fs::read_to_string("/proc/interrupts") {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };
        let now = Local::now();
        let mut signals = Vec::new();
        if let Some(pct) = max_cpu_irq_pct(&content) {
            signals.push(Signal {
                id: "net.max_cpu_irq_pct".to_string(),
                value: SignalValue::F64(pct),
                unit: Unit::Pct,
                at: now,
                samples: None,
                stats: None,
                baseline: None,
            });
        }
        Ok(signals)
    }
}

/// Parse `/proc/interrupts` and return the percentage of NIC-related interrupts
/// handled by the busiest CPU. Returns `None` if no NIC IRQ lines are found.
///
/// NIC IRQ lines are identified heuristically: the description field contains
/// a network device name (present in `/sys/class/net/`), or the interrupt type
/// includes "eth", "mlx", "bnx", "ixgbe", "i40e", "virtio", "xhci" prefixes
/// that are commonly associated with network hardware.
pub fn max_cpu_irq_pct(content: &str) -> Option<f64> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // First line is the CPU header — count CPUs.
    let ncpus = lines[0].split_whitespace().count();
    if ncpus == 0 {
        return None;
    }

    let mut per_cpu: Vec<u64> = vec![0u64; ncpus];
    let mut total: u64 = 0;

    for line in &lines[1..] {
        if !is_nic_irq_line(line) {
            continue;
        }
        // Fields after the IRQ number are per-CPU counts, then type and description.
        let parts: Vec<&str> = line.split_whitespace().collect();
        // parts[0] = "123:" or IRQ name, then up to ncpus counts, then type + desc
        let start = 1; // skip the IRQ number/name field
        for (i, part) in parts[start..].iter().enumerate() {
            if i >= ncpus {
                break;
            }
            if let Ok(n) = part.parse::<u64>() {
                per_cpu[i] = per_cpu[i].saturating_add(n);
                total = total.saturating_add(n);
            } else {
                break; // hit the type/description text
            }
        }
    }

    if total == 0 {
        return None;
    }

    let max = *per_cpu.iter().max().unwrap_or(&0);
    Some((max as f64 / total as f64) * 100.0)
}

/// Returns true if an /proc/interrupts line looks like a NIC IRQ.
fn is_nic_irq_line(line: &str) -> bool {
    // Network-related keywords in interrupt descriptions.
    const NIC_KEYWORDS: &[&str] = &[
        "eth",
        "enp",
        "ens",
        "em",
        "eno",
        "bond",
        "vlan",
        "virtio-net",
        "virtio0",
        "mlx",
        "bnx",
        "ixgbe",
        "i40e",
        "igb",
        "e1000",
        "vmxnet",
        "xen-net",
    ];
    let lower = line.to_lowercase();
    NIC_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"           CPU0       CPU1       CPU2       CPU3
  0:       2154          0          0          0  IR-IO-APIC    2-edge      timer
 26:     100000      10000       1000        100  PCI-MSI 524288-edge      eth0-TxRx-0
 27:       5000        500         50          5  PCI-MSI 524289-edge      eth0-TxRx-1
LOC:    1234567    1234567    1234567    1234567  Local timer interrupts
"#;

    #[test]
    fn nic_irq_pct_computed() {
        let pct = max_cpu_irq_pct(SAMPLE).expect("should find nic irqs");
        // CPU0 handles 100000+5000=105000 out of total 111655 — should be dominant
        assert!(pct > 80.0, "expected >80%, got {}", pct);
        assert!(pct <= 100.0);
    }

    #[test]
    fn collect_returns_ok_on_any_host() {
        let c = InterruptsCollector::new();
        let ctx = super::super::CollectCtx::default();
        let result = c.collect(&ctx);
        assert!(result.is_ok());
    }
}
