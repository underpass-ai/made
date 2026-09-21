use made_core::value_objects::{
    CeremonyId, HostAgentIncarnation, HostDeliveryItem, HostDeliveryLease,
    HostDeliveryLeaseId, HostDeliveryPolicy, HostDeliveryRecord, HostDeliveryTarget,
    IntegratorBindingId, LoopLimits, LoopRoundLimit, ProcessedActionKind, ProcessedActionRef,
};
use time::OffsetDateTime;

use super::*;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(seconds)
}

fn target() -> HostDeliveryTarget {
    HostDeliveryTarget::integrator_binding(IntegratorBindingId::new("b-1").unwrap())
}

fn item(name: &str) -> HostDeliveryItem {
    HostDeliveryItem::attention(
        CeremonyId::new("c-1").unwrap(),
        made_core::value_objects::AttentionEventId::new(name).unwrap(),
    )
}

/// One queued delivery, handed over `attempts` times and never closed.
fn handed_over(name: &str, created: i64, attempts: u32) -> HostDeliveryRecord {
    let mut record =
        HostDeliveryRecord::queued(item(name), target(), HostDeliveryPolicy::pull(), at(created))
            .unwrap();
    for round in 0..attempts {
        let lease = HostDeliveryLease::new(
            record.id().clone(),
            HostDeliveryLeaseId::new(format!("{name}-lease-{round}")).unwrap(),
            HostAgentIncarnation::new("run-1").unwrap(),
            at(created + i64::from(round) + 1),
        );
        record = record.leased(lease, at(created + i64::from(round) + 1));
    }
    record
}

fn closed(name: &str, created: i64, processed: i64) -> HostDeliveryRecord {
    handed_over(name, created, 1).processed(
        ProcessedActionRef::new(
            ProcessedActionKind::Integrated,
            Some(made_core::value_objects::IdempotencyKey::new(format!("{name}-action")).unwrap()),
        ),
        at(processed),
    )
}

fn limits(max_rounds: Option<u32>, no_progress: u32) -> LoopLimits {
    LoopLimits::new(
        max_rounds.map(|value| LoopRoundLimit::new(value).unwrap()),
        LoopRoundLimit::new(no_progress).unwrap(),
    )
}

#[test]
fn a_loop_that_has_been_handed_nothing_is_not_stuck() {
    let detector = NoProgressDetector::new(limits(None, 3));
    assert_eq!(detector.detect(LoopRounds::read(&[], at(100))), None);
}

#[test]
fn the_same_item_handed_over_its_allowance_of_rounds_is_no_progress() {
    let deliveries = [handed_over("a", 0, 3)];
    let detector = NoProgressDetector::new(limits(None, 3));
    assert_eq!(
        detector.detect(LoopRounds::read(&deliveries, at(100))),
        Some(LoopStall::NoProgress)
    );
}

#[test]
fn one_round_short_of_the_allowance_is_still_a_running_loop() {
    let deliveries = [handed_over("a", 0, 2)];
    let detector = NoProgressDetector::new(limits(None, 3));
    assert_eq!(detector.detect(LoopRounds::read(&deliveries, at(100))), None);
}

#[test]
fn work_acted_on_since_the_offer_was_made_is_progress() {
    // The stuck-looking item was offered before the host closed
    // something else, so the loop is moving and must not be stopped.
    let deliveries = [closed("closed", 0, 50), handed_over("a", 10, 5)];
    let detector = NoProgressDetector::new(limits(None, 3));
    assert_eq!(detector.detect(LoopRounds::read(&deliveries, at(100))), None);
}

#[test]
fn the_round_ceiling_outranks_the_stall_and_stops_new_results() {
    let deliveries = [handed_over("a", 0, 4)];
    let detector = NoProgressDetector::new(limits(Some(4), 3));
    let stall = detector.detect(LoopRounds::read(&deliveries, at(100)));
    assert_eq!(stall, Some(LoopStall::RoundLimit));
    assert!(!stall.unwrap().admits_new_results());
    assert!(LoopStall::NoProgress.admits_new_results());
}

#[test]
fn rounds_are_counted_across_every_delivery_the_binding_ever_had() {
    let deliveries = [closed("closed", 0, 5), handed_over("a", 10, 2)];
    let rounds = LoopRounds::read(&deliveries, at(100));
    assert_eq!(rounds.handed_over(), 3);
    assert_eq!(rounds.stuck(), 2);
}
