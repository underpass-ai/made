//! [`EvidenceBundle`] — one ceremony's journal, verified, ready to leave
//! the store it was written in.
//!
//! The hash chain proves a journal is internally consistent; it does not
//! say who wrote it, and a copy on somebody else's disk has no store to
//! vouch for it. A bundle is the records plus the head they end at, and
//! an attestation message over that head is what a signer signs: since
//! the head digest commits to every record before it, signing the head
//! signs the journal.

use crate::error::DomainError;
use crate::value_objects::{AuditChainVerdict, CeremonyId, EvidenceHead, StreamVersion};

use super::{AuditChain, AuditRecord};

/// Domain separator of the attestation message. A different scheme can
/// never produce the same bytes, and bumping it is how the message is
/// versioned.
pub const EVIDENCE_BUNDLE_SCHEME: &str = "underpass.made.evidence-bundle.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundle {
    ceremony_id: CeremonyId,
    records: Vec<AuditRecord>,
}

impl EvidenceBundle {
    /// Take a whole journal, in order, and refuse one that does not
    /// verify: a bundle is a claim that these records are a ceremony's
    /// journal, and the claim is checked where it is made.
    pub fn from_records(records: Vec<AuditRecord>) -> Result<Self, DomainError> {
        let Some(first) = records.first() else {
            return Err(DomainError::NotFound {
                what: "ceremony_instance",
            });
        };
        if !AuditChain::verify(&records).is_intact() {
            return Err(DomainError::InvariantViolated {
                reason: "evidence bundle refuses a journal that does not verify",
            });
        }
        Ok(Self {
            ceremony_id: first.ceremony_id().clone(),
            records,
        })
    }

    #[must_use]
    pub fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }

    #[must_use]
    pub fn into_records(self) -> Vec<AuditRecord> {
        self.records
    }

    /// Where these records end.
    #[must_use]
    pub fn head(&self) -> EvidenceHead {
        Self::head_of(&self.records).expect("a bundle holds at least one record")
    }

    /// The head of any non-empty record list, verified or not: what a
    /// verifier recomputes to compare against the head a bundle claims.
    #[must_use]
    pub fn head_of(records: &[AuditRecord]) -> Option<EvidenceHead> {
        records.last().map(|last| {
            EvidenceHead::new(
                StreamVersion::from_sequence(last.sequence()),
                last.record_hash(),
                records.len(),
            )
        })
    }

    /// The bytes a signer signs and a verifier checks: the scheme, the
    /// ceremony and the head, each field length-delimited so no two
    /// bundles share a message.
    #[must_use]
    pub fn attestation_message(ceremony_id: &CeremonyId, head: &EvidenceHead) -> Vec<u8> {
        let mut message = Vec::new();
        for field in [
            EVIDENCE_BUNDLE_SCHEME.to_owned(),
            ceremony_id.as_str().to_owned(),
            head.version().value().to_string(),
            head.hash().to_hex(),
            head.record_count().to_string(),
        ] {
            message.extend_from_slice(&(field.len() as u64).to_be_bytes());
            message.extend_from_slice(field.as_bytes());
        }
        message
    }

    /// Verify the records again. A bundle is built verified; this is
    /// for the reader who did not build it.
    #[must_use]
    pub fn verify_chain(records: &[AuditRecord]) -> AuditChainVerdict {
        AuditChain::verify(records)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::Value;
    use time::macros::datetime;

    use super::*;
    use crate::entities::ceremony_events::StepCompleted;
    use crate::entities::{AuditFact, CeremonyEvent};
    use crate::value_objects::{
        AuditActor, AuditActorKind, CeremonyName, CeremonyVersion, EventId, RoleId, StepAttempt,
        StepId, StepIteration, StepOutput, StepResult,
    };

    fn fact(event_id: &str, ceremony: &str) -> AuditFact {
        AuditFact {
            event_id: EventId::new(event_id).unwrap(),
            event: CeremonyEvent::StepCompleted(StepCompleted {
                state_visit: None,
                step_id: StepId::new("draft").unwrap(),
                state_iteration: None,
                iteration: StepIteration::FIRST,
                attempt: StepAttempt::FIRST,
                result: StepResult::completed(StepOutput::empty()).unwrap(),
                next_iteration: None,
                finished_by: RoleId::new("author").unwrap(),
                finished_at: datetime!(2026-07-29 09:00:00 UTC),
            }),
            ceremony_id: CeremonyId::new(ceremony).unwrap(),
            definition_name: CeremonyName::new("planning_ceremony").unwrap(),
            definition_version: CeremonyVersion::v1(),
            occurred_at: datetime!(2026-07-29 09:00:00 UTC),
            actor: AuditActor::new("engineer-1", AuditActorKind::Human, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        }
    }

    pub(crate) fn chain() -> Vec<AuditRecord> {
        let first = AuditRecord::first(fact("e1", "ceremony-1")).unwrap();
        let second = AuditRecord::following(fact("e2", "ceremony-1"), &first).unwrap();
        let third = AuditRecord::following(fact("e3", "ceremony-1"), &second).unwrap();
        vec![first, second, third]
    }

    #[test]
    fn a_bundle_is_its_verified_records_and_their_head() {
        let records = chain();
        let bundle = EvidenceBundle::from_records(records.clone()).unwrap();
        assert_eq!(bundle.ceremony_id().as_str(), "ceremony-1");
        assert_eq!(bundle.records(), &records[..]);
        let head = bundle.head();
        assert_eq!(head.version().value(), 3);
        assert_eq!(head.record_count(), 3);
        assert_eq!(head.hash(), records[2].record_hash());
    }

    #[test]
    fn an_empty_or_broken_journal_is_refused() {
        assert!(matches!(
            EvidenceBundle::from_records(Vec::new()),
            Err(DomainError::NotFound { .. })
        ));
        let records = chain();
        assert!(matches!(
            EvidenceBundle::from_records(records[1..].to_vec()),
            Err(DomainError::InvariantViolated { .. })
        ));
        let mut json = serde_json::to_value(&records[1]).unwrap();
        json["actor"]["actor_id"] = Value::String("somebody-else".to_owned());
        let tampered: AuditRecord = serde_json::from_value(json).unwrap();
        let tampered = vec![records[0].clone(), tampered, records[2].clone()];
        assert!(!EvidenceBundle::verify_chain(&tampered).is_intact());
        assert!(EvidenceBundle::from_records(tampered).is_err());
    }

    #[test]
    fn the_attestation_message_is_deterministic_and_bound_to_the_head() {
        let bundle = EvidenceBundle::from_records(chain()).unwrap();
        let message = EvidenceBundle::attestation_message(bundle.ceremony_id(), &bundle.head());
        assert_eq!(
            message,
            EvidenceBundle::attestation_message(bundle.ceremony_id(), &bundle.head())
        );
        assert!(message.starts_with(&(EVIDENCE_BUNDLE_SCHEME.len() as u64).to_be_bytes()));
        let shorter = EvidenceBundle::from_records(chain()[..2].to_vec()).unwrap();
        assert_ne!(
            message,
            EvidenceBundle::attestation_message(shorter.ceremony_id(), &shorter.head())
        );
        let other = CeremonyId::new("ceremony-2").unwrap();
        assert_ne!(
            message,
            EvidenceBundle::attestation_message(&other, &bundle.head())
        );
    }
}
