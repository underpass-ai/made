use std::fmt;
use std::sync::Arc;

use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::ports::{CeremonyEventStorePort, CeremonyProgressNotifierPort};
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, StreamVersion};
use tokio::sync::mpsc;
use tokio::time::{self, Instant};

use super::{
    CeremonyProgressEnd, CeremonyProgressEndReason, CeremonyProgressFrame,
    CeremonyProgressSettings, CeremonyProgressStream, ReadCeremonyEventsInput,
    ReadCeremonyEventsUseCase, StreamCeremonyInput,
};

/// Replays sealed ceremony events and follows new records for a bounded wait.
pub struct StreamCeremonyUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
    notifier: Arc<dyn CeremonyProgressNotifierPort>,
    settings: CeremonyProgressSettings,
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
        }
    }

    pub async fn execute(
        &self,
        input: StreamCeremonyInput,
    ) -> Result<CeremonyProgressStream, DomainError> {
        // Subscribe before reading: an append in the read race leaves a wake
        // behind, while an earlier append is already in the store.
        let subscription = self.notifier.subscribe();
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
            run_progress_stream(events, subscription, sender, input, records, head, settings).await;
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
    sender: mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    input: StreamCeremonyInput,
    initial_records: Vec<AuditRecord>,
    mut head: StreamVersion,
    settings: CeremonyProgressSettings,
) {
    let mut frontier = input.after_sequence();
    let mut remaining = input.max_events().value();
    let Some(delivered_terminal) =
        deliver_records(&sender, initial_records, &mut frontier, &mut remaining).await
    else {
        return;
    };
    if delivered_terminal {
        send_end(&sender, frontier, head, CeremonyProgressEndReason::Terminal).await;
        return;
    }
    if finish_from_terminal_history(
        &sender,
        events.as_ref(),
        input.ceremony_id(),
        frontier,
        head,
    )
    .await
    {
        return;
    }
    if remaining == 0 {
        send_end(
            &sender,
            frontier,
            head,
            CeremonyProgressEndReason::EventLimit,
        )
        .await;
        return;
    }
    let deadline = time::sleep(std::time::Duration::from_millis(u64::from(
        input.wait_timeout().millis(),
    )));
    tokio::pin!(deadline);
    let start = Instant::now() + settings.catch_up_interval();
    let mut catch_up = time::interval_at(start, settings.catch_up_interval());
    loop {
        if head.value() <= frontier.value() {
            if input.wait_timeout().millis() == 0 {
                send_end(
                    &sender,
                    frontier,
                    head,
                    CeremonyProgressEndReason::WaitElapsed,
                )
                .await;
                return;
            }

            tokio::select! {
                biased;
                () = sender.closed() => return,
                () = &mut deadline => {
                    send_end(&sender, frontier, head, CeremonyProgressEndReason::WaitElapsed).await;
                    return;
                }
                () = subscription.wait() => {}
                _ = catch_up.tick() => {}
            }
        }

        if !deliver_next_page(
            &sender,
            events.clone(),
            &input,
            &mut frontier,
            &mut head,
            &mut remaining,
        )
        .await
        {
            return;
        }
    }
}

async fn deliver_next_page(
    sender: &mpsc::Sender<Result<CeremonyProgressFrame, DomainError>>,
    events: Arc<dyn CeremonyEventStorePort>,
    input: &StreamCeremonyInput,
    frontier: &mut StreamVersion,
    head: &mut StreamVersion,
    remaining: &mut usize,
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
) -> bool {
    if head.value() > frontier.value() {
        return false;
    }
    match history_is_terminal(events, ceremony_id, head).await {
        Ok(true) => {
            send_end(sender, frontier, head, CeremonyProgressEndReason::Terminal).await;
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
) {
    let end = CeremonyProgressEnd::new(frontier, head, reason);
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
