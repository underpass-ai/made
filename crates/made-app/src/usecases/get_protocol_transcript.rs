use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::ProtocolEventStorePort;
use made_core::value_objects::{ProtocolId, ProtocolTranscript};

use super::ReadWholeProtocolEventsUseCase;
use crate::services::protocol_transcript_projection;

/// The ordered contributions the steps of one session produced.
///
/// A projection of the stream, computed on read (ADR-012): every
/// `StepCompleted` record is one contribution, and there is nothing
/// else to consult. The transcript is therefore exactly as durable as
/// the session is — the deployable edition used to keep it in process,
/// where it emptied on restart while the stream did not.
pub struct GetProtocolTranscriptUseCase {
    events: Arc<dyn ProtocolEventStorePort>,
}

impl fmt::Debug for GetProtocolTranscriptUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetProtocolTranscriptUseCase")
            .finish()
    }
}

impl GetProtocolTranscriptUseCase {
    #[must_use]
    pub fn new(events: Arc<dyn ProtocolEventStorePort>) -> Self {
        Self { events }
    }

    /// The whole stream, from the first record: a transcript that
    /// began in the middle would be missing what was said before it.
    ///
    /// A stream nothing was ever appended to is a session that was
    /// never started, and it is refused rather than answered with
    /// nothing — the same answer `ReadProtocolEvents` gives, for the
    /// same reason: an empty transcript would tell a caller that a
    /// session it named exists and has said nothing yet.
    #[tracing::instrument(name = "get_protocol_transcript", skip_all, fields(protocol_id = %id))]
    pub async fn execute(&self, id: &ProtocolId) -> Result<ProtocolTranscript, DomainError> {
        let records = ReadWholeProtocolEventsUseCase::new(self.events.clone())
            .execute(id)
            .await?;
        if records.is_empty() {
            return Err(DomainError::NotFound {
                what: "protocol_instance",
            });
        }
        Ok(protocol_transcript_projection::transcript(&records))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use made_core::ports::ProtocolSnapshotStorePort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, ProtocolContext, StepOutput, StepResult,
    };

    use super::*;
    use crate::usecases::protocol_test_support::{
        definition, definition_resolver, idempotency_key, lease_owner, lease_ttl, now, protocol_id,
        role_id, started_instance, step_id, stream, two_step_definition, DefinitionRepositoryFake,
        EventStoreFake, FixedClock, SequenceStepHandlerFake,
    };
    use crate::usecases::{
        CompleteProtocolStepInput, CompleteProtocolStepUseCase, RunProtocolInput,
        RunProtocolUseCase, StartProtocolStepInput, StartProtocolStepUseCase,
    };

    /// What the transcript store held for a two-step session, as JSON,
    /// captured before the store was replaced by a fold of the stream
    /// (plan §3.1 A5).
    ///
    /// The point of writing it out rather than asserting field by
    /// field: the answer that crosses both MCP backends and the gRPC
    /// contract is this value, so a fold that agreed about the steps
    /// and disagreed about their order, their roles or the shape of an
    /// output would still be a different answer to the same question.
    const TRANSCRIPT_GOLDEN: &str = r#"[
  {
    "step_id": "open",
    "role_id": "FACILITATOR",
    "output": {
      "said": "the room is open"
    }
  },
  {
    "step_id": "respond",
    "role_id": "FACILITATOR",
    "output": {
      "said": "and here is the answer"
    }
  }
]"#;

    fn spoke(words: &str) -> StepOutput {
        StepOutput::new(
            Attributes::new(BTreeMap::from([(
                "said".to_owned(),
                serde_json::json!(words),
            )]))
            .unwrap(),
        )
    }

    /// The session the golden was captured over: two steps, two
    /// different outputs, run to its terminal state.
    async fn two_step_session() -> Arc<EventStoreFake> {
        let definition = two_step_definition();
        let events = Arc::new(EventStoreFake::default());
        let handler = Arc::new(SequenceStepHandlerFake::new([
            StepResult::completed(spoke("the room is open")).unwrap(),
            StepResult::completed(spoke("and here is the answer")).unwrap(),
        ]));
        RunProtocolUseCase::new(
            Arc::new(DefinitionRepositoryFake::new(definition.clone())),
            stream(events.clone()),
            handler,
            Arc::new(FixedClock::new(now())),
        )
        .execute(RunProtocolInput::new(
            protocol_id(),
            definition,
            ProtocolContext::empty(),
            lease_owner(),
            lease_ttl(),
            "operator-1",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
        events
    }

    #[tokio::test]
    async fn the_fold_adds_visits_to_what_the_store_used_to_hold() {
        let events = Box::pin(two_step_session()).await;

        let transcript = GetProtocolTranscriptUseCase::new(events)
            .execute(&protocol_id())
            .await
            .unwrap();

        let mut expected: serde_json::Value = serde_json::from_str(TRANSCRIPT_GOLDEN).unwrap();
        expected[1]["state_visit"] = serde_json::json!(2);
        assert_eq!(serde_json::to_value(&transcript).unwrap(), expected);
    }

    /// Snapshots are a cache (ADR-012), and this is a transcript that
    /// never consults one: the records are the whole of the answer.
    #[tokio::test]
    async fn the_answer_survives_losing_every_snapshot() {
        let events = Box::pin(two_step_session()).await;
        let before = GetProtocolTranscriptUseCase::new(events.clone())
            .execute(&protocol_id())
            .await
            .unwrap();

        events.forget(&protocol_id()).await.unwrap();

        assert_eq!(
            GetProtocolTranscriptUseCase::new(events)
                .execute(&protocol_id())
                .await
                .unwrap(),
            before
        );
    }

    /// The gap the store had, closed by the fold: a step the host
    /// claimed, performed where the engine cannot see it, and reported
    /// back never reached the transcript store, because only the two
    /// drivers wrote to it. The event is the same event either way.
    #[tokio::test]
    async fn a_step_the_host_performed_itself_is_in_the_transcript() {
        let definition = definition();
        let events = Arc::new(EventStoreFake::default());
        events.save(&started_instance(&definition)).await.unwrap();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition));
        let clock = Arc::new(FixedClock::new(now()));

        let claim = StartProtocolStepUseCase::new(
            definition_resolver(definitions.clone()),
            stream(events.clone()),
            clock.clone(),
        )
        .execute(StartProtocolStepInput::new(
            protocol_id(),
            role_id(),
            AuditActorKind::Agent,
            step_id(),
            lease_owner(),
            idempotency_key("delegated-1"),
            lease_ttl(),
        ))
        .await
        .unwrap();
        CompleteProtocolStepUseCase::new(
            definition_resolver(definitions),
            stream(events.clone()),
            clock,
        )
        .execute(CompleteProtocolStepInput::new(
            protocol_id(),
            step_id(),
            StepResult::completed(spoke("the host did it")).unwrap(),
            AuditActorKind::Agent,
            claim.claim_fence().clone(),
        ))
        .await
        .unwrap();

        let transcript = GetProtocolTranscriptUseCase::new(events)
            .execute(&protocol_id())
            .await
            .unwrap();

        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript.contributions()[0].step_id(), &step_id());
        assert_eq!(transcript.contributions()[0].role_id(), &role_id());
        assert_eq!(
            transcript.contributions()[0]
                .output()
                .attributes()
                .get("said")
                .and_then(serde_json::Value::as_str),
            Some("the host did it")
        );
    }

    #[tokio::test]
    async fn a_session_that_has_said_nothing_has_an_empty_transcript() {
        let events = Arc::new(EventStoreFake::default());
        events.save(&started_instance(&definition())).await.unwrap();

        assert!(GetProtocolTranscriptUseCase::new(events)
            .execute(&protocol_id())
            .await
            .unwrap()
            .is_empty());
    }

    /// A session that was never started is not a session with nothing
    /// to say. Answering with an empty transcript would tell a caller
    /// that the id it named exists.
    #[tokio::test]
    async fn a_session_that_is_not_there_is_not_found() {
        let events = Arc::new(EventStoreFake::default());

        assert!(matches!(
            GetProtocolTranscriptUseCase::new(events)
                .execute(&ProtocolId::new("no-such-session").unwrap())
                .await,
            Err(DomainError::NotFound {
                what: "protocol_instance"
            })
        ));
    }
}
