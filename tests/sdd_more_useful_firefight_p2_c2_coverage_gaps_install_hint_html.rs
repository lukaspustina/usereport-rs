//! SDD more-useful-firefight Phase 2, C2.
//! GIVEN sar_cpu has install_hint = "apt-get install sysstat" and its binary is absent
//! WHEN the HTML report renders
//! THEN the Coverage Gaps section contains "apt-get install sysstat".

use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::renderer::TemplateRenderer;
use usereport::Renderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn coverage_gaps_install_hint_in_html() {
    let cmd = Command::new("sar_cpu", "sar").with_install_hint("apt-get install sysstat");
    let result = CommandResult::SkippedMissing {
        command: cmd,
        binary: "sar".to_string(),
    };
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![result]],
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
        s.contains("apt-get install sysstat"),
        "HTML Coverage Gaps must show install_hint:\n{}",
        &s[..s.len().min(2000)]
    );
}
