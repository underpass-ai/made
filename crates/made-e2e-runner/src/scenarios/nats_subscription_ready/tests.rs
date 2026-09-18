use std::time::Duration;

use async_nats::Client;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use super::{wait_for_subscription_ready, MARKER};

const TARGET: &str = "made.task.failed";

/// A controlled protocol peer: tests decide when SUB is processed and MSG sent.
struct ProtocolPeer {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

impl ProtocolPeer {
    async fn connect() -> (Client, Self) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let connecting = async_nats::connect(format!("nats://{address}"));
        let accepting = async {
            let (socket, _) = listener.accept().await.unwrap();
            let (reader, writer) = socket.into_split();
            let mut peer = Self {
                reader: BufReader::new(reader),
                writer,
            };
            peer.writer
                .write_all(b"INFO {\"server_id\":\"readiness-test\",\"version\":\"2.15.0\",\"proto\":1,\"host\":\"127.0.0.1\",\"port\":4222,\"headers\":true,\"max_payload\":1048576}\r\n")
                .await
                .unwrap();
            assert!(peer.read_line().await.starts_with("CONNECT "));
            assert_eq!(peer.read_line().await, "PING");
            peer.writer.write_all(b"PONG\r\n").await.unwrap();
            peer
        };
        let (client, peer) = tokio::join!(connecting, accepting);
        (client.unwrap(), peer)
    }

    async fn read_line(&mut self) -> String {
        let mut line = String::new();
        assert_ne!(self.reader.read_line(&mut line).await.unwrap(), 0);
        line.trim_end().to_owned()
    }

    async fn command(&mut self) -> String {
        loop {
            let line = self.read_line().await;
            if line != "PING" {
                return line;
            }
            self.writer.write_all(b"PONG\r\n").await.unwrap();
        }
    }

    async fn subscription(&mut self) -> (String, String) {
        let line = self.command().await;
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 3, "unexpected SUB command: {line}");
        assert_eq!(fields[0], "SUB");
        (fields[1].to_owned(), fields[2].to_owned())
    }

    async fn publication(&mut self) -> (String, Vec<u8>) {
        let line = self.command().await;
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 3, "unexpected PUB command: {line}");
        assert_eq!(fields[0], "PUB");
        let length: usize = fields[2].parse().unwrap();
        let mut payload = vec![0; length + 2];
        self.reader.read_exact(&mut payload).await.unwrap();
        assert_eq!(&payload[length..], b"\r\n");
        payload.truncate(length);
        (fields[1].to_owned(), payload)
    }

    async fn deliver(&mut self, subject: &str, sid: &str, payload: &[u8]) {
        self.writer
            .write_all(format!("MSG {subject} {sid} {}\r\n", payload.len()).as_bytes())
            .await
            .unwrap();
        self.writer.write_all(payload).await.unwrap();
        self.writer.write_all(b"\r\n").await.unwrap();
    }
}

#[tokio::test]
async fn flush_finishes_before_server_processes_subscription() {
    tokio::time::timeout(Duration::from_secs(3), async {
        let (client, mut peer) = ProtocolPeer::connect().await;
        let (release, gate) = oneshot::channel();
        let (registered, mut ready) = oneshot::channel();
        let server = tokio::spawn(async move {
            gate.await.unwrap();
            assert_eq!(peer.subscription().await.0, TARGET);
            registered.send(()).unwrap();
        });

        let _subscription = client.subscribe(TARGET).await.unwrap();
        client.flush().await.unwrap();
        assert!(matches!(
            ready.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));

        release.send(()).unwrap();
        ready.await.unwrap();
        server.await.unwrap();
    })
    .await
    .expect("controlled flush demonstration stalled");
}

#[tokio::test]
async fn readiness_waits_for_marker_receipt_after_target_subscription() {
    tokio::time::timeout(Duration::from_secs(3), async {
        let (client, mut peer) = ProtocolPeer::connect().await;
        let _subscription = client.subscribe(TARGET).await.unwrap();
        let mut readiness = Box::pin(wait_for_subscription_ready(&client));
        let observed = async {
            assert_eq!(peer.subscription().await.0, TARGET);
            let (inbox, sid) = peer.subscription().await;
            assert!(inbox.starts_with("_INBOX."));
            assert_ne!(inbox, TARGET);
            let (subject, payload) = peer.publication().await;
            assert_eq!(subject, inbox);
            assert_eq!(payload, MARKER);
            (inbox, sid, payload)
        };
        let (inbox, sid, payload) = tokio::select! {
            result = &mut readiness => panic!("ready before server response: {result:?}"),
            observed = observed => observed,
        };

        // Actively poll while the server withholds MSG: returning after flush
        // or merely publishing the marker must fail this assertion.
        assert!(futures::poll!(readiness.as_mut()).is_pending());
        peer.deliver(&inbox, &sid, &payload).await;
        readiness.await.unwrap();
    })
    .await
    .expect("controlled readiness round trip stalled");
}
