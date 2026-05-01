//! Integration tests for SDD `specs/done/sdd/version-2.md` Phase 6 (LLM output + skill).
#![cfg(feature = "bin")]

// ---------------------------------------------------------------------------
// Criterion 1 — LlmOutput struct has required fields
// ---------------------------------------------------------------------------

#[test]
fn ac_phase6_1_llm_output_required_fields_serialize() {
    use usereport::llm::{LlmHost, LlmOutput};

    let output = LlmOutput {
        schema_version: "1".to_string(),
        host: LlmHost {
            hostname: "myhost".to_string(),
            kernel: "Linux 6.1.0".to_string(),
            cpu_count: 4,
            mem_total_bytes: 8_589_934_592,
            load_avg_1m: 0.5,
        },
        signals: vec![],
        findings: vec![],
        checked_ok: vec![],
        raw_excerpts: vec![],
    };

    let json = serde_json::to_string(&output).expect("serialize ok");
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

    assert_eq!(v["schema_version"], "1", "schema_version must be '1'");
    assert!(v["host"].is_object(), "host must be an object");
    assert_eq!(v["host"]["hostname"], "myhost");
    assert!(v["signals"].is_array(), "signals must be array");
    assert!(v["findings"].is_array(), "findings must be array");
    assert!(v["checked_ok"].is_array(), "checked_ok must be array");
    assert!(v["raw_excerpts"].is_array(), "raw_excerpts must be array");
}

// ---------------------------------------------------------------------------
// Criterion 2 — --output llm CLI flag produces LLM JSON
// ---------------------------------------------------------------------------

#[test]
fn ac_phase6_2_cli_output_llm_produces_json() {
    let bin = env!("CARGO_BIN_EXE_usereport");
    let output = std::process::Command::new(bin)
        .args(["--output", "llm"])
        .output()
        .expect("run binary");

    assert!(output.status.success(), "exit 0 expected; stderr: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON output");

    assert_eq!(v["schema_version"], "1", "schema_version must be '1'");
    assert!(v.get("host").is_some(), "missing 'host' key");
    assert!(v.get("signals").is_some(), "missing 'signals' key");
    assert!(v.get("findings").is_some(), "missing 'findings' key");
    assert!(v.get("checked_ok").is_some(), "missing 'checked_ok' key");
    assert!(v.get("raw_excerpts").is_some(), "missing 'raw_excerpts' key");
}

// ---------------------------------------------------------------------------
// Criterion 3 — Redaction is deterministic (same salt + input → same hash)
// ---------------------------------------------------------------------------

#[test]
fn ac_phase6_3_redaction_is_deterministic() {
    use usereport::redact::Redactor;

    let r = Redactor::with_salt(b"test-salt-for-tests");
    let h1 = r.redact_value("myhost.example.com");
    let h2 = r.redact_value("myhost.example.com");
    assert_eq!(h1, h2, "same input must produce same hash");

    let h3 = r.redact_value("other.example.com");
    assert_ne!(h1, h3, "different inputs must produce different hashes");
}

// ---------------------------------------------------------------------------
// Criterion 4 — Redacted LlmOutput: hostname/IP replaced (not raw)
// ---------------------------------------------------------------------------

#[test]
fn ac_phase6_4_redact_replaces_hostname_and_ip() {
    use usereport::llm::{LlmHost, LlmOutput};
    use usereport::redact::Redactor;

    let output = LlmOutput {
        schema_version: "1".to_string(),
        host: LlmHost {
            hostname: "prod-server.internal".to_string(),
            kernel: "Linux 6.1.0".to_string(),
            cpu_count: 8,
            mem_total_bytes: 16_000_000_000,
            load_avg_1m: 1.2,
        },
        signals: vec![],
        findings: vec![],
        checked_ok: vec![],
        raw_excerpts: vec!["OOM kill on 192.168.1.100".to_string()],
    };

    let redactor = Redactor::with_salt(b"test-salt-for-tests");
    let redacted = redactor.redact_output(output);

    assert_ne!(redacted.host.hostname, "prod-server.internal", "hostname must be hashed");
    // raw_excerpts containing 192.168.1.100 should have IP replaced
    let excerpts_json = serde_json::to_string(&redacted.raw_excerpts).unwrap();
    assert!(
        !excerpts_json.contains("192.168.1.100"),
        "IPv4 address must be redacted; got: {}",
        excerpts_json
    );
}

// ---------------------------------------------------------------------------
// Criterion 5 — docs/schemas/llm-output-v1.json exists and is valid JSON
// ---------------------------------------------------------------------------

const LLM_SCHEMA: &str = include_str!("../docs/schemas/llm-output-v1.json");

#[test]
fn ac_phase6_5_json_schema_file_is_valid_json() {
    let v: serde_json::Value = serde_json::from_str(LLM_SCHEMA).expect("llm-output-v1.json must be valid JSON");
    assert_eq!(v["$schema"], "http://json-schema.org/draft-07/schema#", "must be draft-07");
    assert!(v.get("properties").is_some(), "schema must have properties");
}

// ---------------------------------------------------------------------------
// Criterion 6 — LlmOutput JSON has schema_version = "1"
// ---------------------------------------------------------------------------

#[test]
fn ac_phase6_6_schema_version_is_1() {
    use usereport::llm::{LlmHost, LlmOutput};

    let output = LlmOutput {
        schema_version: "1".to_string(),
        host: LlmHost {
            hostname: "h".to_string(),
            kernel: "k".to_string(),
            cpu_count: 1,
            mem_total_bytes: 0,
            load_avg_1m: 0.0,
        },
        signals: vec![],
        findings: vec![],
        checked_ok: vec![],
        raw_excerpts: vec![],
    };

    let json = serde_json::to_string(&output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["schema_version"].as_str().unwrap(), "1");
}

// ---------------------------------------------------------------------------
// Criterion 7 — SKILL.md exists with required constraints
// ---------------------------------------------------------------------------

const SKILL_MD: &str = include_str!("../skills/usereport-analyze/SKILL.md");

#[test]
fn ac_phase6_7_skill_md_has_never_fabricate_constraint() {
    assert!(
        SKILL_MD.contains("never fabricate") || SKILL_MD.contains("Never fabricate"),
        "SKILL.md must contain 'never fabricate' constraint"
    );
}

#[test]
fn ac_phase6_7_skill_md_has_schema_version_check() {
    assert!(
        SKILL_MD.contains("schema_version") && (SKILL_MD.contains("\"1\"") || SKILL_MD.contains("'1'")),
        "SKILL.md must reference schema_version check"
    );
}

// ---------------------------------------------------------------------------
// Criterion 8 — 5 fixture pairs exist in skills/usereport-analyze/fixtures/
// ---------------------------------------------------------------------------

const FIXTURE_GOOD_BOX_INPUT: &str = include_str!("../skills/usereport-analyze/fixtures/good-box/input.json");
const FIXTURE_MEM_PRESSURE_INPUT: &str =
    include_str!("../skills/usereport-analyze/fixtures/memory-pressure/input.json");
const FIXTURE_IO_BOUND_INPUT: &str = include_str!("../skills/usereport-analyze/fixtures/io-bound/input.json");
const FIXTURE_THERMAL_INPUT: &str =
    include_str!("../skills/usereport-analyze/fixtures/thermal-throttle/input.json");
const FIXTURE_TIME_WAIT_INPUT: &str =
    include_str!("../skills/usereport-analyze/fixtures/time-wait-exhaustion/input.json");

const FIXTURE_GOOD_BOX_REF: &str = include_str!("../skills/usereport-analyze/fixtures/good-box/reference.md");
const FIXTURE_MEM_PRESSURE_REF: &str =
    include_str!("../skills/usereport-analyze/fixtures/memory-pressure/reference.md");
const FIXTURE_IO_BOUND_REF: &str = include_str!("../skills/usereport-analyze/fixtures/io-bound/reference.md");
const FIXTURE_THERMAL_REF: &str =
    include_str!("../skills/usereport-analyze/fixtures/thermal-throttle/reference.md");
const FIXTURE_TIME_WAIT_REF: &str =
    include_str!("../skills/usereport-analyze/fixtures/time-wait-exhaustion/reference.md");

#[test]
fn ac_phase6_8_fixture_input_files_are_valid_json_with_schema_version() {
    for (name, content) in [
        ("good-box", FIXTURE_GOOD_BOX_INPUT),
        ("memory-pressure", FIXTURE_MEM_PRESSURE_INPUT),
        ("io-bound", FIXTURE_IO_BOUND_INPUT),
        ("thermal-throttle", FIXTURE_THERMAL_INPUT),
        ("time-wait-exhaustion", FIXTURE_TIME_WAIT_INPUT),
    ] {
        let v: serde_json::Value =
            serde_json::from_str(content).unwrap_or_else(|e| panic!("fixture '{}/input.json' invalid JSON: {}", name, e));
        assert_eq!(
            v["schema_version"].as_str().unwrap_or(""),
            "1",
            "fixture '{}/input.json' must have schema_version='1'",
            name
        );
    }
}

#[test]
fn ac_phase6_8_fixture_reference_files_are_nonempty() {
    for (name, content) in [
        ("good-box", FIXTURE_GOOD_BOX_REF),
        ("memory-pressure", FIXTURE_MEM_PRESSURE_REF),
        ("io-bound", FIXTURE_IO_BOUND_REF),
        ("thermal-throttle", FIXTURE_THERMAL_REF),
        ("time-wait-exhaustion", FIXTURE_TIME_WAIT_REF),
    ] {
        assert!(
            !content.trim().is_empty(),
            "fixture '{}/reference.md' must not be empty",
            name
        );
    }
}
