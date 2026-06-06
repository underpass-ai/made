//! [`PrepareCeremonyParticipantsUseCase`] — materialize ceremony agents
//! and councils declared by an adapter.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use choreo_core::entities::Council;
use choreo_core::error::DomainError;
use choreo_core::ports::{AgentFactoryPort, AgentRegistryPort, ClockPort, CouncilRegistryPort};
use choreo_core::value_objects::{AgentId, CouncilId, Specialty};
use tracing::info;

use super::PrepareCeremonyParticipantsInput;

pub struct PrepareCeremonyParticipantsUseCase {
    clock: Arc<dyn ClockPort>,
    agent_factory: Arc<dyn AgentFactoryPort>,
    agent_registry: Arc<dyn AgentRegistryPort>,
    council_registry: Arc<dyn CouncilRegistryPort>,
}

impl std::fmt::Debug for PrepareCeremonyParticipantsUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrepareCeremonyParticipantsUseCase")
            .finish()
    }
}

impl PrepareCeremonyParticipantsUseCase {
    #[must_use]
    pub fn new(
        clock: Arc<dyn ClockPort>,
        agent_factory: Arc<dyn AgentFactoryPort>,
        agent_registry: Arc<dyn AgentRegistryPort>,
        council_registry: Arc<dyn CouncilRegistryPort>,
    ) -> Self {
        Self {
            clock,
            agent_factory,
            agent_registry,
            council_registry,
        }
    }

    #[tracing::instrument(
        name = "prepare_ceremony_participants",
        skip_all,
        fields(participants = input.participants().len())
    )]
    pub async fn execute(
        &self,
        input: PrepareCeremonyParticipantsInput,
    ) -> Result<(), DomainError> {
        let mut councils = BTreeMap::<Specialty, BTreeSet<AgentId>>::new();

        for participant in input.into_participants() {
            let id = participant.id().clone();
            let specialty = participant.specialty().clone();
            let kind = participant.kind().clone();
            let agent = self
                .agent_factory
                .create(participant.into_agent_descriptor())
                .await?;
            match self.agent_registry.register(agent).await {
                Ok(()) | Err(DomainError::AlreadyExists { .. }) => {}
                Err(error) => return Err(error),
            }
            councils.entry(specialty.clone()).or_default().insert(id);
            info!(
                specialty = specialty.as_str(),
                kind = kind.as_str(),
                "ceremony participant prepared"
            );
        }

        for (specialty, agent_ids) in councils {
            let council = Council::new(
                CouncilId::new(format!("ceremony-{}", specialty.as_str()))?,
                specialty.clone(),
                agent_ids,
                self.clock.now(),
            )?;
            match self.council_registry.register(council).await {
                Ok(()) | Err(DomainError::AlreadyExists { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use choreo_core::entities::TaskConstraints;
    use choreo_core::ports::{AgentDescriptor, AgentPort, Critique, DraftRequest, Revision};
    use choreo_core::value_objects::{AgentKind, Attributes};
    use time::OffsetDateTime;

    use super::*;
    use crate::usecases::CeremonyParticipantDescriptor;

    struct FrozenClock;

    impl ClockPort for FrozenClock {
        fn now(&self) -> OffsetDateTime {
            time::macros::datetime!(2026-06-06 12:00:00 UTC)
        }
    }

    #[derive(Debug)]
    struct StubAgent {
        id: AgentId,
        specialty: Specialty,
    }

    #[async_trait]
    impl AgentPort for StubAgent {
        fn id(&self) -> &AgentId {
            &self.id
        }

        fn specialty(&self) -> &Specialty {
            &self.specialty
        }

        async fn generate(&self, _request: DraftRequest) -> Result<Revision, DomainError> {
            Ok(Revision {
                content: String::new(),
            })
        }

        async fn critique(
            &self,
            _peer_content: &str,
            _constraints: &TaskConstraints,
        ) -> Result<Critique, DomainError> {
            Ok(Critique {
                feedback: String::new(),
            })
        }

        async fn revise(
            &self,
            own_content: &str,
            _critique: &Critique,
        ) -> Result<Revision, DomainError> {
            Ok(Revision {
                content: own_content.to_owned(),
            })
        }
    }

    #[derive(Default)]
    struct RecordingFactory {
        descriptors: Mutex<Vec<AgentDescriptor>>,
    }

    #[async_trait]
    impl AgentFactoryPort for RecordingFactory {
        async fn create(
            &self,
            descriptor: AgentDescriptor,
        ) -> Result<Arc<dyn AgentPort>, DomainError> {
            self.descriptors.lock().unwrap().push(descriptor.clone());
            Ok(Arc::new(StubAgent {
                id: descriptor.id,
                specialty: descriptor.specialty,
            }))
        }
    }

    #[derive(Default)]
    struct RecordingAgentRegistry {
        ids: Mutex<Vec<AgentId>>,
    }

    #[async_trait]
    impl AgentRegistryPort for RecordingAgentRegistry {
        async fn register(&self, agent: Arc<dyn AgentPort>) -> Result<(), DomainError> {
            let mut ids = self.ids.lock().unwrap();
            if ids.contains(agent.id()) {
                return Err(DomainError::AlreadyExists { what: "agent" });
            }
            ids.push(agent.id().clone());
            Ok(())
        }

        async fn unregister(&self, _id: &AgentId) -> Result<(), DomainError> {
            unimplemented!()
        }
    }

    #[derive(Default)]
    struct RecordingCouncilRegistry {
        councils: Mutex<BTreeMap<Specialty, Council>>,
    }

    #[async_trait]
    impl CouncilRegistryPort for RecordingCouncilRegistry {
        async fn register(&self, council: Council) -> Result<(), DomainError> {
            let mut councils = self.councils.lock().unwrap();
            if councils.contains_key(council.specialty()) {
                return Err(DomainError::AlreadyExists { what: "council" });
            }
            councils.insert(council.specialty().clone(), council);
            Ok(())
        }

        async fn replace(&self, council: Council) -> Result<(), DomainError> {
            self.councils
                .lock()
                .unwrap()
                .insert(council.specialty().clone(), council);
            Ok(())
        }

        async fn get(&self, specialty: &Specialty) -> Result<Council, DomainError> {
            self.councils
                .lock()
                .unwrap()
                .get(specialty)
                .cloned()
                .ok_or(DomainError::NotFound { what: "council" })
        }

        async fn list(&self) -> Result<Vec<Council>, DomainError> {
            Ok(self.councils.lock().unwrap().values().cloned().collect())
        }

        async fn delete(&self, specialty: &Specialty) -> Result<(), DomainError> {
            self.councils
                .lock()
                .unwrap()
                .remove(specialty)
                .map(|_| ())
                .ok_or(DomainError::NotFound { what: "council" })
        }

        async fn contains(&self, specialty: &Specialty) -> Result<bool, DomainError> {
            Ok(self.councils.lock().unwrap().contains_key(specialty))
        }
    }

    #[tokio::test]
    async fn registers_agents_and_groups_them_into_councils() {
        let factory = Arc::new(RecordingFactory::default());
        let agents = Arc::new(RecordingAgentRegistry::default());
        let councils = Arc::new(RecordingCouncilRegistry::default());
        let usecase = PrepareCeremonyParticipantsUseCase::new(
            Arc::new(FrozenClock),
            factory.clone(),
            agents.clone(),
            councils.clone(),
        );

        usecase
            .execute(PrepareCeremonyParticipantsInput::new(vec![
                participant("agent-editor-0", "editor"),
                participant("agent-editor-1", "editor"),
                participant("agent-reviewer-0", "reviewer"),
            ]))
            .await
            .unwrap();

        assert_eq!(factory.descriptors.lock().unwrap().len(), 3);
        assert_eq!(agents.ids.lock().unwrap().len(), 3);
        let editor = councils
            .get(&Specialty::new("editor").unwrap())
            .await
            .unwrap();
        assert_eq!(editor.size(), 2);
        let reviewer = councils
            .get(&Specialty::new("reviewer").unwrap())
            .await
            .unwrap();
        assert_eq!(reviewer.size(), 1);
    }

    #[tokio::test]
    async fn duplicate_agents_and_councils_are_idempotent() {
        let usecase = PrepareCeremonyParticipantsUseCase::new(
            Arc::new(FrozenClock),
            Arc::new(RecordingFactory::default()),
            Arc::new(RecordingAgentRegistry::default()),
            Arc::new(RecordingCouncilRegistry::default()),
        );
        let input =
            PrepareCeremonyParticipantsInput::new(vec![participant("agent-editor-0", "editor")]);

        usecase.execute(input.clone()).await.unwrap();
        usecase.execute(input).await.unwrap();
    }

    fn participant(id: &str, specialty: &str) -> CeremonyParticipantDescriptor {
        CeremonyParticipantDescriptor::new(
            AgentId::new(id).unwrap(),
            Specialty::new(specialty).unwrap(),
            AgentKind::new("noop").unwrap(),
            Attributes::empty(),
        )
    }
}
