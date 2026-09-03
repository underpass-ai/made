use std::time::Duration;

/// How the pool is configured.
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub url: String,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
}

impl PostgresConfig {
    #[must_use]
    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            max_connections: 10,
            acquire_timeout: Duration::from_secs(5),
        }
    }
}
