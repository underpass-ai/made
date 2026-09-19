use std::sync::Arc;

use made_adapters::artifacts::LocalArtifactStore;
use made_adapters::config::ServiceConfig;
use made_adapters::postgres::{PostgresArtifactStore, PostgresPool};
use made_app::artifacts::ArtifactService;
use tracing::info;

use crate::ComposeError;

pub(super) fn wire(
    config: &ServiceConfig,
    postgres: Option<&PostgresPool>,
) -> Result<Option<Arc<ArtifactService>>, ComposeError> {
    if let Some(pool) = postgres {
        info!("shared postgres artifact storage wired");
        return Ok(Some(Arc::new(ArtifactService::new(Arc::new(
            PostgresArtifactStore::new(pool.clone()),
        )))));
    }
    let Some(path) = config.artifact_store_path.as_deref() else {
        info!("artifact storage disabled; set MADE_ARTIFACT_STORE_PATH or MADE_POSTGRES_URL");
        return Ok(None);
    };
    let store = LocalArtifactStore::open(path)?;
    info!(path, "durable local artifact storage wired");
    Ok(Some(Arc::new(ArtifactService::new(Arc::new(store)))))
}
