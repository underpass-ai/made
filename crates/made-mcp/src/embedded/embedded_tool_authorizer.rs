use made_app::authorization::{
    AuthorizationGateOutcome, ContinueAcceptedStepClaimUseCase, ReadAuthorizationPolicyUseCase,
    TrustedHostAuthorizationGate,
};
use made_core::ports::{
    ArtifactStorePort, ArtifactUploadId, AuthorizationScopeResolverPort, ExecutionReceiptStorePort,
};
use made_core::value_objects::{
    ArtifactId, AuthorizationAction, AuthorizationRequestId, AuthorizationScope, BudgetAccountId,
    CeremonyId, CeremonyName, CeremonyVersion, CouncilId, ExecutionOperationId,
};
use serde_json::Value;

use super::embedded_complete_ceremony_step_request::EmbeddedCompleteCeremonyStepRequest;
use crate::backend::ToolTraceContext;
use crate::protocol::{ToolError, SEARCH_CEREMONY_INSTANCES_TOOL};

use super::embedded_authorization_request;
use super::embedded_ceremony_search_request::EmbeddedCeremonySearchRequest;

#[derive(Clone)]
pub(super) struct EmbeddedToolAuthorizer {
    gate: TrustedHostAuthorizationGate,
    read_policy: ReadAuthorizationPolicyUseCase,
    step_continuation: std::sync::Arc<ContinueAcceptedStepClaimUseCase>,
    artifacts: std::sync::Arc<dyn ArtifactStorePort>,
    execution_receipts: std::sync::Arc<dyn ExecutionReceiptStorePort>,
    scopes: std::sync::Arc<dyn AuthorizationScopeResolverPort>,
}

impl std::fmt::Debug for EmbeddedToolAuthorizer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmbeddedToolAuthorizer")
            .field("gate", &self.gate)
            .finish_non_exhaustive()
    }
}

impl EmbeddedToolAuthorizer {
    pub(super) const fn new(
        gate: TrustedHostAuthorizationGate,
        read_policy: ReadAuthorizationPolicyUseCase,
        step_continuation: std::sync::Arc<ContinueAcceptedStepClaimUseCase>,
        artifacts: std::sync::Arc<dyn ArtifactStorePort>,
        execution_receipts: std::sync::Arc<dyn ExecutionReceiptStorePort>,
        scopes: std::sync::Arc<dyn AuthorizationScopeResolverPort>,
    ) -> Self {
        Self {
            gate,
            read_policy,
            step_continuation,
            artifacts,
            execution_receipts,
            scopes,
        }
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
        tool_name: &str,
        arguments: &Value,
        trace: &ToolTraceContext,
    ) -> Result<made_core::value_objects::AuthorizedOperation, ToolError> {
        let action = action_for_tool(tool_name)?;
        let scope = self.scope_for_tool(tool_name, action, arguments).await?;
        let request_id = AuthorizationRequestId::new(trace.authorization_request_id())?;
        let target_digest = if tool_name == SEARCH_CEREMONY_INSTANCES_TOOL {
            EmbeddedCeremonySearchRequest::try_from(arguments)
                .map_err(ToolError::invalid_request)?
                .authorization_target_digest()
        } else {
            ToolTraceContext::authorization_target_digest(tool_name, arguments)
        };
        let outcome = self
            .gate
            .authorize_outcome(
                request_id.clone(),
                action,
                scope,
                target_digest.clone(),
                trace
                    .approval_decision_id()
                    .map(made_core::value_objects::AuthorizationDecisionId::new)
                    .transpose()?,
            )
            .await?;
        match outcome {
            AuthorizationGateOutcome::Allowed { evidence, .. } => {
                made_core::value_objects::AuthorizedOperation::new(
                    self.gate.principal().clone(),
                    evidence,
                )
                .map_err(Into::into)
            }
            AuthorizationGateOutcome::Denied { .. } | AuthorizationGateOutcome::Expired { .. }
                if action == AuthorizationAction::CompleteCeremonyStep =>
            {
                let request = EmbeddedCompleteCeremonyStepRequest::try_from(arguments)
                    .map_err(ToolError::invalid_request)?;
                self.step_continuation
                    .execute(request.accepted_completion(
                        self.gate.principal().clone(),
                        &request_id,
                        target_digest,
                    )?)
                    .await
                    .map_err(Into::into)
            }
            AuthorizationGateOutcome::Denied { decision } => Err(ToolError::refused(format!(
                "authorization decision {} denied the operation",
                decision.id().as_str()
            ))),
            AuthorizationGateOutcome::Expired { decision } => Err(ToolError::refused(format!(
                "authorization decision {} has expired",
                decision.id().as_str()
            ))),
        }
    }

    pub(super) async fn approve_operation(
        &self,
        arguments: &Value,
        trace: &ToolTraceContext,
    ) -> Result<made_core::value_objects::AuthorizationDecision, ToolError> {
        let (approval_action, execution_action, scope, target_digest) =
            embedded_authorization_request::approval(arguments)?;
        match self
            .gate
            .approve_operation_outcome(
                AuthorizationRequestId::new(trace.authorization_request_id())?,
                approval_action,
                execution_action,
                scope,
                target_digest,
            )
            .await?
        {
            AuthorizationGateOutcome::Allowed { decision, .. } => Ok(decision),
            AuthorizationGateOutcome::Denied { decision } => Err(ToolError::refused(format!(
                "authorization decision {} denied the approval",
                decision.id().as_str()
            ))),
            AuthorizationGateOutcome::Expired { decision } => Err(ToolError::refused(format!(
                "authorization decision {} expired before approval",
                decision.id().as_str()
            ))),
        }
    }
    async fn scope_for_tool(
        &self,
        tool_name: &str,
        action: AuthorizationAction,
        arguments: &Value,
    ) -> Result<AuthorizationScope, ToolError> {
        scope_for_tool(
            &self.read_policy,
            self.artifacts.as_ref(),
            self.execution_receipts.as_ref(),
            self.scopes.as_ref(),
            tool_name,
            action,
            arguments,
        )
        .await
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
    read_policy: &ReadAuthorizationPolicyUseCase,
    artifacts: &dyn ArtifactStorePort,
    execution_receipts: &dyn ExecutionReceiptStorePort,
    scopes: &dyn AuthorizationScopeResolverPort,
    tool_name: &str,
    action: AuthorizationAction,
    arguments: &Value,
) -> Result<AuthorizationScope, ToolError> {
    let object = arguments
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("tools/call.arguments must be an object"))?;

    if tool_name == "made_issue_authorization_grant" {
        return mcp_scope(
            object
                .get("scope")
                .ok_or_else(|| ToolError::invalid_request("field `scope` is required"))?,
        );
    }
    if tool_name == "made_revoke_authorization_grant" {
        let grant_id = string_field(object, "grant_id")?
            .ok_or_else(|| ToolError::invalid_request("field `grant_id` is required"))?;
        let grant_id = made_core::value_objects::AuthorizationGrantId::new(grant_id)?;
        let snapshot = read_policy.execute().await?;
        return Ok(snapshot
            .policy
            .grants()
            .find(|grant| grant.id() == &grant_id)
            .map_or(AuthorizationScope::Global, |grant| grant.scope().clone()));
    }

    if tool_name == "made_get_budget_report" {
        let raw = string_field(object, "ceremony_id")?
            .ok_or_else(|| ToolError::invalid_request("field `ceremony_id` is required"))?;
        return scopes
            .budget_scope(&CeremonyId::new(raw)?)
            .await
            .map_err(Into::into);
    }

    if tool_name == "made_accept_child_completion" {
        let raw = string_field(object, "child_id")?
            .ok_or_else(|| ToolError::invalid_request("field `child_id` is required"))?;
        return scopes
            .child_parent_scope(&CeremonyId::new(raw)?)
            .await
            .map_err(Into::into);
    }

    if tool_name == "made_report_ceremony_agent_status" {
        let status = object
            .get("status")
            .and_then(Value::as_object)
            .ok_or_else(|| ToolError::invalid_request("field `status` is required"))?;
        let raw = string_field(status, "ceremony_id")?
            .ok_or_else(|| ToolError::invalid_request("field `status.ceremony_id` is required"))?;
        return scopes
            .ceremony_scope(&CeremonyId::new(raw)?)
            .await
            .map_err(Into::into);
    }

    // The aggregate and its runs are authorized globally in this
    // version, by decision rather than omission (ADR-021). Said here
    // rather than left to the fallback, so a field named `ceremony_id`
    // added to one of these tools later cannot quietly narrow it.
    if is_agentic_system_action(action) {
        return Ok(AuthorizationScope::Global);
    }

    if is_definition_action(action) {
        let (name, version) = definition_identity(object)?;
        return Ok(AuthorizationScope::Definition { name, version });
    }

    if let Some(raw) = string_field(object, "ceremony_id")? {
        return scopes
            .ceremony_scope(&CeremonyId::new(raw)?)
            .await
            .map_err(Into::into);
    }
    if let Some(raw) = string_field(object, "operation_id")? {
        let operation = execution_receipts
            .operation(&ExecutionOperationId::new(raw)?)
            .await?;
        let operation = operation.ok_or(made_core::DomainError::NotFound {
            what: "execution_operation",
        })?;
        return scopes
            .ceremony_scope(operation.ceremony_id())
            .await
            .map_err(Into::into);
    }
    if let Some(raw) =
        string_field(object, "artifact_id")?.or(string_field(object, "requested_artifact_id")?)
    {
        return Ok(AuthorizationScope::Artifact {
            artifact_id: ArtifactId::new(raw)?,
        });
    }
    if let Some(raw) = string_field(object, "upload_id")? {
        let artifact_id = artifacts
            .artifact_id_for_upload(&ArtifactUploadId::new(raw)?)
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

fn is_agentic_system_action(action: AuthorizationAction) -> bool {
    matches!(
        action,
        AuthorizationAction::DesignAgenticSystem
            | AuthorizationAction::GetAgenticSystem
            | AuthorizationAction::ListAgenticSystems
            | AuthorizationAction::ValidateAgenticSystem
            | AuthorizationAction::PublishAgenticSystem
            | AuthorizationAction::InstantiateAgenticSystem
            | AuthorizationAction::AdvanceAgenticSystemExecution
            | AuthorizationAction::GetAgenticSystemExecution
            | AuthorizationAction::RenderAgenticSystemDiagram
    )
}

fn is_definition_action(action: AuthorizationAction) -> bool {
    matches!(
        action,
        AuthorizationAction::GetCeremonyDefinition
            | AuthorizationAction::MountDefinition
            | AuthorizationAction::ValidateCeremonyDraft
            | AuthorizationAction::ExplainCeremonyDraft
            | AuthorizationAction::PublishCeremonyDefinition
    )
}

fn definition_identity(
    object: &serde_json::Map<String, Value>,
) -> Result<(CeremonyName, Option<CeremonyVersion>), ToolError> {
    if let Some(name) = string_field(object, "name")?.or(string_field(object, "definition_name")?) {
        let version = string_field(object, "version")?
            .or(string_field(object, "definition_version")?)
            .map(CeremonyVersion::new)
            .transpose()?;
        return Ok((CeremonyName::new(name)?, version));
    }
    let yaml = string_field(object, "definition_yaml")?
        .ok_or_else(|| ToolError::invalid_request("definition identity is required"))?;
    let document: serde_yaml::Value = serde_yaml::from_str(yaml)
        .map_err(|error| ToolError::invalid_request(format!("invalid definition YAML: {error}")))?;
    let name = document
        .get("name")
        .and_then(serde_yaml::Value::as_str)
        .ok_or_else(|| ToolError::invalid_request("definition YAML name is required"))?;
    let version = document
        .get("version")
        .and_then(serde_yaml::Value::as_str)
        .map(CeremonyVersion::new)
        .transpose()?;
    Ok((CeremonyName::new(name)?, version))
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

fn mcp_scope(value: &Value) -> Result<AuthorizationScope, ToolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("field `scope` must be an object"))?;
    let kind = string_field(object, "kind")?
        .ok_or_else(|| ToolError::invalid_request("scope.kind is required"))?;
    let required = |field| {
        string_field(object, field)?.ok_or_else(|| {
            ToolError::invalid_request(format!("scope.{field} is required for `{kind}`"))
        })
    };
    match kind {
        "global" => Ok(AuthorizationScope::Global),
        "ceremony" => Ok(AuthorizationScope::Ceremony {
            ceremony_id: CeremonyId::new(required("ceremony_id")?)?,
        }),
        "ceremony_tree" => Ok(AuthorizationScope::CeremonyTree {
            root_id: CeremonyId::new(required("root_id")?)?,
        }),
        "definition" => Ok(AuthorizationScope::Definition {
            name: CeremonyName::new(required("name")?)?,
            version: string_field(object, "version")?
                .map(CeremonyVersion::new)
                .transpose()?,
        }),
        "artifact" => Ok(AuthorizationScope::Artifact {
            artifact_id: ArtifactId::new(required("artifact_id")?)?,
        }),
        "council" => Ok(AuthorizationScope::Council {
            council_id: CouncilId::new(required("council_id")?)?,
        }),
        "budget" => Ok(AuthorizationScope::Budget {
            account_id: BudgetAccountId::new(required("account_id")?)?,
        }),
        _ => Err(ToolError::invalid_request(format!(
            "unknown authorization scope `{kind}`"
        ))),
    }
}
