use std::path::{Path, PathBuf};

use made_proto::v1::ReadArtifactChunkRequest;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{MadeClient, MadeClientError};

const READ_LIMIT: u32 = 1024 * 1024;

impl MadeClient {
    pub async fn export_artifact(
        &self,
        artifact_id: &str,
        destination: &Path,
        overwrite: bool,
    ) -> Result<(), MadeClientError> {
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| MadeClientError::io(parent, error))?;
        let record = self.get_artifact(artifact_id).await?;
        let reference = record.artifact.ok_or_else(|| {
            MadeClientError::ProtocolViolation("artifact record has no reference".to_owned())
        })?;
        if reference.artifact_id != artifact_id {
            return Err(MadeClientError::ProtocolViolation(format!(
                "requested artifact {artifact_id}, got {}",
                reference.artifact_id
            )));
        }
        let temporary = temporary_path(destination);
        let result = self
            .write_verified_artifact(
                &reference.digest,
                reference.size_bytes,
                &temporary,
                artifact_id,
            )
            .await
            .and_then(|()| install_file(&temporary, destination, overwrite));
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result
    }

    async fn write_verified_artifact(
        &self,
        expected_digest: &str,
        expected_size: u64,
        temporary: &Path,
        artifact_id: &str,
    ) -> Result<(), MadeClientError> {
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(temporary)
            .await
            .map_err(|error| MadeClientError::io(temporary, error))?;
        let mut aggregate = Sha256::new();
        let mut offset = 0_u64;
        loop {
            let response = self
                .rpc()
                .read_artifact_chunk(ReadArtifactChunkRequest {
                    artifact_id: artifact_id.to_owned(),
                    offset,
                    max_bytes: READ_LIMIT,
                })
                .await
                .map_err(MadeClientError::from_status)?
                .into_inner();
            if response.bytes.is_empty() && !response.eof {
                return Err(MadeClientError::ProtocolViolation(
                    "artifact read made no progress before EOF".to_owned(),
                ));
            }
            let observed_chunk_digest = sha256_digest(&response.bytes);
            if observed_chunk_digest != response.chunk_digest {
                return Err(MadeClientError::ArtifactIntegrityMismatch {
                    expected: response.chunk_digest,
                    observed: observed_chunk_digest,
                });
            }
            let next_offset = offset
                .checked_add(response.bytes.len() as u64)
                .ok_or_else(|| {
                    MadeClientError::ProtocolViolation("artifact offset overflow".to_owned())
                })?;
            if response.next_offset != next_offset {
                return Err(MadeClientError::ProtocolViolation(format!(
                    "artifact next offset {}, expected {next_offset}",
                    response.next_offset
                )));
            }
            file.write_all(&response.bytes)
                .await
                .map_err(|error| MadeClientError::io(temporary, error))?;
            aggregate.update(&response.bytes);
            offset = next_offset;
            if response.eof {
                break;
            }
        }
        if offset != expected_size {
            return Err(MadeClientError::ArtifactSizeMismatch {
                expected: expected_size,
                observed: offset,
            });
        }
        let observed = format!("sha256:{:x}", aggregate.finalize());
        if observed != expected_digest {
            return Err(MadeClientError::ArtifactIntegrityMismatch {
                expected: expected_digest.to_owned(),
                observed,
            });
        }
        file.sync_all()
            .await
            .map_err(|error| MadeClientError::io(temporary, error))
    }
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn temporary_path(destination: &Path) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact");
    parent.join(format!(".{name}.{}.part", Uuid::new_v4()))
}

fn install_file(
    temporary: &Path,
    destination: &Path,
    overwrite: bool,
) -> Result<(), MadeClientError> {
    if overwrite {
        std::fs::rename(temporary, destination)
            .map_err(|error| MadeClientError::io(destination, error))
    } else {
        std::fs::hard_link(temporary, destination)
            .map_err(|error| MadeClientError::io(destination, error))?;
        std::fs::remove_file(temporary).map_err(|error| MadeClientError::io(temporary, error))
    }
}
