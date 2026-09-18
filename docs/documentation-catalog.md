# Documentation catalogue

This documentation was rebuilt from a new structure on 2026-09-18. The active
entrypoint is [docs/index.md](index.md); it routes by the reader's task instead
of the project's development chronology.

## Authority and scope

| Material | Role |
|:--|:--|
| README and active guides | Current product/developer explanation, with unreleased changes labeled |
| Crate READMEs | Packaging entrypoints for each published crate boundary |
| Plugin skills and setup command | Executable host workflow instructions, rebuilt with the same scope/authority boundaries |
| Protobuf, AsyncAPI, YAML examples and schemas | Operational machine contracts, kept at their original paths |
| `docs/architecture/*.tsv` | Checked conformance, coverage, parity and numeric-conversion ledgers; unchanged |
| Support matrix marked table | Compiled test contract; exact table retained while surrounding prose was rebuilt |
| Golden report and store fixture provenance | Test output/provenance, retained unchanged rather than rewritten as product documentation |
| Helm `NOTES.txt` | Operational render template, retained unchanged |
| Experiment scripts and result files | Historical raw evidence and runnable inputs, retained unchanged |
| LICENSE files | Legal terms, retained unchanged |
| Historical prose snapshot | Audit material, never current instructions or proof of availability |

The previous root prose, crate/plugin READMEs, skill/command prose, complete
docs tree, fixture Markdown and Helm notes were copied into the
[historical snapshot](history/pre-rebuild-2026-09-18/INDEX.md). Its manifest
records path, byte length and SHA-256 against the source commit. Every prior
active prose page was replaced or retired; retained operational exceptions
are listed above. The old ADRs and roadmaps are historical; current contract
choices are explained in [architecture](architecture/README.md).

A small set of former paths now contains newly written compatibility
entrypoints for source comments and external links. They route to the active
guide and contain no copied historical narrative. Machine-consumed paths
were not moved, so no production source import needed modification.

The repository `.kmp` export was removed as requested. This change touches no
live memory store or another project's memory. It is not archived as product
documentation and must not be recreated by the documentation workflow.

## Wordmark

The [SVG](assets/made-wordmark.svg), [ASCII text](assets/made-wordmark.txt)
and [Unicode blocks](assets/made-wordmark-block.txt) share a small
[generator](assets/generate-wordmark.py). The mark uses a 5×7 pixel alphabet
with Spectrum-inspired colored stripes. It is used by the repository README;
it adds no protocol output or startup banner.

## Maintainer checks

Check active local links when moving pages. For an archived page, interpret
its links under the source topology described in the snapshot index; do not
edit archived bytes to repair old links. Changes to a compiled support table
or ledger need the corresponding source-contract checks. Keep release history
traceable through [CHANGELOG](../CHANGELOG.md) and avoid promoting old plans
into present-tense feature claims.
