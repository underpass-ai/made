use std::sync::Arc;

use made_app::services::SessionStream;
use made_app::usecases::{
    CancelCeremonyUseCase, EnforceCeremonyDeadlinesUseCase, PauseCeremonyUseCase,
    ResolveCeremonyDefinitionUseCase, ResumeCeremonyUseCase,
};
use made_core::ports::ClockPort;

pub(super) struct CeremonyLifecycleControls {
    pub(super) pause: Arc<PauseCeremonyUseCase>,
    pub(super) resume: Arc<ResumeCeremonyUseCase>,
    pub(super) cancel: Arc<CancelCeremonyUseCase>,
    pub(super) enforce_deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
}

impl CeremonyLifecycleControls {
    pub(super) fn wire(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
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
}
