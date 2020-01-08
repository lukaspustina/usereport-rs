use usereport::{command, command::CommandResult, report, runner, Config, Renderer, Report, Runner, Analysis};

fn main() {
    #[cfg(target_os = "macos")]
    let config = Config::from_file("contrib/osx.conf").expect("Failed to load config file");
    #[cfg(target_os = "linux")]
    let config = Config::from_file("contrib/linux.conf").expect("Failed to load config file");

    let runner = runner::ThreadRunner::new();
    let hostinfos = config.commands_for_hostinfo();
    let analysis = Analysis::new(Box::new(runner), &hostinfos, &config.commands);
    let analysis_results = analysis.run()
        .expect("failed to run analysis");

    let report = Report::new(&analysis_results);;
    let renderer = report::JsonRenderer::new();

    let stdout = std::io::stdout();
    let handle = stdout.lock();
    renderer.render(&report, handle).expect("Failed to render to stdout");
}
