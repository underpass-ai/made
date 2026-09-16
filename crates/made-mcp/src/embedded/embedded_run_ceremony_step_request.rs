use made_app::usecases::RunCeremonyStepInput;
use made_core::value_objects::{
    AuditActorKind, CeremonyId, DurationMs, IdempotencyKey, LeaseOwnerId, StepId,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{
    load_instance_definition, optional_string, optional_u64, required_actor_kind, required_string,
};

use crate::embedded::EMBEDDED_BACKEND_NAME;
use crate::protocol::{
    default_idempotency_key, default_lease_owner_id, ToolError, RUN_CEREMONY_STEP_LEASE_TTL_MS,
};

/// Validated MCP request that executes one step on a persistent instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedRunCeremonyStepRequest {
    ceremony_id: CeremonyId,
    step_id: StepId,
    actor_kind: AuditActorKind,
    lease_owner_id: LeaseOwnerId,
    idempotency_key: IdempotencyKey,
    lease_ttl: DurationMs,
}

impl EmbeddedRunCeremonyStepRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<CeremonyId, ToolError> {
        let (definition, _instance) = load_instance_definition(made, &self.ceremony_id).await?;
        let role_id = definition.role_id_for_step(&self.step_id)?;

        made.run_step(RunCeremonyStepInput::new(
            self.ceremony_id.clone(),
            role_id,
            self.actor_kind,
            self.step_id,
            self.lease_owner_id,
            self.idempotency_key,
            self.lease_ttl,
        ))
        .await?;
        Ok(self.ceremony_id)
    }
}

impl TryFrom<&Value> for EmbeddedRunCeremonyStepRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let lease_owner_id = optional_string(object, "lease_owner_id")?
            .unwrap_or_else(|| default_lease_owner_id(EMBEDDED_BACKEND_NAME));
        let idempotency_key =
            optional_string(object, "idempotency_key")?.unwrap_or_else(default_idempotency_key);
        let lease_ttl_ms = optional_u64(object, "lease_ttl_ms")?.unwrap_or_default();

        Ok(Self {
            ceremony_id: CeremonyId::new(required_string(object, "ceremony_id")?)
                .map_err(|error| error.to_string())?,
            step_id: StepId::new(required_string(object, "step_id")?)
                .map_err(|error| error.to_string())?,
            actor_kind: required_actor_kind(object, "actor_kind")?,
            lease_owner_id: LeaseOwnerId::new(lease_owner_id).map_err(|error| error.to_string())?,
            idempotency_key: IdempotencyKey::new(idempotency_key)
                .map_err(|error| error.to_string())?,
            lease_ttl: DurationMs::from_millis(if lease_ttl_ms == 0 {
                RUN_CEREMONY_STEP_LEASE_TTL_MS
            } else {
                lease_ttl_ms
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The request the backend is handed already names a runner, so
    /// neither engine falls back to a default of its own.
    #[test]
    fn an_omitted_runner_becomes_the_backends_own_default() {
        let request = EmbeddedRunCeremonyStepRequest::try_from(
            &json!({ "ceremony_id": "c-1", "step_id": "work", "actor_kind": "agent" }),
        )
        .expect("the request should be accepted");
        assert_eq!(request.lease_owner_id.as_str(), "made-mcp:embedded");
    }

    /// Blank is refused rather than defaulted, as the tool's schema
    /// says and as the gRPC arm now does too: a caller who wrote the
    /// field meant to name a runner.
    #[test]
    fn a_blank_runner_is_refused_rather_than_defaulted() {
        let error = EmbeddedRunCeremonyStepRequest::try_from(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "lease_owner_id": "   ",
        }))
        .expect_err("a blank runner is not a runner");
        assert!(error.contains("lease_owner_id"), "{error}");
    }

    /// The key is minted the same way on both arms, because it is
    /// sealed into the journal and a session's evidence must not say
    /// which process the client happened to be pointed at.
    #[test]
    fn an_omitted_execution_key_is_minted_the_same_way_on_both_arms() {
        let request = EmbeddedRunCeremonyStepRequest::try_from(
            &json!({ "ceremony_id": "c-1", "step_id": "work", "actor_kind": "agent" }),
        )
        .expect("the request should be accepted");
        assert!(
            request.idempotency_key.as_str().starts_with("made-mcp:"),
            "{}",
            request.idempotency_key.as_str()
        );
    }

    #[test]
    fn an_execution_key_the_caller_named_is_left_alone() {
        let request = EmbeddedRunCeremonyStepRequest::try_from(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "idempotency_key": "retry-42",
        }))
        .expect("the request should be accepted");
        assert_eq!(request.idempotency_key.as_str(), "retry-42");
    }

    /// The lease length is the same on both arms, because both read
    /// the number the engine declares rather than choosing one.
    #[test]
    fn an_omitted_lease_length_is_the_one_both_arms_apply() {
        let request = EmbeddedRunCeremonyStepRequest::try_from(
            &json!({ "ceremony_id": "c-1", "step_id": "work", "actor_kind": "agent" }),
        )
        .expect("the request should be accepted");
        assert_eq!(request.lease_ttl.get(), RUN_CEREMONY_STEP_LEASE_TTL_MS);
    }

    #[test]
    fn a_runner_the_caller_named_is_left_alone() {
        let request = EmbeddedRunCeremonyStepRequest::try_from(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "lease_owner_id": "the-hosts-own-runner",
        }))
        .expect("the request should be accepted");
        assert_eq!(request.lease_owner_id.as_str(), "the-hosts-own-runner");
    }
}
