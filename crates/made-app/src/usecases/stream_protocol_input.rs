use made_core::value_objects::{
    ProtocolAgentExecutionId, ProtocolEventPageLimit, ProtocolId, ProtocolProgressWait, RoleId,
    StepId, StreamVersion,
};

/// Resume point and bounds for one protocol progress stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamProtocolInput {
    protocol_id: ProtocolId,
    after_sequence: StreamVersion,
    max_events: ProtocolEventPageLimit,
    wait_timeout: ProtocolProgressWait,
    after_activity_sequence: Option<StreamVersion>,
    role_id: Option<RoleId>,
    step_id: Option<StepId>,
    agent_execution_id: Option<ProtocolAgentExecutionId>,
}

impl StreamProtocolInput {
    #[must_use]
    pub const fn new(
        protocol_id: ProtocolId,
        after_sequence: StreamVersion,
        max_events: ProtocolEventPageLimit,
        wait_timeout: ProtocolProgressWait,
    ) -> Self {
        Self {
            protocol_id,
            after_sequence,
            max_events,
            wait_timeout,
            after_activity_sequence: None,
            role_id: None,
            step_id: None,
            agent_execution_id: None,
        }
    }

    #[must_use]
    pub fn with_agent_activity(
        mut self,
        after_activity_sequence: u64,
        role_id: Option<RoleId>,
        step_id: Option<StepId>,
        agent_execution_id: Option<ProtocolAgentExecutionId>,
    ) -> Self {
        self.after_activity_sequence = Some(StreamVersion::new(after_activity_sequence));
        self.role_id = role_id;
        self.step_id = step_id;
        self.agent_execution_id = agent_execution_id;
        self
    }

    #[must_use]
    pub const fn protocol_id(&self) -> &ProtocolId {
        &self.protocol_id
    }

    #[must_use]
    pub const fn after_sequence(&self) -> StreamVersion {
        self.after_sequence
    }

    #[must_use]
    pub const fn max_events(&self) -> ProtocolEventPageLimit {
        self.max_events
    }

    #[must_use]
    pub const fn wait_timeout(&self) -> ProtocolProgressWait {
        self.wait_timeout
    }
    #[must_use]
    pub const fn includes_agent_activity(&self) -> bool {
        self.after_activity_sequence.is_some()
    }
    #[must_use]
    pub const fn after_activity_sequence(&self) -> StreamVersion {
        match self.after_activity_sequence {
            Some(value) => value,
            None => StreamVersion::EMPTY,
        }
    }
    #[must_use]
    pub const fn role_id(&self) -> Option<&RoleId> {
        self.role_id.as_ref()
    }
    #[must_use]
    pub const fn step_id(&self) -> Option<&StepId> {
        self.step_id.as_ref()
    }
    #[must_use]
    pub const fn agent_execution_id(&self) -> Option<&ProtocolAgentExecutionId> {
        self.agent_execution_id.as_ref()
    }
}
