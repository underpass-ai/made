//! Shared fixture for Postgres integration tests.
//!
//! Every test gets its own Postgres container — keeps tests isolated
//! and lets testcontainers reap resources cleanly per case. The
//! `start` helper returns both the pool (with migrations applied)
//! and the container handle so the test's lifetime keeps the
//! container alive.

use std::time::Duration;

use made_adapters::postgres::{PostgresConfig, PostgresPool};
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

const PG_IMAGE: &str = "postgres";
const PG_TAG: &str = "16-alpine";
const PG_USER: &str = "made";
const PG_PASSWORD: &str = "made";
const PG_DB: &str = "made";

pub async fn start() -> (PostgresPool, testcontainers::ContainerAsync<GenericImage>) {
    let (pool, _url, container) = start_with_url().await;
    (pool, container)
}

pub async fn start_with_url() -> (
    PostgresPool,
    String,
    testcontainers::ContainerAsync<GenericImage>,
) {
    let container = GenericImage::new(PG_IMAGE, PG_TAG)
        .with_exposed_port(5432_u16.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_USER", PG_USER)
        .with_env_var("POSTGRES_PASSWORD", PG_PASSWORD)
        .with_env_var("POSTGRES_DB", PG_DB)
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432_u16.tcp())
        .await
        .expect("host port");
    let url = format!("postgres://{PG_USER}:{PG_PASSWORD}@127.0.0.1:{port}/{PG_DB}");

    let mut cfg = PostgresConfig::from_url(url.clone());
    cfg.acquire_timeout = Duration::from_secs(10);

    let scale: f64 = std::env::var("MADE_TEST_TIMING_SCALE")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|scale| *scale >= 1.0)
        .unwrap_or(1.0);
    let mut last_err = None;
    let attempts = (20.0 * scale).ceil() as u32;
    for _ in 0..attempts {
        match PostgresPool::connect(&cfg).await {
            Ok(pool) => {
                pool.run_migrations()
                    .await
                    .expect("migrations must apply on a fresh database");
                return (pool, url, container);
            }
            Err(err) => {
                last_err = Some(err);
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    panic!("could not connect to postgres after warmup (attempts={attempts}): {last_err:?}");
}
