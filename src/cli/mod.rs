use crate::{
    Analysis, AnalysisReport, Command, Config, Context, Renderer, ThreadRunner,
    baseline::BaselineStore,
    collector::{
        Collector, cgroup::CgroupCollector, cpu::CpuCollector, cpufreq::CpuFreqCollector, disk::DiskCollector,
        host::HostCollector, interrupts::InterruptsCollector, memory::MemoryCollector, network::NetworkCollector,
    },
    diff,
    finding::{Finding, Severity},
    llm::LlmOutput,
    redact::Redactor,
    renderer,
    rule::{Rule, RuleEngine, builtin::builtin_rules},
    workload::load_workload_rules,
};
#[cfg(feature = "bpf")]
use crate::{collector::bpf::BpfCollector, rule::builtin::bpf_rules};
use anyhow::{Context as _, anyhow};
use clap::Parser;
use comfy_table::Table;
use termimad;
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
    /// Enable eBPF opt-in collectors (runqlat, biolatency, tcpretrans, execsnoop,
    /// cachestat). Requires bpf feature. Emits an Info finding per tool not found
    /// in PATH and exits 0.
    #[cfg(feature = "bpf")]
    #[arg(long)]
    bpf: bool,
    /// Load a named workload rule pack and merge it with the base rules.
    /// Known values: postgres, java, nginx, kubelet, none (default).
    #[arg(long, value_name = "NAME", default_value = "none")]
    workload: String,
    /// Run CPU profiling for DURATION (e.g. 10s, 1m) using perf or bpftrace,
    /// fold stacks with inferno, and embed the flamegraph SVG in the HTML report.
    /// Requires perf or bpftrace in PATH; emits an info finding when neither is found.
    #[arg(long, value_name = "DURATION")]
    profile_cpu: Option<String>,
    /// Subcommand: `usereport baseline …`, `usereport diff`, or `usereport explain <id>`.
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
    /// Print the definition, what raises it, what to investigate, and links for a rule or signal ID.
    /// When the ID is unknown, lists all known rule IDs.
    Explain { id: String },
    /// Check whether all binaries used by the selected profile(s) are installed on $PATH.
    Check {
        /// Profile to check (checks all profiles when omitted).
        #[arg(short = 'p', long)]
        profile: Option<String>,
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

    // Check subcommand needs config — handle before the config-free dispatch.
    if let Some(Subcommand::Check { profile }) = &opt.command {
        let config = opt
            .config
            .as_ref()
            .map(Config::from_file)
            .unwrap_or_else(|| Config::from_str(defaults::CONFIG))
            .context("could not load configuration file")?;
        config.validate()?;
        return run_check(&config, profile.as_deref());
    }

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
    use comfy_table::{Attribute, Cell};
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Name").add_attribute(Attribute::Bold),
        Cell::new("Commands").add_attribute(Attribute::Bold),
        Cell::new("Description").add_attribute(Attribute::Bold),
    ]);
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
    use comfy_table::{Attribute, Cell};
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Name").add_attribute(Attribute::Bold),
        Cell::new("Command").add_attribute(Attribute::Bold),
        Cell::new("Title").add_attribute(Attribute::Bold),
        Cell::new("Description").add_attribute(Attribute::Bold),
    ]);
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
        Subcommand::Explain { id } => run_explain(id),
        Subcommand::Check { .. } => unreachable!("Check is handled before run_subcommand"),
    }
}

fn run_check(config: &Config, profile_filter: Option<&str>) -> anyhow::Result<()> {
    use config::Profile;

    // (category, name, binary) — binary may be an absolute path or a bare name
    let mut checks: Vec<(String, String, String)> = Vec::new();

    // Profile commands
    let profiles: Vec<&Profile> = match profile_filter {
        Some(name) => vec![config.profile(name)?],
        None => config.profiles.iter().collect(),
    };
    for profile in &profiles {
        for cmd in config.commands_for_profile(profile) {
            let binary = cmd.binary().unwrap_or_else(|| cmd.command().to_string());
            checks.push((profile.name.clone(), cmd.name().to_string(), binary));
        }
    }

    // Built-in collector tools (platform-specific)
    #[cfg(target_os = "linux")]
    {
        checks.push(("collectors".into(), "dmesg".into(), "dmesg".into()));
        checks.push(("collectors".into(), "free".into(), "free".into()));
    }
    #[cfg(target_os = "macos")]
    {
        checks.push(("collectors".into(), "sysctl".into(), "sysctl".into()));
        checks.push(("collectors".into(), "iostat".into(), "/usr/sbin/iostat".into()));
        checks.push(("collectors".into(), "netstat".into(), "netstat".into()));
        checks.push(("collectors".into(), "vm_stat".into(), "vm_stat".into()));
    }

    // CPU profiling tools (--profile-cpu)
    checks.push(("profiling".into(), "perf".into(), "perf".into()));
    checks.push(("profiling".into(), "bpftrace".into(), "bpftrace".into()));

    // eBPF tools (--bpf)
    #[cfg(feature = "bpf")]
    for tool in crate::collector::bpf::TOOLS {
        // Resolve bare name or -bpfcc suffix (Ubuntu packages them with the suffix).
        let binary = crate::collector::bpf::resolve_bcc_tool(tool)
            .unwrap_or_else(|| tool.to_string());
        checks.push(("bpf".into(), (*tool).into(), binary));
    }

    use comfy_table::{Attribute, Cell, Color};
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Category").add_attribute(Attribute::Bold),
        Cell::new("Name").add_attribute(Attribute::Bold),
        Cell::new("Binary").add_attribute(Attribute::Bold),
        Cell::new("Status").add_attribute(Attribute::Bold),
    ]);

    let is_tty = std::io::stdout().is_terminal();
    let mut missing = 0usize;
    for (category, name, binary) in &checks {
        let found = which::which(binary).is_ok() || std::path::Path::new(binary.as_str()).exists();
        let status_str = if found { "ok" } else { missing += 1; "MISSING" };
        let status_cell = if is_tty {
            if found {
                Cell::new(status_str).fg(Color::Green)
            } else {
                Cell::new(status_str).fg(Color::Red)
            }
        } else {
            Cell::new(status_str)
        };
        table.add_row(vec![
            Cell::new(category),
            Cell::new(name),
            Cell::new(binary),
            status_cell,
        ]);
    }
    println!("{table}");

    if missing > 0 {
        eprintln!("{} binary/binaries not found", missing);
        std::process::exit(1);
    }
    Ok(())
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

fn run_explain(id: &str) -> anyhow::Result<()> {
    let all_rules = builtin_rules();
    if let Some(rule) = all_rules.iter().find(|r| r.id == id) {
        let is_tty = std::io::stdout().is_terminal();
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        run_explain_inner(rule, is_tty, &mut handle)
    } else {
        eprintln!("Unknown topic '{}'.", id);
        eprintln!();
        eprintln!("Known rule IDs:");
        for r in &all_rules {
            eprintln!("  {}", r.id);
        }
        std::process::exit(1);
    }
}

fn run_explain_inner(rule: &Rule, is_tty: bool, out: &mut dyn Write) -> anyhow::Result<()> {
    let label = format!("{:?}", rule.severity);
    let severity_str = if is_tty {
        use owo_colors::OwoColorize as _;
        match rule.severity {
            Severity::Crit => label.red().to_string(),
            Severity::Warn => label.yellow().to_string(),
            Severity::Info => label.blue().to_string(),
        }
    } else {
        label
    };

    writeln!(out, "ID:       {}", rule.id)?;
    writeln!(out, "Severity: {}", severity_str)?;
    writeln!(out, "Summary:  {}", rule.summary)?;
    if let Some(desc) = &rule.description {
        writeln!(out)?;
        writeln!(out, "{}", desc)?;
    }
    if !rule.suggest.is_empty() {
        writeln!(out)?;
        writeln!(out, "To investigate:")?;
        for s in &rule.suggest {
            writeln!(out, "  {}", s)?;
        }
    }
    if !rule.links.is_empty() {
        writeln!(out)?;
        writeln!(out, "Links:")?;
        for l in &rule.links {
            writeln!(out, "  {}", l)?;
        }
    }
    Ok(())
}

/// Run CPU profiling for `duration_secs` seconds using perf (or bpftrace when
/// `use_bpf` is true and bpftrace is on PATH). Returns `Ok(Some(svg))` on
/// success, `Ok(None)` when no profiling tool is available.
fn generate_flamegraph(duration_secs: u64, _use_bpf: bool) -> anyhow::Result<Option<String>> {
    let has_perf = which::which("perf").is_ok();
    let has_bpftrace = which::which("bpftrace").is_ok();

    if !has_perf && !has_bpftrace {
        return Ok(None);
    }

    if has_perf {
        let tmpdir = tempfile::tempdir()?;
        let perf_data = tmpdir.path().join("perf.data");

        let status = std::process::Command::new("perf")
            .args(["record", "-F", "99", "-ag", "-o"])
            .arg(&perf_data)
            .args(["--", "sleep", &duration_secs.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .context("failed to run perf record")?;

        if !status.success() {
            return Ok(None);
        }

        let script = std::process::Command::new("perf")
            .args(["script", "-i"])
            .arg(&perf_data)
            .stderr(std::process::Stdio::null())
            .output()
            .context("failed to run perf script")?;

        if script.stdout.is_empty() {
            return Ok(None);
        }

        return generate_svg_from_perf_script(&script.stdout);
    }

    if has_bpftrace {
        let script = format!(
            "profile:hz:99 {{ @[comm, kstack, ustack] = count(); }} interval:s:{duration_secs} {{ exit(); }}"
        );
        let out = std::process::Command::new("bpftrace")
            .args(["-f", "folded", "-e", &script])
            .stderr(std::process::Stdio::null())
            .output()
            .context("failed to run bpftrace")?;

        if out.stdout.is_empty() {
            return Ok(None);
        }

        return generate_svg_from_folded(&out.stdout);
    }

    Ok(None)
}

fn generate_svg_from_perf_script(perf_output: &[u8]) -> anyhow::Result<Option<String>> {
    use inferno::collapse::Collapse;
    use inferno::collapse::perf::{Folder, Options as CollapseOpts};
    use inferno::flamegraph;

    let mut collapsed = Vec::new();
    let mut folder = Folder::from(CollapseOpts::default());
    folder
        .collapse(perf_output, &mut collapsed)
        .context("inferno collapse failed")?;

    let collapsed_str = String::from_utf8_lossy(&collapsed);
    let lines: Vec<&str> = collapsed_str.lines().filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return Ok(None);
    }

    let mut svg = Vec::new();
    flamegraph::from_lines(&mut flamegraph::Options::default(), lines, &mut svg)
        .context("inferno flamegraph failed")?;
    Ok(Some(String::from_utf8(svg)?))
}

fn generate_svg_from_folded(folded: &[u8]) -> anyhow::Result<Option<String>> {
    use inferno::flamegraph;

    let collapsed_str = String::from_utf8_lossy(folded);
    let lines: Vec<&str> = collapsed_str.lines().filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return Ok(None);
    }

    let mut svg = Vec::new();
    flamegraph::from_lines(&mut flamegraph::Options::default(), lines, &mut svg)
        .context("inferno flamegraph failed")?;
    Ok(Some(String::from_utf8(svg)?))
}

fn generate_report(opt: &Opt, config: &Config, profile_name: &str) -> anyhow::Result<Vec<Finding>> {
    let parallel = opt.parallel.unwrap_or(config.defaults.max_parallel_commands);
    let repetitions = opt.repetitions.unwrap_or(config.defaults.repetitions);
    let progress = is_show_progress(opt);
    // Create renderer early to detect misconfiguration early (skip for LLM path)
    let mut writer = output_writer(&opt.output_file)?;
    let renderer = if opt.output != OutputType::Llm {
        Some(create_renderer::<Box<dyn Write + Send>>(
            &opt.output,
            opt.output_template.as_ref(),
        )?)
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
    let mut collectors: Vec<Box<dyn Collector>> = vec![
        Box::new(HostCollector::new()),
        Box::new(CpuCollector::new()),
        Box::new(DiskCollector::new()),
        Box::new(NetworkCollector::new()),
        Box::new(CpuFreqCollector::new()),
        Box::new(MemoryCollector::new()),
        Box::new(InterruptsCollector::new()),
        Box::new(CgroupCollector::new()),
    ];
    let mut all_rules = builtin_rules();
    #[cfg(feature = "bpf")]
    if opt.bpf {
        collectors.push(Box::new(BpfCollector::new()));
        all_rules.extend(bpf_rules());
    }
    // Phase 8: merge workload-specific rules when --workload is set to a known pack.
    let workload_rules =
        load_workload_rules(&opt.workload).with_context(|| format!("invalid --workload value '{}'", opt.workload))?;
    all_rules.extend(workload_rules);
    let rule_engine = RuleEngine::new(all_rules);

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
    let mut report = analysis.run(context)?;

    // --profile-cpu: generate flamegraph and attach to report.
    if let Some(profile_dur) = &opt.profile_cpu {
        let dur = parse_duration(profile_dur)?;
        let dur_secs = dur.as_secs().max(1);
        #[cfg(feature = "bpf")]
        let use_bpf = opt.bpf;
        #[cfg(not(feature = "bpf"))]
        let use_bpf = false;
        match generate_flamegraph(dur_secs, use_bpf) {
            Ok(Some(svg)) => report = report.with_flamegraph(svg),
            Ok(None) => {
                report.findings.push(Finding {
                    id: "profile.cpu.unavailable".to_string(),
                    kind: crate::finding::FindingKind::Rule,
                    severity: Severity::Info,
                    summary:
                        "--profile-cpu requested but neither 'perf' nor 'bpftrace' was found in PATH; flamegraph omitted."
                            .to_string(),
                    evidence: Vec::new(),
                    suggest: vec!["Install linux-perf or bpftrace and re-run.".to_string()],
                });
            }
            Err(e) => log::warn!("flamegraph generation failed: {}", e),
        }
    }

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
        let mut buf: Vec<u8> = Vec::new();
        renderer
            .expect("renderer created for non-LLM output")
            .render(&report, Box::new(&mut buf) as Box<dyn Write + Send>)
            .context("failed to render report")?;
        let rendered = String::from_utf8(buf).context("rendered output is not UTF-8")?;
        let is_tty = opt.output_file.is_none() && std::io::stdout().is_terminal();
        render_for_output(&rendered, &mut *writer, is_tty, &opt.output)?;
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

fn render_for_output(
    rendered: &str,
    writer: &mut dyn Write,
    is_tty: bool,
    format: &OutputType,
) -> anyhow::Result<()> {
    if is_tty && *format == OutputType::Markdown {
        // termimad::print_text writes directly to stdout;
        // writer is intentionally unused here because is_tty=true implies output_file.is_none()
        termimad::print_text(rendered);
    } else {
        write!(writer, "{}", rendered).context("write output")?;
    }
    Ok(())
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
            #[cfg(feature = "bpf")]
            bpf: false,
            workload: "none".to_string(),
            profile_cpu: None,
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

    #[test]
    fn render_for_output_pipe_markdown_no_ansi() {
        let input = "# Hello\n\n**world**\n";
        let mut buf: Vec<u8> = Vec::new();
        render_for_output(input, &mut buf, false, &OutputType::Markdown).unwrap();
        assert_eq!(buf, input.as_bytes());
        assert!(!buf.contains(&0x1b_u8));
    }

    #[test]
    fn render_for_output_tty_html_no_ansi() {
        let input = "<html><body>hello</body></html>";
        let mut buf: Vec<u8> = Vec::new();
        render_for_output(input, &mut buf, true, &OutputType::Html).unwrap();
        assert_eq!(buf, input.as_bytes());
        assert!(!buf.contains(&0x1b_u8));
    }

    #[test]
    fn render_for_output_tty_markdown_returns_ok() {
        // tty path calls termimad::print_text which writes to stdout directly;
        // we can only verify the function returns Ok(())
        let input = "# Hello\n";
        let mut buf: Vec<u8> = Vec::new();
        let result = render_for_output(input, &mut buf, true, &OutputType::Markdown);
        assert!(result.is_ok());
    }

    fn make_rule(severity: Severity) -> Rule {
        use crate::rule::{Predicate, Op, Rhs, Value};
        Rule {
            id: "test.rule".to_string(),
            when: Predicate::Cmp {
                path: vec!["host".to_string(), "uptime_secs".to_string()],
                op: Op::Gt,
                rhs: Rhs::Value(Value::Number(0.0)),
            },
            severity,
            summary: "test summary".to_string(),
            description: None,
            evidence_ids: vec![],
            suggest: vec!["check something".to_string()],
            links: vec![],
        }
    }

    #[test]
    fn run_explain_inner_tty_warn_contains_ansi() {
        let rule = make_rule(Severity::Warn);
        let mut buf: Vec<u8> = Vec::new();
        run_explain_inner(&rule, true, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains('\x1b'), "expected ANSI escape in tty output, got: {:?}", output);
        assert!(output.contains("Warn"), "expected 'Warn' token in output");
    }

    #[test]
    fn run_explain_inner_tty_info_contains_ansi() {
        let rule = make_rule(Severity::Info);
        let mut buf: Vec<u8> = Vec::new();
        run_explain_inner(&rule, true, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains('\x1b'), "expected ANSI escape in tty output, got: {:?}", output);
        assert!(output.contains("Info"), "expected 'Info' token in output");
    }

    #[test]
    fn run_explain_inner_no_tty_no_ansi() {
        let rule = make_rule(Severity::Crit);
        let mut buf: Vec<u8> = Vec::new();
        run_explain_inner(&rule, false, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains('\x1b'), "expected no ANSI escape in non-tty output, got: {:?}", output);
        assert!(output.contains("Crit"), "expected 'Crit' token in output");
    }
}
