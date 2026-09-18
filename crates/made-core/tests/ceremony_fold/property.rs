//! A few hundred random sequences of commands — most valid, many
//! not — against the fixture ceremony. For every sequence the
//! decide-then-apply path and the mutator path must accept and refuse
//! the same commands with the same errors, agree on the session after
//! each one, and the fold of the accepted events must be the session.
//!
//! The generator is a xorshift PRNG rather than a property-testing
//! crate, so the domain crate takes on no dependency for it; the seed
//! is in every assertion message so a failure can be replayed.

use std::collections::BTreeMap;

use made_core::entities::ceremony_commands::{
    ApplyStepResult, ApplyTransition, ApproveGuard, AssertReason, BindParticipant,
    CloseIntervention, DeferGuard, RequestIntervention, RespondToIntervention,
    RespondToInterventionWithEvidence, StartStep,
};
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::value_objects::{
    AuditActorKind, AuditEventType, CeremonyContext, CeremonyId, CeremonyInterventionId,
    CeremonyInterventionKind, CeremonyInterventionProvenance, CeremonyInterventionTarget,
    CeremonyReason, CeremonyReasonKind, CeremonyRecordRef, MemoryConfidence, RoleId,
    StepErrorMessage, StepId, StepResult,
};
use time::{Duration, OffsetDateTime};

use super::fixture::{
    at, content, deferral, definition, evidence_pack, guard, item, lease, readiness, role,
    specialty, step, trigger,
};
use super::fold_equality::mutate;

const SEEDS: u64 = 300;
const COMMANDS_PER_SEED: usize = 48;

struct Xorshift(u64);

impl Xorshift {
    fn seeded(seed: u64) -> Self {
        // xorshift never leaves zero, so the zero seed is nudged.
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn coin(&mut self) -> bool {
        self.below(2) == 0
    }

    fn one_of<'a>(&mut self, options: &[&'a str]) -> &'a str {
        options[self.below(options.len() as u64) as usize]
    }
}

/// Makes commands that are mostly sensible and sometimes not: wrong
/// seats, reused leases, unknown items, moves the guards block.
struct Generator {
    rng: Xorshift,
    clock: OffsetDateTime,
    leases: u64,
    /// Items accepted so far; `item-{n}` exists for every `n` below.
    items: u64,
}

impl Generator {
    fn seeded(seed: u64) -> Self {
        Self {
            rng: Xorshift::seeded(seed),
            clock: at(0),
            leases: 0,
            items: 0,
        }
    }

    fn now(&mut self) -> OffsetDateTime {
        self.clock += Duration::minutes(self.rng.below(8) as i64);
        self.clock
    }

    fn any_role(&mut self) -> RoleId {
        role(self.rng.one_of(&["facilitator", "observer", "stranger"]))
    }

    /// `usual` three times out of four, anyone otherwise.
    fn mostly(&mut self, usual: &str) -> RoleId {
        if self.rng.below(4) == 0 {
            self.any_role()
        } else {
            role(usual)
        }
    }

    fn any_step(&mut self) -> StepId {
        step(self.rng.one_of(&["plan", "check"]))
    }

    /// An existing item four times out of five, once any exist.
    fn some_item(&mut self) -> CeremonyInterventionId {
        if self.items > 0 && self.rng.below(5) != 0 {
            item(&format!("item-{}", self.rng.below(self.items)))
        } else {
            item(&format!("item-{}", self.items))
        }
    }

    fn record(&mut self) -> CeremonyRecordRef {
        match self.rng.below(5) {
            0 => CeremonyRecordRef::step(self.any_step()),
            1 => CeremonyRecordRef::agenda_item(self.some_item()),
            2 => {
                let ordinal = self.rng.below(2) as u32;
                CeremonyRecordRef::contribution(self.some_item(), ordinal)
            }
            3 => CeremonyRecordRef::guard_decision(guard("human_approved")),
            _ => CeremonyRecordRef::transition(self.rng.below(3) as u32),
        }
    }

    fn noted(&mut self, command: &CeremonyCommand) {
        if matches!(command, CeremonyCommand::RequestIntervention(_)) {
            self.items += 1;
        }
    }

    fn command(&mut self) -> CeremonyCommand {
        match self.rng.below(12) {
            0 => self.bind(),
            1 | 2 => self.start_step(),
            3 | 4 => self.apply_step_result(),
            5 => self.transition(),
            6 => self.approve(),
            7 => self.defer(),
            8 => self.request(),
            9 => self.respond(),
            10 => self.reason(),
            _ => self.close(),
        }
    }

    fn bind(&mut self) -> CeremonyCommand {
        CeremonyCommand::BindParticipant(BindParticipant {
            role_id: self.any_role(),
            specialty: specialty(self.rng.one_of(&["queues", "scope", "risk"])),
            now: self.now(),
        })
    }

    fn start_step(&mut self) -> CeremonyCommand {
        let role_id = match self.rng.below(3) {
            0 => None,
            1 => Some(role("facilitator")),
            _ => Some(self.any_role()),
        };
        let key = if self.leases > 0 && self.rng.below(4) == 0 {
            self.rng.below(self.leases)
        } else {
            self.leases += 1;
            self.leases - 1
        };
        let now = self.now();
        CeremonyCommand::StartStep(StartStep {
            role_id,
            step_id: self.any_step(),
            lease: lease(&format!("lease-{key}"), now),
            now,
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
        })
    }

    fn apply_step_result(&mut self) -> CeremonyCommand {
        let result = match self.rng.below(4) {
            0 => StepResult::completed(readiness(false)).unwrap(),
            1 | 2 => StepResult::completed(readiness(true)).unwrap(),
            _ => StepResult::failed(StepErrorMessage::new("boom").unwrap()).unwrap(),
        };
        CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: self.any_step(),
            claim_fence: made_core::value_objects::StepClaimFence::new("0".repeat(64)).unwrap(),
            result,
            now: self.now(),
        })
    }

    fn transition(&mut self) -> CeremonyCommand {
        let role_id = match self.rng.below(3) {
            0 => None,
            1 => Some(role("facilitator")),
            _ => Some(self.any_role()),
        };
        CeremonyCommand::ApplyTransition(ApplyTransition {
            role_id,
            trigger: trigger(self.rng.one_of(&["submit", "approve", "abandon", "bogus"])),
            now: self.now(),
        })
    }

    fn any_guard(&mut self) -> made_core::value_objects::GuardName {
        guard(
            self.rng
                .one_of(&["human_approved", "human_approved", "plan_done", "missing"]),
        )
    }

    fn actor_kind(&mut self) -> AuditActorKind {
        if self.rng.coin() {
            AuditActorKind::Human
        } else {
            AuditActorKind::Agent
        }
    }

    fn approve(&mut self) -> CeremonyCommand {
        CeremonyCommand::ApproveGuard(ApproveGuard {
            guard_name: self.any_guard(),
            approved_by: self.mostly("facilitator"),
            approved_by_kind: self.actor_kind(),
            now: self.now(),
        })
    }

    fn defer(&mut self) -> CeremonyCommand {
        CeremonyCommand::DeferGuard(DeferGuard {
            guard_name: self.any_guard(),
            content: deferral(),
            deferred_by: self.mostly("observer"),
            deferred_by_kind: self.actor_kind(),
            now: self.now(),
        })
    }

    fn request(&mut self) -> CeremonyCommand {
        let target = match self.rng.below(3) {
            0 => CeremonyInterventionTarget::table(),
            1 => CeremonyInterventionTarget::roles([role("observer")]).unwrap(),
            _ => CeremonyInterventionTarget::roles([role("facilitator")]).unwrap(),
        };
        let provenance = (self.items > 0 && self.rng.below(4) == 0).then(|| {
            CeremonyInterventionProvenance::selected_from(
                self.some_item(),
                role("observer"),
                role("observer"),
            )
        });
        let kind = match self.rng.below(3) {
            0 => CeremonyInterventionKind::Opinion,
            1 => CeremonyInterventionKind::Investigation,
            _ => CeremonyInterventionKind::Action,
        };
        CeremonyCommand::RequestIntervention(RequestIntervention {
            intervention_id: self.some_item(),
            role_id: self.mostly("facilitator"),
            kind,
            target,
            content: content("Something to look into."),
            provenance,
            now: self.now(),
        })
    }

    fn respond(&mut self) -> CeremonyCommand {
        let intervention_id = self.some_item();
        let role_id = self.mostly("observer");
        let now = self.now();
        if self.rng.below(3) == 0 {
            CeremonyCommand::RespondToInterventionWithEvidence(RespondToInterventionWithEvidence {
                intervention_id,
                role_id,
                evidence_pack: evidence_pack(self.rng.one_of(&["wiki", "logs"])),
                now,
            })
        } else {
            CeremonyCommand::RespondToIntervention(RespondToIntervention {
                intervention_id,
                role_id,
                content: content("Here is what I found."),
                now,
            })
        }
    }

    fn reason(&mut self) -> CeremonyCommand {
        let from = self.record();
        let mut to = self.record();
        while to == from {
            to = self.record();
        }
        let kind = match self.rng.below(4) {
            0 => CeremonyReasonKind::FollowsFrom,
            1 => CeremonyReasonKind::ChosenBecause,
            2 => CeremonyReasonKind::Answers,
            _ => CeremonyReasonKind::Contradicts,
        };
        let confidence = match self.rng.below(3) {
            0 => MemoryConfidence::High,
            1 => MemoryConfidence::Medium,
            _ => MemoryConfidence::Low,
        };
        let asserted_by = Some(self.any_role());
        let now = self.now();
        CeremonyCommand::AssertReason(AssertReason {
            reason: CeremonyReason::new(
                from,
                to,
                kind,
                "because it looked that way",
                confidence,
                asserted_by,
                now,
            )
            .unwrap(),
        })
    }

    fn close(&mut self) -> CeremonyCommand {
        CeremonyCommand::CloseIntervention(CloseIntervention {
            intervention_id: self.some_item(),
            role_id: self.mostly("facilitator"),
            now: self.now(),
        })
    }
}

/// One random sequence; returns how many events each type produced.
fn run(seed: u64, coverage: &mut BTreeMap<AuditEventType, usize>) {
    let definition = definition();
    let mut generator = Generator::seeded(seed);
    let mut stream = CeremonyInstance::decide_start(
        CeremonyId::new("ceremony-fold").unwrap(),
        &definition,
        CeremonyContext::empty(),
        None,
        at(0),
    )
    .expect("required ceremony inputs");
    let mut by_events = CeremonyInstance::rehydrate(&stream).unwrap();
    let mut by_mutators = by_events.clone();

    for position in 0..COMMANDS_PER_SEED {
        let mut command = generator.command();
        if let CeremonyCommand::ApplyStepResult(result) = &mut command {
            if let Ok(fence) = by_events.step_claim_fence(&result.step_id) {
                result.claim_fence = fence;
            }
        }
        let decided = by_events.decide(&command, &definition);
        let mutated = mutate(&mut by_mutators, &definition, &command);
        match decided {
            Ok(events) => {
                assert_eq!(
                    mutated,
                    Ok(()),
                    "seed {seed}, command {position}: decide accepted what the mutator refused: {command:?}"
                );
                for event in &events {
                    by_events.apply(event);
                    *coverage.entry(event.event_type()).or_default() += 1;
                }
                generator.noted(&command);
                stream.extend(events);
            }
            Err(error) => assert_eq!(
                mutated,
                Err(error),
                "seed {seed}, command {position}: the two paths refuse differently: {command:?}"
            ),
        }
        assert_eq!(
            by_events, by_mutators,
            "seed {seed}, command {position}: the paths diverge after {command:?}"
        );
    }

    assert_eq!(
        CeremonyInstance::rehydrate(&stream).unwrap(),
        by_events,
        "seed {seed}: the fold of {} events is not the session",
        stream.len()
    );
}

#[test]
fn random_command_sequences_fold_back_to_the_session() {
    let mut coverage = BTreeMap::new();
    for seed in 0..SEEDS {
        run(seed, &mut coverage);
    }

    // Random is only evidence if it reached everything.
    for expected in [
        AuditEventType::ParticipantsBound,
        AuditEventType::StepStarted,
        AuditEventType::StepCompleted,
        AuditEventType::StepFailed,
        AuditEventType::TransitionApplied,
        AuditEventType::InterventionRequested,
        AuditEventType::InterventionResponded,
        AuditEventType::InterventionClosed,
        AuditEventType::EvidenceCollected,
        AuditEventType::ReasonAsserted,
        AuditEventType::HumanApprovalRecorded,
        AuditEventType::HumanDeferralRecorded,
        AuditEventType::CeremonyCompleted,
    ] {
        assert!(
            coverage.get(&expected).is_some_and(|count| *count > 0),
            "{SEEDS} seeds never produced {expected:?}; coverage {coverage:?}"
        );
    }
}
