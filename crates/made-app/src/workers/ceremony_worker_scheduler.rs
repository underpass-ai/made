use std::collections::BTreeMap;
use std::sync::Arc;

use super::{
    CeremonyWorkerAdmissionDecision, CeremonyWorkerAdmissionObserver,
    CeremonyWorkerAdmissionReason, CeremonyWorkerEligibility, CeremonyWorkerSchedule,
    CeremonyWorkerScheduleRequest, CeremonyWorkerSchedulerPolicy, CeremonyWorkerWeight,
    NoopCeremonyWorkerAdmissionObserver,
};

type PendingRequest = (u64, CeremonyWorkerScheduleRequest);
type RootQueue = (CeremonyWorkerWeight, u64, Vec<PendingRequest>);

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

    /// Keep only candidates observed in this discovery page. Deficits survive refresh.
    pub fn refresh<I>(&mut self, requests: I) -> CeremonyWorkerSchedule
    where
        I: IntoIterator<Item = CeremonyWorkerScheduleRequest>,
    {
        let requests = requests.into_iter().collect::<Vec<_>>();
        let observed_roots = requests
            .iter()
            .map(|request| request.root_id().clone())
            .collect::<std::collections::BTreeSet<_>>();
        for queue in self.roots.values_mut() {
            queue.2.clear();
        }
        self.roots.retain(|root, _| observed_roots.contains(root));
        self.schedule(requests)
    }

    pub fn set_draining(&mut self, draining: bool) {
        self.draining = draining;
    }

    #[must_use]
    pub const fn is_draining(&self) -> bool {
        self.draining
    }

    pub fn observe(
        &mut self,
        request: CeremonyWorkerScheduleRequest,
        reason: CeremonyWorkerAdmissionReason,
    ) -> CeremonyWorkerAdmissionDecision {
        self.decision(request, reason)
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
            let queue = self
                .roots
                .entry(root_id)
                .or_insert_with(|| (weight, 0, Vec::new()));
            if queue.0 != weight {
                // A root's weight is policy, not caller-controlled per item.
                deferred.push(self.decision(request, CeremonyWorkerAdmissionReason::Backpressure));
                continue;
            }
            self.ordinal = self.ordinal.saturating_add(1);
            queue.2.push((self.ordinal, request.clone()));
            newly_queued.push(request);
        }

        for queue in self.roots.values_mut() {
            queue.2.sort_by(|left, right| {
                right
                    .1
                    .priority()
                    .cmp(&left.1.priority())
                    .then_with(|| left.0.cmp(&right.0))
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
            .map(|pending| pending.1.operation_id().clone())
            .collect::<std::collections::BTreeSet<_>>();
        for pending in selected {
            admitted.push(self.decision(pending.1, CeremonyWorkerAdmissionReason::Admitted));
        }
        for request in newly_queued {
            if !selected_ids.contains(request.operation_id()) {
                deferred.push(self.decision(request, CeremonyWorkerAdmissionReason::Backpressure));
            }
        }
        CeremonyWorkerSchedule::new(admitted, deferred)
    }

    fn dispatch(&mut self) -> Vec<PendingRequest> {
        let mut selected = Vec::new();
        let mut used_capacity = 0_u32;
        while selected.len() < usize::from(self.policy.max_parallel().get()) {
            let remaining_capacity = self.policy.capacity().value().saturating_sub(used_capacity);
            let keys = self.roots.keys().cloned().collect::<Vec<_>>();
            if keys.is_empty() {
                break;
            }
            let any_fits_capacity = self.roots.values().any(|queue| {
                queue
                    .2
                    .iter()
                    .any(|pending| pending.1.requested_capacity().value() <= remaining_capacity)
            });
            if !any_fits_capacity {
                break;
            }
            let start = self
                .next_root
                .as_ref()
                .and_then(|root| keys.iter().position(|candidate| candidate >= root))
                .unwrap_or(0);
            let mut picked = Vec::new();
            let mut visited_root = None;
            for offset in 0..keys.len() {
                let index = (start + offset) % keys.len();
                let root = &keys[index];
                let capacity_limit = self.policy.capacity().value();
                let queue = self
                    .roots
                    .get_mut(root)
                    .expect("root key was copied from the map");
                queue.1 = queue.1.saturating_add(u64::from(queue.0.value()));
                while selected.len() + picked.len() < usize::from(self.policy.max_parallel().get())
                {
                    let Some(position) = queue.2.iter().position(|pending| {
                        u64::from(pending.1.cost().value()) <= queue.1
                            && pending.1.requested_capacity().value()
                                <= capacity_limit.saturating_sub(used_capacity)
                    }) else {
                        break;
                    };
                    let pending = queue.2.remove(position);
                    queue.1 = queue.1.saturating_sub(u64::from(pending.1.cost().value()));
                    used_capacity =
                        used_capacity.saturating_add(pending.1.requested_capacity().value());
                    picked.push(pending);
                }
                visited_root = Some(index);
                if !picked.is_empty() {
                    break;
                }
            }
            let Some(index) = visited_root else { break };
            let root = keys[index].clone();
            selected.extend(picked);
            self.next_root = keys
                .get((index + 1) % keys.len())
                .cloned()
                .or_else(|| Some(root.clone()));
            if self
                .roots
                .get(&root)
                .is_some_and(|queue| queue.2.is_empty())
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

    use made_core::value_objects::{CeremonyId, ExecutionOperationId, MaxParallel};

    use super::*;
    use crate::workers::{
        CeremonyWorkerCapacity, CeremonyWorkerCost, CeremonyWorkerPolicyVersion,
        CeremonyWorkerPriority,
    };

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
            ExecutionOperationId::new(operation_id).unwrap(),
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
            CeremonyWorkerCapacity::new(u32::from(max_parallel)).unwrap(),
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
    fn weight_grants_proportional_service_for_unit_cost_work() {
        let mut scheduler = CeremonyWorkerScheduler::new(policy(8));
        let requests = (1..=8)
            .map(|index| request("root-a", &format!("a{index}"), 1))
            .chain((1..=8).map(|index| request("root-b", &format!("b{index}"), 3)));

        let admitted = scheduler.schedule(requests);
        let root_a = admitted
            .admitted()
            .iter()
            .filter(|decision| decision.request().root_id().as_str() == "root-a")
            .count();
        let root_b = admitted
            .admitted()
            .iter()
            .filter(|decision| decision.request().root_id().as_str() == "root-b")
            .count();

        assert_eq!((root_a, root_b), (2, 6));
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
            request("root", "large", 1).operation_id().clone(),
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

    #[test]
    fn requested_capacity_is_aggregated_across_admitted_requests() {
        let policy = CeremonyWorkerSchedulerPolicy::new(
            MaxParallel::new(8).unwrap(),
            CeremonyWorkerCapacity::new(4).unwrap(),
            CeremonyWorkerPolicyVersion::new(8).unwrap(),
        );
        let mut scheduler = CeremonyWorkerScheduler::new(policy);
        let requests = (1..=3).map(|index| {
            CeremonyWorkerScheduleRequest::new(
                CeremonyId::new(format!("root-{index}")).unwrap(),
                request("ignored", &format!("capacity-{index}"), 3)
                    .operation_id()
                    .clone(),
                CeremonyWorkerPriority::DEFAULT,
                CeremonyWorkerWeight::new(3).unwrap(),
                CeremonyWorkerCost::new(1).unwrap(),
                CeremonyWorkerCapacity::new(3).unwrap(),
                CeremonyWorkerEligibility::Ready,
            )
        });

        let schedule = scheduler.schedule(requests);

        assert_eq!(schedule.admitted().len(), 1);
        assert_eq!(schedule.deferred().len(), 2);
    }

    #[test]
    fn oversized_queue_head_does_not_block_a_later_fitting_request() {
        let policy = CeremonyWorkerSchedulerPolicy::new(
            MaxParallel::new(3).unwrap(),
            CeremonyWorkerCapacity::new(4).unwrap(),
            CeremonyWorkerPolicyVersion::new(9).unwrap(),
        );
        let mut scheduler = CeremonyWorkerScheduler::new(policy);
        let operation = |name: &str| request("ignored", name, 4).operation_id().clone();
        let make = |name: &str, priority: u16, capacity: u32| {
            CeremonyWorkerScheduleRequest::new(
                CeremonyId::new("root").unwrap(),
                operation(name),
                CeremonyWorkerPriority::new(priority).unwrap(),
                CeremonyWorkerWeight::new(4).unwrap(),
                CeremonyWorkerCost::new(1).unwrap(),
                CeremonyWorkerCapacity::new(capacity).unwrap(),
                CeremonyWorkerEligibility::Ready,
            )
        };

        let schedule = scheduler.schedule([
            make("first", 10, 3),
            make("oversized-head", 9, 3),
            make("fitting-tail", 1, 1),
        ]);

        assert_eq!(schedule.admitted().len(), 2);
        assert!(schedule
            .admitted()
            .iter()
            .any(|decision| { decision.request().operation_id() == &operation("fitting-tail") }));
    }
}
