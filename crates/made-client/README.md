# made-client

`made-client` is the reusable operator client for MADE's public gRPC API. It
does not open a MADE store or depend on service composition.

Add the released client to a Rust application with:

```bash
cargo add made-client
```

```rust,no_run
use made_client::{ClientConfig, MadeClient, ProgressCheckpoint};

# async fn example() -> Result<(), made_client::MadeClientError> {
let client = MadeClient::connect_with_config(
    ClientConfig::new("http://127.0.0.1:50055"),
).await?;
let state = client.get_ceremony("incident-review").await?;
let progress = client
    .watch_once(
        &ProgressCheckpoint::new(state.ceremony_id, 0),
        200,
        Some(1_000),
    )
    .await?;
println!("resume at {}", progress.checkpoint().after_sequence());
# Ok(())
# }
```

Read operations may be repeated after transport failure. Mutating methods do
not retry an ambiguous response. Artifact exports verify every chunk and the
final digest and size before installing the destination file.

By default each public client call creates a fresh `RequestContext`. For each
request the client derives `x-made-request-id` from that context, the exact RPC
method and the canonical protobuf payload. This separates repeated calls,
different actions and chunks. To reconstruct one logical invocation after an
ambiguous response, create a `RequestContext`, bind it with
`ClientConfig::with_request_context`, and use that client only for the
invocation and its retries. HTTPS uses system roots; private deployments can
add a PEM CA, an mTLS identity and a TLS domain override through the matching
builders. The server derives the principal from the certificate fingerprint,
never from request metadata.

The checkpoint file stores only a ceremony id and G6 sequence. The server
journal remains authoritative.
