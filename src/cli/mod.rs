use crate::{renderer, Analysis, Command, Config, Context, Renderer, ThreadRunner};
use anyhow::{anyhow, Context as _};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use prettytable::{format, row, Cell, Row, Table};
use std::{
    collections::HashSet,
    fs::File,
    io::{IsTerminal, Read, Write},
    path::PathBuf,
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
};

pub mod config;

#[derive(Debug, Parser)]
#[command(author, about)]
struct Opt {
    /// Configuration from file, or default if not present
    #[arg(short, long)]
    config:               Option<PathBuf>,
    /// Output format
    #[arg(short, long, value_enum, default_value = "markdown")]
    output:               OutputType,
    /// Set output template if output is set to "hbs"
    #[arg(long)]
    output_template:      Option<String>,
    /// Set number of commands to run in parallel; overrides setting from config file
    #[arg(long)]
    parallel:             Option<usize>,
    /// Set number of how many times to run commands in row; overrides setting from config file
    #[arg(long)]
    repetitions:          Option<usize>,
    /// Force to show progress bar while waiting for all commands to finish
    #[arg(long, conflicts_with = "no_progress")]
    progress:             bool,
    /// Force to hide progress bar while waiting for all commands to finish
    #[arg(long, conflicts_with = "progress")]
    no_progress:          bool,
    /// Activate debug mode
    #[arg(short, long)]
    debug:                bool,
    /// Set profile to use
    #[arg(short = 'p', long)]
    profile:              Option<String>,
    /// Show active config
    #[arg(long)]
    show_config:          bool,
    /// Show active template
    #[arg(long)]
    show_output_template: bool,
    /// Show available profiles
    #[arg(long)]
    show_profiles:        bool,
    /// Show available commands
    #[arg(long)]
    show_commands:        bool,
    /// Add or remove commands from selected profile by prefixing the command's name with '+' or
    /// '-', respectively, e.g., +uname -dmesg; you may need to use '--' to signify the end of the
    /// options
    #[arg(name = "+|-command")]
    filter_commands:      Vec<String>,
}

impl Opt {
    pub fn validate(self) -> anyhow::Result<Self> {
        if self.output == OutputType::Hbs && self.output_template.is_none() {
            return Err(anyhow!("Output hbs requires output template"));
        }

        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputType {
    Hbs,
    Html,
    Json,
    Markdown,
}

impl FromStr for OutputType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "hbs" => Ok(OutputType::Hbs),
            "html" => Ok(OutputType::Html),
            "json" => Ok(OutputType::Json),
            "markdown" => Ok(OutputType::Markdown),
            _ => Err(anyhow!("failed to parse {} as output type", s)),
        }
    }
}

pub fn main() -> anyhow::Result<()> {
    human_panic::setup_panic!();
    env_logger::init();

    let opt = Opt::parse().validate()?;
    let config = opt
        .config
        .as_ref()
        .map(Config::from_file)
        .unwrap_or_else(|| Config::from_str(defaults::CONFIG))
        .context("could not load configuration file")?;
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
    if opt.show_output_template {
        show_output_template(&opt);
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
    println!("{}", toml);
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

fn show_output_template(opt: &Opt) {
    let template = match opt.output {
        OutputType::Hbs => {
            let template_file = opt
                .output_template
                .as_ref()
                .expect("output hbs requires output template");
            let mut txt = String::new();
            File::open(template_file)
                .expect("failed to open template file")
                .read_to_string(&mut txt)
                .expect("failed to read template file");
            txt
        }
        OutputType::Html => defaults::HTML_TEMPLATE.to_string(),
        OutputType::Json => "".to_string(),
        OutputType::Markdown => defaults::MD_TEMPLATE.to_string(),
    };

    println!("{}", template);
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

fn generate_report(opt: &Opt, config: &Config, profile_name: &str) -> anyhow::Result<()> {
    let parallel = opt.parallel.unwrap_or(config.defaults.max_parallel_commands);
    let repetitions = opt.repetitions.unwrap_or(config.defaults.repetitions);
    let progress = is_show_progress(&opt);
    // Create renderer early to detect misconfiguration early
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    let renderer = create_renderer(&opt.output, opt.output_template.as_ref())?;

    let hostinfo = config.commands_for_hostinfo();
    let commands = create_commands(opt, config, profile_name)?;
    let number_of_commands = hostinfo.len() + repetitions * commands.len();

    let runner = create_runner(progress, number_of_commands);
    let analysis = Analysis::new(Box::new(runner), &hostinfo, &commands)
        .with_max_parallel_commands(parallel)
        .with_repetitions(repetitions);
    let context = create_context(opt, config, profile_name)?;

    let report = analysis.run(context)?;

    renderer
        .render(&report, handle)
        .context("failed to render report")?;

    Ok(())
}

fn is_show_progress(opt: &Opt) -> bool {
    if opt.progress {
        return true;
    }
    if opt.no_progress {
        return false;
    }
    if std::io::stderr().is_terminal() {
        return true;
    }

    false
}

fn create_commands(opt: &Opt, config: &Config, profile_name: &str) -> anyhow::Result<Vec<Command>> {
    let (add_commands, remove_commands) = create_command_filter(&opt.filter_commands);
    let mut commands: Vec<Command> = config
        .profile(profile_name)
        .and_then(|p| Ok(config.commands_for_profile(p)))?
        .into_iter()
        .filter(|x| !remove_commands.contains(x.name()))
        .collect();
    let mut additional_commands: Vec<Command> = config
        .commands
        .clone()
        .into_iter()
        .filter(|x| add_commands.contains(x.name()))
        .collect();
    commands.append(&mut additional_commands);

    Ok(commands)
}

fn create_command_filter(command_spec: &[String]) -> (HashSet<&str>, HashSet<&str>) {
    let mut add = HashSet::new();
    let mut remove = HashSet::new();

    for cs in command_spec {
        match cs.chars().next() {
            Some('+') => {
                add.insert(&cs[1..]);
            }
            Some('-') => {
                remove.insert(&cs[1..]);
            }
            _ => {}
        }
    }

    (add, remove)
}

fn create_renderer<W: Write>(
    output_type: &OutputType,
    output_template: Option<&String>,
) -> anyhow::Result<Box<dyn Renderer<W>>> {
    let renderer: Box<dyn Renderer<W>> = match output_type {
        OutputType::Hbs => {
            let template_file = output_template.expect("output hbs requires output template");
            let renderer = renderer::HbsRenderer::from_file(template_file)?;
            Box::new(renderer)
        }
        OutputType::Html => Box::new(renderer::HbsRenderer::new(defaults::HTML_TEMPLATE)),
        OutputType::Json => Box::new(renderer::JsonRenderer::new()),
        OutputType::Markdown => Box::new(renderer::HbsRenderer::new(defaults::MD_TEMPLATE)),
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
    let pb = ProgressBar::new(expected as u64).with_style(
        ProgressStyle::default_bar()
            .template("Running commands {bar:40.cyan/blue} {pos}/{len}")
            .expect("valid progress template"),
    );

    let _ = std::thread::Builder::new().name("Progress".to_string()).spawn(move || {
        for _ in 0..expected {
            let _ = rx.recv().expect("Thread failed to receive progress via channel");
            pb.inc(1);
        }
        pb.finish_and_clear();
    });

    tx
}

fn create_context(_opt: &Opt, _config: &Config, profile_name: &str) -> anyhow::Result<Context> {
    let mut context = Context::new()?;
    context.add("Profile", profile_name);
    context.add("Usereport version", env!("CARGO_PKG_VERSION"));

    Ok(context)
}

#[cfg(target_os = "macos")]
mod defaults {
    pub(crate) static CONFIG: &str = include_str!("../../contrib/osx.conf");
    pub(crate) static HTML_TEMPLATE: &str = include_str!("../../contrib/html.hbs");
    pub(crate) static MD_TEMPLATE: &str = include_str!("../../contrib/markdown.hbs");
}

#[cfg(target_os = "linux")]
mod defaults {
    pub(crate) static CONFIG: &str = include_str!("../../contrib/linux.conf");
    pub(crate) static HTML_TEMPLATE: &str = include_str!("../../contrib/html.hbs");
    pub(crate) static MD_TEMPLATE: &str = include_str!("../../contrib/markdown.hbs");
}
