//! Value objects for the MADE domain.
//!
//! Every public function in the domain exchanges value objects instead
//! of primitives. Each value object validates its invariants on
//! construction and cannot be mutated afterwards.

mod agent_kind;
mod artifact;
mod attributes;
mod audit;
mod authorization;
mod budget;
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
mod execution;
mod execution_id;
mod execution_outcome;
mod execution_status;
mod finite_metric_value;
mod ids;
mod llm_error_kind;
mod memory;
mod metric_help;
mod metric_kind;
mod metric_label_name;
mod metric_label_value;
mod metric_name;
mod metric_value;
mod num_agents;
mod output_contract;
mod output_contract_id;
mod output_contract_validation;
mod output_field_rule;
mod output_format;
mod prometheus_text;
mod proposal_content;
mod recorder_name;
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
mod trace_id;
mod validation_mode;
mod validation_passed;

pub use agent_kind::AgentKind;
pub use artifact::{
    ArtifactDigest, ArtifactId, ArtifactImportRef, ArtifactMediaType, ArtifactProvenance,
    ArtifactRef, ArtifactSizeBytes, ArtifactSourceKind,
};
pub use attributes::Attributes;
pub use audit::{
    AuditActor, AuditActorId, AuditActorKind, AuditChainDefect, AuditChainVerdict, AuditEventType,
    AuditRecordHash, AuditSequence, CeremonyEventConsumer, CeremonyEventCursorAttempt,
    CeremonyEventCursorLease, CeremonyEventCursorLeaseId, CeremonyEventPageLimit,
    CeremonyEventQuarantineReason, CeremonyProgressWait, EventSchemaVersion, GlobalPosition,
    QuarantinedCeremonyEvent, StreamVersion,
};
pub use authorization::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecision,
    AuthorizationDecisionId, AuthorizationDecisionKind, AuthorizationDecisionPageLimit,
    AuthorizationDecisionPlan, AuthorizationDecisionTtl, AuthorizationDenialReason,
    AuthorizationEvidence, AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer,
    AuthorizationKeyRing, AuthorizationPolicyId, AuthorizationPolicyVersion, AuthorizationRequest,
    AuthorizationRequestId, AuthorizationRevocation, AuthorizationRevocationReason,
    AuthorizationScope, AuthorizationTargetDigest, AuthorizedOperation, CursorTransition,
    DelegationDepth, PrincipalId, PrincipalKind, SeparationRule, VerificationKey,
    VerificationKeyFingerprint, VerificationKeyId,
};
pub use budget::{
    BudgetAccountId, BudgetBalance, BudgetDimension, BudgetLedgerVersion, BudgetLimits,
    BudgetMeasurement, BudgetOperationId, BudgetPageLimit, BudgetQuantities,
    BudgetReconciliationId, BudgetReservation, BudgetReservationEstimate, BudgetReservationId,
    BudgetReservationPolicy, BudgetReservationPolicyVersion, BudgetReservationRequest,
    BudgetTokenCount, CostMicros, CurrencyCode, ExecutionDuration, MeasuredBudgetQuantities,
    ToolCallCount,
};
pub use ceremony::{
    CeremonyChangeImpact, CeremonyChangeKind, CeremonyChildSpawn, CeremonyChildSpec,
    CeremonyContext, CeremonyDeadline, CeremonyDefinitionChange, CeremonyDefinitionDiff,
    CeremonyDefinitionDigest, CeremonyDescription, CeremonyEndReason, CeremonyEvidenceSourceId,
    CeremonyGuard, CeremonyGuardApproval, CeremonyGuardDeferral, CeremonyGuardDeferralContent,
    CeremonyId, CeremonyInputDefinition, CeremonyInterventionContent, CeremonyInterventionId,
    CeremonyInterventionKind, CeremonyInterventionProvenance, CeremonyInterventionResponse,
    CeremonyInterventionStatus, CeremonyInterventionTarget, CeremonyLifecycle,
    CeremonyLifecyclePhase, CeremonyLineage, CeremonyName, CeremonyOutputDefinition,
    CeremonyParticipantBinding, CeremonyReason, CeremonyReasonKind, CeremonyReasonRationale,
    CeremonyRecordRef, CeremonyRevision, CeremonyRole, CeremonyState, CeremonyStateKind,
    CeremonyStep, CeremonyStepAggregation, CeremonyStepContribution, CeremonyTimeout,
    CeremonyTranscript, CeremonyTransition, CeremonyTransitionRecord, CeremonyValidationFinding,
    CeremonyValidationLocus, CeremonyValidationReport, CeremonyValidationSeverity, CeremonyVersion,
    ChildCeremonyId, ChildCompletionRef, ChildDepth, ChildDepthBudget, ChildGroupId,
    ChildGroupState, ChildJoin, ChildPosition, ChildQuorum, ChildSpawnCoordinates, ChildSpawnPlan,
    ChildrenCompletedCondition, ContextKey, ContextPatch, ContextWrites, DynamicRoleBinding,
    GuardCondition, GuardName, IdempotencyKey, InputName, InputRequirement, InterventionRoleIds,
    JoinStepCount, LateStepResult, LeaseOwnerId, LifecycleReason, MaxBounces, MaxChildDepth,
    MaxChildren, MaxParallel, MaxTransitions, OutputFieldGuardCondition, OutputName, PlannedChild,
    PriorContext, ReasonAsserter, ReconsiderationConditions, RepeatUntilCondition, RetryPolicy,
    RoleAction, RoleId, StateDeadline, StateExecution, StateId, StateIteration, StateRepeatPolicy,
    StateRepeatUntilCondition, StateTimeout, StateVisit, StepAttempt, StepClaimFence, StepDeadline,
    StepErrorMessage, StepExecutionRecord, StepFailureKind, StepHandlerConfig, StepHandlerKind,
    StepId, StepInstructions, StepIteration, StepLease, StepOutput, StepOutputField,
    StepRepeatExhaustedGuardCondition, StepRepeatPolicy, StepResult, StepStatus, StepTimeout,
    TransitionTrigger,
};
pub use ceremony::{CeremonyIdPrefix, CeremonyInstancePageLimit};
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
pub use execution::{
    ExecutionConnectorId, ExecutionIntent, ExecutionOperation, ExecutionOperationId,
    ExecutionReceipt, ExecutionReceiptId, ExecutionReceiptLink, ExecutionReceiptLinkKind,
    ExecutionReconciliationRequirement, ExecutionRecoveryCapability, ExecutionRecoveryCursor,
    ExecutionRecoveryPageLimit, ExecutionRequestBytes, ExecutionRequestDigest, ExternalOperationId,
    MAX_EXECUTION_ARTIFACTS,
};
pub use execution_id::ExecutionId;
pub use execution_outcome::ExecutionOutcome;
pub use execution_status::ExecutionStatus;
pub use finite_metric_value::FiniteMetricValue;
pub use ids::{AgentId, CouncilId, EventId, ProposalId, TaskId};
pub use llm_error_kind::LlmErrorKind;
pub use memory::{
    MemoryCapabilities, MemoryCapability, MemoryConfidence, MemoryEntry, MemoryEntryId,
    MemoryEntryKind, MemoryEvidence, MemoryMoment, MemoryProvenance, MemoryRelation,
    MemoryRelationKind, MemoryScope, MemoryWrite, RecalledEntry, RecollectionBudget,
    RecollectionCompleteness, SessionRecollection,
};
pub use metric_help::MetricHelp;
pub use metric_kind::MetricKind;
pub use metric_label_name::MetricLabelName;
pub use metric_label_value::MetricLabelValue;
pub use metric_name::MetricName;
pub use metric_value::MetricValue;
pub use num_agents::NumAgents;
pub use output_contract::OutputContract;
pub use output_contract_id::OutputContractId;
pub use output_field_rule::OutputFieldRule;
pub use output_format::OutputFormat;
pub use prometheus_text::PrometheusText;
pub use proposal_content::ProposalContent;
pub use recorder_name::RecorderName;
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
pub use trace_id::TraceId;
pub use validation_mode::ValidationMode;
pub use validation_passed::ValidationPassed;

mod council_journal_position;
pub use council_journal_position::CouncilJournalPosition;

mod council_journal_consumer;
pub use council_journal_consumer::CouncilJournalConsumer;

mod council_journal_lease_id;
pub use council_journal_lease_id::CouncilJournalLeaseId;

mod council_journal_page_limit;
pub use council_journal_page_limit::CouncilJournalPageLimit;

mod council_journal_lease;
pub use council_journal_lease::CouncilJournalLease;

mod council_snapshot_source;
pub use council_snapshot_source::CouncilSnapshotSource;
