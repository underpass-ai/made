//! Structured output contract: proto ↔ domain.

use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{OutputContract, OutputFieldRule, OutputFormat};
use made_proto::v1 as pb;

/// Project a domain [`OutputContract`] onto its proto representation.
///
/// Used by the contract CRUD RPCs (`ListContracts`, `RegisterContract`
/// echo-back) so consumers see exactly what was stored in the
/// registry.
#[must_use]
pub fn output_contract_to_proto(contract: &OutputContract) -> pb::OutputContract {
    let fields = contract
        .fields()
        .iter()
        .map(|(name, rule)| {
            (
                name.clone(),
                pb::OutputFieldRule {
                    required: rule.required(),
                    allowed_string_values: rule.allowed_string_values().iter().cloned().collect(),
                },
            )
        })
        .collect();
    let format = match contract.format() {
        OutputFormat::JsonObject => pb::OutputFormat::JsonObject as i32,
    };
    pb::OutputContract {
        contract_id: contract.contract_id().as_str().to_owned(),
        format,
        fields,
        json_schema: contract.json_schema().to_owned(),
    }
}

pub fn output_contract_from_proto(
    contract: Option<pb::OutputContract>,
) -> Result<Option<OutputContract>, DomainError> {
    let Some(contract) = contract else {
        return Ok(None);
    };

    let format = match pb::OutputFormat::try_from(contract.format)
        .unwrap_or(pb::OutputFormat::Unspecified)
    {
        pb::OutputFormat::Unspecified | pb::OutputFormat::JsonObject => OutputFormat::JsonObject,
    };

    let fields = contract
        .fields
        .into_iter()
        .map(|(field_name, rule)| {
            Ok::<_, DomainError>((
                field_name,
                OutputFieldRule::new(rule.required, rule.allowed_string_values)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    Ok(Some(OutputContract::new_with_schema(
        contract.contract_id,
        format,
        fields,
        contract.json_schema,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_maps_to_none() {
        assert!(output_contract_from_proto(None).unwrap().is_none());
    }

    #[test]
    fn output_contract_maps_to_domain() {
        let contract = output_contract_from_proto(Some(pb::OutputContract {
            contract_id: "decision-contract".to_owned(),
            format: pb::OutputFormat::JsonObject as i32,
            fields: std::collections::HashMap::from([(
                "decision".to_owned(),
                pb::OutputFieldRule {
                    required: true,
                    allowed_string_values: vec!["emit_event".to_owned(), "escalate".to_owned()],
                },
            )]),
            json_schema: String::new(),
        }))
        .unwrap()
        .unwrap();

        assert_eq!(contract.contract_id(), "decision-contract");
        assert_eq!(contract.format(), OutputFormat::JsonObject);
        assert!(contract.fields()["decision"].required());
        assert!(contract.json_schema().is_empty());
    }

    #[test]
    fn embedded_json_schema_maps_through() {
        let contract = output_contract_from_proto(Some(pb::OutputContract {
            contract_id: "decision-contract".to_owned(),
            format: pb::OutputFormat::JsonObject as i32,
            fields: std::collections::HashMap::new(),
            json_schema: r#"{"type":"object","required":["decision"]}"#.to_owned(),
        }))
        .unwrap()
        .unwrap();

        assert_eq!(
            contract.json_schema(),
            r#"{"type":"object","required":["decision"]}"#
        );
    }
}
