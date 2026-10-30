use crate::entities::protocol_events::ContextWritten;
use crate::entities::ProtocolInstance;

impl ProtocolInstance {
    pub(super) fn apply_context_written(&mut self, written: &ContextWritten) {
        self.context = self
            .context
            .clone()
            .with_patch(&written.patch)
            .expect("a sealed context patch must remain valid");
        self.updated_at = written.written_at;
    }
}
