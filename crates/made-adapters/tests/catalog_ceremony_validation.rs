use std::path::PathBuf;

use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::CeremonyPatternPreset;

const CATALOG: &[&str] = &[
    "daily-standup.yaml",
    "editorial-planning-meeting.yaml",
    "editorial-planning-meeting-vllm.yaml",
    "engineering-planning.yaml",
    "speaker-talk-qa.yaml",
    "sprint-planning.yaml",
    "technical-debate.yaml",
];

#[test]
fn shipped_catalog_ceremonies_are_free_of_validation_warnings() {
    let mut offenders = Vec::new();

    for file in CATALOG {
        let definition = CeremonyDefinitionYaml::parse_path(catalog_path(file))
            .unwrap_or_else(|error| panic!("{file} must parse: {error}"));
        let report = definition.analyze();

        assert!(
            report.is_valid(),
            "{file} produced blocking findings: {:?}",
            report.errors().collect::<Vec<_>>()
        );

        for warning in report.warnings() {
            offenders.push(format!("{file}: {warning:?}"));
        }
    }

    assert!(
        offenders.is_empty(),
        "catalog ceremonies produced reachability warnings:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn shipped_pattern_fragments_parse_and_analyse_as_ceremonies() {
    for pattern in CeremonyPatternPreset::ALL {
        let draft = CeremonyDefinitionYaml::parse_draft_str(pattern.fragment_source())
            .unwrap_or_else(|error| panic!("{} must parse: {error}", pattern.id()));
        let report = draft.analyze();
        assert!(
            report.is_valid(),
            "{} produced blocking findings: {:?}",
            pattern.id(),
            report.errors().collect::<Vec<_>>()
        );
        assert_eq!(
            report.warnings().count(),
            0,
            "{} produced a validation warning",
            pattern.id()
        );
    }
}

fn catalog_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/e2e/ceremonies")
        .join(file)
}
