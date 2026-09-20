//! Shared construction for the agentic-system contract suites.
//!
//! Its own file so the three suites build the same design: suites that
//! each invented their own would agree with each other and with no
//! adapter in particular.

use time::OffsetDateTime;

use crate::entities::{AgenticSystem, AgenticSystemExecution, PublishedAgenticSystem};
use crate::error::DomainError;
use crate::value_objects::{
    AgenticSystemExecutionId, AgenticSystemId, AttentionPolicy, CeremonyActivation,
    CeremonyComposition, CeremonyDefinitionDigest, CeremonyExecutionLink, CeremonyName,
    CeremonyVersion, DefinitionPin, LogicalParticipant, ParticipantBindingPolicy, ParticipantId,
    ParticipantKind, ParticipantMaterialization, Responsibility, Specialty, SupervisionPolicy,
    SystemCeremonyId, SystemPurpose, SystemRole, SystemRoleId, SystemRoleKind,
};

use super::ConformanceFailure;

/// The instant every suite starts from, so nothing depends on the clock.
pub(super) fn origin() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

pub(super) fn failure(property: &'static str, detail: impl Into<String>) -> ConformanceFailure {
    ConformanceFailure::new(property, detail)
}

pub(super) fn call<T>(
    property: &'static str,
    outcome: Result<T, DomainError>,
) -> Result<T, ConformanceFailure> {
    outcome.map_err(|error| failure(property, format!("the adapter returned an error: {error}")))
}

/// An identifier of this property's own, so suites cannot collide in a
/// store they all share.
pub(super) fn system_id(property: &'static str) -> Result<AgenticSystemId, ConformanceFailure> {
    AgenticSystemId::new(property.replace('_', "-"))
        .map_err(|error| failure(property, error.to_string()))
}

pub(super) fn execution_id(
    property: &'static str,
    suffix: &str,
) -> Result<AgenticSystemExecutionId, ConformanceFailure> {
    AgenticSystemExecutionId::new(format!("{}-{suffix}", property.replace('_', "-")))
        .map_err(|error| failure(property, error.to_string()))
}

pub(super) fn pin(digest: u8) -> Result<DefinitionPin, DomainError> {
    Ok(DefinitionPin::new(
        CeremonyName::new("delivery")?,
        CeremonyVersion::new("1.0")?,
        CeremonyDefinitionDigest::from_bytes([digest; 32]),
    ))
}

/// A minimal but complete design. `purpose` is what two revisions of
/// it differ by, so a suite can tell an overwrite from a new revision.
pub(super) fn design(
    property: &'static str,
    id: &AgenticSystemId,
    purpose: &str,
) -> Result<AgenticSystem, ConformanceFailure> {
    build(id, purpose).map_err(|error| {
        failure(
            property,
            format!("the suite built an invalid design: {error}"),
        )
    })
}

fn build(id: &AgenticSystemId, purpose: &str) -> Result<AgenticSystem, DomainError> {
    let integrator = SystemRoleId::new("integrator")?;
    AgenticSystem::draft(
        id.clone(),
        SystemPurpose::new(purpose)?,
        integrator.clone(),
        [SystemRole::new(
            integrator.clone(),
            Responsibility::new("drives the system from outside it")?,
            SystemRoleKind::Integrator,
        )],
        [LogicalParticipant::new(
            ParticipantId::new("operator")?,
            integrator,
            ParticipantKind::Person,
            ParticipantBindingPolicy::default(),
        )],
        [],
        [],
        [CeremonyComposition::new(
            SystemCeremonyId::new("delivery")?,
            pin(0x0a)?,
            SystemPurpose::new("does the work")?,
            [],
            CeremonyActivation::Manual,
            [],
            [],
        )],
        SupervisionPolicy::default(),
        AttentionPolicy::default(),
        origin(),
    )
}

pub(super) fn sealed(
    property: &'static str,
    id: &AgenticSystemId,
    purpose: &str,
) -> Result<PublishedAgenticSystem, ConformanceFailure> {
    let design = design(property, id, purpose)?;
    let published = design
        .published(origin())
        .and_then(|design| PublishedAgenticSystem::seal(design, origin()));
    published.map_err(|error| failure(property, format!("the suite could not seal: {error}")))
}

pub(super) fn run(
    property: &'static str,
    suffix: &str,
    system: &PublishedAgenticSystem,
) -> Result<AgenticSystemExecution, ConformanceFailure> {
    let id = execution_id(property, suffix)?;
    let build = || -> Result<AgenticSystemExecution, DomainError> {
        AgenticSystemExecution::plan(
            id,
            system.pin(),
            [(
                SystemCeremonyId::new("delivery")?,
                CeremonyExecutionLink::pending(pin(0x0a)?),
            )],
            [],
            [(
                ParticipantId::new("operator")?,
                ParticipantMaterialization::bound(Specialty::new("operator")?),
            )],
            origin(),
        )
    };
    build().map_err(|error| failure(property, format!("the suite built an invalid run: {error}")))
}
