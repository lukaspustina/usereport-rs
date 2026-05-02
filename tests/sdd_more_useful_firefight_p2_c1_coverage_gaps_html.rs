//! SDD more-useful-firefight Phase 2, C1.
//! GIVEN a config where sar_cpu's binary is not present
//! WHEN a report is generated with HTML output
//! THEN the rendered output contains "Coverage Gaps" and "sar_cpu".

use usereport::Renderer;
use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::renderer::TemplateRenderer;

const HTML: &str = include_str!("../contrib/html.j2");

fn make_skipped(name: &str, binary: &str) -> CommandResult {
    CommandResult::SkippedMissing {
        command: Command::new(name, binary),
        binary: binary.to_string(),
    }
}

#[test]
fn coverage_gaps_section_in_html_for_skipped_command() {
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![make_skipped("sar_cpu", "sar")]],
        1,
        64,
        vec![],
        vec![],
        vec![],
    );
    let renderer = TemplateRenderer::new(HTML);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("Coverage Gaps"),
        "HTML output must contain 'Coverage Gaps':\n{}",
        &s[..s.len().min(2000)]
    );
    assert!(
        s.contains("sar_cpu"),
        "HTML output must mention 'sar_cpu' in Coverage Gaps:\n{}",
        &s[..s.len().min(2000)]
    );
}
