//! The transcript, in the stream's terms.
//!
//! A transcript is what the steps of a session said, in the order they
//! said it. That is `StepCompleted` and nothing else: the event
//! carries the step, the seat the definition gives it and the output
//! the step produced, which is exactly a contribution.
//!
//! # Why this is a fold and not a store
//!
//! It used to be a store the drivers appended to beside the append
//! that sealed the step (ADR-012, "everything else is a projection").
//! Two consequences followed from that and both are gone with it: the
//! deployable edition kept the transcript in process, so it emptied on
//! restart while the stream did not; and a step completed through the
//! delegated-host protocol — claimed, performed by the host, reported
//! back — never reached the transcript at all, because only the two
//! drivers wrote to the store. The fold has every step that completed,
//! however it was driven, for as long as the stream is kept.
//!
//! # A record with no event
//!
//! A record sealed under schema version 1 carries no event and cannot
//! say whether a step completed. It contributes nothing here rather
//! than failing the read: a journal from before the stream is what the
//! migration command exists for, and a transcript that refused to
//! answer at all would be a worse answer than a short one.

use made_core::entities::{AuditRecord, CeremonyEvent};
use made_core::value_objects::{CeremonyStepContribution, CeremonyTranscript};

/// The transcript of the session these records are the stream of.
pub(crate) fn transcript(records: &[AuditRecord]) -> CeremonyTranscript {
    CeremonyTranscript::new(
        records
            .iter()
            .filter_map(AuditRecord::event)
            .filter_map(|event| match event {
                CeremonyEvent::StepCompleted(completed) => Some(CeremonyStepContribution::new(
                    completed.step_id.clone(),
                    completed.finished_by.clone(),
                    completed.result.output().clone(),
                )),
                _ => None,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use made_core::entities::ceremony_events::{CeremonyCompleted, StepCompleted, StepFailed};
    use made_core::entities::AuditFact;
    use made_core::value_objects::{
        Attributes, AuditActor, AuditActorKind, CeremonyId, CeremonyName, CeremonyVersion, EventId,
        RoleId, StateId, StepAttempt, StepErrorMessage, StepId, StepIteration, StepOutput,
        StepResult,
    };
    use std::collections::BTreeMap;
    use time::OffsetDateTime;

    use super::*;

    fn record(event_id: &str, event: CeremonyEvent) -> AuditRecord {
        AuditRecord::first(AuditFact {
            event_id: EventId::new(event_id).unwrap(),
            event,
            ceremony_id: CeremonyId::new("c1").unwrap(),
            definition_name: CeremonyName::new("folding").unwrap(),
            definition_version: CeremonyVersion::v1(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        })
        .unwrap()
    }

    fn said(words: &str) -> StepOutput {
        StepOutput::new(
            Attributes::new(BTreeMap::from([(
                "said".to_owned(),
                serde_json::json!(words),
            )]))
            .unwrap(),
        )
    }

    fn completed(step: &str, role: &str, words: &str) -> CeremonyEvent {
        CeremonyEvent::StepCompleted(StepCompleted {
            step_id: StepId::new(step).unwrap(),
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result: StepResult::completed(said(words)).unwrap(),
            next_iteration: None,
            finished_by: RoleId::new(role).unwrap(),
            finished_at: OffsetDateTime::UNIX_EPOCH,
        })
    }

    #[test]
    fn every_completed_step_contributes_once_in_order() {
        let transcript = transcript(&[
            record("e1", completed("open", "FACILITATOR", "the room is open")),
            record("e2", completed("respond", "TABLE_MEMBER", "and the answer")),
        ]);

        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript.contributions()[0].step_id().as_str(), "open");
        assert_eq!(
            transcript.contributions()[0].role_id().as_str(),
            "FACILITATOR"
        );
        assert_eq!(
            transcript.contributions()[1]
                .output()
                .attributes()
                .get("said")
                .and_then(serde_json::Value::as_str),
            Some("and the answer")
        );
    }

    /// A repeat is several passes of one step, and each pass said
    /// something: ADR-010 keeps every successful iteration, and so
    /// does this.
    #[test]
    fn a_step_that_ran_twice_contributes_twice() {
        let transcript = transcript(&[
            record("e1", completed("open", "FACILITATOR", "first pass")),
            record("e2", completed("open", "FACILITATOR", "second pass")),
        ]);

        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript.contributions()[1].step_id().as_str(), "open");
    }

    #[test]
    fn nothing_but_a_completed_step_contributes() {
        let transcript = transcript(&[
            record(
                "e1",
                CeremonyEvent::StepFailed(StepFailed {
                    step_id: StepId::new("open").unwrap(),
                    iteration: StepIteration::FIRST,
                    attempt: StepAttempt::FIRST,
                    result: StepResult::failed(StepErrorMessage::new("no").unwrap()).unwrap(),
                    finished_by: RoleId::new("FACILITATOR").unwrap(),
                    finished_at: OffsetDateTime::UNIX_EPOCH,
                }),
            ),
            record(
                "e2",
                CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
                    final_state: StateId::new("DONE").unwrap(),
                    completed_at: OffsetDateTime::UNIX_EPOCH,
                }),
            ),
        ]);

        assert!(transcript.is_empty());
    }

    #[test]
    fn a_stream_nobody_appended_to_is_an_empty_transcript() {
        assert!(transcript(&[]).is_empty());
    }
}
