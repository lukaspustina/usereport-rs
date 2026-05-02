//! SDD more-useful-firefight Phase 3, C3.
//! GIVEN a command named iostat in command_results
//! WHEN the HTML template renders the command section heading
//! THEN the heading element contains id="cmd-iostat".

use usereport::Renderer;
use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
use usereport::renderer::TemplateRenderer;

const HTML: &str = include_str!("../contrib/html.j2");

#[test]
fn html_command_section_has_id_attribute() {
    let result = CommandResult::Success {
        command: Command::new("iostat", "iostat"),
        stdout: "some output".to_string(),
        run_time_ms: 10,
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
        s.contains("id=\"cmd-iostat\""),
        "HTML command heading must have id=\"cmd-iostat\":\n{}",
        &s[..s.len().min(4000)]
    );
}
