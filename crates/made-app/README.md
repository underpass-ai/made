# made-app

The application layer of
[MADE by Underpass](https://github.com/underpass-ai/made): the use cases
that compose the domain.

Each use case is a struct holding its ports behind `Arc` and exposing one
`execute`. Services compose several use cases when a single operation
needs more than one — automatic dispatch, session facts, transcripts.

## The boundary is the dependency list

This crate depends on [`made-core`](https://crates.io/crates/made-core)
and nothing else of ours. No adapter types, no IO primitives, no
transport-shaped errors reach it, and that is enforced by what it is
allowed to import rather than by review discipline. A use case that
wanted to open a connection could not: it only has the traits.

That is what makes the same use cases run unchanged behind the gRPC
service, the stdio MCP adapter and the in-process embedded distribution.

## License

Apache-2.0.
