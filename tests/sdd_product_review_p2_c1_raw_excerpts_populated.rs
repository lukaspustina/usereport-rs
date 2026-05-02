//! SDD product-review Phase 2, C1.
//! GIVEN a JSON report where at least one CommandResult::Success has stdout
//! longer than 1000 chars WHEN `usereport convert report.json --output llm`
//! runs THEN raw_excerpts is a non-empty array AND every entry's `output`
//! field is at most 1000 characters.
#![cfg(feature = "bin")]

use std::io::Write as _;

#[test]
fn raw_excerpts_populated_and_truncated() {
    // Build a minimal AnalysisReport JSON with one command result whose stdout
    // is longer than MAX_EXCERPT_CHARS (1000).
    let long_stdout = "x".repeat(2000);
    let report_json = serde_json::json!({
        "schema_version": "1",
        "context": {
            "hostname": "testhost",
            "uname": "Linux testhost 5.15",
            "date_time": "2026-01-01T00:00:00+00:00",
            "more": {}
        },
        "hostinfo_results": [],
        "command_results": [[
            {
                "Success": {
                    "command": {
                        "name": "long_cmd",
                        "title": "Long command",
                        "description": "generates lots of output",
                        "command": "echo lots",
                        "timeout": 1,
                        "links": []
                    },
                    "run_time_ms": 5,
                    "stdout": long_stdout
                }
            }
        ]],
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
    let report_path = tmp.path().join("report.json");
    {
        let mut f = std::fs::File::create(&report_path).expect("create report file");
        f.write_all(report_json.to_string().as_bytes()).expect("write report");
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["convert", report_path.to_str().unwrap(), "--output", "llm"])
        .output()
        .expect("run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {stderr}\nstdout: {stdout}"
    );

    let llm: serde_json::Value = serde_json::from_str(&stdout).expect("parse llm JSON");
    let excerpts = llm
        .get("raw_excerpts")
        .and_then(|e| e.as_array())
        .expect("raw_excerpts must be an array");

    assert!(!excerpts.is_empty(), "expected non-empty raw_excerpts");

    for entry in excerpts {
        let output_str = entry
            .get("output")
            .and_then(|o| o.as_str())
            .expect("each excerpt must have an 'output' string field");
        assert!(
            output_str.len() <= 1000,
            "excerpt output too long: {} chars (max 1000)",
            output_str.len()
        );
    }
}
