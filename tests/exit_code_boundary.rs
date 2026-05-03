//! Exit-code boundary tests for `--exit-on` policy (Fix 27).
#![cfg(feature = "bin")]

use usereport::cli::{ExitOn, compute_exit_code};
use usereport::finding::{Finding, FindingKind, Severity};

fn finding(severity: Severity) -> Finding {
    Finding {
        id: "test".to_string(),
        kind: FindingKind::Rule,
        severity,
        summary: "test".to_string(),
        evidence: vec![],
        suggest: vec![],
    }
}

#[test]
fn exit_on_never_always_zero_with_no_findings() {
    assert_eq!(compute_exit_code(ExitOn::Never, &[]), 0);
}

#[test]
fn exit_on_never_always_zero_with_crit() {
    assert_eq!(compute_exit_code(ExitOn::Never, &[finding(Severity::Crit)]), 0);
}

#[test]
fn exit_on_never_always_zero_with_warn() {
    assert_eq!(compute_exit_code(ExitOn::Never, &[finding(Severity::Warn)]), 0);
}

#[test]
fn exit_on_never_always_zero_with_info() {
    assert_eq!(compute_exit_code(ExitOn::Never, &[finding(Severity::Info)]), 0);
}

#[test]
fn exit_on_info_zero_with_no_findings() {
    assert_eq!(compute_exit_code(ExitOn::Info, &[]), 0);
}

#[test]
fn exit_on_info_one_with_info_finding() {
    assert_eq!(compute_exit_code(ExitOn::Info, &[finding(Severity::Info)]), 1);
}

#[test]
fn exit_on_info_one_with_warn_finding() {
    assert_eq!(compute_exit_code(ExitOn::Info, &[finding(Severity::Warn)]), 1);
}

#[test]
fn exit_on_info_one_with_crit_finding() {
    assert_eq!(compute_exit_code(ExitOn::Info, &[finding(Severity::Crit)]), 1);
}

#[test]
fn exit_on_warn_zero_with_no_findings() {
    assert_eq!(compute_exit_code(ExitOn::Warn, &[]), 0);
}

#[test]
fn exit_on_warn_zero_with_info_only() {
    // boundary: Info alone must NOT trigger exit 1 under Warn policy
    assert_eq!(compute_exit_code(ExitOn::Warn, &[finding(Severity::Info)]), 0);
}

#[test]
fn exit_on_warn_one_with_warn_finding() {
    assert_eq!(compute_exit_code(ExitOn::Warn, &[finding(Severity::Warn)]), 1);
}

#[test]
fn exit_on_warn_one_with_crit_finding() {
    assert_eq!(compute_exit_code(ExitOn::Warn, &[finding(Severity::Crit)]), 1);
}

#[test]
fn exit_on_crit_zero_with_no_findings() {
    assert_eq!(compute_exit_code(ExitOn::Crit, &[]), 0);
}

#[test]
fn exit_on_crit_zero_with_warn_only() {
    // boundary: Warn alone must NOT trigger exit code under Crit policy
    assert_eq!(compute_exit_code(ExitOn::Crit, &[finding(Severity::Warn)]), 0);
}

#[test]
fn exit_on_crit_two_with_crit_finding() {
    assert_eq!(compute_exit_code(ExitOn::Crit, &[finding(Severity::Crit)]), 2);
}
