use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Keep the guard alive through the process lifetime. Dropping it
    // on shutdown flushes the OTLP exporter (under the `otel`
    // feature) so no in-flight spans are lost.
    let _telemetry = made::init_tracing()?;

    tracing::info!(
        service = "made",
        version = env!("CARGO_PKG_VERSION"),
        "starting"
    );

    let app = made::compose().await?;
    made::serve(app).await
}
