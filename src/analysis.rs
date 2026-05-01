use crate::{
    baseline::{annotate, outlier_findings, BaselineRecord},
    collector::{CollectCtx, Collector},
    finding::{sort_findings, Finding},
    pattern::PatternEngine,
    rule::RuleEngine,
    runner,
    signal::Signal,
    Command, CommandResult, Runner,
};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug, path::PathBuf, time::Duration};
use thiserror::Error;

/// Error type
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    /// Analysis run failed
    #[error("analysis failed: {source}")]
    RunAnalysisFailed {
        #[from]
        source: runner::Error,
    },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

// Copy: This allows to reuse the into_iter object; safe for &Vec or &[]
#[derive(Debug)]
pub struct Analysis<'a, I: IntoIterator<Item = &'a Command> + Copy> {
    runner: Box<dyn Runner<'a, I>>,
    hostinfos: I,
    commands: I,
    repetitions: usize,
    max_parallel_commands: usize,
    collectors: Vec<Box<dyn Collector>>,
    rule_engine: Option<RuleEngine>,
    pattern_engine: Option<PatternEngine>,
    cgroup_path: Option<PathBuf>,
    baseline_records: Vec<BaselineRecord>,
    sample_duration: Option<Duration>,
    sample_interval: Option<Duration>,
}

impl<'a, I: IntoIterator<Item = &'a Command> + Copy> Analysis<'a, I> {
    pub fn new(runner: Box<dyn Runner<'a, I>>, hostinfos: I, commands: I) -> Self {
        Analysis {
            hostinfos,
            commands,
            runner,
            repetitions: 1,
            max_parallel_commands: 64,
            collectors: Vec::new(),
            rule_engine: None,
            pattern_engine: None,
            cgroup_path: None,
            baseline_records: Vec::new(),
            sample_duration: None,
            sample_interval: None,
        }
    }

    pub fn with_repetitions(self, repetitions: usize) -> Self {
        Analysis { repetitions, ..self }
    }

    pub fn with_max_parallel_commands(self, max_parallel_commands: usize) -> Self {
        Analysis {
            max_parallel_commands,
            ..self
        }
    }

    /// Install a collector set + rule engine. After running the configured
    /// commands, `Analysis::run()` invokes every collector in order, feeds
    /// the union of their signals into the rule engine, and stores the
    /// outputs on the returned `AnalysisReport`. Closes Phase 1's CC5.
    pub fn with_diagnostics(self, collectors: Vec<Box<dyn Collector>>, rule_engine: RuleEngine) -> Self {
        Analysis {
            collectors,
            rule_engine: Some(rule_engine),
            ..self
        }
    }

    /// Install a pattern engine that runs after the rule pass (SDD §641).
    /// Pattern findings are merged and re-sorted with rule findings.
    pub fn with_pattern_engine(self, engine: PatternEngine) -> Self {
        Analysis {
            pattern_engine: Some(engine),
            ..self
        }
    }

    /// Enable time-sampled collection. Collectors that return
    /// `supports_sampling() == true` will loop N = floor(duration/interval)+1
    /// times and populate `Signal::samples`.
    pub fn with_sample_duration(self, duration: Duration, interval: Duration) -> Self {
        Analysis {
            sample_duration: Some(duration),
            sample_interval: Some(interval),
            ..self
        }
    }

    /// Set the cgroup path threaded into `CollectCtx` for the cgroup
    /// collector (Phase 3 follow-up implements the collector itself).
    pub fn with_cgroup<P: Into<PathBuf>>(self, path: P) -> Self {
        Analysis {
            cgroup_path: Some(path.into()),
            ..self
        }
    }

    /// Install baseline records used to annotate signals (and emit
    /// auto-outlier findings) during the diagnostic pipeline (SDD §116).
    pub fn with_baseline_records(self, records: Vec<BaselineRecord>) -> Self {
        Analysis {
            baseline_records: records,
            ..self
        }
    }

    pub fn run(&self, context: Context) -> Result<AnalysisReport> {
        let hostinfo_results = self.run_commands(self.hostinfos)?;
        let command_results = self.run_commands_rep(self.commands, self.repetitions)?;

        let (signals, findings) = self.run_diagnostics();

        Ok(AnalysisReport {
            context,
            hostinfo_results,
            command_results,
            repetitions: self.repetitions,
            max_parallel_commands: self.max_parallel_commands,
            signals,
            findings,
            checked_ok: Vec::new(),
        })
    }

    fn run_diagnostics(&self) -> (Vec<Signal>, Vec<Finding>) {
        if self.collectors.is_empty() && self.rule_engine.is_none() && self.baseline_records.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let ctx = CollectCtx {
            duration: self.sample_duration,
            interval: self.sample_interval,
            cgroup_path: self.cgroup_path.clone(),
            baseline: None,
            cpu_count: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        };
        let mut signals: Vec<Signal> = Vec::new();
        for c in &self.collectors {
            match c.collect(&ctx) {
                Ok(mut more) => signals.append(&mut more),
                Err(e) => log::warn!("collector '{}' failed: {}", c.id(), e),
            }
        }
        if !self.baseline_records.is_empty() {
            annotate(&mut signals, &self.baseline_records);
        }
        let mut findings = match &self.rule_engine {
            Some(engine) => engine.run(&signals, &ctx),
            None => Vec::new(),
        };
        if let Some(pe) = &self.pattern_engine {
            findings.extend(pe.run(&signals, &ctx));
        }
        if !self.baseline_records.is_empty() {
            findings.extend(outlier_findings(&signals));
        }
        if !findings.is_empty() {
            sort_findings(&mut findings);
        }
        (signals, findings)
    }

    fn run_commands_rep(&self, commands: I, repetitions: usize) -> Result<Vec<Vec<CommandResult>>> {
        let mut results = Vec::new();
        for _ in 0..repetitions {
            let run_results = self.run_commands(commands)?;
            results.push(run_results);
        }

        Ok(results)
    }

    fn run_commands(&self, commands: I) -> Result<Vec<CommandResult>> {
        let results = self.runner.run(commands, self.max_parallel_commands)?;

        Ok(results)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub(crate) context: Context,
    pub(crate) hostinfo_results: Vec<CommandResult>,
    pub(crate) command_results: Vec<Vec<CommandResult>>,
    pub(crate) repetitions: usize,
    pub(crate) max_parallel_commands: usize,
    #[serde(default)]
    pub(crate) signals: Vec<Signal>,
    #[serde(default)]
    pub(crate) findings: Vec<Finding>,
    #[serde(default)]
    pub(crate) checked_ok: Vec<String>,
}

impl AnalysisReport {
    pub fn new(
        context: Context,
        hostinfo_results: Vec<CommandResult>,
        command_results: Vec<Vec<CommandResult>>,
        repetitions: usize,
        max_parallel_commands: usize,
    ) -> AnalysisReport {
        AnalysisReport {
            context,
            hostinfo_results,
            command_results,
            repetitions,
            max_parallel_commands,
            signals: Vec::new(),
            findings: Vec::new(),
            checked_ok: Vec::new(),
        }
    }

    /// Constructor for diagnostic-aware reports — the rule engine has already
    /// produced findings for the given signals.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_diagnostics(
        context: Context,
        hostinfo_results: Vec<CommandResult>,
        command_results: Vec<Vec<CommandResult>>,
        repetitions: usize,
        max_parallel_commands: usize,
        signals: Vec<Signal>,
        findings: Vec<Finding>,
        checked_ok: Vec<String>,
    ) -> AnalysisReport {
        AnalysisReport {
            context,
            hostinfo_results,
            command_results,
            repetitions,
            max_parallel_commands,
            signals,
            findings,
            checked_ok,
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn hostinfo_results(&self) -> &[CommandResult] {
        &self.hostinfo_results
    }

    pub fn command_results(&self) -> &[Vec<CommandResult>] {
        &self.command_results
    }

    pub fn repetitions(&self) -> usize {
        self.repetitions
    }

    pub fn max_parallel_commands(&self) -> usize {
        self.max_parallel_commands
    }

    pub fn signals(&self) -> &[Signal] {
        &self.signals
    }

    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    pub fn checked_ok(&self) -> &[String] {
        &self.checked_ok
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Context {
    pub(crate) hostname: String,
    pub(crate) uname: String,
    pub(crate) date_time: DateTime<Local>,
    pub(crate) more: HashMap<String, String>,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Context {
        let utsname = rustix::system::uname();
        let hostname = utsname.nodename().to_string_lossy().into_owned();
        let uname = format!(
            "{} {} {} {} {}",
            utsname.sysname().to_string_lossy(),
            utsname.nodename().to_string_lossy(),
            utsname.release().to_string_lossy(),
            utsname.version().to_string_lossy(),
            utsname.machine().to_string_lossy(),
        );
        let date_time = Local::now();

        Context {
            uname,
            hostname,
            date_time,
            more: Default::default(),
        }
    }

    pub fn add<T: Into<String>>(&mut self, key: T, value: T) -> &mut Context {
        let _ = self.more.insert(key.into(), value.into());
        self
    }

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn uname(&self) -> &str {
        &self.uname
    }

    pub fn date_time(&self) -> &DateTime<Local> {
        &self.date_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use googletest::prelude::*;

    #[test]
    fn run_ok() {
        let hostinfos: Vec<Command> = Vec::new();
        let commands: Vec<Command> = Vec::new();
        let tr = runner::ThreadRunner::new();
        let runner = Box::new(tr);
        let analysis = Analysis::new(runner, &hostinfos, &commands)
            .with_repetitions(1)
            .with_max_parallel_commands(64);
        let context = Context::new();

        let res = analysis.run(context);
        assert_that!(res, ok(anything()));
    }

    #[test]
    fn run_with_real_command_produces_report() {
        #[cfg(target_os = "macos")]
        let cmd = Command::new("uname", r#"/usr/bin/uname -a"#).with_timeout(5u64);
        #[cfg(target_os = "linux")]
        let cmd = Command::new("uname", r#"/bin/uname -a"#).with_timeout(5u64);

        let hostinfos = vec![cmd.clone()];
        let commands = vec![cmd];
        let tr = runner::ThreadRunner::new();
        let analysis = Analysis::new(Box::new(tr), &hostinfos, &commands)
            .with_repetitions(1)
            .with_max_parallel_commands(64);
        let context = Context::new();

        let report = analysis.run(context).expect("analysis ok");

        assert_eq!(report.hostinfo_results().len(), 1);
        assert_eq!(report.command_results().len(), 1); // 1 repetition
        assert_eq!(report.command_results()[0].len(), 1);
        assert_that!(report.context().hostname(), not(eq("")));
        assert_that!(
            report.hostinfo_results()[0],
            matches_pattern!(crate::CommandResult::Success { .. })
        );
    }

    #[test]
    fn second_runner() {
        #[derive(Debug)]
        struct MyRunner {}

        impl<'a, I: IntoIterator<Item = &'a Command>> Runner<'a, I> for MyRunner {
            fn run(&self, _commands: I, _max_parallel_commands: usize) -> runner::Result<Vec<CommandResult>> {
                Ok(Vec::new())
            }
        }

        let hostinfos: Vec<Command> = Vec::new();
        let commands: Vec<Command> = Vec::new();
        let runner = Box::new(MyRunner {});
        let analysis = Analysis::new(runner, hostinfos.as_slice(), commands.as_slice())
            .with_repetitions(1)
            .with_max_parallel_commands(64);
        let context = Context::new();

        let res = analysis.run(context);
        assert_that!(res, ok(anything()));
    }
}
