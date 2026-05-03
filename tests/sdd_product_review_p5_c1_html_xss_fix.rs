//! SDD product-review Phase 5, C1.
//! GIVEN an AnalysisReport where a hostinfo error `reason` field is the string
//! `<script>alert(1)</script>` WHEN the report is rendered to HTML using
//! `html.j2` via `TemplateRenderer` THEN the output contains `&lt;script&gt;`
//! AND does not contain a bare `<script>` tag.
//!
//! Verifies the fix introduced in html.j2: `{{ s.reason | e }}`.

use usereport::{AnalysisReport, CommandResult, TemplateRenderer};

#[test]
fn html_xss_hostinfo_error_reason_escaped() {
    use usereport::renderer::Renderer as _;

    let template = include_str!("../contrib/html.j2");
    let renderer = TemplateRenderer::new(template).with_html_escape();

    // Build a minimal AnalysisReport that contains a HostinfoError with a
    // malicious reason string. We can't construct AnalysisReport directly from
    // scratch easily, so we create a real one via its builder path.
    // The minimal approach: use a fake JSON report and deserialize it.
    let xss_payload = "<script>alert(1)</script>";
    let report_json = serde_json::json!({
        "schema_version": "1",
        "context": {
            "hostname": "host",
            "uname": "Linux host 5.15",
            "date_time": "2026-01-01T00:00:00+00:00",
            "more": {}
        },
        "hostinfo_results": [
            {
                "Error": {
                    "command": {
                        "name": "uname",
                        "title": "Host kernel",
                        "description": "kernel version",
                        "command": "uname -a",
                        "timeout": 1,
                        "links": []
                    },
                    "reason": xss_payload
                }
            }
        ],
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

    let report: AnalysisReport = serde_json::from_str(&report_json.to_string())
        .expect("deserialize AnalysisReport");

    let mut buf = Vec::new();
    renderer
        .render(&report, Box::new(&mut buf) as Box<dyn std::io::Write + Send>)
        .expect("render HTML");

    let html = String::from_utf8(buf).expect("valid UTF-8");

    assert!(
        html.contains("&lt;script&gt;"),
        "expected HTML-escaped '&lt;script&gt;' in output; got raw HTML instead"
    );
    assert!(
        !html.contains("<script>"),
        "expected no bare '<script>' tag in rendered HTML (XSS risk)"
    );
}
