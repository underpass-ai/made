//! [`AuditRecord`] — one tamper-evident fact in a ceremony's journal.
//!
//! Publishing a typed event does not make an audit. A message can be
//! lost, duplicated, published after persistence failed, or stored
//! without a verifiable order. A record binds a fact to its position
//! and to every fact before it, so removing, altering, reordering or
//! inserting one is detectable without trusting the store that held it.
//!
//! Since schema version 2 the record carries the event itself, payload
//! included, and the digest covers it: the chain now verifies what
//! happened, not only that something did.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::entities::{AuditFact, CeremonyEvent};
use crate::error::DomainError;
use crate::value_objects::{
    AuditActor, AuditEventType, AuditRecordHash, AuditSequence, CeremonyId, CeremonyName,
    CeremonyVersion, EventId, EventSchemaVersion,
};

mod audit_record_wire;

use audit_record_wire::AuditRecordWire;

/// Domain separator of the first record shape, which carried no
/// payload. Records sealed under it keep verifying byte for byte.
const CANONICAL_SCHEME_V1: &[u8] = b"underpass.made.audit-record.v1";

/// Domain separator of the current shape. A digest computed under a
/// different scheme can never collide with one computed under this
/// version, and bumping it is how the algorithm is versioned.
const CANONICAL_SCHEME_V2: &[u8] = b"underpass.made.audit-record.v2";

/// The shape records were sealed in before the event travelled inside
/// them.
const LEGACY_SCHEMA_VERSION: u32 = 1;

/// Version of the record's field set. It participates in the digest,
/// so records written under different shapes cannot be silently mixed
/// into one chain.
pub const AUDIT_RECORD_SCHEMA_VERSION: u32 = 2;

/// One fact in a ceremony's audit journal.
///
/// Immutable by construction: the digest is computed once, from the
/// content and the previous record's digest, and every accessor is
/// read-only. There is no setter that could leave the digest stale.
///
/// # Two shapes in one type
///
/// Every record this code seals is schema version 2 and carries its
/// event and the event's payload schema version. Both are `Option` for
/// one reason: records written under schema version 1 — the journals
/// of stores from v0.3.0 and earlier — carry no payload, and they must
/// keep deserializing and verifying exactly as they did until the
/// copy-on-write migration imports them. A version-1 record therefore
/// has neither; a version-2 record without its event is not intact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AuditRecordWire")]
pub struct AuditRecord {
    event_id: EventId,
    event_type: AuditEventType,
    schema_version: u32,
    ceremony_id: CeremonyId,
    definition_name: CeremonyName,
    definition_version: CeremonyVersion,
    sequence: AuditSequence,
    #[serde(with = "time::serde::rfc3339")]
    occurred_at: OffsetDateTime,
    actor: AuditActor,
    correlation_id: Option<EventId>,
    causation_id: Option<EventId>,
    trace_id: Option<String>,
    event_schema_version: Option<EventSchemaVersion>,
    event: Option<CeremonyEvent>,
    previous_record_hash: Option<AuditRecordHash>,
    record_hash: AuditRecordHash,
}

impl AuditRecord {
    /// Seal a fact as the first record of a ceremony's journal.
    pub fn first(fact: AuditFact) -> Result<Self, DomainError> {
        Self::seal(fact, AuditSequence::FIRST, None)
    }

    /// Seal a fact as the record that follows `previous`.
    ///
    /// The successor's position and previous digest are taken from the
    /// predecessor rather than supplied, so a caller cannot append a
    /// record that claims to follow something it does not.
    pub fn following(fact: AuditFact, previous: &Self) -> Result<Self, DomainError> {
        if fact.ceremony_id != previous.ceremony_id {
            return Err(DomainError::InvariantViolated {
                reason: "an audit record must belong to the same ceremony as its predecessor",
            });
        }
        Self::seal(fact, previous.sequence.next(), Some(previous.record_hash))
    }

    fn seal(
        fact: AuditFact,
        sequence: AuditSequence,
        previous_record_hash: Option<AuditRecordHash>,
    ) -> Result<Self, DomainError> {
        let trace_id = fact.trace.map(|trace| trace.trace_id().to_owned());
        let mut record = Self {
            event_id: fact.event_id,
            event_type: fact.event.event_type(),
            schema_version: AUDIT_RECORD_SCHEMA_VERSION,
            ceremony_id: fact.ceremony_id,
            definition_name: fact.definition_name,
            definition_version: fact.definition_version,
            sequence,
            occurred_at: fact.occurred_at,
            actor: fact.actor,
            correlation_id: fact.correlation_id,
            causation_id: fact.causation_id,
            trace_id,
            event_schema_version: Some(fact.event.schema_version()),
            event: Some(fact.event),
            previous_record_hash,
            record_hash: AuditRecordHash::from_bytes([0; 32]),
        };
        record.record_hash = record.compute_hash()?;
        Ok(record)
    }

    #[must_use]
    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    #[must_use]
    pub fn event_type(&self) -> AuditEventType {
        self.event_type
    }

    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub fn definition_name(&self) -> &CeremonyName {
        &self.definition_name
    }

    #[must_use]
    pub fn definition_version(&self) -> &CeremonyVersion {
        &self.definition_version
    }

    #[must_use]
    pub fn sequence(&self) -> AuditSequence {
        self.sequence
    }

    #[must_use]
    pub fn occurred_at(&self) -> OffsetDateTime {
        self.occurred_at
    }

    #[must_use]
    pub fn actor(&self) -> &AuditActor {
        &self.actor
    }

    #[must_use]
    pub fn correlation_id(&self) -> Option<&EventId> {
        self.correlation_id.as_ref()
    }

    #[must_use]
    pub fn causation_id(&self) -> Option<&EventId> {
        self.causation_id.as_ref()
    }

    #[must_use]
    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    /// The shape the sealed event's payload was written in.
    ///
    /// Absent only on records sealed under schema version 1.
    #[must_use]
    pub fn event_schema_version(&self) -> Option<EventSchemaVersion> {
        self.event_schema_version
    }

    /// The event this record seals, payload included.
    ///
    /// Absent only on records sealed under schema version 1, which
    /// recorded that something happened and not what.
    #[must_use]
    pub fn event(&self) -> Option<&CeremonyEvent> {
        self.event.as_ref()
    }

    #[must_use]
    pub fn previous_record_hash(&self) -> Option<AuditRecordHash> {
        self.previous_record_hash
    }

    #[must_use]
    pub fn record_hash(&self) -> AuditRecordHash {
        self.record_hash
    }

    /// Whether the digest still matches the content.
    ///
    /// A record that fails this was altered after it was sealed, or was
    /// never sealed by this implementation — which includes a
    /// version-2 record that has lost its event, and a version-1 record
    /// that has gained one.
    pub fn digest_is_intact(&self) -> Result<bool, DomainError> {
        if !self.has_canonical_shape() {
            return Ok(false);
        }
        Ok(self.compute_hash()? == self.record_hash)
    }

    /// Whether this record legitimately continues `previous`.
    ///
    /// Checks the three ways a chain breaks at a join: the wrong
    /// ceremony, a position that is not the immediate successor, and a
    /// previous digest that does not match the record it names.
    #[must_use]
    pub fn continues(&self, previous: &Self) -> bool {
        self.ceremony_id == previous.ceremony_id
            && self.sequence.follows(previous.sequence)
            && self.previous_record_hash == Some(previous.record_hash)
    }

    /// Whether the fields present match what the schema version seals.
    fn has_canonical_shape(&self) -> bool {
        match self.schema_version {
            LEGACY_SCHEMA_VERSION => self.event.is_none() && self.event_schema_version.is_none(),
            AUDIT_RECORD_SCHEMA_VERSION => {
                self.event.is_some() && self.event_schema_version.is_some()
            }
            _ => false,
        }
    }

    /// The preimage of the digest.
    ///
    /// Under schema version 1 it is, byte for byte, what that version
    /// wrote. Under version 2 the scheme changes and the event's payload
    /// version and canonical JSON follow the envelope fields, so a
    /// payload edit — or a payload reread under another version — is a
    /// different digest.
    fn compute_hash(&self) -> Result<AuditRecordHash, DomainError> {
        let occurred_at =
            self.occurred_at
                .format(&Rfc3339)
                .map_err(|_| DomainError::InvariantViolated {
                    reason: "audit record timestamp cannot be rendered canonically",
                })?;

        let mut canonical = Vec::new();
        match self.schema_version {
            LEGACY_SCHEMA_VERSION => canonical.extend_from_slice(CANONICAL_SCHEME_V1),
            AUDIT_RECORD_SCHEMA_VERSION => canonical.extend_from_slice(CANONICAL_SCHEME_V2),
            _ => {
                return Err(DomainError::InvariantViolated {
                    reason: "audit record schema version has no canonical form",
                })
            }
        }
        canonical.extend_from_slice(&self.schema_version.to_be_bytes());
        write_field(&mut canonical, self.event_id.as_str().as_bytes());
        write_field(&mut canonical, self.event_type.as_str().as_bytes());
        write_field(&mut canonical, self.ceremony_id.as_str().as_bytes());
        write_field(&mut canonical, self.definition_name.as_str().as_bytes());
        write_field(&mut canonical, self.definition_version.as_str().as_bytes());
        canonical.extend_from_slice(&self.sequence.value().to_be_bytes());
        write_field(&mut canonical, occurred_at.as_bytes());
        write_field(&mut canonical, self.actor.actor_id().as_bytes());
        write_field(&mut canonical, self.actor.kind().as_str().as_bytes());
        write_optional(
            &mut canonical,
            self.actor.role_id().map(|role| role.as_str().as_bytes()),
        );
        write_optional(
            &mut canonical,
            self.correlation_id
                .as_ref()
                .map(|id| id.as_str().as_bytes()),
        );
        write_optional(
            &mut canonical,
            self.causation_id.as_ref().map(|id| id.as_str().as_bytes()),
        );
        write_optional(&mut canonical, self.trace_id.as_deref().map(str::as_bytes));
        write_optional(
            &mut canonical,
            self.previous_record_hash
                .as_ref()
                .map(|hash| hash.as_bytes().as_slice()),
        );

        if self.schema_version == AUDIT_RECORD_SCHEMA_VERSION {
            let (Some(version), Some(event)) = (self.event_schema_version, self.event.as_ref())
            else {
                return Err(DomainError::InvariantViolated {
                    reason: "a version-2 audit record must carry its event",
                });
            };
            canonical.extend_from_slice(&version.get().to_be_bytes());
            let payload =
                serde_json::to_vec(event).map_err(|_| DomainError::InvariantViolated {
                    reason: "ceremony event cannot be rendered canonically",
                })?;
            write_field(&mut canonical, &payload);
        }

        let digest = Sha256::digest(&canonical);
        Ok(AuditRecordHash::from_bytes(digest.into()))
    }
}

/// Length-prefixed so no field value can be mistaken for a boundary
/// between fields, whatever bytes it contains.
fn write_field(buffer: &mut Vec<u8>, value: &[u8]) {
    buffer.extend_from_slice(&(value.len() as u64).to_be_bytes());
    buffer.extend_from_slice(value);
}

/// An absent field is one byte; a present one is tagged before its
/// length, so absence and emptiness are distinguishable.
fn write_optional(buffer: &mut Vec<u8>, value: Option<&[u8]>) {
    match value {
        None => buffer.push(0),
        Some(value) => {
            buffer.push(1);
            write_field(buffer, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::entities::ceremony_events::{
        CeremonyCompleted, CeremonyInstanceStarted, StepCompleted, StepFailed, StepStarted,
    };
    use crate::value_objects::{
        AuditActorKind, CeremonyContext, IdempotencyKey, LeaseOwnerId, RoleId, StateId,
        StateIteration, StepAttempt, StepErrorMessage, StepId, StepIteration, StepLease,
        StepOutput, StepResult,
    };
    use serde_json::Value;
    use time::macros::datetime;

    /// One way of editing a stored record, applied to its JSON.
    type Tampering = Box<dyn FnOnce(&mut Value)>;

    const AT: OffsetDateTime = datetime!(2026-07-29 09:00:00 UTC);

    fn event(event_type: AuditEventType) -> CeremonyEvent {
        let step_id = StepId::new("draft").unwrap();
        let author = RoleId::new("author").unwrap();
        match event_type {
            AuditEventType::CeremonyInstanceStarted => {
                CeremonyEvent::CeremonyInstanceStarted(CeremonyInstanceStarted {
                    ceremony_id: CeremonyId::new("ceremony-1").unwrap(),
                    definition_name: CeremonyName::new("planning_ceremony").unwrap(),
                    definition_version: CeremonyVersion::v1(),
                    initial_state: StateId::new("OPEN").unwrap(),
                    step_ids: BTreeSet::from([step_id]),
                    context: CeremonyContext::empty(),
                    bound_definition: None,
                    lineage: None,
                    ceremony_deadline: None,
                    state_deadline: None,
                    created_at: AT,
                })
            }
            AuditEventType::StepStarted => CeremonyEvent::StepStarted(StepStarted {
                state_visit: None,
                step_id,
                state_iteration: Some(StateIteration::FIRST),
                iteration: StepIteration::FIRST,
                attempt: StepAttempt::FIRST,
                lease: StepLease::new(
                    LeaseOwnerId::new("host-1").unwrap(),
                    IdempotencyKey::new("key-1").unwrap(),
                    AT,
                    datetime!(2026-07-29 10:00:00 UTC),
                )
                .unwrap(),
                started_by: author,
                role_from: None,
                sealed_role: None,
                deadline: None,
                started_at: AT,
            }),
            AuditEventType::StepCompleted => CeremonyEvent::StepCompleted(StepCompleted {
                state_visit: None,
                step_id,
                state_iteration: Some(StateIteration::FIRST),
                iteration: StepIteration::FIRST,
                attempt: StepAttempt::FIRST,
                result: StepResult::completed(StepOutput::empty()).unwrap(),
                next_iteration: None,
                finished_by: author,
                finished_at: AT,
            }),
            AuditEventType::StepFailed => CeremonyEvent::StepFailed(StepFailed {
                state_visit: None,
                step_id,
                state_iteration: Some(StateIteration::FIRST),
                iteration: StepIteration::FIRST,
                attempt: StepAttempt::FIRST,
                result: StepResult::failed(StepErrorMessage::new("boom").unwrap()).unwrap(),
                finished_by: author,
                finished_at: AT,
            }),
            AuditEventType::CeremonyCompleted => {
                CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
                    final_state: StateId::new("DONE").unwrap(),
                    completed_at: AT,
                })
            }
            other => panic!("no sample event for {other:?}"),
        }
    }

    fn fact(event_id: &str, event_type: AuditEventType) -> AuditFact {
        AuditFact {
            event_id: EventId::new(event_id).unwrap(),
            event: event(event_type),
            ceremony_id: CeremonyId::new("ceremony-1").unwrap(),
            definition_name: CeremonyName::new("planning_ceremony").unwrap(),
            definition_version: CeremonyVersion::v1(),
            occurred_at: AT,
            actor: AuditActor::new("engineer-1", AuditActorKind::Human, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        }
    }

    fn chain_of_three() -> [AuditRecord; 3] {
        let first =
            AuditRecord::first(fact("e1", AuditEventType::CeremonyInstanceStarted)).unwrap();
        let second =
            AuditRecord::following(fact("e2", AuditEventType::StepStarted), &first).unwrap();
        let third =
            AuditRecord::following(fact("e3", AuditEventType::StepCompleted), &second).unwrap();
        [first, second, third]
    }

    /// Round-trip through JSON so a field can be altered exactly the way
    /// someone editing the stored record would.
    fn tampered(record: &AuditRecord, mutate: impl FnOnce(&mut Value)) -> AuditRecord {
        let mut json = serde_json::to_value(record).unwrap();
        mutate(&mut json);
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn the_first_record_opens_the_chain() {
        let record =
            AuditRecord::first(fact("e1", AuditEventType::CeremonyInstanceStarted)).unwrap();

        assert!(record.sequence().is_first());
        assert!(record.previous_record_hash().is_none());
        assert!(record.digest_is_intact().unwrap());
        assert_eq!(record.schema_version(), AUDIT_RECORD_SCHEMA_VERSION);
    }

    #[test]
    fn a_sealed_record_carries_its_event_and_derives_its_type() {
        let sealed = fact("e1", AuditEventType::StepFailed);
        let record = AuditRecord::first(sealed.clone()).unwrap();

        assert_eq!(record.event(), Some(&sealed.event));
        assert_eq!(record.event_type(), AuditEventType::StepFailed);
        assert_eq!(record.event_schema_version(), Some(EventSchemaVersion::V2));
    }

    #[test]
    fn a_sealed_record_round_trips_through_serde_and_stays_intact() {
        let [first, second, third] = chain_of_three();

        for record in [first, second, third] {
            let json = serde_json::to_string(&record).unwrap();
            let restored: AuditRecord = serde_json::from_str(&json).unwrap();

            assert_eq!(restored, record);
            assert!(restored.digest_is_intact().unwrap());
            assert_eq!(restored.event(), record.event());
        }
    }

    #[test]
    fn a_successor_continues_its_predecessor() {
        let [first, second, third] = chain_of_three();

        assert!(second.continues(&first));
        assert!(third.continues(&second));
        assert!(second.digest_is_intact().unwrap());
    }

    #[test]
    fn sealing_the_same_fact_at_the_same_position_is_deterministic() {
        let once = AuditRecord::first(fact("e1", AuditEventType::StepStarted)).unwrap();
        let twice = AuditRecord::first(fact("e1", AuditEventType::StepStarted)).unwrap();

        assert_eq!(once.record_hash(), twice.record_hash());
    }

    #[test]
    fn altering_any_field_breaks_the_digest() {
        let [first, ..] = chain_of_three();

        let cases: Vec<(&str, Tampering)> = vec![
            (
                "actor identity",
                Box::new(|json: &mut Value| json["actor"]["actor_id"] = "someone-else".into()),
            ),
            (
                "actor kind",
                Box::new(|json: &mut Value| json["actor"]["kind"] = "engine".into()),
            ),
            (
                "timestamp",
                Box::new(|json: &mut Value| {
                    json["occurred_at"] = "2026-07-29T10:00:00Z".into();
                }),
            ),
            (
                "sequence",
                Box::new(|json: &mut Value| json["sequence"] = 7.into()),
            ),
            (
                "ceremony",
                Box::new(|json: &mut Value| json["ceremony_id"] = "ceremony-2".into()),
            ),
            (
                "definition version",
                Box::new(|json: &mut Value| json["definition_version"] = "2.0".into()),
            ),
            (
                "event payload",
                Box::new(|json: &mut Value| json["event"]["initial_state"] = "ELSEWHERE".into()),
            ),
            (
                "event payload context",
                Box::new(|json: &mut Value| {
                    json["event"]["context"]["planted"] = "after the fact".into();
                }),
            ),
            (
                "event removed",
                Box::new(|json: &mut Value| json["event"] = Value::Null),
            ),
        ];

        for (label, mutate) in cases {
            let altered = tampered(&first, mutate);

            assert!(
                !altered.digest_is_intact().unwrap(),
                "altering the {label} left the digest intact"
            );
        }
    }

    #[test]
    fn a_record_whose_type_disagrees_with_its_event_cannot_be_read() {
        let [first, ..] = chain_of_three();
        let mut json = serde_json::to_value(&first).unwrap();
        json["event_type"] = "step_failed".into();

        let error = serde_json::from_value::<AuditRecord>(json).unwrap_err();

        assert!(error.to_string().contains("step_failed"));
    }

    #[test]
    fn a_record_read_under_another_payload_version_cannot_be_read() {
        let [first, ..] = chain_of_three();
        let mut json = serde_json::to_value(&first).unwrap();
        json["event_schema_version"] = 2.into();

        let error = serde_json::from_value::<AuditRecord>(json).unwrap_err();

        assert!(error.to_string().contains("schema version 2"));
    }

    #[test]
    fn an_event_without_a_payload_version_cannot_be_read() {
        let [first, ..] = chain_of_three();
        let mut json = serde_json::to_value(&first).unwrap();
        json["event_schema_version"] = Value::Null;

        assert!(serde_json::from_value::<AuditRecord>(json).is_err());
    }

    #[test]
    fn a_version_two_record_without_its_event_is_not_intact() {
        let [first, ..] = chain_of_three();
        let stripped = tampered(&first, |json| {
            json["event"] = Value::Null;
            json["event_schema_version"] = Value::Null;
        });

        assert!(stripped.event().is_none());
        assert_eq!(stripped.schema_version(), AUDIT_RECORD_SCHEMA_VERSION);
        assert!(!stripped.digest_is_intact().unwrap());
    }

    #[test]
    fn a_version_one_record_that_gained_an_event_is_not_intact() {
        // Sealed under version 2, then relabelled as version 1 with the
        // event still attached: neither shape's preimage matches, and
        // the legacy shape refuses to carry a payload at all.
        let [first, ..] = chain_of_three();
        let relabelled = tampered(&first, |json| json["schema_version"] = 1.into());

        assert!(!relabelled.digest_is_intact().unwrap());
    }

    #[test]
    fn an_unknown_record_schema_version_is_not_intact() {
        let [first, ..] = chain_of_three();
        let relabelled = tampered(&first, |json| json["schema_version"] = 3.into());

        assert!(!relabelled.digest_is_intact().unwrap());
    }

    #[test]
    fn the_payload_version_is_part_of_the_digest() {
        // Two records identical but for the payload version they claim
        // must not share a digest, or a payload could be reread under a
        // shape it was not written in without the chain noticing.
        let sealed = AuditRecord::first(fact("e1", AuditEventType::CeremonyCompleted)).unwrap();
        let mut reclaimed = sealed.clone();
        reclaimed.event_schema_version = Some(EventSchemaVersion::new(2).unwrap());

        assert_ne!(
            reclaimed.compute_hash().unwrap(),
            sealed.compute_hash().unwrap()
        );
    }

    #[test]
    fn removing_a_record_breaks_the_chain() {
        let [first, _removed, third] = chain_of_three();

        assert!(!third.continues(&first));
    }

    #[test]
    fn reordering_records_breaks_the_chain() {
        let [first, second, third] = chain_of_three();

        assert!(!second.continues(&third));
        assert!(!first.continues(&second));
    }

    #[test]
    fn an_inserted_record_cannot_be_woven_into_the_chain() {
        let [first, second, _] = chain_of_three();
        let forged =
            AuditRecord::following(fact("forged", AuditEventType::StepFailed), &first).unwrap();

        // The forgery is a well-formed successor of the first record...
        assert!(forged.continues(&first));
        // ...but the record that really followed still names the first
        // one, so both cannot occupy the position.
        assert!(second.continues(&first));
        assert_eq!(forged.sequence(), second.sequence());
        assert_ne!(forged.record_hash(), second.record_hash());
        // And nothing that followed the genuine record follows the
        // forgery.
        assert!(!second.continues(&forged));
    }

    #[test]
    fn a_record_from_another_ceremony_cannot_follow() {
        let first = AuditRecord::first(fact("e1", AuditEventType::StepStarted)).unwrap();
        let mut foreign = fact("e2", AuditEventType::StepCompleted);
        foreign.ceremony_id = CeremonyId::new("ceremony-2").unwrap();

        assert!(matches!(
            AuditRecord::following(foreign, &first),
            Err(DomainError::InvariantViolated { .. })
        ));
    }

    #[test]
    fn field_boundaries_cannot_be_shifted_between_neighbours() {
        // The classic attack on a concatenated digest: move a character
        // from one field into the next and keep the same byte stream.
        // Length prefixes are what make these two records differ.
        let mut left = fact("e1", AuditEventType::StepStarted);
        left.actor = AuditActor::new(
            "ab",
            AuditActorKind::Agent,
            Some(RoleId::new("reviewer").unwrap()),
        )
        .unwrap();

        let mut right = fact("e1", AuditEventType::StepStarted);
        right.actor = AuditActor::new(
            "a",
            AuditActorKind::Agent,
            Some(RoleId::new("breviewer").unwrap()),
        )
        .unwrap();

        let left = AuditRecord::first(left).unwrap();
        let right = AuditRecord::first(right).unwrap();

        assert_ne!(left.record_hash(), right.record_hash());
    }

    #[test]
    fn an_absent_optional_field_differs_from_a_present_one() {
        let without = AuditRecord::first(fact("e1", AuditEventType::StepStarted)).unwrap();
        let mut with = fact("e1", AuditEventType::StepStarted);
        with.correlation_id = Some(EventId::new("c1").unwrap());
        let with = AuditRecord::first(with).unwrap();

        assert_ne!(without.record_hash(), with.record_hash());
    }
}
