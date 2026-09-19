use serde::{Deserialize, Serialize};

use crate::value_objects::{ArtifactRef, AuthorizationEvidence};

use super::ArtifactTombstone;

/// Metadata and optional retirement audit state for one artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact: ArtifactRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tombstone: Option<ArtifactTombstone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization: Option<AuthorizationEvidence>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_record_and_tombstone_without_authorization_keep_exact_json() {
        let historical = concat!(
            r#"{"artifact":{"artifact_id":"artifact-legacy","digest":"sha256:"#,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            r#"","size_bytes":1,"media_type":"text/plain","provenance":{"source_kind":"generated_report","observed_at":"1970-01-01T00:00:00Z"}},"tombstone":{"actor":"host:legacy","policy":"legacy-retention","retired_at":"1970-01-01T00:00:00Z","digest":"sha256:"#,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            r#""}}"#,
        );
        let record: ArtifactRecord = serde_json::from_str(historical).unwrap();

        assert!(record.authorization.is_none());
        assert!(record.tombstone.as_ref().unwrap().authorization.is_none());
        record.artifact.validate().unwrap();
        assert_eq!(serde_json::to_string(&record).unwrap(), historical);
    }
}
