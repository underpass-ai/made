use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use made_core::entities::{AuthorizationPolicy, AuthorizationPolicyEvent};
use made_core::ports::{
    AuthorizationDecisionPage, AuthorizationPolicyAppendOutcome, AuthorizationPolicySnapshot,
    AuthorizationPolicyStorePort,
};
use made_core::value_objects::{
    AuthorizationDecision, AuthorizationDecisionId, AuthorizationDecisionPageLimit,
    AuthorizationPolicyId, AuthorizationPolicyVersion,
};
use made_core::DomainError;
use rusqlite::{params, Connection, TransactionBehavior};

#[derive(Debug, Clone)]
pub struct SqliteAuthorizationPolicyStore {
    path: PathBuf,
}

impl SqliteAuthorizationPolicyStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let store = Self {
            path: path.as_ref().to_path_buf(),
        };
        store.connection()?;
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, DomainError> {
        let connection = Connection::open(&self.path).map_err(|error| sqlite_error(&error))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| sqlite_error(&error))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; CREATE TABLE IF NOT EXISTS authorization_policy_events (policy_id TEXT NOT NULL, version INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(policy_id, version)); CREATE TABLE IF NOT EXISTS authorization_decisions (policy_id TEXT NOT NULL, decision_id TEXT NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(policy_id, decision_id));").map_err(|error| sqlite_error(&error))?;
        Ok(connection)
    }

    async fn blocking<T, F>(&self, operation: &'static str, work: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(Self) -> Result<T, DomainError> + Send + 'static,
    {
        let store = self.clone();
        tokio::task::spawn_blocking(move || work(store))
            .await
            .map_err(|error| {
                tracing::error!(%error, operation, "authorization SQLite blocking task failed");
                DomainError::InvariantViolated {
                    reason: "authorization SQLite blocking task failed",
                }
            })?
    }
}

#[async_trait]
impl AuthorizationPolicyStorePort for SqliteAuthorizationPolicyStore {
    async fn load(
        &self,
        policy_id: &AuthorizationPolicyId,
    ) -> Result<Option<AuthorizationPolicySnapshot>, DomainError> {
        let policy_id = policy_id.clone();
        self.blocking("load", move |store| {
            load_snapshot(&store.connection()?, &policy_id)
        })
        .await
    }

    async fn append(
        &self,
        policy_id: &AuthorizationPolicyId,
        expected: AuthorizationPolicyVersion,
        events: Vec<AuthorizationPolicyEvent>,
    ) -> Result<AuthorizationPolicyAppendOutcome, DomainError> {
        validate_events(policy_id, &events)?;
        let policy_id = policy_id.clone();
        self.blocking("append", move |store| {
            append_events(store.connection()?, &policy_id, expected, events)
        })
        .await
    }

    async fn decisions(
        &self,
        policy_id: &AuthorizationPolicyId,
        after: Option<&AuthorizationDecisionId>,
        limit: AuthorizationDecisionPageLimit,
    ) -> Result<AuthorizationDecisionPage, DomainError> {
        let policy_id = policy_id.clone();
        let after = after.map_or_else(String::new, |value| value.as_str().to_owned());
        self.blocking("decisions", move |store| {
            read_decisions(&store.connection()?, &policy_id, &after, limit)
        })
        .await
    }
}

fn load_snapshot(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
) -> Result<Option<AuthorizationPolicySnapshot>, DomainError> {
    let events = load_events(connection, policy_id)?;
    if events.is_empty() {
        return Ok(None);
    }
    let policy = AuthorizationPolicy::rehydrate(&events)?;
    Ok(Some(AuthorizationPolicySnapshot {
        version: policy.version(),
        policy,
    }))
}

fn load_events(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
) -> Result<Vec<AuthorizationPolicyEvent>, DomainError> {
    let mut statement = connection
        .prepare(
            "SELECT payload FROM authorization_policy_events WHERE policy_id = ?1 ORDER BY version",
        )
        .map_err(|error| sqlite_error(&error))?;
    let rows = statement
        .query_map([policy_id.as_str()], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|error| sqlite_error(&error))?;
    rows.map(|row| decode(&row.map_err(|error| sqlite_error(&error))?))
        .collect()
}

fn append_events(
    mut connection: Connection,
    policy_id: &AuthorizationPolicyId,
    expected: AuthorizationPolicyVersion,
    events: Vec<AuthorizationPolicyEvent>,
) -> Result<AuthorizationPolicyAppendOutcome, DomainError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| sqlite_error(&error))?;
    let stored = load_events(&transaction, policy_id)?;
    let actual = AuthorizationPolicyVersion::new(stored.len() as u64);
    if actual != expected {
        return Ok(AuthorizationPolicyAppendOutcome::Conflict { expected, actual });
    }
    let mut candidate = stored;
    candidate.extend(events.iter().cloned());
    AuthorizationPolicy::rehydrate(&candidate)?;
    let mut version = expected;
    for event in events {
        version = version.next();
        transaction
            .execute(
                "INSERT INTO authorization_policy_events(policy_id, version, payload) VALUES (?1, ?2, ?3)",
                params![policy_id.as_str(), to_i64(version.value())?, encode(&event)?],
            )
            .map_err(|error| sqlite_error(&error))?;
        project_decision(&transaction, policy_id, &event)?;
    }
    transaction.commit().map_err(|error| sqlite_error(&error))?;
    Ok(AuthorizationPolicyAppendOutcome::Appended { version })
}

fn project_decision(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
    event: &AuthorizationPolicyEvent,
) -> Result<(), DomainError> {
    let AuthorizationPolicyEvent::DecisionRecorded { decision, .. } = event else {
        return Ok(());
    };
    connection
        .execute(
            "INSERT INTO authorization_decisions(policy_id, decision_id, payload) VALUES (?1, ?2, ?3)",
            params![policy_id.as_str(), decision.id().as_str(), encode(decision)?],
        )
        .map_err(|error| sqlite_error(&error))?;
    Ok(())
}

fn read_decisions(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
    after: &str,
    limit: AuthorizationDecisionPageLimit,
) -> Result<AuthorizationDecisionPage, DomainError> {
    let mut statement = connection.prepare("SELECT payload FROM authorization_decisions WHERE policy_id = ?1 AND decision_id > ?2 ORDER BY decision_id LIMIT ?3").map_err(|error| sqlite_error(&error))?;
    let rows = statement
        .query_map(
            params![policy_id.as_str(), after, to_i64(limit.value() as u64)?],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .map_err(|error| sqlite_error(&error))?;
    let decisions = rows
        .map(|row| decode(&row.map_err(|error| sqlite_error(&error))?))
        .collect::<Result<Vec<AuthorizationDecision>, _>>()?;
    Ok(AuthorizationDecisionPage::new(decisions))
}

fn validate_events(
    policy_id: &AuthorizationPolicyId,
    events: &[AuthorizationPolicyEvent],
) -> Result<(), DomainError> {
    if events.is_empty() {
        return Err(DomainError::EmptyCollection {
            field: "authorization_policy_events",
        });
    }
    if events.iter().any(|event| event.policy_id() != policy_id) {
        return Err(DomainError::InvariantViolated {
            reason: "authorization append contains an event for another policy",
        });
    }
    Ok(())
}

fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, DomainError> {
    serde_json::to_vec(value).map_err(|error| encoding_error(&error))
}

fn decode<T: for<'de> serde::Deserialize<'de>>(bytes: &[u8]) -> Result<T, DomainError> {
    serde_json::from_slice(bytes).map_err(|error| encoding_error(&error))
}

fn to_i64(value: u64) -> Result<i64, DomainError> {
    i64::try_from(value).map_err(|_| DomainError::InvariantViolated {
        reason: "authorization SQLite integer exceeds i64",
    })
}

fn sqlite_error(error: &rusqlite::Error) -> DomainError {
    tracing::error!(%error, "authorization SQLite operation failed");
    DomainError::InvariantViolated {
        reason: "authorization SQLite operation failed",
    }
}

fn encoding_error(error: &serde_json::Error) -> DomainError {
    tracing::error!(%error, "authorization SQLite encoding failed");
    DomainError::InvariantViolated {
        reason: "authorization SQLite encoding failed",
    }
}
