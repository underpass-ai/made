use std::sync::Arc;

use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::services::SessionStream;
use made_app::usecases::{
    CancelCeremonyUseCase, EnforceCeremonyDeadlinesUseCase, PauseCeremonyUseCase,
    PlanCeremonySuccessorUseCase, ResolveCeremonyDefinitionUseCase, ResumeCeremonyUseCase,
    StartCeremonySuccessorUseCase,
};
use made_core::ports::ClockPort;

pub(super) struct CeremonyLifecycleControls {
    pub(super) handoff: Arc<made_app::workers::RecordCeremonyHostHandoffUseCase>,
    pub(super) preflight: Arc<made_app::workers::InspectCeremonyResumeUseCase>,
    pub(super) pause: Arc<PauseCeremonyUseCase>,
    pub(super) resume: Arc<ResumeCeremonyUseCase>,
    pub(super) cancel: Arc<CancelCeremonyUseCase>,
    pub(super) enforce_deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
    pub(super) plan_successor: Arc<PlanCeremonySuccessorUseCase>,
    pub(super) start_successor: Arc<StartCeremonySuccessorUseCase>,
}

impl CeremonyLifecycleControls {
    pub(super) fn wire(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
        receipts: Arc<dyn made_core::ports::ExecutionReceiptStorePort>,
        authorize: Arc<made_app::authorization::AuthorizeOperationUseCase>,
        publications: Arc<dyn made_core::ports::CeremonyDefinitionPublicationPort>,
    ) -> Self {
        let preflight = Arc::new(made_app::workers::InspectCeremonyResumeUseCase::new(
            stream.clone(),
            receipts,
            clock.clone(),
        ));
        Self {
            handoff: Arc::new(
                made_app::workers::RecordCeremonyHostHandoffUseCase::new(
                    stream.clone(),
                    definitions.clone(),
                    clock.clone(),
                )
                .with_reauthorization(authorize),
            ),
            plan_successor: Arc::new(PlanCeremonySuccessorUseCase::new(
                definitions.clone(),
                publications.clone(),
                stream.clone(),
                preflight.clone(),
            )),
            start_successor: Arc::new(StartCeremonySuccessorUseCase::new(
                definitions.clone(),
                publications,
                stream.clone(),
                clock.clone(),
            )),
            preflight,
            pause: Arc::new(PauseCeremonyUseCase::new(
                definitions.clone(),
                stream.clone(),
                clock.clone(),
            )),
            resume: Arc::new(ResumeCeremonyUseCase::new(
                definitions.clone(),
                stream.clone(),
                clock.clone(),
            )),
            cancel: Arc::new(CancelCeremonyUseCase::new(
                definitions.clone(),
                stream.clone(),
                clock.clone(),
            )),
            enforce_deadlines: Arc::new(EnforceCeremonyDeadlinesUseCase::new(
                definitions,
                stream,
                clock,
            )),
        }
    }

    pub(super) fn apply_to(self, builder: MadeGrpcServiceBuilder) -> MadeGrpcServiceBuilder {
        builder
            .pause_ceremony(self.pause)
            .record_ceremony_host_handoff(self.handoff)
            .inspect_ceremony_resume(self.preflight)
            .plan_ceremony_successor(self.plan_successor)
            .start_ceremony_successor(self.start_successor)
            .resume_ceremony(self.resume)
            .cancel_ceremony(self.cancel)
            .enforce_ceremony_deadlines(self.enforce_deadlines)
    }
}
