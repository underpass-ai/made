# made-console

`made-console` is a stateless operator CLI over `made-client`. Set
`MADE_ENDPOINT` or pass `--endpoint`.

Install the released CLI from crates.io:

```bash
cargo install made-console --locked
made-console --version
```

To test an unreleased checkout without changing the repository, install from
its path into a temporary root:

```bash
cargo install --path crates/made-console --locked --root ./tmp/made-console
./tmp/made-console/bin/made-console --help
```

The CLI connects to an already running MADE gRPC service. Point it at that
public endpoint before issuing reads or actions:

```bash
export MADE_ENDPOINT=http://127.0.0.1:50055
made-console get ceremony-123
```

```bash
made-console get ceremony-123
made-console tree ceremony-123 --max-nodes 100
made-console watch ceremony-123 --cursor-file ./ceremony-123.cursor --follow
made-console artifact list --limit 50
made-console artifact export artifact-123 ./report.bin
made-console budget report ceremony-123
made-console budget pending --limit 100
made-console report ceremony-123 --destination ./report.md
made-console pause ceremony-123 \
  --actor-id operator-7 --actor-kind human --reason maintenance
made-console resume ceremony-123 \
  --actor-id operator-7 --actor-kind human
made-console cancel ceremony-123 \
  --actor-id operator-7 --actor-kind human --reason superseded
made-console enforce-deadlines ceremony-123
made-console approve ceremony-123 \
  --guard human_approved --role-id reviewer --role-kind human
```

`watch` advances its checkpoint only after the server's end frame. It labels
`late_step_result_observed` records as late history. Artifact bytes are printed
only to an explicitly requested destination.

For a mutual-TLS endpoint, provide the client identity and trust root. MADE
maps the presented certificate fingerprint to the configured principal; the
console never sends principal metadata:

```bash
export MADE_ENDPOINT=https://made.example:50055
export MADE_TLS_CA_CERTIFICATE=./ca.pem
export MADE_TLS_CLIENT_CERTIFICATE=./operator-cert.pem
export MADE_TLS_CLIENT_KEY=./operator-key.pem
made-console budget report ceremony-123
```

Each invocation creates a random namespace and derives one
`x-made-request-id` from that namespace, the exact RPC method and the canonical
protobuf payload. Different actions and artifact chunks cannot collide; an
identical reconstruction retains the same id. Set `MADE_REQUEST_NAMESPACE` or
`--request-namespace` to reconstruct a caller-owned invocation after an
ambiguous response. Request ids are never identities or credentials.

`budget report` identifies bounded dimensions that are exhausted or overrun.
`budget pending` shows the reservations currently reducing availability and
their observed/estimated/unknown measurement quality.

`list` currently reflects the server's legacy unpaged RPC. It is an operator
checkpoint, not the accepted C5.8 search/pagination surface; the bounded
storage query and opaque public cursor are tracked in the C5.8 contract.
