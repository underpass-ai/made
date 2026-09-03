/// Static technical identity used when the adapter creates ephemeral
/// Runtime sessions on behalf of MADE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePrincipal {
    pub tenant_id: String,
    pub actor_id: String,
    pub roles: Vec<String>,
}

use made_proto::runtime_v1 as runtime_pb;

impl RuntimePrincipal {
    pub(super) fn to_proto(&self) -> runtime_pb::Principal {
        runtime_pb::Principal {
            tenant_id: self.tenant_id.clone(),
            actor_id: self.actor_id.clone(),
            roles: self.roles.clone(),
        }
    }
}
