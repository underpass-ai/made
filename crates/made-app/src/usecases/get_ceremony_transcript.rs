use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::CeremonyTranscriptStorePort;
use made_core::value_objects::{CeremonyId, CeremonyTranscript};

/// Retrieves the ordered contributions accumulated by one ceremony instance.
pub struct GetCeremonyTranscriptUseCase {
    transcript_store: Arc<dyn CeremonyTranscriptStorePort>,
}

impl fmt::Debug for GetCeremonyTranscriptUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetCeremonyTranscriptUseCase")
            .finish()
    }
}

impl GetCeremonyTranscriptUseCase {
    #[must_use]
    pub fn new(transcript_store: Arc<dyn CeremonyTranscriptStorePort>) -> Self {
        Self { transcript_store }
    }

    #[tracing::instrument(name = "get_ceremony_transcript", skip_all, fields(ceremony_id = %id))]
    pub async fn execute(&self, id: &CeremonyId) -> Result<CeremonyTranscript, DomainError> {
        self.transcript_store.transcript(id).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use made_core::value_objects::{
        Attributes, AuditActorKind, CeremonyContext, StepOutput, StepResult,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, lease_owner, lease_ttl, now, stream, two_step_definition, ContextStoreFake,
        DefinitionRepositoryFake, EventStoreFake, FixedClock, SequenceStepHandlerFake,
    };
    use crate::usecases::{RunCeremonyInput, RunCeremonyUseCase};

    /// What the transcript store holds for a two-step session, as
    /// JSON, captured before the store was replaced by a fold of the
    /// stream (plan §3.1 A5).
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

    /// The session both halves of this slice are compared over: two
    /// steps, two different outputs, run to its terminal state.
    async fn two_step_session(transcript_store: Arc<ContextStoreFake>) -> Arc<EventStoreFake> {
        let definition = two_step_definition();
        let events = Arc::new(EventStoreFake::default());
        let handler = Arc::new(SequenceStepHandlerFake::new([
            StepResult::completed(spoke("the room is open")).unwrap(),
            StepResult::completed(spoke("and here is the answer")).unwrap(),
        ]));
        RunCeremonyUseCase::new(
            Arc::new(DefinitionRepositoryFake::new(definition.clone())),
            stream(events.clone()),
            handler,
            transcript_store,
            Arc::new(FixedClock::new(now())),
        )
        .execute(RunCeremonyInput::new(
            ceremony_id(),
            definition,
            CeremonyContext::empty(),
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
    async fn the_store_holds_this_transcript_for_this_session() {
        let transcript_store = Arc::new(ContextStoreFake::default());
        two_step_session(transcript_store.clone()).await;

        let transcript = GetCeremonyTranscriptUseCase::new(transcript_store)
            .execute(&ceremony_id())
            .await
            .unwrap();

        assert_eq!(
            serde_json::to_string_pretty(&transcript).unwrap(),
            TRANSCRIPT_GOLDEN
        );
    }
}
