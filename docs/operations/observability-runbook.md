# Observability runbook — wiring traces, metrics and logs in Kubernetes

*Status: verified end-to-end on 2026-07-03 against a kube-prometheus-stack
(Prometheus + Grafana), Grafana Tempo and Loki/promtail installation; claims
re-checked against the code on 2026-09-18.*

This is the operational companion to
[made-observability-design.md](../made-observability-design.md)
(what is instrumented and why). This document covers **how to turn each signal
on, how to prove it is flowing, and what to check when it is not**. Auditable
decision traces are the product's core promise — a MADE instance running
without trace export is running with its main feature dark.

What the code does not do yet is in §5, not in the instructions.

## TL;DR

| Signal | How it ships | One-line enable | Proof it works |
|---|---|---|---|
| Traces (the RPC, the ceremony run, each deliberation and its verdicts) | OTLP/gRPC, `otel` feature, dormant without endpoint | `MADE_OTLP_ENDPOINT` in `providerEnv` | `"otlp exporter wired"` in the pod log at boot |
| Metrics | Prometheus `/metrics` on the HTTP port (8080) | ServiceMonitor (manifest below) | `made_*` series in Prometheus |
| Logs | JSON to stdout | nothing (any log shipper: promtail, fluent-bit…) | `{namespace="<ns>", pod=~"MADE.*"}` in Loki |

The table is the deployable edition. The embedded edition has no Prometheus
scrape endpoint; §2.1 covers its MCP registry read, optional OTLP exporter and
JSON-lines sink.

## 1. Traces (OTLP)

### Wire it

The image ships the `otel` feature (`Dockerfile`, `--features made/otel`) but
it stays **dormant** until `MADE_OTLP_ENDPOINT` is set (see
`crates/made/src/telemetry.rs`: no endpoint → JSON logs only, no background
exporter). Add the endpoint to the chart's `providerEnv`:

```yaml
# values override
providerEnv:
  - name: MADE_OTLP_ENDPOINT
    value: "http://tempo.monitoring.svc:4317"   # any OTLP/gRPC receiver
```

Plain-text OTLP as above suits a lab/POC. For an mTLS collector (the
Underpass posture), add the TLS material — `values.underpass-runtime.yaml`
carries a worked example:

```yaml
  - name: MADE_OTLP_TLS_CA_PATH
    value: "/etc/made/runtime-tls/ca.crt"
  - name: MADE_OTLP_TLS_CERT_PATH
    value: "/etc/made/runtime-tls/tls.crt"
  - name: MADE_OTLP_TLS_KEY_PATH
    value: "/etc/made/runtime-tls/tls.key"
  - name: MADE_OTLP_TLS_DOMAIN_NAME    # SNI override when the server cert
    value: "underpass-runtime"           # SAN is not the Service name
```

### Prove it

1. **At boot** the pod logs exactly one of these:
   - `"otlp exporter wired"` with the endpoint and `mtls: true|false` — good.
   - Nothing about otel — the endpoint variable is missing; the feature is
     dormant (the chart's post-install NOTES banner shouts about this case).
2. **Run anything** (a `hello`-style noop ceremony is enough, no models
   needed) and search the trace backend. Verified span inventory for one
   `RunCeremony` with two deliberating steps:

   ```
   rpc.run_ceremony                      (root, service made)
   ├── prepare_ceremony_participants     (sibling of run_ceremony: the RPC
   │                                      handler seats the council before
   │                                      it drives the session)
   └── run_ceremony
       └── ceremony_step                 (one per semantic step execution)
           └── ceremony_step_handler     (the selected handler boundary)
               └── deliberate            (for a deliberating handler;
                   ├── provider_call       provider/model/error/tokens)
                   └── judge_call          quality or evidence-support judge)
   ```

   An incrementally driven step starts at `run_ceremony_step` and has the same
   `ceremony_step_handler` child. Provider and judge token fields are absent
   when upstream omitted usage; reported zero remains zero. On failure,
   `error_kind` is a stable low-cardinality label rather than error text.

   Tempo example:

   ```bash
   curl -sG http://tempo.monitoring.svc:3200/api/search \
     --data-urlencode "start=$(date -d '10 min ago' +%s)" \
     --data-urlencode "end=$(date +%s)" | jq '.traces[].rootTraceName'
   # → "rpc.run_ceremony"
   ```

### When it is not flowing

- **No `"otlp exporter wired"` log line** → the env var never reached the
  container. `kubectl get deploy <release> -o jsonpath='{.spec.template.spec.containers[0].env}'`
  and check `providerEnv` in your values (a `providerEnvFrom` Secret works
  too, but the chart cannot introspect it — you own the check).
- **Exporter wired, no traces in the backend** → egress. If
  `networkPolicy.enabled=true`, the built-in OTLP egress rule only matches
  pods labelled `app.kubernetes.io/name: otel-collector` on port 4317; a
  direct-to-Tempo (or any other collector name/namespace) export needs a rule
  under `networkPolicy.egress.extra`.
- **TLS handshake errors in the pod log** → SAN mismatch: set
  `MADE_OTLP_TLS_DOMAIN_NAME` to the name in the collector's server cert.

## 2. Metrics (Prometheus)

The HTTP port (`service.httpPort`, default 8080) serves `GET /metrics` — the
catalogue and label sets are documented in the
[observability design](../made-observability-design.md). The chart
does not ship a ServiceMonitor; with the Prometheus Operator
(kube-prometheus-stack) this manifest is all it takes:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: MADE
  namespace: <release namespace>
  labels:
    release: kube-prometheus-stack   # match your Prometheus' selector, if any
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: MADE
  endpoints:
    - port: http
      path: /metrics
      interval: 30s
```

Prove it: the target appears `up` in Prometheus (`Status → Targets`) and
`made_deliberations_total` returns series. The five signals worth a
dashboard first: winner-score distribution, `NoValidProposal` rate, judge
latency/error class, provider token usage, ceremony step durations.

Two shapes of this endpoint are worth knowing before you alert on it:

- `made_service_ready` reads the **NATS connection state only**. Readiness is
  `GET /readyz`, which checks NATS *and* Postgres; alert on the probe.
- The five ceremony families move for a `RunCeremony` run and for nothing
  else. A host driving steps one at a time leaves them at zero, correctly.

### 2.1 The embedded edition

There is no scrape socket, and the same registry is available through MCP.

`made-mcp` on the embedded backend, and any host holding `EmbeddedMade`, wires
a `PrometheusMetricsRecorder` with its own registry **inside the process**
(`EmbeddedMadeBuilder`, since #53) when the host injects none. What exists
there today, exactly:

- **An in-process registry.** It records the five ceremony families of a
  `RunCeremony` run, like the deployable edition does.
- **No Prometheus endpoint.** There is no `/metrics` socket. The registry is
  rendered by `made_get_metrics` instead.
- **The two MCP tools, on both backends:**

  ```json
  {"name":"made_get_status","arguments":{"include_stats":true}}
  {"name":"made_get_metrics","arguments":{}}
  ```

  `made_get_status` answers with the version of the engine that answered, how
  long *that* engine has been up, its condition and — when asked — the
  counters. `made_get_metrics` keeps those counters in `stats` and adds
  `registry_text` plus a typed `registry` array of families and samples. Both
  tools are composed by `GetServiceStatusUseCase` and
  `GetServiceMetricsUseCase`, so the two editions use the same boundary.
- **Honest zeros and live ceremony metrics.** The `stats` counters are `Statistics`:
  deliberations and orchestrations. They are council work, which an embedded
  engine running no council never performs, so it reports them at zero however
  many sessions it runs. Ceremony counters and histograms move in the registry
  on the same answer.
- **The recorder's name off the wire.** `EmbeddedMade::status` names which
  recorder is running, which is how a Rust host can tell whether its own
  injection took; neither MCP arm renders it, because `GetStatusResponse`
  carries four fields and giving one arm a fifth is the divergence ADR-014
  exists to prevent.

A Rust host that supplies its own recorder should call
`EmbeddedMadeBuilder::with_observability` so reads and writes share one `Arc`.
The lower-level recorder and snapshot setters exist for specialized
compositions; wiring different objects makes the read describe a different
registry from the one being recorded.

Build `made-mcp` with `--features otel` to enable the shared OTLP/gRPC
initializer. It is dormant without `MADE_OTLP_ENDPOINT` and accepts the same
`MADE_OTLP_TLS_CA_PATH`, `MADE_OTLP_TLS_CERT_PATH`,
`MADE_OTLP_TLS_KEY_PATH`, and `MADE_OTLP_TLS_DOMAIN_NAME` variables as `made`.

Set `MADE_MCP_EVENT_SINK_PATH` on the embedded backend to append JSON lines to
a host-chosen file. Each delivered ceremony event is immediately followed by
a `metrics_snapshot` line with `registry_text` and `registry`. The pair is
written under one lock and flushed once. A failed append leaves the durable
publisher cursor unacknowledged, so restart recovery retries the event. The
snapshot is the current process registry after event fanout; it is not an
exact-prefix promise for the event stream.

## 3. Logs

Everything is single-line JSON on stdout (`tracing_subscriber` fmt layer) —
any node-level shipper picks it up with zero configuration. With
Loki/promtail, the ceremony timeline is queryable as:

```logql
{namespace="<ns>", pod=~"MADE.*"} |= "ceremony step deliberation completed"
```

Stable message keys, with the fields each one actually carries:

| Message | Fields |
|---|---|
| `ceremony step deliberation completed` | `ceremony_id`, `step_id`, `specialty`, `winner_proposal_id` |
| `ceremony participant prepared` | `specialty`, `kind` |
| `proposal drafted` | `proposal_id`, `author`, `content_len`, `preview` |
| `peer critique and revision` | `round`, `reviewer`, `target_proposal`, `revised_preview` |
| `validator verdict` | `proposal_id`, `validator`, `passed`, `verdict` |
| `proposal scored` | `proposal_id`, `score` |
| `deliberation completed` | `task_id`, `specialty`, `winner_id`, `winner_score`, `duration_ms` |

Only the first carries `ceremony_id` **and** `step_id` **and** `specialty`, so
it is the only one you can filter a single step's work by. The rest are joined
through the trace, not through a label: they are emitted under the
`deliberate` span of the step that produced them.

`made-mcp`'s default filter is
`made_mcp=info,made_app=info,made_adapters::sqlite=info`, so embedded
application events are present without an operator override. Set `RUST_LOG`
to replace that default when a different scope or level is required.

## 4. Order of operations for a new install

1. Deploy with `MADE_OTLP_ENDPOINT` from day one (the NOTES banner reminds
   you if you forget).
2. Check the boot log for `"otlp exporter wired"`.
3. Apply the ServiceMonitor; confirm the target is `up`.
4. Run a noop smoke ceremony; confirm the `rpc.run_ceremony` trace and the
   log lines land in your backends.
5. Only then point real providers/models at it — from here on, every
   deliberation you rely on has an auditable trace.

## 5. Planned — not implemented

The remaining items are not wired today in any edition. Named here with the slice of
[`orchestration-patterns-plan.md`](../orchestration-patterns-plan.md) §3.7
that owns it, so that nothing above has to be written in the future tense.

| What an operator would get | Slice |
|---|---|
| Ceremony progress as a live stream — `StreamCeremony` on the cluster, a pull cursor on the embedded edition | G6 |
