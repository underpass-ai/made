//! What a deployment can do about waking a host that is not asking.

use std::collections::BTreeSet;

use made_core::value_objects::HostActivationAdapterKind;
use serde_json::{json, Value};

use crate::protocol::AWAIT_INTEGRATOR_ATTENTION_TOOL;

/// Which activation adapter this deployment composed, and what a host
/// that cannot be woken does instead.
///
/// The value comes from the composed engine rather than from a constant
/// here: the adapter is chosen at startup — by the operator's
/// `MADE_HOST_ACTIVATION_COMMAND` or by a host wiring its own port —
/// and a discovery answer written into the source would go on saying
/// `none` to a host that was about to be knocked on. With `none` an
/// offer is recorded and nobody is woken, so a host follows its scope
/// by asking; the answer is what tells it whether waiting for a knock
/// is a plan.
pub(super) fn host_activation(
    names: &BTreeSet<String>,
    adapter: HostActivationAdapterKind,
) -> Value {
    if !names.contains(AWAIT_INTEGRATOR_ATTENTION_TOOL) {
        return Value::Null;
    }
    json!({
        "adapter": adapter.as_str(),
        "adapters": ["none", "command"],
        "bounded_follow": AWAIT_INTEGRATOR_ATTENTION_TOOL,
        "note": "An activation receipt is transport, never evidence that anybody acted.",
    })
}
