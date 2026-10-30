use made_proto::v1::{
    GetProtocolInstanceRequest, ListProtocolInstancesRequest, ProtocolInstanceState,
    SearchProtocolInstancesRequest,
};
use std::collections::{BTreeSet, VecDeque};

use crate::{MadeClient, MadeClientError, ProtocolSearchPage, ProtocolTree};

impl MadeClient {
    pub async fn get_protocol(
        &self,
        protocol_id: impl Into<String>,
    ) -> Result<ProtocolInstanceState, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .get_protocol_instance(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/GetProtocolInstance",
                GetProtocolInstanceRequest {
                    protocol_id: protocol_id.into(),
                },
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        response.instance.ok_or_else(|| {
            MadeClientError::ProtocolViolation("get protocol response has no instance".to_owned())
        })
    }

    /// Existing unpaged server listing kept for compatibility.
    pub async fn list_protocols(&self) -> Result<Vec<ProtocolInstanceState>, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .list_protocol_instances(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ListProtocolInstances",
                ListProtocolInstancesRequest {},
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        Ok(response.instances)
    }

    /// Search one bounded page. The returned cursor is opaque and bound to
    /// the request's filters; callers must send it back unchanged.
    pub async fn search_protocols(
        &self,
        query: SearchProtocolInstancesRequest,
    ) -> Result<ProtocolSearchPage, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .search_protocol_instances(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/SearchProtocolInstances",
                query,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        Ok(ProtocolSearchPage::new(
            response.instances,
            (!response.next_cursor.is_empty()).then_some(response.next_cursor),
        ))
    }

    pub async fn protocol_tree(
        &self,
        root_id: impl Into<String>,
        max_nodes: usize,
    ) -> Result<ProtocolTree, MadeClientError> {
        let root_id = root_id.into();
        let mut pending = VecDeque::from([root_id]);
        let mut seen = BTreeSet::new();
        let mut instances = Vec::new();
        while let Some(protocol_id) = pending.pop_front() {
            if !seen.insert(protocol_id.clone()) {
                continue;
            }
            if seen.len() > max_nodes {
                return Err(MadeClientError::ProtocolViolation(format!(
                    "protocol tree exceeds the requested {max_nodes} node limit"
                )));
            }
            let instance = self.get_protocol(protocol_id).await?;
            for child in instance
                .child_groups
                .iter()
                .flat_map(|group| group.children.iter())
            {
                pending.push_back(child.child_id.clone());
            }
            instances.push(instance);
        }
        ProtocolTree::from_instances(instances)
    }
}
