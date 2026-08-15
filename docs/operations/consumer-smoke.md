# Consumer-smoke harness

`made-consumer-smoke` is a CLI that drives MADE's
public surface the way a real downstream consumer would. It does not
share any in-process types with MADE's own runtime — it
only talks gRPC over `tonic` and (optionally) core NATS over
`async-nats`. That makes it a faithful smoke test of the integration
contract a real consumer commits to.

The default smoke runs two provider-free chains. An opt-in positive
path is available when the target MADE has a provider-backed
agent kind enabled and a compatible endpoint is reachable:

- **Chain 1** — Warn-mode reevaluation. Mirrors what a consumer
  triggers after observing an incident or domain event: optionally
  publish a trigger envelope, invoke `RunCouncilDecision` in Warn
  mode with a kernel-rehydration-shaped bundle, then assert on the
  typed response + the outbound `made.deliberation.completed`
  envelope (correlation / causation propagation).

- **Chain 2** — Strict-mode handoff report / rejection path. Registers the canonical
  Report `OutputContract` (JSON Schema body bound in
  `OutputContract.json_schema`), invokes `RunCouncilDecision` in
  Strict mode, and asserts that MADE rejects free-form
  NoopAgent text with `Code::FailedPrecondition` whose message
  mentions the contract id.

- **Positive path** — Provider-backed strict Report. Registers the
  canonical Report contract, registers an `openai` or `vllm` agent
  against an OpenAI-compatible endpoint, creates a one-agent council,
  runs `RunCouncilDecision` in Strict mode, and validates that
  `report_payload_validates` is `Passed`.

## Prerequisites

- A running MADE (e.g. `make e2e-compose`, or a live
  cluster you can reach over gRPC).
- A seeded council under the target specialty (default `triage`)
  with at least one agent registered. Without a council the gRPC
  call returns `Code::NotFound` and Chain 1 records every assertion
  as Failed.
- For Chain 2: a writable contract registry. The chain calls
  `RegisterContract` and tolerates `AlreadyExists` /
  `FailedPrecondition` (already seeded) as a pass — but the registry
  must accept new contracts when starting from empty.
- For `positive-path`: the MADE binary must be built with
  the provider feature and booted with the provider's base config:
  `agent-openai` plus `MADE_OPENAI_API_KEY` for `openai`, or
  `agent-vllm` plus `MADE_VLLM_MODEL` and `MADE_VLLM_ENDPOINT`
  for `vllm`. Per-run `provider.endpoint` and `provider.model`
  overrides are sent through `RegisterAgent.agent_config`.

For the canonical bus subjects (Chain 1's NATS-coupled assertions):

- Trigger: `made.trigger.<specialty>`
- Deliberation completed: `made.deliberation.completed`

If `--nats-url` is omitted, those assertions are recorded as
`Skipped` (never silently dropped) and the rest of the chain still
runs.

`--chain all` runs Chain 2 before Chain 1. That lets a fresh registry
receive the Report contract before Chain 1 consumes the same contract
in Warn mode.

## Invocation

```bash
cargo run -p made-consumer-smoke -- \
    --endpoint http://localhost:50055 \
    [--nats-url nats://localhost:4222] \
    [--chain {one,two,all,positive-path}] \
    [--specialty triage] \
    [--contract-id consumer-smoke-report-v1] \
    [--provider-kind {openai,vllm}] \
    [--provider-endpoint http://localhost:8000] \
    [--provider-model stub-report-v1] \
    [--positive-specialty consumer-smoke-report-openai]
```

Environment overrides:

- `MADE_ENDPOINT` — defaults to `http://localhost:50055`.
- `MADE_NATS_URL` — optional. When set, Chain 1 publishes the
  trigger envelope and subscribes to `made.deliberation.completed`
  for the correlation/causation assertions.
- `MADE_REPORT_SCHEMA_PATH` — Chain 2 reads the schema from this
  path. Default `api/examples/output-contracts/report.schema.json`
  (relative to the binary's cwd).
- `CONSUMER_SMOKE_PROVIDER_KIND` — provider kind for
  `positive-path`; `openai` or `vllm`. Default `openai`.
- `CONSUMER_SMOKE_PROVIDER_ENDPOINT` — OpenAI-compatible endpoint
  used by `positive-path`. Required for that chain.
- `CONSUMER_SMOKE_PROVIDER_MODEL` — model override sent in
  `RegisterAgent.agent_config`. Default `stub-report-v1`.
- `CONSUMER_SMOKE_POSITIVE_SPECIALTY` — optional specialty for the
  positive council. If omitted, the CLI uses
  `consumer-smoke-report-<provider-kind>`.
- `RUST_LOG` — standard tracing filter. Default `info`.

A `make consumer-smoke` target wraps the same call:

```bash
make consumer-smoke
CONSUMER_SMOKE_CHAIN=two MADE_ENDPOINT=https://staging:50055 \
    make consumer-smoke
```

Positive path against a local OpenAI-compatible stub/provider:

```bash
cargo run -p made-consumer-smoke -- \
    --endpoint http://localhost:50055 \
    --chain positive-path \
    --provider-kind openai \
    --provider-endpoint http://localhost:8000 \
    --provider-model stub-report-v1
```

## CI Example

A downstream consumer can gate a staging deploy with two smoke steps:
the default provider-free chain set, then the provider-backed positive
path. The first step checks that the public API is alive, that strict
schema rejection works, and, when `MADE_NATS_URL` is set, that
correlation/causation propagate on the bus. The second step checks
that a structured Report JSON winner is accepted.

```yaml
name: made-consumer-smoke

on:
  workflow_dispatch:
  schedule:
    - cron: "17 * * * *"

jobs:
  smoke:
    runs-on: ubuntu-latest
    env:
      MADE_ENDPOINT: ${{ secrets.MADE_ENDPOINT }}
      MADE_NATS_URL: ${{ secrets.MADE_NATS_URL }}
      CONSUMER_SMOKE_PROVIDER_KIND: openai
      CONSUMER_SMOKE_PROVIDER_ENDPOINT: ${{ secrets.CONSUMER_SMOKE_PROVIDER_ENDPOINT }}
      CONSUMER_SMOKE_PROVIDER_MODEL: ${{ vars.CONSUMER_SMOKE_PROVIDER_MODEL }}
    steps:
      - uses: actions/checkout@v4
        with:
          repository: underpass-ai/made
          ref: <pinned-tag-or-sha>
          path: made-smoke

      - name: Provider-free API and rejection smoke
        working-directory: made-smoke
        run: |
          cargo run -p made-consumer-smoke --locked -- \
            --chain all

      - name: Positive Report smoke
        working-directory: made-smoke
        run: |
          cargo run -p made-consumer-smoke --locked -- \
            --chain positive-path
```

The job relies on exit codes only: `0` passes, `1` means a consumer
assertion failed, and `2` means the smoke could not run.

## What each chain asserts

| Chain | Assertion | Pass when |
|-------|-----------|-----------|
| chain1 | `rpc_returned_winner` | `response.winner` is `Some` |
| chain1 | `validation_summary_present` | `response.validation` is `Some` |
| chain1 | `candidates_non_empty` | `response.candidates.len() > 0` |
| chain1 | `bundle_seam_documented` | always `Skipped` — points at Epic 11 scenario 7 (bundle round-trip) |
| chain1 | `trigger_envelope_observed` | a `made.deliberation.completed` envelope with the run's `correlation_id` arrives within 5 s |
| chain1 | `causal_metadata_propagated` | that envelope's `causation_id` matches the one the harness sent |
| chain2 | `report_schema_registered` | `RegisterContract` succeeds or the contract already exists |
| chain2 | `report_contract_rejects_freeform_text` | `RunCouncilDecision` returns `FailedPrecondition` mentioning the contract id |
| chain2 | `report_payload_validates` | `Skipped` on rejection path; positive validation belongs to `positive-path` |
| positive-path | `positive_provider_endpoint_configured` | `--provider-endpoint` / `CONSUMER_SMOKE_PROVIDER_ENDPOINT` is present |
| positive-path | `report_schema_registered` | `RegisterContract` succeeds or the contract already exists |
| positive-path | `positive_agent_registered` | `RegisterAgent` accepts the `openai`/`vllm` descriptor and endpoint/model overrides |
| positive-path | `positive_council_created` | `CreateCouncil` succeeds or the council already exists |
| positive-path | `run_council_decision_strict` | Strict `RunCouncilDecision` returns a winner proposal |
| positive-path | `report_payload_validates` | the winner's content parses as JSON and satisfies the Report schema |
| positive-path | `report_validation_summary_passed` | `response.validation.passed == true` |
| positive-path | `positive_completion_envelope_observed` | optional: with NATS configured, the completion envelope for the task arrives |

Each assertion is also typed (`Passed` / `Skipped { reason }` /
`Failed { detail }`), so callers that embed the library can assert on
the typed shape without parsing the printed table.

## Known limitations

- **`bundle_seam_documented` is intentionally `Skipped`.** The
  stack-level external context bundle round-trip is covered by
  `make e2e-compose` scenario 7; this consumer harness keeps that
  assertion as a documented out-of-scope seam rather than duplicating
  the stack E2E.
- **Positive path is opt-in.** The default smoke path targets a
  provider-free NoopAgent stack and proves strict schema rejection.
  Run `--chain positive-path` only when the target deployment has
  `openai` or `vllm` enabled and an OpenAI-compatible endpoint is
  reachable.
- The trigger publish in Chain 1 is informational only —
  `RunCouncilDecision` is invoked directly, so the trigger path does
  not gate the run. Pinning the trigger-driven path end-to-end is
  Epic 11's territory.

## Exit codes

- `0` — every selected chain passed (at least one `Passed`
  assertion, no `Failed`).
- `1` — at least one chain recorded a `Failed` assertion.
- `2` — infrastructure error: could not connect to the gRPC endpoint
  within the configured budget, or a chain runner returned `Err`
  (typically a panicking dependency).

## The kernel rehydration seam

`made_consumer_smoke::bundle::deterministic_bundle()` returns a
literal `ExternalContextBundle`. A real consumer integration would
replace it with the result of a kernel rehydration call (Underpass
KMP, RAG, whatever the consumer wires) before invoking
`RunCouncilDecision`:

```text
  let bundle = kernel.rehydrate(...).await?;
  harness.grpc
      .run_council_decision(req.with_external_context(bundle))
      .await?;
```

Keeping the rehydration adapter out of this crate keeps the smoke
binary's dependency surface narrow. The chains exercise the
MADE's public RPC + bus contract; the kernel boundary is a
separate integration concern.
