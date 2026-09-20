use time::{Duration, OffsetDateTime};

use crate::entities::ceremony_events::{
    CeremonyInstanceStarted, MemoryRecalled, SuccessionCarried,
};
use crate::entities::{
    CeremonyDefinition, CeremonyEvent, CeremonyInstance, PublishedCeremonyDefinition,
};
use crate::error::DomainError;
use crate::value_objects::{
    BudgetAccountId, CeremonyContext, CeremonyDeadline, CeremonyDefinitionDigest, CeremonyId,
    CeremonyLineage, CeremonySuccession, SessionRecollection, StateDeadline, StateVisit,
    SuccessionPlan,
};

impl CeremonyInstance {
    /// The opening of a ceremony run from a definition supplied for it.
    ///
    /// A constructor rather than a command: there is no instance yet
    /// to decide against. Required inputs are checked against this run's
    /// context before any opening event is built. The event carries everything
    /// [`Self::from_started`] needs to open the same instance without
    /// the definition in hand.
    ///
    /// A batch rather than one event, because an opening is sometimes
    /// two facts: what was started, and what it was told. See
    /// [`Self::opening_batch`].
    pub fn decide_start(
        id: CeremonyId,
        definition: &CeremonyDefinition,
        context: CeremonyContext,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        Ok(Self::opening_batch(
            Self::opening(id, definition, context, now, None, None, None)?,
            recollection,
            now,
        ))
    }

    /// The opening of a ceremony bound to a published definition, its
    /// digest recorded so a later reader can check which one ran.
    pub fn decide_start_bound(
        id: CeremonyId,
        published: &PublishedCeremonyDefinition,
        context: CeremonyContext,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        Ok(Self::opening_batch(
            Self::opening(
                id,
                published.definition(),
                context,
                now,
                Some(published.digest()),
                None,
                None,
            )?,
            recollection,
            now,
        ))
    }

    /// Open a published child using the exact recollection and lineage sealed by its parent plan.
    pub fn decide_start_bound_child(
        id: CeremonyId,
        published: &PublishedCeremonyDefinition,
        context: CeremonyContext,
        lineage: CeremonyLineage,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        Ok(Self::opening_batch(
            Self::opening(
                id,
                published.definition(),
                context,
                now,
                Some(published.digest()),
                Some(lineage),
                None,
            )?,
            recollection,
            now,
        ))
    }

    /// Open a root ceremony and seal the shared ledger account for its whole tree.
    pub fn decide_start_bound_budgeted(
        id: CeremonyId,
        published: &PublishedCeremonyDefinition,
        context: CeremonyContext,
        budget_account_id: BudgetAccountId,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        Ok(Self::opening_batch(
            Self::opening(
                id,
                published.definition(),
                context,
                now,
                Some(published.digest()),
                None,
                Some(budget_account_id),
            )?,
            recollection,
            now,
        ))
    }

    /// Open a child with the exact shared ledger account sealed by its parent plan.
    pub fn decide_start_bound_budgeted_child(
        id: CeremonyId,
        published: &PublishedCeremonyDefinition,
        context: CeremonyContext,
        lineage: CeremonyLineage,
        budget_account_id: BudgetAccountId,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        Ok(Self::opening_batch(
            Self::opening(
                id,
                published.definition(),
                context,
                now,
                Some(published.digest()),
                Some(lineage),
                Some(budget_account_id),
            )?,
            recollection,
            now,
        ))
    }

    /// Open a successor: the opening the plan settled, and what it was
    /// given to start from.
    ///
    /// The second append of a succession, and the one that may be made
    /// twice. Everything here is derived from facts already sealed in
    /// the predecessor, so the batch a retry builds is byte for byte
    /// the batch the first attempt built — which is what lets a caller
    /// that crashed between the two appends verify the existing stream
    /// instead of opening a second one.
    ///
    /// The successor seals its own deadlines from its own definition.
    /// Inheriting the predecessor's would carry a clock the new
    /// definition never agreed to.
    pub fn decide_start_successor(
        id: CeremonyId,
        published: &PublishedCeremonyDefinition,
        context: CeremonyContext,
        succession: CeremonySuccession,
        plan: &SuccessionPlan,
        now: OffsetDateTime,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        let mut opening = Self::opening(
            id,
            published.definition(),
            context,
            now,
            Some(published.digest()),
            None,
            None,
        )?;
        opening.succession = Some(Box::new(succession));
        let mut events = vec![CeremonyEvent::CeremonyInstanceStarted(opening)];
        if !plan.carried().is_empty() {
            events.push(CeremonyEvent::SuccessionCarried(SuccessionCarried {
                carried: plan.carried().to_vec(),
                carried_at: now,
            }));
        }
        Ok(events)
    }

    /// The opening, and the recollection when there was one.
    ///
    /// The recollection is decided outside — reading memory is IO, and
    /// `decide` stays pure — and handed in. What is decided here is
    /// whether it becomes a fact: a recollection that came back empty
    /// appends nothing, so a session with nothing to recall, which is
    /// every session that declares no scope, has exactly the stream it
    /// had before memory could be read at all.
    fn opening_batch(
        opening: CeremonyInstanceStarted,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Vec<CeremonyEvent> {
        let mut events = vec![CeremonyEvent::CeremonyInstanceStarted(opening)];
        if let Some(recollection) = recollection.filter(|recalled| !recalled.is_empty()) {
            events.push(CeremonyEvent::MemoryRecalled(MemoryRecalled {
                recollection,
                recalled_at: now,
            }));
        }
        events
    }

    /// What starting derives from the definition: the initial state
    /// and the steps that get a pending record.
    pub(in crate::entities::ceremony_instance) fn opening(
        id: CeremonyId,
        definition: &CeremonyDefinition,
        context: CeremonyContext,
        now: OffsetDateTime,
        bound_definition: Option<CeremonyDefinitionDigest>,
        lineage: Option<CeremonyLineage>,
        budget_account_id: Option<BudgetAccountId>,
    ) -> Result<CeremonyInstanceStarted, DomainError> {
        let missing = definition
            .inputs()
            .values()
            .filter(|input| input.requirement().is_required())
            .filter(|input| context.attributes().get(input.name().as_str()).is_none())
            .map(|input| input.name().as_str())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(DomainError::InvalidDocument {
                reason: format!("missing required ceremony inputs: {}", missing.join(", ")),
            });
        }
        let ceremony_deadline = definition
            .ceremony_timeout()
            .map(|timeout| checked_deadline(now, timeout.duration(), "ceremony_deadline"))
            .transpose()?
            .map(CeremonyDeadline::new);
        let state_deadline = definition
            .state_timeout()
            .map(|timeout| checked_deadline(now, timeout.duration(), "state_deadline"))
            .transpose()?
            .map(|at| {
                StateDeadline::new(definition.initial_state_id().clone(), StateVisit::FIRST, at)
            });
        Ok(CeremonyInstanceStarted {
            ceremony_id: id,
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            initial_state: definition.initial_state_id().clone(),
            step_ids: definition.steps().keys().cloned().collect(),
            context,
            bound_definition,
            lineage,
            succession: None,
            budget_account_id,
            ceremony_deadline,
            state_deadline,
            created_at: now,
        })
    }
}

pub(crate) fn checked_deadline(
    now: OffsetDateTime,
    duration: crate::value_objects::DurationMs,
    field: &'static str,
) -> Result<OffsetDateTime, DomainError> {
    let millis = i64::try_from(duration.get()).map_err(|_| DomainError::OutOfRange {
        field,
        value: duration.get() as f64,
        min: 0.0,
        max: i64::MAX as f64,
    })?;
    now.checked_add(Duration::milliseconds(millis))
        .ok_or(DomainError::OutOfRange {
            field,
            value: duration.get() as f64,
            min: 0.0,
            max: i64::MAX as f64,
        })
}
