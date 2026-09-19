use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use made_core::ports::ArtifactStoreError;
use made_core::value_objects::ArtifactId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SCOPE: &str = "made:artifact-list:v1";

/// Opaque, scoped continuation cursor for the public artifact listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactCursor(String);

impl ArtifactCursor {
    #[must_use]
    pub fn after(artifact_id: &ArtifactId) -> Self {
        let payload = format!("{SCOPE}\n{}", artifact_id.as_str());
        let checksum = format!("{:x}", Sha256::digest(payload.as_bytes()));
        Self(URL_SAFE_NO_PAD.encode(format!("{payload}\n{checksum}")))
    }

    pub fn parse(encoded: impl Into<String>) -> Result<Self, ArtifactStoreError> {
        let encoded = encoded.into();
        Self::decode_encoded(&encoded)?;
        Ok(Self(encoded))
    }

    pub fn decode(&self) -> Result<ArtifactId, ArtifactStoreError> {
        Self::decode_encoded(&self.0)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn decode_encoded(encoded: &str) -> Result<ArtifactId, ArtifactStoreError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| ArtifactStoreError::InvalidCursor)?;
        let decoded = std::str::from_utf8(&bytes).map_err(|_| ArtifactStoreError::InvalidCursor)?;
        let mut fields = decoded.split('\n');
        let (Some(scope), Some(artifact_id), Some(checksum), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(ArtifactStoreError::InvalidCursor);
        };
        if scope != SCOPE {
            return Err(ArtifactStoreError::InvalidCursor);
        }
        let payload = format!("{scope}\n{artifact_id}");
        let expected = format!("{:x}", Sha256::digest(payload.as_bytes()));
        if checksum != expected {
            return Err(ArtifactStoreError::InvalidCursor);
        }
        ArtifactId::new(artifact_id).map_err(ArtifactStoreError::from)
    }
}

#[cfg(test)]
mod tests {
    use made_core::ports::ArtifactStoreError;
    use made_core::value_objects::ArtifactId;

    use super::ArtifactCursor;

    #[test]
    fn cursor_round_trips_and_rejects_tamper() {
        let artifact_id = ArtifactId::new("artifact-page-anchor").unwrap();
        let cursor = ArtifactCursor::after(&artifact_id);
        assert_eq!(cursor.decode().unwrap(), artifact_id);

        let mut tampered = cursor.as_str().as_bytes().to_vec();
        let last = tampered.last_mut().unwrap();
        *last = if *last == b'A' { b'B' } else { b'A' };
        assert_eq!(
            ArtifactCursor::parse(String::from_utf8(tampered).unwrap()),
            Err(ArtifactStoreError::InvalidCursor)
        );
    }

    #[test]
    fn cursor_rejects_a_different_scope() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        use sha2::{Digest, Sha256};

        let payload = "made:other-list:v1\nartifact-page-anchor";
        let checksum = format!("{:x}", Sha256::digest(payload.as_bytes()));
        let encoded = URL_SAFE_NO_PAD.encode(format!("{payload}\n{checksum}"));
        assert_eq!(
            ArtifactCursor::parse(encoded),
            Err(ArtifactStoreError::InvalidCursor)
        );
    }
}
