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

/// The exception list itself. Outside this package, as `tests.rs` already
/// reads the proto from outside it; both are `#[cfg(test)]`, so `cargo
/// package` never compiles them.
const PARITY_TSV: &str = include_str!("../../../../docs/architecture/parity.tsv");

/// The canonical contract, not the copy vendored into `made-mcp-proto` that
/// `tests.rs` reads. The contract gate keeps the two byte-identical, so the
/// choice only decides which file a reader is sent to; this one is the
/// source.
const MADE_PROTO: &str = include_str!("../../../made-proto/proto/underpass/made/v1/made.proto");

/// The facade. Parsed rather than mirrored in a const list, so that adding a
/// method is enough to break this test — a mirror has to be updated to break.
const EMBEDDED_MADE_SOURCE: &str = include_str!("../../../made-embedded/src/embedded_made.rs");

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
const FACADE_VARIANTS: [(&str, &str); 6] = [
    ("mount_definitions", "mount_definition"),
    ("mount_yaml", "mount_definition"),
    ("definition", "list_ceremony_definitions"),
    ("published_definition", "list_ceremony_definitions"),
    ("published_definitions", "list_ceremony_definitions"),
    ("definition_for", "list_ceremony_definitions"),
];

/// Constructors. They open or configure the facade; they are not capabilities
/// on any surface.
const FACADE_CONSTRUCTORS: [&str; 2] = ["open", "builder"];

/// The marker a cell uses for "this surface does not have it".
const GAP: &str = "-";

/// One line of `parity.tsv`.
struct ParityRow {
    capability: String,
    proto: String,
    mcp_grpc: String,
    mcp_embedded: String,
    facade: String,
    api: String,
    reason: String,
}

impl ParityRow {
    /// The four surfaces parity is defined over. `api` is deliberately not
    /// one of them: ADR-004 makes it a subset, and a subset is allowed to be
    /// smaller without an excuse.
    fn surfaces(&self) -> [&str; 4] {
        [
            &self.proto,
            &self.mcp_grpc,
            &self.mcp_embedded,
            &self.facade,
        ]
    }

    fn has_gap(&self) -> bool {
        self.surfaces().iter().any(|cell| *cell == GAP)
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
        if FACADE_CONSTRUCTORS.contains(&method.as_str()) {
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

fn parity_rows() -> Vec<ParityRow> {
    let mut rows = Vec::new();
    for (index, line) in PARITY_TSV.lines().enumerate() {
        let number = index + 1;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        assert_eq!(
            cells.len(),
            7,
            "parity.tsv line {number} has {} tab-separated columns, expected 7 \
             (a row with no reason still ends in a tab): {line:?}",
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

fn embedded_made_public_methods() -> BTreeSet<String> {
    declared_fn_names(EMBEDDED_MADE_SOURCE, &["pub async fn ", "pub fn "])
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
