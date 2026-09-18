# Store written by the published v0.6.0 binary

The 90,112-byte `ceremonies.sqlite3` was produced over MCP stdio by the
published Linux arm64 `made-mcp 0.6.0` asset. It contains only generated test
data. `manifest.json` records its checksum, the writer checksum, all sealed
records and the three instance projections returned by that binary.
`requests-responses.json` preserves the calls and responses.

The writer SHA-256 is
`905410b881851ea1e9190694e6ff5f8ed99067d872f902f725f6ea104b6f896d`;
the store SHA-256 is
`ab18617e6f86c8f281b886c4b69354a761f1ef038ea95c11f0bb05f538a19f2c`.
Source: [published v0.6.0 release](https://github.com/underpass-ai/made/releases/tag/v0.6.0).

The sessions cover a completed ceremony (5 records), pending work (1 record),
and an accepted unfinished claim (2 records). The claim has its original
wall-clock lease; tests must not assume that historical lease is still live.

`produce.py --binary <published-arm64-binary> --output <empty-directory>`
repeats the scenario and verifies the published writer checksum. Identifiers
and timestamps created by the binary mean a new run has different sealed
hashes. Never overwrite this historical fixture during a test.

`tests/upgrade_v060.rs` copies the store into disposable workspace scratch,
compares complete records to the manifest, verifies hashes and full replay,
reopens the store and continues pending work. A completed session remains
terminal and refused commands leave its journal unchanged. The fixture does
not prove that an old binary can read new event schemas; that downgrade case
requires a separate old-reader/new-writer check.
