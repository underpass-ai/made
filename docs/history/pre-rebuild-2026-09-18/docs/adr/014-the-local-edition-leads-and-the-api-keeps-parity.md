# ADR-014: The local edition leads, and the API keeps parity

Status: Accepted (decided 2026-09-16); F1–F4 and F6 implemented on `main`
(#55–#64), F5 waits for B3 — the slices are §3.6 of
[`../orchestration-patterns-plan.md`](../orchestration-patterns-plan.md)

## Context

MADE is used local-first: the embedded edition over MCP is where ceremonies
are designed, run and reported day to day. Four surfaces expose the engine —
the gRPC contract, the MCP server on the gRPC backend, the MCP server on the
embedded backend, and the `EmbeddedMade` facade with `made-api` as its
versioned subset — and the code shows how they drift:

- Four tools exist only on the embedded backend and have no RPC: design a
  ceremony, claim a step, complete a step, generate a report. The audit
  journal and the transcript have no read RPC at all. Every one of these
  landed embedded-only and never caught up.
- Sixteen tools exist only on the gRPC backend — deliberation, council and
  agent configuration, contracts, status and metrics — and the editions
  document calls that "not claimed".
- Inside the shared verbs, three divergences are live: a field one presenter
  emits and the other omits, a listing shape that differs, and an error
  envelope that differs. The commit that added step repetition updated one
  presenter and not the other.
- The parity test compares the shape of one tool's result and checks tool
  availability against a hard-coded list of seventeen names. It cannot
  notice a tool added to one side.

## Decision

**Parity is a property of the ceremony capability set across the four
surfaces**: the same capabilities, identical request schemas, identical
result shapes for the same state, one error envelope. `made-api` stays the
read-mostly versioned subset ADR-004 defines; the facade behind it carries
every verb the RPCs carry.

**The local edition leads.** A capability is designed and proven in the
embedded edition first, and it reaches the gRPC contract, both MCP backends
and the facade **in the same pull request**. A capability that exists on one
surface only is a defect.

**Divergence is a named row, and the target is zero rows.** The exception
list is data: `docs/architecture/parity.tsv`, one row per capability, one
column per surface, a reason per gap. A test compares that file with the
real tool catalogs, the proto RPC list and the facade's method list, and
fails when they disagree in either direction. Adding a tool to one side
fails CI until the row says so; removing a gap requires deleting its row.

**The council surface is the first named exception.** Deliberation, council
and agent configuration and the contract registry stay cluster-only, each
with its reason in the file, until the first feature the local edition
would otherwise never see — parallel proposing inside a deliberation — lands;
then they come into the embedded edition behind the provider features.
Status and metrics come to the embedded edition now.

**The parity test drives both backends through the same session with the
same wiring** and compares shape and values for every shared tool, request
acceptance through the schema gate on both arms, and error envelopes. Its
allowlist is replaced by the exception file.

**The support matrix records editions.** One row per capability group with
the surfaces it is supported on and the gate that proves it; the editions
document points there instead of saying "surfaces differ by design".

## Consequences

- The delegated-host protocol — claim, perform, complete — works against a
  cluster as well as against a local store, because the RPCs exist.
- Every primitive the orchestration plan adds ships as proto, YAML, design
  tool, both presenters and facade at once; a slice that touches one is not
  done.
- A checked-in list of gaps is visible in review and in the support matrix;
  a gap that outlives its reason is a defect with a name.
- The gRPC contract grows by the verbs the local edition already has, under
  `buf breaking`; the additions are additive and the contract gate proves it.
- The test costs a full session on both backends per run; it runs in the
  full gate, not in the development loop, and the development loop runs the
  set-equality check alone because it is seconds.
