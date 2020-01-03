use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::{StructOpt, clap};
use usereport::{command, command::CommandResult, report, report::OutputType, runner, Config, Renderer, Report, Runner};

#[derive(Debug, StructOpt)]
#[structopt(name = "usereport", author, about, setting = clap::AppSettings::ColoredHelp)]
struct Opt {
    /// Configuration from file, or default if not present
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config: Option<PathBuf>,
    /// Output format
    #[structopt(short, long, possible_values = &["json", "markdown"], default_value = "markdown")]
    output_type: OutputType,
    /// Activate debug mode
    #[structopt(short, long)]
    debug: bool,
}

fn main() {
    let opt = Opt::from_args();
    let config =
        opt.config.as_ref().map(Config::from_file).unwrap_or(Config::from_str(defaults::CONFIG))
            .expect("Failed to load config file");

    if opt.debug {
        eprintln!("Options: {:#?}", &opt);
        eprintln!("Configuration: {:#?}", &config);
    }

    let runner = runner::thread::ThreadRunner::new(config.commands);
    let results = runner
        .run()
        .expect("Failed to run commands")
        .into_iter()
        .collect::<command::Result<Vec<CommandResult>>>()
        .expect("Some commands failed");

    let report = Report::new(&results)
        .expect("Failed to create report");

    let stdout = std::io::stdout();
    let handle = stdout.lock();
    render(&report, opt.output_type, handle)
        .expect("Failed to render to stdout");
}

fn render<W: Write>(report: &Report, output_type: OutputType, writer: W) -> report::Result<()> {
    match output_type {
        OutputType::Markdown => report::MdRenderer::new(defaults::MD_TEMPLATE).render(&report, writer),
        OutputType::JSON => report::JsonRenderer::new().render(&report, writer),
    }
}

#[cfg(target_os = "macos")]
mod defaults {
    pub(crate) static CONFIG: &str = include_str!("../../contrib/osx.conf");
    pub(crate) static MD_TEMPLATE: &str = include_str!("../../contrib/markdown.hbs");
}

#[cfg(target_os = "linux")]
mod defaults {
    pub(crate) static CONFIG: &str = include_str!("../../contrib/linux.conf");
    pub(crate) static MD_TEMPLATE: &str = include_str!("../../contrib/markdown.hbs");
}

