use made_core::error::DomainError;
use made_core::value_objects::{PriorContext, Rounds, StepId, StepInstructions};

use super::{CeremonyDesignDocument, CeremonyDesignStage};
use crate::usecases::CeremonyPatternPreset;

pub(super) fn materialize(
    document: &CeremonyDesignDocument,
) -> Result<CeremonyDesignDocument, DomainError> {
    let Some(pattern) = document.pattern() else {
        return Ok(document.clone());
    };
    if !document.stages().is_empty() {
        return Err(invalid(
            "fields `pattern` and `stages` are mutually exclusive",
        ));
    }
    if pattern.fragment_source().trim().is_empty() {
        return Err(invalid(format!(
            "pattern `{}` has no shipped definition fragment",
            pattern.id()
        )));
    }
    match pattern {
        CeremonyPatternPreset::RoundtableFixedOrder => roundtable_fixed_order(document),
    }
}

fn roundtable_fixed_order(
    document: &CeremonyDesignDocument,
) -> Result<CeremonyDesignDocument, DomainError> {
    if document.participants().len() < 2 {
        return Err(invalid(
            "pattern `roundtable_fixed_order` requires at least two participants",
        ));
    }
    let stages = document
        .participants()
        .iter()
        .enumerate()
        .map(|(index, participant)| {
            let ordinal = index + 1;
            let prior = if index == 0 {
                "Open the fixed-order roundtable."
            } else {
                "Read every prior contribution, add a distinct point, and avoid repetition."
            };
            Ok(CeremonyDesignStage::new(
                StepId::new(format!("roundtable_turn_{ordinal}"))?,
                participant.role_id().clone(),
                StepInstructions::new(format!(
                    "{prior} Contribute as role `{}` toward this objective: {}",
                    participant.role_id(),
                    document.objective()
                ))?,
                None,
                Some(PriorContext::from_visible(index > 0)),
                None,
                Rounds::new(0)?,
                None,
            ))
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    Ok(document.materialized_with_stages(stages))
}

fn invalid(reason: impl Into<String>) -> DomainError {
    DomainError::InvalidDocument {
        reason: reason.into(),
    }
}
