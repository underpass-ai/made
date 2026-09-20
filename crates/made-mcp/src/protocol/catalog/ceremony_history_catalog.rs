use serde_json::Value;

use super::super::ceremony_schemas::verify_ceremony_journal_schema;
use super::super::schema_primitives::tool_def;
use super::super::tool_names::VERIFY_CEREMONY_JOURNAL_TOOL;

pub(super) fn verify_ceremony_journal_tool() -> Value {
    tool_def(
        VERIFY_CEREMONY_JOURNAL_TOOL,
        "Verify the hash chain of a ceremony's journal: whether every record is sealed, positioned and linked as written, and where it stopped being trustworthy if it is not. Read-only. A broken chain is an answer, not an error.",
        verify_ceremony_journal_schema(),
    )
}
