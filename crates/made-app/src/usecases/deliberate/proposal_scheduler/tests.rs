use super::*;
use async_trait::async_trait;
use made_core::entities::TaskConstraints;
use made_core::ports::Critique;
use made_core::value_objects::{
    AgentId, Attributes, ProposalContent, Specialty, TaskDescription, TaskId,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Default, Debug)]
struct Calls {
    active: AtomicUsize,
    peak: AtomicUsize,
    finished: AtomicUsize,
}

struct ActiveCall(Arc<Calls>);
impl Drop for ActiveCall {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct DelayedAgent {
    id: AgentId,
    specialty: Specialty,
    delay: Duration,
    fail: bool,
    calls: Arc<Calls>,
    provider: Arc<Semaphore>,
}

#[async_trait]
impl AgentPort for DelayedAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }
    fn specialty(&self) -> &Specialty {
        &self.specialty
    }
    async fn generate(&self, _: DraftRequest) -> Result<Revision, DomainError> {
        let active = self.calls.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.calls.peak.fetch_max(active, Ordering::SeqCst);
        let _active = ActiveCall(self.calls.clone());
        // This fixture models a provider with a finite number of serving slots.
        let _slot = self.provider.acquire().await.unwrap();
        tokio::time::sleep(self.delay).await;
        self.calls.finished.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(DomainError::InvalidDocument {
                reason: self.id.to_string(),
            });
        }
        Ok(Revision {
            content: self.id.to_string().into(),
        })
    }
    async fn critique(
        &self,
        _: &ProposalContent,
        _: &TaskConstraints,
    ) -> Result<Critique, DomainError> {
        unreachable!()
    }
    async fn revise(&self, _: &ProposalContent, _: &Critique) -> Result<Revision, DomainError> {
        unreachable!()
    }
}

fn fixture(
    delays: &[u64],
    failure: Option<usize>,
    capacity: usize,
) -> (Vec<Arc<dyn AgentPort>>, Arc<Calls>) {
    let calls = Arc::new(Calls::default());
    let provider = Arc::new(Semaphore::new(capacity));
    let agents = delays
        .iter()
        .enumerate()
        .map(|(index, delay)| {
            Arc::new(DelayedAgent {
                id: AgentId::new(format!("agent-{index}")).unwrap(),
                specialty: Specialty::new("fixture").unwrap(),
                delay: Duration::from_millis(*delay),
                fail: failure == Some(index),
                calls: calls.clone(),
                provider: provider.clone(),
            }) as Arc<dyn AgentPort>
        })
        .collect();
    (agents, calls)
}

fn task() -> Task {
    Task::new(
        TaskId::new("proposal-fixture").unwrap(),
        Specialty::new("fixture").unwrap(),
        TaskDescription::new("Measure scheduling only").unwrap(),
        TaskConstraints::default(),
        Attributes::empty(),
    )
}

#[tokio::test]
async fn reverse_completions_still_return_declaration_order_and_drain_failures() {
    let (agents, calls) = fixture(&[30, 20, 1], None, 3);
    let scheduler = ProposalScheduler::new(MaxParallel::new(3).unwrap());
    let results = scheduler.generate(&agents, &task()).await.unwrap();
    assert_eq!(
        results
            .iter()
            .map(|r| r.content.as_str())
            .collect::<Vec<_>>(),
        ["agent-0", "agent-1", "agent-2"]
    );
    assert_eq!(calls.peak.load(Ordering::SeqCst), 3);
    let (agents, calls) = fixture(&[1, 15, 25], Some(0), 3);
    assert!(scheduler.generate(&agents, &task()).await.is_err());
    assert_eq!(calls.finished.load(Ordering::SeqCst), 3);
    assert_eq!(calls.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn simultaneous_councils_share_the_same_proposal_limit() {
    let (agents, calls) = fixture(&[15; 4], None, 8);
    let scheduler = ProposalScheduler::new(MaxParallel::new(2).unwrap());
    let task = task();
    let (a, b) = tokio::join!(
        scheduler.generate(&agents, &task),
        scheduler.generate(&agents, &task)
    );
    assert_eq!(a.unwrap().len() + b.unwrap().len(), 8);
    assert_eq!(calls.peak.load(Ordering::SeqCst), 2);
    assert_eq!(calls.finished.load(Ordering::SeqCst), 8);
}

#[tokio::test]
async fn sequential_default_stops_before_later_provider_calls_on_failure() {
    let (agents, calls) = fixture(&[1; 4], Some(1), 4);
    assert!(ProposalScheduler::sequential(&agents, &task())
        .await
        .is_err());
    assert_eq!(calls.finished.load(Ordering::SeqCst), 2);
    assert_eq!(calls.peak.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancelling_the_caller_releases_permits_for_the_next_request() {
    let (agents, calls) = fixture(&[100; 3], None, 3);
    let scheduler = ProposalScheduler::new(MaxParallel::new(2).unwrap());
    assert!(tokio::time::timeout(
        Duration::from_millis(10),
        scheduler.generate(&agents, &task())
    )
    .await
    .is_err());
    assert_eq!(calls.active.load(Ordering::SeqCst), 0);
    let results = scheduler.generate(&agents, &task()).await.unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(calls.peak.load(Ordering::SeqCst), 2);
}

#[tokio::test]
#[ignore = "explicit timing experiment; no latency assertion in the test gate"]
async fn experiment_003_bounded_proposing() {
    println!("provider_slots,width,repeat,elapsed_ms,peak_calls,completed");
    for capacity in [1, 4] {
        for width in [1, 2, 4, 8] {
            for repeat in 0..5 {
                let (agents, calls) = fixture(&[20; 8], None, capacity);
                let scheduler = ProposalScheduler::new(MaxParallel::new(width).unwrap());
                let start = Instant::now();
                let drafts = if width == 1 {
                    ProposalScheduler::sequential(&agents, &task())
                        .await
                        .unwrap()
                } else {
                    scheduler.generate(&agents, &task()).await.unwrap()
                };
                assert_eq!(drafts.len(), 8);
                println!(
                    "{capacity},{width},{repeat},{:.3},{},{}",
                    start.elapsed().as_secs_f64() * 1000.0,
                    calls.peak.load(Ordering::SeqCst),
                    calls.finished.load(Ordering::SeqCst)
                );
            }
        }
    }
}
