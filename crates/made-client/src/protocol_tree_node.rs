use made_proto::v1::ProtocolInstanceState;

/// One node in a hierarchy derived from public protocol lineage.
#[derive(Clone, Debug)]
pub struct ProtocolTreeNode {
    instance: ProtocolInstanceState,
    children: Vec<ProtocolTreeNode>,
}

impl ProtocolTreeNode {
    pub(crate) fn new(instance: ProtocolInstanceState, children: Vec<Self>) -> Self {
        Self { instance, children }
    }

    #[must_use]
    pub fn instance(&self) -> &ProtocolInstanceState {
        &self.instance
    }

    #[must_use]
    pub fn children(&self) -> &[Self] {
        &self.children
    }
}
