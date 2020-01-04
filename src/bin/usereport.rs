use exitfailure::ExitFailure;
use failure::ResultExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use structopt::{StructOpt, clap};
use usereport::{command, command::CommandResult, report, report::OutputType, runner, Config, Renderer, Report, Runner};

#[derive(Debug, StructOpt)]
#[structopt(name = "usereport", author, about, setting = clap::AppSettings::ColoredHelp)]
struct Opt {
    /// Configuration from file, or default if not present
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,
    /// Output format
    #[structopt(short, long, possible_values = &["json", "markdown"], default_value = "markdown")]
    output_type: OutputType,
    /// Show progress bar while waiting for all commands to finish
    #[structopt(short, long)]
    progress: bool,
    /// Activate debug mode
    #[structopt(short, long)]
    debug: bool,
}

fn main() -> Result<(), ExitFailure>{
    human_panic::setup_panic!();
    env_logger::init();

    let opt = Opt::from_args();
    let config = opt.config.as_ref()
        .map(Config::from_file)
        .unwrap_or(Config::from_str(defaults::CONFIG))
        .with_context(|_| "could not load configuration file")?;

    if opt.debug {
        eprintln!("Options: {:#?}", &opt);
        eprintln!("Configuration: {:#?}", &config);
    }

    let results = create_runner(&opt, config)
        .run()
        .with_context(|_| "failed to execute commands")?
        .into_iter()
        .collect::<command::Result<Vec<CommandResult>>>()
        .with_context(|_| "failed to execute some commands")?;

    let report = Report::new(&results)
        .with_context(|_| "failed to create report")?;
    render(&report, opt.output_type)
        .with_context(|_| "failed to render report")?;

    Ok(())
}

fn create_runner(opt: &Opt, config: Config) -> runner::ThreadRunner {
    if opt.progress {
        let tx = create_progress_bar(config.commands.len());
         runner::ThreadRunner::with_progress(config.commands, tx)
    } else {
        runner::ThreadRunner::new(config.commands)
    }
}

fn create_progress_bar(expected: usize) -> Sender<usize> {
    let (tx, rx): (Sender<usize>, Receiver<usize>) = mpsc::channel();
    let pb = ProgressBar::new(expected as u64)
        .with_style(ProgressStyle::default_bar()
            .template("Running commands {bar:40.cyan/blue} {pos}/{len}")
        );

    let _ = std::thread::Builder::new()
        .name("Progress".to_string())
        .spawn(move || {
            for _ in 0..expected {
                let _ = rx.recv().expect("Thread failed to receive progress via channel");
                pb.inc(1);
            }
            pb.finish_and_clear();
        });

    tx
}

fn render(report: &Report, output_type: OutputType) -> report::Result<()> {
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    match output_type {
        OutputType::Markdown => report::MdRenderer::new(defaults::MD_TEMPLATE).render(&report, handle),
        OutputType::JSON => report::JsonRenderer::new().render(&report, handle),
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

