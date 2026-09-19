use serde_json::{json, Value};

/// Transport-neutral fields of one sealed ceremony record.
///
/// Domain and protobuf adapters rebuild this view, then this type alone owns
/// the JSON contract. Optional fields are already represented as `Option`, so
/// proto3's empty scalar convention cannot leak into the rendered shape.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AuditRecordView {
    pub(crate) global_position: Option<u64>,
    pub(crate) event_id: String,
    pub(crate) event_type: String,
    pub(crate) schema_version: u32,
    pub(crate) ceremony_id: String,
    pub(crate) definition_name: String,
    pub(crate) definition_version: String,
    pub(crate) sequence: u64,
    pub(crate) occurred_at: String,
    pub(crate) actor: Value,
    pub(crate) correlation_id: Option<String>,
    pub(crate) causation_id: Option<String>,
    pub(crate) trace_id: Option<String>,
    pub(crate) event_schema_version: Option<u32>,
    pub(crate) event: Option<Value>,
    pub(crate) previous_record_hash: Option<Vec<u8>>,
    pub(crate) record_hash: Vec<u8>,
    pub(crate) authorization: Option<Value>,
}

impl AuditRecordView {
    /// Render public record fields. Schema 3 additionally carries the sealed
    /// authorization; persisted domain records use a separate v3 envelope.
    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        let mut rendered = json!({
            "event_id": self.event_id,
            "event_type": self.event_type,
            "schema_version": self.schema_version,
            "ceremony_id": self.ceremony_id,
            "definition_name": self.definition_name,
            "definition_version": self.definition_version,
            "sequence": self.sequence,
            "occurred_at": self.occurred_at,
            "actor": self.actor,
            "correlation_id": self.correlation_id,
            "causation_id": self.causation_id,
            "trace_id": self.trace_id,
            "event_schema_version": self.event_schema_version,
            "event": self.event,
            "previous_record_hash": self.previous_record_hash,
            "record_hash": self.record_hash,
        });
        if let Some(authorization) = &self.authorization {
            rendered["authorization"] = authorization.clone();
        }
        if let Some(position) = self.global_position {
            rendered["global_position"] = json!(position);
        }
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_complete_sealed_record_contract() {
        let rendered = AuditRecordView {
            global_position: None,
            event_id: "event-1".to_owned(),
            event_type: "ceremony_instance_started".to_owned(),
            schema_version: 2,
            ceremony_id: "ceremony-1".to_owned(),
            definition_name: "planning".to_owned(),
            definition_version: "1.0".to_owned(),
            sequence: 1,
            occurred_at: "2026-09-17T10:00:00Z".to_owned(),
            actor: json!({"actor_id": "operator", "kind": "service", "role_id": null}),
            correlation_id: Some("event-1".to_owned()),
            causation_id: None,
            trace_id: None,
            event_schema_version: Some(1),
            event: Some(json!({"type": "ceremony_instance_started"})),
            previous_record_hash: None,
            record_hash: vec![0; 32],
            authorization: None,
        }
        .to_json();

        assert_eq!(rendered["event_id"], "event-1");
        assert_eq!(rendered["actor"]["role_id"], Value::Null);
        assert_eq!(rendered["causation_id"], Value::Null);
        assert_eq!(rendered["event_schema_version"], 1);
        assert_eq!(rendered["record_hash"].as_array().map(Vec::len), Some(32));
        assert_eq!(rendered.as_object().map(serde_json::Map::len), Some(16));
    }
}
