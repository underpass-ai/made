# Choreographer Observability Design

*Owner: Observability lead · Date: 2026-06-10 (implemented 2026-06-11) · Status: **implemented***

> **Implementation status.** This started as a design and is now shipped. The catalogue below was built across PRs #102–#113 and is served at `GET /metrics` by `PrometheusMetricsRecorder` (`crates/choreo-adapters/src/metrics/prometheus_recorder.rs`) behind the core `MetricsRecorderPort`. The shipped surface matches this plan with three deltas to keep in mind while reading the catalogue, alerts, and dashboard:
>
> - **Deferred (not shipped yet):** the gRPC front-door RED (`choreo_grpc_request_duration_seconds`, `choreo_grpc_in_flight`) — per-RPC latency/status is already observable via the OTEL request traces; Postgres `query_duration` (Postgres's own `pg_stat_statements` serves slow-query detection better); deliberation phase durations; proposals/revisions counts; the validator metrics (§2.6); and `ceremony_step_attempts`. Alerts and dashboard panels that reference any of these are pending those metrics.
> - **Label corrections vs the plan:** ceremony metrics use the labels `ceremony` and `step` (the plan wrote `ceremony_name` / `step_id`), and `choreo_judge_score` is labelled by `model` (the plan wrote `specialty`).
> - The original five `Statistics`-backed series still ship **alongside** the rich families — they were not replaced.

This document was grounded in the code on the `feat/observability-deliberation-trace` branch and is now implemented on `main`. Every metric below names a concrete instrumentation point in the source. High-cardinality identifiers (`task_id`, `proposal_id`, `ceremony_id`, `contract_id` on hot paths) are **never** used as metric labels.

---

## 1. Executive summary

**The gap this closed, in one paragraph.** The Choreographer *exposed* exactly five Prometheus series, hand-rolled in `crates/choreo/src/health.rs::metrics` and fed by a deliberately thin `StatisticsPort` (`record_deliberation(specialty, duration)`, `record_orchestration(duration)`): `choreo_deliberations_total`, `choreo_orchestrations_total`, `choreo_deliberations_specialty_total{specialty}`, `choreo_operation_duration_milliseconds` (a *summary* with only `_sum`/`_count` — no quantiles, no histogram), and `choreo_service_ready` (a NATS-only gauge). There were **no histograms, no error counters, and no signal whatsoever** for the things that make this service interesting: the LLM judge, the providers, the validators, the ceremonies, or deliberation outcome quality. OTEL tracing was wired (`telemetry.rs`, `otel` feature, OTLP/gRPC export) and the `deliberate`/`orchestrate`/`run_ceremony` spans carried useful fields, but the rich per-phase span *events* went through the observer, not the metrics path — so none of it was queryable or alertable. A `NoValidProposal` failure, a judge timeout, a 429 from vLLM, a token-cost blowout, or a ceremony stuck on an unsatisfiable guard were all invisible. Worst of all, judge token usage was **structurally impossible** to read: `openai_compat::ChatResponse` deserialized only `choices` — the `usage` field was dropped at the wire layer for every provider and the judge. **All of that is now instrumented** (the `usage` field was restored, and the rich families below are recorded through `MetricsRecorderPort`); the catalogue describes the shipped surface, subject to the deltas in the implementation-status note above.

**The five signals that matter most** for a deliberation orchestrator (in priority order):

1. **Deliberation outcome quality** — winner-score distribution and `NoValidProposal` rate. This is the product working or not.
2. **Judge discrimination** — does the LLM judge actually re-rank proposals, or is it dead weight burning tokens? (`judge_winner ≠ first-passing` rate + score spread.)
3. **Judge & provider health** — latency, error class (401/429/5xx/timeout), and token cost per provider kind, with the judge isolated from the proposing agents.
4. **vLLM serial saturation** — gemma runs `max-num-seqs=1`, so concurrent deliberations queue head-to-tail; in-flight depth and provider wait-time predict timeouts before they happen.
5. **Ceremony completion health** — per-step duration and the failure/blocked-transition modes that strand a meeting short of its terminal state.

---

## 2. Metrics catalogue

Conventions: histograms use seconds (`_seconds`) for latency; the original series keep their `_milliseconds` names. Rows marked **NEW** below were the additions this design proposed — they are now **shipped** except for the ones called out as *Deferred* in the implementation-status note at the top (gRPC RED, Postgres `query_duration`, phase durations, proposals/revisions, the validator metrics in §2.6, and `ceremony_step_attempts`). All deliberation/judge/provider/ceremony metrics are recorded through the `MetricsRecorderPort` (see §6), not the original `StatisticsPort` (which keeps its narrow contract).

### 2.1 RED / infra

| Metric | Type | Labels | Instrumentation point | Exists? | Why it matters |
|---|---|---|---|---|---|
| `choreo_deliberations_total` | counter | — | `health.rs::metrics` via `Statistics` | **Existing** | Baseline throughput. Keep. |
| `choreo_orchestrations_total` | counter | — | `health.rs::metrics` | **Existing** | Baseline throughput. Keep. |
| `choreo_deliberations_specialty_total` | counter | `specialty` | `health.rs::metrics` | **Existing** | Per-specialty volume; denominator for rates. Keep. |
| `choreo_service_ready` | gauge | — | `health.rs::metrics` (NATS state) | **Existing** | Readiness. Extend to also reflect the `/readyz` Postgres check, not NATS only. |
| `choreo_operation_duration_milliseconds` (summary) | summary | — | `health.rs::metrics` | **Existing** | **Replace.** A sum/count summary cannot produce p95. Superseded by the histograms below. |
| `choreo_grpc_request_duration_seconds` | histogram | `method`, `code` | tonic layer in `grpc/service.rs` (one `tower` layer over all handlers) | **NEW** | Per-RPC RED. `code` from `Status::code()`. Low cardinality (≈6 methods × handful of codes). |
| `choreo_grpc_in_flight` | gauge | `method` | same tonic layer, inc on entry / dec on exit | **NEW** | Server saturation; pairs with vLLM serial saturation below. |
| `choreo_postgres_pool_in_use` | gauge | — | sample `pool.num_idle()` vs configured max in `postgres/pool.rs` (periodic task or on `/metrics` scrape) | **NEW** | Pool is small (max 10, 5s acquire timeout); saturation cascades into readiness failures. |
| `choreo_postgres_query_duration_seconds` | histogram | `op` | wrap `sqlx` calls in repositories; `op` ∈ {`deliberation_save`,`council_get`,`agent_resolve`,`ceremony_save`,…} — a closed set | **NEW** | Slow-query / missing-index detection without per-statement cardinality. |
| `choreo_nats_publish_duration_seconds` | histogram | `subject_kind` | `nats/messaging.rs::publish_*`; `subject_kind` ∈ {`deliberation_completed`,`task_dispatched`,`task_failed`,`phase_changed`} | **NEW** | Completion events drive downstream orchestration; publish stalls lose work. |
| `choreo_nats_publish_errors_total` | counter | `subject_kind`, `reason` | `nats/messaging.rs` map_err (currently only debug-logged) | **NEW** | Silent publish failures today. |

### 2.2 Deliberation quality

| Metric | Type | Labels | Instrumentation point | Exists? | Why it matters |
|---|---|---|---|---|---|
| `choreo_deliberation_duration_seconds` | histogram | `specialty` | `deliberate.rs::execute_with_observer` — `completed_at − started_at` (already computed as `duration` at line 169) | **NEW** | The real distribution the summary can't give. |
| `choreo_deliberation_phase_duration_seconds` | histogram | `specialty`, `phase` | `deliberate.rs` — clock deltas around the four phase blocks: Proposing (`seed_proposals`, ~139), Revising (`run_peer_review_rounds`, ~144), Validating (`attach_validations`, ~153), Scoring (`complete`, ~159) | **NEW** | Isolates the bottleneck. Revising scales with `rounds`; Validating is dominated by the judge. `phase` is a 4-value enum. |
| `choreo_deliberation_completed_total` | counter | `specialty`, `outcome` | `deliberate.rs` end of `execute_with_observer`; `outcome=success` on the `Ok` path (line 200), `outcome=no_valid_proposal` when `pick_winner` returns `DomainError::NoValidProposal` (line 231) | **NEW** | **Failure rate of the product.** Note: `record_deliberation` already ran (line 171) *before* `pick_winner`, so a `NoValidProposal` is counted in `choreo_deliberations_total` but the task fails — this metric is the only place that distinction surfaces. |
| `choreo_deliberation_winner_score` | histogram | `specialty` | `deliberate.rs` line 175–182, `winner.outcome().score().get()` (0.0–1.0) | **NEW** | Outcome quality. Buckets `[0,0.1,…,1.0]`. Persistent low scores = weak proposals; persistent ≥0.95 = lenient validators/rubric. |
| `choreo_deliberation_proposals` | histogram | `specialty` | `deliberate.rs` line 175, `ranked.len()` / `deliberation.proposals().len()` | **NEW** | Council size actually realised (after the `num_agents` cap in `resolve_agents`, ~263). A `0` here is an invariant violation worth paging. |
| `choreo_deliberation_revisions` | histogram | `specialty` | `deliberate.rs` after Revising; `sum(proposal.revision_count())` across `deliberation.proposals()` | **NEW** | Peer-review effectiveness. Counts of 0 with `rounds>0` = agents ignoring peers; high churn = thrashing. |

> Cut from the mined catalogue: `proposal_content_length_bytes` (vanity — length is not quality, and the `author_agent` label flirts with cardinality), `deliberation_proposals_total{phase}` as a *counter* (redundant with the histogram above), and the duplicate `proposals_per_deliberation` gauge (a gauge of a per-event value is lossy; the histogram is correct).

### 2.3 Judge

| Metric | Type | Labels | Instrumentation point | Exists? | Why it matters |
|---|---|---|---|---|---|
| `choreo_judge_latency_seconds` | histogram | `model` | `judge.rs::rate` — `Instant::now()` around the `post().send()` + `json()` block (lines 147–172) | **NEW** | Judge has a 60s timeout (`DEFAULT_TIMEOUT`) and dominates the Validating phase. `model` from `self.model` (one value per deploy). |
| `choreo_judge_score` | histogram | `model` | `judge.rs::validate` (line 187) or the `attach_validations` loop; the 0.0–1.0 `score` | **NEW** | Distribution/calibration. Clustering at extremes or a flat band signals a broken judge or a wrong threshold. |
| `choreo_judge_discrimination_total` | counter | `specialty`, `result` | `deliberate.rs` after scoring/ranking; compare the judge-ranked top vs. the first proposal that passes the structural validators. `result` ∈ {`reranked`,`agreed`,`tie`} | **NEW** | **The killer metric.** If `reranked` ≈ 0 over a long window, the judge never changes the outcome → it is pure cost. Derive a discrimination ratio in the dashboard, not a separate gauge. |
| `choreo_judge_errors_total` | counter | `model`, `error_kind` | `judge.rs::rate` error paths. Today *all* transport failures collapse to one reason (line 155) and HTTP statuses go through `classify_error`. §6 splits these into `error_kind` ∈ {`unauthorized`,`rate_limited`,`bad_request`,`upstream_error`,`malformed_body`,`empty_content`,`timeout`,`transport`} | **NEW** | Judge errors fail deliberations. `timeout` vs `rate_limited` vs `unauthorized` demand different responses. |
| `choreo_judge_tokens_total` | counter | `model`, `token_type` | **Requires wire change.** Add `usage` to `ChatResponse` (§6); read `prompt_tokens`/`completion_tokens` in `rate`. `token_type` ∈ {`prompt`,`completion`} | **NEW** | Cost. Combined with discrimination → cost-per-rerank ROI. Today structurally undeliverable. |
| `choreo_judge_scoring_mode_total` | counter | `mode` | `scoring.rs::JudgeAwareScoring::score` — `mode=judge_verdict` on the branch at line 72, `mode=uniform_fallback` on the branch at line 79 | **NEW** | Confirms the judge verdict is actually being consumed. A spike in `uniform_fallback` means the judge silently isn't running (misconfig) even though `JudgeAwareScoring` is plugged in. |

> Cut: `judge_model_availability` via a synthetic background probe (a separate active health-check is its own subsystem; for a 60s-timeout serialized judge it adds load and races real traffic — derive availability from `choreo_judge_errors_total` + the absence of successes instead), `judge_score_by_proposal_generation` (requires tagging every proposal with its generation and a per-generation histogram — high complexity, speculative payoff; revisit only if revisions look ineffective), and the `judge_threshold_pass_rate{threshold_value}` label (`threshold_value` as a label is config, not a dimension — use `choreo_judge_score` + a dashboard threshold line).

### 2.4 Providers (proposing agents)

| Metric | Type | Labels | Instrumentation point | Exists? | Why it matters |
|---|---|---|---|---|---|
| `choreo_provider_request_duration_seconds` | histogram | `provider`, `operation` | `agents/{openai,vllm,anthropic}.rs` around each HTTP call; `provider` ∈ {`openai`,`vllm`,`anthropic`}, `operation` ∈ {`generate`,`critique`,`revise`} | **NEW** | Per-provider, per-op latency. vLLM diverges from cloud under load; `generate` vs `revise` may have different SLOs. |
| `choreo_provider_errors_total` | counter | `provider`, `error_kind` | `openai_compat::classify_error` / `extract_text` results, recorded in each adapter's call site; same `error_kind` set as the judge | **NEW** | Root-cause breakdown. 429 = saturation (alert), 401 = credential rotation, 5xx = upstream outage. |
| `choreo_provider_tokens_total` | counter | `provider`, `token_type` | **Requires the same `ChatResponse.usage` change** as the judge | **NEW** | Cost attribution and prompt-bloat detection per provider. |
| `choreo_provider_in_flight` | gauge | `provider` | `Arc<AtomicI64>` inc/dec around each adapter call (the adapters share the `openai_compat` call shape, so wrap there) | **NEW** | **gemma serial saturation.** With vLLM `max-num-seqs=1`, in-flight > 1 against vLLM means requests are queued server-side; this is the leading indicator of the 60s-timeout cliff. Pair with vLLM's own `/metrics` (scrape `vllm:num_requests_waiting` directly from the vLLM pod). |

> Note on gemma: `max-num-seqs=1` is **not** configured in this repo — it is a property of the vLLM deployment. The authoritative queue-depth signal is vLLM's `vllm:num_requests_running` / `num_requests_waiting`, scraped from the model server. `choreo_provider_in_flight` is the Choreographer-side complement that attributes the wait to *us*. Cut: `provider_queue_depth{model}` as a Choreographer metric (we cannot observe vLLM's internal queue from the client; scrape vLLM instead) and `otel_span_provider_attributes` as a *counter* (it is a span-enrichment task, not a metric — fold it into the trace work in §6, adding `error_kind`/`tokens`/`model` as span attributes).

### 2.5 Ceremonies

`run_ceremony_use_case.rs` reality check, because the mined notes referenced a non-existent `run_ceremony_step_use_case.rs`: there is one use case; `run_step` acquires a `StepLease` in-loop and calls `execute_handler`; a step that fails aborts the whole ceremony with `InvariantViolated{"ceremony step did not complete successfully"}` (line 127) — **there is no retry loop inside this use case**, so "retries" must be measured as repeated `attempt` numbers across re-driven runs, not an in-call retry. `CeremonyStepTrace{state, step, role, attempt, status}` is already assembled in-memory (line 119) and is the natural emission hook.

| Metric | Type | Labels | Instrumentation point | Exists? | Why it matters |
|---|---|---|---|---|---|
| `choreo_ceremony_completed_total` | counter | `ceremony`, `outcome` | `execute` return points; `outcome` ∈ {`completed`,`step_failed`,`no_transition`,`iteration_limit`,`already_exists`} from the four error sites (lines 78/127/138/151) | **NEW** | Completion rate per ceremony type and the failure-mode split. `ceremony` = `definition_name` (bounded set of YAML definitions), **not** the instance id. |
| `choreo_ceremony_duration_seconds` | histogram | `ceremony` | `execute` — `clock.now() − instance.started_at()` at completion | **NEW** | End-to-end meeting latency; per-type p95. |
| `choreo_ceremony_step_duration_seconds` | histogram | `ceremony`, `step` | `run_step` — clock delta around `execute_handler` (line 201) | **NEW** | Slow-step isolation (a deliberate-step vs a decision-step). `step_id` is a bounded set per definition. |
| `choreo_ceremony_step_total` | counter | `ceremony`, `step`, `status` | at `step_traces.push` (line 119); `status` from `step_result.status()` | **NEW** | Per-step volume + failure split; denominator for step failure rate. |
| `choreo_ceremony_step_attempts` | histogram | `ceremony`, `step` | `attempt.get()` returned from `start_step_as` (line 185) | **NEW** | Retry pressure across re-driven runs; high attempts = flaky handler or too-short lease TTL. |
| `choreo_ceremony_transition_blocked_total` | counter | `ceremony`, `from_state` | when `next_satisfied_transition` returns `None` and `execute` errors with `no satisfied transition` (line 138) | **NEW** | Distinguishes a deadlocked state machine (guard never satisfiable) from waiting on a pending event. Drop `blocked_guard_names` as a label — guard sets can be large/varied; surface them on the trace instead. |

> Cut: `ceremony_lease_contention_total` (the in-process `run_ceremony` use case is single-driver; `StepLease::acquire` here does not fail on contention — idempotency rejection happens only across distributed re-drives at the repository layer, which this use case does not exercise. Re-add only when a distributed executor pool lands), `ceremony_state_dwell_seconds` and `ceremony_guard_approval_latency_seconds` (require new `state_entered_at` / `guard_started_at` fields on `CeremonyInstance` — real value but a domain-model change; phase 3, not now), `ceremony_definition_versions_active` (deploy-tracking vanity), and `ceremony_timeouts_triggered_total` / `ceremony_max_attempts_exhausted_total` (the YAML `timeouts`/`retry_policies` are parsed but **not enforced** by this use case — you cannot emit a metric for an unimplemented behaviour; implement enforcement first).

### 2.6 Validators / contracts

| Metric | Type | Labels | Instrumentation point | Exists? | Why it matters |
|---|---|---|---|---|---|
| `choreo_validator_duration_seconds` | histogram | `kind` | `deliberate.rs::run_validators` loop (line 384) — clock delta around `validator.validate()`; `kind` from `validator.kind()` (7 wired values incl. `llm_judge`) | **NEW** | Judge (≈seconds) vs structural validators (≈ms). Sizes the Validating-phase budget. |
| `choreo_validator_evaluations_total` | counter | `kind`, `result` | same loop; `result` ∈ {`passed`,`failed`} from `report.passed()` | **NEW** | Per-validator pass rate. A schema validator failing 80% = broken contract; a validator passing 100% = not discriminating. |
| `choreo_validator_errors_total` | counter | `kind` | same loop — when `validate()` returns `Err` (transport/timeout, not a failing-but-well-formed report). Recall a validator `Err` aborts the whole deliberation (test `validator_error_aborts_deliberation…`) | **NEW** | Separates "the judge endpoint is down" (Err) from "the proposal is bad" (passed=false). |

> Cut: `output_contract_violations_total{violation_type}` keyed by `contract_id` (the `contract_id` label is unbounded — one per registered contract; track violations by `kind`+`result` above and pivot to the specific contract via the trace), and `validator_pass_rate` as a *gauge* (a point-in-time ratio loses history; compute the rate from the two counters in PromQL).

---

## 3. Differentiating signals

These are the metrics a generic RED/USE dashboard would never have, and which make this specifically a *deliberation-orchestrator* dashboard:

1. **Judge discrimination** — `rate(choreo_judge_discrimination_total{result="reranked"}[1h]) / rate(choreo_judge_discrimination_total[1h])`. Answers the one question no generic dashboard asks: *is the expensive LLM judge actually doing anything?* A near-zero ratio means the judge is burning tokens to confirm the first proposal.
2. **Winner-score distribution** — `choreo_deliberation_winner_score` (p50/p95 + the full histogram). Outcome *quality*, not throughput. A drifting median is a silent regression no latency SLO would catch.
3. **NoValidProposal rate** — `rate(choreo_deliberation_completed_total{outcome="no_valid_proposal"}[5m]) / rate(choreo_deliberation_completed_total[5m])`. The product-failure signal unique to contract-enforced deliberation: every proposal was generated but none satisfied the `OutputContract`.
4. **Proposals & revisions per deliberation** — `choreo_deliberation_proposals` and `choreo_deliberation_revisions`. Reveals whether councils are realising their configured size (after the `num_agents` cap) and whether peer review converges or thrashes — the health of the *deliberation mechanism itself*.
5. **gemma serial saturation** — `choreo_provider_in_flight{provider="vllm"}` joined with `vllm:num_requests_waiting`. Because `max-num-seqs=1` serialises inference, concurrency that any other service would absorb here becomes a queue. This is the metric that explains a p95 latency cliff that the providers' own per-request latency hides.
6. **Judge score calibration** — `choreo_judge_score` spread per model. A collapsed distribution (all 0.7–0.8) means the judge has lost signal; a bimodal one means proposal quality is genuinely split. Drives threshold tuning.
7. **Scoring-mode integrity** — `choreo_judge_scoring_mode_total{mode}`. Confirms the judge verdict is consumed rather than silently falling back to uniform pass-fraction — a misconfiguration class that is otherwise undetectable because the service keeps "working."
8. **Cost-per-rerank ROI** — `increase(choreo_judge_tokens_total[1h]) / increase(choreo_judge_discrimination_total{result="reranked"}[1h])`. Ties spend directly to value delivered; the basis for "keep / cheaper model / disable judge" decisions.

---

## 4. Alerts

Symptom-based; **page only on user-visible pain.** All windows assume the new metrics. SLO burn-rate alerts at the end.

| Name | Expr (PromQL-ish) | for | Severity | Rationale |
|---|---|---|---|---|
| `DeliberationNoValidProposalHigh` | `sum(rate(choreo_deliberation_completed_total{outcome="no_valid_proposal"}[5m])) / sum(rate(choreo_deliberation_completed_total[5m])) > 0.2` | 10m | **page** | >20% of deliberations return nothing usable — direct task failures for clients. Contract too strict, judge miscalibrated, or quality regression. |
| `JudgeErrorsHigh` | `sum(rate(choreo_judge_errors_total{error_kind=~"rate_limited|unauthorized|upstream_error|timeout"}[5m])) / sum(rate(choreo_judge_latency_seconds_count[5m])) > 0.05` | 10m | **page** | Judge failing >5% → deliberations fail at the Validating gate. `unauthorized` = creds; `rate_limited`/`upstream` = provider; `timeout` = saturation. |
| `ProviderRateLimited` | `sum(rate(choreo_provider_errors_total{error_kind="rate_limited"}[5m])) by (provider) > 0.1` | 10m | **page** | >10% 429s → backpressure into every deliberation. Reduce `num_agents`/concurrency or raise quota. |
| `RuntimeExecutorUnavailable` | `sum(rate(choreo_grpc_request_duration_seconds_count{method="Orchestrate",code=~"UNAVAILABLE|DEADLINE_EXCEEDED"}[5m])) > 0.1` | 5m | **page** | Orchestrations fail at execution (300s runtime timeout). Executor down/unreachable. |
| `PostgresPoolSaturated` | `choreo_postgres_pool_in_use / 10 > 0.9` | 5m | **page** | Pool max is 10 with a 5s acquire timeout; >9 in use means the next acquire times out and `/readyz` flips. |
| `NatsPublishErrors` | `sum(rate(choreo_nats_publish_errors_total{subject_kind="deliberation_completed"}[5m])) > 0.01` | 5m | **page** | Completion events lost → downstream orchestration silently drops work. |
| `JudgeLatencyApproachingTimeout` | `histogram_quantile(0.99, sum by (le) (rate(choreo_judge_latency_seconds_bucket[5m]))) > 45` | 10m | **ticket** | p99 nearing the 60s judge timeout; Validating phase will start failing. Provider overload or model regression. |
| `vLLMSerialSaturation` | `avg_over_time(choreo_provider_in_flight{provider="vllm"}[10m]) > 3` | 10m | **ticket** | Sustained queueing against the `max-num-seqs=1` model; latency cliff imminent. Scale vLLM or cap concurrency. |
| `ValidatorPassRateBroken` | `sum(rate(choreo_validator_evaluations_total{result="passed"}[15m])) by (kind) / sum(rate(choreo_validator_evaluations_total[15m])) by (kind) < 0.1` | 15m | **ticket** | A structural validator rejecting ≥90% → broken contract or regressed proposals; leads to `NoValidProposal`. |
| `CeremonyCompletionLow` | `sum(rate(choreo_ceremony_completed_total{outcome="completed"}[15m])) by (ceremony) / sum(rate(choreo_ceremony_completed_total[15m])) by (ceremony) < 0.9` | 15m | **ticket** | <90% of a ceremony type reaches terminal — deadlocked guard, flaky step, or unsatisfiable transition. |
| `CeremonyTransitionsDeadlocked` | `sum(rate(choreo_ceremony_transition_blocked_total[5m])) by (ceremony, from_state) > 0.05` | 10m | **ticket** | A state can't advance — guard logic deadlock or a missing event. |
| `JudgeDiscriminationZero` | `sum(rate(choreo_judge_discrimination_total{result="reranked"}[6h])) / sum(rate(choreo_judge_discrimination_total[6h])) < 0.01` | 6h | **info** | Judge never changes the winner over 6h → likely dead weight; consider a cheaper model or disabling. Cost/quality decision, not an incident. |
| `JudgeFallbackUnexpected` | `sum(rate(choreo_judge_scoring_mode_total{mode="uniform_fallback"}[1h])) / sum(rate(choreo_judge_scoring_mode_total[1h])) > 0.5` | 1h | **info** | Judge verdict not consumed though `JudgeAwareScoring` is wired — silent misconfig. Score-key missing or judge not in the validator list. |

### SLOs (multi-window burn-rate)

Burn-rate alerts follow the Google SRE 2-window pattern: page on a fast-burn (budget gone in ~2 days) confirmed by a slower window; ticket on slow-burn.

**SLO 1 — Deliberation success ≥ 99%.** `success = choreo_deliberation_completed_total{outcome="success"}`; error budget = 1%.
- Page: `burn_rate_1h > 14.4 AND burn_rate_5m > 14.4` (2% of budget in 1h).
- Ticket: `burn_rate_6h > 6 AND burn_rate_30m > 6`.
- Where `burn_rate_W = (1 - sum(rate(success[W])) / sum(rate(choreo_deliberation_completed_total[W]))) / 0.01`.

**SLO 2 — Deliberation latency: 95% of deliberations < 90s.** Good events = `choreo_deliberation_duration_seconds_bucket{le="90"}`; budget = 5%.
- Page: fast-burn `1h & 5m` windows; Ticket: slow-burn `6h & 30m`, same multipliers (14.4 / 6) against the 5% budget.

**SLO 3 — Orchestration end-to-end availability ≥ 99%.** Good = `Orchestrate` RPCs not returning `INTERNAL|UNAVAILABLE|DEADLINE_EXCEEDED`. Burn-rate windows as SLO 1. This is the only SLO that spans deliberation **and** the Runtime executor.

---

## 5. Dashboard layout

One Grafana dashboard, `specialty` and `provider` as template variables. Top-to-bottom = triage order: is it down → is it slow/erroring → is the *output* good → why (judge/providers) → are meetings healthy → infra → drill into one meeting.

**Row 1 — Health & SLO (top, always visible)**
- *Service up* — stat from `choreo_service_ready` + `/readyz` (NATS+Postgres). Viz: stat, green/red.
- *SLO burn-down* — three gauges: deliberation success, deliberation p95<90s, orchestration availability, each showing remaining 30-day error budget. Source: SLO recording rules.
- *Readiness dependencies* — `choreo_postgres_pool_in_use`, NATS connected, runtime reachability. Viz: stat row.

**Row 2 — RED**
- *gRPC request rate / error % / p95* by `method`. Source: `choreo_grpc_request_duration_seconds`. Viz: 3 time series.
- *gRPC in-flight* by `method`. Viz: time series.
- *Deliberation & orchestration throughput*. Source: `rate(choreo_deliberations_total)`, `rate(choreo_orchestrations_total)`.

**Row 3 — Deliberation quality (the differentiators)**
- *Outcome rate, stacked* — success vs no_valid_proposal by specialty. Source: `choreo_deliberation_completed_total`. Viz: stacked area.
- *Winner-score distribution* — heatmap of `choreo_deliberation_winner_score` + p50/p95 overlay. Viz: heatmap.
- *Phase duration breakdown* — p95 of `choreo_deliberation_phase_duration_seconds` by phase. Viz: stacked bar (Proposing/Revising/Validating/Scoring).
- *Proposals & revisions per deliberation* — p50/p95 of the two histograms by specialty. Viz: time series.

**Row 4 — Judge**
- *Discrimination ratio* — `reranked / total` from `choreo_judge_discrimination_total`, with the `agreed`/`tie` split. Viz: time series + the ratio as a stat.
- *Judge score distribution* — heatmap of `choreo_judge_score` with the configured threshold as a horizontal marker. Viz: heatmap.
- *Judge latency* p50/p95/p99 vs the 60s timeout line. Source: `choreo_judge_latency_seconds`. Viz: time series.
- *Judge errors* stacked by `error_kind`. Source: `choreo_judge_errors_total`.
- *Judge token cost & ROI* — `increase(choreo_judge_tokens_total[1h])` (prompt+completion) and cost-per-rerank. Viz: time series + stat.
- *Scoring mode* — judge_verdict vs uniform_fallback share. Source: `choreo_judge_scoring_mode_total`. Viz: pie/stat.

**Row 5 — Providers**
- *Latency comparison* — p95 of `choreo_provider_request_duration_seconds` by `provider` × `operation`. Viz: time series.
- *Error breakdown* — `choreo_provider_errors_total` stacked by `error_kind`, faceted by `provider`. Viz: stacked area.
- *Token usage* — `choreo_provider_tokens_total` by provider, prompt vs completion. Viz: stacked area.
- *vLLM serial saturation* — `choreo_provider_in_flight{provider="vllm"}` overlaid with scraped `vllm:num_requests_waiting`; scatter of in-flight vs vLLM p95 latency to expose the saturation knee. Viz: time series + scatter.

**Row 6 — Ceremonies**
- *Completion rate* by `ceremony`, stacked by `outcome`. Source: `choreo_ceremony_completed_total`. Viz: stacked bar.
- *Ceremony duration* p50/p95 by type. Source: `choreo_ceremony_duration_seconds`.
- *Step duration heatmap* — `choreo_ceremony_step_duration_seconds` over `ceremony`×`step`. Viz: heatmap.
- *Step attempts & failures* — p95 `choreo_ceremony_step_attempts` and `choreo_ceremony_step_total{status="failed"}`. Viz: table sorted by failure rate.
- *Blocked transitions* — `choreo_ceremony_transition_blocked_total` by `from_state`. Viz: time series.

**Row 7 — Validators & infra**
- *Validator pass rate* by `kind`. Source: `choreo_validator_evaluations_total`. Viz: time series (one line per validator).
- *Validator latency* p95 by `kind` (judge will tower over the rest). Source: `choreo_validator_duration_seconds`. Viz: bar.
- *Postgres* — pool in-use, query p95 by `op`. Source: `choreo_postgres_*`.
- *NATS* — publish p95 + error rate by `subject_kind`. Source: `choreo_nats_*`.

**Row 8 — Meeting view (trace drill-down)**
- *Recent failed deliberations / ceremonies* — table from logs/exemplars with a **"Open in Tempo"** link per row, jumping to the `deliberate` / `run_ceremony` trace. Source: trace exemplars attached to `choreo_deliberation_completed_total{outcome!="success"}` and `choreo_ceremony_completed_total{outcome!="completed"}`. This is where an operator goes from "the NoValidProposal rate spiked" to the exact debate: which agents proposed, what the peer critiques said, what each validator verdict was, and the judge's score per proposal — all already on the `deliberate` span and its events.

---

## 6. Instrumentation plan

Ordered by leverage (highest value / lowest churn first). The architectural decision: **do not widen `StatisticsPort`.** Its contract (`record_deliberation`, `record_orchestration`, `snapshot`) is clean and persistence-backed. Instead add a new `MetricsRecorderPort` plus a Prometheus-backed recorder, so adding metrics never forces a trait change on the existing statistics adapters.

**Step 0 — Adopt a metrics client and a recorder port (foundational).**
Replace the hand-rolled exposition in `health.rs::metrics` with the `metrics` + `metrics-exporter-prometheus` crates (or `prometheus`), which give real histograms. Define in `choreo-core/src/ports/`:
```
trait MetricsRecorderPort: Send + Sync {
    fn observe_deliberation(&self, specialty: &Specialty, phase: Phase, secs: f64);
    fn record_outcome(&self, specialty: &Specialty, outcome: DeliberationOutcome);
    fn observe_winner_score(&self, specialty: &Specialty, score: f64);
    fn observe_judge(&self, model: &str, secs: f64, tokens: Option<TokenUsage>, err: Option<ErrorKind>);
    fn record_discrimination(&self, specialty: &Specialty, result: Discrimination);
    fn observe_provider(&self, provider: ProviderKind, op: Operation, secs: f64, tokens: Option<TokenUsage>, err: Option<ErrorKind>);
    fn observe_validator(&self, kind: &str, secs: f64, result: ValidatorResult);
    fn observe_ceremony_step(&self, name: &str, step: &str, secs: f64, status: StepStatus, attempt: u32);
    fn record_ceremony_outcome(&self, name: &str, outcome: CeremonyOutcome);
}
```
Inject `Arc<dyn MetricsRecorderPort>` into `DeliberateUseCase`, `OrchestrateUseCase`, `RunCeremonyUseCase`, and the provider/judge adapters via `compose.rs`. A `NoopRecorder` keeps tests and the `otel`-off build clean. **This single step unblocks every NEW metric below.**

**Step 1 — Deliberation quality (highest product value, all in `deliberate.rs`, no I/O changes).**
In `execute_with_observer`: capture `clock.now()` at each of the four phase boundaries (the calls are already sequential at lines 139/144/153/159) and call `observe_deliberation`. After `pick_winner`: `observe_winner_score`, `observe_deliberation` proposals/revisions from `ranked.len()` and `sum(proposal.revision_count())`, and `record_outcome(success)`. In `pick_winner`'s `NoValidProposal` arm (line 231), `record_outcome(no_valid_proposal)`. This delivers differentiators #2, #3, #4 with zero new dependencies.

**Step 2 — Token-usage wire change (unblocks all cost metrics).**
In `openai_compat.rs`, extend `ChatResponse`:
```
#[derive(Deserialize)] pub(super) struct Usage {
    #[serde(default)] pub prompt_tokens: u32,
    #[serde(default)] pub completion_tokens: u32,
}
// add to ChatResponse: #[serde(default)] pub usage: Option<Usage>,
```
`extract_text` consumes `ChatResponse` today (line 94) — change it to return `(String, Option<Usage>)`, or read `usage` before calling it. Thread the usage into `observe_judge`/`observe_provider`. This is the only structural blocker for `choreo_judge_tokens_total` and `choreo_provider_tokens_total`; do it before any cost panel.

**Step 3 — Judge instrumentation (`judge.rs::rate`).**
Wrap lines 147–172 in `Instant::now()`. Split the single transport `map_err` (line 153) into `timeout` (check `err.is_timeout()`) vs `transport`, and map `classify_error`'s status into the `error_kind` set. Call `observe_judge(model, secs, usage, err)`. In `validate`, after `rate`, call `observe_judge` for the success score path / record the `choreo_judge_score`. In `scoring.rs::JudgeAwareScoring::score`, emit `record` for `judge_verdict` (line 72 branch) vs `uniform_fallback` (line 79 branch). Discrimination (`record_discrimination`) is computed in `deliberate.rs` after ranking by comparing the judge-ranked top against the first structurally-passing proposal — it needs the ranked list, which `complete()` already returns.

**Step 4 — Provider instrumentation (`agents/{openai,vllm,anthropic}.rs`).**
The three adapters share the `openai_compat` call shape; wrap the HTTP call with timing + an `Arc<AtomicI64>` in-flight inc/dec, and record `observe_provider(provider, op, secs, usage, err)`. `op` comes from the method (`generate`/`critique`/`revise`). This delivers per-provider RED, tokens, and the vLLM in-flight saturation gauge (differentiator #5) in one pass.

**Step 5 — Validators (`deliberate.rs::run_validators`).**
Wrap `validator.validate()` (line 384) with timing; on `Ok(report)` record `observe_validator(kind, secs, passed/failed)`; on `Err` increment `choreo_validator_errors_total{kind}`. `kind` comes from `validator.kind()`.

**Step 6 — Ceremonies (`run_ceremony_use_case.rs`).**
At the `step_traces.push` site (line 119) the `(attempt, status, step_id, state)` are all in hand — call `observe_ceremony_step`. Time `execute_handler` (line 201) for step duration. At each `execute` return point, call `record_ceremony_outcome` with the mapped outcome (completed / step_failed / no_transition / iteration_limit / already_exists). Emit `choreo_ceremony_transition_blocked_total` at the `next_satisfied_transition == None` site (line 138).

**Step 7 — Infra (RED/USE).**
Add a tonic `tower` layer in `grpc/service.rs` for `choreo_grpc_request_duration_seconds` + `choreo_grpc_in_flight` (covers every handler at once). Wrap `sqlx` calls in the postgres repositories and sample `pool.num_idle()` in `pool.rs`. Time and error-count `publish_*` in `nats/messaging.rs` (the map_err sites that today only debug-log). Time `invoke_tool` in `runtime.rs` keyed by tonic status code (reuse the gRPC client error mapping).

**Step 8 — Trace enrichment (for the meeting view, Row 8).**
On the provider/judge spans, set attributes `error_kind`, `prompt_tokens`/`completion_tokens`, `model`, `provider`. On `deliberate`, ensure the per-proposal judge score and the final outcome are span events (the observer path already emits phase events; add `winner_score` to the completion event so Tempo shows it). Attach trace exemplars to `choreo_deliberation_completed_total` and `choreo_ceremony_completed_total` so the dashboard tables can deep-link.

**Deferred (explicitly out of this slice, require domain-model or infra work):** ceremony state-dwell and human-approval latency (need `state_entered_at`/`guard_started_at` on `CeremonyInstance`); ceremony timeout/max-attempts metrics (need the YAML `timeouts`/`retry_policies` to actually be *enforced* first); a distributed lease-contention counter (only meaningful once a multi-worker executor pool exists); and any synthetic judge-availability prober.