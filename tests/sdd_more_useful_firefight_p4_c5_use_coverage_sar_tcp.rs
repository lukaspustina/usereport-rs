//! SDD more-useful-firefight Phase 4, C5.
//! GIVEN sar_tcp ran successfully and has use_dimension = {resource: "network", aspect: "saturation"}
//! WHEN use_coverage is computed
//! THEN the entry {network, saturation} has covered = true.

use usereport::analysis::compute_use_coverage;
use usereport::command::{Command, CommandResult, UseDimension};

#[test]
fn use_coverage_network_saturation_true_when_sar_tcp_succeeded() {
    let cmd = Command::new("sar_tcp", "sar -n TCP 1 1").with_use_dimension(UseDimension {
        resource: "network".to_string(),
        aspect: "saturation".to_string(),
    });
    let results = vec![CommandResult::Success {
        command: cmd,
        run_time_ms: 50,
        stdout: "output".to_string(),
    }];
    let coverage = compute_use_coverage(&results);
    let entry = coverage
        .iter()
        .find(|e| e.resource == "network" && e.aspect == "saturation")
        .expect("network/saturation entry must be present");
    assert!(
        entry.covered,
        "network/saturation must be covered=true when sar_tcp succeeded"
    );
}
