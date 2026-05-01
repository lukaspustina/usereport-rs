//! HMAC-SHA-256 redaction for LLM output (SDD Req 22).
//!
//! Redaction is deterministic: the same `(salt, value)` pair always produces
//! the same hex hash. The salt comes from `USEREPORT_REDACT_SALT` env var or
//! falls back to a compile-time constant (provides weak privacy — same host
//! always hashes identically, but hashes are not secret).

use hmac::{Hmac, Mac};
use regex::Regex;
use sha2::Sha256;
use std::sync::OnceLock;

use crate::llm::LlmOutput;
use crate::signal::SignalValue;

const FALLBACK_SALT: &[u8] = b"usereport-default-redaction-salt-v1";

type HmacSha256 = Hmac<Sha256>;

static IPV4_RE: OnceLock<Regex> = OnceLock::new();
static IPV6_RE: OnceLock<Regex> = OnceLock::new();
static MAC_RE: OnceLock<Regex> = OnceLock::new();

pub struct Redactor {
    salt: Vec<u8>,
}

impl Redactor {
    /// Build a `Redactor` keyed on `salt`.
    pub fn with_salt(salt: &[u8]) -> Self {
        Self { salt: salt.to_vec() }
    }

    /// Build from `USEREPORT_REDACT_SALT` env var; falls back to compile-time constant.
    pub fn from_env() -> Self {
        let salt = std::env::var("USEREPORT_REDACT_SALT")
            .map(|s| s.into_bytes())
            .unwrap_or_else(|_| FALLBACK_SALT.to_vec());
        Self { salt }
    }

    /// HMAC-SHA-256(salt, value) → first 16 bytes hex-encoded (deterministic).
    pub fn redact_value(&self, value: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.salt).expect("HMAC accepts any key length");
        mac.update(value.as_bytes());
        let result = mac.finalize().into_bytes();
        result[..16].iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Replace IPv4, IPv6, and MAC address patterns in `text` with their hashes.
    pub fn redact_text(&self, text: &str) -> String {
        let ipv4 = IPV4_RE.get_or_init(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap());
        let ipv6 = IPV6_RE.get_or_init(|| Regex::new(r"\b(?:[0-9A-Fa-f]{1,4}:){2,7}[0-9A-Fa-f]{1,4}\b").unwrap());
        let mac = MAC_RE.get_or_init(|| Regex::new(r"\b(?:[0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}\b").unwrap());

        let mut result = text.to_string();
        for re in [ipv4, ipv6, mac] {
            let replaced = re.replace_all(&result, |caps: &regex::Captures| {
                format!("[redacted:{}]", &self.redact_value(&caps[0]))
            });
            result = replaced.into_owned();
        }
        result
    }

    /// Return a new `LlmOutput` with PII replaced per SDD Req 22.
    ///
    /// Redacted: hostname, IP addresses and MACs inside `raw_excerpts`,
    /// `findings[].summary`, `findings[].evidence[].observed` (Text values),
    /// and `signals[].value` (Text values).
    pub fn redact_output(&self, mut output: LlmOutput) -> LlmOutput {
        output.host.hostname = self.redact_value(&output.host.hostname);
        output.raw_excerpts = output
            .raw_excerpts
            .into_iter()
            .map(|line| self.redact_text(&line))
            .collect();

        for finding in &mut output.findings {
            finding.summary = self.redact_text(&finding.summary);
            for ev in &mut finding.evidence {
                if let SignalValue::Text(ref s) = ev.observed {
                    ev.observed = SignalValue::Text(self.redact_text(s));
                }
            }
        }

        for signal in &mut output.signals {
            if let SignalValue::Text(ref s) = signal.value {
                signal.value = SignalValue::Text(self.redact_text(s));
            }
        }

        output
    }
}
