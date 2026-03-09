//! HTTP server implementation for Reinhardt framework.
//!
//! This module provides HTTP/1.1 and HTTP/2 server implementations based on Hyper.
//!
//! ## Basic Usage
//!
//! ```rust,no_run,ignore
//! # use reinhardt_server::{HttpServer, serve};
//! # use std::net::SocketAddr;
//! # use std::sync::Arc;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // let addr: SocketAddr = "127.0.0.1:8000".parse()?;
//! // let handler = Arc::new(MyHandler);
//! //
//! // let server = HttpServer::new(handler.clone());
//! //
//! // // Start server
//! // serve(addr, handler).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Accessing Handler
//!
//! Use the `handler()` method to get a clone of the server's handler:
//!
//! ```rust,no_run,ignore
//! # use reinhardt_server::HttpServer;
//! # use std::sync::Arc;
//! # let handler = Arc::new(());
//! // let server = HttpServer::new(handler);
//! // let handler_clone = server.handler();  // Returns Arc<dyn Handler>
//! ```

/// HTTP/1.1 server implementation based on Hyper.
pub mod http;
/// HTTP/2 server implementation with TLS support.
pub mod http2;
/// Rate limiting handler for controlling request throughput.
pub mod rate_limit;
/// Graceful shutdown coordination for server instances.
pub mod shutdown;
/// Request timeout handler for enforcing maximum execution time.
pub mod timeout;

#[cfg(feature = "graphql")]
/// GraphQL request handler integration (requires `graphql` feature).
pub mod graphql;

#[cfg(feature = "websocket")]
/// WebSocket server support with broadcast capabilities (requires `websocket` feature).
pub mod websocket;

pub use http::{HttpServer, serve, serve_with_shutdown};
pub use http2::{Http2Server, serve_http2, serve_http2_with_shutdown};
pub use rate_limit::{RateLimitConfig, RateLimitHandler, RateLimitStrategy};
pub use shutdown::{ShutdownCoordinator, shutdown_signal, with_shutdown};
pub use timeout::TimeoutHandler;

#[cfg(feature = "graphql")]
pub use graphql::{GraphQLHandler, graphql_handler};

#[cfg(feature = "websocket")]
pub use websocket::{BroadcastManager, WebSocketHandler, WebSocketServer, serve_websocket};

// Re-export types needed for server trait
pub use reinhardt_http::{Request, Response};

/// Common server trait that all server types implement
pub trait ServerHandler: Send + Sync {
	/// The error type returned by this handler.
	type Error;
	/// Handles an incoming HTTP request and returns a response.
	fn handle(
		&self,
		request: Request,
	) -> impl std::future::Future<Output = Result<Response, Self::Error>> + Send;
}
