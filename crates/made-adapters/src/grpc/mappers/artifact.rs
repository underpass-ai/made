use made_core::error::DomainError;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactChunkPage, ArtifactIdempotencyKey,
    ArtifactPageLimit, ArtifactRecord, ArtifactRetentionActor, ArtifactRetentionPolicy,
    ArtifactTombstone, ArtifactUploadId, ArtifactUploadStatus, BeginArtifactUpload,
    PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactId, ArtifactImportRef, ArtifactMediaType, ArtifactProvenance,
    ArtifactRef, ArtifactSizeBytes, ArtifactSourceKind, ExecutionOperationId, ExecutionReceiptId,
    StepClaimFence,
};
use made_proto::v1 as pb;
use prost_types::Timestamp;
use time::OffsetDateTime;

use super::offset_to_timestamp;

pub fn begin_artifact_upload_from_proto(
    request: pb::BeginArtifactUploadRequest,
) -> Result<BeginArtifactUpload, DomainError> {
    Ok(BeginArtifactUpload {
        requested_artifact_id: request
            .requested_artifact_id
            .map(ArtifactId::new)
            .transpose()?,
        expected_digest: ArtifactDigest::new(request.expected_digest)?,
        size_bytes: ArtifactSizeBytes::new(request.size_bytes),
        media_type: ArtifactMediaType::new(request.media_type)?,
        provenance: artifact_provenance_from_proto(required(
            request.provenance,
            "artifact_provenance",
        )?)?,
        idempotency_key: ArtifactIdempotencyKey::new(request.idempotency_key)?,
    })
}

pub fn put_artifact_chunk_from_proto(
    request: pb::PutArtifactChunkRequest,
) -> Result<PutArtifactChunk, DomainError> {
    Ok(PutArtifactChunk {
        upload_id: ArtifactUploadId::new(request.upload_id)?,
        offset: ArtifactByteOffset::new(request.offset),
        bytes: request.bytes,
        chunk_digest: ArtifactDigest::new(request.chunk_digest)?,
    })
}

pub fn read_artifact_chunk_from_proto(
    request: pb::ReadArtifactChunkRequest,
) -> Result<ReadArtifactChunk, DomainError> {
    let max_bytes = if request.max_bytes == 0 {
        ArtifactChunkLimit::DEFAULT
    } else {
        ArtifactChunkLimit::new(request.max_bytes)?
    };
    Ok(ReadArtifactChunk {
        artifact_id: ArtifactId::new(request.artifact_id)?,
        offset: ArtifactByteOffset::new(request.offset),
        max_bytes,
    })
}

pub fn artifact_page_limit_from_proto(limit: u32) -> Result<ArtifactPageLimit, DomainError> {
    if limit == 0 {
        return Ok(ArtifactPageLimit::default());
    }
    let limit = u16::try_from(limit).map_err(|_| DomainError::OutOfRange {
        field: "artifact_page_limit",
        value: f64::from(limit),
        min: 1.0,
        max: f64::from(u16::MAX),
    })?;
    ArtifactPageLimit::new(limit)
}

pub fn tombstone_artifact_from_proto(
    request: pb::TombstoneArtifactRequest,
) -> Result<TombstoneArtifact, DomainError> {
    Ok(TombstoneArtifact {
        artifact_id: ArtifactId::new(request.artifact_id)?,
        actor: ArtifactRetentionActor::new(request.actor)?,
        policy: ArtifactRetentionPolicy::new(request.policy)?,
        retired_at: timestamp_to_offset(required(request.retired_at, "retired_at")?)?,
    })
}

#[must_use]
pub fn artifact_upload_status_to_proto(status: &ArtifactUploadStatus) -> pb::ArtifactUploadStatus {
    pb::ArtifactUploadStatus {
        upload_id: status.upload_id.as_str().to_owned(),
        next_offset: status.next_offset.get(),
        chunk_limit: status.chunk_limit.get(),
    }
}

#[must_use]
pub fn artifact_ref_to_proto(artifact: &ArtifactRef) -> pb::ArtifactReference {
    pb::ArtifactReference {
        artifact_id: artifact.artifact_id().as_str().to_owned(),
        digest: artifact.digest().as_str().to_owned(),
        size_bytes: artifact.size_bytes().get(),
        media_type: artifact.media_type().as_str().to_owned(),
        provenance: Some(artifact_provenance_to_proto(artifact.provenance())),
    }
}

#[must_use]
pub fn artifact_record_to_proto(record: &ArtifactRecord) -> pb::ArtifactRecord {
    pb::ArtifactRecord {
        artifact: Some(artifact_ref_to_proto(&record.artifact)),
        tombstone: record.tombstone.as_ref().map(artifact_tombstone_to_proto),
    }
}

#[must_use]
pub fn artifact_chunk_to_proto(chunk: ArtifactChunkPage) -> pb::ReadArtifactChunkResponse {
    let eof = chunk.is_complete();
    pb::ReadArtifactChunkResponse {
        bytes: chunk.bytes,
        next_offset: chunk.next_offset.get(),
        chunk_digest: chunk.chunk_digest.as_str().to_owned(),
        eof,
    }
}

#[must_use]
pub fn artifact_tombstone_to_proto(tombstone: &ArtifactTombstone) -> pb::ArtifactTombstoneRecord {
    pb::ArtifactTombstoneRecord {
        actor: tombstone.actor.as_str().to_owned(),
        policy: tombstone.policy.as_str().to_owned(),
        retired_at: Some(offset_to_timestamp(tombstone.retired_at)),
        digest: tombstone.digest.as_str().to_owned(),
    }
}

fn artifact_provenance_from_proto(
    provenance: pb::ArtifactProvenance,
) -> Result<ArtifactProvenance, DomainError> {
    let observed_at = timestamp_to_offset(required(provenance.observed_at, "observed_at")?)?;
    let source_kind = pb::ArtifactSourceKind::try_from(provenance.source_kind).map_err(|_| {
        DomainError::InvalidCharacters {
            field: "artifact_source_kind",
        }
    })?;
    match source_kind {
        pb::ArtifactSourceKind::ExternalExecution
        | pb::ArtifactSourceKind::Fixture
        | pb::ArtifactSourceKind::NoOp => {
            if provenance.import_ref.is_some() {
                return Err(DomainError::InvariantViolated {
                    reason: "execution artifact provenance cannot carry an import reference",
                });
            }
            ArtifactProvenance::execution(
                source_kind_from_proto(source_kind)?,
                ExecutionReceiptId::new(required(
                    provenance.execution_receipt_id,
                    "execution_receipt_id",
                )?)?,
                ExecutionOperationId::new(required(provenance.operation_id, "operation_id")?)?,
                StepClaimFence::new(required(
                    provenance.accepted_claim_fence,
                    "accepted_claim_fence",
                )?)?,
                observed_at,
            )
        }
        pb::ArtifactSourceKind::GeneratedReport => {
            ensure_execution_fields_absent(&provenance)?;
            if provenance.import_ref.is_some() {
                return Err(DomainError::InvariantViolated {
                    reason: "generated report provenance cannot carry an import reference",
                });
            }
            Ok(ArtifactProvenance::generated_report(observed_at))
        }
        pb::ArtifactSourceKind::Imported => {
            ensure_execution_fields_absent(&provenance)?;
            Ok(ArtifactProvenance::imported(
                ArtifactImportRef::new(required(provenance.import_ref, "import_ref")?)?,
                observed_at,
            ))
        }
        pb::ArtifactSourceKind::Unspecified => Err(DomainError::EmptyField {
            field: "artifact_source_kind",
        }),
    }
}

fn ensure_execution_fields_absent(provenance: &pb::ArtifactProvenance) -> Result<(), DomainError> {
    if provenance.execution_receipt_id.is_some()
        || provenance.operation_id.is_some()
        || provenance.accepted_claim_fence.is_some()
    {
        return Err(DomainError::InvariantViolated {
            reason: "non-execution artifact provenance cannot carry execution identity",
        });
    }
    Ok(())
}

fn source_kind_from_proto(kind: pb::ArtifactSourceKind) -> Result<ArtifactSourceKind, DomainError> {
    match kind {
        pb::ArtifactSourceKind::ExternalExecution => Ok(ArtifactSourceKind::ExternalExecution),
        pb::ArtifactSourceKind::Fixture => Ok(ArtifactSourceKind::Fixture),
        pb::ArtifactSourceKind::NoOp => Ok(ArtifactSourceKind::NoOp),
        _ => Err(DomainError::InvariantViolated {
            reason: "artifact provenance source is not an execution source",
        }),
    }
}

fn artifact_provenance_to_proto(provenance: &ArtifactProvenance) -> pb::ArtifactProvenance {
    pb::ArtifactProvenance {
        source_kind: match provenance.source_kind() {
            ArtifactSourceKind::ExternalExecution => pb::ArtifactSourceKind::ExternalExecution,
            ArtifactSourceKind::Fixture => pb::ArtifactSourceKind::Fixture,
            ArtifactSourceKind::NoOp => pb::ArtifactSourceKind::NoOp,
            ArtifactSourceKind::GeneratedReport => pb::ArtifactSourceKind::GeneratedReport,
            ArtifactSourceKind::Imported => pb::ArtifactSourceKind::Imported,
        }
        .into(),
        execution_receipt_id: provenance.execution_receipt_id().map(ToString::to_string),
        operation_id: provenance.operation_id().map(ToString::to_string),
        accepted_claim_fence: provenance
            .accepted_claim_fence()
            .map(|fence| fence.as_str().to_owned()),
        observed_at: Some(offset_to_timestamp(provenance.observed_at())),
        import_ref: provenance
            .import_ref()
            .map(|import_ref| String::from(import_ref.clone())),
    }
}

fn timestamp_to_offset(timestamp: Timestamp) -> Result<OffsetDateTime, DomainError> {
    if !(0..1_000_000_000).contains(&timestamp.nanos) {
        return Err(DomainError::OutOfRange {
            field: "timestamp.nanos",
            value: f64::from(timestamp.nanos),
            min: 0.0,
            max: 999_999_999.0,
        });
    }
    let nanos = i128::from(timestamp.seconds)
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(i128::from(timestamp.nanos)))
        .ok_or(DomainError::OutOfRange {
            field: "timestamp",
            value: timestamp.seconds as f64,
            min: i64::MIN as f64,
            max: i64::MAX as f64,
        })?;
    OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| DomainError::OutOfRange {
        field: "timestamp",
        value: timestamp.seconds as f64,
        min: i64::MIN as f64,
        max: i64::MAX as f64,
    })
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, DomainError> {
    value.ok_or(DomainError::EmptyField { field })
}
