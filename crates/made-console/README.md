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

Actor and role fields are host assertions until C5.7 authenticates and
authorizes them. Budget is rendered as `unknown` until C5.3 publishes its
budget report contract.

`list` currently reflects the server's legacy unpaged RPC. It is an operator
checkpoint, not the accepted C5.8 search/pagination surface; the bounded
storage query and opaque public cursor are tracked in the C5.8 contract.
