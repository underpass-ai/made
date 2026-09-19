use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::MadeClientError;

/// The only durable local console state: a per-ceremony G6 resume coordinate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProgressCheckpoint {
    ceremony_id: String,
    after_sequence: u64,
}

impl ProgressCheckpoint {
    pub fn new(ceremony_id: impl Into<String>, after_sequence: u64) -> Self {
        Self {
            ceremony_id: ceremony_id.into(),
            after_sequence,
        }
    }

    #[must_use]
    pub fn ceremony_id(&self) -> &str {
        &self.ceremony_id
    }

    #[must_use]
    pub fn after_sequence(&self) -> u64 {
        self.after_sequence
    }

    pub async fn load(path: &Path, expected_ceremony_id: &str) -> Result<Self, MadeClientError> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|error| MadeClientError::io(path, error))?;
        let checkpoint: Self = serde_json::from_slice(&bytes)
            .map_err(|error| MadeClientError::CursorCheckpointCorrupt(error.to_string()))?;
        if checkpoint.ceremony_id != expected_ceremony_id {
            return Err(MadeClientError::CursorScopeMismatch {
                expected: expected_ceremony_id.to_owned(),
                actual: checkpoint.ceremony_id,
            });
        }
        Ok(checkpoint)
    }

    pub async fn save(&self, path: &Path) -> Result<(), MadeClientError> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| MadeClientError::io(parent, error))?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("made-cursor");
        let temporary = parent.join(format!(".{name}.{}.part", Uuid::new_v4()));
        let payload = serde_json::to_vec(self)
            .map_err(|error| MadeClientError::CursorCheckpointCorrupt(error.to_string()))?;
        let result = async {
            let mut file = tokio::fs::File::create(&temporary)
                .await
                .map_err(|error| MadeClientError::io(&temporary, error))?;
            file.write_all(&payload)
                .await
                .map_err(|error| MadeClientError::io(&temporary, error))?;
            file.sync_all()
                .await
                .map_err(|error| MadeClientError::io(&temporary, error))?;
            drop(file);
            tokio::fs::rename(&temporary, path)
                .await
                .map_err(|error| MadeClientError::io(path, error))
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result
    }
}
