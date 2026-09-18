# Council output schemas

[report.schema.json](report.schema.json) is a reusable JSON Schema for a
structured report. It is an example supplied by the caller, not a hardcoded
MADE interpretation of report fields.

Supply its contents as `constraints.output_contract.json_schema` on the
council API. The gRPC `OutputContract` carries the schema string; MCP forwards
that same field on council operations. The JSON Schema validator compiles and
checks the supplied schema against proposal output.

Field-level `OutputFieldRule` constraints cover required fields and allowed
string values. JSON Schema can express nested structure, array bounds and
other shape rules. When both are configured, every active rule must pass.
A valid shape does not establish the truth of the report's evidence.

Council operations require the service distribution; check
[available surfaces](../../../docs/operations/support-matrix.md) before using
these examples from an embedded MCP host.
