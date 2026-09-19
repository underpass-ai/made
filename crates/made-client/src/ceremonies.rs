use made_proto::v1::{
    CeremonyInstanceState, GetCeremonyInstanceRequest, ListCeremonyInstancesRequest,
};
use std::collections::{BTreeSet, VecDeque};

use crate::{CeremonyTree, MadeClient, MadeClientError};

impl MadeClient {
    pub async fn get_ceremony(
        &self,
        ceremony_id: impl Into<String>,
    ) -> Result<CeremonyInstanceState, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .get_ceremony_instance(GetCeremonyInstanceRequest {
                ceremony_id: ceremony_id.into(),
            })
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        response.instance.ok_or_else(|| {
            MadeClientError::ProtocolViolation("get ceremony response has no instance".to_owned())
        })
    }

    /// Existing unpaged server listing. Prefer `search_ceremonies` once the
    /// paginated C5.8 RPC is available.
    pub async fn list_ceremonies(&self) -> Result<Vec<CeremonyInstanceState>, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .list_ceremony_instances(ListCeremonyInstancesRequest {})
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        Ok(response.instances)
    }

    pub async fn ceremony_tree(
        &self,
        root_id: impl Into<String>,
        max_nodes: usize,
    ) -> Result<CeremonyTree, MadeClientError> {
        let root_id = root_id.into();
        let mut pending = VecDeque::from([root_id]);
        let mut seen = BTreeSet::new();
        let mut instances = Vec::new();
        while let Some(ceremony_id) = pending.pop_front() {
            if !seen.insert(ceremony_id.clone()) {
                continue;
            }
            if seen.len() > max_nodes {
                return Err(MadeClientError::ProtocolViolation(format!(
                    "ceremony tree exceeds the requested {max_nodes} node limit"
                )));
            }
            let instance = self.get_ceremony(ceremony_id).await?;
            for child in instance
                .child_groups
                .iter()
                .flat_map(|group| group.children.iter())
            {
                pending.push_back(child.child_id.clone());
            }
            instances.push(instance);
        }
        CeremonyTree::from_instances(instances)
    }
}
