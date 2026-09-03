use async_trait::async_trait;
use made_core::entities::{TaskConstraints, ValidatorReport};
use made_core::error::DomainError;
use made_core::ports::ValidatorPort;
use made_core::value_objects::Attributes;
use serde_json::{json, Value};

use super::json_validation::{attributes, strip_markdown_fences};
use super::shape_violation::ShapeViolation;

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub struct BoundedEventShapeValidator {
    pub(super) max_total_size_bytes: usize,
    pub(super) max_depth: usize,
    pub(super) max_object_keys: usize,
    pub(super) max_array_len: usize,
    pub(super) max_string_len: usize,
}

/// Defends downstream consumers against pathological structured
/// outputs: payloads that nest too deep, carry too many keys, or
/// blow up an array. A consumer that trusts MADE's
/// `OutputContract` chain should never have to second-guess these
/// limits; a `JsonSchema` can constrain shape but not raw size.
///
/// The validator runs **only** when the task has an `OutputContract`
/// — without one there is no structured-output expectation to bound.
/// In that case it short-circuits with a pass and a `not applicable`
/// note, mirroring the JsonSchemaValidator's posture.
///
/// Limits are conservative by default and tunable through the
/// `with_*` builder methods. Each one is an inclusive upper bound;
/// the validator counts at most one violation per dimension so a
/// huge payload that breaks several limits still produces a small,
/// human-readable report.
impl BoundedEventShapeValidator {
    /// Defaults chosen for use cases where the output is fed into a
    /// downstream event bus or audit log:
    ///
    /// - 256 KiB total — matches the `OutputContract.json_schema`
    ///   cap so the schema and the validated instance share a budget.
    /// - 32 levels of nesting — deeper than any realistic Report.
    /// - 256 object keys — Report fields plus generous extension room.
    /// - 1024 array elements — caps repeated-findings explosions.
    /// - 64 KiB per string — a single field cannot dominate the
    ///   envelope.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_total_size_bytes: 256 * 1024,
            max_depth: 32,
            max_object_keys: 256,
            max_array_len: 1024,
            max_string_len: 64 * 1024,
        }
    }

    #[must_use]
    pub const fn with_max_total_size_bytes(mut self, bytes: usize) -> Self {
        self.max_total_size_bytes = bytes;
        self
    }
    #[must_use]
    pub const fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }
    #[must_use]
    pub const fn with_max_object_keys(mut self, keys: usize) -> Self {
        self.max_object_keys = keys;
        self
    }
    #[must_use]
    pub const fn with_max_array_len(mut self, len: usize) -> Self {
        self.max_array_len = len;
        self
    }
    #[must_use]
    pub const fn with_max_string_len(mut self, len: usize) -> Self {
        self.max_string_len = len;
        self
    }
}

impl Default for BoundedEventShapeValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidatorPort for BoundedEventShapeValidator {
    fn kind(&self) -> &'static str {
        "output-bounded-event-shape"
    }

    async fn validate(
        &self,
        proposal_content: &str,
        constraints: &TaskConstraints,
    ) -> Result<ValidatorReport, DomainError> {
        let Some(contract) = constraints.output_contract() else {
            return ValidatorReport::new(
                self.kind(),
                true,
                "no structured output contract configured",
                Attributes::empty(),
            );
        };

        let trimmed = proposal_content.trim();
        let total_bytes = trimmed.len();
        if total_bytes > self.max_total_size_bytes {
            return ValidatorReport::new(
                self.kind(),
                false,
                format!(
                    "proposal is {total_bytes} bytes; limit is {} bytes",
                    self.max_total_size_bytes
                ),
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "violation": "max_total_size_bytes",
                    "limit": self.max_total_size_bytes,
                    "actual": total_bytes,
                }))?,
            );
        }

        let instance: Value = match serde_json::from_str(strip_markdown_fences(trimmed)) {
            Ok(v) => v,
            Err(err) => {
                return ValidatorReport::new(
                    self.kind(),
                    false,
                    format!("proposal is not valid JSON: {err}"),
                    attributes(json!({ "contract_id": contract.contract_id() }))?,
                );
            }
        };

        if let Some(violation) = self.walk(&instance, "$", 0) {
            return ValidatorReport::new(
                self.kind(),
                false,
                violation.summary(),
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "violation": violation.kind,
                    "path": violation.path,
                    "limit": violation.limit,
                    "actual": violation.actual,
                }))?,
            );
        }

        ValidatorReport::new(
            self.kind(),
            true,
            "proposal satisfies the event-shape budget",
            attributes(json!({ "contract_id": contract.contract_id() }))?,
        )
    }
}

impl BoundedEventShapeValidator {
    fn walk(&self, value: &Value, path: &str, depth: usize) -> Option<ShapeViolation> {
        if depth > self.max_depth {
            return Some(ShapeViolation {
                kind: "max_depth",
                path: path.to_owned(),
                limit: self.max_depth,
                actual: depth,
            });
        }
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => None,
            Value::String(s) => {
                if s.len() > self.max_string_len {
                    Some(ShapeViolation {
                        kind: "max_string_len",
                        path: path.to_owned(),
                        limit: self.max_string_len,
                        actual: s.len(),
                    })
                } else {
                    None
                }
            }
            Value::Array(items) => {
                if items.len() > self.max_array_len {
                    return Some(ShapeViolation {
                        kind: "max_array_len",
                        path: path.to_owned(),
                        limit: self.max_array_len,
                        actual: items.len(),
                    });
                }
                for (i, item) in items.iter().enumerate() {
                    if let Some(v) = self.walk(item, &format!("{path}[{i}]"), depth + 1) {
                        return Some(v);
                    }
                }
                None
            }
            Value::Object(map) => {
                if map.len() > self.max_object_keys {
                    return Some(ShapeViolation {
                        kind: "max_object_keys",
                        path: path.to_owned(),
                        limit: self.max_object_keys,
                        actual: map.len(),
                    });
                }
                for (key, child) in map {
                    if let Some(v) = self.walk(child, &format!("{path}.{key}"), depth + 1) {
                        return Some(v);
                    }
                }
                None
            }
        }
    }
}
