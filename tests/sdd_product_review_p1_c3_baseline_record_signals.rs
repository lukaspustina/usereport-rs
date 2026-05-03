//! Tests for SDD `specs/sdd/product-review.md` Phase 1, criterion 3.
//! GIVEN the binary is invoked with no `--config` flag (uses builtin default config)
//! WHEN `usereport baseline record --name smoke` is invoked
//! THEN exit code is 0 AND the baseline file on disk contains a `signals` map with
//! at least one key present (non-empty map; value may be zero).
//!
//! This test FAILS today because `run_baseline` calls `store.record(label, &[])` with
//! a hardcoded empty slice — the stored JSON will have `"signals": {}` (empty map).
//! Once the fix records actual signals, the test will PASS.
#![cfg(feature = "bin")]

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize)]
struct BaselineRecord {
    signals: HashMap<String, serde_json::Value>,
}

#[test]
fn baseline_record_stores_non_empty_signals() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let xdg_data_home = tmp.path().to_str().unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_usereport"))
        .args(["baseline", "record", "--name", "smoke"])
        .env("XDG_DATA_HOME", xdg_data_home)
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let baseline_path = tmp.path().join("usereport").join("baselines").join("smoke.json");

    assert!(
        baseline_path.exists(),
        "expected baseline file at {}",
        baseline_path.display()
    );

    let contents = std::fs::read(&baseline_path).expect("read baseline file");
    let record: BaselineRecord = serde_json::from_slice(&contents).expect("parse baseline JSON");

    assert!(
        !record.signals.is_empty(),
        "expected at least one signal in baseline; got empty map"
    );
}
