use crate::{
    baseline::BaselineStore,
    collector::{cpu::CpuCollector, disk::DiskCollector, host::HostCollector, Collector},
    diff,
    finding::{Finding, Severity},
    llm::LlmOutput,
    redact::Redactor,
    renderer,
    rule::{builtin::builtin_rules, RuleEngine},
    Analysis, AnalysisReport, Command, Config, Context, Renderer, ThreadRunner,
};
use anyhow::{anyhow, Context as _};
use clap::Parser;
use comfy_table::Table;
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    collections::HashSet,
    fs::File,
    io::{IsTerminal, Read, Write},
    path::PathBuf,
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
};

pub mod config;

#[derive(Debug, Parser)]
#[command(
    author,
    about,
    after_help = "Set RUST_LOG=debug for verbose output, e.g.: RUST_LOG=debug usereport"
)]
pub struct Opt {
    /// Configuration from file, or default if not present
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// Output format
    #[arg(short, long, value_enum, default_value = "markdown")]
    output: OutputType,
    /// Set output template if output is set to "template"
    #[arg(long)]
    output_template: Option<String>,
    /// Write rendered output to a file (parent directories are created automatically);
    /// when absent, output goes to stdout.
    #[arg(short = 'O', long)]
    output_file: Option<PathBuf>,
    /// Set number of commands to run in parallel; overrides setting from config file
    #[arg(long)]
    parallel: Option<usize>,
    /// Set number of how many times to run commands in row; overrides setting from config file.
    /// Mutually exclusive with --duration.
    #[arg(long, conflicts_with = "duration")]
    repetitions: Option<usize>,
    /// Time-sampling window (e.g. 30s, 2m). When given, collectors that
    /// support sampling loop N = floor(duration/interval)+1 times. Mutually
    /// exclusive with --repetitions.
    #[arg(long, value_name = "DURATION", conflicts_with = "repetitions")]
    duration: Option<String>,
    /// Sampling interval within the --duration window (e.g. 2s). Requires
    /// --duration; defaults to 5s when --duration is given without --interval.
    #[arg(long, value_name = "INTERVAL", requires = "duration")]
    interval: Option<String>,
    /// Force to show progress bar while waiting for all commands to finish
    #[arg(long, conflicts_with = "no_progress")]
    progress: bool,
    /// Force to hide progress bar while waiting for all commands to finish
    #[arg(long, conflicts_with = "progress")]
    no_progress: bool,
    /// Activate debug mode
    #[arg(short, long)]
    debug: bool,
    /// Exit-code policy based on the highest-severity finding produced.
    #[arg(long, value_enum, default_value = "never")]
    exit_on: ExitOn,
    /// Target cgroup path for the cgroup collector (Phase 3 wiring).
    /// Example: --cgroup /sys/fs/cgroup/system.slice/foo.service
    #[arg(long)]
    pub cgroup: Option<PathBuf>,
    /// Set profile to use
    #[arg(short = 'p', long)]
    profile: Option<String>,
    /// Show active config
    #[arg(long)]
    show_config: bool,
    /// Show active template
    #[arg(long)]
    show_output_template: bool,
    /// Show available profiles
    #[arg(long)]
    show_profiles: bool,
    /// Show available commands
    #[arg(long)]
    show_commands: bool,
    /// Annotate signals with a named baseline (loaded from
    /// ${XDG_DATA_HOME}/usereport/baselines/<NAME>.json) and emit
    /// auto-outlier findings (|z|>3 → warn, |z|>6 → crit).
    #[arg(long, value_name = "NAME")]
    pub baseline: Option<String>,
    /// Add or remove commands from selected profile by prefixing the command's name with '+' or
    /// '-', respectively, e.g., +uname -dmesg; you may need to use '--' to signify the end of the
    /// options
    #[arg(name = "+|-command")]
    filter_commands: Vec<String>,
    /// Apply HMAC-SHA-256 redaction to `--output llm` output. Redacts
    /// hostnames, IPv4/IPv6 addresses, and MAC addresses. Uses
    /// `USEREPORT_REDACT_SALT` env var; falls back to a compile-time constant
    /// (provides weak privacy — hashes are not secret).
    #[arg(long)]
    redact: bool,
    /// Subcommand: `usereport baseline …` or `usereport diff <a.json> <b.json>`.
    #[command(subcommand)]
    pub command: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Manage named baselines (record / list / show / delete).
    Baseline {
        #[command(subcommand)]
        action: BaselineAction,
    },
    /// Diff two AnalysisReport JSON files.
    Diff {
        a: PathBuf,
        b: PathBuf,
        /// Output format: `text` (default) or `json`.
        #[arg(long, default_value = "text")]
        output: String,
    },
}

#[derive(Debug, clap::Subcommand)]
pub enum BaselineAction {
    /// Capture the current run as a named baseline.
    Record {
        #[arg(long)]
        name: Option<String>,
    },
    /// List stored baselines.
    List,
    /// Show a stored baseline's contents.
    Show { name: String },
    /// Delete a stored baseline.
    Delete { name: String },
}

impl Opt {
    pub fn validate(self) -> anyhow::Result<Self> {
        if self.output == OutputType::Template && self.output_template.is_none() {
            return Err(anyhow!("Output template requires --output-template <PATH>"));
        }

        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputType {
    Template,
    Html,
    Json,
    Markdown,
    Llm,
}

impl FromStr for OutputType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "hbs" => {
                eprintln!("warning: --output hbs is deprecated; use --output template");
                Ok(OutputType::Template)
            }
            "template" => Ok(OutputType::Template),
            "html" => Ok(OutputType::Html),
            "json" => Ok(OutputType::Json),
            "markdown" => Ok(OutputType::Markdown),
            "llm" => Ok(OutputType::Llm),
            _ => Err(anyhow!("failed to parse {} as output type", s)),
        }
    }
}

impl clap::ValueEnum for OutputType {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            OutputType::Template,
            OutputType::Html,
            OutputType::Json,
            OutputType::Markdown,
            OutputType::Llm,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            OutputType::Template => clap::builder::PossibleValue::new("template"),
            OutputType::Html => clap::builder::PossibleValue::new("html"),
            OutputType::Json => clap::builder::PossibleValue::new("json"),
            OutputType::Markdown => clap::builder::PossibleValue::new("markdown"),
            OutputType::Llm => clap::builder::PossibleValue::new("llm"),
        })
    }

    fn from_str(input: &str, _ignore_case: bool) -> Result<Self, String> {
        <Self as std::str::FromStr>::from_str(input).map_err(|e| e.to_string())
    }
}

/// Exit-code policy controlled by `--exit-on`. See SDD §103.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ExitOn {
    Never,
    Warn,
    Crit,
}

/// Compute the process exit code for a run based on the configured policy and
/// the produced findings.
///
/// Per SDD Req 8 / AC-4:
/// - `Never` → always 0.
/// - `Warn`  → 1 if any `warn` finding fires, else 0. Exit 1 is never emitted
///   under `Never`.
/// - `Crit`  → 2 if any `crit` finding fires, else 0. Exit 2 is never emitted
///   outside `Crit`. A `warn` finding under `Crit` does not raise exit code.
pub fn compute_exit_code(exit_on: ExitOn, findings: &[Finding]) -> i32 {
    match exit_on {
        ExitOn::Never => 0,
        ExitOn::Warn => {
            if findings
                .iter()
                .any(|f| f.severity == Severity::Warn || f.severity == Severity::Crit)
            {
                1
            } else {
                0
            }
        }
        ExitOn::Crit => {
            if findings.iter().any(|f| f.severity == Severity::Crit) {
                2
            } else {
                0
            }
        }
    }
}

pub fn main() -> anyhow::Result<()> {
    human_panic::setup_panic!();
    env_logger::init();
    log::debug!("RUST_LOG={:?}", std::env::var("RUST_LOG").unwrap_or_default());

    let opt = Opt::parse().validate()?;

    // Phase 2: subcommand dispatch (baseline / diff). The default code path
    // (no subcommand) preserves the existing report-generation behaviour.
    if let Some(cmd) = opt.command.as_ref() {
        return run_subcommand(cmd);
    }

    let config = opt
        .config
        .as_ref()
        .map(Config::from_file)
        .unwrap_or_else(|| Config::from_str(defaults::CONFIG))
        .context("could not load configuration file")?;
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

    let findings = generate_report(&opt, &config, profile_name)?;

    let code = compute_exit_code(opt.exit_on, &findings);
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

fn show_config(config: &Config) {
    let toml = toml::to_string_pretty(config).expect("failed to serialize active config in TOML");
    println!("{}", toml);
}

fn show_profiles(config: &Config) {
    let mut table = Table::new();
    table.set_header(vec!["Name", "Commands", "Description"]);
    for p in &config.profiles {
        table.add_row(vec![
            p.name.clone(),
            p.commands.as_slice().join("\n"),
            p.description.as_deref().unwrap_or("-").to_string(),
        ]);
    }
    println!("{table}");
}

fn show_output_template(opt: &Opt) -> anyhow::Result<()> {
    let template = match opt.output {
        OutputType::Template => {
            let template_file = opt
                .output_template
                .as_ref()
                .expect("output template requires --output-template <PATH>");
            let mut txt = String::new();
            File::open(template_file)
                .context("failed to open template file")?
                .read_to_string(&mut txt)
                .context("failed to read template file")?;
            txt
        }
        OutputType::Html => defaults::HTML_TEMPLATE.to_string(),
        OutputType::Json => "".to_string(),
        OutputType::Markdown => defaults::MD_TEMPLATE.to_string(),
        OutputType::Llm => "".to_string(),
    };

    println!("{}", template);
    Ok(())
}

fn show_commands(config: &Config) {
    let mut table = Table::new();
    table.set_header(vec!["Name", "Command", "Title", "Description"]);
    for c in &config.commands {
        table.add_row(vec![
            c.name().to_string(),
            c.command().to_string(),
            c.title().unwrap_or("-").to_string(),
            c.description().unwrap_or("-").to_string(),
        ]);
    }
    println!("{table}");
}

fn run_subcommand(cmd: &Subcommand) -> anyhow::Result<()> {
    match cmd {
        Subcommand::Baseline { action } => run_baseline(action),
        Subcommand::Diff { a, b, output } => run_diff(a, b, output),
    }
}

fn run_baseline(action: &BaselineAction) -> anyhow::Result<()> {
    let store = BaselineStore::xdg().context("locate baseline directory")?;
    match action {
        BaselineAction::Record { name } => {
            let label = name.as_deref().unwrap_or("default");
            store
                .record(label, &[])
                .with_context(|| format!("record baseline '{}'", label))?;
            println!(
                "recorded baseline '{}' at {}",
                label,
                store.dir().join(format!("{}.json", label)).display()
            );
        }
        BaselineAction::List => {
            for name in store.list().context("list baselines")? {
                println!("{}", name);
            }
        }
        BaselineAction::Show { name } => match store.load(name).with_context(|| format!("load baseline '{}'", name))? {
            Some(record) => println!("{}", serde_json::to_string_pretty(&record)?),
            None => return Err(anyhow!("baseline '{}' not found", name)),
        },
        BaselineAction::Delete { name } => {
            store
                .delete(name)
                .with_context(|| format!("delete baseline '{}'", name))?;
            println!("deleted baseline '{}'", name);
        }
    }
    Ok(())
}

fn run_diff(a_path: &PathBuf, b_path: &PathBuf, output: &str) -> anyhow::Result<()> {
    let a_bytes = std::fs::read(a_path).with_context(|| format!("read {}", a_path.display()))?;
    let b_bytes = std::fs::read(b_path).with_context(|| format!("read {}", b_path.display()))?;
    let a: AnalysisReport = serde_json::from_slice(&a_bytes).with_context(|| format!("parse {}", a_path.display()))?;
    let b: AnalysisReport = serde_json::from_slice(&b_bytes).with_context(|| format!("parse {}", b_path.display()))?;
    let report = diff::diff(&a, &b);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match output {
        "json" => {
            serde_json::to_writer_pretty(&mut handle, &report)?;
            writeln!(handle)?;
        }
        _ => {
            diff::render_text(&report, &mut handle)?;
        }
    }
    Ok(())
}

fn generate_report(opt: &Opt, config: &Config, profile_name: &str) -> anyhow::Result<Vec<Finding>> {
    let parallel = opt.parallel.unwrap_or(config.defaults.max_parallel_commands);
    let repetitions = opt.repetitions.unwrap_or(config.defaults.repetitions);
    let progress = is_show_progress(opt);
    // Create renderer early to detect misconfiguration early (skip for LLM path)
    let writer = output_writer(&opt.output_file)?;
    let renderer = if opt.output != OutputType::Llm {
        Some(create_renderer::<Box<dyn Write + Send>>(&opt.output, opt.output_template.as_ref())?)
    } else {
        None
    };

    let hostinfo = config.commands_for_hostinfo();
    let commands = create_commands(opt, config, profile_name)?;
    let number_of_commands = hostinfo.len() + repetitions * commands.len();

    let (runner, progress_handle) = create_runner(progress, number_of_commands);

    // Phase 3: wire direct collectors + built-in rule engine. On hosts
    // without /proc (e.g. macOS) the collectors return empty signals fast,
    // so this is portable.
    let collectors: Vec<Box<dyn Collector>> = vec![
        Box::new(HostCollector::new()),
        Box::new(CpuCollector::new()),
        Box::new(DiskCollector::new()),
    ];
    let rule_engine = RuleEngine::new(builtin_rules());

    // Phase 4: parse --duration / --interval and thread them into the collector context.
    let sample_duration = opt
        .duration
        .as_deref()
        .map(parse_duration)
        .transpose()
        .context("invalid --duration value")?;
    let default_interval = std::time::Duration::from_secs(5);
    let sample_interval = opt
        .interval
        .as_deref()
        .map(parse_duration)
        .transpose()
        .context("invalid --interval value")?
        .or_else(|| sample_duration.map(|_| default_interval));

    let mut analysis = Analysis::new(Box::new(runner), &hostinfo, &commands)
        .with_max_parallel_commands(parallel)
        .with_repetitions(repetitions)
        .with_diagnostics(collectors, rule_engine);
    if let Some(d) = sample_duration {
        analysis = analysis.with_sample_duration(d, sample_interval.unwrap_or(default_interval));
    }
    if let Some(cgroup_path) = opt.cgroup.clone() {
        analysis = analysis.with_cgroup(cgroup_path);
    }
    if let Some(name) = opt.baseline.as_deref() {
        let store = BaselineStore::xdg().context("locate baseline directory")?;
        match store.load(name).with_context(|| format!("load baseline '{}'", name))? {
            Some(record) => analysis = analysis.with_baseline_records(vec![record]),
            None => return Err(anyhow!("baseline '{}' not found", name)),
        }
    }

    let context = create_context(opt, config, profile_name);
    let report = analysis.run(context)?;

    if let Some(handle) = progress_handle {
        if handle.join().is_err() {
            log::warn!("progress bar thread panicked");
        }
    }

    if opt.output == OutputType::Llm {
        let mut llm_out = LlmOutput::from_report(&report);
        if opt.redact {
            let redactor = Redactor::from_env();
            llm_out = redactor.redact_output(llm_out);
        }
        serde_json::to_writer(writer, &llm_out).context("failed to write LLM JSON")?;
    } else {
        renderer
            .expect("renderer created for non-LLM output")
            .render(&report, writer)
            .context("failed to render report")?;
    }

    // Phase 2 §116: every successful run appends one record to the rolling
    // JSONL, pruned to baseline_rolling_n. Failures are logged but do not
    // fail the run — the report is the user's primary deliverable.
    if !report.signals().is_empty() {
        match BaselineStore::xdg() {
            Ok(store) => {
                if let Err(e) = store.append_rolling(report.signals(), config.defaults.baseline_rolling_n) {
                    log::warn!("failed to append rolling baseline: {}", e);
                }
            }
            Err(e) => log::warn!("could not locate rolling baseline directory: {}", e),
        }
    }

    Ok(report.findings().to_vec())
}

fn parse_duration(s: &str) -> anyhow::Result<std::time::Duration> {
    humantime::parse_duration(s).with_context(|| format!("invalid duration {:?}", s))
}

/// Build a writer for the rendered report. When `output_file` is `Some(path)`,
/// the file is created (with parent directories) and a buffered writer is
/// returned. When `None`, stdout is returned.
pub fn output_writer(output_file: &Option<PathBuf>) -> anyhow::Result<Box<dyn Write + Send>> {
    match output_file {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create parent directories for {}", path.display()))?;
                }
            }
            let file = File::create(path).with_context(|| format!("failed to open output file {}", path.display()))?;
            Ok(Box::new(file))
        }
        None => Ok(Box::new(std::io::stdout())),
    }
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
) -> anyhow::Result<Box<dyn Renderer<W>>> {
    let renderer: Box<dyn Renderer<W>> = match output_type {
        OutputType::Template => {
            let template_file = output_template.expect("output template requires --output-template <PATH>");
            let renderer = renderer::TemplateRenderer::from_file(template_file)?;
            Box::new(renderer)
        }
        OutputType::Html => Box::new(renderer::TemplateRenderer::new(defaults::HTML_TEMPLATE).with_html_escape()),
        OutputType::Json => Box::new(renderer::JsonRenderer::new()),
        OutputType::Markdown => Box::new(renderer::TemplateRenderer::new(defaults::MD_TEMPLATE)),
        OutputType::Llm => unreachable!("LLM output is handled before renderer dispatch"),
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
    let pb = ProgressBar::new(expected as u64).with_style(
        ProgressStyle::default_bar()
            .template("Running commands {bar:40.cyan/blue} {pos}/{len}")
            .expect("valid progress template"),
    );

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

fn create_context(_opt: &Opt, _config: &Config, profile_name: &str) -> Context {
    let mut context = Context::new();
    context.add("Profile", profile_name);
    context.add("Usereport version", env!("CARGO_PKG_VERSION"));

    context
}

mod defaults {
    pub(crate) static HTML_TEMPLATE: &str = include_str!("../../contrib/html.j2");
    pub(crate) static MD_TEMPLATE: &str = include_str!("../../contrib/markdown.j2");

    #[cfg(target_os = "macos")]
    pub(crate) static CONFIG: &str = include_str!("../../contrib/osx.conf");

    #[cfg(target_os = "linux")]
    pub(crate) static CONFIG: &str = include_str!("../../contrib/linux.conf");
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    fn make_opt(output: OutputType, output_template: Option<&str>) -> Opt {
        Opt {
            output,
            output_template: output_template.map(|s| s.to_string()),
            output_file: None,
            config: None,
            parallel: None,
            repetitions: None,
            progress: false,
            no_progress: false,
            debug: false,
            exit_on: ExitOn::Never,
            cgroup: None,
            profile: None,
            show_config: false,
            show_output_template: false,
            show_profiles: false,
            show_commands: false,
            baseline: None,
            duration: None,
            interval: None,
            redact: false,
            filter_commands: vec![],
            command: None,
        }
    }

    #[test]
    fn opt_parses_cgroup_flag() {
        use clap::Parser;
        let opt = Opt::try_parse_from(["usereport", "--cgroup", "/sys/fs/cgroup/foo"]).expect("parse");
        assert_eq!(opt.cgroup, Some(PathBuf::from("/sys/fs/cgroup/foo")));
    }

    #[test]
    fn opt_cgroup_default_is_none() {
        use clap::Parser;
        let opt = Opt::try_parse_from(["usereport"]).expect("parse");
        assert_eq!(opt.cgroup, None);
    }

    #[test]
    fn test_filter_plus_adds_to_add_set() {
        let specs = vec!["+foo".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert!(add.contains("foo"));
        assert!(remove.is_empty());
    }

    #[test]
    fn test_filter_minus_adds_to_remove_set() {
        let specs = vec!["-bar".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert!(remove.contains("bar"));
        assert!(add.is_empty());
    }

    #[test]
    fn test_filter_bare_word_ignored() {
        let specs = vec!["bare".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert!(add.is_empty());
        assert!(remove.is_empty());
    }

    #[test]
    fn test_filter_empty_string_ignored() {
        let specs = vec!["".to_string()];
        let (add, remove) = create_command_filter(&specs);
        assert!(add.is_empty());
        assert!(remove.is_empty());
    }

    #[test]
    fn test_filter_mixed() {
        let specs = vec![
            "+foo".to_string(),
            "-bar".to_string(),
            "bare".to_string(),
            "".to_string(),
        ];
        let (add, remove) = create_command_filter(&specs);
        assert!(add.contains("foo"));
        assert!(remove.contains("bar"));
        assert_eq!(add.len(), 1);
        assert_eq!(remove.len(), 1);
    }

    #[test]
    fn test_validate_template_without_template_returns_err() {
        let opt = make_opt(OutputType::Template, None);
        assert_that!(opt.validate(), err(anything()));
    }

    #[test]
    fn test_validate_template_with_template_returns_ok() {
        let opt = make_opt(OutputType::Template, Some("f.j2"));
        assert_that!(opt.validate(), ok(anything()));
    }

    #[test]
    fn test_validate_markdown_without_template_returns_ok() {
        let opt = make_opt(OutputType::Markdown, None);
        assert_that!(opt.validate(), ok(anything()));
    }

    #[test]
    fn test_output_writer_none_returns_writer() {
        let _ = output_writer(&None).expect("stdout writer ok");
    }

    #[test]
    fn test_output_writer_some_creates_parent_dirs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("nested/dir/out.txt");
        {
            let mut w = output_writer(&Some(path.clone())).expect("writer ok");
            w.write_all(b"x").unwrap();
            w.flush().unwrap();
        }
        assert!(path.exists());
    }
}
