use thiserror::Error;

#[derive(Debug, Error)]
pub enum PostgresPoolError {
    #[error("postgres connect failed: {0}")]
    Connect(#[source] sqlx::Error),
    #[error("postgres migrations failed: {0}")]
    Migrate(#[source] sqlx::migrate::MigrateError),
    #[error("postgres health check failed: {0}")]
    HealthCheck(#[source] sqlx::Error),
}
