use crate::{runner, Command, CommandResult, Runner};

use chrono::{DateTime, Local};
use serde::Serialize;
use snafu::{ResultExt, Snafu};
use uname;

/// Error type
#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum Error {
    /// Analysis initialization failed
    #[snafu(display("analysis initialization failed because {}", source))]
    InitFailed { source: std::io::Error },
    /// Analysis run failed
    #[snafu(display("analysis failed because {}", source))]
    AnalysisFailed { source: runner::Error },
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

// Copy: This allows to reuse the into_iter object; safe for &Vec or &[]
pub struct Analysis<'a, I: IntoIterator<Item = &'a Command> + Copy> {
    runner:                Box<dyn Runner<'a, I>>,
    hostinfos:             I,
    commands:              I,
    repetitions:           u64,
    max_parallel_commands: u64,
}

impl<'a, I: IntoIterator<Item = &'a Command> + Copy> Analysis<'a, I> {
    pub fn new(runner: Box<dyn Runner<'a, I>>, hostinfos: I, commands: I) -> Self {
        Analysis {
            hostinfos,
            commands,
            runner,
            repetitions: 1,
            max_parallel_commands: 64,
        }
    }

    pub fn set_repetitions(self, repetitions: u64) -> Self { Analysis { repetitions, ..self } }

    pub fn set_max_parallel_commands(self, max_parallel_commands: u64) -> Self {
        Analysis {
            max_parallel_commands,
            ..self
        }
    }

    pub fn run(&self) -> Result<AnalysisResult> {
        let uname = uname::uname().context(InitFailed {})?;
        let hostname = uname.nodename.to_string();
        let uname = format!(
            "{} {} {} {} {}",
            uname.sysname, uname.nodename, uname.release, uname.version, uname.machine
        );
        let date_time = Local::now();

        let hostinfo_results = self.run_commands(self.hostinfos)?;
        let command_results = self.run_commands_rep(self.commands, self.repetitions)?;

        Ok(AnalysisResult {
            hostname,
            uname,
            date_time,
            hostinfo_results,
            command_results,
            repetitions: self.repetitions,
            max_parallel_commands: self.max_parallel_commands,
        })
    }

    fn run_commands_rep(&self, commands: I, repetitions: u64) -> Result<Vec<Vec<CommandResult>>> {
        let mut results = Vec::new();
        for _ in 0..repetitions {
            let run_results = self.run_commands(commands)?;
            results.push(run_results);
        }

        Ok(results)
    }

    fn run_commands(&self, commands: I) -> Result<Vec<CommandResult>> {
        let results = self.runner.run(commands).context(AnalysisFailed {})?;

        Ok(results)
    }
}

#[derive(Debug, Serialize)]
pub struct AnalysisResult {
    pub hostname:              String,
    pub uname:                 String,
    pub date_time:             DateTime<Local>,
    pub hostinfo_results:      Vec<CommandResult>,
    pub command_results:       Vec<Vec<CommandResult>>,
    pub repetitions:           u64,
    pub max_parallel_commands: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    use spectral::prelude::*;

    #[test]
    fn ok() {
        let hostinfos: Vec<Command> = Vec::new();
        let commands: Vec<Command> = Vec::new();
        let tr = runner::ThreadRunner::new();
        let runner = Box::new(tr);
        let analysis = Analysis::new(runner, &hostinfos, &commands)
            .set_repetitions(1)
            .set_max_parallel_commands(64);

        let res = analysis.run();
        asserting("Analysis run").that(&res).is_ok();
    }

    #[test]
    fn second_runner() {
        struct MyRunner {};

        impl<'a, I: IntoIterator<Item = &'a Command>> Runner<'a, I> for MyRunner {
            fn run(&self, _commands: I) -> runner::Result<Vec<CommandResult>> { Ok(Vec::new()) }
        }

        let hostinfos: Vec<Command> = Vec::new();
        let commands: Vec<Command> = Vec::new();
        let runner = Box::new(MyRunner {});
        let analysis = Analysis::new(runner, hostinfos.as_slice(), commands.as_slice())
            .set_repetitions(1)
            .set_max_parallel_commands(64);

        let res = analysis.run();
        asserting("Analysis run").that(&res).is_ok();
    }
}
