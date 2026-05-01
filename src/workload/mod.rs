//! Workload rule packs — named TOML rule sets for specific workloads.
//!
//! Call `load_workload_rules("postgres")` to get a `Vec<Rule>` to merge with
//! the base rule engine. The special name `"none"` returns an empty vec.

use thiserror::Error;

use crate::rule::{parse_rules_toml, Rule};

const POSTGRES: &str = include_str!("../../contrib/rules/workloads/postgres.toml");
const JAVA: &str = include_str!("../../contrib/rules/workloads/java.toml");
const NGINX: &str = include_str!("../../contrib/rules/workloads/nginx.toml");
const KUBELET: &str = include_str!("../../contrib/rules/workloads/kubelet.toml");

#[derive(Debug, Error)]
pub enum WorkloadError {
    #[error("unknown workload '{0}'; known workloads: postgres, java, nginx, kubelet, none")]
    Unknown(String),
    #[error("failed to parse workload rule file for '{name}': {source}")]
    ParseFailed { name: String, source: crate::rule::Error },
}

pub type Result<T, E = WorkloadError> = std::result::Result<T, E>;

/// Load rules for a named workload. `"none"` returns an empty vec (no error).
/// Unknown names return `WorkloadError::Unknown`.
pub fn load_workload_rules(name: &str) -> Result<Vec<Rule>> {
    let src = match name {
        "none" => return Ok(Vec::new()),
        "postgres" => POSTGRES,
        "java" => JAVA,
        "nginx" => NGINX,
        "kubelet" => KUBELET,
        other => return Err(WorkloadError::Unknown(other.to_string())),
    };
    parse_rules_toml(src).map_err(|source| WorkloadError::ParseFailed {
        name: name.to_string(),
        source,
    })
}
