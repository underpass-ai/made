# Release MADE

Release from reviewed, tested `main`. A release tag is immutable. The
[release helper](../../scripts/release.sh) owns version synchronization and
publication ordering; it does not replace the required checks.

The 0.6.0 version bump prepares a candidate. Its source plugin manifests point
at 0.6.0 assets, which cannot be downloaded before publication. Until both
the public asset set and the stable marketplace are ready, test the candidate
through a source build and an explicit `MADE_MCP_BIN`; see
[local setup](../embedded/README.md#test-a-source-candidate). A merged bump
does not authorize a tag push or advance the stable catalogue.

## Prepare

Choose the new version, then run:

```bash
make version VERSION=X.Y.Z
```

Review the resulting workspace/lockfile, chart, plugin manifests and immutable
marketplace source reference. Update [CHANGELOG](../../CHANGELOG.md), complete
migration guidance and run the [relevant gates](README.md), including service,
plugin and deployment checks for the released surfaces. Open a PR for the
version change and merge only after the required checks and review pass.

## Publish

From synchronized, clean `main` with the chosen version already merged:

```bash
make release VERSION=X.Y.Z
```

The helper checks branch/version/catalogue consistency, creates or resumes
the exact annotated tag, pushes it, waits for the complete checksummed plugin
asset set and fast-forwards `marketplace` to that released commit. It never
moves an existing tag to another commit. A failed publication is resumed for
that same identity rather than rewritten.

Tag workflows split responsibilities:

- [publish-distribution](../../.github/workflows/publish-distribution.yml)
  builds/publishes container images, Helm and public crates.
- [plugin-package](../../.github/workflows/plugin-package.yml) publishes
  platform executables, SHA-256 files and plugin bundles to GitHub Releases.

Verify both workflows for the exact tag. Confirm image digests, the versioned
OCI chart, checksums and all required platform assets before announcing the
release. Catalogue publication is co-located in this repository, named
`made`, and points to immutable released content. The stable branch must not
advertise a source commit whose required binary assets do not exist.

The public crate order is encoded in
[publish-crates.sh](../../scripts/ci/publish-crates.sh). It skips already
published versions so a partially completed run can resume; do not change
the tag or republish different bytes under an existing version.

`made-client` is published after `made-proto`, and `made-console` after the
client. Before a release, run the non-publishing native console candidate on
each supported host:

```bash
bash scripts/ci/package-made-console.sh
```

The command packages both crate manifests, builds the CLI, checks its version
and help surface, and produces a checksummed candidate under `dist/console/`.
The supported release matrix is Linux x86_64, Linux arm64, macOS arm64 and
Windows x86_64. Candidate output is evidence for review; it is not a release
asset until the tag-gated workflow publishes that exact commit.

## Recover

For an unpublished failure, repair the workflow or complete the existing
publication while preserving identity. For bad published behavior, cut a new
patch release. Operational rollback must consider event-schema/database
compatibility; a Helm revision does not restore store contents. See
[migrations](../migrations/README.md) and the
[Kubernetes guide](../operations/deploy-kubernetes.md).
