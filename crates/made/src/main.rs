use anyhow::Result;

mod authorization_bootstrap;

#[tokio::main]
async fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("bootstrap-authorization") => {
            return authorization_bootstrap::run(arguments.collect()).await
        }
        Some("maintenance") => return made::maintenance::run(arguments.collect()).await,
        Some(unexpected) => anyhow::bail!("unknown command `{unexpected}`"),
        None => {}
    }
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
