#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCheck {
    pub name: &'static str,
    pub status: HealthStatus,
    pub detail: &'static str,
}

impl HealthCheck {
    pub fn liveness() -> Self {
        Self {
            name: "liveness",
            status: HealthStatus::Pass,
            detail: "process is running",
        }
    }

    pub fn memgraph_unchecked() -> Self {
        Self {
            name: "memgraph",
            status: HealthStatus::Warn,
            detail: "driver integration is introduced in G04",
        }
    }
}
