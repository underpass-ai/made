//! Structured output contract: proto → domain.

use std::collections::BTreeMap;

use choreo_core::error::DomainError;
use choreo_core::value_objects::{OutputContract, OutputFieldRule, OutputFormat};
use choreo_proto::v1 as pb;

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

    Ok(Some(OutputContract::new(
        contract.contract_id,
        format,
        fields,
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
        }))
        .unwrap()
        .unwrap();

        assert_eq!(contract.contract_id(), "decision-contract");
        assert_eq!(contract.format(), OutputFormat::JsonObject);
        assert!(contract.fields()["decision"].required());
    }
}
