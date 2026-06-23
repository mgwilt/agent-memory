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
    pub fn from_env() -> Result<Self, String> {
        let mut config = Self::default();
        if let Ok(bind_addr) = std::env::var("ACTR_API_BIND_ADDR") {
            config.bind_addr = bind_addr;
        }
        if let Ok(memgraph_uri) = std::env::var("ACTR_MEMGRAPH_URI") {
            config.memgraph_uri = memgraph_uri;
        }
        if let Ok(memgraph_user) = std::env::var("ACTR_MEMGRAPH_USER") {
            config.memgraph_user = memgraph_user;
        }
        if let Ok(candidate_limit) = std::env::var("ACTR_CANDIDATE_LIMIT") {
            config.candidate_limit = candidate_limit
                .parse()
                .map_err(|_| "ACTR_CANDIDATE_LIMIT must be a positive integer".to_string())?;
        }
        if let Ok(retrieval_threshold) = std::env::var("ACTR_RETRIEVAL_THRESHOLD") {
            config.retrieval_threshold = retrieval_threshold
                .parse()
                .map_err(|_| "ACTR_RETRIEVAL_THRESHOLD must be a finite number".to_string())?;
        }
        if let Ok(seed) = std::env::var("ACTR_DETERMINISTIC_SEED") {
            config.deterministic_seed = if seed.trim().is_empty() {
                None
            } else {
                Some(seed.parse().map_err(|_| {
                    "ACTR_DETERMINISTIC_SEED must be an unsigned integer".to_string()
                })?)
            };
        }
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.candidate_limit == 0 {
            return Err("candidate_limit must be greater than zero".to_string());
        }
        if self.candidate_limit > 200 {
            return Err("candidate_limit must be at most 200".to_string());
        }
        if !self.retrieval_threshold.is_finite() {
            return Err("retrieval_threshold must be finite".to_string());
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
