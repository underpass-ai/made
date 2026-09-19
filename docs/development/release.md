# Release MADE

Release from reviewed, tested `main`. A release tag is immutable. The
[release helper](../../scripts/release.sh) owns version synchronization and
publication ordering; it does not replace the required checks.

The version bump prepares a candidate. Its source plugin manifests point at
assets which cannot be downloaded before publication. Until the public asset
set is ready, test the candidate through a source build and an explicit
`MADE_MCP_BIN`; see
[local setup](../embedded/README.md#test-a-source-candidate). A merged bump
does not authorize a tag push or advance the stable catalogue.

## Prepare

Choose the new version, then run:

```bash
make version VERSION=SEMVER
```

Review the resulting workspace/lockfile, chart, plugin manifests and immutable
marketplace source reference. Update [CHANGELOG](../../CHANGELOG.md), complete
migration guidance and run the [relevant gates](README.md), including service,
plugin and deployment checks for the released surfaces. Open a PR for the
version change and merge only after the required checks and review pass.

## Publish

From synchronized, clean `main` with the chosen version already merged:

```bash
make release VERSION=SEMVER
```

`SEMVER` may be a stable version such as `0.7.0` or a prerelease such as
`0.7.0-rc.1`; build metadata is not accepted. The helper checks the workspace,
lockfile, chart, plugin manifests, internal dependency pins and catalogue as
one release identity before creating or resuming the exact annotated tag. It
then waits until the checksummed plugin assets, all public crates, all four
container tags and the OCI chart are externally visible. Stable releases
fast-forward `marketplace` only after that proof. Prereleases remain GitHub
prereleases and never advance the stable marketplace branch.

The public verification step uses `gh`, `curl`, Docker Buildx and Helm. It
needs the normal GitHub authentication used to create a release, but it does
not print credentials. Missing tools or inaccessible public registries fail
before any marketplace update.

Tag workflows split responsibilities:

- [publish-distribution](../../.github/workflows/publish-distribution.yml)
  promotes the exact `sha-<commit>` image digests that passed the full Compose
  smoke, then publishes Helm and public crates. A release tag never rebuilds
  image bytes. Development `main` images keep their separate build path.
- [plugin-package](../../.github/workflows/plugin-package.yml) publishes
  platform executables, SHA-256 files and plugin bundles to GitHub Releases.

Verify both workflows for the exact tag. Confirm image digests, the versioned
OCI chart, checksums and all required platform assets before announcing the
release. Catalogue publication is co-located in this repository, named
`made`, and points to immutable released content. The stable branch must not
advertise a source commit whose required binary assets do not exist.

Release assets are immutable. A resumed plugin upload accepts an existing
asset only when its bytes match, refuses unexpected assets and never uses
`--clobber`. An existing chart version is unpacked and compared before it is
accepted. An existing image tag must already resolve to the verified commit
digest.

The public crate order is encoded in
[publish-crates.sh](../../scripts/ci/publish-crates.sh). It skips already
published versions so a partially completed run can resume, and waits for
each exact version to appear in the crates.io sparse index before publishing
the dependent crate. Do not change the tag or republish different bytes under
an existing version.

`made-client` is published after `made-proto`, and `made-console` after the
client. Before a release, run the non-publishing native console candidate on
each supported host:

```bash
bash scripts/ci/package-made-console.sh
```

The command packages both crate manifests, builds the CLI, checks its version
and help surface, and produces a checksummed candidate under `dist/console/`.
The supported release matrix is Linux x86_64, Linux arm64, macOS arm64 and
Windows x86_64. These console candidates remain workflow artifacts for review;
the GitHub Release asset inventory contains the plugin bundles, standalone
`made-mcp` binaries and their checksums. The console is distributed through
its published crate.

## Recover

For an unpublished failure, repair the workflow or complete the existing
publication while preserving identity. For bad published behavior, cut a new
patch release. Operational rollback must consider event-schema/database
compatibility; a Helm revision does not restore store contents. See
[migrations](../migrations/README.md) and the
[Kubernetes guide](../operations/deploy-kubernetes.md).
