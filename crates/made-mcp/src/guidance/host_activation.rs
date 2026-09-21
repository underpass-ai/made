//! What a deployment can do about waking a host that is not asking.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::protocol::AWAIT_INTEGRATOR_ATTENTION_TOOL;

/// Which activation adapter a deployment has, and what a host that
/// cannot be woken does instead.
///
/// Every composition in this build installs the `none` adapter: an
/// offer is recorded and nobody is woken, so a host follows its scope
/// by asking. Reported rather than assumed by the reader, because the
/// answer is what tells a host whether waiting for a knock is a plan.
/// The `command` adapter arrives with D2, and this becomes something
/// the composed engine is asked rather than something said here.
pub(super) fn host_activation(names: &BTreeSet<String>) -> Value {
    if !names.contains(AWAIT_INTEGRATOR_ATTENTION_TOOL) {
        return Value::Null;
    }
    json!({
        "adapter": "none",
        "adapters": ["none", "command"],
        "bounded_follow": AWAIT_INTEGRATOR_ATTENTION_TOOL,
        "note": "An activation receipt is transport, never evidence that anybody acted.",
    })
}
