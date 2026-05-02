//! SDD more-useful-firefight Phase 3, C4.
//! GIVEN command_results[0] contains evidence commands ["iostat","sar_dev"] and non-evidence ["df","free"]
//! WHEN the HTML template renders
//! THEN iostat and sar_dev sections appear before the <details> block,
//!      and df and free appear inside <details>.

use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::finding::{Evidence, Finding, FindingKind, Severity};
use usereport::renderer::TemplateRenderer;
use usereport::signal::SignalValue;
use usereport::Renderer;

const HTML: &str = include_str!("../contrib/html.j2");

fn make_success(name: &str) -> CommandResult {
    CommandResult::Success {
        command: Command::new(name, name),
        run_time_ms: 10,
        stdout: "output".to_string(),
    }
}

#[test]
fn evidence_commands_appear_before_details_block() {
    let finding = Finding {
        id: "test.finding".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "test".to_string(),
        evidence: vec![
            Evidence {
                signal_id: "disk.util_pct".to_string(),
                observed: SignalValue::F64(95.0),
                source_commands: vec!["iostat".to_string(), "sar_dev".to_string()],
            },
        ],
        suggest: vec![],
    };
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![
            make_success("iostat"),
            make_success("sar_dev"),
            make_success("df"),
            make_success("free"),
        ]],
        1,
        64,
        vec![],
        vec![finding],
        vec![],
    );
    let renderer = TemplateRenderer::new(HTML).with_html_escape();
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();

    let details_pos = s.find("<details").expect("<details> block must appear in HTML");
    let iostat_pos = s.find("id=\"cmd-iostat\"").expect("cmd-iostat heading must appear");
    let sar_dev_pos = s.find("id=\"cmd-sar_dev\"").expect("cmd-sar_dev heading must appear");
    let df_pos = s.find("id=\"cmd-df\"").expect("cmd-df heading must appear");
    let free_pos = s.find("id=\"cmd-free\"").expect("cmd-free heading must appear");

    assert!(
        iostat_pos < details_pos,
        "iostat must appear before <details>: iostat={} details={}",
        iostat_pos,
        details_pos
    );
    assert!(
        sar_dev_pos < details_pos,
        "sar_dev must appear before <details>: sar_dev={} details={}",
        sar_dev_pos,
        details_pos
    );
    assert!(
        df_pos > details_pos,
        "df must appear inside <details>: df={} details={}",
        df_pos,
        details_pos
    );
    assert!(
        free_pos > details_pos,
        "free must appear inside <details>: free={} details={}",
        free_pos,
        details_pos
    );
}
