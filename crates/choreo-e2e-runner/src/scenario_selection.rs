use std::collections::BTreeSet;

use anyhow::{bail, Result};

pub(crate) const SCENARIO_SELECTION_ENV: &str = "CHOREO_E2E_SCENARIOS";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum E2eScenario {
    SeededCouncil,
    Deliberate,
    DeleteMissingCouncil,
    CausalMetadata,
    RuntimeExecutor,
    StrictSchemaRejection,
    ExternalContextBundle,
    OpenAiStructuredOutput,
    VllmStructuredOutput,
    VllmRealMultiAgent,
}

impl E2eScenario {
    // Repo-owned compose scenarios only. Real provider scenarios stay
    // opt-in because they require external infrastructure.
    const ALL: [Self; 9] = [
        Self::SeededCouncil,
        Self::Deliberate,
        Self::DeleteMissingCouncil,
        Self::CausalMetadata,
        Self::RuntimeExecutor,
        Self::StrictSchemaRejection,
        Self::ExternalContextBundle,
        Self::OpenAiStructuredOutput,
        Self::VllmStructuredOutput,
    ];

    const VLLM_REAL: [Self; 1] = [Self::VllmRealMultiAgent];

    const CLUSTER_CONNECTIVITY: [Self; 4] = [
        Self::SeededCouncil,
        Self::Deliberate,
        Self::DeleteMissingCouncil,
        Self::CausalMetadata,
    ];

    const RUNTIME_STUB: [Self; 1] = [Self::RuntimeExecutor];

    const STRUCTURED_OUTPUT: [Self; 4] = [
        Self::StrictSchemaRejection,
        Self::ExternalContextBundle,
        Self::OpenAiStructuredOutput,
        Self::VllmStructuredOutput,
    ];

    const fn number(self) -> u8 {
        match self {
            Self::SeededCouncil => 1,
            Self::Deliberate => 2,
            Self::DeleteMissingCouncil => 3,
            Self::CausalMetadata => 4,
            Self::RuntimeExecutor => 5,
            Self::StrictSchemaRejection => 6,
            Self::ExternalContextBundle => 7,
            Self::OpenAiStructuredOutput => 8,
            Self::VllmStructuredOutput => 9,
            Self::VllmRealMultiAgent => 10,
        }
    }
}

pub(crate) fn parse_scenario_selection(raw: Option<&str>) -> Result<BTreeSet<E2eScenario>> {
    let raw = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("compose");
    let mut selected = BTreeSet::new();
    for token in raw
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|token| !token.is_empty())
    {
        add_scenario_token(token, &mut selected)?;
    }
    if selected.is_empty() {
        bail!("scenario selection is empty");
    }
    Ok(selected)
}

fn add_scenario_token(token: &str, selected: &mut BTreeSet<E2eScenario>) -> Result<()> {
    let normalized = token.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "compose" | "all" => {
            selected.extend(E2eScenario::ALL);
            return Ok(());
        }
        "cluster-connectivity" => {
            selected.extend(E2eScenario::CLUSTER_CONNECTIVITY);
            return Ok(());
        }
        "runtime-stub" => {
            selected.extend(E2eScenario::RUNTIME_STUB);
            return Ok(());
        }
        "structured-output" => {
            selected.extend(E2eScenario::STRUCTURED_OUTPUT);
            return Ok(());
        }
        "vllm-real" | "vllm-real-multi-agent" | "council-vllm" => {
            selected.extend(E2eScenario::VLLM_REAL);
            return Ok(());
        }
        _ => {}
    }

    if let Some((start, end)) = normalized.split_once('-') {
        if let (Ok(start), Ok(end)) = (start.parse::<u8>(), end.parse::<u8>()) {
            if start > end {
                bail!("invalid scenario range `{token}`: start is greater than end");
            }
            for number in start..=end {
                selected.insert(scenario_from_number(number)?);
            }
            return Ok(());
        }
    }

    let numeric = normalized
        .strip_prefix("scenario-")
        .or_else(|| normalized.strip_prefix('s'))
        .unwrap_or(&normalized);
    if let Ok(number) = numeric.parse::<u8>() {
        selected.insert(scenario_from_number(number)?);
        return Ok(());
    }

    bail!(
        "unknown scenario selector `{token}`; expected compose, cluster-connectivity, runtime-stub, structured-output, vllm-real-multi-agent, 1-10, or a range like 1-4"
    )
}

fn scenario_from_number(number: u8) -> Result<E2eScenario> {
    match number {
        1 => Ok(E2eScenario::SeededCouncil),
        2 => Ok(E2eScenario::Deliberate),
        3 => Ok(E2eScenario::DeleteMissingCouncil),
        4 => Ok(E2eScenario::CausalMetadata),
        5 => Ok(E2eScenario::RuntimeExecutor),
        6 => Ok(E2eScenario::StrictSchemaRejection),
        7 => Ok(E2eScenario::ExternalContextBundle),
        8 => Ok(E2eScenario::OpenAiStructuredOutput),
        9 => Ok(E2eScenario::VllmStructuredOutput),
        10 => Ok(E2eScenario::VllmRealMultiAgent),
        _ => bail!("scenario number {number} is out of range; expected 1-10"),
    }
}

pub(crate) fn scenario_selection_summary(selected: &BTreeSet<E2eScenario>) -> String {
    selected
        .iter()
        .map(|scenario| scenario.number().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(raw: Option<&str>) -> Vec<u8> {
        parse_scenario_selection(raw)
            .unwrap()
            .iter()
            .map(|scenario| scenario.number())
            .collect()
    }

    #[test]
    fn default_selection_is_full_compose_suite() {
        assert_eq!(numbers(None), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(numbers(Some("")), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(
            scenario_selection_summary(&parse_scenario_selection(None).unwrap()),
            "1,2,3,4,5,6,7,8,9"
        );
    }

    #[test]
    fn named_groups_expand_to_expected_scenarios() {
        assert_eq!(numbers(Some("compose")), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(numbers(Some("cluster-connectivity")), vec![1, 2, 3, 4]);
        assert_eq!(numbers(Some("runtime-stub")), vec![5]);
        assert_eq!(numbers(Some("structured-output")), vec![6, 7, 8, 9]);
        assert_eq!(numbers(Some("vllm-real-multi-agent")), vec![10]);
        assert_eq!(numbers(Some("council-vllm")), vec![10]);
    }

    #[test]
    fn selection_accepts_numbers_ranges_and_mixed_tokens() {
        assert_eq!(numbers(Some("1,3,scenario-5,s8")), vec![1, 3, 5, 8]);
        assert_eq!(
            numbers(Some("1-4 structured-output")),
            vec![1, 2, 3, 4, 6, 7, 8, 9]
        );
        assert_eq!(numbers(Some("s10")), vec![10]);
    }

    #[test]
    fn invalid_selection_is_rejected() {
        assert!(parse_scenario_selection(Some("11")).is_err());
        assert!(parse_scenario_selection(Some("4-1")).is_err());
        assert!(parse_scenario_selection(Some("unknown")).is_err());
    }
}
