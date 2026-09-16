# MADE Observability Design

*Owner: Observability lead · Date: 2026-06-10 (implemented 2026-06-11; truth
pass 2026-09-17) · Status: **shipped, except §7***

> **How to read this.** This started as a design and is now the description of
> a shipped surface. Every family in §2 is registered by
> `PrometheusMetricsRecorder`
> (`crates/made-adapters/src/metrics/prometheus_recorder.rs`) behind the core
> `MetricsRecorderPort` (`crates/made-core/src/ports/metrics_recorder.rs`),
> except the five legacy series hand-rolled in `crates/made/src/health.rs`;
> every row names the file that records it. What the code does not have is in
> **§7 Planned — not implemented**, each item with the plan slice that would
> land it, and nothing there is written in the present tense. Alerts (§4) and
> dashboard panels (§5) only reference series that exist. High-cardinality
> identifiers (`task_id`, `proposal_id`, `ceremony_id`, `contract_id` on hot
> paths) are **never** used as metric labels.

The catalogue was built across PRs #102–#113. The original five
`Statistics`-backed series still ship **alongside** the rich families — they
were not replaced. Two label sets differ from the first draft of this design
and the code is the authority: ceremony metrics use `ceremony` and `step` (the
draft wrote `ceremony_name` / `step_id`), and `made_judge_score` is labelled by
`model` (the draft wrote `specialty`).

---

## 1. Executive summary

**The gap this closed, in one paragraph.** MADE *exposed* exactly five
Prometheus series, hand-rolled in `crates/made/src/health.rs::metrics` and fed
by a deliberately thin `StatisticsPort` (`record_deliberation(specialty,
duration)`, `record_orchestration(duration)`): `made_deliberations_total`,
`made_orchestrations_total`, `made_deliberations_specialty_total{specialty}`,
`made_operation_duration_milliseconds` (a *summary* with only `_sum`/`_count`
— no quantiles, no histogram), and `made_service_ready` (a NATS-only gauge).
There were **no histograms, no error counters, and no signal whatsoever** for
the things that make this service interesting: the LLM judge, the providers,
the ceremonies, or deliberation outcome quality. OTEL tracing was wired
(`telemetry.rs`, `otel` feature, OTLP/gRPC export) and the
`deliberate`/`orchestrate`/`run_ceremony` spans carried useful fields, but the
rich per-phase span *events* went through the observer, not the metrics path —
so none of it was queryable or alertable. A `NoValidProposal` failure, a judge
timeout, a 429 from a provider, a token-cost blowout, or a ceremony stuck on an
unsatisfiable guard were all invisible. Worst of all, judge token usage was
**structurally impossible** to read: `openai_compat::ChatResponse` deserialized
only `choices` — the `usage` field was dropped at the wire layer for every
provider and the judge. The `usage` field was restored and the families in §2
are recorded through `MetricsRecorderPort`; §7 says what is still missing.

**The five signals that matter most** for a deliberation orchestrator (in
priority order):

1. **Deliberation outcome quality** — winner-score distribution and
   `NoValidProposal` rate. This is the product working or not.
2. **Judge discrimination** — does the LLM judge actually re-rank proposals, or
   is it dead weight burning tokens? (`judge_winner ≠ first-passing` rate +
   score spread.)
3. **Judge & provider health** — latency, error class (401/429/5xx/timeout),
   and token cost per provider kind, with the judge isolated from the proposing
   agents.
4. **Serial model saturation** — a model server configured `max-num-seqs=1`
   queues concurrent deliberations head-to-tail; in-flight depth predicts
   timeouts before they happen.
5. **Ceremony completion health** — per-step duration and the
   failure/blocked-transition modes that strand a meeting short of its terminal
   state.

---

## 2. Metrics catalogue

Twenty-one families in the registry, plus the five legacy series. Conventions:
histograms use seconds (`_seconds`); the legacy series keep their
`_milliseconds` name. "Recorded at" is a path under `crates/`.

### 2.1 RED / infra

| Metric | Type | Labels | Recorded at | Why it matters |
|---|---|---|---|---|
| `made_deliberations_total` | counter | — | `made/src/health.rs::metrics`, from `StatisticsPort` | Baseline throughput. |
| `made_orchestrations_total` | counter | — | `made/src/health.rs::metrics` | Baseline throughput. |
| `made_deliberations_specialty_total` | counter | `specialty` | `made/src/health.rs::metrics` | Per-specialty volume; denominator for rates. |
| `made_service_ready` | gauge | — | `made/src/health.rs::metrics` | **Reads the NATS connection state and nothing else.** `GET /readyz` checks NATS *and* Postgres and is the readiness signal; this gauge is narrower than that endpoint, so alert on the probe, not on the gauge. |
| `made_operation_duration_milliseconds` | summary | — | `made/src/health.rs::metrics` | Legacy `_sum`/`_count`; it cannot produce a p95. Superseded by `made_deliberation_duration_seconds` and still shipped: removing a series clients may scrape is its own change. |
| `made_postgres_pool_in_use` | gauge | — | `made/src/health.rs::metrics`, sampled from `pool.in_use()` at scrape time | Pool is small (max 10, 5s acquire timeout); saturation cascades into readiness failures. |
| `made_nats_publish_duration_seconds` | histogram | `subject_kind` | `made-adapters/src/nats/messaging.rs` | Completion events drive downstream orchestration; publish stalls lose work. |
| `made_nats_publish_errors_total` | counter | `subject_kind`, `reason` | `made-adapters/src/nats/messaging.rs`; `reason` ∈ {`serialize`,`publish`} | Publish failures that would otherwise only be debug-logged. |

The five legacy series and the Postgres gauge exist on the **deployable**
edition only: they are written by the HTTP `/metrics` handler, and the embedded
edition has no HTTP surface (§8).

### 2.2 Deliberation quality

Recorded in `made-app/src/usecases/deliberate.rs`, at the completion path of
`execute_with_observer`.

| Metric | Type | Labels | Recorded at | Why it matters |
|---|---|---|---|---|
| `made_deliberation_duration_seconds` | histogram | `specialty` | `deliberate.rs`, from `deliberation.duration()`, before the winner is picked — so a `NoValidProposal` is still timed | The real distribution the legacy summary cannot give. |
| `made_deliberation_completed_total` | counter | `specialty`, `outcome` | `deliberate.rs`; `outcome=no_valid_proposal` when `winner_selection::pick_winner` fails, `outcome=success` otherwise | **Failure rate of the product.** `record_deliberation` on `StatisticsPort` runs before `pick_winner`, so a `NoValidProposal` is counted in `made_deliberations_total` while the task fails — this metric is the only place that distinction surfaces. |
| `made_deliberation_winner_score` | histogram | `specialty` | `deliberate.rs`, `winner.outcome().score()` (0.0–1.0), buckets `[0.1,…,1.0]` | Outcome quality. Persistent low scores = weak proposals; persistent ≥0.95 = lenient validators or rubric. |

> Cut from the mined catalogue: `proposal_content_length_bytes` (vanity —
> length is not quality, and the `author_agent` label flirts with cardinality)
> and `deliberation_proposals_total{phase}` as a *counter*. Per-deliberation
> proposal and revision counts are §7.

### 2.3 Judge

| Metric | Type | Labels | Recorded at | Why it matters |
|---|---|---|---|---|
| `made_judge_latency_seconds` | histogram | `model` | `made-adapters/src/agents/judge.rs::rate` (and `support_judge.rs`), around the HTTP call, success or failure | The judge has a 60s timeout (`DEFAULT_TIMEOUT`) and dominates the validating phase. `model` is one value per deploy. |
| `made_judge_score` | histogram | `model` | `judge.rs`, the 0.0–1.0 verdict | Distribution and calibration. Clustering at extremes or a flat band signals a broken judge or a wrong threshold. |
| `made_judge_discrimination_total` | counter | `specialty`, `result` | `deliberate.rs`, from the `Discrimination` the scoring policy reports; `result` ∈ {`reranked`,`agreed`,`tie`} (`made-core/src/value_objects/discrimination.rs`) | **The killer metric.** If `reranked` ≈ 0 over a long window, the judge never changes the outcome → it is pure cost. Derive the ratio in the dashboard, not as a separate gauge. |
| `made_judge_errors_total` | counter | `model`, `error_kind` | `judge.rs` error paths; `error_kind` is `LlmErrorKind` ∈ {`unauthorized`,`rate_limited`,`bad_request`,`upstream_error`,`malformed_body`,`empty_content`,`timeout`,`transport`} | Judge errors fail deliberations. `timeout` vs `rate_limited` vs `unauthorized` demand different responses. |
| `made_judge_tokens_total` | counter | `model`, `token_type` | `judge.rs`, from `ChatResponse.usage`; `token_type` ∈ {`prompt`,`completion`} | Cost. Combined with discrimination → cost-per-rerank ROI. |
| `made_judge_scoring_mode_total` | counter | `mode` | `made-adapters/src/scoring/judge_aware_scoring.rs`; `mode` ∈ {`judge_verdict`,`uniform_fallback`} | Confirms the judge verdict is actually consumed. A spike in `uniform_fallback` means the judge silently is not running (misconfig) even though `JudgeAwareScoring` is plugged in. |

> Cut: `judge_model_availability` via a synthetic background probe (a separate
> active health check is its own subsystem; for a 60s-timeout serialized judge
> it adds load and races real traffic — derive availability from
> `made_judge_errors_total` plus the absence of successes),
> `judge_score_by_proposal_generation` (requires tagging every proposal with
> its generation; high complexity, speculative payoff), and the
> `judge_threshold_pass_rate{threshold_value}` label (`threshold_value` is
> config, not a dimension — use `made_judge_score` and a dashboard threshold
> line).

### 2.4 Providers (proposing agents)

Latency and in-flight depth come from one RAII guard,
`made-adapters/src/agents/instrument.rs::ProviderCallGuard`, entered by each
adapter around its HTTP call; errors and tokens are recorded by the adapters
themselves (`agents/openai.rs`, `agents/vllm.rs`, `agents/anthropic.rs`).

| Metric | Type | Labels | Recorded at | Why it matters |
|---|---|---|---|---|
| `made_provider_request_duration_seconds` | histogram | `provider`, `operation` | `agents/instrument.rs` on guard drop; `operation` ∈ {`generate`,`critique`,`revise`} | Per-provider, per-op latency. A local model server diverges from a cloud one under load; `generate` and `revise` may have different SLOs. |
| `made_provider_in_flight` | gauge | `provider` | `agents/instrument.rs`, incremented on entry and decremented on drop | **Serial saturation.** Against a model server pinned to one sequence, in-flight > 1 means requests are queued server-side; this is the leading indicator of the timeout cliff. |
| `made_provider_errors_total` | counter | `provider`, `error_kind` | each adapter, from `openai_compat::classify_error`; same `LlmErrorKind` set as the judge | Root-cause breakdown. 429 = saturation (alert), 401 = credential rotation, 5xx = upstream outage. |
| `made_provider_tokens_total` | counter | `provider`, `token_type` | each adapter, from `ChatResponse.usage` | Cost attribution and prompt-bloat detection per provider. |

> A model server's own queue is not observable from the client: the
> authoritative queue-depth signal is the server's `num_requests_running` /
> `num_requests_waiting`, scraped from the model pod.
> `made_provider_in_flight` is the MADE-side complement that attributes the
> wait to *us*. Cut: `provider_queue_depth{model}` as a MADE metric.

### 2.5 Ceremonies

All five families are recorded in
`made-app/src/usecases/run_ceremony_use_case.rs` and **only** there. It is the
one-shot driver behind `RunCeremony`: a host that claims, runs and completes
steps itself moves none of them, in either edition. `run_step` acquires a
`StepLease` in-loop and calls `execute_handler`; a step that fails aborts the
whole run with `InvariantViolated` — there is no retry loop inside this use
case, so "retries" would have to be measured as repeated `attempt` numbers
across re-driven runs. `iteration` identifies a successful semantic repeat
(ADR-010) and must not be aggregated as retry pressure.

| Metric | Type | Labels | Recorded at | Why it matters |
|---|---|---|---|---|
| `made_ceremony_completed_total` | counter | `ceremony`, `outcome` | every `execute` return point; `outcome` is `CeremonyOutcome` ∈ {`completed`,`step_failed`,`no_transition`,`iteration_limit`,`repeat_limit`,`already_exists`} | Completion rate per ceremony type and the failure-mode split. `ceremony` is the definition name (a bounded set), **not** the instance id. |
| `made_ceremony_duration_seconds` | histogram | `ceremony` | `execute`, clock delta from the run's start at each completion return | End-to-end meeting latency; per-type p95. |
| `made_ceremony_step_duration_seconds` | histogram | `ceremony`, `step` | `execute`, clock delta around `run_step` | Slow-step isolation (a deliberating step vs a decision step). `step` is a bounded set per definition. |
| `made_ceremony_step_total` | counter | `ceremony`, `step`, `status` | `execute`, from `step_result.status()` | Per-step volume and failure split; denominator for step failure rate. |
| `made_ceremony_transition_blocked_total` | counter | `ceremony`, `from_state` | `execute`, where `next_satisfied_transition` returns `None` | Distinguishes a deadlocked state machine (guard never satisfiable) from waiting on a pending event. Guard names stay off the labels — guard sets can be large and varied; surface them on the trace instead. |

> Cut: `ceremony_lease_contention_total` (this driver is single-driver;
> idempotency rejection happens across distributed re-drives at the repository
> layer, which this use case does not exercise — re-add only when a distributed
> executor pool lands), `ceremony_state_dwell_seconds` and
> `ceremony_guard_approval_latency_seconds` (they need `state_entered_at` /
> `guard_started_at` on `CeremonyInstance`: real value, but a domain-model
> change), `ceremony_definition_versions_active` (deploy-tracking vanity), and
> `ceremony_timeouts_triggered_total` / `ceremony_max_attempts_exhausted_total`
> (the YAML `timeouts` / `retry_policies` are parsed but **not enforced** by
> this use case — you cannot emit a metric for an unimplemented behaviour;
> implement enforcement first).

---

## 3. Differentiating signals

These are the metrics a generic RED/USE dashboard would never have, and which
make this specifically a *deliberation-orchestrator* dashboard:

1. **Judge discrimination** —
   `rate(made_judge_discrimination_total{result="reranked"}[1h]) / rate(made_judge_discrimination_total[1h])`.
   Answers the one question no generic dashboard asks: *is the expensive LLM
   judge actually doing anything?* A near-zero ratio means the judge is burning
   tokens to confirm the first proposal.
2. **Winner-score distribution** — `made_deliberation_winner_score` (p50/p95
   plus the full histogram). Outcome *quality*, not throughput. A drifting
   median is a silent regression no latency SLO would catch.
3. **NoValidProposal rate** —
   `rate(made_deliberation_completed_total{outcome="no_valid_proposal"}[5m]) / rate(made_deliberation_completed_total[5m])`.
   The product-failure signal unique to contract-enforced deliberation: every
   proposal was generated but none satisfied the `OutputContract`.
4. **Serial model saturation** — `made_provider_in_flight` joined with the
   model server's own waiting-requests gauge. Where inference is serialized,
   concurrency that any other service would absorb becomes a queue. This is the
   metric that explains a p95 latency cliff that per-request latency hides.
5. **Judge score calibration** — `made_judge_score` spread per model. A
   collapsed distribution (all 0.7–0.8) means the judge has lost signal; a
   bimodal one means proposal quality is genuinely split. Drives threshold
   tuning.
6. **Scoring-mode integrity** — `made_judge_scoring_mode_total{mode}`. Confirms
   the judge verdict is consumed rather than silently falling back to uniform
   pass-fraction — a misconfiguration class that is otherwise undetectable
   because the service keeps "working."
7. **Cost-per-rerank ROI** —
   `increase(made_judge_tokens_total[1h]) / increase(made_judge_discrimination_total{result="reranked"}[1h])`.
   Ties spend directly to value delivered; the basis for "keep / cheaper model
   / disable judge" decisions.

---

## 4. Alerts

Symptom-based; **page only on user-visible pain.** Every expression below reads
a series from §2. SLO burn-rate alerts at the end.

| Name | Expr (PromQL-ish) | for | Severity | Rationale |
|---|---|---|---|---|
| `DeliberationNoValidProposalHigh` | `sum(rate(made_deliberation_completed_total{outcome="no_valid_proposal"}[5m])) / sum(rate(made_deliberation_completed_total[5m])) > 0.2` | 10m | **page** | >20% of deliberations return nothing usable — direct task failures for clients. Contract too strict, judge miscalibrated, or quality regression. |
| `JudgeErrorsHigh` | `sum(rate(made_judge_errors_total{error_kind=~"rate_limited\|unauthorized\|upstream_error\|timeout"}[5m])) / sum(rate(made_judge_latency_seconds_count[5m])) > 0.05` | 10m | **page** | Judge failing >5% → deliberations fail at the validating gate. `unauthorized` = creds; `rate_limited`/`upstream` = provider; `timeout` = saturation. |
| `ProviderRateLimited` | `sum(rate(made_provider_errors_total{error_kind="rate_limited"}[5m])) by (provider) > 0.1` | 10m | **page** | >10% 429s → backpressure into every deliberation. Reduce `num_agents`/concurrency or raise quota. |
| `PostgresPoolSaturated` | `made_postgres_pool_in_use / 10 > 0.9` | 5m | **page** | Pool max is 10 with a 5s acquire timeout; >9 in use means the next acquire times out and `/readyz` flips. |
| `NatsPublishErrors` | `sum(rate(made_nats_publish_errors_total{subject_kind="deliberation_completed"}[5m])) > 0.01` | 5m | **page** | Completion events lost → downstream orchestration silently drops work. |
| `JudgeLatencyApproachingTimeout` | `histogram_quantile(0.99, sum by (le) (rate(made_judge_latency_seconds_bucket[5m]))) > 45` | 10m | **ticket** | p99 nearing the 60s judge timeout; the validating phase will start failing. Provider overload or model regression. |
| `ProviderSerialSaturation` | `avg_over_time(made_provider_in_flight[10m]) > 3` | 10m | **ticket** | Sustained queueing against a serialized model server; latency cliff imminent. Scale it or cap concurrency. |
| `CeremonyCompletionLow` | `sum(rate(made_ceremony_completed_total{outcome="completed"}[15m])) by (ceremony) / sum(rate(made_ceremony_completed_total[15m])) by (ceremony) < 0.9` | 15m | **ticket** | <90% of a ceremony type reaches terminal — deadlocked guard, flaky step, or unsatisfiable transition. Covers `RunCeremony` runs only (§2.5). |
| `CeremonyTransitionsDeadlocked` | `sum(rate(made_ceremony_transition_blocked_total[5m])) by (ceremony, from_state) > 0.05` | 10m | **ticket** | A state cannot advance — guard logic deadlock or a missing event. |
| `JudgeDiscriminationZero` | `sum(rate(made_judge_discrimination_total{result="reranked"}[6h])) / sum(rate(made_judge_discrimination_total[6h])) < 0.01` | 6h | **info** | Judge never changes the winner over 6h → likely dead weight; consider a cheaper model or disabling. Cost/quality decision, not an incident. |
| `JudgeFallbackUnexpected` | `sum(rate(made_judge_scoring_mode_total{mode="uniform_fallback"}[1h])) / sum(rate(made_judge_scoring_mode_total[1h])) > 0.5` | 1h | **info** | Judge verdict not consumed though `JudgeAwareScoring` is wired — silent misconfig. Score key missing, or judge not in the validator list. |

### SLOs (multi-window burn-rate)

Burn-rate alerts follow the Google SRE 2-window pattern: page on a fast burn
(budget gone in ~2 days) confirmed by a slower window; ticket on slow burn.

**SLO 1 — Deliberation success ≥ 99%.**
`success = made_deliberation_completed_total{outcome="success"}`; error budget
= 1%.
- Page: `burn_rate_1h > 14.4 AND burn_rate_5m > 14.4` (2% of budget in 1h).
- Ticket: `burn_rate_6h > 6 AND burn_rate_30m > 6`.
- Where `burn_rate_W = (1 - sum(rate(success[W])) / sum(rate(made_deliberation_completed_total[W]))) / 0.01`.

**SLO 2 — Deliberation latency: 95% of deliberations < 90s.** Good events =
`made_deliberation_duration_seconds_bucket{le="90"}`; budget = 5%.
- Page: fast burn `1h & 5m`; Ticket: slow burn `6h & 30m`, same multipliers
  (14.4 / 6) against the 5% budget.

An orchestration-availability SLO would need per-RPC status, which no series
carries (§7).

---

## 5. Dashboard layout

One Grafana dashboard, `specialty` and `provider` as template variables.
Top-to-bottom = triage order: is it down → is the *output* good → why
(judge/providers) → are meetings healthy → infra → drill into one meeting.

**Row 1 — Health & SLO (top, always visible)**
- *Service up* — stat from the `/readyz` probe (NATS + Postgres).
  `made_service_ready` beside it reads the NATS connection only, so it is a
  dependency panel, not the verdict.
- *SLO burn-down* — two gauges: deliberation success and deliberation p95<90s,
  each showing the remaining 30-day error budget. Source: SLO recording rules
  over §4.
- *Readiness dependencies* — `made_postgres_pool_in_use` and
  `made_service_ready`. Viz: stat row.

**Row 2 — Throughput**
- *Deliberation & orchestration throughput*. Source:
  `rate(made_deliberations_total)`, `rate(made_orchestrations_total)`.

**Row 3 — Deliberation quality (the differentiators)**
- *Outcome rate, stacked* — success vs no_valid_proposal by specialty. Source:
  `made_deliberation_completed_total`. Viz: stacked area.
- *Winner-score distribution* — heatmap of `made_deliberation_winner_score`
  plus a p50/p95 overlay. Viz: heatmap.
- *Deliberation duration* p50/p95 by specialty. Source:
  `made_deliberation_duration_seconds`.

**Row 4 — Judge**
- *Discrimination ratio* — `reranked / total` from
  `made_judge_discrimination_total`, with the `agreed`/`tie` split. Viz: time
  series plus the ratio as a stat.
- *Judge score distribution* — heatmap of `made_judge_score` with the
  configured threshold as a horizontal marker.
- *Judge latency* p50/p95/p99 against the 60s timeout line. Source:
  `made_judge_latency_seconds`.
- *Judge errors* stacked by `error_kind`. Source: `made_judge_errors_total`.
- *Judge token cost & ROI* — `increase(made_judge_tokens_total[1h])`
  (prompt+completion) and cost-per-rerank.
- *Scoring mode* — judge_verdict vs uniform_fallback share. Source:
  `made_judge_scoring_mode_total`.

**Row 5 — Providers**
- *Latency comparison* — p95 of `made_provider_request_duration_seconds` by
  `provider` × `operation`.
- *Error breakdown* — `made_provider_errors_total` stacked by `error_kind`,
  faceted by `provider`.
- *Token usage* — `made_provider_tokens_total` by provider, prompt vs
  completion.
- *Serial saturation* — `made_provider_in_flight` overlaid with the model
  server's own scraped waiting-requests gauge; a scatter of in-flight against
  provider p95 exposes the saturation knee.

**Row 6 — Ceremonies** (`RunCeremony` runs only, §2.5)
- *Completion rate* by `ceremony`, stacked by `outcome`. Source:
  `made_ceremony_completed_total`.
- *Ceremony duration* p50/p95 by type. Source: `made_ceremony_duration_seconds`.
- *Step duration heatmap* — `made_ceremony_step_duration_seconds` over
  `ceremony`×`step`.
- *Step failures* — `made_ceremony_step_total{status="failed"}`. Viz: table
  sorted by failure rate.
- *Blocked transitions* — `made_ceremony_transition_blocked_total` by
  `from_state`.

**Row 7 — Infra**
- *Postgres* — `made_postgres_pool_in_use`.
- *NATS* — publish p95 from `made_nats_publish_duration_seconds` and the error
  rate from `made_nats_publish_errors_total`, both by `subject_kind`.

**Row 8 — Meeting view (trace drill-down)**
- *Recent failed deliberations / ceremonies* — a table over the logs, with a
  Tempo search per row on the `deliberate` / `run_ceremony` span name. The link
  is a **search, not a deep link**: no metric carries trace exemplars (§7), so
  the operator narrows by time window and service. Once in the trace, the
  `deliberate` span events carry the debate — which agents proposed, what the
  peer critiques said, each validator verdict, and the score per proposal —
  because `deliberate.rs` emits them (`proposal drafted`, `peer critique and
  revision`, `validator verdict`, `proposal scored`, `deliberation
  completed`).

---

## 6. Where the instrumentation lives

The architectural decision, unchanged: **`StatisticsPort` was not widened.**
Its contract (`record_deliberation`, `record_orchestration`, `snapshot`) is
persistence-backed and stayed narrow; everything else went behind
`MetricsRecorderPort`, so adding a metric never forces a trait change on the
statistics adapters.

| Concern | Port method group | Adapter / call site |
|---|---|---|
| Registry and exposition | all | `made-adapters/src/metrics/prometheus_recorder.rs`; rendered by `made/src/health.rs::metrics` at `GET /metrics` |
| Legacy counters | `StatisticsPort` | `made/src/health.rs::metrics` |
| Deliberation quality | `observe_deliberation_duration`, `record_deliberation_outcome`, `observe_winner_score`, `record_discrimination` | `made-app/src/usecases/deliberate.rs` |
| Judge | `observe_judge_latency`, `observe_judge_score`, `record_judge_error`, `record_judge_tokens` | `made-adapters/src/agents/judge.rs`, `support_judge.rs` |
| Scoring mode | `record_scoring_mode` | `made-adapters/src/scoring/judge_aware_scoring.rs` |
| Providers | `observe_provider_request`, `inc_provider_in_flight`, `dec_provider_in_flight`, `record_provider_error`, `record_provider_tokens` | `made-adapters/src/agents/instrument.rs` (RAII guard) and `agents/{openai,vllm,anthropic}.rs` |
| Ceremonies | `record_ceremony_outcome`, `observe_ceremony_duration`, `observe_ceremony_step_duration`, `record_ceremony_step`, `record_ceremony_transition_blocked` | `made-app/src/usecases/run_ceremony_use_case.rs` |
| Messaging | `observe_nats_publish`, `record_nats_publish_error` | `made-adapters/src/nats/messaging.rs` |
| Pool | `set_postgres_pool_in_use` | `made/src/health.rs::metrics`, sampled at scrape |
| Who is recording | `recorder_name` | read by `GetServiceStatusUseCase`; see §8 |

`NoopMetricsRecorder` (`made-core`) keeps tests and the recorder-less
compositions clean. Composition is `made/src/compose.rs` for the deployable
edition and `made-embedded/src/embedded_made_builder.rs` for the embedded one.

Tracing is separate: `#[tracing::instrument]` on the use cases and on the gRPC
handlers (`rpc.*`), JSON logs always, and OTLP/gRPC export with optional mTLS
behind the `otel` feature of the `made` binary (`crates/made/src/telemetry.rs`).
The operator-facing half of that is
[`operations/observability-runbook.md`](operations/observability-runbook.md).

---

## 7. Planned — not implemented

Nothing in this section exists in the code. It is here because earlier versions
of this document described it in the present tense; per PRINCIPLES §1 the claim
is removed and the intent kept where a slice of
[`orchestration-patterns-plan.md`](orchestration-patterns-plan.md) §3.7 owns
it.

**Scheduled, with the slice that lands it**

| What | Slice |
|---|---|
| Ceremony metrics from the event stream instead of thirteen hand-placed calls in one use case, so the step-at-a-time path records too; and the families that path needs — step claimed, **attempt**, iteration, guard decided, intervention opened/answered, transition applied, lease acquired/expired, fan-out width | G1 |
| `trace_id`, `correlation_id` and `causation_id` on every sealed record, and exposed on the instance read, the event read and the report | G2 |
| The embedded edition's registry on the `made_get_metrics` answer (and the gRPC backend answering the same registry instead of the five legacy counters); OTLP export from `made-mcp`; a JSON-lines file sink; a default log filter that does not drop `made_app` | G3 |
| A span per ceremony step in the one-shot driver, a span for the step handler, and spans on the provider and judge adapters carrying `provider`, `model`, `error_kind` and token counts | G4 |
| Ceremony progress as a stream — `StreamCeremony` on the cluster, a pull cursor on the embedded edition | G6 |

**Not scheduled.** These were in the catalogue, the alert list or the dashboard
and had no code; they now have no slice either, so they are gone rather than
pending. Re-adding any of them starts with a slice, not with a paragraph here:

- gRPC front-door RED (`made_grpc_request_duration_seconds`,
  `made_grpc_in_flight`) and the `RuntimeExecutorUnavailable` alert and
  orchestration-availability SLO built on it. Per-RPC latency and status are
  visible today in the `rpc.*` spans, not in a series.
- `made_postgres_query_duration_seconds` — Postgres's own
  `pg_stat_statements` serves slow-query detection better.
- `made_deliberation_phase_duration_seconds` (proposing / revising /
  validating / scoring).
- `made_deliberation_proposals` and `made_deliberation_revisions`.
- The validator families `made_validator_duration_seconds`,
  `made_validator_evaluations_total`, `made_validator_errors_total`, and the
  `ValidatorPassRateBroken` alert on them.
- Trace exemplars on `made_deliberation_completed_total` and
  `made_ceremony_completed_total`, and the deep links in the dashboard's
  meeting view.
- A readiness gauge that reflects the Postgres check as well as NATS.

---

## 8. Editions: what runs where

The catalogue above is the **deployable** edition's. The embedded edition —
`made-mcp` with `MADE_MCP_BACKEND=embedded`, or a host holding
`EmbeddedMade` — is a different composition, and what it runs is smaller. Only
what has code behind it is listed here.

| | Embedded edition | Deployable edition |
|---|---|---|
| `MetricsRecorderPort` | `PrometheusMetricsRecorder` with its own `Registry`, wired by `EmbeddedMadeBuilder` when the host wires none | `PrometheusMetricsRecorder`, wired in `compose.rs` |
| Exposition | none — the registry is in process and nothing renders it over a socket | `GET /metrics` on the HTTP port |
| Families that move | the five ceremony families, and only from `RunCeremony` (§2.5) | every family in §2 |
| `made_get_status` / `made_get_metrics` | served, over `GetServiceStatusUseCase` / `GetServiceMetricsUseCase` | served, over the same two use cases |
| Traces | none — no exporter is wired | OTLP over gRPC behind the `otel` feature |

Three things follow, and each is a gap rather than a claim:

- **The step-at-a-time path records nothing, in either edition.** The five
  ceremony families are recorded from `RunCeremonyUseCase`; a host that
  claims, runs and completes steps itself moves no counter. G1 is where that
  changes.
- **`made_get_metrics` answers with the `Statistics` counters, not the
  registry.** Deliberations and orchestrations are council work, so an
  embedded engine reports them at zero however many sessions it runs — every
  family present, every value true. Putting the registry itself on that
  answer is G3, and it needs a contract field the deployable edition can fill
  too.
- **`ServiceStatus::recorder` names what is recording and is not on the
  wire.** A Rust host reads it through the facade; neither MCP arm renders it,
  because `GetStatusResponse` has four fields and giving one arm a fifth is
  the divergence ADR-014 exists to prevent.
