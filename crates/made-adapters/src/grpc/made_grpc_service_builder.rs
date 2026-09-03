use std::sync::Arc;

use made_app::services::AutoDispatchService;
use made_app::usecases::{
    ApplyCeremonyTransitionUseCase, ApproveCeremonyGuardUseCase, AssertCeremonyReasonUseCase,
    BindCeremonyParticipantsUseCase, CloseCeremonyInterventionUseCase,
    CollectCeremonyEvidenceUseCase, CreateCouncilUseCase, DeferCeremonyGuardUseCase,
    DeleteCouncilUseCase, DeliberateUseCase, DiffCeremonyDefinitionsUseCase,
    GetCeremonyInstanceUseCase, GetDeliberationUseCase, ListCeremonyInstancesUseCase,
    ListCouncilsUseCase, OrchestrateUseCase, PrepareCeremonyParticipantsUseCase,
    PublishCeremonyDefinitionUseCase, RegisterAgentUseCase, RequestCeremonyInterventionUseCase,
    ResolveCeremonyDefinitionUseCase, RespondToCeremonyInterventionUseCase, RunCeremonyStepUseCase,
    RunCeremonyUseCase, RunCouncilDecisionUseCase, StartCeremonyUseCase,
    StartPublishedCeremonyUseCase, UnregisterAgentUseCase,
};
use made_core::ports::{CeremonyDefinitionRepositoryPort, ContractRegistryPort, StatisticsPort};

/// Builder so composition-root wiring is readable even as the number
/// of use cases grows.
#[derive(Default)]
pub struct MadeGrpcServiceBuilder {
    pub(super) deliberate: Option<Arc<DeliberateUseCase>>,
    pub(super) orchestrate: Option<Arc<OrchestrateUseCase>>,
    pub(super) create_council: Option<Arc<CreateCouncilUseCase>>,
    pub(super) delete_council: Option<Arc<DeleteCouncilUseCase>>,
    pub(super) list_councils: Option<Arc<ListCouncilsUseCase>>,
    pub(super) get_deliberation: Option<Arc<GetDeliberationUseCase>>,
    pub(super) register_agent: Option<Arc<RegisterAgentUseCase>>,
    pub(super) unregister_agent: Option<Arc<UnregisterAgentUseCase>>,
    pub(super) run_council_decision: Option<Arc<RunCouncilDecisionUseCase>>,
    pub(super) run_ceremony: Option<Arc<RunCeremonyUseCase>>,
    pub(super) get_ceremony_instance: Option<Arc<GetCeremonyInstanceUseCase>>,
    pub(super) list_ceremony_instances: Option<Arc<ListCeremonyInstancesUseCase>>,
    pub(super) resolve_ceremony_definition: Option<Arc<ResolveCeremonyDefinitionUseCase>>,
    pub(super) start_ceremony: Option<Arc<StartCeremonyUseCase>>,
    pub(super) start_published_ceremony: Option<Arc<StartPublishedCeremonyUseCase>>,
    pub(super) run_ceremony_step: Option<Arc<RunCeremonyStepUseCase>>,
    pub(super) apply_ceremony_transition: Option<Arc<ApplyCeremonyTransitionUseCase>>,
    pub(super) approve_ceremony_guard: Option<Arc<ApproveCeremonyGuardUseCase>>,
    pub(super) defer_ceremony_guard: Option<Arc<DeferCeremonyGuardUseCase>>,
    pub(super) assert_ceremony_reason: Option<Arc<AssertCeremonyReasonUseCase>>,
    pub(super) request_ceremony_intervention: Option<Arc<RequestCeremonyInterventionUseCase>>,
    pub(super) respond_to_ceremony_intervention: Option<Arc<RespondToCeremonyInterventionUseCase>>,
    pub(super) close_ceremony_intervention: Option<Arc<CloseCeremonyInterventionUseCase>>,
    pub(super) collect_ceremony_evidence: Option<Arc<CollectCeremonyEvidenceUseCase>>,
    pub(super) diff_ceremony_definitions: Option<Arc<DiffCeremonyDefinitionsUseCase>>,
    pub(super) bind_ceremony_participants: Option<Arc<BindCeremonyParticipantsUseCase>>,
    pub(super) publish_ceremony_definition: Option<Arc<PublishCeremonyDefinitionUseCase>>,
    pub(super) ceremony_definitions: Option<Arc<dyn CeremonyDefinitionRepositoryPort>>,
    pub(super) prepare_ceremony_participants: Option<Arc<PrepareCeremonyParticipantsUseCase>>,
    pub(super) contract_registry: Option<Arc<dyn ContractRegistryPort>>,
    pub(super) auto_dispatch: Option<Arc<AutoDispatchService>>,
    pub(super) statistics: Option<Arc<dyn StatisticsPort>>,
    pub(super) service_version: Option<&'static str>,
}
