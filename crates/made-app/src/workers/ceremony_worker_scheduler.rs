use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::value_objects::MaxParallel;

use super::{
    CeremonyWorkerAdmissionDecision, CeremonyWorkerAdmissionReason, CeremonyWorkerCapacity,
    CeremonyWorkerEligibility, CeremonyWorkerPolicyVersion, CeremonyWorkerScheduleRequest,
    CeremonyWorkerWeight,
};

/// A read-only sink for admission decisions. The returned schedule is also a
/// complete observation, so hosts can use this trait only when they want a
/// metrics or audit projection.
pub trait CeremonyWorkerAdmissionObserver: Send + Sync {
    fn observe(&self, decision: &CeremonyWorkerAdmissionDecision);
}

#[derive(Debug, Default)]
pub struct NoopCeremonyWorkerAdmissionObserver;

impl CeremonyWorkerAdmissionObserver for NoopCeremonyWorkerAdmissionObserver {
    fn observe(&self, _decision: &CeremonyWorkerAdmissionDecision) {}
}

/// Bounded scheduler policy. `capacity` is a declared host capacity, not a
/// budget balance; budget and permission facts are supplied per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyWorkerSchedulerPolicy {
    max_parallel: MaxParallel,
    capacity: CeremonyWorkerCapacity,
    version: CeremonyWorkerPolicyVersion,
}

impl CeremonyWorkerSchedulerPolicy {
    #[must_use]
    pub const fn new(
        max_parallel: MaxParallel,
        capacity: CeremonyWorkerCapacity,
        version: CeremonyWorkerPolicyVersion,
    ) -> Self {
        Self {
            max_parallel,
            capacity,
            version,
        }
    }

    #[must_use]
    pub const fn max_parallel(self) -> MaxParallel {
        self.max_parallel
    }

    #[must_use]
    pub const fn capacity(self) -> CeremonyWorkerCapacity {
        self.capacity
    }

    #[must_use]
    pub const fn version(self) -> CeremonyWorkerPolicyVersion {
        self.version
    }
}

/// One deterministic, bounded scheduling result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyWorkerSchedule {
    admitted: Vec<CeremonyWorkerAdmissionDecision>,
    deferred: Vec<CeremonyWorkerAdmissionDecision>,
}

impl CeremonyWorkerSchedule {
    #[must_use]
    pub const fn new(
        admitted: Vec<CeremonyWorkerAdmissionDecision>,
        deferred: Vec<CeremonyWorkerAdmissionDecision>,
    ) -> Self {
        Self { admitted, deferred }
    }

    #[must_use]
    pub fn admitted(&self) -> &[CeremonyWorkerAdmissionDecision] {
        &self.admitted
    }

    #[must_use]
    pub fn deferred(&self) -> &[CeremonyWorkerAdmissionDecision] {
        &self.deferred
    }
}

#[derive(Debug)]
struct RootQueue {
    weight: CeremonyWorkerWeight,
    deficit: u64,
    requests: Vec<PendingRequest>,
}

#[derive(Debug)]
struct PendingRequest {
    ordinal: u64,
    request: CeremonyWorkerScheduleRequest,
}

/// Pure in-process weighted deficit round-robin scheduler. It owns no permits,
/// credentials, balances or execution side effects; the worker use cases remain
/// the authority for claims, receipts and fenced completion.
pub struct CeremonyWorkerScheduler {
    policy: CeremonyWorkerSchedulerPolicy,
    draining: bool,
    next_root: Option<made_core::value_objects::CeremonyId>,
    ordinal: u64,
    decision_sequence: u64,
    roots: BTreeMap<made_core::value_objects::CeremonyId, RootQueue>,
    observer: Arc<dyn CeremonyWorkerAdmissionObserver>,
}

impl std::fmt::Debug for CeremonyWorkerScheduler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyWorkerScheduler")
            .field("policy", &self.policy)
            .field("draining", &self.draining)
            .field("roots", &self.roots.len())
            .finish_non_exhaustive()
    }
}

impl CeremonyWorkerScheduler {
    #[must_use]
    pub fn new(policy: CeremonyWorkerSchedulerPolicy) -> Self {
        Self::with_observer(policy, Arc::new(NoopCeremonyWorkerAdmissionObserver))
    }

    #[must_use]
    pub fn with_observer(
        policy: CeremonyWorkerSchedulerPolicy,
        observer: Arc<dyn CeremonyWorkerAdmissionObserver>,
    ) -> Self {
        Self {
            policy,
            draining: false,
            next_root: None,
            ordinal: 0,
            decision_sequence: 0,
            roots: BTreeMap::new(),
            observer,
        }
    }

    pub fn set_draining(&mut self, draining: bool) {
        self.draining = draining;
    }

    #[must_use]
    pub const fn is_draining(&self) -> bool {
        self.draining
    }

    /// Submit a batch and dispatch at most `max_parallel` admitted requests.
    /// Requests deferred for backpressure remain queued and must not be
    /// submitted a second time; call `schedule([])` to drain that queue later.
    pub fn schedule<I>(&mut self, requests: I) -> CeremonyWorkerSchedule
    where
        I: IntoIterator<Item = CeremonyWorkerScheduleRequest>,
    {
        let mut admitted = Vec::new();
        let mut deferred = Vec::new();
        let mut newly_queued = Vec::new();

        for request in requests {
            let reason = if self.draining {
                Some(CeremonyWorkerAdmissionReason::Draining)
            } else {
                match request.eligibility() {
                    CeremonyWorkerEligibility::Ready => None,
                    CeremonyWorkerEligibility::BudgetUnavailable => {
                        Some(CeremonyWorkerAdmissionReason::Budget)
                    }
                    CeremonyWorkerEligibility::PermissionDenied => {
                        Some(CeremonyWorkerAdmissionReason::Permission)
                    }
                }
            };
            if let Some(reason) = reason {
                deferred.push(self.decision(request, reason));
                continue;
            }
            if request.cost().value() > self.policy.capacity().value()
                || request.requested_capacity().value() > self.policy.capacity().value()
            {
                deferred.push(self.decision(request, CeremonyWorkerAdmissionReason::Backpressure));
                continue;
            }
            let root_id = request.root_id().clone();
            let weight = request.weight();
            let queue = self.roots.entry(root_id).or_insert_with(|| RootQueue {
                weight,
                deficit: 0,
                requests: Vec::new(),
            });
            if queue.weight != weight {
                // A root's weight is policy, not caller-controlled per item.
                deferred.push(self.decision(request, CeremonyWorkerAdmissionReason::Backpressure));
                continue;
            }
            self.ordinal = self.ordinal.saturating_add(1);
            queue.requests.push(PendingRequest {
                ordinal: self.ordinal,
                request: request.clone(),
            });
            newly_queued.push(request);
        }

        for queue in self.roots.values_mut() {
            queue.requests.sort_by(|left, right| {
                right
                    .request
                    .priority()
                    .cmp(&left.request.priority())
                    .then_with(|| left.ordinal.cmp(&right.ordinal))
            });
        }

        // Draining closes admission, including queued-but-not-yet-started
        // work. Accepted work is already outside this pure scheduler and is
        // drained by the host/driver.
        let selected = if self.draining {
            Vec::new()
        } else {
            self.dispatch()
        };
        let selected_ids = selected
            .iter()
            .map(|pending| pending.request.lease().operation_id().clone())
            .collect::<std::collections::BTreeSet<_>>();
        for pending in selected {
            admitted.push(self.decision(pending.request, CeremonyWorkerAdmissionReason::Admitted));
        }
        for request in newly_queued {
            if !selected_ids.contains(request.lease().operation_id()) {
                deferred.push(self.decision(request, CeremonyWorkerAdmissionReason::Backpressure));
            }
        }
        CeremonyWorkerSchedule::new(admitted, deferred)
    }

    fn dispatch(&mut self) -> Vec<PendingRequest> {
        let mut selected = Vec::new();
        let rounds = usize::try_from(self.policy.capacity().value()).unwrap_or(usize::MAX);
        for _ in 0..=rounds {
            if selected.len() >= usize::from(self.policy.max_parallel().get()) {
                break;
            }
            let keys = self.roots.keys().cloned().collect::<Vec<_>>();
            if keys.is_empty() {
                break;
            }
            let start = self
                .next_root
                .as_ref()
                .and_then(|root| keys.iter().position(|candidate| candidate >= root))
                .unwrap_or(0);
            let mut picked = None;
            for offset in 0..keys.len() {
                let index = (start + offset) % keys.len();
                let root = &keys[index];
                let queue = self
                    .roots
                    .get_mut(root)
                    .expect("root key was copied from the map");
                queue.deficit = queue
                    .deficit
                    .saturating_add(u64::from(queue.weight.value()));
                if queue.requests.first().is_some_and(|pending| {
                    u64::from(pending.request.cost().value()) <= queue.deficit
                }) {
                    let pending = queue.requests.remove(0);
                    queue.deficit = queue
                        .deficit
                        .saturating_sub(u64::from(pending.request.cost().value()));
                    picked = Some((index, pending));
                    break;
                }
            }
            let Some((index, pending)) = picked else {
                continue;
            };
            let root = keys[index].clone();
            selected.push(pending);
            self.next_root = keys
                .get((index + 1) % keys.len())
                .cloned()
                .or_else(|| Some(root.clone()));
            if self
                .roots
                .get(&root)
                .is_some_and(|queue| queue.requests.is_empty())
            {
                self.roots.remove(&root);
                if self.next_root.as_ref() == Some(&root) {
                    self.next_root = None;
                }
            }
        }
        selected
    }

    fn decision(
        &mut self,
        request: CeremonyWorkerScheduleRequest,
        reason: CeremonyWorkerAdmissionReason,
    ) -> CeremonyWorkerAdmissionDecision {
        self.decision_sequence = self.decision_sequence.saturating_add(1);
        let decision = CeremonyWorkerAdmissionDecision::new(
            request,
            reason,
            self.policy.version(),
            self.decision_sequence,
        );
        self.observer.observe(&decision);
        decision
    }
}

#[cfg(test)]
mod tests {
    use std::hash::{Hash, Hasher};
    use std::sync::{Arc, Mutex};

    use made_core::value_objects::{
        CeremonyId, ExecutionOperationId, IdempotencyKey, LeaseOwnerId, StepClaimFence,
    };
    use time::OffsetDateTime;

    use super::*;
    use crate::workers::{CeremonyWorkerCost, CeremonyWorkerLeaseContext, CeremonyWorkerPriority};

    #[derive(Debug, Default)]
    struct RecordingObserver(Mutex<Vec<CeremonyWorkerAdmissionDecision>>);

    impl CeremonyWorkerAdmissionObserver for RecordingObserver {
        fn observe(&self, decision: &CeremonyWorkerAdmissionDecision) {
            self.0.lock().unwrap().push(decision.clone());
        }
    }

    fn request(root: &str, operation: &str, weight: u32) -> CeremonyWorkerScheduleRequest {
        let mut operation_hasher = std::collections::hash_map::DefaultHasher::new();
        operation.hash(&mut operation_hasher);
        let operation_id = format!("{:064x}", operation_hasher.finish());
        CeremonyWorkerScheduleRequest::new(
            CeremonyId::new(root).unwrap(),
            CeremonyWorkerLeaseContext::new(
                ExecutionOperationId::new(operation_id.clone()).unwrap(),
                StepClaimFence::new(operation_id).unwrap(),
                LeaseOwnerId::new("host-a").unwrap(),
                OffsetDateTime::UNIX_EPOCH,
                IdempotencyKey::new(format!("idempotency-{operation}")).unwrap(),
            ),
            CeremonyWorkerPriority::DEFAULT,
            CeremonyWorkerWeight::new(weight).unwrap(),
            CeremonyWorkerCost::new(1).unwrap(),
            CeremonyWorkerCapacity::new(1).unwrap(),
            CeremonyWorkerEligibility::Ready,
        )
    }

    fn policy(max_parallel: u8) -> CeremonyWorkerSchedulerPolicy {
        CeremonyWorkerSchedulerPolicy::new(
            MaxParallel::new(max_parallel).unwrap(),
            CeremonyWorkerCapacity::new(1).unwrap(),
            CeremonyWorkerPolicyVersion::new(7).unwrap(),
        )
    }

    #[test]
    fn weighted_round_robin_is_deterministic_and_shares_roots() {
        let mut scheduler = CeremonyWorkerScheduler::new(policy(2));
        let first = scheduler.schedule([
            request("root-a", "a1", 1),
            request("root-a", "a2", 1),
            request("root-b", "b1", 2),
            request("root-b", "b2", 2),
        ]);
        assert_eq!(
            first
                .admitted()
                .iter()
                .map(|decision| decision.request().root_id().as_str())
                .collect::<Vec<_>>(),
            ["root-a", "root-b"]
        );
        let second = scheduler.schedule([]);
        assert_eq!(
            second
                .admitted()
                .iter()
                .map(|decision| decision.request().root_id().as_str())
                .collect::<Vec<_>>(),
            ["root-a", "root-b"]
        );
    }

    #[test]
    fn concurrency_is_bounded_and_deferred_work_is_reusable() {
        let mut scheduler = CeremonyWorkerScheduler::new(policy(2));
        let batch =
            scheduler.schedule((1..=5).map(|index| request("root", &format!("x{index}"), 1)));
        assert_eq!(batch.admitted().len(), 2);
        assert_eq!(batch.deferred().len(), 3);
        assert!(batch
            .deferred()
            .iter()
            .all(|decision| decision.reason() == CeremonyWorkerAdmissionReason::Backpressure));
        assert_eq!(scheduler.schedule([]).admitted().len(), 2);
    }

    #[test]
    fn policy_reasons_are_external_and_metadata_survives_admission() {
        let observer = Arc::new(RecordingObserver::default());
        let mut scheduler = CeremonyWorkerScheduler::with_observer(policy(1), observer.clone());
        let budget = request("budget", "budget", 1)
            .with_eligibility(CeremonyWorkerEligibility::BudgetUnavailable);
        let permission = request("permission", "permission", 1)
            .with_eligibility(CeremonyWorkerEligibility::PermissionDenied);
        let admitted = request("root", "admitted", 1);
        let schedule = scheduler.schedule([budget, permission, admitted.clone()]);
        assert_eq!(schedule.admitted()[0].request(), &admitted);
        assert_eq!(
            schedule.deferred()[0].reason(),
            CeremonyWorkerAdmissionReason::Budget
        );
        assert_eq!(
            schedule.deferred()[1].reason(),
            CeremonyWorkerAdmissionReason::Permission
        );
        assert_eq!(observer.0.lock().unwrap().len(), 3);
    }

    #[test]
    fn draining_and_invalid_cost_never_admit() {
        let mut scheduler = CeremonyWorkerScheduler::new(policy(1));
        scheduler.set_draining(true);
        let draining = scheduler.schedule([request("root", "draining", 1)]);
        assert_eq!(draining.admitted().len(), 0);
        assert_eq!(
            draining.deferred()[0].reason(),
            CeremonyWorkerAdmissionReason::Draining
        );

        scheduler.set_draining(false);
        let too_large = CeremonyWorkerScheduleRequest::new(
            CeremonyId::new("root").unwrap(),
            request("root", "large", 1).lease().clone(),
            CeremonyWorkerPriority::DEFAULT,
            CeremonyWorkerWeight::new(1).unwrap(),
            CeremonyWorkerCost::new(2).unwrap(),
            CeremonyWorkerCapacity::new(2).unwrap(),
            CeremonyWorkerEligibility::Ready,
        );
        let result = scheduler.schedule([too_large]);
        assert_eq!(result.admitted().len(), 0);
        assert_eq!(
            result.deferred()[0].reason(),
            CeremonyWorkerAdmissionReason::Backpressure
        );
    }

    #[test]
    fn draining_does_not_release_queued_backpressure_work() {
        let mut scheduler = CeremonyWorkerScheduler::new(policy(1));
        let first = scheduler.schedule([request("root", "first", 1), request("root", "queued", 1)]);
        assert_eq!(first.admitted().len(), 1);
        scheduler.set_draining(true);
        assert!(scheduler.schedule([]).admitted().is_empty());
        scheduler.set_draining(false);
        assert_eq!(scheduler.schedule([]).admitted().len(), 1);
    }
}
