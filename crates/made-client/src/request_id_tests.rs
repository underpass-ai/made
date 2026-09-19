use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use made_proto::v1::{GetBudgetReportRequest, GetBudgetReportResponse};
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::codegen::{http, Body, BoxFuture, Service, StdError};
use tonic::server::NamedService;
use tonic::transport::Server;

use crate::{ClientConfig, MadeClient, RequestContext};

#[derive(Clone, Debug)]
struct CaptureService {
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

impl<B> Service<http::Request<B>> for CaptureService
where
    B: Body + Send + 'static,
    B::Error: Into<StdError> + Send + 'static,
{
    type Response = http::Response<tonic::body::BoxBody>;
    type Error = Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        if request.uri().path() != "/underpass.made.v1.MadeService/GetBudgetReport" {
            return Box::pin(async move { Ok(unimplemented_response()) });
        }

        let captured = Arc::clone(&self.requests);
        let future = async move {
            let method = CaptureBudgetRequest { requests: captured };
            let codec = tonic::codec::ProstCodec::default();
            let response = tonic::server::Grpc::new(codec).unary(method, request).await;
            Ok(response)
        };
        Box::pin(future)
    }
}

impl NamedService for CaptureService {
    const NAME: &'static str = "underpass.made.v1.MadeService";
}

struct CaptureBudgetRequest {
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

impl tonic::server::UnaryService<GetBudgetReportRequest> for CaptureBudgetRequest {
    type Response = GetBudgetReportResponse;
    type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;

    fn call(&mut self, request: tonic::Request<GetBudgetReportRequest>) -> Self::Future {
        let requests = Arc::clone(&self.requests);
        Box::pin(async move {
            let request_id = request
                .metadata()
                .get("x-made-request-id")
                .expect("client must send x-made-request-id")
                .to_str()
                .expect("request id must be printable")
                .to_owned();
            let ceremony_id = request.into_inner().ceremony_id;
            requests.lock().unwrap().push((ceremony_id, request_id));
            Ok(tonic::Response::new(GetBudgetReportResponse::default()))
        })
    }
}

#[tokio::test]
async fn default_calls_are_fresh_and_explicit_retry_context_is_stable() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let service = CaptureService {
        requests: Arc::clone(&requests),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(
        Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = shutdown_rx.await;
            }),
    );

    let endpoint = format!("http://{address}");
    let client = MadeClient::connect(endpoint.clone()).await.unwrap();
    client.get_budget_report("ceremony-a").await.unwrap();
    client.get_budget_report("ceremony-a").await.unwrap();

    let context = RequestContext::from_id("invocation-7").unwrap();
    let retry_client =
        MadeClient::connect_with_config(ClientConfig::new(endpoint).with_request_context(context))
            .await
            .unwrap();
    retry_client.get_budget_report("ceremony-a").await.unwrap();
    retry_client.get_budget_report("ceremony-b").await.unwrap();
    retry_client.get_budget_report("ceremony-a").await.unwrap();

    let captured = requests.lock().unwrap().clone();
    assert_eq!(captured.len(), 5);
    assert_eq!(captured[0].0, "ceremony-a");
    assert_eq!(captured[1].0, "ceremony-a");
    assert_eq!(captured[2].0, "ceremony-a");
    assert_eq!(captured[3].0, "ceremony-b");
    assert_eq!(captured[4].0, "ceremony-a");
    assert_ne!(captured[0].1, captured[1].1);
    assert_ne!(captured[2].1, captured[3].1);
    assert_eq!(captured[2].1, captured[4].1);
    assert!(captured.iter().all(|(_, id)| id.len() <= 256));

    shutdown_tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}

fn unimplemented_response() -> http::Response<tonic::body::BoxBody> {
    http::Response::builder()
        .status(200)
        .header("grpc-status", "12")
        .header("content-type", "application/grpc")
        .body(tonic::body::empty_body())
        .unwrap()
}
