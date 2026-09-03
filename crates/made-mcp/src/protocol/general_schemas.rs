use serde_json::{json, Value};

use super::schema_primitives::string_schema;

pub(super) fn help_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["audience"],
        "properties": {
            "audience": {
                "type": "string",
                "enum": ["user", "agent"],
                "description": "Choose concise product guidance for a person or operational guidance for an agent/host."
            }
        }
    })
}

pub(super) fn empty_object_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

pub(super) fn run_council_decision_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["contract_id", "description"],
        "properties": {
            "council_id": string_schema("Stable council id. Exactly one of council_id / specialty must be set."),
            "specialty": string_schema("Council specialty. Exactly one of council_id / specialty must be set."),
            "contract_id": string_schema("Registered contract id the deliberation winner must satisfy."),
            "description": string_schema("Free-form task description the council reads."),
            "external_context": external_context_bundle_schema(),
            "validation_mode": {
                "type": "string",
                "enum": [
                    "VALIDATION_MODE_UNSPECIFIED",
                    "VALIDATION_MODE_STRICT",
                    "VALIDATION_MODE_WARN"
                ],
                "description": "STRICT (default) fails when no candidate passes; WARN returns the top-ranked candidate even on failure."
            },
            "metadata": task_metadata_schema()
        },
        "oneOf": [
            { "required": ["council_id"], "not": { "required": ["specialty"] } },
            { "required": ["specialty"], "not": { "required": ["council_id"] } }
        ]
    })
}

// ---------------------------------------------------------------------------
// Composite schema fragments (kept in sync with `made.proto`)
// ---------------------------------------------------------------------------

pub(super) fn task_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["task_id", "description", "specialty"],
        "properties": {
            "task_id": string_schema("Stable task identifier."),
            "description": string_schema("Free-form prompt the council deliberates over."),
            "specialty": string_schema("Specialty label of the council to dispatch to."),
            "constraints": constraints_schema(),
            "attributes": {
                "type": "object",
                "additionalProperties": true,
                "description": "Opaque per-task attributes. Forwarded to agents and validators."
            },
            "external_context": external_context_bundle_schema(),
            "metadata": task_metadata_schema()
        }
    })
}

fn constraints_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "rubric": {
                "type": "object",
                "additionalProperties": true,
                "description": "Opaque rubric forwarded to agents and validators."
            },
            "rounds": { "type": "integer", "minimum": 0, "description": "Peer-review rounds (0 = adapter default)." },
            "num_agents": { "type": "integer", "minimum": 0, "description": "Requested parallelism (0 = use council size)." },
            "deadline_ms": { "type": "integer", "minimum": 0, "description": "Optional soft deadline in ms (0 = none)." },
            "output_contract": output_contract_schema()
        }
    })
}

pub(super) fn output_contract_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["contract_id", "format"],
        "properties": {
            "contract_id": string_schema("Stable contract identifier."),
            "format": {
                "type": "string",
                "enum": ["json_object"],
                "description": "Wire format. Only `json_object` is implemented today."
            },
            "fields": {
                "type": "object",
                "additionalProperties": output_field_rule_schema(),
                "description": "Map from field name to its rule."
            },
            "json_schema": {
                "type": "string",
                "description": "Optional embedded JSON Schema (draft 2020-12 or earlier). When non-empty, every proposal output is validated against it via the JsonSchemaValidator. Canonical Report-shape schema lives at api/examples/output-contracts/report.schema.json."
            }
        }
    })
}

fn output_field_rule_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "required": { "type": "boolean" },
            "allowed_string_values": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

fn external_context_bundle_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "bundle_id": string_schema("Caller-supplied bundle id."),
            "schema_version": string_schema("Bundle schema version label."),
            "summary": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "text": string_schema("Human-facing summary."),
                    "attributes": {
                        "type": "object",
                        "additionalProperties": true
                    }
                }
            },
            "items": {
                "type": "array",
                "items": context_item_schema()
            },
            "references": {
                "type": "array",
                "items": context_reference_schema()
            },
            "metadata": {
                "type": "object",
                "additionalProperties": true,
                "description": "Application-owned bundle metadata. made treats this as opaque."
            }
        }
    })
}

fn context_item_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["item_id", "kind"],
        "properties": {
            "item_id": string_schema("Stable item id within the bundle."),
            "kind": string_schema("Caller-defined kind label."),
            "title": { "type": "string" },
            "narrative": { "type": "string" },
            "attributes": { "type": "object", "additionalProperties": true },
            "reference_ids": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

fn context_reference_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["reference_id", "uri"],
        "properties": {
            "reference_id": string_schema("Stable reference id within the bundle."),
            "uri": string_schema("Pointer to the referenced artifact."),
            "title": { "type": "string" },
            "media_type": { "type": "string" },
            "attributes": { "type": "object", "additionalProperties": true }
        }
    })
}

fn task_metadata_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "source_event_id": { "type": "string" },
            "causation_id": { "type": "string" },
            "correlation_id": { "type": "string" },
            "council_contract_id": { "type": "string" },
            "output_contract_id": { "type": "string" },
            "execution_profile": {
                "type": "object",
                "additionalProperties": true,
                "description": "Opaque executor hints. Explicit Orchestrate options take precedence on overlap."
            }
        }
    })
}

pub(super) fn agent_summary_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["agent_id", "specialty", "kind"],
        "properties": {
            "agent_id": string_schema("Stable agent id."),
            "specialty": string_schema("Specialty the agent serves."),
            "kind": string_schema("Adapter-defined agent kind (e.g. noop, vllm, anthropic, openai)."),
            "attributes": {
                "type": "object",
                "additionalProperties": true,
                "description": "Per-agent factory hints (provider.model, provider.endpoint, …)."
            }
        }
    })
}

pub(super) fn trigger_event_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["event_id", "kind", "source", "requested_specialties"],
        "properties": {
            "event_id": string_schema("Stable producer-side event id."),
            "kind": string_schema("Free-form event kind (e.g. alert.fired, case.opened)."),
            "source": string_schema("Producer identifier."),
            "emitted_at": string_schema("RFC3339 emit timestamp. Server fills in `now` when absent."),
            "requested_specialties": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string" }
            },
            "task_description_template": { "type": "string" },
            "constraints": constraints_schema(),
            "payload": {
                "type": "object",
                "additionalProperties": true,
                "description": "Opaque domain payload."
            },
            "external_context": external_context_bundle_schema(),
            "correlation_id": { "type": "string" },
            "causation_id": { "type": "string" }
        }
    })
}
