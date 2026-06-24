use std::{collections::BTreeMap, fs};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    #[default]
    Development,
    Staging,
    Production,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryBackend {
    #[default]
    Memgraph,
    InMemory,
}

impl RepositoryBackend {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "memgraph" => Ok(Self::Memgraph),
            "memory" | "in-memory" | "in_memory" => Ok(Self::InMemory),
            _ => Err("NESTOR_REPOSITORY must be memgraph or memory".to_string()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Memgraph => "memgraph",
            Self::InMemory => "memory",
        }
    }
}

impl RuntimeProfile {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dev" | "development" => Ok(Self::Development),
            "stage" | "staging" => Ok(Self::Staging),
            "prod" | "production" => Ok(Self::Production),
            _ => {
                Err("NESTOR_PROFILE must be one of development, staging, or production".to_string())
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretSource {
    EnvVar(String),
    File(String),
}

impl SecretSource {
    fn validate(&self) -> Result<(), String> {
        match self {
            Self::EnvVar(name) => {
                if name.trim().is_empty() {
                    return Err("memgraph credential env var name must not be empty".to_string());
                }
                if !name
                    .chars()
                    .all(|character| character == '_' || character.is_ascii_alphanumeric())
                {
                    return Err(
                        "memgraph credential env var name must be ASCII alphanumeric or underscore"
                            .to_string(),
                    );
                }
            }
            Self::File(path) => {
                if path.trim().is_empty() {
                    return Err("memgraph credential file path must not be empty".to_string());
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemgraphSecurityConfig {
    pub tls_enabled: bool,
    pub tls_ca_file: Option<String>,
    pub tls_server_name: Option<String>,
    pub credentials: Option<SecretSource>,
}

impl MemgraphSecurityConfig {
    pub fn development() -> Self {
        Self {
            tls_enabled: false,
            tls_ca_file: None,
            tls_server_name: None,
            credentials: None,
        }
    }

    pub fn hardened() -> Self {
        Self {
            tls_enabled: true,
            tls_ca_file: None,
            tls_server_name: Some("memgraph".to_string()),
            credentials: Some(SecretSource::EnvVar("NESTOR_MEMGRAPH_PASSWORD".to_string())),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self
            .tls_ca_file
            .as_ref()
            .is_some_and(|path| path.trim().is_empty())
        {
            return Err("memgraph TLS CA file path must not be empty".to_string());
        }
        if self
            .tls_server_name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err("memgraph TLS server name must not be empty".to_string());
        }
        if let Some(credentials) = &self.credentials {
            credentials.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeConfig {
    pub profile: RuntimeProfile,
    pub repository_backend: RepositoryBackend,
    pub bind_addr: String,
    pub memgraph_uri: String,
    pub memgraph_user: String,
    pub memgraph_max_connections: usize,
    pub memgraph_security: MemgraphSecurityConfig,
    pub candidate_limit: usize,
    pub retrieval_threshold: f64,
    pub deterministic_seed: Option<u64>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::for_profile(RuntimeProfile::Development)
    }
}

impl RuntimeConfig {
    pub fn for_profile(profile: RuntimeProfile) -> Self {
        let memgraph_security = match profile {
            RuntimeProfile::Development => MemgraphSecurityConfig::development(),
            RuntimeProfile::Staging | RuntimeProfile::Production => {
                MemgraphSecurityConfig::hardened()
            }
        };
        let (bind_addr, memgraph_uri, deterministic_seed) = match profile {
            RuntimeProfile::Development => (
                "127.0.0.1:8080".to_string(),
                "bolt://127.0.0.1:7687".to_string(),
                Some(42),
            ),
            RuntimeProfile::Staging => (
                "0.0.0.0:8080".to_string(),
                "bolt+s://memgraph.staging.internal:7687".to_string(),
                Some(42),
            ),
            RuntimeProfile::Production => (
                "0.0.0.0:8080".to_string(),
                "bolt+s://memgraph.production.internal:7687".to_string(),
                None,
            ),
        };

        Self {
            profile,
            repository_backend: RepositoryBackend::Memgraph,
            bind_addr,
            memgraph_uri,
            memgraph_user: "memgraph".to_string(),
            memgraph_max_connections: 16,
            memgraph_security,
            candidate_limit: 200,
            retrieval_threshold: 0.25,
            deterministic_seed,
        }
    }

    pub fn from_env() -> Result<Self, String> {
        Self::from_env_vars(std::env::vars())
    }

    pub fn from_env_vars(vars: impl IntoIterator<Item = (String, String)>) -> Result<Self, String> {
        let vars = vars.into_iter().collect::<BTreeMap<_, _>>();
        let profile = vars
            .get("NESTOR_PROFILE")
            .map(|value| RuntimeProfile::parse(value))
            .transpose()?
            .unwrap_or_default();
        let mut config = Self::for_profile(profile);

        if let Some(repository) = vars.get("NESTOR_REPOSITORY") {
            config.repository_backend = RepositoryBackend::parse(repository)?;
        }
        if let Some(bind_addr) = vars.get("NESTOR_API_BIND_ADDR") {
            config.bind_addr = bind_addr.clone();
        }
        if let Some(memgraph_uri) = vars.get("NESTOR_MEMGRAPH_URI") {
            config.memgraph_uri = memgraph_uri.clone();
        }
        if let Some(memgraph_user) = vars.get("NESTOR_MEMGRAPH_USER") {
            config.memgraph_user = memgraph_user.clone();
        }
        if let Some(max_connections) = vars.get("NESTOR_MEMGRAPH_MAX_CONNECTIONS") {
            config.memgraph_max_connections = max_connections.parse().map_err(|_| {
                "NESTOR_MEMGRAPH_MAX_CONNECTIONS must be a positive integer".to_string()
            })?;
        }
        if let Some(candidate_limit) = vars.get("NESTOR_CANDIDATE_LIMIT") {
            config.candidate_limit = candidate_limit
                .parse()
                .map_err(|_| "NESTOR_CANDIDATE_LIMIT must be a positive integer".to_string())?;
        }
        if let Some(retrieval_threshold) = vars.get("NESTOR_RETRIEVAL_THRESHOLD") {
            config.retrieval_threshold = retrieval_threshold
                .parse()
                .map_err(|_| "NESTOR_RETRIEVAL_THRESHOLD must be a finite number".to_string())?;
        }
        if let Some(seed) = vars.get("NESTOR_DETERMINISTIC_SEED") {
            config.deterministic_seed = if seed.trim().is_empty() {
                None
            } else {
                Some(seed.parse().map_err(|_| {
                    "NESTOR_DETERMINISTIC_SEED must be an unsigned integer".to_string()
                })?)
            };
        }

        if let Some(tls_enabled) = vars.get("NESTOR_MEMGRAPH_TLS_ENABLED") {
            config.memgraph_security.tls_enabled = parse_bool(
                tls_enabled,
                "NESTOR_MEMGRAPH_TLS_ENABLED must be true or false",
            )?;
        }
        if let Some(ca_file) = vars.get("NESTOR_MEMGRAPH_TLS_CA_FILE") {
            config.memgraph_security.tls_ca_file = empty_as_none(ca_file);
        }
        if let Some(server_name) = vars.get("NESTOR_MEMGRAPH_TLS_SERVER_NAME") {
            config.memgraph_security.tls_server_name = empty_as_none(server_name);
        }
        if let Some(password_env) = vars.get("NESTOR_MEMGRAPH_PASSWORD_ENV") {
            config.memgraph_security.credentials =
                empty_as_none(password_env).map(SecretSource::EnvVar);
        } else if vars
            .get("NESTOR_MEMGRAPH_PASSWORD")
            .is_some_and(|password| !password.trim().is_empty())
        {
            config.memgraph_security.credentials =
                Some(SecretSource::EnvVar("NESTOR_MEMGRAPH_PASSWORD".to_string()));
        }
        if let Some(password_file) = vars.get("NESTOR_MEMGRAPH_PASSWORD_FILE") {
            config.memgraph_security.credentials =
                empty_as_none(password_file).map(SecretSource::File);
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
        if self.memgraph_user.trim().is_empty() {
            return Err("memgraph_user must not be empty".to_string());
        }
        if self.memgraph_max_connections == 0 {
            return Err("memgraph_max_connections must be greater than zero".to_string());
        }
        if self.memgraph_uri.starts_with("bolt+s://") && !self.memgraph_security.tls_enabled {
            return Err("secure Memgraph URI requires TLS to be enabled".to_string());
        }
        let production_loopback = self.profile == RuntimeProfile::Production
            && (self.memgraph_uri.contains("127.0.0.1") || self.memgraph_uri.contains("localhost"));
        if production_loopback {
            return Err("production Memgraph URI must not use loopback hosts".to_string());
        }
        if self.profile == RuntimeProfile::Production && !self.memgraph_security.tls_enabled {
            return Err("production profile requires Memgraph TLS".to_string());
        }
        if matches!(
            self.profile,
            RuntimeProfile::Staging | RuntimeProfile::Production
        ) && self.memgraph_security.credentials.is_none()
        {
            return Err("staging and production profiles require Memgraph credentials".to_string());
        }
        self.memgraph_security.validate()?;
        Ok(())
    }

    pub fn resolve_memgraph_password(&self) -> Result<String, String> {
        match &self.memgraph_security.credentials {
            Some(SecretSource::EnvVar(name)) => std::env::var(name)
                .map_err(|_| format!("Memgraph password env var {name} is not set")),
            Some(SecretSource::File(path)) => fs::read_to_string(path)
                .map(|value| value.trim_end_matches(['\r', '\n']).to_string())
                .map_err(|error| format!("failed to read Memgraph password file {path}: {error}")),
            None => Ok(String::new()),
        }
    }
}

fn empty_as_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_bool(value: &str, error: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = RuntimeConfig::default();

        assert!(config.validate().is_ok());
        assert_eq!(config.repository_backend, RepositoryBackend::Memgraph);
    }

    #[test]
    fn repository_backend_parses_default_and_explicit_memory_opt_in() {
        assert_eq!(
            RepositoryBackend::parse("memgraph"),
            Ok(RepositoryBackend::Memgraph)
        );
        assert_eq!(
            RepositoryBackend::parse("memory"),
            Ok(RepositoryBackend::InMemory)
        );
        assert_eq!(
            RepositoryBackend::parse("in-memory"),
            Ok(RepositoryBackend::InMemory)
        );
        assert_eq!(RepositoryBackend::Memgraph.as_str(), "memgraph");
        assert_eq!(RepositoryBackend::InMemory.as_str(), "memory");

        let config =
            RuntimeConfig::from_env_vars([("NESTOR_REPOSITORY".to_string(), "memory".to_string())])
                .expect("memory backend should parse");

        assert_eq!(config.repository_backend, RepositoryBackend::InMemory);
    }

    #[test]
    fn invalid_repository_backend_is_rejected() {
        let error =
            RuntimeConfig::from_env_vars([("NESTOR_REPOSITORY".to_string(), "sqlite".to_string())])
                .expect_err("unknown backend should fail");

        assert!(error.contains("NESTOR_REPOSITORY"));
    }

    #[test]
    fn profile_defaults_are_valid_and_harden_non_development() {
        let development = RuntimeConfig::for_profile(RuntimeProfile::Development);
        let staging = RuntimeConfig::for_profile(RuntimeProfile::Staging);
        let production = RuntimeConfig::for_profile(RuntimeProfile::Production);

        assert!(development.validate().is_ok());
        assert!(staging.validate().is_ok());
        assert!(production.validate().is_ok());
        assert!(!development.memgraph_security.tls_enabled);
        assert!(staging.memgraph_security.tls_enabled);
        assert!(production.memgraph_security.tls_enabled);
        assert_eq!(production.deterministic_seed, None);
    }

    #[test]
    fn from_env_vars_parses_profile_and_security_overrides() {
        let config = RuntimeConfig::from_env_vars([
            ("NESTOR_PROFILE".to_string(), "prod".to_string()),
            (
                "NESTOR_MEMGRAPH_URI".to_string(),
                "bolt+s://memgraph.private:7687".to_string(),
            ),
            (
                "NESTOR_MEMGRAPH_PASSWORD_FILE".to_string(),
                "/run/secrets/memgraph-password".to_string(),
            ),
            (
                "NESTOR_MEMGRAPH_TLS_CA_FILE".to_string(),
                "/etc/ssl/private/memgraph-ca.pem".to_string(),
            ),
            (
                "NESTOR_MEMGRAPH_TLS_SERVER_NAME".to_string(),
                "memgraph.private".to_string(),
            ),
            ("NESTOR_CANDIDATE_LIMIT".to_string(), "64".to_string()),
            ("NESTOR_RETRIEVAL_THRESHOLD".to_string(), "0.5".to_string()),
        ])
        .expect("profile should parse");

        assert_eq!(config.profile, RuntimeProfile::Production);
        assert_eq!(config.candidate_limit, 64);
        assert_eq!(config.retrieval_threshold, 0.5);
        assert_eq!(
            config.memgraph_security.credentials,
            Some(SecretSource::File(
                "/run/secrets/memgraph-password".to_string()
            ))
        );
        assert_eq!(
            config.memgraph_security.tls_server_name.as_deref(),
            Some("memgraph.private")
        );
    }

    #[test]
    fn production_rejects_loopback_memgraph_without_tls_or_credentials() {
        let mut config = RuntimeConfig::for_profile(RuntimeProfile::Production);
        config.memgraph_uri = "bolt://127.0.0.1:7687".to_string();
        config.memgraph_security.tls_enabled = false;
        config.memgraph_security.credentials = None;

        let error = config.validate().expect_err("config should be rejected");

        assert!(
            error.contains("secure Memgraph URI")
                || error.contains("production Memgraph URI")
                || error.contains("production profile")
                || error.contains("credentials")
        );
    }

    #[test]
    fn invalid_profile_and_bounds_are_reported() {
        let profile_error =
            RuntimeConfig::from_env_vars([("NESTOR_PROFILE".to_string(), "sandbox".to_string())])
                .expect_err("invalid profile should fail");
        assert!(profile_error.contains("NESTOR_PROFILE"));

        let bound_error =
            RuntimeConfig::from_env_vars([("NESTOR_CANDIDATE_LIMIT".to_string(), "0".to_string())])
                .expect_err("zero candidate limit should fail");
        assert!(bound_error.contains("candidate_limit"));

        let pool_error = RuntimeConfig::from_env_vars([(
            "NESTOR_MEMGRAPH_MAX_CONNECTIONS".to_string(),
            "0".to_string(),
        )])
        .expect_err("zero Memgraph pool size should fail");
        assert!(pool_error.contains("memgraph_max_connections"));
    }

    #[test]
    fn resolve_memgraph_password_from_file_trims_line_endings()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "nestor-memgraph-password-{}.txt",
            std::process::id()
        ));
        std::fs::write(&path, "secret\r\n")?;

        let config = RuntimeConfig::from_env_vars([(
            "NESTOR_MEMGRAPH_PASSWORD_FILE".to_string(),
            path.to_string_lossy().to_string(),
        )])?;

        assert_eq!(config.resolve_memgraph_password()?, "secret");
        std::fs::remove_file(path)?;
        Ok(())
    }
}
