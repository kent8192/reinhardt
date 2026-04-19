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
//! use reinhardt_pages::reactive::hooks::{use_websocket, UseWebSocketOptions};
//!
//! let ws = use_websocket("ws://localhost:8000/ws/chat", UseWebSocketOptions::default());
//!
//! // Send message
//! ws.send_text("Hello, server!".to_string()).ok();
//!
//! // Monitor connection state
//! use_effect({
//!     let ws = ws.clone();
//!     move || {
//!         match ws.connection_state().get() {
//!             ConnectionState::Open => log!("Connected"),
//!             ConnectionState::Closed => log!("Disconnected"),
//!             _ => {}
//!         }
//!         None::<fn()>
//!     }
//! });
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
/// Room-based connection grouping for targeted broadcasts.
pub mod room;
/// URL-based WebSocket endpoint routing.
pub mod routing;
/// Compile-time endpoint metadata and URL parameter substitution.
pub mod endpoint;
/// Connection and message rate limiting.
pub mod throttling;

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
pub use connection::{
	ConnectionConfig, ConnectionTimeoutMonitor, HeartbeatConfig, HeartbeatMonitor, Message,
	PingPongConfig, WebSocketConnection, WebSocketError, WebSocketResult,
};
pub use consumers::{
	BroadcastConsumer, ConsumerChain, ConsumerContext, EchoConsumer, JsonConsumer,
	WebSocketConsumer,
};
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
pub use origin::{
	OriginPolicy, OriginValidationConfig, OriginValidationMiddleware, validate_origin,
};
pub use protocol::{
	DEFAULT_MAX_FRAME_SIZE, DEFAULT_MAX_MESSAGE_SIZE, default_websocket_config,
	websocket_config_with_limits,
};
pub use reconnection::{
	AutoReconnectHandler, ReconnectionConfig, ReconnectionState, ReconnectionStrategy,
};
#[cfg(feature = "redis-channel")]
pub use redis_channel::{RedisChannelLayer, RedisConfig};
pub use room::{BroadcastResult, Room, RoomError, RoomManager, RoomResult};
pub use endpoint::{WebSocketEndpointInfo, WebSocketEndpointMetadata, substitute_ws_params};
pub use routing::{
	RouteError, RouteResult, WebSocketRoute, WebSocketRouter, clear_websocket_router,
	get_websocket_router, register_websocket_router, reverse_websocket_url,
};
pub use throttling::{
	CombinedThrottler, ConnectionRateLimiter, ConnectionThrottler, RateLimitConfig,
	RateLimitMiddleware, RateLimiter, ThrottleError, ThrottleResult, WebSocketRateLimitConfig,
};

#[cfg(test)]
mod tests;
