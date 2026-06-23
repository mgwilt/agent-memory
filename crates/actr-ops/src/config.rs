#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeConfig {
    pub profile: RuntimeProfile,
    pub bind_addr: String,
    pub memgraph_uri: String,
    pub memgraph_user: String,
    pub candidate_limit: usize,
    pub retrieval_threshold: f64,
    pub deterministic_seed: Option<u64>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            profile: RuntimeProfile::Development,
            bind_addr: "127.0.0.1:8080".to_string(),
            memgraph_uri: "bolt://127.0.0.1:7687".to_string(),
            memgraph_user: "memgraph".to_string(),
            candidate_limit: 200,
            retrieval_threshold: 0.25,
            deterministic_seed: Some(42),
        }
    }
}

impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.candidate_limit == 0 {
            return Err("candidate_limit must be greater than zero".to_string());
        }
        if self.bind_addr.trim().is_empty() {
            return Err("bind_addr must not be empty".to_string());
        }
        if self.memgraph_uri.trim().is_empty() {
            return Err("memgraph_uri must not be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        assert!(RuntimeConfig::default().validate().is_ok());
    }
}
