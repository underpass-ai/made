use serde_json::{json, Value};

/// A readable session or the reason its persisted stream cannot be rehydrated.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CeremonyInstanceListingEntry {
    Rehydratable(Value),
    Unrehydratable { ceremony_id: String, reason: String },
}

impl CeremonyInstanceListingEntry {
    #[must_use]
    pub(crate) fn rehydratable(instance: Value) -> Self {
        Self::Rehydratable(instance)
    }

    #[must_use]
    pub(crate) fn unrehydratable(
        ceremony_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::Unrehydratable {
            ceremony_id: ceremony_id.into(),
            reason: reason.into(),
        }
    }

    pub(super) fn to_json(&self) -> Value {
        match self {
            Self::Rehydratable(instance) => {
                let mut rendered = instance.clone();
                if let Some(fields) = rendered.as_object_mut() {
                    fields.insert("rehydratable".to_owned(), Value::Bool(true));
                    fields.insert("reason".to_owned(), Value::Null);
                }
                rendered
            }
            Self::Unrehydratable {
                ceremony_id,
                reason,
            } => json!({
                "ceremony_id": ceremony_id,
                "rehydratable": false,
                "reason": reason,
            }),
        }
    }
}
