//! Built-in rules — bundled at compile time via `include_str!`.
//!
//! The 15 default rules cover CPU, memory, disk, network, and dmesg signal
//! groups (SDD §99). Rules referencing signals not yet emitted in Phase 1
//! (e.g. `dmesg.oom_count`) remain inert: the predicate evaluator returns
//! false on absent signals (SDD §453).

use super::{Rule, parse_rules_toml};

const CPU_RULES: &str = include_str!("../../contrib/rules/cpu.toml");
const MEMORY_RULES: &str = include_str!("../../contrib/rules/memory.toml");
const DISK_RULES: &str = include_str!("../../contrib/rules/disk.toml");
const NETWORK_RULES: &str = include_str!("../../contrib/rules/network.toml");
const DMESG_RULES: &str = include_str!("../../contrib/rules/dmesg.toml");
#[cfg(feature = "bpf")]
const BPF_RULES: &str = include_str!("../../contrib/rules/bpf.toml");

/// Returns the bundled built-in rule set. Panics if a bundled TOML file fails
/// to parse — this is a build-time invariant, not a runtime concern, and a
/// panic here is preferable to silently dropping rules.
pub fn builtin_rules() -> Vec<Rule> {
    let mut rules = Vec::new();
    for (name, src) in [
        ("cpu.toml", CPU_RULES),
        ("memory.toml", MEMORY_RULES),
        ("disk.toml", DISK_RULES),
        ("network.toml", NETWORK_RULES),
        ("dmesg.toml", DMESG_RULES),
    ] {
        match parse_rules_toml(src) {
            Ok(mut more) => rules.append(&mut more),
            Err(e) => panic!("built-in rule file {} failed to parse: {}", name, e),
        }
    }
    rules
}

/// Returns BPF tool-availability rules. Only available with the `bpf` feature.
#[cfg(feature = "bpf")]
pub fn bpf_rules() -> Vec<Rule> {
    match parse_rules_toml(BPF_RULES) {
        Ok(rules) => rules,
        Err(e) => panic!("built-in bpf.toml failed to parse: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_rules_are_parseable_and_at_least_15() {
        let rules = builtin_rules();
        assert!(rules.len() >= 15, "expected >= 15 built-in rules, got {}", rules.len());
    }
}
