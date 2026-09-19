use clap::{Args, Subcommand, ValueEnum};
use made_client::v1::AuthorizationScope;
use made_client::MadeClientError;

#[derive(Debug, Subcommand)]
pub enum AuthorizationCommand {
    Policy,
    Issue {
        grant_id: String,
        grantee_id: String,
        #[arg(long, required = true, value_delimiter = ',')]
        actions: Vec<String>,
        #[command(flatten)]
        scope: AuthorizationScopeArgs,
        #[arg(long)]
        valid_from: String,
        #[arg(long)]
        valid_until: Option<String>,
        #[arg(long, default_value_t = 0)]
        delegation_depth: u32,
        #[arg(long)]
        parent_grant_id: Option<String>,
    },
    Revoke {
        grant_id: String,
        #[arg(long)]
        reason: String,
    },
    Decisions {
        #[arg(long)]
        after: Option<String>,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=500))]
        limit: u32,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AuthorizationScopeKind {
    Global,
    Ceremony,
    CeremonyTree,
    Definition,
    Artifact,
    Council,
    Budget,
}

#[derive(Debug, Args)]
pub struct AuthorizationScopeArgs {
    #[arg(long, value_enum)]
    pub scope: AuthorizationScopeKind,
    /// Resource id, or definition name; omit only for global scope.
    #[arg(long)]
    pub scope_id: Option<String>,
    /// Optional version when scope is definition.
    #[arg(long)]
    pub definition_version: Option<String>,
}

impl AuthorizationScopeArgs {
    pub fn into_proto(self) -> Result<AuthorizationScope, MadeClientError> {
        let id = self.scope_id;
        let missing = || {
            MadeClientError::ProtocolViolation(
                "--scope-id is required for every non-global authorization scope".to_owned(),
            )
        };
        let mut scope = AuthorizationScope {
            kind: match self.scope {
                AuthorizationScopeKind::Global => "global",
                AuthorizationScopeKind::Ceremony => "ceremony",
                AuthorizationScopeKind::CeremonyTree => "ceremony_tree",
                AuthorizationScopeKind::Definition => "definition",
                AuthorizationScopeKind::Artifact => "artifact",
                AuthorizationScopeKind::Council => "council",
                AuthorizationScopeKind::Budget => "budget",
            }
            .to_owned(),
            ..AuthorizationScope::default()
        };
        match self.scope {
            AuthorizationScopeKind::Global if id.is_some() || self.definition_version.is_some() => {
                return Err(MadeClientError::ProtocolViolation(
                    "global authorization scope accepts no resource id or version".to_owned(),
                ));
            }
            AuthorizationScopeKind::Global => {}
            AuthorizationScopeKind::Ceremony => scope.ceremony_id = Some(id.ok_or_else(missing)?),
            AuthorizationScopeKind::CeremonyTree => scope.root_id = Some(id.ok_or_else(missing)?),
            AuthorizationScopeKind::Definition => {
                scope.definition_name = Some(id.ok_or_else(missing)?);
                scope.definition_version = self.definition_version;
            }
            AuthorizationScopeKind::Artifact => scope.artifact_id = Some(id.ok_or_else(missing)?),
            AuthorizationScopeKind::Council => scope.council_id = Some(id.ok_or_else(missing)?),
            AuthorizationScopeKind::Budget => {
                scope.budget_account_id = Some(id.ok_or_else(missing)?);
            }
        }
        Ok(scope)
    }
}
