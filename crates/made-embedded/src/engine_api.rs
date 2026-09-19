//! [`EmbeddedMade`] as an implementation of the published contract.
//!
//! The conversion in this module is the whole of the coupling a consumer is
//! allowed: domain aggregate in, plain view out. Nothing of `made-core`
//! crosses the trait.

use std::collections::BTreeMap;

use made_adapters::yaml::CeremonyDefinitionYaml;
use made_api::{
    ApiCapabilities, ApiError, BudgetLimits as BudgetLimitsView, BudgetReport,
    BudgetReservation as BudgetReservationView, CeremonyEngineApi, CeremonyParticipant,
    CeremonySummary, DefinitionAnalysisView, DefinitionDefectView, InterventionResponseView,
    InterventionView, PublishedDefinitionView, RaiseInterventionRequest, RecalledEntryView,
    RecollectionView, RespondToInterventionRequest, StartCeremonyRequest, CONTRACT_VERSION,
};
use made_app::usecases::{
    RequestCeremonyInterventionInput, RespondToCeremonyInterventionInput, StartCeremonyInput,
};
use made_core::entities::CeremonyInstance;
use made_core::entities::CeremonyIntervention;
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, AuditActorKind, AuthorizationAction, CeremonyContext, CeremonyId,
    CeremonyInterventionContent, CeremonyInterventionId, CeremonyInterventionKind,
    CeremonyInterventionTarget, CeremonyName, CeremonyVersion, RoleId,
};
use time::OffsetDateTime;

use crate::{EmbeddedMade, VERSION};

mod budget_api;

/// What this build can do, by name.
///
/// Listed here, next to the implementation, so that adding a method to the
/// trait without adding its name is a diff a reviewer sees in one place.
const CAPABILITIES: [&str; 10] = [
    "list_ceremonies",
    "get_ceremony",
    "start_ceremony",
    "raise_intervention",
    "respond_to_intervention",
    "analyze_definition",
    "publish_definition",
    "start_budgeted_ceremony",
    "get_budget_report",
    "list_pending_budget_reservations",
];

#[async_trait::async_trait]
impl CeremonyEngineApi for EmbeddedMade {
    fn capabilities(&self) -> ApiCapabilities {
        ApiCapabilities::new(CONTRACT_VERSION, VERSION, CAPABILITIES)
    }

    async fn ceremonies(&self) -> Result<Vec<CeremonySummary>, ApiError> {
        let instances = self
            .instances()
            .await
            .map_err(|error| unavailable(&error))?;
        Ok(instances.iter().map(summarize).collect())
    }

    async fn ceremony(&self, ceremony_id: &str) -> Result<CeremonySummary, ApiError> {
        let id = CeremonyId::new(ceremony_id).map_err(|error| ApiError::Refused {
            reason: error.to_string(),
        })?;
        match self.instance(&id).await {
            Ok(instance) => Ok(summarize(&instance)),
            Err(DomainError::NotFound { .. }) => Err(ApiError::CeremonyNotFound {
                ceremony_id: ceremony_id.to_owned(),
            }),
            Err(error) => Err(unavailable(&error)),
        }
    }

    async fn raise_intervention(
        &self,
        request: RaiseInterventionRequest,
    ) -> Result<CeremonySummary, ApiError> {
        let input = raise_input(request).map_err(|error| ApiError::Refused {
            reason: error.to_string(),
        })?;
        match self.request_intervention(input).await {
            Ok(instance) => Ok(summarize(&instance)),
            Err(DomainError::NotFound { .. }) => Err(ApiError::CeremonyNotFound {
                ceremony_id: "the ceremony the intervention names".to_owned(),
            }),
            // A lost race, not a refusal: reading the session again and
            // repeating the call is the remedy, which is the one thing a
            // refusal never is.
            Err(DomainError::Conflict { what }) => Err(ApiError::Conflict {
                what: what.to_owned(),
            }),
            Err(error) => Err(ApiError::Refused {
                reason: error.to_string(),
            }),
        }
    }

    async fn respond_to_intervention(
        &self,
        request: RespondToInterventionRequest,
    ) -> Result<CeremonySummary, ApiError> {
        let input = respond_input(request).map_err(|error| ApiError::Refused {
            reason: error.to_string(),
        })?;
        match self.respond_to_intervention(input).await {
            Ok(instance) => Ok(summarize(&instance)),
            Err(DomainError::NotFound { .. }) => Err(ApiError::CeremonyNotFound {
                ceremony_id: "the ceremony the intervention names".to_owned(),
            }),
            // A lost race, not a refusal: reading the session again and
            // repeating the call is the remedy, which is the one thing a
            // refusal never is.
            Err(DomainError::Conflict { what }) => Err(ApiError::Conflict {
                what: what.to_owned(),
            }),
            Err(error) => Err(ApiError::Refused {
                reason: error.to_string(),
            }),
        }
    }

    async fn analyze_definition(
        &self,
        definition_yaml: &str,
    ) -> Result<DefinitionAnalysisView, ApiError> {
        self.require_authorized_action(AuthorizationAction::ValidateCeremonyDraft)
            .map_err(|error| ApiError::Refused {
                reason: error.to_string(),
            })?;
        let draft = CeremonyDefinitionYaml::parse_draft_str(definition_yaml).map_err(|error| {
            ApiError::Refused {
                reason: format!("that is not a definition at all: {error}"),
            }
        })?;
        self.require_authorized_definition_action(
            AuthorizationAction::ValidateCeremonyDraft,
            draft.name(),
            Some(draft.version()),
        )
        .map_err(|error| ApiError::Refused {
            reason: error.to_string(),
        })?;
        let report = draft.analyze();
        let publishable = report.is_valid();
        let definition_digest = if publishable {
            let definition = draft.clone().publish().map_err(|error| ApiError::Refused {
                reason: format!(
                    "the analyzed definition could not derive its publication identity: {error}"
                ),
            })?;
            Some(
                definition
                    .digest()
                    .map_err(|error| ApiError::Refused {
                        reason: format!(
                            "the analyzed definition could not derive its publication identity: {error}"
                        ),
                    })?
                    .to_hex(),
            )
        } else {
            None
        };
        Ok(DefinitionAnalysisView {
            definition_name: draft.name().as_str().to_owned(),
            definition_version: draft.version().as_str().to_owned(),
            publishable,
            definition_digest,
            defects: report
                .findings()
                .iter()
                .map(|finding| DefinitionDefectView {
                    severity: match finding.severity() {
                        made_core::value_objects::CeremonyValidationSeverity::Error => {
                            "error".to_owned()
                        }
                        made_core::value_objects::CeremonyValidationSeverity::Warning => {
                            "warning".to_owned()
                        }
                    },
                    locus: finding.locus().to_string(),
                    defect: finding.defect().to_string(),
                    blocking: finding.is_blocking(),
                })
                .collect(),
        })
    }

    async fn publish_definition(
        &self,
        definition_yaml: &str,
    ) -> Result<PublishedDefinitionView, ApiError> {
        let definition = CeremonyDefinitionYaml::parse_str(definition_yaml).map_err(|error| {
            ApiError::Refused {
                reason: format!("the definition does not construct: {error}"),
            }
        })?;
        match self.publish_definition(definition).await {
            Ok(made_core::entities::PublicationOutcome::Published(published)) => {
                Ok(published_view(&published, false))
            }
            Ok(made_core::entities::PublicationOutcome::AlreadyPublished(published)) => {
                Ok(published_view(&published, true))
            }
            Ok(made_core::entities::PublicationOutcome::VersionOccupied { published, offered }) => {
                Err(ApiError::Refused {
                    reason: format!(
                        "that name and version already publish {}, not {}; a \
                     published version is immutable — publish a new version",
                        published.to_hex(),
                        offered.to_hex()
                    ),
                })
            }
            Err(error) => Err(ApiError::Refused {
                reason: error.to_string(),
            }),
        }
    }

    async fn start_ceremony(
        &self,
        request: StartCeremonyRequest,
    ) -> Result<CeremonySummary, ApiError> {
        let ceremony_id = request.ceremony_id.clone();
        let input = start_input(request).map_err(|error| ApiError::Refused {
            reason: error.to_string(),
        })?;
        match self.start_published(input).await {
            Ok(instance) => Ok(summarize(&instance)),
            // Nothing published under that name and version. Publishing is the
            // remedy, not retrying.
            Err(DomainError::NotFound { .. }) => Err(ApiError::CeremonyNotFound {
                ceremony_id: format!("no published definition for `{ceremony_id}`"),
            }),
            // A lost race, which is worth repeating once the session has
            // been read again.
            Err(DomainError::Conflict { what }) => Err(ApiError::Conflict {
                what: what.to_owned(),
            }),
            // Everything else the domain says here is about the request — a
            // taken identity, a defective field. Refused, so nobody retries an
            // answer that will not change.
            Err(error) => Err(ApiError::Refused {
                reason: error.to_string(),
            }),
        }
    }

    async fn start_budgeted_ceremony(
        &self,
        request: StartCeremonyRequest,
        limits: BudgetLimitsView,
    ) -> Result<CeremonySummary, ApiError> {
        budget_api::start(self, request, limits).await
    }

    async fn budget_report(&self, ceremony_id: &str) -> Result<BudgetReport, ApiError> {
        budget_api::report(self, ceremony_id).await
    }

    async fn pending_budget_reservations(
        &self,
        after_reservation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<BudgetReservationView>, ApiError> {
        budget_api::pending(self, after_reservation_id, limit).await
    }
}

/// Parse the plain request into the domain's terms — the one place a consumer's
/// strings meet the engine's validation.
fn start_input(request: StartCeremonyRequest) -> Result<StartCeremonyInput, DomainError> {
    let actor_kind = parse_actor_kind(&request.actor_kind)?;
    Ok(StartCeremonyInput::new(
        CeremonyId::new(request.ceremony_id)?,
        CeremonyName::new(request.definition_name)?,
        CeremonyVersion::new(request.definition_version)?,
        CeremonyContext::new(Attributes::new(BTreeMap::from_iter(request.context))?),
        request.actor_id,
        actor_kind,
    ))
}

fn summarize(instance: &CeremonyInstance) -> CeremonySummary {
    CeremonySummary {
        ceremony_id: instance.id().as_str().to_owned(),
        definition_name: instance.definition_name().as_str().to_owned(),
        definition_version: instance.definition_version().as_str().to_owned(),
        definition_digest: instance
            .bound_definition()
            .map(made_core::value_objects::CeremonyDefinitionDigest::to_hex),
        current_state: instance.current_state().as_str().to_owned(),
        interventions: instance
            .interventions()
            .iter()
            .map(intervention_view)
            .collect(),
        participants: instance
            .participant_bindings()
            .values()
            .map(|binding| CeremonyParticipant {
                role_id: binding.role_id().as_str().to_owned(),
                specialty: binding.specialty().as_str().to_owned(),
                bound_at_millis: millis(binding.bound_at()),
            })
            .collect(),
        context: instance.context().attributes().as_map().clone(),
        recollection: instance.recollection().map(recollection_view),
        created_at_millis: millis(instance.created_at()),
        updated_at_millis: millis(instance.updated_at()),
        completed_at_millis: instance.completed_at().map(millis),
    }
}

/// What earlier sessions decided, as a consumer sees it.
///
/// Absent rather than empty when the ceremony was told nothing: a
/// consumer that read an empty list would have no way to tell a scope
/// nobody has written to from a ceremony that shares no memory at all.
fn recollection_view(
    recollection: &made_core::value_objects::SessionRecollection,
) -> RecollectionView {
    RecollectionView {
        scope: recollection.scope().as_str().to_owned(),
        entries: recollection
            .entries()
            .iter()
            .map(|entry| RecalledEntryView {
                entry_id: entry.id().as_str().to_owned(),
                kind: entry.kind().as_label().to_owned(),
                summary: entry.summary().to_owned(),
                from_ceremony_id: entry.from_ceremony().as_str().to_owned(),
                observed_at_millis: millis(entry.observed_at()),
            })
            .collect(),
        truncated: recollection.completeness().is_truncated(),
    }
}

fn published_view(
    published: &made_core::entities::PublishedCeremonyDefinition,
    already_published: bool,
) -> PublishedDefinitionView {
    PublishedDefinitionView {
        name: published.name().as_str().to_owned(),
        version: published.version().as_str().to_owned(),
        digest: published.digest().to_hex(),
        already_published,
    }
}

mod mappings;

use mappings::{
    intervention_view, millis, parse_actor_kind, raise_input, respond_input, unavailable,
};
