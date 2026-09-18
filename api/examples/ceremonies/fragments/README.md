# Reusable ceremony fragments

[roundtable_fixed_order.yaml](roundtable_fixed_order.yaml) is the canonical
three-seat example for the fixed-order preset. The preset substitutes the
caller's participants and creates one sequential turn per participant in
declaration order. Later turns receive prior contributions; it performs no
automatic aggregation or dynamic speaker selection.

Omit `stages` or pass an empty list when selecting this preset. A nonempty
stage list is refused. `made-app` selects/materializes the preset;
`made-adapters` parses and renders YAML. Packaging copies in `made-app` and
`made-mcp` must match this file byte for byte, as checked by the contract gate.

See [authoring](../../../../docs/authoring/README.md) for draft analysis and
publication. The fragment is input to the shipped preset, not a promise of
all group-chat orchestration patterns.
