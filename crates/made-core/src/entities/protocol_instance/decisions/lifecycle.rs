use crate::entities::protocol_commands::{
    CancelProtocol, EnforceProtocolDeadlines, PauseProtocol, ResumeProtocol,
};
use crate::entities::protocol_events::{
    ProtocolCancelled, ProtocolDeadlineExceeded, ProtocolPaused, ProtocolResumed,
    StateDeadlineExceeded, StepDeadlineExceeded,
};
use crate::entities::{ProtocolDefinition, ProtocolEvent, ProtocolInstance};
use crate::error::DomainError;
use crate::value_objects::{ProtocolEndReason, ProtocolLifecyclePhase, StepResult};

impl ProtocolInstance {
    pub(super) fn decide_pause(
        &self,
        command: &PauseProtocol,
        definition: &ProtocolDefinition,
    ) -> Result<Vec<ProtocolEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.is_ended() {
            return Err(DomainError::LifecycleRefused {
                operation: "pause",
                phase: ProtocolLifecyclePhase::Ended,
            });
        }
        match self.lifecycle.phase() {
            ProtocolLifecyclePhase::Running => {
                Ok(vec![ProtocolEvent::ProtocolPaused(ProtocolPaused {
                    reason: command.reason.clone(),
                    paused_at: command.now,
                })])
            }
            ProtocolLifecyclePhase::Paused => Ok(Vec::new()),
            phase @ ProtocolLifecyclePhase::Ended => Err(DomainError::LifecycleRefused {
                operation: "pause",
                phase,
            }),
        }
    }

    pub(super) fn decide_resume(
        &self,
        command: &ResumeProtocol,
        definition: &ProtocolDefinition,
    ) -> Result<Vec<ProtocolEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.is_ended() {
            return Err(DomainError::LifecycleRefused {
                operation: "resume",
                phase: ProtocolLifecyclePhase::Ended,
            });
        }
        // A session that sealed a handoff has said, on the record,
        // which definition it believes in, and it is not this one.
        // Cancelling stays available: abandoning a superseded origin
        // is still a legitimate decision.
        if self.is_superseded() {
            return Err(DomainError::LifecycleRefused {
                operation: "superseded_by_successor",
                phase: self.lifecycle.phase(),
            });
        }
        match self.lifecycle.phase() {
            ProtocolLifecyclePhase::Paused => {
                Ok(vec![ProtocolEvent::ProtocolResumed(ProtocolResumed {
                    resumed_at: command.now,
                })])
            }
            ProtocolLifecyclePhase::Running => Ok(Vec::new()),
            phase @ ProtocolLifecyclePhase::Ended => Err(DomainError::LifecycleRefused {
                operation: "resume",
                phase,
            }),
        }
    }

    pub(super) fn decide_cancel(
        &self,
        command: &CancelProtocol,
        definition: &ProtocolDefinition,
    ) -> Result<Vec<ProtocolEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.lifecycle.end_reason() == Some(ProtocolEndReason::Cancelled) {
            return Ok(Vec::new());
        }
        if self.is_ended() {
            return Err(DomainError::LifecycleRefused {
                operation: "cancel",
                phase: ProtocolLifecyclePhase::Ended,
            });
        }
        Ok(vec![ProtocolEvent::ProtocolCancelled(ProtocolCancelled {
            reason: command.reason.clone(),
            cancelled_at: command.now,
        })])
    }

    pub(super) fn decide_enforce_deadlines(
        &self,
        command: &EnforceProtocolDeadlines,
        definition: &ProtocolDefinition,
    ) -> Result<Vec<ProtocolEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.is_ended() {
            return Ok(Vec::new());
        }
        let protocol = self
            .protocol_deadline
            .filter(|deadline| deadline.at() <= command.now);
        let state = self
            .state_deadline
            .clone()
            .filter(|deadline| deadline.at() <= command.now);
        let terminal_at = match (protocol, state.as_ref()) {
            (Some(protocol), Some(state)) => Some(protocol.at().min(state.at())),
            (Some(protocol), None) => Some(protocol.at()),
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
                Ok(ProtocolEvent::StepDeadlineExceeded(StepDeadlineExceeded {
                    deadline,
                    result: StepResult::timed_out()?,
                    observed_at: command.now,
                }))
            })
            .collect::<Result<Vec<_>, DomainError>>()?;
        match (protocol, state) {
            (Some(protocol), Some(state)) if protocol.at() <= state.at() => events.push(
                ProtocolEvent::ProtocolDeadlineExceeded(ProtocolDeadlineExceeded {
                    deadline: protocol,
                    observed_at: command.now,
                }),
            ),
            (_, Some(state)) => events.push(ProtocolEvent::StateDeadlineExceeded(
                StateDeadlineExceeded {
                    deadline: state,
                    observed_at: command.now,
                },
            )),
            (Some(protocol), None) => events.push(ProtocolEvent::ProtocolDeadlineExceeded(
                ProtocolDeadlineExceeded {
                    deadline: protocol,
                    observed_at: command.now,
                },
            )),
            (None, None) => {}
        }
        Ok(events)
    }
}
