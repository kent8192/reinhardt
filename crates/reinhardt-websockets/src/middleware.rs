//! WebSocket middleware integration
//!
//! This module provides middleware support for WebSocket connections,
//! allowing pre-processing and post-processing of connections and messages.

use crate::connection::{Message, WebSocketConnection};
use async_trait::async_trait;
use std::sync::Arc;

/// WebSocket middleware result
pub type MiddlewareResult<T> = Result<T, MiddlewareError>;

/// Middleware errors
#[derive(Debug, thiserror::Error)]
pub enum MiddlewareError {
	#[error("Connection rejected")]
	ConnectionRejected(String),
	#[error("Message rejected")]
	MessageRejected(String),
	#[error("Middleware error")]
	Error(String),
}

/// WebSocket connection context for middleware
#[non_exhaustive]
pub struct ConnectionContext {
	/// Client IP address
	pub ip: String,
	/// Connection ID (set by the server after connection creation)
	pub connection_id: Option<String>,
	/// Connection headers (if available)
	pub headers: std::collections::HashMap<String, String>,
	/// Custom metadata
	pub metadata: std::collections::HashMap<String, String>,
}

impl ConnectionContext {
	/// Create a new connection context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::middleware::ConnectionContext;
	///
	/// let context = ConnectionContext::new("192.168.1.1".to_string());
	/// assert_eq!(context.ip, "192.168.1.1");
	/// ```
	pub fn new(ip: String) -> Self {
		Self {
			ip,
			connection_id: None,
			headers: std::collections::HashMap::new(),
			metadata: std::collections::HashMap::new(),
		}
	}

	/// Add a header to the context
	pub fn with_header(mut self, key: String, value: String) -> Self {
		self.headers.insert(key, value);
		self
	}

	/// Add metadata to the context
	pub fn with_metadata(mut self, key: String, value: String) -> Self {
		self.metadata.insert(key, value);
		self
	}
}

/// WebSocket connection middleware trait
///
/// Implementors can intercept WebSocket connections before they are established.
#[async_trait]
pub trait ConnectionMiddleware: Send + Sync {
	/// Process a connection before it is established
	///
	/// # Arguments
	///
	/// * `context` - Connection context with client information
	///
	/// # Returns
	///
	/// Returns `Ok(())` to allow the connection, or an error to reject it.
	async fn on_connect(&self, context: &mut ConnectionContext) -> MiddlewareResult<()>;

	/// Process a connection after it is closed
	///
	/// # Arguments
	///
	/// * `connection` - The closed connection
	async fn on_disconnect(&self, connection: &Arc<WebSocketConnection>) -> MiddlewareResult<()>;
}

/// WebSocket message middleware trait
///
/// Implementors can intercept and modify messages before they are processed.
#[async_trait]
pub trait MessageMiddleware: Send + Sync {
	/// Process a message before it is handled
	///
	/// # Arguments
	///
	/// * `connection` - The connection that sent the message
	/// * `message` - The message to process
	///
	/// # Returns
	///
	/// Returns the processed message, or an error to reject it.
	async fn on_message(
		&self,
		connection: &Arc<WebSocketConnection>,
		message: Message,
	) -> MiddlewareResult<Message>;
}

/// Logging middleware for WebSocket connections
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::middleware::{LoggingMiddleware, ConnectionMiddleware, ConnectionContext};
///
/// # tokio_test::block_on(async {
/// let middleware = LoggingMiddleware::new("WebSocket".to_string());
/// let mut context = ConnectionContext::new("192.168.1.1".to_string());
///
/// assert!(middleware.on_connect(&mut context).await.is_ok());
/// # });
/// ```
pub struct LoggingMiddleware {
	prefix: String,
}

impl LoggingMiddleware {
	/// Create a new logging middleware
	pub fn new(prefix: String) -> Self {
		Self { prefix }
	}
}

#[async_trait]
impl ConnectionMiddleware for LoggingMiddleware {
	async fn on_connect(&self, context: &mut ConnectionContext) -> MiddlewareResult<()> {
		println!(
			"[{}] Connection established from {}",
			self.prefix, context.ip
		);
		Ok(())
	}

	async fn on_disconnect(&self, connection: &Arc<WebSocketConnection>) -> MiddlewareResult<()> {
		println!("[{}] Connection closed: {}", self.prefix, connection.id());
		Ok(())
	}
}

#[async_trait]
impl MessageMiddleware for LoggingMiddleware {
	async fn on_message(
		&self,
		connection: &Arc<WebSocketConnection>,
		message: Message,
	) -> MiddlewareResult<Message> {
		match &message {
			Message::Text { data } => {
				println!(
					"[{}] Text message from {}: {}",
					self.prefix,
					connection.id(),
					data
				);
			}
			Message::Binary { data } => {
				println!(
					"[{}] Binary message from {}: {} bytes",
					self.prefix,
					connection.id(),
					data.len()
				);
			}
			Message::Ping => {
				println!("[{}] Ping from {}", self.prefix, connection.id());
			}
			Message::Pong => {
				println!("[{}] Pong from {}", self.prefix, connection.id());
			}
			Message::Close { .. } => {
				println!("[{}] Close from {}", self.prefix, connection.id());
			}
		}
		Ok(message)
	}
}

/// IP filtering middleware
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::middleware::{IpFilterMiddleware, ConnectionMiddleware, ConnectionContext};
///
/// # tokio_test::block_on(async {
/// let middleware = IpFilterMiddleware::whitelist(vec!["192.168.1.1".to_string()]);
/// let mut context = ConnectionContext::new("192.168.1.1".to_string());
///
/// assert!(middleware.on_connect(&mut context).await.is_ok());
///
/// let mut blocked_context = ConnectionContext::new("10.0.0.1".to_string());
/// assert!(middleware.on_connect(&mut blocked_context).await.is_err());
/// # });
/// ```
pub struct IpFilterMiddleware {
	allowed_ips: Vec<String>,
	blocked_ips: Vec<String>,
	mode: IpFilterMode,
}

#[derive(Debug, Clone, Copy)]
enum IpFilterMode {
	Whitelist,
	Blacklist,
}

impl IpFilterMiddleware {
	/// Create a whitelist-based filter
	pub fn whitelist(allowed_ips: Vec<String>) -> Self {
		Self {
			allowed_ips,
			blocked_ips: Vec::new(),
			mode: IpFilterMode::Whitelist,
		}
	}

	/// Create a blacklist-based filter
	pub fn blacklist(blocked_ips: Vec<String>) -> Self {
		Self {
			allowed_ips: Vec::new(),
			blocked_ips,
			mode: IpFilterMode::Blacklist,
		}
	}
}

#[async_trait]
impl ConnectionMiddleware for IpFilterMiddleware {
	async fn on_connect(&self, context: &mut ConnectionContext) -> MiddlewareResult<()> {
		match self.mode {
			IpFilterMode::Whitelist => {
				if self.allowed_ips.contains(&context.ip) {
					Ok(())
				} else {
					Err(MiddlewareError::ConnectionRejected(format!(
						"IP not in whitelist: {}",
						context.ip
					)))
				}
			}
			IpFilterMode::Blacklist => {
				if self.blocked_ips.contains(&context.ip) {
					Err(MiddlewareError::ConnectionRejected(format!(
						"IP is blacklisted: {}",
						context.ip
					)))
				} else {
					Ok(())
				}
			}
		}
	}

	async fn on_disconnect(&self, _connection: &Arc<WebSocketConnection>) -> MiddlewareResult<()> {
		Ok(())
	}
}

/// WebSocket close code for "Message Too Big" as defined in RFC 6455 Section 7.4.1
const CLOSE_CODE_MESSAGE_TOO_BIG: u16 = 1009;

/// Message size limit middleware
///
/// Enforces maximum message size to prevent memory exhaustion attacks.
/// By default, uses a 1 MB limit matching the protocol-level default.
/// When an oversized message is detected, the connection is closed with
/// status code 1009 (Message Too Big) as per RFC 6455.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::middleware::{MessageSizeLimitMiddleware, MessageMiddleware};
/// use reinhardt_websockets::{Message, WebSocketConnection};
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// // Use default 1 MB limit
/// let middleware = MessageSizeLimitMiddleware::default();
///
/// let (tx, _rx) = mpsc::unbounded_channel();
/// let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
///
/// let small_msg = Message::text("Small".to_string());
/// assert!(middleware.on_message(&conn, small_msg).await.is_ok());
///
/// // Custom limit
/// let strict = MessageSizeLimitMiddleware::new(100);
/// let large_msg = Message::text("x".repeat(200));
/// assert!(strict.on_message(&conn, large_msg).await.is_err());
/// # });
/// ```
pub struct MessageSizeLimitMiddleware {
	max_size: usize,
}

impl MessageSizeLimitMiddleware {
	/// Create a new message size limit middleware with a custom limit
	pub fn new(max_size: usize) -> Self {
		Self { max_size }
	}

	/// Get the configured maximum message size
	pub fn max_size(&self) -> usize {
		self.max_size
	}
}

impl Default for MessageSizeLimitMiddleware {
	/// Create a message size limit middleware with the default 1 MB limit
	fn default() -> Self {
		Self {
			max_size: crate::protocol::DEFAULT_MAX_MESSAGE_SIZE,
		}
	}
}

#[async_trait]
impl MessageMiddleware for MessageSizeLimitMiddleware {
	async fn on_message(
		&self,
		connection: &Arc<WebSocketConnection>,
		message: Message,
	) -> MiddlewareResult<Message> {
		let size = match &message {
			Message::Text { data } => data.len(),
			Message::Binary { data } => data.len(),
			_ => 0,
		};

		if size > self.max_size {
			// Send close frame with 1009 (Message Too Big) before rejecting
			let reason = format!(
				"Message size {} bytes exceeds limit of {} bytes",
				size, self.max_size
			);
			let _ = connection
				.close_with_reason(CLOSE_CODE_MESSAGE_TOO_BIG, reason.clone())
				.await;

			Err(MiddlewareError::MessageRejected(format!(
				"Message size {} exceeds limit {}",
				size, self.max_size
			)))
		} else {
			Ok(message)
		}
	}
}

/// Middleware chain for composing multiple middlewares
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::middleware::{
///     MiddlewareChain, LoggingMiddleware, ConnectionContext, ConnectionMiddleware
/// };
///
/// # tokio_test::block_on(async {
/// let mut chain = MiddlewareChain::new();
/// chain.add_connection_middleware(Box::new(LoggingMiddleware::new("WS".to_string())));
///
/// let mut context = ConnectionContext::new("192.168.1.1".to_string());
/// assert!(chain.process_connect(&mut context).await.is_ok());
/// # });
/// ```
pub struct MiddlewareChain {
	connection_middlewares: Vec<Box<dyn ConnectionMiddleware>>,
	message_middlewares: Vec<Box<dyn MessageMiddleware>>,
}

impl MiddlewareChain {
	/// Create a new middleware chain
	pub fn new() -> Self {
		Self {
			connection_middlewares: Vec::new(),
			message_middlewares: Vec::new(),
		}
	}

	/// Add a connection middleware to the chain
	pub fn add_connection_middleware(&mut self, middleware: Box<dyn ConnectionMiddleware>) {
		self.connection_middlewares.push(middleware);
	}

	/// Add a message middleware to the chain
	pub fn add_message_middleware(&mut self, middleware: Box<dyn MessageMiddleware>) {
		self.message_middlewares.push(middleware);
	}

	/// Process connection through all middlewares
	pub async fn process_connect(&self, context: &mut ConnectionContext) -> MiddlewareResult<()> {
		for middleware in &self.connection_middlewares {
			middleware.on_connect(context).await?;
		}
		Ok(())
	}

	/// Process disconnection through all middlewares
	pub async fn process_disconnect(
		&self,
		connection: &Arc<WebSocketConnection>,
	) -> MiddlewareResult<()> {
		for middleware in &self.connection_middlewares {
			middleware.on_disconnect(connection).await?;
		}
		Ok(())
	}

	/// Process message through all middlewares
	pub async fn process_message(
		&self,
		connection: &Arc<WebSocketConnection>,
		mut message: Message,
	) -> MiddlewareResult<Message> {
		for middleware in &self.message_middlewares {
			message = middleware.on_message(connection, message).await?;
		}
		Ok(message)
	}
}

impl Default for MiddlewareChain {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use tokio::sync::mpsc;

	#[rstest]
	#[tokio::test]
	async fn test_connection_context() {
		// Arrange & Act
		let context = ConnectionContext::new("192.168.1.1".to_string())
			.with_header("User-Agent".to_string(), "Test".to_string())
			.with_metadata("session_id".to_string(), "abc123".to_string());

		// Assert
		assert_eq!(context.ip, "192.168.1.1");
		assert_eq!(context.headers.get("User-Agent").unwrap(), "Test");
		assert_eq!(context.metadata.get("session_id").unwrap(), "abc123");
	}

	#[rstest]
	#[tokio::test]
	async fn test_logging_middleware_connect() {
		// Arrange
		let middleware = LoggingMiddleware::new("Test".to_string());
		let mut context = ConnectionContext::new("192.168.1.1".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_logging_middleware_message() {
		// Arrange
		let middleware = LoggingMiddleware::new("Test".to_string());
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let msg = Message::text("Hello".to_string());

		// Act
		let result = middleware.on_message(&conn, msg).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_ip_filter_whitelist_allowed() {
		// Arrange
		let middleware = IpFilterMiddleware::whitelist(vec!["192.168.1.1".to_string()]);
		let mut context = ConnectionContext::new("192.168.1.1".to_string());

		// Act & Assert
		assert!(middleware.on_connect(&mut context).await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_ip_filter_whitelist_blocked() {
		// Arrange
		let middleware = IpFilterMiddleware::whitelist(vec!["192.168.1.1".to_string()]);
		let mut context = ConnectionContext::new("10.0.0.1".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MiddlewareError::ConnectionRejected(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_ip_filter_blacklist_allowed() {
		// Arrange
		let middleware = IpFilterMiddleware::blacklist(vec!["10.0.0.1".to_string()]);
		let mut context = ConnectionContext::new("192.168.1.1".to_string());

		// Act & Assert
		assert!(middleware.on_connect(&mut context).await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_ip_filter_blacklist_blocked() {
		// Arrange
		let middleware = IpFilterMiddleware::blacklist(vec!["10.0.0.1".to_string()]);
		let mut context = ConnectionContext::new("10.0.0.1".to_string());

		// Act
		let result = middleware.on_connect(&mut context).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_message_size_limit_within_limit() {
		// Arrange
		let middleware = MessageSizeLimitMiddleware::new(100);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let msg = Message::text("Small message".to_string());

		// Act & Assert
		assert!(middleware.on_message(&conn, msg).await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_message_size_limit_exceeds_limit() {
		// Arrange
		let middleware = MessageSizeLimitMiddleware::new(10);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let msg = Message::text("This is a very long message".to_string());

		// Act
		let result = middleware.on_message(&conn, msg).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MiddlewareError::MessageRejected(_)
		));
	}

	#[rstest]
	fn test_message_size_limit_default_is_1mb() {
		// Arrange & Act
		let middleware = MessageSizeLimitMiddleware::default();

		// Assert
		assert_eq!(middleware.max_size(), 1_048_576);
	}

	#[rstest]
	#[tokio::test]
	async fn test_message_size_limit_default_accepts_normal_messages() {
		// Arrange
		let middleware = MessageSizeLimitMiddleware::default();
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		// 10 KB message - well within 1 MB limit
		let msg = Message::text("x".repeat(10_000));

		// Act
		let result = middleware.on_message(&conn, msg).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_message_size_limit_default_rejects_oversized_messages() {
		// Arrange
		let middleware = MessageSizeLimitMiddleware::default();
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		// 2 MB message - exceeds 1 MB limit
		let msg = Message::text("x".repeat(2 * 1024 * 1024));

		// Act
		let result = middleware.on_message(&conn, msg).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_message_size_limit_sends_close_frame_on_rejection() {
		// Arrange
		let middleware = MessageSizeLimitMiddleware::new(10);
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let msg = Message::text("This exceeds the limit".to_string());

		// Act
		let result = middleware.on_message(&conn, msg).await;

		// Assert
		assert!(result.is_err());

		// Verify close frame was sent with code 1009 (Message Too Big)
		let close_msg = rx.recv().await.unwrap();
		match close_msg {
			Message::Close { code, reason } => {
				assert_eq!(code, 1009);
				assert!(reason.contains("exceeds limit"));
			}
			_ => panic!("Expected close message with code 1009"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_message_size_limit_binary_messages() {
		// Arrange
		let middleware = MessageSizeLimitMiddleware::new(100);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));

		// Act - within limit
		let small_binary = Message::binary(vec![0u8; 50]);
		assert!(middleware.on_message(&conn, small_binary).await.is_ok());

		// Act - exceeds limit
		let large_binary = Message::binary(vec![0u8; 200]);
		let result = middleware.on_message(&conn, large_binary).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_message_size_limit_control_frames_always_pass() {
		// Arrange
		let middleware = MessageSizeLimitMiddleware::new(1);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));

		// Act & Assert - control frames (Ping, Pong) should always pass
		assert!(middleware.on_message(&conn, Message::Ping).await.is_ok());
		assert!(middleware.on_message(&conn, Message::Pong).await.is_ok());
	}

	#[rstest]
	fn test_message_size_limit_custom_configuration() {
		// Arrange & Act
		let middleware = MessageSizeLimitMiddleware::new(512 * 1024); // 512 KB

		// Assert
		assert_eq!(middleware.max_size(), 512 * 1024);
	}

	#[rstest]
	#[tokio::test]
	async fn test_middleware_chain_connect() {
		// Arrange
		let mut chain = MiddlewareChain::new();
		chain.add_connection_middleware(Box::new(LoggingMiddleware::new("WS".to_string())));
		let mut context = ConnectionContext::new("192.168.1.1".to_string());

		// Act & Assert
		assert!(chain.process_connect(&mut context).await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_middleware_chain_message() {
		// Arrange
		let mut chain = MiddlewareChain::new();
		chain.add_message_middleware(Box::new(MessageSizeLimitMiddleware::new(100)));
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let msg = Message::text("Test".to_string());

		// Act & Assert
		assert!(chain.process_message(&conn, msg).await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_middleware_chain_rejection() {
		// Arrange
		let mut chain = MiddlewareChain::new();
		chain.add_connection_middleware(Box::new(IpFilterMiddleware::whitelist(vec![
			"192.168.1.1".to_string(),
		])));
		let mut context = ConnectionContext::new("10.0.0.1".to_string());

		// Act & Assert
		assert!(chain.process_connect(&mut context).await.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_middleware_chain_with_default_size_limit() {
		// Arrange
		let mut chain = MiddlewareChain::new();
		chain.add_message_middleware(Box::new(MessageSizeLimitMiddleware::default()));
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));

		// Act - normal message should pass through default chain
		let msg = Message::text("Normal message".to_string());
		let result = chain.process_message(&conn, msg).await;

		// Assert
		assert!(result.is_ok());
	}
}
