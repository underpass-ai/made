use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt};

pub(super) async fn drain<R: AsyncRead + Unpin>(
    mut reader: R,
    total: Arc<AtomicUsize>,
    limit: usize,
    overflow: tokio::sync::mpsc::UnboundedSender<()>,
) -> std::io::Result<()> {
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        if total
            .fetch_add(read, Ordering::Relaxed)
            .saturating_add(read)
            > limit
        {
            let _ = overflow.send(());
            return Ok(());
        }
    }
}
