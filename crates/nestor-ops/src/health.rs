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
    pub detail: String,
}

impl HealthCheck {
    pub fn liveness() -> Self {
        Self {
            name: "liveness",
            status: HealthStatus::Pass,
            detail: "process is running".to_string(),
        }
    }

    pub fn memgraph_ready() -> Self {
        Self {
            name: "memgraph",
            status: HealthStatus::Pass,
            detail: "Memgraph connection is ready".to_string(),
        }
    }

    pub fn memgraph_failed(detail: impl Into<String>) -> Self {
        Self {
            name: "memgraph",
            status: HealthStatus::Fail,
            detail: detail.into(),
        }
    }
}
