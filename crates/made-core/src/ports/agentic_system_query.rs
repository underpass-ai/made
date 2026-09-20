use crate::value_objects::{AgenticSystemId, AgenticSystemLifecycle, AgenticSystemPageLimit};

/// Which designs a listing wants, and how many of them.
///
/// The cursor is the last identifier the previous page returned rather
/// than an opaque token: the catalogue is ordered by identity, the
/// order is public, and a token that only re-encoded it would be one
/// more thing to keep true.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgenticSystemQuery {
    lifecycle: Option<AgenticSystemLifecycle>,
    limit: AgenticSystemPageLimit,
    after: Option<AgenticSystemId>,
}

impl AgenticSystemQuery {
    #[must_use]
    pub const fn new(
        lifecycle: Option<AgenticSystemLifecycle>,
        limit: AgenticSystemPageLimit,
        after: Option<AgenticSystemId>,
    ) -> Self {
        Self {
            lifecycle,
            limit,
            after,
        }
    }

    #[must_use]
    pub const fn lifecycle(&self) -> Option<AgenticSystemLifecycle> {
        self.lifecycle
    }

    #[must_use]
    pub const fn limit(&self) -> AgenticSystemPageLimit {
        self.limit
    }

    #[must_use]
    pub const fn after(&self) -> Option<&AgenticSystemId> {
        self.after.as_ref()
    }

    /// Whether this design belongs in the answer.
    #[must_use]
    pub fn admits(&self, id: &AgenticSystemId, lifecycle: AgenticSystemLifecycle) -> bool {
        self.lifecycle.is_none_or(|wanted| wanted == lifecycle)
            && self.after.as_ref().is_none_or(|after| id > after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> AgenticSystemId {
        AgenticSystemId::new(raw).unwrap()
    }

    #[test]
    fn a_cursor_excludes_the_page_already_read() {
        let query = AgenticSystemQuery::new(
            Some(AgenticSystemLifecycle::Published),
            AgenticSystemPageLimit::default(),
            Some(id("b")),
        );

        assert!(!query.admits(&id("a"), AgenticSystemLifecycle::Published));
        assert!(!query.admits(&id("b"), AgenticSystemLifecycle::Published));
        assert!(query.admits(&id("c"), AgenticSystemLifecycle::Published));
        assert!(!query.admits(&id("c"), AgenticSystemLifecycle::Draft));
    }
}
