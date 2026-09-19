use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::SessionRecollection;

use super::{
    CeremonyContext, CeremonyDefinitionDigest, CeremonyId, CeremonyLineage, CeremonyName,
    CeremonyVersion, ChildPosition,
};
use crate::value_objects::BudgetAccountId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedChild {
    child_id: CeremonyId,
    position: ChildPosition,
    ceremony: CeremonyName,
    version: CeremonyVersion,
    digest: CeremonyDefinitionDigest,
    context: CeremonyContext,
    lineage: CeremonyLineage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recollection: Option<SessionRecollection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_account_id: Option<BudgetAccountId>,
    #[serde(with = "time::serde::rfc3339")]
    opened_at: OffsetDateTime,
}

impl PlannedChild {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        child_id: CeremonyId,
        position: ChildPosition,
        ceremony: CeremonyName,
        version: CeremonyVersion,
        digest: CeremonyDefinitionDigest,
        context: CeremonyContext,
        lineage: CeremonyLineage,
        recollection: Option<SessionRecollection>,
        opened_at: OffsetDateTime,
    ) -> Self {
        Self {
            child_id,
            position,
            ceremony,
            version,
            digest,
            context,
            lineage,
            recollection,
            budget_account_id: None,
            opened_at,
        }
    }

    #[must_use]
    pub fn with_budget_account_id(mut self, account_id: Option<BudgetAccountId>) -> Self {
        self.budget_account_id = account_id;
        self
    }
    #[must_use]
    pub fn child_id(&self) -> &CeremonyId {
        &self.child_id
    }
    #[must_use]
    pub const fn position(&self) -> ChildPosition {
        self.position
    }
    #[must_use]
    pub fn ceremony(&self) -> &CeremonyName {
        &self.ceremony
    }
    #[must_use]
    pub fn version(&self) -> &CeremonyVersion {
        &self.version
    }
    #[must_use]
    pub const fn digest(&self) -> CeremonyDefinitionDigest {
        self.digest
    }
    #[must_use]
    pub fn context(&self) -> &CeremonyContext {
        &self.context
    }
    #[must_use]
    pub fn lineage(&self) -> &CeremonyLineage {
        &self.lineage
    }
    #[must_use]
    pub fn recollection(&self) -> Option<&SessionRecollection> {
        self.recollection.as_ref()
    }
    #[must_use]
    pub fn budget_account_id(&self) -> Option<&BudgetAccountId> {
        self.budget_account_id.as_ref()
    }
    #[must_use]
    pub const fn opened_at(&self) -> OffsetDateTime {
        self.opened_at
    }
}
