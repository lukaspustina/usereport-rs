use exitfailure::ExitFailure;
use failure::{format_err, ResultExt};
use indicatif::{ProgressBar, ProgressStyle};
use prettytable::{cell, format, row, Cell, Row, Table};
use std::{
    io::Write,
    path::PathBuf,
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
};
use structopt::{clap, StructOpt};
use usereport::{renderer, runner::ThreadRunner, Analysis, Config, Renderer};

#[derive(Debug, StructOpt)]
#[structopt(name = "usereport", author, about, setting = clap::AppSettings::ColoredHelp)]
struct Opt {
    /// Configuration from file, or default if not present
    #[structopt(short, long, parse(from_os_str))]
    config:          Option<PathBuf>,
    /// Output format
    #[structopt(short, long, possible_values = & ["hbs", "json", "markdown"], default_value = "markdown")]
    output_type:     OutputType,
    /// Set output template if output-type is set to "hbs"
    #[structopt(long)]
    output_template: Option<String>,
    /// Set number of commands to run in parallel; overrides setting from config file
    #[structopt(long)]
    parallel:        Option<usize>,
    /// Set number of how many times to run commands in row; overrides setting from config file
    #[structopt(long)]
    repetitions:     Option<usize>,
    /// Show progress bar while waiting for all commands to finish
    #[structopt(short = "P", long)]
    progress:        bool,
    /// Activate debug mode
    #[structopt(short, long)]
    debug:           bool,
    /// Set profile to use
    #[structopt(short = "p", long)]
    profile:         Option<String>,
    /// Show active config
    #[structopt(long)]
    show_config:     bool,
    /// Show available profiles
    #[structopt(long)]
    show_profiles:   bool,
    /// Show available commands
    #[structopt(long)]
    show_commands:   bool,
}

impl Opt {
    pub fn validate(self) -> Result<Self, failure::Error> {
        if self.output_type == OutputType::Hbs && self.output_template.is_none() {
            return Err(format_err!("Output type hbs requires output template"));
        }

        Ok(self)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum OutputType {
    Hbs,
    JSON,
    Markdown,
}

impl FromStr for OutputType {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "hbs" => Ok(OutputType::Hbs),
            "json" => Ok(OutputType::JSON),
            "markdown" => Ok(OutputType::Markdown),
            _ => Err(format_err!("failed to parse {} as output type", s)),
        }
    }
}

fn main() -> Result<(), ExitFailure> {
    human_panic::setup_panic!();
    env_logger::init();

    let opt = Opt::from_args().validate()?;
    let config = opt
        .config
        .as_ref()
        .map(Config::from_file)
        .unwrap_or_else(|| Config::from_str(defaults::CONFIG))
        .with_context(|_| "could not load configuration file")?;
    config.validate()?;
    let profile_name = opt.profile.as_ref().unwrap_or(&config.defaults.profile);

    if opt.debug {
        eprintln!("Options: {:#?}", &opt);
        eprintln!("Configuration: {:#?}", &config);
        eprintln!("Using profile '{}'", profile_name);
    }

    if opt.show_config {
        show_config(&config);
        return Ok(());
    }
    if opt.show_profiles {
        show_profiles(&config);
        return Ok(());
    }
    if opt.show_commands {
        show_commands(&config);
        return Ok(());
    }

    generate_report(&opt, &config, profile_name)
}

fn show_config(config: &Config) {
    let toml = toml::to_string_pretty(config).expect("failed to serialize active config in TOML");
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(toml.as_bytes()).expect("failed write TOML to stdout");
}

fn show_profiles(config: &Config) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row!["Name", "Commands", "Description"]);
    for p in &config.profiles {
        table.add_row(Row::new(vec![
            Cell::new(&p.name),
            Cell::new(&p.commands.as_slice().join("\n")),
            Cell::new(&p.description.as_ref().map(|x| x.as_str()).unwrap_or("-")),
        ]));
    }
    table.printstd();
}

fn show_commands(config: &Config) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row!["Name", "Command", "Title", "Description"]);
    for c in &config.commands {
        table.add_row(Row::new(vec![
            Cell::new(&c.name()),
            Cell::new(&c.command()),
            Cell::new(&c.title().unwrap_or("-")),
        ]));
    }
    table.printstd();
}

fn generate_report(opt: &Opt, config: &Config, profile_name: &str) -> Result<(), ExitFailure> {
    let parallel = opt.parallel.unwrap_or(config.defaults.max_parallel_commands);
    let repetitions = opt.repetitions.unwrap_or(config.defaults.repetitions);
    // Create renderer early to detect misconfiguration early
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    let renderer = create_renderer(&opt.output_type, opt.output_template.as_ref())?;

    let hostinfo = config.commands_for_hostinfo();
    let commands = config
        .profile(profile_name)
        .and_then(|p| Ok(config.commands_for_profile(p)))?;
    let number_of_commands = hostinfo.len() + repetitions * commands.len();

    let runner = create_runner(opt.progress, number_of_commands);
    let analysis = Analysis::new(Box::new(runner), &hostinfo, &commands)
        .with_max_parallel_commands(parallel)
        .with_repetitions(repetitions);
    let report = analysis.run().with_context(|_| "failed to run analysis")?;

    renderer
        .render(&report, handle)
        .with_context(|_| "failed to render report")?;

    Ok(())
}

fn create_renderer<W: Write>(
    output_type: &OutputType,
    output_template: Option<&String>,
) -> Result<Box<dyn Renderer<W>>, ExitFailure> {
    let renderer: Box<dyn Renderer<W>> = match output_type {
        OutputType::Hbs => {
            let template_file = output_template.expect("output type hbs requires output template");
            let renderer = renderer::HbsRenderer::from_file(template_file)?;
            Box::new(renderer)
        }
        OutputType::Markdown => Box::new(renderer::HbsRenderer::new(defaults::MD_TEMPLATE)),
        OutputType::JSON => Box::new(renderer::JsonRenderer::new()),
    };

    Ok(renderer)
}

fn create_runner(progress: bool, number_of_commands: usize) -> ThreadRunner {
    let mut runner = ThreadRunner::new();
    if progress {
        let tx = create_progress_bar(number_of_commands);
        runner = runner.with_progress(tx);
    }

    runner
}

fn create_progress_bar(expected: usize) -> Sender<usize> {
    let (tx, rx): (Sender<usize>, Receiver<usize>) = mpsc::channel();
    let pb = ProgressBar::new(expected as u64)
        .with_style(ProgressStyle::default_bar().template("Running commands {bar:40.cyan/blue} {pos}/{len}"));

    let _ = std::thread::Builder::new().name("Progress".to_string()).spawn(move || {
        for _ in 0..expected {
            let _ = rx.recv().expect("Thread failed to receive progress via channel");
            pb.inc(1);
        }
        pb.finish_and_clear();
    });

    tx
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
