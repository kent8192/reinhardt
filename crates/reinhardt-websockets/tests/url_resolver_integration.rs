use reinhardt_websockets::{
    WebSocketEndpointInfo, WebSocketRouter,
    consumers::{ConsumerContext, WebSocketConsumer},
    connection::{Message, WebSocketResult},
};
use rstest::{fixture, rstest};
use async_trait::async_trait;

// ── consumer definitions ──────────────────────────────────────────────────

struct ChatWsConsumer;

impl WebSocketEndpointInfo for ChatWsConsumer {
    fn path() -> &'static str { "/ws/chat/{room_id}/" }
    fn name() -> Option<&'static str> { Some("chat_ws") }
}

#[async_trait]
impl WebSocketConsumer for ChatWsConsumer {
    async fn on_connect(&self, _ctx: &mut ConsumerContext) -> WebSocketResult<()> { Ok(()) }
    async fn on_message(&self, _ctx: &mut ConsumerContext, _msg: Message) -> WebSocketResult<()> { Ok(()) }
    async fn on_disconnect(&self, _ctx: &mut ConsumerContext) -> WebSocketResult<()> { Ok(()) }
}

struct NotifWsConsumer;

impl WebSocketEndpointInfo for NotifWsConsumer {
    fn path() -> &'static str { "/ws/notif/" }
    fn name() -> Option<&'static str> { Some("notif_ws") }
}

#[async_trait]
impl WebSocketConsumer for NotifWsConsumer {
    async fn on_connect(&self, _ctx: &mut ConsumerContext) -> WebSocketResult<()> { Ok(()) }
    async fn on_message(&self, _ctx: &mut ConsumerContext, _msg: Message) -> WebSocketResult<()> { Ok(()) }
    async fn on_disconnect(&self, _ctx: &mut ConsumerContext) -> WebSocketResult<()> { Ok(()) }
}

// ── fixtures ─────────────────────────────────────────────────────────────

#[fixture]
fn router() -> WebSocketRouter {
    WebSocketRouter::new()
        .consumer(|| ChatWsConsumer)
        .consumer(|| NotifWsConsumer)
}

// ── tests ─────────────────────────────────────────────────────────────────

#[rstest]
fn test_endpoint_info_path() {
    assert_eq!(<ChatWsConsumer as WebSocketEndpointInfo>::path(), "/ws/chat/{room_id}/");
    assert_eq!(<NotifWsConsumer as WebSocketEndpointInfo>::path(), "/ws/notif/");
}

#[rstest]
fn test_endpoint_info_name() {
    assert_eq!(<ChatWsConsumer as WebSocketEndpointInfo>::name(), Some("chat_ws"));
    assert_eq!(<NotifWsConsumer as WebSocketEndpointInfo>::name(), Some("notif_ws"));
}

#[rstest]
fn test_consumer_builder_registers_route(router: WebSocketRouter) {
    // Act
    let chat_route = router.find_pending("chat_ws");
    let notif_route = router.find_pending("notif_ws");

    // Assert
    assert!(chat_route.is_some());
    assert_eq!(chat_route.unwrap().path(), "/ws/chat/{room_id}/");
    assert!(notif_route.is_some());
    assert_eq!(notif_route.unwrap().path(), "/ws/notif/");
}

#[rstest]
fn test_reverse_with_param(router: WebSocketRouter) {
    // Act
    let url = router.reverse("chat_ws", &[("room_id", "42")]);
    // Assert
    assert_eq!(url, Some("/ws/chat/42/".to_string()));
}

#[rstest]
fn test_reverse_no_params(router: WebSocketRouter) {
    // Act
    let url = router.reverse("notif_ws", &[]);
    // Assert
    assert_eq!(url, Some("/ws/notif/".to_string()));
}

#[rstest]
fn test_reverse_unknown_name(router: WebSocketRouter) {
    // Act
    let url = router.reverse("nonexistent_ws", &[]);
    // Assert
    assert_eq!(url, None);
}

#[rstest]
fn test_substitute_ws_params_multiple() {
    // Arrange
    let path = "/ws/{org}/{repo}/";
    // Act
    let result = reinhardt_websockets::substitute_ws_params(path, &[("org", "acme"), ("repo", "app")]);
    // Assert
    assert_eq!(result, "/ws/acme/app/");
}

#[rstest]
fn test_substitute_ws_params_no_params() {
    // Arrange
    let path = "/ws/notif/";
    // Act
    let result = reinhardt_websockets::substitute_ws_params(path, &[]);
    // Assert
    assert_eq!(result, "/ws/notif/");
}
