use made_app::authorization::{ReadAuthorizationPolicyUseCase, TrustedHostAuthorizationGate};
use made_core::value_objects::{
    ArtifactId, AuthorizationAction, AuthorizationRequestId, AuthorizationScope, BudgetAccountId,
    CeremonyId, CouncilId, ExecutionOperationId,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::backend::ToolTraceContext;
use crate::protocol::ToolError;

#[derive(Clone, Debug)]
pub(super) struct EmbeddedToolAuthorizer {
    gate: TrustedHostAuthorizationGate,
    read_policy: ReadAuthorizationPolicyUseCase,
}

impl EmbeddedToolAuthorizer {
    pub(super) const fn new(
        gate: TrustedHostAuthorizationGate,
        read_policy: ReadAuthorizationPolicyUseCase,
    ) -> Self {
        Self { gate, read_policy }
    }

    pub(super) async fn validate_configuration(&self) -> Result<(), String> {
        self.read_policy
            .execute()
            .await
            .map(|_| ())
            .map_err(|error| format!("embedded authorization policy is not available: {error}"))
    }

    pub(super) async fn authorize(
        &self,
        made: &EmbeddedMade,
        tool_name: &str,
        arguments: &Value,
        request_id: &str,
    ) -> Result<made_core::value_objects::AuthorizedOperation, ToolError> {
        let action = action_for_tool(tool_name)?;
        let scope = scope_for_tool(made, tool_name, arguments).await?;
        self.gate
            .authorize(
                AuthorizationRequestId::new(request_id)?,
                action,
                scope,
                ToolTraceContext::authorization_target_digest(tool_name, arguments),
                approval_id(arguments)?,
            )
            .await
            .map_err(Into::into)
    }
}

fn action_for_tool(tool_name: &str) -> Result<AuthorizationAction, ToolError> {
    let action = match tool_name {
        "made_get_budget_report" | "made_list_pending_budget_reservations" => {
            return Ok(AuthorizationAction::ReadBudget);
        }
        "made_get_authorization_policy" => {
            return Ok(AuthorizationAction::ReadAuthorizationPolicy);
        }
        "made_issue_authorization_grant" => {
            return Ok(AuthorizationAction::IssueAuthorizationGrant);
        }
        "made_revoke_authorization_grant" => {
            return Ok(AuthorizationAction::RevokeAuthorizationGrant);
        }
        "made_list_authorization_decisions" => {
            return Ok(AuthorizationAction::ReadAuthorizationDecisions);
        }
        name => name.strip_prefix("made_").ok_or_else(|| {
            ToolError::refused("embedded authorization rejected an unrecognized tool identity")
        })?,
    };
    serde_json::from_value(Value::String(action.to_owned())).map_err(|_| {
        ToolError::refused(format!(
            "embedded authorization has no action mapping for `{tool_name}`"
        ))
    })
}

async fn scope_for_tool(
    made: &EmbeddedMade,
    tool_name: &str,
    arguments: &Value,
) -> Result<AuthorizationScope, ToolError> {
    let object = arguments
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("tools/call.arguments must be an object"))?;

    if let Some(raw) = string_field(object, "ceremony_id")? {
        return resolved_ceremony(made, CeremonyId::new(raw)?).await;
    }
    if let Some(raw) = string_field(object, "operation_id")? {
        let operation = made
            .execution_operation(&ExecutionOperationId::new(raw)?)
            .await?;
        return resolved_ceremony(made, operation.ceremony_id().clone()).await;
    }
    if let Some(raw) =
        string_field(object, "artifact_id")?.or(string_field(object, "requested_artifact_id")?)
    {
        return Ok(AuthorizationScope::Artifact {
            artifact_id: ArtifactId::new(raw)?,
        });
    }
    if let Some(raw) = string_field(object, "upload_id")? {
        let artifact_id = made
            .artifact_id_for_upload(&made_core::ports::ArtifactUploadId::new(raw)?)
            .await?;
        return Ok(AuthorizationScope::Artifact { artifact_id });
    }
    if let Some(raw) = string_field(object, "council_id")? {
        return Ok(AuthorizationScope::Council {
            council_id: CouncilId::new(raw)?,
        });
    }
    if let Some(raw) = string_field(object, "account_id")? {
        return Ok(AuthorizationScope::Budget {
            account_id: BudgetAccountId::new(raw)?,
        });
    }

    // Collection reads, creation before an authoritative resource id exists,
    // and host-level controls require an explicit Global grant.
    let _ = tool_name;
    Ok(AuthorizationScope::Global)
}

async fn resolved_ceremony(
    made: &EmbeddedMade,
    ceremony_id: CeremonyId,
) -> Result<AuthorizationScope, ToolError> {
    match made.instance(&ceremony_id).await {
        Ok(instance) => Ok(AuthorizationScope::ResolvedCeremony {
            root_id: instance
                .lineage()
                .map_or_else(|| ceremony_id.clone(), |lineage| lineage.root_id().clone()),
            ceremony_id,
        }),
        Err(made_core::DomainError::NotFound { .. }) => {
            Ok(AuthorizationScope::Ceremony { ceremony_id })
        }
        Err(error) => Err(error.into()),
    }
}

fn approval_id(
    arguments: &Value,
) -> Result<Option<made_core::value_objects::AuthorizationDecisionId>, ToolError> {
    let Some(meta) = arguments.get("_meta") else {
        return Ok(None);
    };
    let Some(raw) = meta
        .as_object()
        .and_then(|object| object.get("made_approval_decision_id"))
    else {
        return Ok(None);
    };
    let raw = raw.as_str().ok_or_else(|| {
        ToolError::invalid_request("_meta.made_approval_decision_id must be a string")
    })?;
    made_core::value_objects::AuthorizationDecisionId::new(raw)
        .map(Some)
        .map_err(Into::into)
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, ToolError> {
    object
        .get(field)
        .map(|value| {
            value.as_str().ok_or_else(|| {
                ToolError::invalid_request(format!("field `{field}` must be a string"))
            })
        })
        .transpose()
}
