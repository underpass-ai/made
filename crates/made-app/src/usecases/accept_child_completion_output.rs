use made_core::entities::ProtocolInstance;
use made_core::value_objects::ChildCompletionRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptChildCompletionOutput {
    parent: ProtocolInstance,
    completion: ChildCompletionRef,
}

impl AcceptChildCompletionOutput {
    #[must_use]
    pub fn new(parent: ProtocolInstance, completion: ChildCompletionRef) -> Self {
        Self { parent, completion }
    }

    #[must_use]
    pub fn parent(&self) -> &ProtocolInstance {
        &self.parent
    }

    #[must_use]
    pub fn completion(&self) -> &ChildCompletionRef {
        &self.completion
    }

    #[must_use]
    pub fn into_parts(self) -> (ProtocolInstance, ChildCompletionRef) {
        (self.parent, self.completion)
    }
}
