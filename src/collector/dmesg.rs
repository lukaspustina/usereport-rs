//! Structured dmesg event parser (SDD Req 19).
//!
//! Each detected event type becomes a `Signal` with a count value. The seven
//! event types are: OOM kills, segfaults, blocked-task warnings, machine check
//! exceptions (MCEs), EXT4/XFS filesystem errors, NIC link flaps, and
//! blk_update_request I/O errors.

use chrono::Local;
use regex::Regex;
use std::sync::OnceLock;

use crate::collector::{CollectCtx, Result};
use crate::signal::{Signal, SignalValue, Unit};

#[derive(Debug, Default)]
pub struct DmesgCollector;

impl DmesgCollector {
    pub fn new() -> Self {
        Self
    }

    /// Parse a dmesg text blob and return count signals for all 7 event types.
    pub fn parse(text: &str) -> Vec<Signal> {
        let now = Local::now();
        let counts = [
            ("dmesg.oom_count", oom_count(text)),
            ("dmesg.blocked_task_count", blocked_task_count(text)),
            ("dmesg.fs_error_count", fs_error_count(text)),
            ("dmesg.segfault_count", segfault_count(text)),
            ("dmesg.mce_count", mce_count(text)),
            ("dmesg.nic_flap_count", nic_flap_count(text)),
            ("dmesg.io_error_count", io_error_count(text)),
        ];

        counts
            .iter()
            .map(|(id, count)| Signal {
                id: id.to_string(),
                value: SignalValue::F64(*count as f64),
                unit: Unit::None,
                at: now,
                samples: None,
                stats: None,
                baseline: None,
            })
            .collect()
    }
}

impl super::Collector for DmesgCollector {
    fn id(&self) -> &str {
        "dmesg"
    }

    fn collect(&self, _ctx: &CollectCtx) -> Result<Vec<Signal>> {
        let text = match std::process::Command::new("dmesg").output() {
            Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
            Err(_) => return Ok(Self::parse("")),
        };
        Ok(Self::parse(&text))
    }
}

// ---------------------------------------------------------------------------
// Pattern matchers — one per event type
// ---------------------------------------------------------------------------

fn count_lines(text: &str, re: &Regex) -> usize {
    text.lines().filter(|l| re.is_match(l)).count()
}

static OOM_RE: OnceLock<Regex> = OnceLock::new();
static BLOCKED_RE: OnceLock<Regex> = OnceLock::new();
static FS_ERROR_RE: OnceLock<Regex> = OnceLock::new();
static SEGFAULT_RE: OnceLock<Regex> = OnceLock::new();
static MCE_RE: OnceLock<Regex> = OnceLock::new();
static NIC_FLAP_RE: OnceLock<Regex> = OnceLock::new();
static IO_ERROR_RE: OnceLock<Regex> = OnceLock::new();

fn oom_count(text: &str) -> usize {
    let re = OOM_RE.get_or_init(|| Regex::new(r"(?i)(Out of memory|Killed process \d+|oom_kill)").unwrap());
    count_lines(text, re)
}

fn blocked_task_count(text: &str) -> usize {
    let re = BLOCKED_RE.get_or_init(|| Regex::new(r"blocked for more than \d+ seconds").unwrap());
    count_lines(text, re)
}

fn fs_error_count(text: &str) -> usize {
    let re = FS_ERROR_RE.get_or_init(|| Regex::new(r"(?i)(EXT4-fs error|XFS.*error|xfs_do_force_shutdown)").unwrap());
    count_lines(text, re)
}

fn segfault_count(text: &str) -> usize {
    let re = SEGFAULT_RE.get_or_init(|| Regex::new(r"segfault at ").unwrap());
    count_lines(text, re)
}

fn mce_count(text: &str) -> usize {
    let re = MCE_RE.get_or_init(|| Regex::new(r"(?i)(mce:|Machine check events|EDAC MC\d+:.*CE )").unwrap());
    count_lines(text, re)
}

fn nic_flap_count(text: &str) -> usize {
    let re = NIC_FLAP_RE.get_or_init(|| Regex::new(r"(?i)(Link is (Down|Up)|NIC Link is)").unwrap());
    count_lines(text, re)
}

fn io_error_count(text: &str) -> usize {
    let re = IO_ERROR_RE.get_or_init(|| Regex::new(r"blk_update_request: I/O error").unwrap());
    count_lines(text, re)
}
