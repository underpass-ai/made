use super::SemanticSupportSpec;

/// Declared evidence-grounding configuration for a step, before the
/// context-borne refs are resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceGroundingSpec {
    pub(crate) claims_field: String,
    pub(crate) refs_field: String,
    pub(crate) static_refs: Vec<String>,
    pub(crate) context_key: Option<String>,
    pub(crate) semantic: Option<SemanticSupportSpec>,
}
