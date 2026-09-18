use crate::entities::ceremony_commands::{
    CancelCeremony, EnforceCeremonyDeadlines, PauseCeremony, ResumeCeremony,
};
use crate::entities::ceremony_events::{
    CeremonyCancelled, CeremonyDeadlineExceeded, CeremonyPaused, CeremonyResumed,
    StateDeadlineExceeded, StepDeadlineExceeded,
};
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{CeremonyEndReason, CeremonyLifecyclePhase, StepResult};

impl CeremonyInstance {
    pub(super) fn decide_pause(
        &self,
        command: &PauseCeremony,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.is_ended() {
            return Err(DomainError::LifecycleRefused {
                operation: "pause",
                phase: CeremonyLifecyclePhase::Ended,
            });
        }
        match self.lifecycle.phase() {
            CeremonyLifecyclePhase::Running => {
                Ok(vec![CeremonyEvent::CeremonyPaused(CeremonyPaused {
                    reason: command.reason.clone(),
                    paused_at: command.now,
                })])
            }
            CeremonyLifecyclePhase::Paused => Ok(Vec::new()),
            phase @ CeremonyLifecyclePhase::Ended => Err(DomainError::LifecycleRefused {
                operation: "pause",
                phase,
            }),
        }
    }

    pub(super) fn decide_resume(
        &self,
        command: &ResumeCeremony,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.is_ended() {
            return Err(DomainError::LifecycleRefused {
                operation: "resume",
                phase: CeremonyLifecyclePhase::Ended,
            });
        }
        match self.lifecycle.phase() {
            CeremonyLifecyclePhase::Paused => {
                Ok(vec![CeremonyEvent::CeremonyResumed(CeremonyResumed {
                    resumed_at: command.now,
                })])
            }
            CeremonyLifecyclePhase::Running => Ok(Vec::new()),
            phase @ CeremonyLifecyclePhase::Ended => Err(DomainError::LifecycleRefused {
                operation: "resume",
                phase,
            }),
        }
    }

    pub(super) fn decide_cancel(
        &self,
        command: &CancelCeremony,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.lifecycle.end_reason() == Some(CeremonyEndReason::Cancelled) {
            return Ok(Vec::new());
        }
        if self.is_ended() {
            return Err(DomainError::LifecycleRefused {
                operation: "cancel",
                phase: CeremonyLifecyclePhase::Ended,
            });
        }
        Ok(vec![CeremonyEvent::CeremonyCancelled(CeremonyCancelled {
            reason: command.reason.clone(),
            cancelled_at: command.now,
        })])
    }

    pub(super) fn decide_enforce_deadlines(
        &self,
        command: &EnforceCeremonyDeadlines,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.is_ended() {
            return Ok(Vec::new());
        }
        let ceremony = self
            .ceremony_deadline
            .filter(|deadline| deadline.at() <= command.now);
        let state = self
            .state_deadline
            .clone()
            .filter(|deadline| deadline.at() <= command.now);
        let terminal_at = match (ceremony, state.as_ref()) {
            (Some(ceremony), Some(state)) => Some(ceremony.at().min(state.at())),
            (Some(ceremony), None) => Some(ceremony.at()),
            (None, Some(state)) => Some(state.at()),
            (None, None) => None,
        };
        let mut steps = self
            .step_deadlines
            .values()
            .filter(|deadline| {
                deadline.at() <= command.now && terminal_at.is_none_or(|at| deadline.at() < at)
            })
            .cloned()
            .collect::<Vec<_>>();
        steps.sort_by(|left, right| {
            left.at()
                .cmp(&right.at())
                .then_with(|| left.step_id().cmp(right.step_id()))
        });
        let mut events = steps
            .into_iter()
            .map(|deadline| {
                Ok(CeremonyEvent::StepDeadlineExceeded(StepDeadlineExceeded {
                    deadline,
                    result: StepResult::timed_out()?,
                    observed_at: command.now,
                }))
            })
            .collect::<Result<Vec<_>, DomainError>>()?;
        match (ceremony, state) {
            (Some(ceremony), Some(state)) if ceremony.at() <= state.at() => events.push(
                CeremonyEvent::CeremonyDeadlineExceeded(CeremonyDeadlineExceeded {
                    deadline: ceremony,
                    observed_at: command.now,
                }),
            ),
            (_, Some(state)) => events.push(CeremonyEvent::StateDeadlineExceeded(
                StateDeadlineExceeded {
                    deadline: state,
                    observed_at: command.now,
                },
            )),
            (Some(ceremony), None) => events.push(CeremonyEvent::CeremonyDeadlineExceeded(
                CeremonyDeadlineExceeded {
                    deadline: ceremony,
                    observed_at: command.now,
                },
            )),
            (None, None) => {}
        }
        Ok(events)
    }
}
