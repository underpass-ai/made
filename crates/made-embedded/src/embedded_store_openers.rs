//! Opening the durable stores an embedded engine is built over.
//!
//! Beside the facade rather than inside it: these are the concrete
//! decisions about where state lives on disk, and the engine itself
//! only ever sees the ports they produce.

use made_adapters::artifacts::LocalArtifactStore;
use made_adapters::sqlite::SqliteBudgetLedgerStore;
use made_api::ApiError;
use made_app::usecases::{
    ProtocolSearchCursorCodec, ProtocolSearchCursorKey, ProtocolSearchCursorNamespace,
};

pub(crate) fn open_artifact_store(path: &std::path::Path) -> Result<LocalArtifactStore, ApiError> {
    let mut root = path.as_os_str().to_owned();
    root.push(".artifacts");
    LocalArtifactStore::open(std::path::PathBuf::from(root)).map_err(|error| {
        ApiError::Unavailable {
            reason: format!("the durable local artifact store did not open: {error}"),
        }
    })
}

pub(crate) fn protocol_search_cursors_from_env(
) -> Result<Option<ProtocolSearchCursorCodec>, ApiError> {
    const KEY: &str = "MADE_PROTOCOL_SEARCH_CURSOR_HMAC_KEY";
    const STORE: &str = "MADE_PROTOCOL_STORE_ID";
    const POLICY: &str = "MADE_AUTH_POLICY_ID";
    let key = std::env::var(KEY).ok();
    let store = std::env::var(STORE).ok();
    let policy = std::env::var(POLICY).ok();
    if key.is_none() && store.is_none() && policy.is_none() {
        return Ok(None);
    }
    let missing = |name| ApiError::Unavailable {
        reason: format!("{name} is required for scoped, restart-stable protocol search cursors"),
    };
    let key =
        ProtocolSearchCursorKey::from_hex(&key.ok_or_else(|| missing(KEY))?).map_err(|error| {
            ApiError::Unavailable {
                reason: format!("{KEY} is invalid: {error}"),
            }
        })?;
    let namespace = ProtocolSearchCursorNamespace::new(
        store.ok_or_else(|| missing(STORE))?,
        policy.ok_or_else(|| missing(POLICY))?,
    )
    .map_err(|error| ApiError::Unavailable {
        reason: format!("protocol search cursor namespace is invalid: {error}"),
    })?;
    Ok(Some(ProtocolSearchCursorCodec::new(key, namespace)))
}

pub(crate) fn open_budget_store(
    path: &std::path::Path,
) -> Result<SqliteBudgetLedgerStore, ApiError> {
    SqliteBudgetLedgerStore::open(path).map_err(|error| ApiError::Unavailable {
        reason: format!("the durable SQLite budget ledger did not open: {error}"),
    })
}
