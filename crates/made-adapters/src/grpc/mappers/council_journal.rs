//! Council cursor and record translation at the gRPC boundary.
use made_core::entities::CouncilJournalRecord;
use made_core::error::DomainError;
use made_core::value_objects::{
    CouncilJournalConsumer, CouncilJournalLease, CouncilJournalLeaseId, CouncilJournalPosition,
};
use made_proto::v1 as pb;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::ceremony_authorization::evidence_to_proto;

pub(crate) fn record_to_proto(
    record: &CouncilJournalRecord,
) -> Result<pb::CouncilJournalRecord, DomainError> {
    Ok(pb::CouncilJournalRecord {
        position: record.position().value(),
        event_json: serde_json::to_string(record.event()).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "cannot encode council journal event",
            }
        })?,
        authorization: record.authorization().map(evidence_to_proto).transpose()?,
    })
}
pub(crate) fn lease_to_proto(
    lease: &CouncilJournalLease,
) -> Result<pb::CouncilJournalLease, DomainError> {
    Ok(pb::CouncilJournalLease {
        consumer: lease.consumer().as_str().to_owned(),
        lease_id: lease.id().as_str().to_owned(),
        acknowledged_through: lease
            .acknowledged_through()
            .map(CouncilJournalPosition::value),
        expires_at: lease.expires_at().format(&Rfc3339).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "cannot encode council lease expiration",
            }
        })?,
    })
}
pub(crate) fn lease_from_proto(
    lease: Option<pb::CouncilJournalLease>,
) -> Result<CouncilJournalLease, DomainError> {
    let lease = lease.ok_or(DomainError::EmptyField {
        field: "council_journal_lease",
    })?;
    Ok(CouncilJournalLease::new(
        CouncilJournalConsumer::new(lease.consumer)?,
        CouncilJournalLeaseId::new(lease.lease_id)?,
        lease
            .acknowledged_through
            .map(CouncilJournalPosition::new)
            .transpose()?,
        OffsetDateTime::parse(&lease.expires_at, &Rfc3339).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "council lease expiration must be RFC3339",
            }
        })?,
    ))
}
