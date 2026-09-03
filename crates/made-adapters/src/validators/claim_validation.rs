use made_core::value_objects::EvidenceGroundingRule;
use serde_json::{json, Map, Value};

use super::json_validation::value_type_name;

const CLAIM_TEXT_PREVIEW_LEN: usize = 80;

/// Grounding violations for one claim: shape problems, absent or empty
/// refs, non-string refs, and refs outside the allowed pack.
pub(super) fn claim_violations(
    index: usize,
    claim: &Value,
    rule: &EvidenceGroundingRule,
) -> Vec<Value> {
    let Some(claim_object) = claim.as_object() else {
        return vec![json!({
            "claim_index": index,
            "problem": "claim is not a JSON object",
            "actual_type": value_type_name(claim),
        })];
    };
    let preview = claim_preview(claim_object);
    let Some(refs) = claim_object
        .get(rule.refs_field())
        .and_then(Value::as_array)
    else {
        return vec![json!({
            "claim_index": index,
            "claim_preview": preview,
            "problem": format!(
                "refs field `{}` is missing or not an array",
                rule.refs_field()
            ),
        })];
    };
    if refs.is_empty() {
        return vec![json!({
            "claim_index": index,
            "claim_preview": preview,
            "problem": "claim cites no evidence",
        })];
    }

    let mut violations = Vec::new();
    let mut orphans = Vec::new();
    for reference in refs {
        match reference.as_str() {
            Some(id) if rule.allowed_refs().contains(id) => {}
            Some(id) => orphans.push(id.to_owned()),
            None => violations.push(json!({
                "claim_index": index,
                "claim_preview": preview,
                "problem": "evidence ref is not a string",
                "actual_type": value_type_name(reference),
            })),
        }
    }
    if !orphans.is_empty() {
        violations.push(json!({
            "claim_index": index,
            "claim_preview": preview,
            "problem": "evidence refs not present in the allowed pack",
            "orphan_refs": orphans,
        }));
    }
    violations
}

/// Short human preview of a claim for violation details: its `text`
/// field when present, otherwise the serialized object, truncated at a
/// char boundary.
pub(super) fn claim_preview(claim: &Map<String, Value>) -> String {
    let raw = claim
        .get("text")
        .and_then(Value::as_str)
        .map_or_else(|| Value::Object(claim.clone()).to_string(), str::to_owned);
    if raw.len() > CLAIM_TEXT_PREVIEW_LEN {
        let mut end = CLAIM_TEXT_PREVIEW_LEN;
        while !raw.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &raw[..end])
    } else {
        raw
    }
}
