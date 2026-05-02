//! SDD more-useful-firefight Phase 4, C5.
//! GIVEN sar_tcp ran successfully and has use_dimension = {resource: "network", aspect: "saturation"}
//! WHEN use_coverage is computed
//! THEN the entry {Network, Saturation} has covered = true.

use usereport::UseDimension;
use usereport::analysis::compute_use_coverage;
use usereport::command::{Command, CommandResult};

#[test]
fn use_coverage_network_saturation_covered_when_sar_tcp_succeeded() {
    let cmd = Command::new("sar_tcp", "sar").with_use_dimension(UseDimension {
        resource: "network".to_string(),
        aspect: "saturation".to_string(),
    });
    let results = vec![CommandResult::Success {
        command: cmd,
        stdout: "output".to_string(),
        run_time_ms: 5,
    }];
    let coverage = compute_use_coverage(&results);
    let entry = coverage
        .iter()
        .find(|e| e.resource == "network" && e.aspect == "saturation")
        .expect("network/saturation entry must exist");
    assert!(entry.covered, "network saturation must be covered");
}
