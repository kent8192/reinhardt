//! Integration tests for UnifiedRouter with hierarchical routing and namespace support

use async_trait::async_trait;
use reinhardt_core::reactive::ReactiveScope;
use reinhardt_core::ws::WebSocketEndpointInfo;
use reinhardt_di::{DiRegistrationList, InjectionContext, SingletonScope};
use reinhardt_grpc::{GrpcRouteError, GrpcRouter};
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response, Result, ViewResult};
use reinhardt_macros::get;
use reinhardt_urls::routers::{NativeHttpRoutes, NativeRoutes, ServerRouter, UnifiedRouter};
use reinhardt_views::viewsets::{Action, ActionType, ViewSet};
use std::convert::Infallible;
use std::future::{Ready, ready};
use std::sync::Arc;
use std::task::{Context, Poll};
use tonic::body::Body;
use tonic::codegen::Service;
use tonic::codegen::http::{Request as GrpcRequest, Response as GrpcResponse};
use tonic::server::NamedService;

// Mock ViewSet for testing
#[derive(Clone)]
struct UserViewSet;

#[async_trait]
impl ViewSet for UserViewSet {
	fn get_basename(&self) -> &str {
		"users"
	}

	async fn dispatch(&self, _req: Request, action: Action) -> Result<Response> {
		match action.action_type {
			ActionType::List => Ok(Response::ok().with_body(b"User list".to_vec())),
			ActionType::Retrieve => Ok(Response::ok().with_body(b"User detail".to_vec())),
			ActionType::Create => Ok(Response::ok().with_body(b"User created".to_vec())),
			ActionType::Update => Ok(Response::ok().with_body(b"User updated".to_vec())),
			ActionType::Destroy => Ok(Response::ok().with_body(b"User deleted".to_vec())),
			_ => Ok(Response::not_found()),
		}
	}
}

// Mock handlers using HTTP Method Macro
#[get("/health", name = "health")]
async fn health_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"OK".to_vec()))
}

#[get("/list", name = "list")]
async fn list_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"List".to_vec()))
}

#[get("/action", name = "action")]
async fn action_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"Action".to_vec()))
}

#[get("/export", name = "export")]
async fn export_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"Export".to_vec()))
}

// Mock view handler
#[derive(Clone)]
struct AboutView;

#[async_trait]
impl Handler for AboutView {
	async fn handle(&self, _req: Request) -> Result<Response> {
		Ok(Response::ok().with_body(b"About page".to_vec()))
	}
}

struct ChatConsumer;

impl WebSocketEndpointInfo for ChatConsumer {
	fn path() -> &'static str {
		"/chat/"
	}

	fn name() -> Option<&'static str> {
		Some("socket")
	}
}

struct NotificationConsumer;

impl WebSocketEndpointInfo for NotificationConsumer {
	fn path() -> &'static str {
		"/notifications/"
	}

	fn name() -> Option<&'static str> {
		Some("socket")
	}
}

#[derive(Clone)]
struct ChatGrpcService;

#[derive(Clone)]
struct NotificationGrpcService;

macro_rules! impl_grpc_service {
	($service:ty, $name:literal) => {
		impl Service<GrpcRequest<Body>> for $service {
			type Response = GrpcResponse<Body>;
			type Error = Infallible;
			type Future = Ready<std::result::Result<Self::Response, Self::Error>>;

			fn poll_ready(
				&mut self,
				_context: &mut Context<'_>,
			) -> Poll<std::result::Result<(), Self::Error>> {
				Poll::Ready(Ok(()))
			}

			fn call(&mut self, _request: GrpcRequest<Body>) -> Self::Future {
				ready(Ok(GrpcResponse::new(Body::empty())))
			}
		}

		impl NamedService for $service {
			const NAME: &'static str = $name;
		}
	};
}

impl_grpc_service!(ChatGrpcService, "chat.ChatService");
impl_grpc_service!(NotificationGrpcService, "notifications.NotificationService");

fn chat_routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.with_namespace("chat")
		.websocket(|router| router.consumer(|| ChatConsumer))
		.grpc(|router| router.service(ChatGrpcService))
}

fn notification_routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.with_namespace("notifications")
		.websocket(|router| router.consumer(|| NotificationConsumer))
		.grpc(|router| router.service(NotificationGrpcService))
}

fn websocket_registration_order(native: &NativeRoutes) -> Vec<&str> {
	native
		.websocket
		.routes()
		.iter()
		.map(|route| route.name().expect("test routes are named"))
		.collect()
}

#[test]
fn native_merge_preserves_protocol_order_and_namespaces() {
	let native = UnifiedRouter::new()
		.merge(chat_routes())
		.merge(notification_routes())
		.__into_native_routes();

	assert_eq!(native.websocket.len(), 2);
	assert_eq!(native.grpc.len(), 2);
	assert_eq!(
		websocket_registration_order(&native),
		["chat:socket", "notifications:socket"]
	);
	let diagnostics = GrpcRouter::new()
		.service(ChatGrpcService)
		.service(NotificationGrpcService)
		.merge(native.grpc);
	assert_eq!(
		diagnostics.validation_errors(),
		[
			GrpcRouteError::DuplicateService {
				service: "chat.ChatService",
				first_namespace: None,
				second_namespace: Some(String::from("chat")),
			},
			GrpcRouteError::DuplicateService {
				service: "notifications.NotificationService",
				first_namespace: None,
				second_namespace: Some(String::from("notifications")),
			},
		]
	);
}

#[test]
fn native_mount_prefixes_websocket_and_validates_grpc_prefixes() {
	ReactiveScope::run(|| {
		let root = UnifiedRouter::new()
			.client(|client| client)
			.mount_unified("/", chat_routes().client(|client| client))
			.__into_native_routes();
		let non_root = UnifiedRouter::new()
			.client(|client| client)
			.mount_unified("/api/", notification_routes().client(|client| client))
			.__into_native_routes();

		assert_eq!(root.websocket.routes()[0].path(), "/chat/");
		assert_eq!(root.grpc.len(), 1);
		assert_eq!(non_root.websocket.routes()[0].path(), "/api/notifications/");
		assert_eq!(
			non_root.grpc.validation_errors(),
			[GrpcRouteError::NonRootMount {
				prefix: "/api/".into()
			}]
		);
	});
}

#[test]
fn native_extraction_preserves_context_deferred_di_and_streaming_once() {
	let context = Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());
	let mut first = DiRegistrationList::new();
	first.register(String::from("chat"));
	let mut second = DiRegistrationList::new();
	second.register(String::from("notifications"));

	let native = UnifiedRouter::new()
		.with_di_context(Arc::clone(&context))
		.with_di_registrations(first)
		.mount_streaming(
			reinhardt_streaming::StreamingRouter::new().producer("chat", "chat-producer"),
		)
		.merge(
			UnifiedRouter::new()
				.with_di_registrations(second)
				.mount_streaming(
					reinhardt_streaming::StreamingRouter::new()
						.producer("notifications", "notification-producer"),
				),
		)
		.__into_native_routes();

	assert!(matches!(native.server, NativeHttpRoutes::Owned(_)));
	assert!(Arc::ptr_eq(
		native
			.di_context
			.as_ref()
			.expect("attached context must be preserved"),
		&context
	));
	assert_eq!(native.di_registrations.len(), 2);
	native.di_registrations.apply_to(context.singleton_scope());
	assert_eq!(
		context
			.singleton_scope()
			.get::<String>()
			.expect("same-type registrations must be applied")
			.as_str(),
		"notifications"
	);
	assert_eq!(
		native
			.streaming_handlers
			.iter()
			.map(|handler| handler.name)
			.collect::<Vec<_>>(),
		["chat-producer", "notification-producer"]
	);
}

#[test]
fn http_only_into_server_remains_compatible() {
	let server = UnifiedRouter::new()
		.server(|router| router.with_prefix("/api"))
		.into_server();

	assert_eq!(server.prefix(), "/api");
}

#[tokio::test]
async fn test_unified_router_basic_structure() {
	let router = ServerRouter::new()
		.with_prefix("/api")
		.with_namespace("api");

	assert_eq!(router.prefix(), "/api");
	assert_eq!(router.namespace(), Some("api"));
}

#[tokio::test]
async fn test_unified_router_mount_child() {
	let child = ServerRouter::new().with_namespace("users");

	let router = ServerRouter::new()
		.with_prefix("/api")
		.with_namespace("api")
		.mount("/users/", child);

	assert_eq!(router.children_count(), 1);
}

#[tokio::test]
async fn test_unified_router_with_viewset() {
	let router = ServerRouter::new()
		.with_prefix("/api")
		.viewset("users", UserViewSet);

	// Check that routes are generated
	let routes = router.get_all_routes();
	assert!(!routes.is_empty());
}

#[tokio::test]
async fn test_unified_router_hierarchical_namespace() {
	let users = ServerRouter::new()
		.with_namespace("users")
		.viewset("users", UserViewSet);

	let mut api = ServerRouter::new()
		.with_namespace("v1")
		.with_prefix("/api/v1")
		.mount("/users/", users);

	// Register all routes
	let errors = api.register_all_routes();
	assert!(errors.is_empty());

	// Check namespace resolution
	assert_eq!(api.namespace(), Some("v1"));
}

#[tokio::test]
async fn test_unified_router_url_reversal() {
	let mut router = ServerRouter::new()
		.with_namespace("api")
		.endpoint(health_handler);

	let errors = router.register_all_routes();
	assert!(errors.is_empty());

	// Reverse URL with namespace
	let url = router.reverse("api:health", &[]);
	assert!(url.is_some());
	assert_eq!(url.unwrap(), "/health");
}

#[tokio::test]
async fn test_unified_router_nested_namespace_reversal() {
	let users = ServerRouter::new()
		.with_namespace("users")
		.endpoint(list_handler);

	let v1 = ServerRouter::new()
		.with_namespace("v1")
		.mount("/users/", users);

	let mut api = ServerRouter::new().with_namespace("api").mount("/v1/", v1);

	let errors = api.register_all_routes();
	assert!(errors.is_empty());

	// Reverse with full namespace chain
	let url = api.reverse("api:v1:users:list", &[]);
	assert!(url.is_some());
}

#[tokio::test]
async fn test_unified_router_multiple_children() {
	let users = ServerRouter::new()
		.with_namespace("users")
		.viewset("users", UserViewSet);

	let posts = ServerRouter::new()
		.with_namespace("posts")
		.endpoint(list_handler);

	let router = ServerRouter::new()
		.with_prefix("/api")
		.mount("/users/", users)
		.mount("/posts/", posts);

	assert_eq!(router.children_count(), 2);

	let routes = router.get_all_routes();
	assert!(!routes.is_empty());
}

#[tokio::test]
async fn test_unified_router_mixed_api_styles() {
	let router = ServerRouter::new()
		.with_prefix("/api")
		.endpoint(health_handler)
		.viewset("users", UserViewSet)
		.view("/about", AboutView);

	let routes = router.get_all_routes();
	// Should have routes from endpoint, ViewSet, and view
	assert!(routes.len() >= 3);
}

#[tokio::test]
async fn test_unified_router_deep_nesting() {
	// Create a dedicated handler for the action endpoint with POST method
	#[reinhardt_macros::post("/action", name = "action")]
	async fn action_post_handler() -> ViewResult<Response> {
		Ok(Response::ok().with_body(b"Action".to_vec()))
	}

	let resource = ServerRouter::new()
		.with_namespace("resource")
		.endpoint(action_post_handler);

	let v2 = ServerRouter::new()
		.with_namespace("v2")
		.mount("/resource/", resource);

	let v1 = ServerRouter::new().with_namespace("v1").mount("/v2/", v2);

	let mut api = ServerRouter::new()
		.with_namespace("api")
		.with_prefix("/api")
		.mount("/v1/", v1);

	let errors = api.register_all_routes();
	assert!(errors.is_empty());

	// Test deep namespace resolution
	let url = api.reverse("api:v1:v2:resource:action", &[]);
	assert!(url.is_some());
}

#[tokio::test]
async fn test_unified_router_get_all_routes() {
	let users = ServerRouter::new()
		.with_namespace("users")
		.endpoint(export_handler);

	let router = ServerRouter::new()
		.with_prefix("/api")
		.with_namespace("api")
		.endpoint(health_handler)
		.mount("/users/", users);

	let routes = router.get_all_routes();

	// Should have routes from both parent and child
	assert!(routes.len() >= 2);

	// Check namespace combination in routes
	let has_combined_namespace = routes
		.iter()
		.any(|(_, _, ns, _)| ns.as_ref().is_some_and(|n| n.contains(':')));
	assert!(has_combined_namespace);
}

#[tokio::test]
async fn test_unified_router_viewset_url_reversal() {
	let mut router = ServerRouter::new()
		.with_namespace("api")
		.with_prefix("/api")
		.viewset("users", UserViewSet);

	let errors = router.register_all_routes();
	assert!(errors.is_empty());

	// ViewSets should register standard action names
	let list_url = router.reverse("api:users-list", &[]);
	assert!(list_url.is_some());

	let detail_url = router.reverse("api:users-detail", &[("id", "123")]);
	assert!(detail_url.is_some());
}

#[tokio::test]
async fn test_unified_router_namespace_inheritance() {
	// Create a dedicated handler for this test with POST method
	#[reinhardt_macros::post("/action", name = "action")]
	async fn action_inherit_handler() -> ViewResult<Response> {
		Ok(Response::ok().with_body(b"Action".to_vec()))
	}

	let child = ServerRouter::new().endpoint(action_inherit_handler);

	let mut parent = ServerRouter::new()
		.with_namespace("parent")
		.mount("/child/", child);

	let errors = parent.register_all_routes();
	assert!(errors.is_empty());

	// Child route should inherit parent namespace
	let url = parent.reverse("parent:action", &[]);
	assert!(url.is_some());
}
