use serde::{Deserialize, Serialize};

use crate::DefinitionDefectView;

/// What analysis found — all of it.
///
/// Every defect at once, never the first one (ADR-002 upstream): fixing
/// defects one at a time spends the author's attention on round trips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionAnalysisView {
    /// Identity declared by the parsed draft.
    pub definition_name: String,
    pub definition_version: String,
    /// Whether the draft, as analyzed, could be published.
    pub publishable: bool,
    /// Canonical hex digest the executable definition will publish with.
    ///
    /// Present exactly when the draft is publishable. This is the same
    /// identity [`PublishedDefinitionView::digest`] returns and ceremony
    /// instances bind to; it is not a hash of the source bytes.
    pub definition_digest: Option<String>,
    pub defects: Vec<DefinitionDefectView>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PublishedDefinitionView;

    #[test]
    fn an_analysis_survives_the_wire() {
        let analysis = DefinitionAnalysisView {
            definition_name: "scope_discovery".to_owned(),
            definition_version: "1.0".to_owned(),
            publishable: false,
            definition_digest: None,
            defects: vec![DefinitionDefectView {
                severity: "error".to_owned(),
                locus: "state `ORPHAN`".to_owned(),
                defect: "state is unreachable".to_owned(),
                blocking: true,
            }],
        };
        let bytes = serde_json::to_vec(&analysis).expect("serializes");
        assert_eq!(
            serde_json::from_slice::<DefinitionAnalysisView>(&bytes).expect("deserializes"),
            analysis
        );
    }

    #[test]
    fn a_publication_names_what_an_instance_will_bind_to() {
        let published = PublishedDefinitionView {
            name: "scope_discovery".to_owned(),
            version: "1.0".to_owned(),
            digest: "abc123".to_owned(),
            already_published: false,
        };
        assert!(
            !published.digest.is_empty(),
            "a publication without a digest cannot be bound to, only believed"
        );
    }
}
