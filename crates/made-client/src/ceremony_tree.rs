use std::collections::{BTreeMap, BTreeSet};

use made_proto::v1::CeremonyInstanceState;

use crate::{CeremonyTreeNode, MadeClientError};

/// A non-authoritative hierarchy assembled from one bounded server result.
#[derive(Clone, Debug)]
pub struct CeremonyTree {
    roots: Vec<CeremonyTreeNode>,
}

impl CeremonyTree {
    pub fn from_instances(instances: Vec<CeremonyInstanceState>) -> Result<Self, MadeClientError> {
        let mut by_id = BTreeMap::new();
        let mut children = BTreeMap::<String, Vec<String>>::new();
        for instance in instances {
            if instance.ceremony_id.is_empty() {
                return Err(MadeClientError::ProtocolViolation(
                    "ceremony listing contains an empty id".to_owned(),
                ));
            }
            let id = instance.ceremony_id.clone();
            if by_id.insert(id.clone(), instance).is_some() {
                return Err(MadeClientError::ProtocolViolation(format!(
                    "ceremony listing repeats {id}"
                )));
            }
        }
        for (id, instance) in &by_id {
            if let Some(lineage) = &instance.lineage {
                if !lineage.parent_id.is_empty() && by_id.contains_key(&lineage.parent_id) {
                    children
                        .entry(lineage.parent_id.clone())
                        .or_default()
                        .push(id.clone());
                }
            }
        }
        for child_ids in children.values_mut() {
            child_ids.sort_by(|left, right| {
                let left_state = &by_id[left];
                let right_state = &by_id[right];
                let left_position = left_state
                    .lineage
                    .as_ref()
                    .map_or(0, |value| value.position);
                let right_position = right_state
                    .lineage
                    .as_ref()
                    .map_or(0, |value| value.position);
                left_position.cmp(&right_position).then(left.cmp(right))
            });
        }

        let mut roots: Vec<_> = by_id
            .iter()
            .filter(|(_, instance)| {
                instance.lineage.as_ref().is_none_or(|lineage| {
                    lineage.parent_id.is_empty() || !by_id.contains_key(&lineage.parent_id)
                })
            })
            .map(|(id, _)| id.clone())
            .collect();
        roots.sort();

        let mut visiting = BTreeSet::new();
        let mut built = BTreeSet::new();
        let roots = roots
            .iter()
            .map(|id| build_node(id, &by_id, &children, &mut visiting, &mut built))
            .collect::<Result<Vec<_>, _>>()?;
        if built.len() != by_id.len() {
            return Err(MadeClientError::ProtocolViolation(
                "ceremony lineage contains a cycle disconnected from every root".to_owned(),
            ));
        }
        Ok(Self { roots })
    }

    #[must_use]
    pub fn roots(&self) -> &[CeremonyTreeNode] {
        &self.roots
    }
}

fn build_node(
    id: &str,
    by_id: &BTreeMap<String, CeremonyInstanceState>,
    children: &BTreeMap<String, Vec<String>>,
    visiting: &mut BTreeSet<String>,
    built: &mut BTreeSet<String>,
) -> Result<CeremonyTreeNode, MadeClientError> {
    if !visiting.insert(id.to_owned()) {
        return Err(MadeClientError::ProtocolViolation(format!(
            "ceremony lineage contains a cycle at {id}"
        )));
    }
    let nested = children
        .get(id)
        .into_iter()
        .flatten()
        .map(|child| build_node(child, by_id, children, visiting, built))
        .collect::<Result<Vec<_>, _>>()?;
    visiting.remove(id);
    built.insert(id.to_owned());
    let instance = by_id.get(id).cloned().ok_or_else(|| {
        MadeClientError::ProtocolViolation(format!("missing ceremony tree node {id}"))
    })?;
    Ok(CeremonyTreeNode::new(instance, nested))
}
