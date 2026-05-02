//! SDD more-useful-firefight Phase 3, C6.
//! GIVEN suggest = ["iostat -x 1 5"] on a finding AND command_results contains
//!   a non-SkippedMissing command with .command() = "iostat -x 1 5"
//! WHEN the HTML template renders the suggest list
//! THEN the output contains href="#cmd-iostat" and does not contain the bare string
//!   "iostat -x 1 5" outside an anchor tag.

use usereport::Renderer;
use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::finding::{Finding, FindingKind, Severity};
use usereport::renderer::TemplateRenderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn html_suggest_renders_crossref_link() {
    let finding = Finding {
        id: "cpu.iowait_high".to_string(),
        kind: FindingKind::Rule,
        severity: Severity::Warn,
        summary: "iowait high".to_string(),
        evidence: vec![],
        suggest: vec!["iostat -x 1 5".to_string()],
    };
    let cmd = Command::new("iostat", "iostat -x 1 5");
    let result = CommandResult::Success {
        command: cmd,
        stdout: "some output".to_string(),
        run_time_ms: 5,
    };
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![result]],
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
    assert!(
        s.contains("href=\"#cmd-iostat\""),
        "HTML suggest must contain crossref link to cmd-iostat:\n{}",
        &s[..s.len().min(4000)]
    );
}
