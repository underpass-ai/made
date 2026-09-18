# Historical documentation snapshot

This directory preserves the prior documentation and supporting material
from commit `4dd5eed9bc447097eac842dbf4d7fd736432f195` before the fresh documentation tree was written on
2026-09-18. It is historical audit material: statements may be obsolete,
incorrect or plans that never shipped. Follow [current documentation](../../index.md)
for installation and behavior.

[manifest.json](manifest.json) records every original repository-relative
path, byte length and SHA-256. The files beneath this directory preserve that
relative topology and their original bytes. The snapshot includes old root
and crate/plugin prose, the complete previous docs tree, fixture Markdown
and Helm notes. It intentionally does not include the deleted repository
`.kmp` export, live stores, secrets or build outputs.

Relative links to another captured document can still be read within this
topology. Links to uncaptured code, assets, fixtures or repository-root paths
refer to the original source tree at
[the snapshot commit](https://github.com/underpass-ai/made/tree/4dd5eed).
Resolve those links there; do not alter a historical page to make its links
look current. Previously broken links and stale product names remain evidence
of what the documentation said.

Useful historical entrypoints:

- [README](README.md): the former repository overview.
- [Changelog](CHANGELOG.md): the complete original release record.
- [Documentation index](docs/index.md).
- [Architecture decisions](docs/adr/README.md).
- [Orchestration plan](docs/orchestration-patterns-plan.md).
- [Experiment narratives](docs/experiments/README.md).
