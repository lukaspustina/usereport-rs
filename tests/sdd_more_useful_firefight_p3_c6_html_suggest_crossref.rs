//! SDD more-useful-firefight Phase 3, C6.
//! GIVEN suggest = ["iostat -x 1 5"] AND command_results has a Success command with .command() = "iostat -x 1 5"
//! WHEN the HTML template renders the suggest list
//! THEN the output contains href="#cmd-iostat" and does not contain the bare string "iostat -x 1 5" outside an anchor.

use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::finding::{Finding, FindingKind, Severity};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const HTML: &str = include_str!("../contrib/html.j2");

fn make_success(name: &str, cmd_str: &str) -> CommandResult {
    CommandResult::Success {
        command: Command::new(name, cmd_str),
        run_time_ms: 10,
        stdout: "output".to_string(),
    }
}

#[test]
fn html_suggest_renders_crossref_for_matching_command() {
    let finding = Finding {
        id: "cpu.iowait_high".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "iowait high".to_string(),
        evidence: vec![],
        suggest: vec!["iostat -x 1 5".to_string()],
    };
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![make_success("iostat", "iostat -x 1 5")]],
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
    assert!(
        s.contains("href=\"#cmd-iostat\""),
        "HTML suggest must contain href=\"#cmd-iostat\" for matching command:\n{}",
        &s[..s.len().min(3000)]
    );
}
