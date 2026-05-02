//! SDD more-useful-firefight Phase 3, C3.
//! GIVEN a command named "iostat" in command_results
//! WHEN the HTML template renders the command section heading
//! THEN the heading element contains id="cmd-iostat".

use usereport::analysis::{AnalysisReport, Context};
use usereport::command::{Command, CommandResult};
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
fn html_command_heading_has_id_attribute() {
    let report = AnalysisReport::new_with_diagnostics(
        Context::new(),
        vec![],
        vec![vec![make_success("iostat", "iostat -x 1 5")]],
        1,
        64,
        vec![],
        vec![],
        vec![],
    );
    let renderer = TemplateRenderer::new(HTML).with_html_escape();
    let mut out = Vec::new();
    renderer.render(&report, &mut out).expect("render ok");
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("id=\"cmd-iostat\""),
        "HTML command heading must contain id=\"cmd-iostat\":\n{}",
        &s[..s.len().min(2000)]
    );
}
