//! Only the counted guard needs a map for the internally tagged enum.
//! Its former scalar newtype could never produce canonical bytes or a digest.
//! Keep the public variant and the validated count, and leave every other
//! guard's serialization untouched.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::JoinStepCount;

#[derive(Serialize, Deserialize)]
struct CountedJoinCondition {
    count: JoinStepCount,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // Serde's field adapter receives a reference.
pub(super) fn serialize<S: Serializer>(
    count: &JoinStepCount,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    CountedJoinCondition { count: *count }.serialize(serializer)
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<JoinStepCount, D::Error> {
    CountedJoinCondition::deserialize(deserializer).map(|condition| condition.count)
}
