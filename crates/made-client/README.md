# made-client

`made-client` is the reusable operator client for MADE's public gRPC API. It
does not open a MADE store or depend on service composition.

Add the released client to a Rust application with:

```bash
cargo add made-client
```

```rust,no_run
use made_client::{MadeClient, ProgressCheckpoint};

# async fn example() -> Result<(), made_client::MadeClientError> {
let client = MadeClient::connect("http://127.0.0.1:50055").await?;
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

The checkpoint file stores only a ceremony id and G6 sequence. The server
journal remains authoritative.
