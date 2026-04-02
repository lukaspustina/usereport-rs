use crate::{renderer, Analysis, Command, Config, Context, Renderer, ThreadRunner};
use atty;
use exitfailure::ExitFailure;
use failure::{format_err, ResultExt};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use prettytable::{cell, format, row, Cell, Row, Table};
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
};
use structopt::{clap, StructOpt};

pub mod config;

#[derive(Debug, StructOpt)]
#[structopt(name = "usereport", author, about, setting = clap::AppSettings::ColoredHelp)]
struct Opt {
    /// Configuration from file, or default if not present
    #[structopt(short, long, parse(from_os_str))]
    config:               Option<PathBuf>,
    /// Output format
    #[structopt(short, long, possible_values = & ["hbs", "html", "json", "markdown"], default_value = "markdown")]
    output:               OutputType,
    /// Set output template if output is set to "hbs"
    #[structopt(long)]
    output_template:      Option<String>,
    /// Set number of commands to run in parallel; overrides setting from config file
    #[structopt(long)]
    parallel:             Option<usize>,
    /// Set number of how many times to run commands in row; overrides setting from config file
    #[structopt(long)]
    repetitions:          Option<usize>,
    /// Force to show progress bar while waiting for all commands to finish
    #[structopt(long, conflicts_with = "no_progress")]
    progress:             bool,
    /// Force to hide progress bar while waiting for all commands to finish
    #[structopt(long, conflicts_with = "progress")]
    no_progress:          bool,
    /// Activate debug mode
    #[structopt(short, long)]
    debug:                bool,
    /// Set profile to use
    #[structopt(short = "p", long)]
    profile:              Option<String>,
    /// Show active config
    #[structopt(long)]
    show_config:          bool,
    /// Show active template
    #[structopt(long)]
    show_output_template: bool,
    /// Show available profiles
    #[structopt(long)]
    show_profiles:        bool,
    /// Show available commands
    #[structopt(long)]
    show_commands:        bool,
    /// Add or remove commands from selected profile by prefixing the command's name with '+' or
    /// '-', respectively, e.g., +uname -dmesg; you may need to use '--' to signify the end of the
    /// options
    #[structopt(name = "+|-command")]
    filter_commands:      Vec<String>,
}

impl Opt {
    pub fn validate(self) -> Result<Self, failure::Error> {
        if self.output == OutputType::Hbs && self.output_template.is_none() {
            return Err(format_err!("Output hbs requires output template"));
        }

        Ok(self)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum OutputType {
    Hbs,
    Html,
    Json,
    Markdown,
}

impl FromStr for OutputType {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "hbs" => Ok(OutputType::Hbs),
            "html" => Ok(OutputType::Html),
            "json" => Ok(OutputType::Json),
            "markdown" => Ok(OutputType::Markdown),
            _ => Err(format_err!("failed to parse {} as output type", s)),
        }
    }
}

pub fn main() -> Result<(), ExitFailure> {
    human_panic::setup_panic!();
    env_logger::init();
    log::debug!("RUST_LOG={:?}", std::env::var("RUST_LOG").unwrap_or_default());

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
        log::debug!("Options: {:#?}", &opt);
        log::debug!("Configuration: {:#?}", &config);
        log::debug!("Using profile '{}'", profile_name);
    }

    if opt.show_config {
        show_config(&config);
        return Ok(());
    }
    if opt.show_output_template {
        show_output_template(&opt)?;
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

fn show_output_template(opt: &Opt) -> Result<(), failure::Error> {
    let template = match opt.output {
        OutputType::Hbs => {
            let template_file = opt
                .output_template
                .as_ref()
                .expect("output hbs requires output template");
            let mut txt = String::new();
            File::open(template_file)
                .with_context(|_| "failed to open template file")?
                .read_to_string(&mut txt)
                .with_context(|_| "failed to read template file")?;
            txt
        }
        OutputType::Html => defaults::HTML_TEMPLATE.to_string(),
        OutputType::Json => "".to_string(),
        OutputType::Markdown => defaults::MD_TEMPLATE.to_string(),
    };

    println!("{}", template);
    Ok(())
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
    let progress = is_show_progress(&opt);
    // Create renderer early to detect misconfiguration early
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    let renderer = create_renderer(&opt.output, opt.output_template.as_ref())?;

    let hostinfo = config.commands_for_hostinfo();
    let commands = create_commands(opt, config, profile_name)?;
    let number_of_commands = hostinfo.len() + repetitions * commands.len();

    let (runner, progress_handle) = create_runner(progress, number_of_commands);
    let analysis = Analysis::new(Box::new(runner), &hostinfo, &commands)
        .with_max_parallel_commands(parallel)
        .with_repetitions(repetitions);
    let context = create_context(opt, config, profile_name)?;

    let report = analysis.run(context)?;

    if let Some(handle) = progress_handle {
        if handle.join().is_err() {
            log::warn!("progress bar thread panicked");
        }
    }

    renderer
        .render(&report, handle)
        .with_context(|_| "failed to render report")?;

    Ok(())
}

fn is_show_progress(opt: &Opt) -> bool {
    if opt.progress {
        return true;
    }
    if opt.no_progress {
        return false;
    }
    if atty::is(atty::Stream::Stderr) {
        return true;
    }

    false
}

fn create_commands(opt: &Opt, config: &Config, profile_name: &str) -> Result<Vec<Command>, ExitFailure> {
    let (add_commands, remove_commands) = create_command_filter(&opt.filter_commands);
    let mut commands: Vec<Command> = config
        .profile(profile_name)
        .map(|p| config.commands_for_profile(p))?
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
) -> Result<Box<dyn Renderer<W>>, ExitFailure> {
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

fn create_runner(progress: bool, number_of_commands: usize) -> (ThreadRunner, Option<JoinHandle<()>>) {
    let mut runner = ThreadRunner::new();
    let mut join_handle = None;
    if progress {
        let (tx, handle) = create_progress_bar(number_of_commands);
        runner = runner.with_progress(tx);
        join_handle = Some(handle);
    }

    (runner, join_handle)
}

fn create_progress_bar(expected: usize) -> (Sender<usize>, JoinHandle<()>) {
    let (tx, rx): (Sender<usize>, Receiver<usize>) = mpsc::channel();
    let dt = ProgressDrawTarget::stderr_nohz();
    let pb = ProgressBar::with_draw_target(expected as u64, dt)
        .with_style(ProgressStyle::default_bar().template("Running commands {bar:40.cyan/blue} {pos}/{len}"));

    let handle = std::thread::Builder::new()
        .name("Progress".to_string())
        .spawn(move || {
            for _ in 0..expected {
                let _ = rx.recv().expect("Thread failed to receive progress via channel");
                pb.inc(1);
            }
            pb.finish_and_clear();
        })
        .expect("failed to spawn progress thread");

    (tx, handle)
}

fn create_context(_opt: &Opt, _config: &Config, profile_name: &str) -> Result<Context, ExitFailure> {
    let mut context = Context::new()?;
    context.add("Profile", profile_name);
    context.add("Usereport version", env!("CARGO_PKG_VERSION"));

    Ok(context)
}

mod defaults {
    pub(crate) static HTML_TEMPLATE: &str = include_str!("../../contrib/html.hbs");
    pub(crate) static MD_TEMPLATE: &str = include_str!("../../contrib/markdown.hbs");

    #[cfg(target_os = "macos")]
    pub(crate) static CONFIG: &str = include_str!("../../contrib/osx.conf");

    #[cfg(target_os = "linux")]
    pub(crate) static CONFIG: &str = include_str!("../../contrib/linux.conf");
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectral::prelude::*;

    // TST-3: Tests for create_command_filter

    #[test]
    fn test_filter_plus_adds_to_add_set() {
        let specs = vec!["+foo".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert_that!(add.contains("foo")).is_true();
        assert_that!(remove.is_empty()).is_true();
    }

    #[test]
    fn test_filter_minus_adds_to_remove_set() {
        let specs = vec!["-bar".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert_that!(remove.contains("bar")).is_true();
        assert_that!(add.is_empty()).is_true();
    }

    #[test]
    fn test_filter_bare_word_ignored() {
        let specs = vec!["bare".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert_that!(add.is_empty()).is_true();
        assert_that!(remove.is_empty()).is_true();
    }

    #[test]
    fn test_filter_empty_string_ignored() {
        let specs = vec!["".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert_that!(add.is_empty()).is_true();
        assert_that!(remove.is_empty()).is_true();
    }

    #[test]
    fn test_filter_mixed() {
        let specs = vec!["+foo".to_string(), "-bar".to_string(), "bare".to_string(), "".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert_that!(add.contains("foo")).is_true();
        assert_that!(remove.contains("bar")).is_true();
        assert_that!(add.len()).is_equal_to(1);
        assert_that!(remove.len()).is_equal_to(1);
    }

    // TST-4: Tests for Opt::validate

    fn make_opt(output: OutputType, output_template: Option<&str>) -> Opt {
        Opt {
            output,
            output_template: output_template.map(|s| s.to_string()),
            config: None,
            parallel: None,
            repetitions: None,
            progress: false,
            no_progress: false,
            debug: false,
            profile: None,
            show_config: false,
            show_output_template: false,
            show_profiles: false,
            show_commands: false,
            filter_commands: vec![],
        }
    }

    #[test]
    fn test_validate_hbs_without_template_returns_err() {
        let opt = make_opt(OutputType::Hbs, None);
        assert_that!(opt.validate()).is_err();
    }

    #[test]
    fn test_validate_hbs_with_template_returns_ok() {
        let opt = make_opt(OutputType::Hbs, Some("f.j2"));
        assert_that!(opt.validate()).is_ok();
    }

    #[test]
    fn test_validate_markdown_without_template_returns_ok() {
        let opt = make_opt(OutputType::Markdown, None);
        assert_that!(opt.validate()).is_ok();
    }
}
