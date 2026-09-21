use made_core::value_objects::{
    GlobalPosition, LoopLimits, LoopProgressMark, LoopRoundLimit, Owed,
};

use super::*;

fn at(value: u64) -> GlobalPosition {
    GlobalPosition::new(value).unwrap()
}

fn limits(max_rounds: Option<u32>, no_progress: u32) -> LoopLimits {
    LoopLimits::new(
        max_rounds.map(|value| LoopRoundLimit::new(value).unwrap()),
        LoopRoundLimit::new(no_progress).unwrap(),
    )
}

/// `rounds` asks, all of them holding work and all of them finding
/// the feed and the ledger exactly where the last one left them.
fn going_nowhere(rounds: u32) -> LoopRoundTally {
    let mut mark = LoopProgressMark::default();
    for _ in 0..rounds {
        mark = mark.observing(Some(at(4)), 0, Owed::Something);
    }
    LoopRoundTally::of(mark)
}

#[test]
fn a_loop_that_has_asked_nothing_is_not_stuck() {
    let detector = NoProgressDetector::new(limits(None, 3));
    assert_eq!(detector.detect(LoopRoundTally::default()), None);
}

#[test]
fn asking_its_allowance_of_times_and_seeing_nothing_new_is_no_progress() {
    let detector = NoProgressDetector::new(limits(None, 3));
    assert_eq!(
        detector.detect(going_nowhere(4)),
        Some(LoopStall::NoProgress),
        "the first ask at a head is not stuck; the three after it are"
    );
}

#[test]
fn one_ask_short_of_the_allowance_is_still_a_running_loop() {
    let detector = NoProgressDetector::new(limits(None, 3));
    assert_eq!(detector.detect(going_nowhere(3)), None);
}

#[test]
fn the_feed_moving_clears_the_count_however_long_the_lease_was() {
    let mark = LoopProgressMark::default()
        .observing(Some(at(4)), 0, Owed::Something)
        .observing(Some(at(4)), 0, Owed::Something)
        .observing(Some(at(4)), 0, Owed::Something)
        .observing(Some(at(9)), 0, Owed::Something);
    let detector = NoProgressDetector::new(limits(None, 3));

    assert_eq!(detector.detect(LoopRoundTally::of(mark)), None);
    assert_eq!(
        mark.rounds(),
        4,
        "the ask still counted towards the ceiling"
    );
}

#[test]
fn closing_a_delivery_is_progress_at_an_unmoved_head() {
    let mark = LoopProgressMark::default()
        .observing(Some(at(4)), 0, Owed::Something)
        .observing(Some(at(4)), 0, Owed::Something)
        .observing(Some(at(4)), 0, Owed::Something)
        .observing(Some(at(4)), 1, Owed::Something);

    assert_eq!(
        NoProgressDetector::new(limits(None, 3)).detect(LoopRoundTally::of(mark)),
        None
    );
}

#[test]
fn the_round_ceiling_outranks_the_stall_and_stops_new_results() {
    let detector = NoProgressDetector::new(limits(Some(4), 3));
    let stall = detector.detect(going_nowhere(4));

    assert_eq!(stall, Some(LoopStall::RoundLimit));
    assert!(!stall.unwrap().admits_new_results());
    assert!(LoopStall::NoProgress.admits_new_results());
}

#[test]
fn the_ceiling_counts_every_ask_and_not_only_the_fruitless_ones() {
    let mark = LoopProgressMark::default()
        .observing(Some(at(1)), 0, Owed::Something)
        .observing(Some(at(2)), 0, Owed::Something)
        .observing(Some(at(3)), 0, Owed::Something);

    assert_eq!(
        NoProgressDetector::new(limits(Some(3), 3)).detect(LoopRoundTally::of(mark)),
        Some(LoopStall::RoundLimit),
        "a loop that moved every round still used up the rounds it had"
    );
}
