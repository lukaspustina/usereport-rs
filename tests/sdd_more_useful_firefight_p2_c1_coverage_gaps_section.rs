//! SDD more-useful-firefight Phase 2, C1.
//! GIVEN a config where sar_cpu's binary is not present
//! WHEN a report is generated
//! THEN the rendered output contains "Coverage Gaps" and "sar_cpu".

use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

fn make_skipped(name: &str, binary: &str) -> CommandResult {
    CommandResult::SkippedMissing {
        command: Command::new(name, binary),
        binary: binary.to_string(),
    }
}

#[test]
fn coverage_gaps_section_appears_for_skipped_command() {
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
    let renderer = TemplateRenderer::new(MARKDOWN);
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("Coverage Gaps"),
        "rendered output must contain 'Coverage Gaps':\n{}",
        s
    );
    assert!(
        s.contains("sar_cpu"),
        "rendered output must mention 'sar_cpu' in Coverage Gaps:\n{}",
        s
    );
}
