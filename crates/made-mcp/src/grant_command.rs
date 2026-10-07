//! `made-mcp grant`: what the host may do, decided by a person at a
//! terminal the agent's MCP session cannot write to.
//!
//! Bootstrapping a store opens its authorization policy and makes the
//! trusted host its owner; it grants nothing. Every business tool the
//! host calls is then refused until a grant names its action. Over MCP
//! the grant is issued with `made_issue_authorization_grant`, a tool the
//! plugin's `core` profile hides on purpose: a session that can widen
//! its own authority has no authority boundary. This command is the
//! other channel. It opens the same store and the same policy, shows
//! the person which actions, for which principal, at which scope, asks,
//! and issues the grant as the policy owner. The record is the same
//! `GrantIssued` fact the MCP tool would seal; what differs is who
//! could have written it.
//!
//! Like `approve-guard`, it proves the channel and not the person: a
//! process that can drive a pseudo-terminal can answer the prompt.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::{
    AuthorizationMutationOutcome, AuthorizationPolicyAdministrationService,
    ReadAuthorizationPolicyUseCase,
};
use made_core::entities::AuthorizationPolicy;
use made_core::ports::AuthorizationPolicyStorePort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationGrant,
    AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId, AuthorizationScope,
    CeremonyId, CeremonyName, CeremonyVersion, DelegationDepth, PrincipalId, PrincipalKind,
};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::terminal::Terminal;

mod grant_arguments;
mod grant_outcome;
mod grant_selection;
mod granted_authority;

pub use grant_arguments::GrantArguments;
pub use grant_outcome::GrantOutcome;
pub use grant_selection::GrantSelection;
pub use granted_authority::GrantedAuthority;

pub use crate::authorization_channel::{GRANT_COMMAND, GRANT_SCRIPT};

use granted_authority::{action_name, scope_label};

/// Why the command refuses piped input. Spelled once so the refusal
/// and the tests agree.
pub const NOT_INTERACTIVE: &str = "grant decides what a host may do and runs only at an \
     interactive terminal; its stdin and stdout are not terminals here. It does not read \
     answers from a pipe, an environment variable or a flag, because any of those is a \
     channel an agent can write to. A script that must grant uses \
     made_issue_authorization_grant over MCP, under a tool profile that lists it.";

/// One grant to put to a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantCommand {
    store: PathBuf,
    policy_id: AuthorizationPolicyId,
    trusted_host_id: PrincipalId,
    grantee: PrincipalId,
    grant_id: AuthorizationGrantId,
    actions: BTreeSet<AuthorizationAction>,
    scope: AuthorizationScope,
    valid_until: Option<OffsetDateTime>,
    selection: String,
}

impl GrantCommand {
    /// Build the grant from what the person typed and the policy
    /// identity the launcher gives the server. Everything is validated
    /// here, before a store is opened.
    pub fn from_arguments(
        arguments: &GrantArguments,
        policy_id: &str,
        trusted_host_id: &str,
    ) -> Result<Self, String> {
        let selection = arguments.selection.as_ref().ok_or_else(|| {
            "choose what to grant: --profile core, or --actions <a,b,...>".to_owned()
        })?;
        let actions = selection.actions()?;
        let trusted_host_id =
            PrincipalId::new(trusted_host_id).map_err(|error| error.to_string())?;
        let grantee = match &arguments.grantee {
            Some(grantee) => PrincipalId::new(grantee).map_err(|error| error.to_string())?,
            None => trusted_host_id.clone(),
        };
        let grant_id = match &arguments.grant_id {
            Some(id) => id.clone(),
            None => format!(
                "terminal-{}-{}",
                selection.slug(),
                &uuid::Uuid::new_v4().simple().to_string()[..8]
            ),
        };
        Ok(Self {
            store: arguments.store.clone(),
            policy_id: AuthorizationPolicyId::new(policy_id).map_err(|error| error.to_string())?,
            trusted_host_id,
            grantee,
            grant_id: AuthorizationGrantId::new(grant_id).map_err(|error| error.to_string())?,
            actions,
            scope: parse_scope(arguments.scope.as_deref().unwrap_or("global"))?,
            valid_until: arguments
                .valid_until
                .as_deref()
                .map(|until| {
                    OffsetDateTime::parse(until, &Rfc3339)
                        .map_err(|error| format!("--valid-until must be RFC 3339: {error}"))
                })
                .transpose()?,
            selection: selection.describe(),
        })
    }

    #[must_use]
    pub fn store(&self) -> &Path {
        &self.store
    }

    #[must_use]
    pub fn grant_id(&self) -> &AuthorizationGrantId {
        &self.grant_id
    }

    /// Show the person what they are granting, ask, and issue it.
    ///
    /// # Errors
    ///
    /// Refuses a non-interactive terminal before touching the store;
    /// otherwise returns the store's or the policy's refusal, worded
    /// for the person at the terminal.
    pub async fn run(&self, terminal: &mut dyn Terminal) -> Result<GrantOutcome, String> {
        if !terminal.is_interactive() {
            return Err(NOT_INTERACTIVE.to_owned());
        }
        let store = open_policy_store(&self.store)?;
        let owner = trusted_host(&self.trusted_host_id)?;
        let policy = read_policy(&self.policy_id, &store, &self.store).await?;
        if policy.owner() != Some(&owner) {
            return Err(format!(
                "trusted host `{}` does not own policy `{}`; only the owner issues grants from \
                 the terminal",
                self.trusted_host_id.as_str(),
                self.policy_id.as_str()
            ));
        }
        let now = OffsetDateTime::now_utc();
        let held = GrantedAuthority::project(&policy, &trusted_host(&self.grantee)?, now);
        self.show(terminal, &held);

        let prompt = format!(
            "Issue grant `{}` to `{}` ({} action(s), scope {})?",
            self.grant_id.as_str(),
            self.grantee.as_str(),
            self.actions.len(),
            scope_label(&serde_json::to_value(&self.scope).unwrap_or_default())
        );
        if !terminal.confirm(&prompt)? {
            terminal.show("Nothing was recorded.");
            return Ok(GrantOutcome::Declined);
        }

        // No start restriction: the policy journal records when the
        // grant was issued, and a grant whose content does not depend on
        // the clock is one a re-run recognises as already recorded.
        let grant = AuthorizationGrant::new(
            self.grant_id.clone(),
            self.grantee.clone(),
            self.actions.iter().copied(),
            self.scope.clone(),
            (OffsetDateTime::UNIX_EPOCH, self.valid_until),
            DelegationDepth::none(),
            AuthorizationGrantIssuer::direct(owner.clone()),
        )
        .map_err(|error| format!("the grant is not valid: {error}"))?;
        let administration = AuthorizationPolicyAdministrationService::new(
            self.policy_id.clone(),
            store,
            Arc::new(SystemClock::new()),
        );
        let outcome = administration
            .issue(&owner, grant)
            .await
            .map_err(|error| match error {
                made_core::DomainError::Conflict { .. } => format!(
                    "grant id `{}` already names a different grant in policy `{}`; choose \
                     another --grant-id, or revoke that grant first",
                    self.grant_id.as_str(),
                    self.policy_id.as_str()
                ),
                other => format!("the policy refused the grant: {other}"),
            })?;
        let (version, existing) = match outcome {
            AuthorizationMutationOutcome::Applied { version } => (version, false),
            AuthorizationMutationOutcome::Existing { version } => (version, true),
        };
        terminal.show(&if existing {
            format!(
                "Already recorded: an identical grant `{}` is in policy `{}` (version {}).",
                self.grant_id.as_str(),
                self.policy_id.as_str(),
                version.value()
            )
        } else {
            format!(
                "Recorded grant `{}` in policy `{}` at version {}: `{}` may perform {} action(s) \
                 at scope {}. A running MCP session sees it on its next call.",
                self.grant_id.as_str(),
                self.policy_id.as_str(),
                version.value(),
                self.grantee.as_str(),
                self.actions.len(),
                scope_label(&serde_json::to_value(&self.scope).unwrap_or_default())
            )
        });
        Ok(GrantOutcome::Recorded {
            grant_id: self.grant_id.clone(),
            version,
            existing,
        })
    }

    /// `--show`: what the grantee holds, read from the policy and asked
    /// of nobody. Allowed off a terminal because it writes nothing.
    pub async fn show_authority(
        arguments: &GrantArguments,
        policy_id: &str,
        trusted_host_id: &str,
    ) -> Result<Vec<String>, String> {
        let store = open_policy_store(&arguments.store)?;
        let policy_id = AuthorizationPolicyId::new(policy_id).map_err(|error| error.to_string())?;
        let grantee = PrincipalId::new(arguments.grantee.as_deref().unwrap_or(trusted_host_id))
            .map_err(|error| error.to_string())?;
        let policy = read_policy(&policy_id, &store, &arguments.store).await?;
        let held =
            GrantedAuthority::project(&policy, &trusted_host(&grantee)?, OffsetDateTime::now_utc());
        let mut lines = held.lines();
        if held
            .granted_actions()
            .iter()
            .all(|action| granted_authority::is_administrative(*action))
        {
            lines.push(format!(
                "Grant the ordinary route with: made-mcp grant {} --profile core",
                arguments.store.display()
            ));
        }
        Ok(lines)
    }

    fn show(&self, terminal: &mut dyn Terminal, held: &GrantedAuthority) {
        terminal.show(&format!(
            "Store `{}`; issuer `{}` (the policy owner).",
            self.store.display(),
            self.trusted_host_id.as_str()
        ));
        for line in held.lines() {
            terminal.show(&line);
        }
        let already_held = self
            .actions
            .iter()
            .filter(|action| held.granted_actions().contains(*action))
            .count();
        terminal.show(&format!(
            "This grant `{}` ({}) names {} action(s) at scope {}, {}:",
            self.grant_id.as_str(),
            self.selection,
            self.actions.len(),
            scope_label(&serde_json::to_value(&self.scope).unwrap_or_default()),
            self.valid_until
                .and_then(|until| until.format(&Rfc3339).ok())
                .map_or_else(
                    || "with no expiry".to_owned(),
                    |until| format!("valid until {until}")
                )
        ));
        for action in &self.actions {
            terminal.show(&format!("  {}", action_name(*action)));
        }
        if already_held > 0 {
            terminal.show(&format!(
                "  ({already_held} of them already admitted at global scope by an earlier grant)"
            ));
        }
        terminal.show(
            "A grant is sealed in the policy's journal with you as its issuer. It is not edited \
             in place; it is revoked by id (made_revoke_authorization_grant, in the \
             authorization_administration group) and replaced.",
        );
    }
}

fn open_policy_store(store: &Path) -> Result<Arc<dyn AuthorizationPolicyStorePort>, String> {
    if !store.exists() {
        return Err(format!(
            "no store at `{}`; bootstrap it first (the plugin's made-setup does)",
            store.display()
        ));
    }
    Ok(Arc::new(
        SqliteAuthorizationPolicyStore::open(store)
            .map_err(|error| format!("could not open the authorization policy: {error}"))?,
    ))
}

async fn read_policy(
    policy_id: &AuthorizationPolicyId,
    store: &Arc<dyn AuthorizationPolicyStorePort>,
    path: &Path,
) -> Result<AuthorizationPolicy, String> {
    ReadAuthorizationPolicyUseCase::new(policy_id.clone(), store.clone())
        .execute()
        .await
        .map(|snapshot| snapshot.policy)
        .map_err(|error| {
            format!(
                "could not read policy `{}` in `{}`: {error}. Bootstrap the store first \
                 (`made-mcp bootstrap-authorization`; the plugin's made-setup does)",
                policy_id.as_str(),
                path.display()
            )
        })
}

fn trusted_host(id: &PrincipalId) -> Result<AuthenticatedPrincipal, String> {
    AuthenticatedPrincipal::new(
        id.clone(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .map_err(|error| error.to_string())
}

/// `global`, `ceremony:<id>`, `ceremony_tree:<id>` or
/// `definition:<name>[@<version>]`.
fn parse_scope(spec: &str) -> Result<AuthorizationScope, String> {
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("global") {
        return Ok(AuthorizationScope::Global);
    }
    let Some((kind, target)) = spec.split_once(':') else {
        return Err(format!(
            "--scope must be `global`, `ceremony:<id>`, `ceremony_tree:<id>` or \
             `definition:<name>[@<version>]`; got `{spec}`"
        ));
    };
    let domain = |error: made_core::DomainError| error.to_string();
    match kind {
        "ceremony" => Ok(AuthorizationScope::Ceremony {
            ceremony_id: CeremonyId::new(target).map_err(domain)?,
        }),
        "ceremony_tree" => Ok(AuthorizationScope::CeremonyTree {
            root_id: CeremonyId::new(target).map_err(domain)?,
        }),
        "definition" => {
            let (name, version) = target
                .split_once('@')
                .map_or((target, None), |(name, version)| (name, Some(version)));
            Ok(AuthorizationScope::Definition {
                name: CeremonyName::new(name).map_err(domain)?,
                version: version
                    .map(|version| CeremonyVersion::new(version).map_err(domain))
                    .transpose()?,
            })
        }
        other => Err(format!(
            "--scope kind `{other}` is not grantable here; use global, ceremony, ceremony_tree \
             or definition"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_json::{json, Value};

    use super::*;
    use crate::tool_profile::ToolProfile;
    use crate::MadeMcpServer;

    const POLICY: &str = "terminal-grant-policy";
    const HOST: &str = "terminal-grant-host";

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

    /// What `made-mcp bootstrap-authorization` does: open the policy
    /// with the trusted host as owner and grant nothing.
    async fn bootstrap(store: &Path) {
        let administration = AuthorizationPolicyAdministrationService::new(
            AuthorizationPolicyId::new(POLICY).unwrap(),
            Arc::new(SqliteAuthorizationPolicyStore::open(store).unwrap()),
            Arc::new(SystemClock::new()),
        );
        administration
            .open(
                trusted_host(&PrincipalId::new(HOST).unwrap()).unwrap(),
                Vec::new(),
            )
            .await
            .unwrap();
    }

    async fn core_session(store: &Path) -> MadeMcpServer {
        let server = MadeMcpServer::embedded_sqlite_authorized(store, POLICY, HOST)
            .unwrap()
            .with_tool_profile(ToolProfile::core());
        server.initialize_backend().await.unwrap();
        server
    }

    async fn call(server: &MadeMcpServer, id: u64, tool: &str, arguments: Value) -> Value {
        let request = json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": tool, "arguments": arguments},
        });
        let response = server.handle_json_line(&request.to_string()).await.unwrap();
        let response: Value = serde_json::from_str(&response).unwrap();
        response["result"].clone()
    }

    fn design_arguments() -> Value {
        json!({
            "name": "pr_review",
            "objective": "Review a change: an author proposes and a lead approves.",
            "outputs": ["verdict"],
            "participants": [{"role_id": "AUTHOR"}, {"role_id": "LEAD"}],
            "stages": [{"id": "propose", "owner_role_id": "AUTHOR", "instructions": "Propose."}],
            "final_approval": {"role_id": "LEAD"},
        })
    }

    fn arguments(store: &Path, flags: &[&str]) -> GrantArguments {
        let mut args = vec![store.display().to_string()];
        args.extend(flags.iter().map(|flag| (*flag).to_owned()));
        GrantArguments::parse(&args).unwrap()
    }

    #[tokio::test]
    async fn a_fresh_store_refuses_with_the_remedy_and_a_terminal_grant_unblocks_it() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        bootstrap(&store).await;
        let server = core_session(&store).await;

        // Before: refused, and the refusal says what to do and by whom.
        let refused = call(&server, 1, "made_design_ceremony", design_arguments()).await;
        assert_eq!(refused["isError"], true, "{refused}");
        assert_eq!(refused["structuredContent"]["code"], "refused");
        let message = refused["structuredContent"]["message"].as_str().unwrap();
        assert!(message.contains("denied `design_ceremony`"), "{message}");
        assert!(
            message.contains(&format!("principal `{HOST}` holds no live grant")),
            "{message}"
        );
        assert!(
            message.contains("made-mcp grant <store> --profile core"),
            "{message}"
        );
        assert!(message.contains("--actions design_ceremony"), "{message}");
        assert!(message.contains("cannot grant itself"), "{message}");

        // Discovery said so first.
        let discovery = call(&server, 2, "made_discover_capabilities", json!({})).await;
        let authorization = &discovery["structuredContent"]["authorization"];
        assert_eq!(authorization["principal"], HOST);
        assert_eq!(authorization["policy_id"], POLICY);
        assert_eq!(authorization["owner"], true);
        assert!(authorization["grants"].as_array().unwrap().is_empty());
        let without: Vec<&str> = authorization["listed_tools_without_grant"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(without.contains(&"made_design_ceremony"), "{authorization}");
        assert!(
            !without.contains(&"made_discover_capabilities"),
            "{authorization}"
        );
        assert_eq!(authorization["grant_command"], GRANT_COMMAND);

        // A person at a terminal grants the ordinary route.
        let command = GrantCommand::from_arguments(
            &arguments(
                &store,
                &["--profile", "core", "--grant-id", "ordinary-route"],
            ),
            POLICY,
            HOST,
        )
        .unwrap();
        let mut terminal = ScriptedTerminal {
            interactive: true,
            answer: true,
            ..ScriptedTerminal::default()
        };
        let outcome = Box::pin(command.run(&mut terminal)).await.unwrap();
        let GrantOutcome::Recorded {
            grant_id, existing, ..
        } = outcome
        else {
            panic!("expected the grant to be recorded");
        };
        assert_eq!(grant_id.as_str(), "ordinary-route");
        assert!(!existing);
        let shown = terminal.shown.borrow().join("\n");
        assert!(shown.contains("No grant names this principal."), "{shown}");
        assert!(shown.contains("tool profile `core`"), "{shown}");
        assert!(shown.contains("  design_ceremony"), "{shown}");
        assert!(shown.contains("Recorded grant `ordinary-route`"), "{shown}");
        assert_eq!(terminal.asked.borrow().len(), 1);

        // After: the same session, no restart, is admitted.
        let designed = call(&server, 3, "made_design_ceremony", design_arguments()).await;
        assert_ne!(designed["isError"], true, "{designed}");
        let discovery = call(&server, 4, "made_discover_capabilities", json!({})).await;
        let authorization = &discovery["structuredContent"]["authorization"];
        assert!(authorization["granted_actions"]
            .as_array()
            .unwrap()
            .contains(&json!("design_ceremony")));
        assert!(
            authorization["listed_tools_without_grant"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{authorization}"
        );
        assert_eq!(authorization["grants"][0]["grant_id"], "ordinary-route");
    }

    /// An identical re-run appends nothing; the same id with other
    /// content is a conflict, named as one.
    #[tokio::test]
    async fn a_grant_id_is_idempotent_for_the_same_content_and_a_conflict_otherwise() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        bootstrap(&store).await;
        let command = GrantCommand::from_arguments(
            &arguments(
                &store,
                &["--profile", "core", "--grant-id", "ordinary-route"],
            ),
            POLICY,
            HOST,
        )
        .unwrap();
        let mut terminal = ScriptedTerminal {
            interactive: true,
            answer: true,
            ..ScriptedTerminal::default()
        };
        let first = Box::pin(command.run(&mut terminal)).await.unwrap();
        assert!(matches!(
            first,
            GrantOutcome::Recorded {
                existing: false,
                ..
            }
        ));
        let again = Box::pin(command.run(&mut terminal)).await.unwrap();
        assert!(matches!(
            again,
            GrantOutcome::Recorded { existing: true, .. }
        ));
        let other = GrantCommand::from_arguments(
            &arguments(
                &store,
                &[
                    "--actions",
                    "design_ceremony",
                    "--grant-id",
                    "ordinary-route",
                ],
            ),
            POLICY,
            HOST,
        )
        .unwrap();
        let error = Box::pin(other.run(&mut terminal)).await.unwrap_err();
        assert!(error.contains("already names a different grant"), "{error}");
    }

    #[tokio::test]
    async fn piped_input_is_refused_before_the_store_is_opened() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        let command =
            GrantCommand::from_arguments(&arguments(&store, &["--profile", "core"]), POLICY, HOST)
                .unwrap();
        let mut terminal = ScriptedTerminal {
            interactive: false,
            answer: true,
            ..ScriptedTerminal::default()
        };
        let error = Box::pin(command.run(&mut terminal)).await.unwrap_err();
        assert_eq!(error, NOT_INTERACTIVE);
        assert!(!store.exists(), "a refused command must not create a store");
        assert!(terminal.asked.borrow().is_empty());
    }

    #[tokio::test]
    async fn a_declined_prompt_and_a_missing_store_write_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        let command =
            GrantCommand::from_arguments(&arguments(&store, &["--profile", "core"]), POLICY, HOST)
                .unwrap();
        let mut terminal = ScriptedTerminal {
            interactive: true,
            answer: false,
            ..ScriptedTerminal::default()
        };
        let error = Box::pin(command.run(&mut terminal)).await.unwrap_err();
        assert!(error.contains("no store at"), "{error}");
        assert!(!store.exists());

        bootstrap(&store).await;
        assert_eq!(
            Box::pin(command.run(&mut terminal)).await.unwrap(),
            GrantOutcome::Declined
        );
        let server = core_session(&store).await;
        let refused = call(&server, 1, "made_design_ceremony", design_arguments()).await;
        assert_eq!(refused["isError"], true, "{refused}");
    }

    #[tokio::test]
    async fn show_reads_the_policy_and_names_the_command_when_nothing_is_held() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        bootstrap(&store).await;
        let lines = GrantCommand::show_authority(&arguments(&store, &["--show"]), POLICY, HOST)
            .await
            .unwrap();
        let text = lines.join("\n");
        assert!(text.contains("owns it and may administer grants"), "{text}");
        assert!(text.contains("No grant names this principal."), "{text}");
        assert!(text.contains("no business action"), "{text}");
        assert!(text.contains("--profile core"), "{text}");

        let command = GrantCommand::from_arguments(
            &arguments(
                &store,
                &[
                    "--actions",
                    "design_ceremony",
                    "--scope",
                    "definition:pr_review",
                ],
            ),
            POLICY,
            HOST,
        )
        .unwrap();
        let mut terminal = ScriptedTerminal {
            interactive: true,
            answer: true,
            ..ScriptedTerminal::default()
        };
        Box::pin(command.run(&mut terminal)).await.unwrap();
        let lines = GrantCommand::show_authority(&arguments(&store, &["--show"]), POLICY, HOST)
            .await
            .unwrap();
        let text = lines.join("\n");
        assert!(
            text.contains("1 action(s) at scope definition `pr_review`, any version"),
            "{text}"
        );
        // Scoped to one definition, so still nothing at global scope.
        assert!(text.contains("no business action"), "{text}");
    }

    #[tokio::test]
    async fn another_grantee_scopes_and_an_unowned_policy_are_handled() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ceremonies.sqlite3");
        bootstrap(&store).await;

        let command = GrantCommand::from_arguments(
            &arguments(
                &store,
                &[
                    "--actions",
                    "get_ceremony_instance",
                    "--grantee",
                    "reviewer-laptop",
                    "--scope",
                    "ceremony:pr-1",
                    "--valid-until",
                    "2099-01-01T00:00:00Z",
                ],
            ),
            POLICY,
            HOST,
        )
        .unwrap();
        assert!(command
            .grant_id()
            .as_str()
            .starts_with("terminal-get-ceremony-instance-"));
        let mut terminal = ScriptedTerminal {
            interactive: true,
            answer: true,
            ..ScriptedTerminal::default()
        };
        Box::pin(command.run(&mut terminal)).await.unwrap();
        let shown = terminal.shown.borrow().join("\n");
        assert!(shown.contains("scope ceremony `pr-1`"), "{shown}");
        assert!(shown.contains("until 2099-01-01T00:00:00Z"), "{shown}");

        let other = GrantCommand::from_arguments(
            &arguments(&store, &["--profile", "core"]),
            POLICY,
            "not-the-owner",
        )
        .unwrap();
        let error = Box::pin(other.run(&mut terminal)).await.unwrap_err();
        assert!(error.contains("does not own policy"), "{error}");

        assert!(parse_scope("budget:acct").is_err());
        assert!(parse_scope("nonsense").is_err());
        assert_eq!(
            parse_scope("definition:pr_review@1.0").unwrap(),
            AuthorizationScope::Definition {
                name: CeremonyName::new("pr_review").unwrap(),
                version: Some(CeremonyVersion::new("1.0").unwrap()),
            }
        );
    }
}
