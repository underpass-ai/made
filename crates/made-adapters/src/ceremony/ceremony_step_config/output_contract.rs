use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{OutputContract, OutputFieldRule, OutputFormat};

use super::{
    contract_key, field, key, optional_string, required_string, string_array, CeremonyStepConfig,
};

impl CeremonyStepConfig<'_> {
    /// Structured output contract declared on the step, if any.
    ///
    /// Shape (all under the step's `config`):
    ///
    /// ```yaml
    /// output_contract:
    ///   contract_id: evidence-bound-decision   # required
    ///   format: json_object                    # optional; only value today
    ///   required_fields: [claims, decision]    # optional
    ///   allowed_values:                        # optional, per field
    ///     decision: [accept, reject, request_changes, request_more_evidence]
    ///   json_schema: '{"type":"object"}'       # optional, inline body
    /// ```
    ///
    /// `required_fields` and `allowed_values` merge into per-field rules
    /// (a field named only under `allowed_values` is constrained but not
    /// required). Unknown keys inside the block are rejected: a typo in a
    /// policy gate must fail the parse, not silently weaken the contract.
    /// Enforcement itself is the existing deliberation pipeline — the
    /// contract validators run per proposal, and when no proposal
    /// satisfies the contract the step fails with
    /// `NoValidProposal { contract_id }` (the ceremony retries per its
    /// retry policy and otherwise stops at the guard).
    pub(crate) fn output_contract(&self) -> Result<Option<OutputContract>, DomainError> {
        let Some(value) = self.attributes.get(key::OUTPUT_CONTRACT) else {
            return Ok(None);
        };
        if value.is_null() {
            return Ok(None);
        }
        let Some(block) = value.as_object() else {
            return Err(DomainError::InvalidCharacters {
                field: field::OUTPUT_CONTRACT,
            });
        };

        let known = [
            contract_key::CONTRACT_ID,
            contract_key::FORMAT,
            contract_key::REQUIRED_FIELDS,
            contract_key::ALLOWED_VALUES,
            contract_key::JSON_SCHEMA,
            contract_key::EVIDENCE,
        ];
        if block.keys().any(|key| !known.contains(&key.as_str())) {
            return Err(DomainError::InvalidCharacters {
                field: field::OUTPUT_CONTRACT,
            });
        }

        let contract_id =
            required_string(block.get(contract_key::CONTRACT_ID), field::CONTRACT_ID)?;

        let format = match optional_string(block.get(contract_key::FORMAT)) {
            None | Some("json_object") => OutputFormat::JsonObject,
            Some(_) => {
                return Err(DomainError::InvalidCharacters {
                    field: field::CONTRACT_FORMAT,
                })
            }
        };

        let required = string_array(
            block.get(contract_key::REQUIRED_FIELDS),
            field::CONTRACT_REQUIRED_FIELDS,
        )?;

        let mut allowed: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Some(value) = block.get(contract_key::ALLOWED_VALUES) {
            if !value.is_null() {
                let Some(map) = value.as_object() else {
                    return Err(DomainError::InvalidCharacters {
                        field: field::CONTRACT_ALLOWED_VALUES,
                    });
                };
                for (field_name, values) in map {
                    allowed.insert(
                        field_name.clone(),
                        string_array(Some(values), field::CONTRACT_ALLOWED_VALUES)?,
                    );
                }
            }
        }

        let mut fields: BTreeMap<String, OutputFieldRule> = BTreeMap::new();
        for name in &required {
            let values = allowed.remove(name).unwrap_or_default();
            fields.insert(name.clone(), OutputFieldRule::new(true, values)?);
        }
        for (name, values) in allowed {
            fields.insert(name, OutputFieldRule::new(false, values)?);
        }

        let json_schema = optional_string(block.get(contract_key::JSON_SCHEMA)).unwrap_or_default();
        if let Some(value) = block.get(contract_key::JSON_SCHEMA) {
            if !value.is_null() && !value.is_string() {
                return Err(DomainError::InvalidCharacters {
                    field: field::CONTRACT_JSON_SCHEMA,
                });
            }
        }

        Ok(Some(OutputContract::new_with_schema(
            contract_id,
            format,
            fields,
            json_schema,
        )?))
    }
}
