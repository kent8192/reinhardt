//! WebSocket support for Reinhardt framework
//!
//! This crate provides comprehensive WebSocket support for the Reinhardt framework,
//! including connection management, room-based messaging, authentication, rate limiting,
//! middleware integration, and distributed channel layers.
//!
//! ## Features
//!
//! - **Connection Management**: Robust WebSocket connection handling with lifecycle hooks
//! - **Room-Based Messaging**: Group connections into rooms for targeted broadcasting
//! - **Authentication & Authorization**: Token-based auth and permission-based authorization
//! - **Rate Limiting**: Connection and message rate limiting to prevent abuse
//! - **Middleware Integration**: Pre-processing and post-processing of connections and messages
//! - **WebSocket Routing**: URL-based WebSocket endpoint registration
//! - **Channel Layers**: Distributed messaging for multi-instance deployments
//! - **Consumer Classes**: Django Channels-inspired message handling patterns
//!
//! ## Basic Usage
//!
//! ```
//! use reinhardt_websockets::{WebSocketConnection, Message};
//! use tokio::sync::mpsc;
//! use std::sync::Arc;
//!
//! # tokio_test::block_on(async {
//! let (tx, mut rx) = mpsc::unbounded_channel();
//! let conn = Arc::new(WebSocketConnection::new("user_1".to_string(), tx));
//!
//! conn.send_text("Hello, WebSocket!".to_string()).await.unwrap();
//!
//! let msg = rx.recv().await.unwrap();
//! match msg {
//!     Message::Text { data } => println!("Received: {}", data),
//!     _ => {}
//! }
//! # });
//! ```
//!
//! ## Native application routing
//!
//! A Pages app can expose WebSocket routes through its generated
//! `urls/ws_urls.rs` module and merge that module into its `UnifiedRouter`.
//! The project-level `#[routes]` function then merges each app aggregate.
//! `manage runserver` mounts those upgrades on the HTTP listener, so a
//! separate WebSocket `main.rs` is not required.
//!
//! ```rust,ignore
//! use reinhardt::{websocket, Message, UnifiedRouter, WebSocketRouter};
//! use reinhardt::{ConsumerContext, WebSocketResult};
//!
//! #[websocket("/ws/echo/", name = "echo")]
//! async fn echo(context: &mut ConsumerContext, message: Message) -> WebSocketResult<()> {
//!     if let Message::Text { data } = message {
//!         context.connection.send_text(data).await?;
//!     }
//!     Ok(())
//! }
//!
//! pub fn ws_url_patterns() -> WebSocketRouter {
//!     WebSocketRouter::new().consumer(echo)
//! }
//! ```
//!
//! ## Advanced Features
//!
//! ### Message Compression
//!
//! The `compression` feature enables gzip, deflate, and brotli compression for WebSocket messages:
//!
//! ```toml
//! [dependencies]
//! reinhardt-websockets = { version = "0.1", features = ["compression"] }
//! ```
//!
//! ### Automatic Reconnection
//!
//! The `reconnection` module provides automatic reconnection with exponential backoff:
//!
//! ```
//! use reinhardt_websockets::reconnection::{ReconnectionConfig, ReconnectionStrategy};
//! use std::time::Duration;
//!
//! let config = ReconnectionConfig::default()
//!     .with_max_attempts(5)
//!     .with_initial_delay(Duration::from_secs(1));
//!
//! let mut strategy = ReconnectionStrategy::new(config);
//! ```
//!
//! ### Redis Channel Layer
//!
//! The `redis-channel` feature enables distributed messaging via Redis:
//!
//! ```toml
//! [dependencies]
//! reinhardt-websockets = { version = "0.1", features = ["redis-channel"] }
//! ```
//!
//! ### Metrics and Monitoring
//!
//! The `metrics` module provides comprehensive WebSocket metrics:
//!
//! ```
//! use reinhardt_websockets::metrics::{WebSocketMetrics, MetricsCollector};
//!
//! let metrics = WebSocketMetrics::new();
//! metrics.record_connection();
//! metrics.record_message_sent();
//!
//! let snapshot = metrics.snapshot();
//! println!("{}", snapshot.summary());
//! ```
//!
//! ### Integration with reinhardt-pages
//!
//! The `pages-integration` feature enables seamless integration with reinhardt-pages,
//! allowing WebSocket connections to use the same Cookie/session-based authentication
//! as the HTTP layer:
//!
//! ```toml
//! [dependencies]
//! reinhardt-websockets = { version = "0.1", features = ["pages-integration"] }
//! ```
//!
//! **Server-side setup:**
//!
//! ```ignore
//! use reinhardt_websockets::{PagesAuthenticator, WebSocketRouter, WebSocketRoute};
//! use std::sync::Arc;
//!
//! // Create authenticator that integrates with reinhardt-pages sessions
//! let authenticator = Arc::new(PagesAuthenticator::new());
//!
//! // Register WebSocket routes
//! let mut router = WebSocketRouter::new();
//! router.register_route(WebSocketRoute::new(
//!     "/ws/chat".to_string(),
//!     Some("websocket:chat".to_string()),
//! )).await.unwrap();
//! ```
//!
//! **Client-side usage (WASM):**
//!
//! On the client side, use the `use_websocket` hook from reinhardt-pages:
//!
//! ```ignore
//! use reinhardt_pages::deps;
//! use reinhardt_pages::reactive::hooks::{
//!     use_retained_effect, use_websocket, ConnectionState, UseWebSocketOptions,
//! };
//!
//! let ws = use_websocket("ws://localhost:8000/ws/chat", UseWebSocketOptions::default());
//!
//! // Send message
//! ws.send_text("Hello, server!".to_string()).ok();
//!
//! // Monitor connection state
//! let connection_state = ws.connection_state().clone();
//! use_retained_effect(
//!     {
//!         let connection_state = connection_state.clone();
//!         move || {
//!             match connection_state.get() {
//!                 ConnectionState::Open => log!("Connected"),
//!                 ConnectionState::Closed => log!("Disconnected"),
//!                 _ => {}
//!             }
//!             None::<fn()>
//!         }
//!     },
//!     deps![connection_state],
//! );
//! ```
//!
//! The authentication cookies from the user's HTTP session are automatically included
//! in the WebSocket handshake, allowing the server to authenticate the connection.

#![warn(missing_docs)]

/// Token-based authentication and authorization for WebSocket connections.
pub mod auth;
/// Channel layer abstraction for cross-process messaging.
pub mod channels;
/// Message compression (gzip, deflate, brotli).
#[cfg(feature = "compression")]
pub mod compression;
/// WebSocket connection management and ping/pong keepalive.
pub mod connection;
/// Django Channels-inspired consumer classes for message handling.
pub mod consumers;
/// Compile-time endpoint metadata and URL parameter substitution.
pub mod endpoint;
/// WebSocket upgrade handler and connection lifecycle.
pub mod handler;
/// Integration with reinhardt-pages for cookie/session-based auth.
#[cfg(feature = "pages-integration")]
pub mod integration;
/// WebSocket connection and message metrics.
pub mod metrics;
/// WebSocket middleware for pre/post-processing.
pub mod middleware;
/// Origin validation for WebSocket handshake requests.
pub mod origin;
/// WebSocket protocol frame handling.
pub mod protocol;
/// Automatic reconnection with exponential backoff.
pub mod reconnection;
/// Redis-backed channel layer for distributed deployments.
#[cfg(feature = "redis-channel")]
pub mod redis_channel;
/// Executable WebSocket consumer registrations.
#[cfg(feature = "di")]
pub mod registry;
/// Room-based connection grouping for targeted broadcasts.
pub mod room;
/// URL-based WebSocket endpoint routing.
pub mod routing;
/// Settings-first configuration fragments.
pub mod settings;
/// Connection and message rate limiting.
pub mod throttling;
/// Hyper Upgrade protocol helpers.
pub mod upgrade;

pub use auth::{
	AuthError, AuthResult, AuthUser, AuthenticatedConnection, AuthorizationPolicy,
	PermissionBasedPolicy, SimpleAuthUser, TokenAuthenticator, WebSocketAuthenticator,
};
pub use channels::{
	ChannelError, ChannelLayer, ChannelLayerWrapper, ChannelMessage, ChannelResult,
	InMemoryChannelLayer,
};
#[cfg(feature = "compression")]
pub use compression::{
	CompressionCodec, CompressionConfig, compress_message, decompress_message,
	decompress_message_with_config,
};
#[allow(deprecated)] // `ConnectionConfig` is deprecated in favor of `ConnectionSettings`.
pub use connection::ConnectionConfig;
pub use connection::{
	ConnectionTimeoutMonitor, HeartbeatConfig, HeartbeatMonitor, Message, PingPongConfig,
	WebSocketConnection, WebSocketError, WebSocketResult,
};
#[cfg(feature = "di")]
pub use consumers::InjectionContext;
pub use consumers::{
	BroadcastConsumer, ConsumerChain, ConsumerContext, EchoConsumer, JsonConsumer,
	WebSocketConsumer,
};
pub use endpoint::{WebSocketEndpointInfo, WebSocketEndpointMetadata, substitute_ws_params};
pub use handler::WebSocketHandler;
#[cfg(feature = "pages-integration")]
pub use integration::pages::{PagesAuthUser, PagesAuthenticator};
#[cfg(feature = "metrics")]
pub use metrics::MetricsExporter;
pub use metrics::{MetricsCollector, MetricsSnapshot, PeriodicReporter, WebSocketMetrics};
pub use middleware::{
	ConnectionContext, ConnectionMiddleware, IpFilterMiddleware, LoggingMiddleware,
	MessageMiddleware, MessageSizeLimitMiddleware, MiddlewareChain, MiddlewareError,
	MiddlewareResult,
};
#[allow(deprecated)]
// `OriginValidationConfig` is deprecated in favor of `OriginValidationSettings`.
pub use origin::OriginValidationConfig;
pub use origin::{OriginPolicy, OriginValidationMiddleware, validate_origin};
pub use protocol::{
	DEFAULT_MAX_FRAME_SIZE, DEFAULT_MAX_MESSAGE_SIZE, default_websocket_config,
	websocket_config_with_limits,
};
#[allow(deprecated)] // `ReconnectionConfig` is deprecated in favor of `ReconnectionSettings`.
pub use reconnection::ReconnectionConfig;
pub use reconnection::{AutoReconnectHandler, ReconnectionState, ReconnectionStrategy};
#[cfg(feature = "redis-channel")]
pub use redis_channel::RedisChannelLayer;
#[cfg(feature = "redis-channel")]
#[allow(deprecated)] // `RedisConfig` is deprecated in favor of `RedisChannelSettings`.
pub use redis_channel::RedisConfig;
#[cfg(feature = "di")]
pub use registry::{
	ConsumerBuildError, ConsumerBuildFuture, ConsumerPreflightFuture, WebSocketConsumerRegistration,
};
pub use reinhardt_core::ws::WebSocketConsumerKey;
pub use room::{BroadcastResult, Room, RoomError, RoomManager, RoomResult};
pub use routing::{
	RouteError, RouteResult, WebSocketRoute, WebSocketRouter, clear_websocket_router,
	get_websocket_router, register_websocket_router, reverse_websocket_url,
};
pub use settings::{
	ConnectionSettings, OriginPolicySettings, OriginValidationSettings, RateLimitSettings,
	ReconnectionSettings, create_connection_config_from_settings,
	create_origin_validation_config_from_settings, create_rate_limit_config_from_settings,
	create_reconnection_config_from_settings,
};
#[cfg(feature = "redis-channel")]
pub use settings::{RedisChannelSettings, create_redis_config_from_settings};
#[allow(deprecated)]
// `WebSocketRateLimitConfig` is deprecated in favor of `RateLimitSettings`.
pub use throttling::WebSocketRateLimitConfig;
pub use throttling::{
	CombinedThrottler, ConnectionRateLimiter, ConnectionThrottler, RateLimitConfig,
	RateLimitMiddleware, RateLimiter, ThrottleError, ThrottleResult,
};
pub use upgrade::create_upgrade_response;
#[cfg(feature = "di")]
pub use upgrade::{
	serve_upgraded_consumer, serve_upgraded_consumer_with_shutdown,
	serve_upgraded_consumer_with_shutdown_and_config,
};

#[doc(hidden)]
pub use inventory;
#[cfg(feature = "di")]
#[doc(hidden)]
pub use reinhardt_di;

#[cfg(test)]
mod tests;
