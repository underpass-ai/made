use crate::council_data_snapshot::CouncilDataSnapshot;
use made_core::entities::{Council, Deliberation, Proposal, ValidatorReport};
use made_core::error::DomainError;
use made_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, EvidenceGroundingRule, OutputContract,
    OutputFieldRule, ProposalId, Rounds, Score, SemanticSupportRule, Specialty, TaskId,
};
use std::collections::{BTreeMap, BTreeSet};

/// Legacy domain values predate validating Deserialize implementations. At
/// this new file boundary, re-run their constructors without rewriting data.
pub(crate) fn validate(snapshot: &CouncilDataSnapshot) -> Result<(), DomainError> {
    for council in &snapshot.councils {
        let members = council
            .agents()
            .iter()
            .map(|id| AgentId::new(id.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        same(
            council,
            &Council::new(
                CouncilId::new(council.id().as_str())?,
                Specialty::new(council.specialty().as_str())?,
                members,
                council.created_at(),
            )?,
        )?;
    }
    for agent in &snapshot.agents {
        same(&agent.id, &AgentId::new(agent.id.as_str())?)?;
        same(&agent.specialty, &Specialty::new(agent.specialty.as_str())?)?;
        same(&agent.kind, &AgentKind::new(agent.kind.as_str())?)?;
        crate::persisted_agent_descriptor::validate(agent)?;
    }
    for contract in &snapshot.contracts {
        validate_contract(contract)?;
    }
    for deliberation in &snapshot.deliberations {
        validate_deliberation(deliberation)?;
    }
    for specialty in snapshot.statistics.per_specialty().keys() {
        same(specialty, &Specialty::new(specialty.as_str())?)?;
    }
    Ok(())
}
fn validate_contract(contract: &OutputContract) -> Result<(), DomainError> {
    let fields = contract
        .fields()
        .iter()
        .map(|(name, rule)| {
            Ok((
                name.clone(),
                OutputFieldRule::new(
                    rule.required(),
                    rule.allowed_string_values().iter().cloned(),
                )?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, DomainError>>()?;
    let mut validated = OutputContract::new_with_schema(
        contract.contract_id().as_str(),
        contract.format(),
        fields,
        contract.json_schema(),
    )?;
    if let Some(rule) = contract.evidence_grounding() {
        let mut grounding = EvidenceGroundingRule::new(
            rule.claims_field(),
            rule.refs_field(),
            rule.allowed_refs().iter().cloned(),
        )?;
        if let Some(support) = rule.semantic_support() {
            grounding = grounding.with_semantic_support(SemanticSupportRule::new(
                support.min_confidence(),
                support.bodies().clone(),
            )?)?;
        }
        validated = validated.with_evidence_grounding(grounding);
    }
    same(contract, &validated)
}
fn validate_deliberation(deliberation: &Deliberation) -> Result<(), DomainError> {
    same(
        deliberation.task_id(),
        &TaskId::new(deliberation.task_id().as_str())?,
    )?;
    same(
        deliberation.specialty(),
        &Specialty::new(deliberation.specialty().as_str())?,
    )?;
    Rounds::new(deliberation.rounds_budget().get())?;
    for (key, proposal) in deliberation.proposals() {
        same(key, proposal.id())?;
        same(key, &ProposalId::new(key.as_str())?)?;
        same(
            proposal.author(),
            &AgentId::new(proposal.author().as_str())?,
        )?;
        same(
            proposal.specialty(),
            &Specialty::new(proposal.specialty().as_str())?,
        )?;
        Proposal::new(
            proposal.id().clone(),
            proposal.author().clone(),
            proposal.specialty().clone(),
            proposal.content().clone(),
            Attributes::new(proposal.attributes().as_map().clone())?,
            proposal.created_at(),
        )?;
    }
    for (key, outcome) in deliberation.outcomes() {
        if !deliberation.proposals().contains_key(key) {
            return Err(invalid());
        }
        Score::new(outcome.score().get())?;
        for report in outcome.reports() {
            ValidatorReport::new(
                report.kind(),
                report.passed(),
                report.summary(),
                Attributes::new(report.details().as_map().clone())?,
            )?;
        }
    }
    let mut seen = BTreeSet::new();
    for key in deliberation.ranking() {
        if !seen.insert(key)
            || !deliberation.proposals().contains_key(key)
            || !deliberation.outcomes().contains_key(key)
        {
            return Err(invalid());
        }
    }
    Ok(())
}
fn same<T: PartialEq>(original: &T, validated: &T) -> Result<(), DomainError> {
    if original == validated {
        Ok(())
    } else {
        Err(invalid())
    }
}
fn invalid() -> DomainError {
    DomainError::InvariantViolated {
        reason: "council snapshot contains invalid or noncanonical domain data",
    }
}
