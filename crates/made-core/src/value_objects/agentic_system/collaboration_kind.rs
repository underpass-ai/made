use std::fmt;

use serde::{Deserialize, Serialize};

/// What one arrow between two participants means.
///
/// Four kinds, because the picture of a system is unreadable when
/// every line means the same thing: talking to somebody, arranging
/// work with them, doing work for them and deciding what work is are
/// different relationships, and the diagram draws each differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollaborationKind {
    Communication,
    Coordination,
    Execution,
    Definition,
}

impl CollaborationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Communication => "communication",
            Self::Coordination => "coordination",
            Self::Execution => "execution",
            Self::Definition => "definition",
        }
    }

    /// The Mermaid edge that carries this meaning.
    #[must_use]
    pub const fn edge(self) -> &'static str {
        match self {
            Self::Communication => "-->",
            Self::Coordination => "-.->",
            Self::Execution => "==>",
            Self::Definition => "---",
        }
    }

    /// Every kind, in declaration order, for a legend that cannot fall
    /// behind the enum.
    #[must_use]
    pub const fn every() -> [Self; 4] {
        [
            Self::Communication,
            Self::Coordination,
            Self::Execution,
            Self::Definition,
        ]
    }
}

impl fmt::Display for CollaborationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn no_two_kinds_are_drawn_the_same_way() {
        let edges: BTreeSet<&str> = CollaborationKind::every()
            .into_iter()
            .map(CollaborationKind::edge)
            .collect();

        assert_eq!(edges.len(), CollaborationKind::every().len());
    }
}
