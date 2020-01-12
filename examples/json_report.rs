use usereport::{Analysis, Config, Renderer, ThreadRunner, JsonRenderer};

fn main() {
    #[cfg(target_os = "macos")]
    let config = Config::from_file("contrib/osx.conf").expect("Failed to load config file");
    #[cfg(target_os = "linux")]
    let config = Config::from_file("contrib/linux.conf").expect("Failed to load config file");

    let runner = ThreadRunner::new();
    let hostinfos = config.commands_for_hostinfo();
    let analysis = Analysis::new(Box::new(runner), &hostinfos, &config.commands);
    let report = analysis.run().expect("failed to run analysis");

    let renderer = JsonRenderer::new();
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    renderer.render(&report, handle).expect("Failed to render to stdout");
}
