use usereport::{Config, Report, Runner, Renderer, command, report, runner};
use usereport::command::CommandResult;

fn main() {
    #[cfg(target_os = "macos")]
    let config = Config::from_file("contrib/osx.conf").expect("Failed to load config file");
    #[cfg(target_os = "linux")]
    let config = Config::from_file("contrib/linux.conf").expect("Failed to load config file");

    let runner = runner::thread::ThreadRunner::new(config.commands);
    let results = runner
        .run()
        .expect("Failed to run commands")
        .into_iter()
        .collect::<command::Result<Vec<CommandResult>>>()
        .expect("Some commands failed");

    let report = Report::new(&results);
    let json = report::json::JsonRenderer::new(&report);

    let stdout = std::io::stdout();
    let handle = stdout.lock();
    json.render(handle).expect("Failed to render to stdout");
}