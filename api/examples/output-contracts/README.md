# Output contract JSON Schemas

Canonical schemas for use with `Constraints.output_contract.json_schema`
(proto field 4 of `OutputContract`). MADE's
`JsonSchemaValidator` adapter compiles whatever schema body the caller
supplies and validates every proposal output against it.

These examples are **not** hardcoded into the service — they are
suggestions consumers can copy verbatim, embed inline, fetch from a
schema registry, or replace with their own. MADE does
not interpret any of these field names.

## Files

| File                                         | Purpose                                                              |
|----------------------------------------------|----------------------------------------------------------------------|
| [`report.schema.json`](./report.schema.json) | Generic structured report: summary + timeline + findings + remediations + open risks + recommended actions + evidence references. Replaces the historical "human-handoff-report" / "incident analysis" needs without baking any product vocabulary into the core. |

## Wiring

In Rust:

```rust
let schema = std::fs::read_to_string("api/examples/output-contracts/report.schema.json")?;
let contract = OutputContract::new_with_schema(
    "report-v1",
    OutputFormat::JsonObject,
    BTreeMap::new(),
    schema,
)?;
let constraints = TaskConstraints::default().with_output_contract(contract);
```

Over gRPC (proto):

```protobuf
Constraints {
  output_contract: {
    contract_id: "report-v1"
    format: OUTPUT_FORMAT_JSON_OBJECT
    json_schema: "<the file's contents as a string>"
  }
}
```

The MCP adapter forwards the schema verbatim through
`made_deliberate` / `made_orchestrate` / `made_process_trigger_event`
when the caller includes `constraints.output_contract.json_schema` in
the tool arguments.

## When to use a JSON Schema vs. field-level rules

- **Field-level rules** (`OutputFieldRule` — `required` +
  `allowed_string_values`) — enough when you only need a flat
  "required + enum" shape. Cheaper than parsing a full schema.
- **JSON Schema** — when you need nested objects, arrays with
  `maxItems` / `minItems`, string `pattern` / `format`, number
  bounds, or `additionalProperties: false`. Subsumes "bounded event
  proposal shape" via the standard schema vocabulary.

Both surfaces can be set simultaneously on the same
`OutputContract`; the validators run together and a proposal must
satisfy every active rule.
