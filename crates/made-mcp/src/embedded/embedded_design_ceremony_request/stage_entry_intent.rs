mod group_intent;
mod group_repeat_intent;
mod group_repeat_until_intent;
mod group_stage_intent;
mod join_intent;

use made_app::usecases::CeremonyDesignStageEntry;
use made_core::error::DomainError;
use serde::{de::Error as _, Deserialize, Deserializer};
use serde_json::Value;

use self::group_stage_intent::GroupStageIntent;
use super::StageIntent;

#[derive(Clone, Debug)]
pub(super) enum StageEntryIntent {
    Group(GroupStageIntent),
    Leaf(StageIntent),
}

impl<'de> Deserialize<'de> for StageEntryIntent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value
            .as_object()
            .is_some_and(|object| object.contains_key("group"))
        {
            serde_json::from_value(value)
                .map(Self::Group)
                .map_err(D::Error::custom)
        } else {
            serde_json::from_value(value)
                .map(Self::Leaf)
                .map_err(D::Error::custom)
        }
    }
}

impl StageEntryIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignStageEntry, DomainError> {
        match self {
            Self::Leaf(stage) => stage.into_domain().map(CeremonyDesignStageEntry::Leaf),
            Self::Group(stage) => stage.into_domain().map(CeremonyDesignStageEntry::Group),
        }
    }
}
