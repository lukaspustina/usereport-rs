use usereport::{command, command::CommandResult, report, runner, Config, Renderer, Report, Runner};

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

    let report = Report::new(&results).expect("Failed to create report");
    let renderer = report::JsonRenderer::new();

    let stdout = std::io::stdout();
    let handle = stdout.lock();
    renderer.render(&report, handle).expect("Failed to render to stdout");
}
