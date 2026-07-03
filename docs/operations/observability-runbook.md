# Observability runbook — wiring traces, metrics and logs in Kubernetes

*Status: verified end-to-end on 2026-07-03 against a kube-prometheus-stack
(Prometheus + Grafana), Grafana Tempo and Loki/promtail installation.*

This is the operational companion to
[choreographer-observability-design.md](../choreographer-observability-design.md)
(what is instrumented and why). This document covers **how to turn each signal
on, how to prove it is flowing, and what to check when it is not**. Auditable
decision traces are the product's core promise — a Choreographer running
without trace export is running with its main feature dark.

## TL;DR

| Signal | How it ships | One-line enable | Proof it works |
|---|---|---|---|
| Traces (deliberations, ceremonies, judge verdicts) | OTLP/gRPC, `otel` feature, dormant without endpoint | `CHOREO_OTLP_ENDPOINT` in `providerEnv` | `"otlp exporter wired"` in the pod log at boot |
| Metrics | Prometheus `/metrics` on the HTTP port (8080) | ServiceMonitor (manifest below) | `choreo_*` series in Prometheus |
| Logs | JSON to stdout | nothing (any log shipper: promtail, fluent-bit…) | `{namespace="<ns>", pod=~"choreographer.*"}` in Loki |

## 1. Traces (OTLP)

### Wire it

The image ships the `otel` feature but it stays **dormant** until
`CHOREO_OTLP_ENDPOINT` is set (see `crates/choreo/src/telemetry.rs`: no
endpoint → JSON logs only, no background exporter). Add the endpoint to the
chart's `providerEnv`:

```yaml
# values override
providerEnv:
  - name: CHOREO_OTLP_ENDPOINT
    value: "http://tempo.monitoring.svc:4317"   # any OTLP/gRPC receiver
```

Plain-text OTLP as above suits a lab/POC. For an mTLS collector (the
Underpass posture), add the TLS material — `values.underpass-runtime.yaml`
carries a worked example:

```yaml
  - name: CHOREO_OTLP_TLS_CA_PATH
    value: "/etc/choreographer/runtime-tls/ca.crt"
  - name: CHOREO_OTLP_TLS_CERT_PATH
    value: "/etc/choreographer/runtime-tls/tls.crt"
  - name: CHOREO_OTLP_TLS_KEY_PATH
    value: "/etc/choreographer/runtime-tls/tls.key"
  - name: CHOREO_OTLP_TLS_DOMAIN_NAME    # SNI override when the server cert
    value: "underpass-runtime"           # SAN is not the Service name
```

### Prove it

1. **At boot** the pod logs exactly one of these:
   - `"otlp exporter wired"` with the endpoint and `mtls: true|false` — good.
   - Nothing about otel — the endpoint variable is missing; the feature is
     dormant (the chart's post-install NOTES banner shouts about this case).
2. **Run anything** (a `hello`-style noop ceremony is enough, no models
   needed) and search the trace backend. Verified span inventory for one
   `RunCeremony` with two steps:

   ```
   rpc.run_ceremony                    (root, service underpass-choreographer)
   └── run_ceremony
       ├── prepare_ceremony_participants
       ├── deliberate                  (one per deliberating step;
       └── deliberate                   per-proposal + judge-verdict span events)
   ```

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
  `CHOREO_OTLP_TLS_DOMAIN_NAME` to the name in the collector's server cert.

## 2. Metrics (Prometheus)

The HTTP port (`service.httpPort`, default 8080) serves `GET /metrics` — the
catalogue and label sets are documented in the
[observability design](../choreographer-observability-design.md). The chart
does not ship a ServiceMonitor; with the Prometheus Operator
(kube-prometheus-stack) this manifest is all it takes:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: choreographer
  namespace: <release namespace>
  labels:
    release: kube-prometheus-stack   # match your Prometheus' selector, if any
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: choreographer
  endpoints:
    - port: http
      path: /metrics
      interval: 30s
```

Prove it: the target appears `up` in Prometheus (`Status → Targets`) and
`choreo_deliberations_total` returns series. The five signals worth a
dashboard first: winner-score distribution, `NoValidProposal` rate, judge
latency/error class, provider token usage, ceremony step durations.

## 3. Logs

Everything is single-line JSON on stdout (`tracing_subscriber` fmt layer) —
any node-level shipper picks it up with zero configuration. With
Loki/promtail, the ceremony timeline is queryable as:

```logql
{namespace="<ns>", pod=~"choreographer.*"} |= "ceremony step deliberation completed"
```

Useful stable message keys: `ceremony participant prepared`,
`proposal drafted`, `validator verdict`, `proposal scored`,
`deliberation completed`, `ceremony step deliberation completed` — each
carries `ceremony_id`/`step_id`/`specialty` fields for filtering.

## 4. Order of operations for a new install

1. Deploy with `CHOREO_OTLP_ENDPOINT` from day one (the NOTES banner reminds
   you if you forget).
2. Check the boot log for `"otlp exporter wired"`.
3. Apply the ServiceMonitor; confirm the target is `up`.
4. Run a noop smoke ceremony; confirm the `rpc.run_ceremony` trace and the
   log lines land in your backends.
5. Only then point real providers/models at it — from here on, every
   deliberation you rely on has an auditable trace.
