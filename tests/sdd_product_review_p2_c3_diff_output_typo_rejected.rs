//! SDD product-review Phase 2, C3.
//! GIVEN `usereport diff a.json b.json --output jsn` WHEN the command runs
//! THEN exit code is non-zero AND stderr contains `jsn` AND at least one
//! valid format name such as `json` or `text`.
//!
//! This test FAILS today because Diff.output is a plain String and accepts
//! any value without validation. It will pass once OutputType::Text is added
//! and Diff.output is changed to OutputType with value_enum validation.
#![cfg(feature = "bin")]

use std::io::Write as _;

#[test]
fn diff_output_typo_rejected() {
    let report_json = serde_json::json!({
        "schema_version": "1",
        "context": {
            "hostname": "host",
            "uname": "Linux host 5.15",
            "date_time": "2026-01-01T00:00:00+00:00",
            "more": {}
        },
        "hostinfo_results": [],
        "command_results": [[]],
        "repetitions": 1,
        "max_parallel_commands": 1,
        "signals": [],
        "findings": [],
        "checked_ok": [],
        "vital_signs": {
            "cpu": {"iowait_pct": null, "severity": null, "trend": null},
            "memory": {"used_pct": null, "severity": null},
            "disk": {"util_pct": null, "severity": null},
            "network": {"util_pct": null, "severity": null}
        },
        "use_coverage": [],
        "followup_recommendations": [],
        "signal_thresholds": {}
    });

    let tmp = tempfile::tempdir().expect("create tempdir");
    let a_path = tmp.path().join("a.json");
    let b_path = tmp.path().join("b.json");
    for p in [&a_path, &b_path] {
        let mut f = std::fs::File::create(p).expect("create file");
        f.write_all(report_json.to_string().as_bytes()).expect("write file");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args([
            "diff",
            a_path.to_str().unwrap(),
            b_path.to_str().unwrap(),
            "--output",
            "jsn",
        ])
        .output()
        .expect("run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit for invalid --output value; stderr: {stderr}"
    );
    assert!(
        stderr.contains("jsn"),
        "expected stderr to mention the invalid value 'jsn'; got: {stderr}"
    );
    assert!(
        stderr.contains("json") || stderr.contains("text"),
        "expected stderr to list valid format names; got: {stderr}"
    );
}
