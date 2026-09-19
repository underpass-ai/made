use serde_json::Value;

use super::super::{authorization_schemas, general_schemas::empty_object_schema};
use super::tool_def;

pub(super) fn insert_authorization_tools(tools: &mut Vec<Value>) {
    let position = tools
        .iter()
        .position(|tool| tool["name"] == "made_get_status")
        .expect("observability tools belong to the catalog");
    tools.splice(position..position, [
        tool_def("made_get_authorization_policy", "Read the configured policy, grants, revocations and separation rules. Requires authenticated policy read authority.", empty_object_schema()),
        tool_def("made_issue_authorization_grant", "Issue an explicit, scoped grant. Issuer and policy are supplied by the authenticated runtime, never by tool arguments.", authorization_schemas::grant_schema()),
        tool_def("made_revoke_authorization_grant", "Revoke a grant durably with a reason. Revoker identity comes from the authenticated channel.", authorization_schemas::revoke_schema()),
        tool_def("made_list_authorization_decisions", "Read a bounded page of persisted authorization decisions, including denials and their evidence.", authorization_schemas::decisions_schema()),
    ]);
}
