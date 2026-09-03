//! Value objects for the MADE domain.
//!
//! Every public function in the domain exchanges value objects instead
//! of primitives. Each value object validates its invariants on
//! construction and cannot be mutated afterwards.

mod agent_kind;
mod attributes;
mod audit;
mod ceremony;
mod ceremony_outcome;
mod claim_text;
mod council_contract_id;
mod council_selector;
mod critique_feedback;
mod deliberation_outcome;
mod discrimination;
mod diversity_preference;
mod duration;
mod evidence_body;
mod evidence_excerpt;
mod evidence_grounding_rule;
mod evidence_reference;
mod execution_id;
mod execution_outcome;
mod execution_status;
mod ids;
mod llm_error_kind;
mod memory;
mod num_agents;
mod outbox;
mod output_contract;
mod output_contract_id;
mod output_contract_validation;
mod output_field_rule;
mod output_format;
mod proposal_content;
mod rounds;
mod rubric;
mod score;
mod scoring_mode;
mod semantic_support_rule;
mod specialty;
mod support_confidence;
mod support_decision;
mod support_rationale;
mod support_verdict;
mod task_description;
mod token_usage;
mod trace_context;
mod validation_mode;

pub use agent_kind::AgentKind;
pub use attributes::Attributes;
pub use audit::{
    AuditActor, AuditActorKind, AuditChainDefect, AuditChainVerdict, AuditEventType,
    AuditRecordHash, AuditSequence,
};
pub use ceremony::{
    CeremonyChangeImpact, CeremonyChangeKind, CeremonyContext, CeremonyDefinitionChange,
    CeremonyDefinitionDiff, CeremonyDefinitionDigest, CeremonyDefinitionDigestMigration,
    CeremonyDescription, CeremonyEvidenceSourceId, CeremonyGuard, CeremonyGuardApproval,
    CeremonyGuardDeferral, CeremonyGuardDeferralContent, CeremonyId, CeremonyInputDefinition,
    CeremonyInterventionContent, CeremonyInterventionId, CeremonyInterventionKind,
    CeremonyInterventionProvenance, CeremonyInterventionResponse, CeremonyInterventionStatus,
    CeremonyInterventionTarget, CeremonyName, CeremonyOutputDefinition, CeremonyParticipantBinding,
    CeremonyReason, CeremonyReasonKind, CeremonyRecordRef, CeremonyRevision, CeremonyRole,
    CeremonyState, CeremonyStateKind, CeremonyStep, CeremonyStepContribution, CeremonyTranscript,
    CeremonyTransition, CeremonyTransitionRecord, CeremonyValidationFinding,
    CeremonyValidationLocus, CeremonyValidationReport, CeremonyValidationSeverity, CeremonyVersion,
    ExpectedRevision, GuardCondition, GuardName, IdempotencyKey, InputName, InputRequirement,
    LeaseOwnerId, OutputName, ReasonAsserter, RepeatUntilCondition, RetryPolicy, RoleAction,
    RoleId, StateId, StepAttempt, StepErrorMessage, StepExecutionRecord, StepHandlerConfig,
    StepHandlerKind, StepId, StepIteration, StepLease, StepOutput, StepOutputField,
    StepRepeatPolicy, StepResult, StepStatus, StepTimeout, TransitionTrigger,
};
pub use ceremony_outcome::CeremonyOutcome;
pub use claim_text::ClaimText;
pub use council_contract_id::CouncilContractId;
pub use council_selector::CouncilSelector;
pub use critique_feedback::CritiqueFeedback;
pub use deliberation_outcome::DeliberationOutcome;
pub use discrimination::Discrimination;
pub use diversity_preference::DiversityPreference;
pub use duration::DurationMs;
pub use evidence_body::EvidenceBody;
pub use evidence_excerpt::EvidenceExcerpt;
pub use evidence_grounding_rule::EvidenceGroundingRule;
pub use evidence_reference::EvidenceReference;
pub use execution_id::ExecutionId;
pub use execution_outcome::ExecutionOutcome;
pub use execution_status::ExecutionStatus;
pub use ids::{AgentId, CouncilId, EventId, ProposalId, TaskId};
pub use llm_error_kind::LlmErrorKind;
pub use memory::{
    MemoryCapabilities, MemoryCapability, MemoryConfidence, MemoryDimension, MemoryEntry,
    MemoryEntryId, MemoryEntryKind, MemoryEvidence, MemoryMoment, MemoryProvenance, MemoryQuestion,
    MemoryRelation, MemoryRelationKind, MemoryScope, MemoryWrite,
};
pub use num_agents::NumAgents;
pub use outbox::{
    ClaimedOutboxMessage, OutboxAttempt, OutboxMessage, OutboxQuarantineReason, OutboxSubject,
};
pub use output_contract::OutputContract;
pub use output_contract_id::OutputContractId;
pub use output_field_rule::OutputFieldRule;
pub use output_format::OutputFormat;
pub use proposal_content::ProposalContent;
pub use rounds::Rounds;
pub use rubric::Rubric;
pub use score::Score;
pub use scoring_mode::ScoringMode;
pub use semantic_support_rule::{SemanticSupportRule, DEFAULT_SUPPORT_MIN_CONFIDENCE};
pub use specialty::Specialty;
pub use support_confidence::SupportConfidence;
pub use support_decision::SupportDecision;
pub use support_rationale::SupportRationale;
pub use support_verdict::SupportVerdict;
pub use task_description::TaskDescription;
pub use token_usage::TokenUsage;
pub use trace_context::TraceContext;
pub use validation_mode::ValidationMode;
