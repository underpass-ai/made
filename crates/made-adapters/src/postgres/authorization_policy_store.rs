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
use sqlx::{Postgres, Row, Transaction};

use super::ceremony_store::{decode, encode, i64_to_u64, sqlx_error, u64_to_i64};
use super::{PostgresPool, PostgresStoredAuthorizationPolicyState};

#[derive(Debug, Clone)]
pub struct PostgresAuthorizationPolicyStore {
    pool: PostgresPool,
}

impl PostgresAuthorizationPolicyStore {
    #[must_use]
    pub const fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthorizationPolicyStorePort for PostgresAuthorizationPolicyStore {
    async fn load(
        &self,
        policy_id: &AuthorizationPolicyId,
    ) -> Result<Option<AuthorizationPolicySnapshot>, DomainError> {
        if let Some((version, state)) = load_state(self.pool.inner(), policy_id).await? {
            return Ok(Some(AuthorizationPolicySnapshot {
                version,
                policy: projected_policy_for(policy_id, &state.events, version)?,
            }));
        }
        let events = load_events(self.pool.inner(), policy_id).await?;
        snapshot_from_events(&events)
    }

    async fn append(
        &self,
        policy_id: &AuthorizationPolicyId,
        expected: AuthorizationPolicyVersion,
        events: Vec<AuthorizationPolicyEvent>,
    ) -> Result<AuthorizationPolicyAppendOutcome, DomainError> {
        validate_events(policy_id, &events)?;
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin authorization policy append"))?;
        ensure_state_row(&mut transaction, policy_id).await?;
        let (actual, mut state) = lock_state(&mut transaction, policy_id).await?;
        if actual != expected {
            return Ok(AuthorizationPolicyAppendOutcome::Conflict { expected, actual });
        }
        let mut policy = projected_policy_for(policy_id, &state.events, actual)?;
        let mut version = actual;
        for event in events {
            let approval = approval_for_event(&mut transaction, policy_id, &event).await?;
            policy.apply_projected(event.clone(), approval.as_ref())?;
            version = version.next();
            insert_event(&mut transaction, policy_id, version, &event).await?;
            project_decision(&mut transaction, policy_id, &event).await?;
            if !matches!(&event, AuthorizationPolicyEvent::DecisionRecorded { .. }) {
                state.events.push(event);
            }
        }
        save_state(&mut transaction, policy_id, version, &state).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit authorization policy append"))?;
        Ok(AuthorizationPolicyAppendOutcome::Appended { version })
    }

    async fn decisions(
        &self,
        policy_id: &AuthorizationPolicyId,
        after: Option<&AuthorizationDecisionId>,
        limit: AuthorizationDecisionPageLimit,
    ) -> Result<AuthorizationDecisionPage, DomainError> {
        let rows = sqlx::query(
            "SELECT decision_id, request_id, payload FROM authorization_decisions \
             WHERE policy_id = $1 AND decision_id > $2 ORDER BY decision_id LIMIT $3",
        )
        .bind(policy_id.as_str())
        .bind(after.map_or("", AuthorizationDecisionId::as_str))
        .bind(i64::try_from(limit.value()).unwrap_or(i64::MAX))
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "list authorization decisions"))?;
        let decisions = rows
            .into_iter()
            .map(|row| decode_decision_row(&row))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AuthorizationDecisionPage::new(decisions))
    }

    async fn decision(
        &self,
        policy_id: &AuthorizationPolicyId,
        decision_id: &AuthorizationDecisionId,
    ) -> Result<Option<AuthorizationDecision>, DomainError> {
        read_decision(
            self.pool.inner(),
            policy_id,
            "SELECT decision_id, request_id, payload FROM authorization_decisions \
             WHERE policy_id = $1 AND decision_id = $2",
            decision_id.as_str(),
        )
        .await
    }

    async fn decision_for_request(
        &self,
        policy_id: &AuthorizationPolicyId,
        request_id: &AuthorizationRequestId,
    ) -> Result<Option<AuthorizationDecision>, DomainError> {
        read_decision(
            self.pool.inner(),
            policy_id,
            "SELECT decision_id, request_id, payload FROM authorization_decisions \
             WHERE policy_id = $1 AND request_id = $2",
            request_id.as_str(),
        )
        .await
    }
}

async fn ensure_state_row(
    transaction: &mut Transaction<'_, Postgres>,
    policy_id: &AuthorizationPolicyId,
) -> Result<(), DomainError> {
    let empty = PostgresStoredAuthorizationPolicyState::default();
    sqlx::query(
        "INSERT INTO authorization_policy_state(policy_id, version, payload) \
         VALUES ($1, 0, $2) ON CONFLICT (policy_id) DO NOTHING",
    )
    .bind(policy_id.as_str())
    .bind(encode(&empty, "encode empty authorization policy state")?)
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "create authorization policy state"))?;
    Ok(())
}

async fn lock_state(
    transaction: &mut Transaction<'_, Postgres>,
    policy_id: &AuthorizationPolicyId,
) -> Result<
    (
        AuthorizationPolicyVersion,
        PostgresStoredAuthorizationPolicyState,
    ),
    DomainError,
> {
    let row = sqlx::query(
        "SELECT version, payload FROM authorization_policy_state \
         WHERE policy_id = $1 FOR UPDATE",
    )
    .bind(policy_id.as_str())
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "lock authorization policy state"))?;
    decode_state_row(&row)
}

async fn load_state(
    pool: &sqlx::PgPool,
    policy_id: &AuthorizationPolicyId,
) -> Result<
    Option<(
        AuthorizationPolicyVersion,
        PostgresStoredAuthorizationPolicyState,
    )>,
    DomainError,
> {
    let row =
        sqlx::query("SELECT version, payload FROM authorization_policy_state WHERE policy_id = $1")
            .bind(policy_id.as_str())
            .fetch_optional(pool)
            .await
            .map_err(|error| sqlx_error(error, "load authorization policy state"))?;
    row.map(|row| decode_state_row(&row)).transpose()
}

fn decode_state_row(
    row: &sqlx::postgres::PgRow,
) -> Result<
    (
        AuthorizationPolicyVersion,
        PostgresStoredAuthorizationPolicyState,
    ),
    DomainError,
> {
    let version: i64 = row
        .try_get("version")
        .map_err(|error| sqlx_error(error, "decode authorization policy version"))?;
    let payload: Vec<u8> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "decode authorization policy state payload"))?;
    Ok((
        AuthorizationPolicyVersion::new(i64_to_u64(version)?),
        decode(&payload, "decode authorization policy state")?,
    ))
}

async fn load_events(
    pool: &sqlx::PgPool,
    policy_id: &AuthorizationPolicyId,
) -> Result<Vec<AuthorizationPolicyEvent>, DomainError> {
    let rows = sqlx::query(
        "SELECT version, payload FROM authorization_policy_events \
         WHERE policy_id = $1 ORDER BY version",
    )
    .bind(policy_id.as_str())
    .fetch_all(pool)
    .await
    .map_err(|error| sqlx_error(error, "load authorization policy events"))?;
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let stored_version: i64 = row
                .try_get("version")
                .map_err(|error| sqlx_error(error, "decode authorization event version"))?;
            if i64_to_u64(stored_version)? != index as u64 + 1 {
                return Err(DomainError::InvariantViolated {
                    reason: "postgres: authorization policy journal has a non-contiguous version",
                });
            }
            let payload: Vec<u8> = row
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode authorization event payload"))?;
            let event: AuthorizationPolicyEvent =
                decode(&payload, "decode authorization policy event")?;
            if event.policy_id() != policy_id {
                return Err(DomainError::InvariantViolated {
                    reason: "postgres: authorization event key does not match its payload",
                });
            }
            Ok(event)
        })
        .collect()
}

fn snapshot_from_events(
    events: &[AuthorizationPolicyEvent],
) -> Result<Option<AuthorizationPolicySnapshot>, DomainError> {
    if events.is_empty() {
        return Ok(None);
    }
    let policy = AuthorizationPolicy::rehydrate(events)?;
    Ok(Some(AuthorizationPolicySnapshot {
        version: policy.version(),
        policy,
    }))
}

fn projected_policy(
    events: &[AuthorizationPolicyEvent],
    version: AuthorizationPolicyVersion,
) -> Result<AuthorizationPolicy, DomainError> {
    let mut policy = AuthorizationPolicy::rehydrate(events)?;
    policy.restore_version(version)?;
    Ok(policy)
}

fn projected_policy_for(
    policy_id: &AuthorizationPolicyId,
    events: &[AuthorizationPolicyEvent],
    version: AuthorizationPolicyVersion,
) -> Result<AuthorizationPolicy, DomainError> {
    let policy = projected_policy(events, version)?;
    if policy.id().is_some_and(|stored| stored != policy_id) {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: authorization policy state key does not match its payload",
        });
    }
    Ok(policy)
}

async fn approval_for_event(
    transaction: &mut Transaction<'_, Postgres>,
    policy_id: &AuthorizationPolicyId,
    event: &AuthorizationPolicyEvent,
) -> Result<Option<AuthorizationDecision>, DomainError> {
    let AuthorizationPolicyEvent::DecisionRecorded { decision, .. } = event else {
        return Ok(None);
    };
    let Some(approval_id) = decision.request().approval_decision_id() else {
        return Ok(None);
    };
    let row = sqlx::query(
        "SELECT decision_id, request_id, payload FROM authorization_decisions \
         WHERE policy_id = $1 AND decision_id = $2",
    )
    .bind(policy_id.as_str())
    .bind(approval_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "load authorization approval decision"))?;
    row.map(|row| decode_decision_row(&row)).transpose()
}

async fn insert_event(
    transaction: &mut Transaction<'_, Postgres>,
    policy_id: &AuthorizationPolicyId,
    version: AuthorizationPolicyVersion,
    event: &AuthorizationPolicyEvent,
) -> Result<(), DomainError> {
    sqlx::query(
        "INSERT INTO authorization_policy_events(policy_id, version, payload) VALUES ($1, $2, $3)",
    )
    .bind(policy_id.as_str())
    .bind(u64_to_i64(version.value())?)
    .bind(encode(event, "encode authorization policy event")?)
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "insert authorization policy event"))?;
    Ok(())
}

async fn project_decision(
    transaction: &mut Transaction<'_, Postgres>,
    policy_id: &AuthorizationPolicyId,
    event: &AuthorizationPolicyEvent,
) -> Result<(), DomainError> {
    let AuthorizationPolicyEvent::DecisionRecorded { decision, .. } = event else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO authorization_decisions(policy_id, decision_id, request_id, payload) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(policy_id.as_str())
    .bind(decision.id().as_str())
    .bind(decision.request().id().as_str())
    .bind(encode(decision, "encode authorization decision")?)
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "project authorization decision"))?;
    Ok(())
}

async fn save_state(
    transaction: &mut Transaction<'_, Postgres>,
    policy_id: &AuthorizationPolicyId,
    version: AuthorizationPolicyVersion,
    state: &PostgresStoredAuthorizationPolicyState,
) -> Result<(), DomainError> {
    sqlx::query(
        "UPDATE authorization_policy_state SET version = $2, payload = $3 WHERE policy_id = $1",
    )
    .bind(policy_id.as_str())
    .bind(u64_to_i64(version.value())?)
    .bind(encode(state, "encode authorization policy state")?)
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "save authorization policy state"))?;
    Ok(())
}

async fn read_decision(
    pool: &sqlx::PgPool,
    policy_id: &AuthorizationPolicyId,
    query: &'static str,
    value: &str,
) -> Result<Option<AuthorizationDecision>, DomainError> {
    let row = sqlx::query(query)
        .bind(policy_id.as_str())
        .bind(value)
        .fetch_optional(pool)
        .await
        .map_err(|error| sqlx_error(error, "read authorization decision"))?;
    row.map(|row| decode_decision_row(&row)).transpose()
}

fn decode_decision_row(row: &sqlx::postgres::PgRow) -> Result<AuthorizationDecision, DomainError> {
    let stored_id: String = row
        .try_get("decision_id")
        .map_err(|error| sqlx_error(error, "decode authorization decision id"))?;
    let stored_request: String = row
        .try_get("request_id")
        .map_err(|error| sqlx_error(error, "decode authorization request id"))?;
    let payload: Vec<u8> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "decode authorization decision payload"))?;
    let decision: AuthorizationDecision = decode(&payload, "decode authorization decision")?;
    decision.validate()?;
    if decision.id().as_str() != stored_id || decision.request().id().as_str() != stored_request {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: authorization decision keys contradict its payload",
        });
    }
    Ok(decision)
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
