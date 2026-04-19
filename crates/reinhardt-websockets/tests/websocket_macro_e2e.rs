//! E2E tests for the WebSocket URL resolver + real connection integration.
//!
//! Verifies the full chain without importing the proc-macro crate directly
//! (which would create a circular dev-dependency):
//!
//!   WebSocketEndpointInfo impl → WebSocketRouter::consumer()
//!   → reverse() URL resolution → real WebSocket connection via tokio_tungstenite

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use reinhardt_websockets::{
    WebSocketEndpointInfo, WebSocketRouter,
    connection::Message,
    consumers::{ConsumerContext, WebSocketConsumer},
};
use rstest::{fixture, rstest};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message as TMsg};

// Convenience alias for test return types (consumer API uses the public alias
// re-exported from connection; but tests use Result directly for simplicity).
type TestResult<T = ()> = std::result::Result<T, reinhardt_websockets::connection::WebSocketError>;

// ── Test consumers (equivalent to what #[websocket] generates) ────────────

/// Echo consumer: sends "echo: <text>" back for every text message.
/// Parallel to `#[websocket("/ws/echo/", name = "echo_ws")]`.
struct EchoWsConsumer;

impl WebSocketEndpointInfo for EchoWsConsumer {
    fn path() -> &'static str { "/ws/echo/" }
    fn name() -> Option<&'static str> { Some("echo_ws") }
}

#[async_trait]
impl WebSocketConsumer for EchoWsConsumer {
    async fn on_connect(&self, _ctx: &mut ConsumerContext) -> TestResult { Ok(()) }

    async fn on_message(&self, context: &mut ConsumerContext, message: Message) -> TestResult {
        if let Message::Text { data } = message {
            context
                .connection
                .send_text(format!("echo: {}", data))
                .await?;
        }
        Ok(())
    }

    async fn on_disconnect(&self, _ctx: &mut ConsumerContext) -> TestResult { Ok(()) }
}

/// Parameterised consumer: path contains `{session_id}` placeholder.
/// Parallel to `#[websocket("/ws/session/{session_id}/", name = "session_ws")]`.
struct SessionWsConsumer;

impl WebSocketEndpointInfo for SessionWsConsumer {
    fn path() -> &'static str { "/ws/session/{session_id}/" }
    fn name() -> Option<&'static str> { Some("session_ws") }
}

#[async_trait]
impl WebSocketConsumer for SessionWsConsumer {
    async fn on_connect(&self, _ctx: &mut ConsumerContext) -> TestResult { Ok(()) }
    async fn on_message(&self, _ctx: &mut ConsumerContext, _msg: Message) -> TestResult { Ok(()) }
    async fn on_disconnect(&self, _ctx: &mut ConsumerContext) -> TestResult { Ok(()) }
}

// ── Fixtures ──────────────────────────────────────────────────────────────

#[fixture]
fn router() -> WebSocketRouter {
    WebSocketRouter::new()
        .consumer(|| EchoWsConsumer)
        .consumer(|| SessionWsConsumer)
}

// ── URL resolution E2E tests ──────────────────────────────────────────────

#[rstest]
fn test_endpoint_info_no_params() {
    assert_eq!(EchoWsConsumer::path(), "/ws/echo/");
    assert_eq!(EchoWsConsumer::name(), Some("echo_ws"));
}

#[rstest]
fn test_endpoint_info_with_param() {
    assert_eq!(SessionWsConsumer::path(), "/ws/session/{session_id}/");
    assert_eq!(SessionWsConsumer::name(), Some("session_ws"));
}

#[rstest]
fn test_router_reverse_no_params(router: WebSocketRouter) {
    let url = router.reverse("echo_ws", &[]);
    assert_eq!(url, Some("/ws/echo/".to_string()));
}

#[rstest]
fn test_router_reverse_with_param(router: WebSocketRouter) {
    let url = router.reverse("session_ws", &[("session_id", "abc123")]);
    assert_eq!(url, Some("/ws/session/abc123/".to_string()));
}

#[rstest]
fn test_router_reverse_unknown(router: WebSocketRouter) {
    assert_eq!(router.reverse("unknown_ws", &[]), None);
}

#[rstest]
fn test_router_finds_all_pending(router: WebSocketRouter) {
    assert!(router.find_pending("echo_ws").is_some());
    assert!(router.find_pending("session_ws").is_some());
}

// ── Real WebSocket connection E2E tests ───────────────────────────────────

/// Spawn a TCP server that dispatches WebSocket messages via
/// `EchoWsConsumer::on_message`.
async fn spawn_echo_server() -> (tokio::task::JoinHandle<()>, String, WebSocketRouter) {
    use reinhardt_websockets::connection::WebSocketConnection;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // Unencrypted loopback URL — for localhost tests only, not for production.
    let ws_url = ["ws", "://", &addr.to_string()].concat();

    let router = WebSocketRouter::new().consumer(|| EchoWsConsumer);

    let handle = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let ws_stream = match accept_async(stream).await {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let (mut write, mut read) = ws_stream.split();

                let consumer = EchoWsConsumer;
                let (tx, mut rx) = mpsc::unbounded_channel();
                let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
                let mut ctx = ConsumerContext::new(conn);

                while let Some(Ok(msg)) = read.next().await {
                    match msg {
                        TMsg::Text(text) => {
                            let message = Message::Text { data: text.to_string() };
                            // Dispatch to the consumer's on_message handler
                            if consumer.on_message(&mut ctx, message).await.is_err() {
                                break;
                            }
                            // Drain the consumer's outgoing channel and forward to client
                            while let Ok(out) = rx.try_recv() {
                                if let Message::Text { data } = out
                                    && write.send(TMsg::Text(data.into())).await.is_err()
                                {
                                    return;
                                }
                            }
                        }
                        TMsg::Close(_) => break,
                        _ => {}
                    }
                }
            });
        }
    });

    (handle, ws_url, router)
}

#[rstest]
#[tokio::test]
async fn test_e2e_url_resolution_then_connect() {
    // Arrange: start server + get router
    let (handle, server_url, router) = spawn_echo_server().await;

    // Act: resolve URL via router (simulates urls.ws().chat().echo_ws())
    let resolved = router.reverse("echo_ws", &[]).unwrap();
    assert_eq!(resolved, "/ws/echo/");

    // Connect to the actual server using the resolved URL path so that this
    // test actually exercises the resolver (not just the server root).
    let connect_url = format!(
        "{}/{}",
        server_url.trim_end_matches('/'),
        resolved.trim_start_matches('/')
    );
    let (mut ws, _) = connect_async(&connect_url).await.unwrap();
    ws.send(TMsg::Text("hello".into())).await.unwrap();

    // Assert: consumer's on_message echoed back
    if let Some(Ok(TMsg::Text(resp))) = ws.next().await {
        assert_eq!(resp.as_str(), "echo: hello");
    } else {
        panic!("Expected echo response");
    }

    ws.close(None).await.unwrap();
    handle.abort();
}

#[rstest]
#[tokio::test]
async fn test_e2e_url_with_path_param() {
    let router = WebSocketRouter::new().consumer(|| SessionWsConsumer);
    let url = router
        .reverse("session_ws", &[("session_id", "user42")])
        .unwrap();
    assert_eq!(url, "/ws/session/user42/");
}

#[rstest]
#[tokio::test]
async fn test_e2e_multiple_messages() {
    let (handle, server_url, router) = spawn_echo_server().await;
    assert_eq!(router.reverse("echo_ws", &[]).unwrap(), "/ws/echo/");

    let (mut ws, _) = connect_async(&server_url).await.unwrap();

    for i in 0..3_u32 {
        let msg = format!("msg_{}", i);
        ws.send(TMsg::Text(msg.clone().into())).await.unwrap();

        if let Some(Ok(TMsg::Text(resp))) = ws.next().await {
            assert_eq!(resp.as_str(), format!("echo: {}", msg).as_str());
        } else {
            panic!("Expected echo for msg {}", i);
        }
    }

    ws.close(None).await.unwrap();
    handle.abort();
}
