# Ceremony fragments

These files are reusable definition inputs for shipped authoring presets. MADE
embeds them in `made-app`; the application layer selects and materializes a
preset, while YAML parsing and rendering stay in `made-adapters`.
The canonical files live here. Packaging copies under `made-app` and
`made-mcp` are compared byte for byte by the contract gate because published
crates cannot include files outside their own package.

`roundtable_fixed_order.yaml` is the concrete three-seat example for the
`roundtable_fixed_order` preset. The preset accepts the caller's participants
instead: it creates one sequential turn per participant in declaration order.
The request omits `stages` or supplies an empty list, as repeated proto fields
cannot distinguish those two forms; any non-empty stage list is refused.
The first turn has no prior contribution; every later turn receives all prior
contributions. It performs no aggregation and does not implement the dynamic
speaker selection of the full D1 group-chat pattern.
