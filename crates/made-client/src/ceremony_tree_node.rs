use made_proto::v1::CeremonyInstanceState;

/// One node in a hierarchy derived from public ceremony lineage.
#[derive(Clone, Debug)]
pub struct CeremonyTreeNode {
    instance: CeremonyInstanceState,
    children: Vec<CeremonyTreeNode>,
}

impl CeremonyTreeNode {
    pub(crate) fn new(instance: CeremonyInstanceState, children: Vec<Self>) -> Self {
        Self { instance, children }
    }

    #[must_use]
    pub fn instance(&self) -> &CeremonyInstanceState {
        &self.instance
    }

    #[must_use]
    pub fn children(&self) -> &[Self] {
        &self.children
    }
}
