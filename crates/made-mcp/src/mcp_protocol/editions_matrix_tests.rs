//! The support matrix's Editions table, against `parity.tsv` (ADR-014,
//! plan §3.6 F6).
//!
//! `docs/operations/support-matrix.md` is the page where a support claim
//! names its source of truth and its enforcement. Its Editions section
//! claims, per capability group and per surface, `supported` or `not
//! supported` with a reason — and both halves of that claim are already
//! data: the surfaces are `docs/architecture/parity.tsv`, which
//! [`super::parity_tests`] keeps equal to the code in both directions,
//! and the grouping is [`CAPABILITY_GROUPS`], the list
//! `made_discover_capabilities` answers with.
//!
//! So the table is derived here from those two and compared with the
//! markdown cell by cell, in both directions: a cell that promises what
//! the TSV denies fails, a group the table forgets fails, and a row the
//! code no longer groups fails too. A support claim that only a reviewer
//! checked is how `docs/editions.md` came to say "not claimed" about
//! capabilities that had already landed.
//!
//! No server, no store, milliseconds — the reason F1's gate lives in
//! this crate is the reason this one does: the development loop has to
//! run it.

use std::collections::{BTreeMap, BTreeSet};

use super::parity_tests::{parity_rows, ParityRow, GAP};
use crate::guidance::capability_group::CAPABILITY_GROUPS;

/// The page carrying the claim.
const SUPPORT_MATRIX: &str = include_str!("../../../../docs/operations/support-matrix.md");

/// The markers that delimit the checked table. Everything between them
/// is this test's business; the prose around them is the page's.
const SECTION_BEGIN: &str = "<!-- editions:begin -->";
const SECTION_END: &str = "<!-- editions:end -->";

/// The columns, in order. The parser reads cells by position, so the
/// header is asserted rather than assumed: a renamed or reordered column
/// fails here instead of silently comparing the wrong surface.
const TABLE_HEADER: [&str; 6] = [
    "Capability group",
    "proto",
    "MCP on gRPC",
    "MCP embedded",
    "`EmbeddedMade` facade",
    "Proved by",
];

/// The four surfaces, in the order [`ParityRow::surfaces`] returns them,
/// which is the order the table's columns are in.
const PROTO: usize = 0;
const MCP_GRPC: usize = 1;
const MCP_EMBEDDED: usize = 2;
const FACADE: usize = 3;

/// What a cell says when the surface serves every capability of the row.
const SUPPORTED: &str = "supported";

/// What it says when the surface serves none of them. The reason follows,
/// verbatim from `parity.tsv` — an em dash rather than parentheses
/// because the reasons have parentheses of their own.
const NOT_SUPPORTED: &str = "not supported — ";

/// The gates, spelled the way the table's last column spells them. The
/// page names the full path of each one above the table; a cell names it
/// short enough to read.
const SET_EQUALITY_GATE: &str = "F1 set equality (`protocol/parity_tests.rs`)";
const BIJECTION_GATE: &str = "catalog ⇔ proto (`protocol/tests.rs`)";
const SESSION_GATE: &str = "F4 session parity (`mcp_parity_session.rs`)";

/// The two tools the MCP server answers itself have no RPC to be
/// bijective with, so what proves their row is the test that pins them as
/// the only catalog entries in that position.
const SERVER_TOOL_GATE: &str = "catalog ⇔ proto (`protocol/tests.rs`), \
     which pins these as the only catalog entries with no RPC";

/// Why the server's own tools are not on the other two surfaces. These
/// two reasons are the only ones not read from `parity.tsv`, because a
/// server tool is not a MADE capability and names no row there (the same
/// decision F1 took when it filtered `SERVER_TOOL_NAMES` out of the
/// embedded catalog).
const SERVER_TOOL_NO_RPC: &str = "server tool: the MCP server answers it itself on every \
     backend, so it has no RPC and no parity.tsv row";
const SERVER_TOOL_NO_FACADE: &str = "server tool: it describes the running server rather than \
     the engine, so the facade has no method for it";

/// One row of the table: what it claims per surface, and what proves it.
#[derive(Debug, PartialEq, Eq)]
struct Claim {
    /// The capability group, or `group / capability` when the group is
    /// split because its capabilities disagree about a surface.
    key: String,
    cells: [String; 4],
    gate: String,
}

#[test]
fn the_editions_table_claims_exactly_what_parity_tsv_says() {
    let expected = expected_claims();
    let found = table_claims();

    let expected_keys: BTreeSet<&str> = expected.iter().map(|claim| claim.key.as_str()).collect();
    let found_keys: BTreeSet<&str> = found.iter().map(|claim| claim.key.as_str()).collect();
    let missing: Vec<&str> = expected_keys.difference(&found_keys).copied().collect();
    let unknown: Vec<&str> = found_keys.difference(&expected_keys).copied().collect();
    assert!(
        missing.is_empty() && unknown.is_empty(),
        "the Editions section of docs/operations/support-matrix.md disagrees with \
         docs/architecture/parity.tsv about which rows exist: grouped by the code and \
         claimed by no row: {missing:?}; claimed by a row and grouped by no code: {unknown:?}. \
         A capability changes surface only with its parity.tsv row and this table in the \
         same PR (ADR-014)."
    );

    for claim in &expected {
        let row = found
            .iter()
            .find(|row| row.key == claim.key)
            .expect("every expected key was just found in the table");
        for surface in [PROTO, MCP_GRPC, MCP_EMBEDDED, FACADE] {
            assert_eq!(
                row.cells[surface],
                claim.cells[surface],
                "the Editions table claims `{}` for `{}` on the {} surface, and \
                 docs/architecture/parity.tsv says `{}`. The TSV is the source of truth; \
                 fix the table, or fix the row and the code together (ADR-014).",
                row.cells[surface],
                claim.key,
                TABLE_HEADER[surface + 1],
                claim.cells[surface],
            );
        }
        assert_eq!(
            row.gate, claim.gate,
            "the Editions table says `{}` proves the row `{}`; what proves it, given the \
             surfaces parity.tsv fills, is `{}`. A support claim names the gate that holds it up.",
            row.gate, claim.key, claim.gate,
        );
    }
}

#[test]
fn the_editions_table_lists_the_groups_in_the_order_the_code_declares_them() {
    let expected_keys: Vec<String> = expected_claims()
        .into_iter()
        .map(|claim| claim.key)
        .collect();
    let found_keys: Vec<String> = table_claims().into_iter().map(|claim| claim.key).collect();
    assert_eq!(
        expected_keys, found_keys,
        "the Editions table lists its rows in a different order than \
         crates/made-mcp/src/guidance/capability_group.rs declares the groups. \
         The table is a projection of that list, and a reader comparing the two \
         should not have to search."
    );
}

#[test]
fn every_capability_with_an_mcp_tool_belongs_to_exactly_one_group() {
    let mut group_of: BTreeMap<&str, &str> = BTreeMap::new();
    for group in CAPABILITY_GROUPS {
        for tool in group.tools {
            if let Some(previous) = group_of.insert(tool, group.id) {
                panic!(
                    "`{tool}` is in the capability groups `{previous}` and `{}`; \
                     two rows of the Editions table would claim the same capability",
                    group.id
                );
            }
        }
    }

    let grouped: BTreeSet<String> = group_of
        .keys()
        .filter(|tool| !super::is_server_tool(tool))
        .map(|tool| (*tool).to_owned())
        .collect();
    let in_file: BTreeSet<String> = parity_rows()
        .iter()
        .filter(|row| row.mcp_grpc != GAP)
        .map(|row| row.mcp_grpc.clone())
        .collect();

    let ungrouped: Vec<&str> = in_file.difference(&grouped).map(String::as_str).collect();
    let unknown: Vec<&str> = grouped.difference(&in_file).map(String::as_str).collect();
    assert!(
        ungrouped.is_empty() && unknown.is_empty(),
        "the capability groups and docs/architecture/parity.tsv disagree about which tools \
         exist: named by a row and in no group: {ungrouped:?}; in a group and named by no \
         row: {unknown:?}. A capability in no group has no row in the Editions table, which \
         is how a support claim goes missing."
    );
}

#[test]
fn a_capability_no_group_can_offer_is_still_named_in_the_section() {
    let section = editions_section();
    for row in parity_rows().iter().filter(|row| row.mcp_grpc == GAP) {
        assert!(
            section.contains(&row.capability),
            "docs/architecture/parity.tsv has the capability `{}` with no MCP tool, so no \
             capability group can offer it and the table has no row for it. The Editions \
             section must still name it, or the page reads as if it did not exist.",
            row.capability
        );
    }
}

// ---------------------------------------------------------------------------
// What the code and the TSV say
// ---------------------------------------------------------------------------

/// The table as `parity.tsv` and `CAPABILITY_GROUPS` together define it.
fn expected_claims() -> Vec<Claim> {
    let rows = parity_rows();
    let by_tool: BTreeMap<&str, &ParityRow> = rows
        .iter()
        .filter(|row| row.mcp_grpc != GAP)
        .map(|row| (row.mcp_grpc.as_str(), row))
        .collect();

    let mut claims = Vec::new();
    for group in CAPABILITY_GROUPS {
        let server_tools = group
            .tools
            .iter()
            .filter(|tool| super::is_server_tool(tool))
            .count();
        if server_tools == group.tools.len() {
            claims.push(server_claim(group.id));
            continue;
        }
        assert_eq!(
            server_tools, 0,
            "the capability group `{}` mixes the server's own tools with MADE capabilities; \
             one row cannot claim both, because a server tool has no RPC and no facade method \
             while a capability has both",
            group.id
        );

        let capabilities: Vec<&ParityRow> = group
            .tools
            .iter()
            .map(|tool| {
                *by_tool.get(tool).unwrap_or_else(|| {
                    panic!(
                        "the capability group `{}` offers `{tool}`, which no \
                         docs/architecture/parity.tsv row names in its mcp_grpc column",
                        group.id
                    )
                })
            })
            .collect();

        let served = |row: &ParityRow| row.surfaces().map(|cell| cell != GAP);
        if capabilities
            .iter()
            .all(|row| served(row) == served(capabilities[0]))
        {
            claims.push(claim(group.id.to_owned(), &capabilities));
        } else {
            // The capabilities of this group disagree about a surface, so
            // one row would have to say "partly" — which is not a support
            // claim. The group is written out per capability instead.
            for row in &capabilities {
                claims.push(claim(
                    format!("{} / {}", group.id, row.capability),
                    std::slice::from_ref(row),
                ));
            }
        }
    }
    claims
}

fn claim(key: String, capabilities: &[&ParityRow]) -> Claim {
    Claim {
        key,
        cells: [PROTO, MCP_GRPC, MCP_EMBEDDED, FACADE].map(|surface| cell(capabilities, surface)),
        gate: gate(capabilities),
    }
}

/// `supported` when the surface serves every capability of the row, and
/// `not supported` with the reasons when it serves none. There is no
/// third answer: a group that would need one is split before it gets
/// here.
fn cell(capabilities: &[&ParityRow], surface: usize) -> String {
    let served: Vec<bool> = capabilities
        .iter()
        .map(|row| row.surfaces()[surface] != GAP)
        .collect();
    if served.iter().all(|filled| *filled) {
        return SUPPORTED.to_owned();
    }
    assert!(
        !served.iter().any(|filled| *filled),
        "a row of the Editions table would be partly supported on one surface; \
         expected_claims splits such a group per capability before building a cell"
    );

    // First-appearance order, which is the order of the rows in
    // `parity.tsv`: a group whose gap has one reason reads as one
    // sentence, and a group with two (council configuration and the
    // contract registry) keeps both, in file order.
    let mut reasons: Vec<&str> = Vec::new();
    for row in capabilities {
        let reason = row.reason.trim();
        assert!(
            !reason.is_empty(),
            "docs/architecture/parity.tsv leaves the capability `{}` unserved on a surface \
             with no reason; the Editions table quotes that reason",
            row.capability
        );
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
    }
    format!("{NOT_SUPPORTED}{}", reasons.join("; "))
}

/// What proves the row, from the surfaces it fills. Set equality always;
/// the bijection once a capability is both an RPC and a gRPC-backend
/// tool; the session comparison once both MCP backends serve it, because
/// that test drives exactly the shared tools.
fn gate(capabilities: &[&ParityRow]) -> String {
    let filled = |surface: usize| {
        capabilities
            .iter()
            .all(|row| row.surfaces()[surface] != GAP)
    };
    let mut gates = vec![SET_EQUALITY_GATE];
    if filled(PROTO) && filled(MCP_GRPC) {
        gates.push(BIJECTION_GATE);
    }
    if filled(MCP_GRPC) && filled(MCP_EMBEDDED) {
        gates.push(SESSION_GATE);
    }
    gates.join("; ")
}

/// The group whose tools the MCP server answers itself. Both MCP
/// backends serve them — `available_tool_catalog` admits a server tool
/// whatever the backend supports — and neither the contract nor the
/// facade has anything for them.
fn server_claim(group_id: &str) -> Claim {
    Claim {
        key: group_id.to_owned(),
        cells: [
            format!("{NOT_SUPPORTED}{SERVER_TOOL_NO_RPC}"),
            SUPPORTED.to_owned(),
            SUPPORTED.to_owned(),
            format!("{NOT_SUPPORTED}{SERVER_TOOL_NO_FACADE}"),
        ],
        gate: SERVER_TOOL_GATE.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// What the page says
// ---------------------------------------------------------------------------

/// The text between the two markers. The markers exist so that the check
/// reads one table and not every table the page happens to carry.
fn editions_section() -> &'static str {
    let rest = SUPPORT_MATRIX.split_once(SECTION_BEGIN).unwrap_or_else(|| {
        panic!(
            "docs/operations/support-matrix.md has no `{SECTION_BEGIN}` marker; \
             the Editions table is a checked claim and the check has to find it"
        )
    });
    rest.1
        .split_once(SECTION_END)
        .unwrap_or_else(|| {
            panic!("docs/operations/support-matrix.md opens the Editions table with `{SECTION_BEGIN}` and never closes it with `{SECTION_END}`")
        })
        .0
}

fn table_claims() -> Vec<Claim> {
    let mut lines = editions_section()
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('|'));

    let header = cells(lines.next().unwrap_or_else(|| {
        panic!("the Editions section of docs/operations/support-matrix.md has no table in it")
    }));
    assert_eq!(
        header,
        TABLE_HEADER.map(str::to_owned).to_vec(),
        "the Editions table's columns are not the ones this check reads by position"
    );
    let separator = cells(
        lines
            .next()
            .expect("a markdown table has a separator row under its header"),
    );
    assert!(
        separator
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|c| c == '-' || c == ':')),
        "the row under the Editions table's header is not a separator: {separator:?}"
    );

    lines
        .map(|line| {
            let cells = cells(line);
            assert_eq!(
                cells.len(),
                TABLE_HEADER.len(),
                "a row of the Editions table has {} cells, expected {}: {line:?}",
                cells.len(),
                TABLE_HEADER.len()
            );
            Claim {
                key: cells[0].replace('`', ""),
                cells: [
                    cells[1].clone(),
                    cells[2].clone(),
                    cells[3].clone(),
                    cells[4].clone(),
                ],
                gate: cells[5].clone(),
            }
        })
        .collect()
}

fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}
