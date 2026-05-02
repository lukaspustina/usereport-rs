//! SDD more-useful-firefight Phase 4, C3.
//! GIVEN all commands with use_dimension.resource = "network" returned SkippedMissing
//! WHEN use_coverage is computed
//! THEN all 3 entries with resource = Network have covered = false.

use usereport::UseDimension;
use usereport::analysis::compute_use_coverage;
use usereport::command::{Command, CommandResult};

fn skipped_with_dim(name: &str, resource: &str, aspect: &str) -> CommandResult {
    let cmd = Command::new(name, name).with_use_dimension(UseDimension {
        resource: resource.to_string(),
        aspect: aspect.to_string(),
    });
    CommandResult::SkippedMissing {
        command: cmd,
        binary: name.to_string(),
    }
}

#[test]
fn use_coverage_network_all_skipped_is_not_covered() {
    let results = vec![
        skipped_with_dim("sar_dev", "network", "utilization"),
        skipped_with_dim("sar_tcp", "network", "saturation"),
        skipped_with_dim("sar_edev", "network", "errors"),
    ];
    let coverage = compute_use_coverage(&results);
    let net_entries: Vec<_> = coverage.iter().filter(|e| e.resource == "network").collect();
    assert_eq!(net_entries.len(), 3, "must have 3 network entries");
    for e in net_entries {
        assert!(
            !e.covered,
            "network {} must be uncovered when only SkippedMissing results",
            e.aspect
        );
    }
}
