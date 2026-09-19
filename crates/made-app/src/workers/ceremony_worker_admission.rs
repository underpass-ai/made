use std::fmt;

use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyId, ExecutionOperationId, IdempotencyKey, LeaseOwnerId, StepClaimFence,
};
use time::OffsetDateTime;

/// A small, typed priority used by the worker scheduler. Larger values run first
/// within a root; roots are still shared by weighted deficit round robin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerPriority(u16);

impl CeremonyWorkerPriority {
    pub const DEFAULT: Self = Self(0);
    pub const MAX: u16 = 1_000;

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "worker_priority",
                value: f64::from(value),
                min: 0.0,
                max: f64::from(Self::MAX),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Work cost used by WDRR. Zero is deliberately not a valid implicit cost:
/// callers must make a real estimate before asking the scheduler to admit work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerCost(u32);

impl CeremonyWorkerCost {
    pub const MAX: u32 = 100_000;

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "worker_cost",
            });
        }
        if value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "worker_cost",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(Self::MAX),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Capacity requested by one unit of work. It is reported, not inferred from
/// a budget ledger by this scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerCapacity(u32);

impl CeremonyWorkerCapacity {
    pub const MAX: u32 = 100_000;

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "worker_capacity",
            });
        }
        if value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "worker_capacity",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(Self::MAX),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// A positive root weight for weighted deficit round robin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerWeight(u32);

impl CeremonyWorkerWeight {
    pub const MAX: u32 = 1_000;

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "worker_weight",
            });
        }
        if value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "worker_weight",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(Self::MAX),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Version stamped on every scheduler decision. It is supplied by the host,
/// never generated from time or from a request payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerPolicyVersion(u64);

impl CeremonyWorkerPolicyVersion {
    pub fn new(value: u64) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "worker_policy_version",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Admission facts that must come from the owning use case or policy store.
/// The scheduler does not derive either permission or remaining budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyWorkerEligibility {
    Ready,
    BudgetUnavailable,
    PermissionDenied,
}

/// The durable identity carried through a scheduler handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyWorkerLeaseContext {
    operation_id: ExecutionOperationId,
    claim_fence: StepClaimFence,
    lease_owner_id: LeaseOwnerId,
    deadline: OffsetDateTime,
    idempotency_key: IdempotencyKey,
}

impl CeremonyWorkerLeaseContext {
    #[must_use]
    pub const fn new(
        operation_id: ExecutionOperationId,
        claim_fence: StepClaimFence,
        lease_owner_id: LeaseOwnerId,
        deadline: OffsetDateTime,
        idempotency_key: IdempotencyKey,
    ) -> Self {
        Self {
            operation_id,
            claim_fence,
            lease_owner_id,
            deadline,
            idempotency_key,
        }
    }

    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }

    #[must_use]
    pub const fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }

    #[must_use]
    pub const fn lease_owner_id(&self) -> &LeaseOwnerId {
        &self.lease_owner_id
    }

    #[must_use]
    pub const fn deadline(&self) -> OffsetDateTime {
        self.deadline
    }

    #[must_use]
    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
}

/// One typed unit submitted to the scheduler. Its identity is opaque to the
/// policy; execution and fenced completion remain delegated to the use cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyWorkerScheduleRequest {
    root_id: CeremonyId,
    lease: CeremonyWorkerLeaseContext,
    priority: CeremonyWorkerPriority,
    weight: CeremonyWorkerWeight,
    cost: CeremonyWorkerCost,
    requested_capacity: CeremonyWorkerCapacity,
    eligibility: CeremonyWorkerEligibility,
}

impl CeremonyWorkerScheduleRequest {
    #[must_use]
    pub const fn new(
        root_id: CeremonyId,
        lease: CeremonyWorkerLeaseContext,
        priority: CeremonyWorkerPriority,
        weight: CeremonyWorkerWeight,
        cost: CeremonyWorkerCost,
        requested_capacity: CeremonyWorkerCapacity,
        eligibility: CeremonyWorkerEligibility,
    ) -> Self {
        Self {
            root_id,
            lease,
            priority,
            weight,
            cost,
            requested_capacity,
            eligibility,
        }
    }

    #[must_use]
    pub const fn root_id(&self) -> &CeremonyId {
        &self.root_id
    }

    #[must_use]
    pub const fn lease(&self) -> &CeremonyWorkerLeaseContext {
        &self.lease
    }

    #[must_use]
    pub const fn priority(&self) -> CeremonyWorkerPriority {
        self.priority
    }

    #[must_use]
    pub const fn weight(&self) -> CeremonyWorkerWeight {
        self.weight
    }

    #[must_use]
    pub const fn cost(&self) -> CeremonyWorkerCost {
        self.cost
    }

    #[must_use]
    pub const fn requested_capacity(&self) -> CeremonyWorkerCapacity {
        self.requested_capacity
    }

    #[must_use]
    pub const fn eligibility(&self) -> CeremonyWorkerEligibility {
        self.eligibility
    }

    #[must_use]
    pub const fn with_eligibility(mut self, eligibility: CeremonyWorkerEligibility) -> Self {
        self.eligibility = eligibility;
        self
    }
}

/// Why a request was or was not admitted by the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyWorkerAdmissionReason {
    Admitted,
    Backpressure,
    Budget,
    Permission,
    Draining,
}

/// Observable, deterministic admission record. The request is retained so a
/// caller cannot accidentally lose operation/fence/lease identity at a queue
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyWorkerAdmissionDecision {
    request: CeremonyWorkerScheduleRequest,
    reason: CeremonyWorkerAdmissionReason,
    policy_version: CeremonyWorkerPolicyVersion,
    sequence: u64,
}

impl CeremonyWorkerAdmissionDecision {
    #[must_use]
    pub const fn new(
        request: CeremonyWorkerScheduleRequest,
        reason: CeremonyWorkerAdmissionReason,
        policy_version: CeremonyWorkerPolicyVersion,
        sequence: u64,
    ) -> Self {
        Self {
            request,
            reason,
            policy_version,
            sequence,
        }
    }

    #[must_use]
    pub const fn request(&self) -> &CeremonyWorkerScheduleRequest {
        &self.request
    }

    #[must_use]
    pub const fn reason(&self) -> CeremonyWorkerAdmissionReason {
        self.reason
    }

    #[must_use]
    pub const fn policy_version(&self) -> CeremonyWorkerPolicyVersion {
        self.policy_version
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}

impl fmt::Display for CeremonyWorkerAdmissionReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Admitted => "admitted",
            Self::Backpressure => "backpressure",
            Self::Budget => "budget",
            Self::Permission => "permission",
            Self::Draining => "draining",
        })
    }
}
