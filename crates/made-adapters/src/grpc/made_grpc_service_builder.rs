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

use made_core::error::DomainError;

use super::MadeGrpcService;

impl std::fmt::Debug for MadeGrpcServiceBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MadeGrpcServiceBuilder").finish()
    }
}

macro_rules! required {
    ($self:ident, $field:ident) => {
        required!($self, $field, "use case")
    };
    ($self:ident, $field:ident, $what:literal) => {
        $self.$field.ok_or(DomainError::InvariantViolated {
            reason: concat!("grpc: ", stringify!($field), " ", $what, " is required"),
        })?
    };
}

macro_rules! setter {
    ($name:ident, $ty:ty, $field:ident) => {
        #[must_use]
        pub fn $name(mut self, value: Arc<$ty>) -> Self {
            self.$field = Some(value);
            self
        }
    };
}

impl MadeGrpcServiceBuilder {
    setter!(deliberate, DeliberateUseCase, deliberate);
    setter!(orchestrate, OrchestrateUseCase, orchestrate);
    setter!(create_council, CreateCouncilUseCase, create_council);
    setter!(delete_council, DeleteCouncilUseCase, delete_council);
    setter!(list_councils, ListCouncilsUseCase, list_councils);
    setter!(get_deliberation, GetDeliberationUseCase, get_deliberation);
    setter!(register_agent, RegisterAgentUseCase, register_agent);
    setter!(unregister_agent, UnregisterAgentUseCase, unregister_agent);
    setter!(
        run_council_decision,
        RunCouncilDecisionUseCase,
        run_council_decision
    );
    setter!(run_ceremony, RunCeremonyUseCase, run_ceremony);
    setter!(
        get_ceremony_instance,
        GetCeremonyInstanceUseCase,
        get_ceremony_instance
    );
    setter!(
        list_ceremony_instances,
        ListCeremonyInstancesUseCase,
        list_ceremony_instances
    );
    setter!(
        resolve_ceremony_definition,
        ResolveCeremonyDefinitionUseCase,
        resolve_ceremony_definition
    );
    setter!(start_ceremony, StartCeremonyUseCase, start_ceremony);
    setter!(
        start_published_ceremony,
        StartPublishedCeremonyUseCase,
        start_published_ceremony
    );
    setter!(run_ceremony_step, RunCeremonyStepUseCase, run_ceremony_step);
    setter!(
        apply_ceremony_transition,
        ApplyCeremonyTransitionUseCase,
        apply_ceremony_transition
    );
    setter!(
        approve_ceremony_guard,
        ApproveCeremonyGuardUseCase,
        approve_ceremony_guard
    );
    setter!(
        defer_ceremony_guard,
        DeferCeremonyGuardUseCase,
        defer_ceremony_guard
    );
    setter!(
        assert_ceremony_reason,
        AssertCeremonyReasonUseCase,
        assert_ceremony_reason
    );
    setter!(
        request_ceremony_intervention,
        RequestCeremonyInterventionUseCase,
        request_ceremony_intervention
    );
    setter!(
        respond_to_ceremony_intervention,
        RespondToCeremonyInterventionUseCase,
        respond_to_ceremony_intervention
    );
    setter!(
        close_ceremony_intervention,
        CloseCeremonyInterventionUseCase,
        close_ceremony_intervention
    );
    setter!(
        collect_ceremony_evidence,
        CollectCeremonyEvidenceUseCase,
        collect_ceremony_evidence
    );
    setter!(
        prepare_ceremony_participants,
        PrepareCeremonyParticipantsUseCase,
        prepare_ceremony_participants
    );
    setter!(
        bind_ceremony_participants,
        BindCeremonyParticipantsUseCase,
        bind_ceremony_participants
    );
    setter!(
        diff_ceremony_definitions,
        DiffCeremonyDefinitionsUseCase,
        diff_ceremony_definitions
    );
    setter!(
        publish_ceremony_definition,
        PublishCeremonyDefinitionUseCase,
        publish_ceremony_definition
    );
    setter!(auto_dispatch, AutoDispatchService, auto_dispatch);

    #[must_use]
    pub fn statistics(mut self, value: Arc<dyn StatisticsPort>) -> Self {
        self.statistics = Some(value);
        self
    }

    #[must_use]
    pub fn ceremony_definitions(
        mut self,
        value: Arc<dyn CeremonyDefinitionRepositoryPort>,
    ) -> Self {
        self.ceremony_definitions = Some(value);
        self
    }

    #[must_use]
    pub fn contract_registry(mut self, value: Arc<dyn ContractRegistryPort>) -> Self {
        self.contract_registry = Some(value);
        self
    }

    #[must_use]
    pub fn service_version(mut self, value: &'static str) -> Self {
        self.service_version = Some(value);
        self
    }

    /// Consume the builder. Missing dependencies are reported via
    /// [`DomainError::InvariantViolated`] so wiring errors surface
    /// through the same error channel the rest of the app uses.
    pub fn build(self) -> Result<MadeGrpcService, DomainError> {
        Ok(MadeGrpcService {
            deliberate: required!(self, deliberate),
            orchestrate: required!(self, orchestrate),
            create_council: required!(self, create_council),
            delete_council: required!(self, delete_council),
            list_councils: required!(self, list_councils),
            get_deliberation: required!(self, get_deliberation),
            register_agent: required!(self, register_agent),
            unregister_agent: required!(self, unregister_agent),
            run_council_decision: required!(self, run_council_decision),
            run_ceremony: required!(self, run_ceremony),
            get_ceremony_instance: required!(self, get_ceremony_instance),
            list_ceremony_instances: required!(self, list_ceremony_instances),
            resolve_ceremony_definition: required!(self, resolve_ceremony_definition),
            start_ceremony: required!(self, start_ceremony),
            start_published_ceremony: required!(self, start_published_ceremony),
            run_ceremony_step: required!(self, run_ceremony_step),
            apply_ceremony_transition: required!(self, apply_ceremony_transition),
            approve_ceremony_guard: required!(self, approve_ceremony_guard),
            defer_ceremony_guard: required!(self, defer_ceremony_guard),
            assert_ceremony_reason: required!(self, assert_ceremony_reason),
            request_ceremony_intervention: required!(self, request_ceremony_intervention),
            respond_to_ceremony_intervention: required!(self, respond_to_ceremony_intervention),
            close_ceremony_intervention: required!(self, close_ceremony_intervention),
            collect_ceremony_evidence: required!(self, collect_ceremony_evidence),
            publish_ceremony_definition: required!(self, publish_ceremony_definition),
            diff_ceremony_definitions: required!(self, diff_ceremony_definitions),
            bind_ceremony_participants: required!(self, bind_ceremony_participants),
            ceremony_definitions: required!(self, ceremony_definitions, "port"),
            prepare_ceremony_participants: required!(self, prepare_ceremony_participants),
            contract_registry: required!(self, contract_registry, "port"),
            auto_dispatch: required!(self, auto_dispatch, "service"),
            statistics: required!(self, statistics, "port"),
            started_at: std::time::Instant::now(),
            service_version: self.service_version.unwrap_or(""),
        })
    }
}
