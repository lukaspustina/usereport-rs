//! SDD more-useful-firefight Phase 2, C2.
//! GIVEN sar_cpu has install_hint and its binary is absent
//! WHEN the report renders
//! THEN the Coverage Gaps section contains the install_hint.

use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const MARKDOWN: &str = include_str!("../contrib/markdown.j2");

#[test]
fn coverage_gaps_shows_install_hint() {
    let cmd = Command::new("sar_cpu", "sar -u 1 5")
        .with_install_hint("apt-get install sysstat");
    let skipped = CommandResult::SkippedMissing {
        command: cmd,
        binary: "sar".to_string(),
    };
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![skipped]],
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
        s.contains("apt-get install sysstat"),
        "Coverage Gaps must contain install_hint:\n{}",
        s
    );
}
