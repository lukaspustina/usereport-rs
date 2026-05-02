use crate::finding::Severity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VitalSigns {
    pub cpu: CpuVitalSigns,
    pub memory: MemoryVitalSigns,
    pub disk: DiskVitalSigns,
    pub network: NetworkVitalSigns,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuVitalSigns {
    pub iowait_pct: Option<f64>,
    pub severity: Option<Severity>,
    pub trend: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryVitalSigns {
    pub used_pct: Option<f64>,
    pub severity: Option<Severity>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskVitalSigns {
    pub util_pct: Option<f64>,
    pub severity: Option<Severity>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkVitalSigns {
    pub util_pct: Option<f64>,
    pub severity: Option<Severity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseCoverageEntry {
    pub resource: String,
    pub aspect: String,
    pub covered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UseDimension {
    pub resource: String,
    pub aspect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileFollowup {
    pub finding: String,
    pub recommend: String,
    pub reason: String,
}
