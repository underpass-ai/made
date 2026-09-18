use made_core::entities::CeremonyInstance;
use made_core::value_objects::ChildCompletionRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptChildCompletionOutput {
    parent: CeremonyInstance,
    completion: ChildCompletionRef,
}

impl AcceptChildCompletionOutput {
    #[must_use]
    pub fn new(parent: CeremonyInstance, completion: ChildCompletionRef) -> Self {
        Self { parent, completion }
    }

    #[must_use]
    pub fn parent(&self) -> &CeremonyInstance {
        &self.parent
    }

    #[must_use]
    pub fn completion(&self) -> &ChildCompletionRef {
        &self.completion
    }

    #[must_use]
    pub fn into_parts(self) -> (CeremonyInstance, ChildCompletionRef) {
        (self.parent, self.completion)
    }
}
