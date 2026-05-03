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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::{Aggregate, CommandExtract};
    use crate::signal::Unit;

    fn make_extract(pattern: &str, aggregate: Aggregate) -> CommandExtract {
        CommandExtract {
            pattern: pattern.to_string(),
            signal_id: "test.signal".to_string(),
            unit: Unit::None,
            aggregate,
        }
    }

    #[test]
    fn aggregate_min_returns_smallest_value() {
        let extracts = vec![make_extract(r"val=(?P<val>\d+)", Aggregate::Min)];
        let stdout = "val=10\nval=3\nval=7\n";
        let signals = extract_signals("test_cmd", stdout, &extracts);
        assert_eq!(signals.len(), 1);
        match signals[0].value {
            crate::signal::SignalValue::F64(v) => assert!((v - 3.0).abs() < f64::EPSILON, "expected min=3.0, got {v}"),
            _ => panic!("expected F64 signal value"),
        }
    }
}
