# Reusable ceremony fragments

[roundtable_fixed_order.yaml](roundtable_fixed_order.yaml) is the canonical
three-seat example for the fixed-order preset. The preset substitutes the
caller's participants and creates one sequential turn per participant in
declaration order. Later turns receive prior contributions; it performs no
automatic aggregation or dynamic speaker selection.

The stage-pattern catalog also ships five complete, executable fragments:

- `broadcast_collect` fans the same brief to independent roles and collects
  every result in the following state.
- `group_chat` lets a manager choose a dynamic speaker, stops early on
  `done: true`, and reaches its fallback after the declared turn cap.
- `maker_checker` routes an accepted revision to delivery and the final
  rejected revision to fallback.
- `handoff` routes by `handoff_to`, includes a human-approved exit, and carries
  a bounce budget for its cyclic role graph.
- `magentic` promotes a mutable task ledger into context, binds work to the
  selected owner, and routes completed or stalled ledgers explicitly.

These files are concrete outputs of `stages[].pattern`, with `x-pattern`
annotations for diagrams and reports. The annotations are metadata; runtime
guards, transitions and caps carry the behavior.

Omit `stages` or pass an empty list when selecting this preset. A nonempty
stage list is refused. `made-app` selects/materializes the preset;
`made-adapters` parses and renders YAML. Packaging copies in `made-app` and
`made-mcp` must match every canonical fragment byte for byte, as checked by
the contract gate.

See [authoring](../../../../docs/authoring/README.md) for draft analysis and
publication. The fragment is input to the shipped preset, not a promise of
all group-chat orchestration patterns.
