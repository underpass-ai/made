//! The catalogue entries for putting an intervention in front of a
//! live agent and reading back whether it got there.

use serde_json::Value;

use crate::protocol::ceremony_schemas::{
    acknowledge_ceremony_agent_intervention_schema, get_ceremony_intervention_schema,
    list_ceremony_interventions_schema, pull_ceremony_agent_interventions_schema,
};
use crate::protocol::schema_primitives::tool_def;
use crate::protocol::tool_names::{
    ACKNOWLEDGE_CEREMONY_AGENT_INTERVENTION_TOOL, GET_CEREMONY_INTERVENTION_TOOL,
    LIST_CEREMONY_INTERVENTIONS_TOOL, PULL_CEREMONY_AGENT_INTERVENTIONS_TOOL,
};

pub(super) fn intervention_delivery_tool_catalog() -> Vec<Value> {
    vec![
        tool_def(
            PULL_CEREMONY_AGENT_INTERVENTIONS_TOOL,
            "Take the interventions put to a live agent, under an expiring lease. A lease is an offer and not a receipt: acknowledge what you were handed, or it comes back for somebody else.",
            pull_ceremony_agent_interventions_schema(),
        ),
        tool_def(
            ACKNOWLEDGE_CEREMONY_AGENT_INTERVENTION_TOOL,
            "Say what you saw of an intervention you were handed. Received, refused and incapable are sealed in the ceremony's stream and are the evidence that it arrived; busy and timeout count an attempt and put it back.",
            acknowledge_ceremony_agent_intervention_schema(),
        ),
        tool_def(
            GET_CEREMONY_INTERVENTION_TOOL,
            "Read one intervention with every route it took towards a host and where it stands. Never reports delivered without a lease or an activation receipt behind it.",
            get_ceremony_intervention_schema(),
        ),
        tool_def(
            LIST_CEREMONY_INTERVENTIONS_TOOL,
            "List a ceremony's interventions with their delivery status, filtered by status, seat, agent execution, or whether anybody still owes them something.",
            list_ceremony_interventions_schema(),
        ),
    ]
}
