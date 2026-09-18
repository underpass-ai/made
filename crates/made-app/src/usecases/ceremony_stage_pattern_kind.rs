use made_core::error::DomainError;

/// Authoring-only coordination shape expanded into ordinary ceremony data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyStagePatternKind {
    Sequential,
    Concurrent,
    BroadcastCollect,
    GroupChat,
    MakerChecker,
    Handoff,
    Magentic,
}

impl CeremonyStagePatternKind {
    pub const ALL: [Self; 7] = [
        Self::Sequential,
        Self::Concurrent,
        Self::BroadcastCollect,
        Self::GroupChat,
        Self::MakerChecker,
        Self::Handoff,
        Self::Magentic,
    ];

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "sequential" => Ok(Self::Sequential),
            "concurrent" => Ok(Self::Concurrent),
            "broadcast_collect" => Ok(Self::BroadcastCollect),
            "group_chat" => Ok(Self::GroupChat),
            "maker_checker" => Ok(Self::MakerChecker),
            "handoff" => Ok(Self::Handoff),
            "magentic" => Ok(Self::Magentic),
            other => Err(DomainError::InvalidDocument {
                reason: format!("unknown ceremony stage pattern `{other}`"),
            }),
        }
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Concurrent => "concurrent",
            Self::BroadcastCollect => "broadcast_collect",
            Self::GroupChat => "group_chat",
            Self::MakerChecker => "maker_checker",
            Self::Handoff => "handoff",
            Self::Magentic => "magentic",
        }
    }

    #[must_use]
    pub const fn requires_cap(self) -> bool {
        matches!(
            self,
            Self::GroupChat | Self::MakerChecker | Self::Handoff | Self::Magentic
        )
    }
}
