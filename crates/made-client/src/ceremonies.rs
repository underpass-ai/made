use made_proto::v1::{
    CeremonyInstanceState, GetCeremonyInstanceRequest, ListCeremonyInstancesRequest,
    SearchCeremonyInstancesRequest,
};
use std::collections::{BTreeSet, VecDeque};

use crate::{CeremonySearchPage, CeremonyTree, MadeClient, MadeClientError};

impl MadeClient {
    pub async fn get_ceremony(
        &self,
        ceremony_id: impl Into<String>,
    ) -> Result<CeremonyInstanceState, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .get_ceremony_instance(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/GetCeremonyInstance",
                GetCeremonyInstanceRequest {
                    ceremony_id: ceremony_id.into(),
                },
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        response.instance.ok_or_else(|| {
            MadeClientError::ProtocolViolation("get ceremony response has no instance".to_owned())
        })
    }

    /// Existing unpaged server listing kept for compatibility.
    pub async fn list_ceremonies(&self) -> Result<Vec<CeremonyInstanceState>, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .list_ceremony_instances(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ListCeremonyInstances",
                ListCeremonyInstancesRequest {},
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        Ok(response.instances)
    }

    /// Search one bounded page. The returned cursor is opaque and bound to
    /// the request's filters; callers must send it back unchanged.
    pub async fn search_ceremonies(
        &self,
        query: SearchCeremonyInstancesRequest,
    ) -> Result<CeremonySearchPage, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .search_ceremony_instances(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/SearchCeremonyInstances",
                query,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        Ok(CeremonySearchPage::new(
            response.instances,
            (!response.next_cursor.is_empty()).then_some(response.next_cursor),
        ))
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
