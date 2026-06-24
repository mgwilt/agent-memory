use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiClientConfig {
    pub api_url: String,
    pub timeout: Duration,
}

impl ApiClientConfig {
    pub fn new(api_url: impl Into<String>, timeout: Duration) -> Self {
        Self {
            api_url: api_url.into().trim_end_matches('/').to_string(),
            timeout,
        }
    }
}

impl Default for ApiClientConfig {
    fn default() -> Self {
        Self::new("http://127.0.0.1:8080", Duration::from_millis(5_000))
    }
}
