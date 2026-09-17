//! The parity gate (ADR-014, plan §3.6 F1).
//!
//! `docs/architecture/parity.tsv` is the checked-in exception list — the
//! precedent is `docs/architecture/conformance.tsv`: the list of gaps is
//! data, it may shrink, and it must never grow silently. This module keeps
//! that file equal to the four surfaces **in both directions**: a capability
//! named by a row that the code does not have fails, and a capability the
//! code has that no row names fails too.
//!
//! One assertion per column, no server, no store, milliseconds.

use std::collections::{BTreeMap, BTreeSet};

mod facade_scan;

use facade_scan::embedded_made_public_methods;

/// The exception list itself. Outside this package, as `tests.rs` already
/// reads the proto from outside it; both are `#[cfg(test)]`, so `cargo
/// package` never compiles them.
const PARITY_TSV: &str = include_str!("../../../../docs/architecture/parity.tsv");

/// The canonical contract, not the copy vendored into `made-mcp-proto` that
/// `tests.rs` reads. The contract gate keeps the two byte-identical, so the
/// choice only decides which file a reader is sent to; this one is the
/// source.
const MADE_PROTO: &str = include_str!("../../../made-proto/proto/underpass/made/v1/made.proto");

/// The versioned read-mostly subset (ADR-004).
const CEREMONY_ENGINE_API_SOURCE: &str =
    include_str!("../../../made-api/src/ceremony_engine_api.rs");

/// The one `CeremonyEngineApi` method that is not a capability: it reports
/// which capabilities the implementation has, so it can name no row.
const API_NON_CAPABILITY_METHOD: &str = "capabilities";

/// Facade methods that are a spelling of a capability another row already
/// carries, mapped to that capability. Every one of these is explicit: an
/// unmapped public method of `EmbeddedMade` fails the test rather than being
/// waved through.
const FACADE_VARIANTS: [(&str, &str); 7] = [
    ("mount_definitions", "mount_definition"),
    ("mount_yaml", "mount_definition"),
    ("definition", "list_ceremony_definitions"),
    ("published_definition", "list_ceremony_definitions"),
    ("published_definitions", "list_ceremony_definitions"),
    ("definition_for", "list_ceremony_definitions"),
    // One capability, two questions: the whole stream for a caller
    // that verifies the chain it was given, one page for a caller
    // following it.
    ("audit_records_from", "read_ceremony_events"),
];

/// Facade methods that are not capabilities, each with the reason it is not.
///
/// Host lifecycle: they construct, initialize or describe the engine rather
/// than expose an operator command. Listed the way
/// `FACADE_VARIANTS` is — an unlisted public method fails the test rather
/// than being waved through — and with a reason each, because "not a
/// capability" is a judgement and a judgement with no reason is a hole.
const FACADE_NON_CAPABILITIES: [(&str, &str); 5] = [
    (
        "open",
        "opens the durable store the engine runs over; making an engine is not          something an engine does",
    ),
    (
        "open_with_event_transport",
        "opens and composes the durable engine with an outbound transport; constructing an engine is not an engine capability",
    ),
    (
        "recover_event_publication",
        "completes async host initialization before accepting work, matching the cluster startup drain; lifecycle recovery is not an operator command",
    ),
    (
        "builder",
        "configures the ports an engine is built from; the same reason as `open`",
    ),
    (
        "version",
        "reports which build of the crate this is. `made_get_status` carries the          version as a field of an answer, which is the capability; a bare          accessor for it is not a second one",
    ),
];

/// The marker a cell uses for "this surface does not have it".
pub(super) const GAP: &str = "-";

/// One line of `parity.tsv`. Read here and by the Editions-table check
/// next door, which groups these rows the way the MCP server groups them.
pub(super) struct ParityRow {
    pub(super) capability: String,
    proto: String,
    pub(super) mcp_grpc: String,
    mcp_embedded: String,
    facade: String,
    api: String,
    pub(super) reason: String,
}

impl ParityRow {
    /// The four surfaces parity is defined over. `api` is deliberately not
    /// one of them: ADR-004 makes it a subset, and a subset is allowed to be
    /// smaller without an excuse.
    pub(super) fn surfaces(&self) -> [&str; 4] {
        [
            &self.proto,
            &self.mcp_grpc,
            &self.mcp_embedded,
            &self.facade,
        ]
    }

    fn has_gap(&self) -> bool {
        self.surfaces().contains(&GAP)
    }
}

#[test]
fn parity_file_parses_and_names_each_capability_once() {
    let rows = parity_rows();
    assert!(
        rows.len() > 1,
        "docs/architecture/parity.tsv has no capability rows"
    );

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for row in &rows {
        assert!(
            seen.insert(&row.capability),
            "parity.tsv names the capability `{}` on more than one row",
            row.capability
        );
    }

    for (column, cells) in [
        ("proto", column(&rows, |row| &row.proto)),
        ("mcp_grpc", column(&rows, |row| &row.mcp_grpc)),
        ("mcp_embedded", column(&rows, |row| &row.mcp_embedded)),
        ("facade", column(&rows, |row| &row.facade)),
    ] {
        let mut named: BTreeSet<&str> = BTreeSet::new();
        for (capability, cell) in cells.iter().filter(|(_, cell)| *cell != GAP) {
            assert!(
                named.insert(cell),
                "parity.tsv column `{column}` names `{cell}` twice; \
                 the second time on row `{capability}`"
            );
        }
    }
}

#[test]
fn every_gap_carries_a_reason_and_a_complete_row_carries_none() {
    for row in parity_rows() {
        if row.has_gap() {
            assert!(
                !row.reason.trim().is_empty(),
                "parity.tsv row `{}` leaves a surface empty ({:?}) with no reason; \
                 a gap without a reason is an unexplained divergence (ADR-014)",
                row.capability,
                row.surfaces()
            );
        } else {
            assert!(
                row.reason.trim().is_empty(),
                "parity.tsv row `{}` is served by all four surfaces but still carries \
                 the reason {:?}; delete the reason, the gap it described is closed",
                row.capability,
                row.reason
            );
        }
    }
}

#[test]
fn proto_column_matches_the_ceremony_contract() {
    assert_columns_agree(
        "proto",
        "the underpass.made.v1 RPC list",
        &filled(column(&parity_rows(), |row| &row.proto)),
        &proto_rpc_names(),
    );
}

#[test]
fn mcp_grpc_column_matches_the_grpc_backend_catalog() {
    let advertised = super::tool_names::GRPC_TOOL_NAMES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();

    assert_columns_agree(
        "mcp_grpc",
        "GRPC_TOOL_NAMES",
        &filled(column(&parity_rows(), |row| &row.mcp_grpc)),
        &advertised,
    );
}

#[cfg(feature = "embedded")]
#[test]
fn mcp_embedded_column_matches_the_embedded_backend_catalog() {
    assert_columns_agree(
        "mcp_embedded",
        "the catalog EmbeddedMadeMcpBackend::supports_tool admits",
        &filled(column(&parity_rows(), |row| &row.mcp_embedded)),
        &embedded_catalog_tool_names(),
    );
}

#[test]
fn facade_column_matches_the_public_methods_of_embedded_made() {
    let rows = parity_rows();
    let by_capability: BTreeMap<&str, &ParityRow> = rows
        .iter()
        .map(|row| (row.capability.as_str(), row))
        .collect();

    let mut methods = BTreeSet::new();
    for method in embedded_made_public_methods() {
        if FACADE_NON_CAPABILITIES
            .iter()
            .any(|(excused, _)| *excused == method)
        {
            continue;
        }
        if let Some((_, capability)) = FACADE_VARIANTS
            .iter()
            .find(|(variant, _)| *variant == method)
        {
            let row = by_capability.get(capability).unwrap_or_else(|| {
                panic!(
                    "EmbeddedMade::{method} is mapped to the capability `{capability}`, \
                     which parity.tsv does not have a row for"
                )
            });
            assert_ne!(
                row.facade, GAP,
                "EmbeddedMade::{method} is mapped to the capability `{capability}`, \
                 whose parity.tsv row says the facade does not serve it"
            );
            continue;
        }
        methods.insert(method);
    }

    assert_columns_agree(
        "facade",
        "the public methods of EmbeddedMade",
        &filled(column(&rows, |row| &row.facade)),
        &methods,
    );
}

/// Every excuse names something that is really there.
///
/// An excuse for a method nobody declares excuses nothing and hides the next
/// one — the same failure mode as a normalised path for a tool nobody drives.
/// It is also what keeps the scan honest in the direction a scan cannot fail
/// by itself: a scan that finds too little agrees with every list there is,
/// and `pub const fn version` was invisible to the old one for exactly that
/// reason.
#[test]
fn every_excused_facade_method_is_one_the_scan_finds() {
    let found = embedded_made_public_methods();
    for (method, reason) in FACADE_NON_CAPABILITIES {
        assert!(
            found.contains(method),
            "`EmbeddedMade::{method}` is excused from the facade column, and the scan \
             does not find it: either it is gone and the entry should go with it, or \
             the scan has stopped seeing it"
        );
        assert!(
            !reason.trim().is_empty(),
            "`EmbeddedMade::{method}` is excused with no reason"
        );
    }
    for (method, capability) in FACADE_VARIANTS {
        assert!(
            found.contains(method),
            "`EmbeddedMade::{method}` is mapped to the capability `{capability}`, and \
             the scan does not find it"
        );
    }
    // And the one the old scan could not see at all, named on purpose: the
    // whole point of reading every `pub` form is that this is found.
    assert!(
        found.contains("version"),
        "`EmbeddedMade::version` is a `pub const fn`, and the scan must read that form"
    );
}

/// The cells of a row, derived from its key rather than compared as a set.
///
/// Set equality says the file and the code name the same things. It cannot
/// say they name them on the same rows: swap `run_ceremony`'s and
/// `start_ceremony`'s proto cells and both sets are unchanged, so both
/// comparisons pass while the file says something false about every surface.
///
/// A capability key is `run_ceremony`; its RPC is `RunCeremony` and its tool
/// is `made_run_ceremony`, through the one transform `protocol/tests.rs`
/// owns. The facade is not mechanical everywhere, so it is derived where it
/// is — drop the word `ceremony` — and listed where it is not.
#[test]
fn each_row_names_the_cells_its_capability_implies() {
    for row in parity_rows() {
        let capability = row.capability.as_str();
        if row.proto != GAP {
            assert_eq!(
                row.proto,
                super::tests::capability_to_rpc_name(capability),
                "parity.tsv row `{capability}` names the RPC `{}`",
                row.proto
            );
        }
        let tool = format!("made_{capability}");
        for (column, cell) in [
            ("mcp_grpc", &row.mcp_grpc),
            ("mcp_embedded", &row.mcp_embedded),
        ] {
            if cell != GAP {
                assert_eq!(
                    cell, &tool,
                    "parity.tsv row `{capability}` names `{cell}` in column `{column}`"
                );
            }
        }
        if row.facade != GAP {
            assert_eq!(
                row.facade,
                expected_facade_method(capability),
                "parity.tsv row `{capability}` names the facade method `{}`",
                row.facade
            );
        }
    }
}

/// `get_ceremony_instance` -> `instance`: the facade drops the word
/// `ceremony`, because every one of its methods is about a ceremony and
/// saying so in each name says nothing.
///
/// The exceptions are the methods whose name is a noun where the capability
/// is a verb phrase, plus the two reads named after what they answer with
/// rather than after the call. Listed the way `FACADE_VARIANTS` is, with the
/// reason each.
const FACADE_NAME_EXCEPTIONS: [(&str, &str, &str); 9] = [
    ("get_ceremony_instance", "instance", "named after what it answers with, not after the asking"),
    ("list_ceremony_instances", "instances", "the plural of the row above, for the same reason"),
    ("list_ceremony_definitions", "definitions", "the same shape again, for definitions"),
    ("get_ceremony_transcript", "transcript", "named after what it answers with"),
    ("get_status", "status", "named after what it answers with"),
    ("get_metrics", "metrics", "named after what it answers with"),
    (
        "generate_ceremony_report",
        "report",
        "the facade hands back the report; generating it is what a tool call is for",
    ),
    (
        "read_ceremony_events",
        "audit_records",
        "the facade answers with the audit records themselves; `audit_records_from` \
         is the paged half of the same capability and is mapped in FACADE_VARIANTS",
    ),
    (
        "claim_ceremony_step",
        "start_step",
        "the facade calls it starting because that is the use case it runs          (`StartCeremonyStepUseCase`); claiming is what the tool calls the same act",
    ),
];

fn expected_facade_method(capability: &str) -> String {
    if let Some((_, method, _)) = FACADE_NAME_EXCEPTIONS
        .iter()
        .find(|(named, _, _)| *named == capability)
    {
        return (*method).to_owned();
    }
    let parts: Vec<&str> = capability
        .split('_')
        .filter(|word| *word != "ceremony")
        .collect();
    parts.join("_")
}

#[test]
fn every_facade_naming_exception_is_one_a_row_still_needs() {
    let rows = parity_rows();
    for (capability, method, reason) in FACADE_NAME_EXCEPTIONS {
        let row = rows
            .iter()
            .find(|row| row.capability == capability)
            .unwrap_or_else(|| panic!("parity.tsv has no row for `{capability}`"));
        assert_eq!(
            row.facade, method,
            "the exception for `{capability}` is stale"
        );
        assert!(
            !reason.trim().is_empty(),
            "the exception for `{capability}` carries no reason"
        );
    }
}

#[test]
fn api_column_matches_the_ceremony_engine_api_trait() {
    assert_columns_agree(
        "api",
        "the CeremonyEngineApi trait methods",
        &filled(column(&parity_rows(), |row| &row.api)),
        &ceremony_engine_api_capabilities(),
    );
}

/// Both directions, with the offenders named. One-sided equality is how an
/// exception list rots: it would let a tool exist with no row, or a row
/// survive the capability it described.
fn assert_columns_agree(
    column: &str,
    code: &str,
    in_file: &BTreeSet<String>,
    in_code: &BTreeSet<String>,
) {
    let unlisted: Vec<&str> = in_code.difference(in_file).map(String::as_str).collect();
    let stale: Vec<&str> = in_file.difference(in_code).map(String::as_str).collect();

    assert!(
        unlisted.is_empty() && stale.is_empty(),
        "docs/architecture/parity.tsv column `{column}` disagrees with {code}: \
         in the code and named by no row: {unlisted:?}; \
         named by a row and absent from the code: {stale:?}. \
         Add, update or delete the row — a surface that changes without its row \
         is exactly the drift this gate exists to catch (ADR-014)."
    );
}

pub(super) fn parity_rows() -> Vec<ParityRow> {
    let mut rows = Vec::new();
    for (index, line) in PARITY_TSV.lines().enumerate() {
        let number = index + 1;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cells: Vec<&str> = line.split('\t').collect();
        if cells.len() == 6 {
            cells.push("");
        }
        assert_eq!(
            cells.len(),
            7,
            "parity.tsv line {number} has {} tab-separated columns, expected 6 or 7: {line:?}",
            cells.len()
        );
        if cells[0] == "capability" {
            continue;
        }
        for (position, cell) in cells.iter().enumerate() {
            assert!(
                !cell.is_empty() || position == 6,
                "parity.tsv line {number} leaves column {position} empty; \
                 an absent surface is spelled `{GAP}`, never blank"
            );
        }
        rows.push(ParityRow {
            capability: cells[0].to_owned(),
            proto: cells[1].to_owned(),
            mcp_grpc: cells[2].to_owned(),
            mcp_embedded: cells[3].to_owned(),
            facade: cells[4].to_owned(),
            api: cells[5].to_owned(),
            reason: cells[6].to_owned(),
        });
    }
    rows
}

/// One column as `(capability, cell)` pairs, so a failure can name the row.
fn column<'a>(
    rows: &'a [ParityRow],
    cell: impl Fn(&'a ParityRow) -> &'a str,
) -> Vec<(&'a str, &'a str)> {
    rows.iter()
        .map(|row| (row.capability.as_str(), cell(row)))
        .collect()
}

fn filled(cells: Vec<(&str, &str)>) -> BTreeSet<String> {
    cells
        .into_iter()
        .filter(|(_, cell)| *cell != GAP)
        .map(|(_, cell)| cell.to_owned())
        .collect()
}

fn proto_rpc_names() -> BTreeSet<String> {
    MADE_PROTO
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("rpc ")?;
            rest.split_once('(').map(|(rpc, _)| rpc.trim().to_owned())
        })
        .collect()
}

fn ceremony_engine_api_capabilities() -> BTreeSet<String> {
    let mut methods = declared_fn_names(CEREMONY_ENGINE_API_SOURCE, &["async fn ", "fn "]);
    assert!(
        methods.remove(API_NON_CAPABILITY_METHOD),
        "CeremonyEngineApi no longer declares `{API_NON_CAPABILITY_METHOD}`; \
         the one method that reports capabilities instead of being one"
    );
    methods
}

fn declared_fn_names(source: &str, prefixes: &[&str]) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = prefixes
                .iter()
                .find_map(|prefix| trimmed.strip_prefix(prefix))?;
            rest.split(['(', '<', ' ']).next().map(str::to_owned)
        })
        .collect()
}

#[cfg(feature = "embedded")]
fn embedded_catalog_tool_names() -> BTreeSet<String> {
    use crate::backend::MadeMcpToolBackend;
    use crate::embedded::EmbeddedMadeMcpBackend;

    use super::tool_names::SERVER_TOOL_NAMES;
    use super::tools_list_result;

    // The default composition is in-memory and side-effect free: this asks
    // the backend which tools it admits, nothing more.
    let backend = EmbeddedMadeMcpBackend::default();
    tools_list_result(|name| backend.supports_tool(name))["tools"]
        .as_array()
        .expect("tools/list answers with an array of tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        // Server-level introspection is not a MADE capability: it is served
        // by the MCP server itself on every backend, so it names no row.
        .filter(|name| !SERVER_TOOL_NAMES.contains(name))
        .map(str::to_owned)
        .collect()
}
