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
    AuthorizationPolicyId, AuthorizationPolicyVersion, AuthorizationRequestId,
};
use made_core::DomainError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use super::StoredAuthorizationPolicyState;

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
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; CREATE TABLE IF NOT EXISTS authorization_policy_events (policy_id TEXT NOT NULL, version INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(policy_id, version)); CREATE TABLE IF NOT EXISTS authorization_decisions (policy_id TEXT NOT NULL, decision_id TEXT NOT NULL, request_id TEXT NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(policy_id, decision_id), UNIQUE(policy_id, request_id)); CREATE TABLE IF NOT EXISTS authorization_policy_state (policy_id TEXT PRIMARY KEY, version INTEGER NOT NULL, payload BLOB NOT NULL);").map_err(|error| sqlite_error(&error))?;
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

    async fn decision(
        &self,
        policy_id: &AuthorizationPolicyId,
        decision_id: &AuthorizationDecisionId,
    ) -> Result<Option<AuthorizationDecision>, DomainError> {
        let policy_id = policy_id.clone();
        let decision_id = decision_id.as_str().to_owned();
        self.blocking("decision", move |store| {
            read_decision(
                &store.connection()?,
                &policy_id,
                "decision_id",
                &decision_id,
            )
        })
        .await
    }

    async fn decision_for_request(
        &self,
        policy_id: &AuthorizationPolicyId,
        request_id: &AuthorizationRequestId,
    ) -> Result<Option<AuthorizationDecision>, DomainError> {
        let policy_id = policy_id.clone();
        let request_id = request_id.as_str().to_owned();
        self.blocking("decision_for_request", move |store| {
            read_decision(&store.connection()?, &policy_id, "request_id", &request_id)
        })
        .await
    }
}

fn load_snapshot(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
) -> Result<Option<AuthorizationPolicySnapshot>, DomainError> {
    if let Some((version, state)) = load_state(connection, policy_id)? {
        return Ok(Some(AuthorizationPolicySnapshot {
            version,
            policy: projected_policy(&state.events, version)?,
        }));
    }
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
            "SELECT version, payload FROM authorization_policy_events WHERE policy_id = ?1 ORDER BY version",
        )
        .map_err(|error| sqlite_error(&error))?;
    let rows = statement
        .query_map([policy_id.as_str()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|error| sqlite_error(&error))?;
    let mut events = Vec::new();
    for (index, row) in rows.enumerate() {
        let (stored_version, payload) = row.map_err(|error| sqlite_error(&error))?;
        let expected_version =
            i64::try_from(index + 1).map_err(|_| DomainError::InvariantViolated {
                reason: "authorization policy version exceeds i64",
            })?;
        if stored_version != expected_version {
            return Err(DomainError::InvariantViolated {
                reason: "authorization policy journal has a non-contiguous version",
            });
        }
        let event: AuthorizationPolicyEvent = decode(&payload)?;
        if event.policy_id() != policy_id {
            return Err(DomainError::InvariantViolated {
                reason: "authorization event payload belongs to another policy",
            });
        }
        events.push(event);
    }
    Ok(events)
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
    let (mut policy, mut state, actual) = load_projection(&transaction, policy_id)?;
    if actual != expected {
        return Ok(AuthorizationPolicyAppendOutcome::Conflict { expected, actual });
    }
    let mut version = expected;
    for event in events {
        let approval = match &event {
            AuthorizationPolicyEvent::DecisionRecorded { decision, .. } => decision
                .request()
                .approval_decision_id()
                .map(|id| read_decision(&transaction, policy_id, "decision_id", id.as_str()))
                .transpose()?
                .flatten(),
            _ => None,
        };
        let accepted = match &event {
            AuthorizationPolicyEvent::DecisionRecorded { decision, .. } => decision
                .request()
                .accepted_work_decision_id()
                .map(|id| read_decision(&transaction, policy_id, "decision_id", id.as_str()))
                .transpose()?
                .flatten(),
            _ => None,
        };
        policy.apply_projected_with_authorities(
            event.clone(),
            approval.as_ref(),
            accepted.as_ref(),
        )?;
        version = version.next();
        transaction
            .execute(
                "INSERT INTO authorization_policy_events(policy_id, version, payload) VALUES (?1, ?2, ?3)",
                params![policy_id.as_str(), to_i64(version.value())?, encode(&event)?],
            )
            .map_err(|error| sqlite_error(&error))?;
        let request_recorded = project_decision(&transaction, policy_id, &event)?;
        if request_recorded {
            // Another host recorded a decision for this request between this
            // store's read and its append. The CAS passed because the events
            // differ, but the per-request invariant does not hold: roll back
            // and report the conflict so the caller re-reads by request_id and
            // returns the recorded decision instead of minting a second one.
            // The unique index remains as a safety net this path proves.
            return Ok(AuthorizationPolicyAppendOutcome::Conflict { expected, actual });
        }
        if !matches!(&event, AuthorizationPolicyEvent::DecisionRecorded { .. }) {
            state.events.push(event);
        }
    }
    save_state(&transaction, policy_id, version, &state)?;
    transaction.commit().map_err(|error| sqlite_error(&error))?;
    Ok(AuthorizationPolicyAppendOutcome::Appended { version })
}

fn load_projection(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
) -> Result<
    (
        AuthorizationPolicy,
        StoredAuthorizationPolicyState,
        AuthorizationPolicyVersion,
    ),
    DomainError,
> {
    if let Some((version, state)) = load_state(connection, policy_id)? {
        let policy = projected_policy(&state.events, version)?;
        return Ok((policy, state, version));
    }
    let events = load_events(connection, policy_id)?;
    if events.is_empty() {
        return Ok((
            AuthorizationPolicy::empty(),
            StoredAuthorizationPolicyState::default(),
            AuthorizationPolicyVersion::default(),
        ));
    }
    let policy = AuthorizationPolicy::rehydrate(&events)?;
    let version = policy.version();
    let state = StoredAuthorizationPolicyState {
        events: events
            .into_iter()
            .filter(|event| !matches!(event, AuthorizationPolicyEvent::DecisionRecorded { .. }))
            .collect(),
    };
    Ok((projected_policy(&state.events, version)?, state, version))
}

fn projected_policy(
    events: &[AuthorizationPolicyEvent],
    version: AuthorizationPolicyVersion,
) -> Result<AuthorizationPolicy, DomainError> {
    let mut policy = AuthorizationPolicy::rehydrate(events)?;
    policy.restore_version(version)?;
    Ok(policy)
}

fn load_state(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
) -> Result<Option<(AuthorizationPolicyVersion, StoredAuthorizationPolicyState)>, DomainError> {
    let stored = connection
        .query_row(
            "SELECT version, payload FROM authorization_policy_state WHERE policy_id = ?1",
            [policy_id.as_str()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .optional()
        .map_err(|error| sqlite_error(&error))?;
    stored
        .map(|(version, payload)| {
            let version = u64::try_from(version).map_err(|_| DomainError::InvariantViolated {
                reason: "authorization projection has a negative version",
            })?;
            Ok((AuthorizationPolicyVersion::new(version), decode(&payload)?))
        })
        .transpose()
}

fn save_state(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
    version: AuthorizationPolicyVersion,
    state: &StoredAuthorizationPolicyState,
) -> Result<(), DomainError> {
    connection
        .execute(
            "INSERT INTO authorization_policy_state(policy_id, version, payload) VALUES (?1, ?2, ?3) ON CONFLICT(policy_id) DO UPDATE SET version = excluded.version, payload = excluded.payload",
            params![policy_id.as_str(), to_i64(version.value())?, encode(state)?],
        )
        .map_err(|error| sqlite_error(&error))?;
    Ok(())
}

fn project_decision(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
    event: &AuthorizationPolicyEvent,
) -> Result<bool, DomainError> {
    let AuthorizationPolicyEvent::DecisionRecorded { decision, .. } = event else {
        return Ok(false);
    };
    // The unique index on (policy_id, request_id) is the durable form of the
    // per-request invariant. A constraint failure here is an expected outcome
    // of two hosts appending concurrently, not a storage fault, so it is
    // reported to the caller (which turns it into an append conflict) instead
    // of crashed on.
    match connection.execute(
        "INSERT INTO authorization_decisions(policy_id, decision_id, request_id, payload) VALUES (?1, ?2, ?3, ?4)",
        params![policy_id.as_str(), decision.id().as_str(), decision.request().id().as_str(), encode(decision)?],
    ) {
        Ok(_) => Ok(false),
        Err(error)
            if error.sqlite_error().is_some_and(|value| {
                value.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
            }) =>
        {
            Ok(true)
        }
        Err(error) => Err(sqlite_error(&error)),
    }
}

fn read_decision(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
    key: &str,
    value: &str,
) -> Result<Option<AuthorizationDecision>, DomainError> {
    let query = match key {
        "decision_id" => "SELECT decision_id, request_id, payload FROM authorization_decisions WHERE policy_id = ?1 AND decision_id = ?2",
        "request_id" => "SELECT decision_id, request_id, payload FROM authorization_decisions WHERE policy_id = ?1 AND request_id = ?2",
        _ => {
            return Err(DomainError::InvariantViolated {
                reason: "unknown authorization decision lookup",
            })
        }
    };
    let mut statement = connection
        .prepare(query)
        .map_err(|error| sqlite_error(&error))?;
    let mut rows = statement
        .query(params![policy_id.as_str(), value])
        .map_err(|error| sqlite_error(&error))?;
    let Some(row) = rows.next().map_err(|error| sqlite_error(&error))? else {
        return Ok(None);
    };
    let stored_id = row
        .get::<_, String>(0)
        .map_err(|error| sqlite_error(&error))?;
    let stored_request = row
        .get::<_, String>(1)
        .map_err(|error| sqlite_error(&error))?;
    let payload = row
        .get::<_, Vec<u8>>(2)
        .map_err(|error| sqlite_error(&error))?;
    let decision: AuthorizationDecision = decode(&payload)?;
    decision.validate()?;
    if decision.id().as_str() != stored_id || decision.request().id().as_str() != stored_request {
        return Err(DomainError::InvariantViolated {
            reason: "authorization decision projection keys contradict its payload",
        });
    }
    Ok(Some(decision))
}

fn read_decisions(
    connection: &Connection,
    policy_id: &AuthorizationPolicyId,
    after: &str,
    limit: AuthorizationDecisionPageLimit,
) -> Result<AuthorizationDecisionPage, DomainError> {
    let mut statement = connection.prepare("SELECT decision_id, payload FROM authorization_decisions WHERE policy_id = ?1 AND decision_id > ?2 ORDER BY decision_id LIMIT ?3").map_err(|error| sqlite_error(&error))?;
    let rows = statement
        .query_map(
            params![policy_id.as_str(), after, to_i64(limit.value() as u64)?],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .map_err(|error| sqlite_error(&error))?;
    let decisions = rows
        .map(|row| {
            let (stored_id, payload) = row.map_err(|error| sqlite_error(&error))?;
            let decision: AuthorizationDecision = decode(&payload)?;
            decision.validate()?;
            if decision.id().as_str() != stored_id {
                return Err(DomainError::InvariantViolated {
                    reason: "authorization decision projection id contradicts its payload",
                });
            }
            Ok(decision)
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    tracing::error!(%error, sqlite_extended_code = error.sqlite_error().map(|value| value.extended_code), "authorization SQLite operation failed");
    DomainError::InvalidDocument {
        reason: format!("authorization SQLite operation failed: {error}"),
    }
}

fn encoding_error(error: &serde_json::Error) -> DomainError {
    tracing::error!(%error, "authorization SQLite encoding failed");
    DomainError::InvariantViolated {
        reason: "authorization SQLite encoding failed",
    }
}
