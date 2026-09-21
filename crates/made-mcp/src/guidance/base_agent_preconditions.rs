//! What every agent is told, whatever this server serves.
//!
//! Its own file for the same reason the boundaries next door have one:
//! these five hold regardless of which tools a backend admits, and the
//! ones that depend on the catalogue are added beside them where the
//! catalogue is read. Keeping the unconditional half here is what makes
//! the conditional half legible.

use crate::protocol::DISCOVER_CAPABILITIES_TOOL;

pub(super) fn base_agent_preconditions() -> Vec<String> {
    vec![
        format!(
            "Call {DISCOVER_CAPABILITIES_TOOL} and plan against its returned tools, backend, and version."
        ),
        "Preserve stable ceremony ids and actor identity across calls.".to_owned(),
        "For delegated work, resolve an explicit host execution profile before claiming: keep requested and actual model/effort, capabilities, fallback, inheritance, and host incarnation separate from the ceremony definition.".to_owned(),
        "Have the exact definition, required context, and host permissions before starting."
            .to_owned(),
        "Treat isError=true, completed=false, and missing evidence as explicit non-success."
            .to_owned(),
    ]
}
