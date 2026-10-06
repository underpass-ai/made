//! `made-mcp approve-guard`: a person's decision, recorded from a
//! terminal the agent's MCP session cannot write to.
//!
//! The command opens the same store and the same authorization policy
//! the MCP server serves, authorizes itself as the same trusted host,
//! and seals the same `HumanApprovalRecorded` fact the MCP tool would.
//! What differs is the channel: it runs only where a person can answer
//! a prompt, shows what is about to be approved, and asks. In
//! `terminal` mode (see [`HumanApprovalSource`](crate::HumanApprovalSource))
//! the MCP tool is refused, so this is the only way a human guard gets
//! its approval.
//!
//! It proves the channel, not the person: a process that can drive a
//! pseudo-terminal can answer the prompt. That limit is declared beside
//! the capability in discovery rather than hidden behind the word
//! "human".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::{AuthorizeOperationUseCase, TrustedHostAuthorizationGate};
use made_app::services::AuthorizationOperationScope;
use made_app::usecases::ApproveCeremonyGuardInput;
use made_core::entities::CeremonyInstance;
use made_core::ports::AuthorizationPolicyStorePort;
use made_core::value_objects::{
    AuditActorKind, AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction,
    AuthorizationDecisionTtl, AuthorizationPolicyId, AuthorizationRequestId, AuthorizationScope,
    AuthorizedOperation, CeremonyId, GuardName, PrincipalId, PrincipalKind, RoleId,
};
use made_embedded::EmbeddedMade;
use serde_json::json;

use crate::backend::ToolTraceContext;
use crate::protocol::APPROVE_CEREMONY_GUARD_TOOL;

mod approve_guard_outcome;
mod stdio_terminal;
mod terminal;

pub use approve_guard_outcome::ApproveGuardOutcome;
pub use stdio_terminal::StdioTerminal;
pub use terminal::Terminal;

/// Why the command refuses piped input. Spelled once so the refusal
/// and the tests agree.
pub const NOT_INTERACTIVE: &str = "approve-guard records a person's decision and runs only at an \
     interactive terminal; its stdin and stdout are not terminals here. It does not read \
     answers from a pipe, an environment variable or a flag, because any of those is a \
     channel an agent can write to.";

/// One approval to put to a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveGuardCommand {
    store: PathBuf,
    policy_id: AuthorizationPolicyId,
    trusted_host_id: PrincipalId,
    ceremony_id: CeremonyId,
    guard_name: GuardName,
    role_id: RoleId,
    reason: Option<String>,
}

impl ApproveGuardCommand {
    pub fn new(
        store: impl Into<PathBuf>,
        policy_id: &str,
        trusted_host_id: &str,
        ceremony_id: &str,
        guard_name: &str,
        role_id: &str,
        reason: Option<String>,
    ) -> Result<Self, String> {
        Ok(Self {
            store: store.into(),
            policy_id: AuthorizationPolicyId::new(policy_id).map_err(|error| error.to_string())?,
            trusted_host_id: PrincipalId::new(trusted_host_id)
                .map_err(|error| error.to_string())?,
            ceremony_id: CeremonyId::new(ceremony_id).map_err(|error| error.to_string())?,
            guard_name: GuardName::new(guard_name).map_err(|error| error.to_string())?,
            role_id: RoleId::new(role_id).map_err(|error| error.to_string())?,
            reason: reason
                .map(|reason| reason.trim().to_owned())
                .filter(|reason| !reason.is_empty()),
        })
    }

    #[must_use]
    pub fn store(&self) -> &Path {
        &self.store
    }

    /// Show the person what they are deciding, ask, and seal the answer.
    ///
    /// # Errors
    ///
    /// Refuses a non-interactive terminal before touching the store;
    /// otherwise returns the store's, the policy's or the engine's
    /// refusal, worded for the person at the terminal.
    pub async fn run(&self, terminal: &mut dyn Terminal) -> Result<ApproveGuardOutcome, String> {
        if !terminal.is_interactive() {
            return Err(NOT_INTERACTIVE.to_owned());
        }
        let (made, gate) = self.compose()?;

        let read = self
            .authorize(
                &gate,
                AuthorizationAction::GetCeremonyInstance,
                json!({"ceremony_id": self.ceremony_id.as_str()}),
            )
            .await?;
        let instance = AuthorizationOperationScope::run(read, made.instance(&self.ceremony_id))
            .await
            .map_err(|error| {
                format!(
                    "could not read ceremony `{}`: {error}",
                    self.ceremony_id.as_str()
                )
            })?;
        self.show(terminal, &instance);

        let prompt = format!(
            "Record your approval of guard `{}` on ceremony `{}` as role `{}` (actor kind: human)?",
            self.guard_name.as_str(),
            self.ceremony_id.as_str(),
            self.role_id.as_str()
        );
        if !terminal.confirm(&prompt)? {
            terminal.show("Nothing was recorded.");
            return Ok(ApproveGuardOutcome::Declined);
        }

        let approve = self
            .authorize(
                &gate,
                AuthorizationAction::ApproveCeremonyGuard,
                self.approval_arguments(),
            )
            .await?;
        let decision_id = approve.evidence().decision_id().clone();
        let input = ApproveCeremonyGuardInput::new(
            self.ceremony_id.clone(),
            self.guard_name.clone(),
            self.role_id.clone(),
            AuditActorKind::Human,
        );
        let instance = Box::pin(AuthorizationOperationScope::run(
            approve,
            made.approve_guard(input),
        ))
        .await
        .map_err(|error| format!("the engine refused the approval: {error}"))?;
        let approval = instance
            .guard_approvals()
            .iter()
            .rev()
            .find(|approval| approval.guard_name() == &self.guard_name)
            .ok_or_else(|| "the approval was accepted but is not in the instance".to_owned())?;
        terminal.show(&format!(
            "Recorded: guard `{}` approved by `{}` (human) at {} under authorization decision {}.",
            approval.guard_name().as_str(),
            approval.approved_by().as_str(),
            approval.approved_at(),
            decision_id.as_str()
        ));
        Ok(ApproveGuardOutcome::Recorded {
            guard_name: approval.guard_name().clone(),
            role_id: approval.approved_by().clone(),
            approved_at: approval.approved_at(),
            decision_id,
        })
    }

    fn compose(&self) -> Result<(EmbeddedMade, TrustedHostAuthorizationGate), String> {
        let made = EmbeddedMade::open(&self.store).map_err(|error| {
            format!(
                "could not open the ceremony store at `{}`: {error}",
                self.store.display()
            )
        })?;
        let store: Arc<dyn AuthorizationPolicyStorePort> = Arc::new(
            SqliteAuthorizationPolicyStore::open(&self.store)
                .map_err(|error| format!("could not open the authorization policy: {error}"))?,
        );
        let principal = AuthenticatedPrincipal::new(
            self.trusted_host_id.clone(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .map_err(|error| error.to_string())?;
        let clock = Arc::new(SystemClock::new());
        let authorize = Arc::new(AuthorizeOperationUseCase::new(
            self.policy_id.clone(),
            store.clone(),
            clock,
            AuthorizationDecisionTtl::from_seconds(60).expect("fixed TTL is valid"),
        ));
        let gate = TrustedHostAuthorizationGate::new(authorize, principal)
            .map_err(|error| error.to_string())?;
        let made = made.with_authorization_policy(self.policy_id.clone(), store);
        Ok((made, gate))
    }

    /// One authorization per action, against the ceremony's scope, with
    /// the same target digest the MCP path would compute for the same
    /// call: the decision in the journal reads the same whichever
    /// channel recorded it.
    async fn authorize(
        &self,
        gate: &TrustedHostAuthorizationGate,
        action: AuthorizationAction,
        arguments: serde_json::Value,
    ) -> Result<AuthorizedOperation, String> {
        let tool = match action {
            AuthorizationAction::ApproveCeremonyGuard => APPROVE_CEREMONY_GUARD_TOOL,
            _ => "made_get_ceremony_instance",
        };
        let request_id =
            AuthorizationRequestId::new(format!("made-terminal-{}", uuid::Uuid::new_v4().simple()))
                .map_err(|error| error.to_string())?;
        gate.authorize(
            request_id,
            action,
            AuthorizationScope::Ceremony {
                ceremony_id: self.ceremony_id.clone(),
            },
            ToolTraceContext::authorization_target_digest(tool, &arguments),
            None,
        )
        .await
        .map_err(|error| {
            format!(
                "the authorization policy did not admit `{}` for `{}`: {error}. Issue a grant \
                 for that action to this trusted host, or ask whoever administers the policy.",
                serde_json::to_value(action)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                self.trusted_host_id.as_str()
            )
        })
    }

    fn approval_arguments(&self) -> serde_json::Value {
        json!({
            "ceremony_id": self.ceremony_id.as_str(),
            "guard_name": self.guard_name.as_str(),
            "role_id": self.role_id.as_str(),
            "role_kind": "human",
        })
    }

    fn show(&self, terminal: &mut dyn Terminal, instance: &CeremonyInstance) {
        terminal.show(&format!(
            "Ceremony `{}` runs `{}` version `{}`; it is {} in state `{}`.",
            instance.id().as_str(),
            instance.definition_name().as_str(),
            instance.definition_version().as_str(),
            instance.lifecycle().phase().as_label(),
            instance.current_state().as_str()
        ));
        for (step_id, record) in instance.step_records() {
            terminal.show(&format!(
                "  step `{}`: {}",
                step_id.as_str(),
                record.status().as_label()
            ));
        }
        for approval in instance.guard_approvals() {
            terminal.show(&format!(
                "  already approved: guard `{}` by `{}` ({}) at {}",
                approval.guard_name().as_str(),
                approval.approved_by().as_str(),
                approval.approved_by_kind().as_str(),
                approval.approved_at()
            ));
        }
        if let Some(reason) = &self.reason {
            terminal.show(&format!("  your stated reason: {reason}"));
        }
        terminal.show(
            "An approval is sealed in the ceremony's journal with your role and the kind \
             `human`; it cannot be withdrawn, only superseded by what the ceremony does next.",
        );
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use made_app::authorization::AuthorizationPolicyAdministrationService;
    use made_core::value_objects::{
        AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, DelegationDepth,
    };
    use serde_json::Value;

    use super::*;
    use crate::{EmbeddedMadeMcpBackend, HumanApprovalSource, MadeMcpServer};

    const POLICY: &str = "terminal-approval-policy";
    const HOST: &str = "terminal-approval-host";
    const CEREMONY_YAML: &str = r#"
version: "1.0"
name: "terminal_approval"
states:
  - id: REVIEW
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: REVIEW
    to: DONE
    trigger: approve
    guards:
      - lead_approved
steps:
  - id: review
    state: REVIEW
    handler: embedded_noop
guards:
  lead_approved:
    type: human
    check: manual_approval
roles:
  - id: REVIEWER
    allowed_actions:
      - review
  - id: LEAD
    allowed_actions:
      - approve
"#;

    #[derive(Default)]
    struct ScriptedTerminal {
        interactive: bool,
        answer: bool,
        shown: Rc<RefCell<Vec<String>>>,
        asked: Rc<RefCell<Vec<String>>>,
    }

    impl Terminal for ScriptedTerminal {
        fn is_interactive(&self) -> bool {
            self.interactive
        }

        fn show(&mut self, line: &str) {
            self.shown.borrow_mut().push(line.to_owned());
        }

        fn confirm(&mut self, prompt: &str) -> Result<bool, String> {
            self.asked.borrow_mut().push(prompt.to_owned());
            Ok(self.answer)
        }
    }

    async fn bootstrap(store: &Path) {
        let administration = AuthorizationPolicyAdministrationService::new(
            AuthorizationPolicyId::new(POLICY).unwrap(),
            Arc::new(SqliteAuthorizationPolicyStore::open(store).unwrap()),
            Arc::new(SystemClock::new()),
        );
        let owner = AuthenticatedPrincipal::new(
            PrincipalId::new(HOST).unwrap(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )
        .unwrap();
        administration
            .open(owner.clone(), Vec::new())
            .await
            .unwrap();
        administration
            .issue(
                &owner,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new("terminal-approval-grant").unwrap(),
                    owner.id().clone(),
                    [
                        AuthorizationAction::PublishCeremonyDefinition,
                        AuthorizationAction::StartPublishedCeremony,
                        AuthorizationAction::GetCeremonyInstance,
                        AuthorizationAction::ApproveCeremonyGuard,
                        AuthorizationAction::ReadCeremonyEvents,
                    ],
                    AuthorizationScope::Global,
                    (time::OffsetDateTime::UNIX_EPOCH, None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(owner.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }

    async fn call(server: &MadeMcpServer, id: u64, tool: &str, arguments: Value) -> Value {
        let request = json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": tool, "arguments": arguments},
        });
        let response = server.handle_json_line(&request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_ne!(response["result"]["isError"], true, "{response}");
        response["result"]["structuredContent"].clone()
    }

    async fn started_session(store: &Path) -> MadeMcpServer {
        bootstrap(store).await;
        let server = MadeMcpServer::embedded_sqlite_authorized(store, POLICY, HOST).unwrap();
        server.initialize_backend().await.unwrap();
        call(
            &server,
            1,
            "made_publish_ceremony_definition",
            json!({"definition_yaml": CEREMONY_YAML}),
        )
        .await;
        call(
            &server,
            2,
            "made_start_published_ceremony",
            json!({
                "ceremony": "terminal_approval", "version": "1.0", "ceremony_id": "review-7",
                "actor_id": "operator", "actor_kind": "service", "context": {},
            }),
        )
        .await;
        server
    }

    fn command(store: &Path) -> ApproveGuardCommand {
        ApproveGuardCommand::new(
            store,
            POLICY,
            HOST,
            "review-7",
            "lead_approved",
            "LEAD",
            Some("looks right".to_owned()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn a_person_at_a_terminal_records_the_approval_as_a_human_under_a_decision() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        let server = started_session(&store).await;

        let mut terminal = ScriptedTerminal {
            interactive: true,
            answer: true,
            ..ScriptedTerminal::default()
        };
        let outcome = Box::pin(command(&store).run(&mut terminal)).await.unwrap();
        let ApproveGuardOutcome::Recorded {
            guard_name,
            role_id,
            decision_id,
            ..
        } = outcome
        else {
            panic!("expected the approval to be recorded");
        };
        assert_eq!(guard_name.as_str(), "lead_approved");
        assert_eq!(role_id.as_str(), "LEAD");
        let shown = terminal.shown.borrow().join("\n");
        assert!(shown.contains("state `REVIEW`"), "{shown}");
        assert!(shown.contains("step `review`: pending"), "{shown}");
        assert!(shown.contains("your stated reason: looks right"), "{shown}");
        assert!(shown.contains(decision_id.as_str()), "{shown}");
        assert_eq!(terminal.asked.borrow().len(), 1);

        // The MCP session reads the same record the terminal sealed: a
        // human approval admitted under its own authorization decision.
        let instance = call(
            &server,
            3,
            "made_get_ceremony_instance",
            json!({"ceremony_id": "review-7"}),
        )
        .await;
        assert_eq!(instance["context"]["lead_approved"], true);
        let events = call(
            &server,
            4,
            "made_read_ceremony_events",
            json!({"ceremony_id": "review-7"}),
        )
        .await;
        let approval = events["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["event"]["type"] == "human_approval_recorded")
            .expect("the approval is in the journal");
        assert_eq!(approval["actor"]["kind"], "human");
        assert_eq!(
            approval["authorization"]["decision_id"],
            decision_id.as_str()
        );
        assert_eq!(
            approval["authorization"]["action"],
            "approve_ceremony_guard"
        );
    }

    #[tokio::test]
    async fn piped_input_is_refused_before_the_store_is_opened() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        let mut terminal = ScriptedTerminal {
            interactive: false,
            answer: true,
            ..ScriptedTerminal::default()
        };
        let error = Box::pin(command(&store).run(&mut terminal))
            .await
            .unwrap_err();
        assert_eq!(error, NOT_INTERACTIVE);
        assert!(!store.exists(), "a refused command must not create a store");
        assert!(terminal.asked.borrow().is_empty());
    }

    #[tokio::test]
    async fn a_declined_prompt_writes_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        let server = started_session(&store).await;
        let mut terminal = ScriptedTerminal {
            interactive: true,
            answer: false,
            ..ScriptedTerminal::default()
        };
        assert_eq!(
            Box::pin(command(&store).run(&mut terminal)).await.unwrap(),
            ApproveGuardOutcome::Declined
        );
        let instance = call(
            &server,
            3,
            "made_get_ceremony_instance",
            json!({"ceremony_id": "review-7"}),
        )
        .await;
        assert!(
            instance["context"].get("lead_approved").is_none(),
            "{instance}"
        );
    }

    #[tokio::test]
    async fn in_terminal_mode_the_mcp_tool_refuses_and_names_the_command() {
        let server = MadeMcpServer::with_backend(
            EmbeddedMadeMcpBackend::new(EmbeddedMade::default())
                .with_human_approval_source(HumanApprovalSource::Terminal),
        );
        let request = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": APPROVE_CEREMONY_GUARD_TOOL, "arguments": {
                "ceremony_id": "any", "guard_name": "lead_approved",
                "role_id": "LEAD", "role_kind": "human",
            }},
        });
        let response = server.handle_json_line(&request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["result"]["isError"], true);
        assert_eq!(response["result"]["structuredContent"]["code"], "refused");
        let message = response["result"]["structuredContent"]["message"]
            .as_str()
            .unwrap();
        assert!(message.contains("made-mcp approve-guard"), "{message}");

        // Discovery says which channel this server accepts.
        let request = json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "made_discover_capabilities", "arguments": {}},
        });
        let response = server.handle_json_line(&request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(
            response["result"]["structuredContent"]["human_approval"]["source"],
            "terminal"
        );
    }
}
