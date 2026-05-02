//! SDD more-useful-firefight Phase 4, C3.
//! GIVEN all commands with use_dimension.resource = "network" returned SkippedMissing
//! WHEN use_coverage is computed
//! THEN all 3 entries with resource = "network" have covered = false.

use usereport::analysis::compute_use_coverage;
use usereport::command::{Command, CommandResult, UseDimension};

fn make_skipped_network(name: &str, cmd: &str, aspect: &str) -> CommandResult {
    CommandResult::SkippedMissing {
        command: Command::new(name, cmd).with_use_dimension(UseDimension {
            resource: "network".to_string(),
            aspect: aspect.to_string(),
        }),
        binary: cmd.to_string(),
    }
}

#[test]
fn use_coverage_network_all_false_when_skipped() {
    let results = vec![
        make_skipped_network("sar_dev", "sar -n DEV 1 1", "utilization"),
        make_skipped_network("sar_tcp", "sar -n TCP 1 1", "saturation"),
        make_skipped_network("sar_edev", "sar -n EDEV 1 1", "errors"),
    ];
    let coverage = compute_use_coverage(&results);
    let network_entries: Vec<_> = coverage.iter().filter(|e| e.resource == "network").collect();
    assert!(
        !network_entries.is_empty(),
        "use_coverage must contain network entries"
    );
    for entry in &network_entries {
        assert!(
            !entry.covered,
            "network entry {:?} must have covered=false when all skipped",
            entry.aspect
        );
    }
}
