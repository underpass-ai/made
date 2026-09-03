/// Declared semantic-support configuration for a step, before the
/// context-borne evidence bodies are resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticSupportSpec {
    pub(crate) min_confidence: Option<u8>,
    pub(crate) bodies_context_key: Option<String>,
}
