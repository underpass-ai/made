use std::sync::Arc;

use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::services::SessionStream;
use made_app::usecases::{
    CancelProtocolUseCase, EnforceProtocolDeadlinesUseCase, PauseProtocolUseCase,
    PlanProtocolSuccessorUseCase, ResolveProtocolDefinitionUseCase, ResumeProtocolUseCase,
    StartProtocolSuccessorUseCase,
};
use made_core::ports::ClockPort;

pub(super) struct ProtocolLifecycleControls {
    pub(super) handoff: Arc<made_app::workers::RecordProtocolHostHandoffUseCase>,
    pub(super) preflight: Arc<made_app::workers::InspectProtocolResumeUseCase>,
    pub(super) pause: Arc<PauseProtocolUseCase>,
    pub(super) resume: Arc<ResumeProtocolUseCase>,
    pub(super) cancel: Arc<CancelProtocolUseCase>,
    pub(super) enforce_deadlines: Arc<EnforceProtocolDeadlinesUseCase>,
    pub(super) plan_successor: Arc<PlanProtocolSuccessorUseCase>,
    pub(super) start_successor: Arc<StartProtocolSuccessorUseCase>,
}

impl ProtocolLifecycleControls {
    pub(super) fn wire(
        definitions: Arc<ResolveProtocolDefinitionUseCase>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
        receipts: Arc<dyn made_core::ports::ExecutionReceiptStorePort>,
        authorize: Arc<made_app::authorization::AuthorizeOperationUseCase>,
        publications: Arc<dyn made_core::ports::ProtocolDefinitionPublicationPort>,
    ) -> Self {
        let preflight = Arc::new(made_app::workers::InspectProtocolResumeUseCase::new(
            stream.clone(),
            receipts,
            clock.clone(),
        ));
        Self {
            handoff: Arc::new(
                made_app::workers::RecordProtocolHostHandoffUseCase::new(
                    stream.clone(),
                    definitions.clone(),
                    clock.clone(),
                )
                .with_reauthorization(authorize.clone()),
            ),
            plan_successor: Arc::new(PlanProtocolSuccessorUseCase::new(
                definitions.clone(),
                publications.clone(),
                stream.clone(),
                preflight.clone(),
            )),
            start_successor: Arc::new(
                StartProtocolSuccessorUseCase::new(
                    definitions.clone(),
                    publications,
                    stream.clone(),
                    clock.clone(),
                )
                .with_reauthorization(authorize),
            ),
            preflight,
            pause: Arc::new(PauseProtocolUseCase::new(
                definitions.clone(),
                stream.clone(),
                clock.clone(),
            )),
            resume: Arc::new(ResumeProtocolUseCase::new(
                definitions.clone(),
                stream.clone(),
                clock.clone(),
            )),
            cancel: Arc::new(CancelProtocolUseCase::new(
                definitions.clone(),
                stream.clone(),
                clock.clone(),
            )),
            enforce_deadlines: Arc::new(EnforceProtocolDeadlinesUseCase::new(
                definitions,
                stream,
                clock,
            )),
        }
    }

    pub(super) fn apply_to(self, builder: MadeGrpcServiceBuilder) -> MadeGrpcServiceBuilder {
        builder
            .pause_protocol(self.pause)
            .record_protocol_host_handoff(self.handoff)
            .inspect_protocol_resume(self.preflight)
            .plan_protocol_successor(self.plan_successor)
            .start_protocol_successor(self.start_successor)
            .resume_protocol(self.resume)
            .cancel_protocol(self.cancel)
            .enforce_protocol_deadlines(self.enforce_deadlines)
    }
}
