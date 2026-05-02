//! SDD more-useful-firefight Phase 3, C4.
//! GIVEN command_results[0] contains evidence commands ["iostat", "sar_dev"] and
//!   non-evidence commands ["df", "free"]
//! WHEN the HTML template renders
//! THEN the iostat and sar_dev sections appear before the <details> block,
//!   and df and free appear inside <details>.

use usereport::Renderer;
use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::finding::{Evidence, Finding, FindingKind, Severity};
use usereport::renderer::TemplateRenderer;
use usereport::signal::SignalValue;

const HTML: &str = include_str!("../contrib/html.j2");

fn make_success(name: &str) -> CommandResult {
    CommandResult::Success {
        command: Command::new(name, name),
        stdout: format!("{name} output"),
        run_time_ms: 5,
    }
}

#[test]
fn html_evidence_commands_appear_before_details() {
    let finding = Finding {
        id: "disk.util_high".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "disk util high".to_string(),
        evidence: vec![
            Evidence {
                signal_id: "disk.util_pct".to_string(),
                observed: SignalValue::F64(90.0),
                source_commands: vec!["iostat".to_string()],
            },
            Evidence {
                signal_id: "net.drops".to_string(),
                observed: SignalValue::I64(5),
                source_commands: vec!["sar_dev".to_string()],
            },
        ],
        suggest: vec![],
    };
    let results = vec![
        make_success("iostat"),
        make_success("sar_dev"),
        make_success("df"),
        make_success("free"),
    ];
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![results],
        1,
        64,
        vec![],
        vec![finding],
        vec![],
    );
    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();

    let details_pos = s.find("<details>").expect("<details> block must exist");
    let iostat_pos = s.find("id=\"cmd-iostat\"").expect("iostat section must exist");
    let sar_dev_pos = s.find("id=\"cmd-sar_dev\"").expect("sar_dev section must exist");
    let df_pos = s.find("id=\"cmd-df\"").expect("df section must exist");
    let free_pos = s.find("id=\"cmd-free\"").expect("free section must exist");

    assert!(iostat_pos < details_pos, "iostat must appear before <details>");
    assert!(sar_dev_pos < details_pos, "sar_dev must appear before <details>");
    assert!(df_pos > details_pos, "df must appear inside <details>");
    assert!(free_pos > details_pos, "free must appear inside <details>");
}
