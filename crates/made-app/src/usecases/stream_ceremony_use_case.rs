use std::fmt;
use std::sync::Arc;

use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyAgentStatusPort, CeremonyEventStorePort, CeremonyProgressNotifierPort,
};
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, StreamVersion};
use tokio::sync::mpsc;
use tokio::time::{self, Instant};

use super::ceremony_agent_activity_feed::CeremonyAgentActivityFeed;
use super::{
    CeremonyProgressEnd, CeremonyProgressEndReason, CeremonyProgressFrame,
    CeremonyProgressSettings, CeremonyProgressStream, ReadCeremonyEventsInput,
    ReadCeremonyEventsUseCase, StreamCeremonyInput,
};
use crate::services::AuthorizationOperationScope;

/// Replays sealed ceremony events and follows new records for a bounded wait.
pub struct StreamCeremonyUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
    notifier: Arc<dyn CeremonyProgressNotifierPort>,
    settings: CeremonyProgressSettings,
    agent_status: Option<Arc<dyn CeremonyAgentStatusPort>>,
}

impl fmt::Debug for StreamCeremonyUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("StreamCeremonyUseCase").finish()
    }
}

impl StreamCeremonyUseCase {
    #[must_use]
    pub fn new(
        events: Arc<dyn CeremonyEventStorePort>,
        notifier: Arc<dyn CeremonyProgressNotifierPort>,
    ) -> Self {
        Self::with_settings(events, notifier, CeremonyProgressSettings::default())
    }

    #[must_use]
    pub fn with_settings(
        events: Arc<dyn CeremonyEventStorePort>,
        notifier: Arc<dyn CeremonyProgressNotifierPort>,
        settings: CeremonyProgressSettings,
    ) -> Self {
        Self {
            events,
            notifier,
            settings,
            agent_status: None,
        }
    }

    #[must_use]
    pub fn with_agent_activity(mut self, agent_status: Arc<dyn CeremonyAgentStatusPort>) -> Self {
        self.agent_status = Some(agent_status);
        self
    }

    pub async fn execute(
        &self,
        input: StreamCeremonyInput,
    ) -> Result<CeremonyProgressStream, DomainError> {
        // Subscribe before reading: an append in the read race leaves a wake
        // behind, while an earlier append is already in the store.
        let subscription = self.notifier.subscribe();
        let authorization =
            AuthorizationOperationScope::current().map(|operation| operation.evidence().clone());
        let activity =
            CeremonyAgentActivityFeed::open(&input, self.agent_status.clone(), authorization)
                .await?;
        let initial = self
            .read_page(
                input.ceremony_id().clone(),
                input.after_sequence(),
                input.max_events(),
            )
            .await?;
        let head = initial.head_version();
        let records = initial.into_records();
        let (sender, receiver) = mpsc::channel(self.settings.channel_capacity());
        let events = self.events.clone();
        let settings = self.settings;
        let producer = tokio::spawn(async move {
            run_progress_stream(
                events,
                subscription,
                activity,
                sender,
                input,
                records,
                head,
                settings,
            )
            .await;
        });
        Ok(CeremonyProgressStream::new(receiver, producer))
    }

    async fn read_page(
        &self,
        ceremony_id: CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<super::CeremonyEventPage, DomainError> {
        ReadCeremonyEventsUseCase::new(self.events.clone())
            .execute(ReadCeremonyEventsInput::new(ceremony_id, after, limit))
            .await
    }
}

async fn run_progress_stream(
    events: Arc<dyn CeremonyEventStorePort>,
    mut subscription: Box<dyn made_core::ports::CeremonyProgressSubscriptionPort>,
    mut activity: CeremonyAgentActivityFeed,
    sender: mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    input: StreamCeremonyInput,
    initial_records: Vec<AuditRecord>,
    mut head: StreamVersion,
    settings: CeremonyProgressSettings,
) {
    let Some((mut frontier, mut remaining)) = deliver_initial_progress(
        &sender,
        events.clone(),
        &input,
        &mut activity,
        initial_records,
        head,
    )
    .await
    else {
        return;
    };
    let deadline = time::sleep(std::time::Duration::from_millis(u64::from(
        input.wait_timeout().millis(),
    )));
    tokio::pin!(deadline);
    let start = Instant::now() + settings.catch_up_interval();
    let mut catch_up = time::interval_at(start, settings.catch_up_interval());
    loop {
        if head.value() <= frontier.value() && activity.caught_up() {
            if input.wait_timeout().millis() == 0 {
                send_end(
                    &sender,
                    frontier,
                    head,
                    CeremonyProgressEndReason::WaitElapsed,
                    activity.frontier(),
                    activity.head(),
                )
                .await;
                return;
            }

            tokio::select! {
                biased;
                () = sender.closed() => return,
                () = &mut deadline => {
                    send_end(
                        &sender,
                        frontier,
                        head,
                        CeremonyProgressEndReason::WaitElapsed,
                        activity.frontier(),
                        activity.head(),
                    ).await;
                    return;
                }
                () = subscription.wait() => {}
                () = activity.wait() => {}
                _ = catch_up.tick() => {}
            }
        }

        if !activity.deliver_next(&sender, &mut remaining).await {
            return;
        }
        if remaining == 0 {
            send_end(
                &sender,
                frontier,
                head,
                CeremonyProgressEndReason::EventLimit,
                activity.frontier(),
                activity.head(),
            )
            .await;
            return;
        }

        if !deliver_next_page(
            &sender,
            events.clone(),
            &input,
            &mut frontier,
            &mut head,
            &mut remaining,
            activity.frontier(),
            activity.head(),
        )
        .await
        {
            return;
        }
    }
}

async fn deliver_initial_progress(
    sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    events: Arc<dyn CeremonyEventStorePort>,
    input: &StreamCeremonyInput,
    activity: &mut CeremonyAgentActivityFeed,
    records: Vec<AuditRecord>,
    head: StreamVersion,
) -> Option<(StreamVersion, usize)> {
    let mut frontier = input.after_sequence();
    let mut remaining = input.max_events().value();
    if !activity.deliver_initial(sender, &mut remaining).await {
        return None;
    }
    if remaining == 0 {
        send_end(
            sender,
            frontier,
            head,
            CeremonyProgressEndReason::EventLimit,
            activity.frontier(),
            activity.head(),
        )
        .await;
        return None;
    }
    let delivered_terminal =
        deliver_records(sender, records, &mut frontier, &mut remaining).await?;
    if delivered_terminal
        || finish_from_terminal_history(
            sender,
            events.as_ref(),
            input.ceremony_id(),
            frontier,
            head,
            activity.frontier(),
            activity.head(),
        )
        .await
    {
        if delivered_terminal {
            send_end(
                sender,
                frontier,
                head,
                CeremonyProgressEndReason::Terminal,
                activity.frontier(),
                activity.head(),
            )
            .await;
        }
        return None;
    }
    if remaining == 0 {
        send_end(
            sender,
            frontier,
            head,
            CeremonyProgressEndReason::EventLimit,
            activity.frontier(),
            activity.head(),
        )
        .await;
        return None;
    }
    Some((frontier, remaining))
}

async fn deliver_next_page(
    sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    events: Arc<dyn CeremonyEventStorePort>,
    input: &StreamCeremonyInput,
    frontier: &mut StreamVersion,
    head: &mut StreamVersion,
    remaining: &mut usize,
    activity_frontier: u64,
    activity_head: u64,
) -> bool {
    let Some(page) = read_progress_page(
        sender,
        events.clone(),
        input.ceremony_id().clone(),
        *frontier,
        *remaining,
    )
    .await
    else {
        return false;
    };
    *head = page.head_version();
    let records = page.into_records();
    if records.is_empty() {
        // Records are read before head. An append between the two reads leaves
        // an empty page with head ahead; retry before inferring terminality.
        if head.value() > frontier.value() {
            return true;
        }
        return !finish_from_terminal_history(
            sender,
            events.as_ref(),
            input.ceremony_id(),
            *frontier,
            *head,
            activity_frontier,
            activity_head,
        )
        .await;
    }
    let Some(delivered_terminal) = deliver_records(sender, records, frontier, remaining).await
    else {
        return false;
    };
    if delivered_terminal {
        send_end(
            sender,
            *frontier,
            *head,
            CeremonyProgressEndReason::Terminal,
            activity_frontier,
            activity_head,
        )
        .await;
        return false;
    }
    if finish_from_terminal_history(
        sender,
        events.as_ref(),
        input.ceremony_id(),
        *frontier,
        *head,
        activity_frontier,
        activity_head,
    )
    .await
    {
        return false;
    }
    if *remaining == 0 {
        send_end(
            sender,
            *frontier,
            *head,
            CeremonyProgressEndReason::EventLimit,
            activity_frontier,
            activity_head,
        )
        .await;
        return false;
    }
    true
}

async fn deliver_records(
    sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    records: Vec<AuditRecord>,
    frontier: &mut StreamVersion,
    remaining: &mut usize,
) -> Option<bool> {
    for record in records {
        if *remaining == 0 {
            break;
        }
        let terminal = is_terminal_record(&record);
        let sequence = StreamVersion::from_sequence(record.sequence());
        if sender
            .send(Ok(CeremonyProgressFrame::Record(Box::new(record))))
            .await
            .is_err()
        {
            return None;
        }
        *frontier = sequence;
        *remaining -= 1;
        if terminal {
            return Some(true);
        }
    }
    Some(false)
}

async fn read_progress_page(
    sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    events: Arc<dyn CeremonyEventStorePort>,
    ceremony_id: CeremonyId,
    frontier: StreamVersion,
    remaining: usize,
) -> Option<super::CeremonyEventPage> {
    let limit = CeremonyEventPageLimit::new(remaining)
        .expect("remaining progress capacity is positive and bounded");
    match ReadCeremonyEventsUseCase::new(events)
        .execute(ReadCeremonyEventsInput::new(ceremony_id, frontier, limit))
        .await
    {
        Ok(page) => Some(page),
        Err(error) => {
            let _ignored = sender.send(Err(error)).await;
            None
        }
    }
}

async fn finish_from_terminal_history(
    sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    events: &dyn CeremonyEventStorePort,
    ceremony_id: &CeremonyId,
    frontier: StreamVersion,
    head: StreamVersion,
    activity_frontier: u64,
    activity_head: u64,
) -> bool {
    if head.value() > frontier.value() {
        return false;
    }
    match history_is_terminal(events, ceremony_id, head).await {
        Ok(true) => {
            send_end(
                sender,
                frontier,
                head,
                CeremonyProgressEndReason::Terminal,
                activity_frontier,
                activity_head,
            )
            .await;
            true
        }
        Ok(false) => false,
        Err(error) => {
            let _ignored = sender.send(Err(error)).await;
            true
        }
    }
}

async fn send_end(
    sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    frontier: StreamVersion,
    head: StreamVersion,
    reason: CeremonyProgressEndReason,
    activity_frontier: u64,
    activity_head: u64,
) {
    let end = CeremonyProgressEnd::new(frontier, head, reason)
        .with_activity(activity_frontier, activity_head);
    let _ignored = sender.send(Ok(CeremonyProgressFrame::End(end))).await;
}

async fn history_is_terminal(
    events: &dyn CeremonyEventStorePort,
    ceremony_id: &CeremonyId,
    head: StreamVersion,
) -> Result<bool, DomainError> {
    if head.is_empty() {
        return Ok(false);
    }
    let mut from = StreamVersion::EMPTY;
    while from < head {
        let remaining = usize::try_from(head.value() - from.value()).unwrap_or(usize::MAX);
        let records = events
            .read(
                ceremony_id,
                from,
                CeremonyEventPageLimit::new(
                    remaining.min(CeremonyEventPageLimit::DEFAULT.value()),
                )?,
            )
            .await?;
        if records.iter().any(is_terminal_record) {
            return Ok(true);
        }
        let Some(last) = records.last() else {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony history did not advance to its captured head",
            });
        };
        from = StreamVersion::from_sequence(last.sequence());
    }
    Ok(false)
}

fn is_terminal_record(record: &AuditRecord) -> bool {
    matches!(
        record.event_type().as_str(),
        "ceremony_completed"
            | "ceremony_cancelled"
            | "ceremony_deadline_exceeded"
            | "state_deadline_exceeded"
    )
}

#[cfg(test)]
mod tests;
