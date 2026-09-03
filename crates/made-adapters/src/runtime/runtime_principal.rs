/// Static technical identity used when the adapter creates ephemeral
/// Runtime sessions on behalf of MADE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePrincipal {
    pub tenant_id: String,
    pub actor_id: String,
    pub roles: Vec<String>,
}
