use crate::cli::config::{Aggregate, CommandExtract};
use crate::signal::{Signal, SignalValue};
use regex::Regex;

pub fn extract_signals(command_name: &str, stdout: &str, extracts: &[CommandExtract]) -> Vec<Signal> {
    let mut out = Vec::new();

    for extract in extracts {
        let re = Regex::new(&extract.pattern).expect("extract pattern pre-validated");

        if extract.aggregate == Aggregate::Count {
            let count = stdout.lines().filter(|line| re.is_match(line)).count();
            out.push(Signal {
                id: extract.signal_id.clone(),
                value: SignalValue::I64(count as i64),
                unit: extract.unit,
                at: chrono::Local::now(),
                samples: None,
                stats: None,
                baseline: None,
            });
            continue;
        }

        let values: Vec<f64> = stdout
            .lines()
            .filter_map(|line| {
                let caps = re.captures(line)?;
                let val_str = caps.name("val")?.as_str();
                match val_str.parse::<f64>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        log::warn!(
                            "extract: non-numeric capture command={command_name} pattern={} value={val_str}",
                            extract.pattern
                        );
                        None
                    }
                }
            })
            .collect();

        if values.is_empty() {
            log::warn!(
                "extract: no matches for command={command_name} pattern={}",
                extract.pattern
            );
            continue;
        }

        let result = match extract.aggregate {
            Aggregate::Last => *values.last().unwrap(),
            Aggregate::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            Aggregate::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
            Aggregate::Avg => values.iter().sum::<f64>() / values.len() as f64,
            Aggregate::Count => unreachable!(),
        };

        out.push(Signal {
            id: extract.signal_id.clone(),
            value: SignalValue::F64(result),
            unit: extract.unit,
            at: chrono::Local::now(),
            samples: None,
            stats: None,
            baseline: None,
        });
    }

    out
}
