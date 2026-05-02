//! SDD product-review Phase 4, C2.
//! GIVEN two valid baseline JSON files `before.json` and `after.json` WHEN
//! `usereport diff before.json after.json` produces text output THEN stdout
//! contains the substring `before.json` AND contains `after.json` as part
//! of section headings.
//!
//! This test FAILS today because render_text uses hardcoded "Before" and
//! "After" strings instead of the filenames.
#![cfg(feature = "bin")]

use std::io::Write as _;

#[test]
fn diff_shows_filenames() {
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
    let before_path = tmp.path().join("before.json");
    let after_path = tmp.path().join("after.json");
    for p in [&before_path, &after_path] {
        let mut f = std::fs::File::create(p).expect("create file");
        f.write_all(report_json.to_string().as_bytes()).expect("write");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["diff", before_path.to_str().unwrap(), after_path.to_str().unwrap()])
        .output()
        .expect("run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {stderr}\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("before.json"),
        "expected stdout to contain 'before.json'; got: {stdout}"
    );
    assert!(
        stdout.contains("after.json"),
        "expected stdout to contain 'after.json'; got: {stdout}"
    );
}
