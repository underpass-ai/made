use made_core::error::DomainError;
use made_core::value_objects::{MemoryEntryId, MemoryMoment, MemoryScope};
use serde_json::{json, Value};

use super::{entry_ref, timestamp, PAGE_SIZE};

/// Reading everything is reading as of the end of time.
pub(in crate::kmp) fn end_of_time() -> MemoryMoment {
    MemoryMoment::at(time::macros::datetime!(9999-12-31 00:00:00 UTC))
}

/// The arguments for reading `scope` as it stood at `moment`.
pub(in crate::kmp) fn goto_arguments(
    scope: &MemoryScope,
    moment: MemoryMoment,
    cursor: Option<&str>,
) -> Result<Value, DomainError> {
    let at = match cursor {
        Some(reference) => json!({ "ref": reference }),
        None => json!({ "time": timestamp(moment.instant())? }),
    };
    Ok(json!({
        "about": scope.as_str(),
        "at": at,
        "include": { "evidence": true, "relations": true },
        "limit": { "entries": PAGE_SIZE },
    }))
}

/// The arguments for asking how one entry came from another.
pub(in crate::kmp) fn trace_arguments(
    scope: &MemoryScope,
    from: &MemoryEntryId,
    to: &MemoryEntryId,
) -> Value {
    json!({
        "from": entry_ref(scope, from),
        "to": entry_ref(scope, to),
    })
}
