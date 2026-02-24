//! WebSocket consumers for advanced message handling patterns
//!
//! This module provides consumer classes inspired by Django Channels,
//! enabling structured WebSocket message handling with lifecycle hooks.
//!
//! # Dependency Injection Support
//!
//! When the `di` feature is enabled, `ConsumerContext` supports dependency injection:
//!
//! ```rust,no_run
//! use reinhardt_websockets::consumers::{ConsumerContext, WebSocketConsumer};
//! use reinhardt_websockets::{Message, WebSocketResult};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! # type DatabaseConnection = ();
//! # type CacheService = ();
//! # struct MyConsumer;
//! #
//! # #[async_trait]
//! # impl WebSocketConsumer for MyConsumer {
//! #     async fn on_connect(&self, _ctx: &mut ConsumerContext) -> WebSocketResult<()> {
//! #         Ok(())
//! #     }
//! #
//! async fn on_message(&self, ctx: &mut ConsumerContext, msg: Message) -> WebSocketResult<()> {
//!     // Resolve dependencies from DI context
//!     // let db: Arc<DatabaseConnection> = ctx.resolve().await?;
//!     // let cache: CacheService = ctx.resolve_uncached().await?;
//!
//!     // Use the dependencies...
//!     Ok(())
//! }
//! #
//! #     async fn on_disconnect(&self, _ctx: &mut ConsumerContext) -> WebSocketResult<()> {
//! #         Ok(())
//! #     }
//! # }
//! ```
// WebSocketError is used in #[cfg(feature = "di")] code
#[allow(unused_imports)]
use crate::connection::{Message, WebSocketConnection, WebSocketError, WebSocketResult};
use async_trait::async_trait;
use std::sync::Arc;

#[cfg(feature = "di")]
use reinhardt_di::{Injectable, Injected, InjectionContext};

/// Consumer context containing connection and message information
///
/// This context is passed to WebSocket consumer methods and provides access to:
/// - The WebSocket connection for sending messages
/// - Metadata for storing request-scoped data
/// - Dependency injection (when the `di` feature is enabled)
pub struct ConsumerContext {
	/// The WebSocket connection
	pub connection: Arc<WebSocketConnection>,
	/// Additional metadata
	pub metadata: std::collections::HashMap<String, String>,
	/// DI context for dependency injection (when `di` feature is enabled)
	#[cfg(feature = "di")]
	di_context: Option<Arc<InjectionContext>>,
}

impl ConsumerContext {
	/// Create a new consumer context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::consumers::ConsumerContext;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));
	/// let context = ConsumerContext::new(conn);
	/// ```
	pub fn new(connection: Arc<WebSocketConnection>) -> Self {
		Self {
			connection,
			metadata: std::collections::HashMap::new(),
			#[cfg(feature = "di")]
			di_context: None,
		}
	}

	/// Create a new consumer context with DI context
	///
	/// This constructor is used when dependency injection is needed in WebSocket handlers.
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_websockets::consumers::ConsumerContext;
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// let singleton = Arc::new(SingletonScope::new());
	/// let di_ctx = Arc::new(InjectionContext::builder(singleton).build());
	/// let context = ConsumerContext::with_di_context(connection, di_ctx);
	/// ```
	#[cfg(feature = "di")]
	pub fn with_di_context(
		connection: Arc<WebSocketConnection>,
		di_context: Arc<InjectionContext>,
	) -> Self {
		Self {
			connection,
			metadata: std::collections::HashMap::new(),
			di_context: Some(di_context),
		}
	}

	/// Add metadata to the context
	pub fn with_metadata(mut self, key: String, value: String) -> Self {
		self.metadata.insert(key, value);
		self
	}

	/// Get metadata value
	pub fn get_metadata(&self, key: &str) -> Option<&String> {
		self.metadata.get(key)
	}

	/// Get the DI context if available
	#[cfg(feature = "di")]
	pub fn di_context(&self) -> Option<&Arc<InjectionContext>> {
		self.di_context.as_ref()
	}

	/// Set the DI context
	#[cfg(feature = "di")]
	pub fn set_di_context(&mut self, ctx: Arc<InjectionContext>) {
		self.di_context = Some(ctx);
	}

	/// Resolve a dependency with caching
	///
	/// This method extracts the dependency from the DI context. The resolved
	/// dependency is cached for the duration of the connection.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The DI context is not set
	/// - The dependency cannot be resolved
	///
	/// # Examples
	///
	/// ```ignore
	/// let db: Arc<DatabaseConnection> = ctx.resolve().await?;
	/// ```
	#[cfg(feature = "di")]
	pub async fn resolve<T>(&self) -> WebSocketResult<T>
	where
		T: Injectable + Clone + Send + Sync + 'static,
	{
		let ctx = self
			.di_context
			.as_ref()
			.ok_or_else(|| WebSocketError::Internal("DI context not available".to_string()))?;

		Injected::<T>::resolve(ctx)
			.await
			.map(|injected| injected.into_inner())
			.map_err(|_| WebSocketError::Internal("dependency resolution failed".to_string()))
	}

	/// Resolve a dependency without caching
	///
	/// This method is similar to `resolve()` but creates a fresh instance
	/// of the dependency each time.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The DI context is not set
	/// - The dependency cannot be resolved
	///
	/// # Examples
	///
	/// ```ignore
	/// let fresh_service: MyService = ctx.resolve_uncached().await?;
	/// ```
	#[cfg(feature = "di")]
	pub async fn resolve_uncached<T>(&self) -> WebSocketResult<T>
	where
		T: Injectable + Clone + Send + Sync + 'static,
	{
		let ctx = self
			.di_context
			.as_ref()
			.ok_or_else(|| WebSocketError::Internal("DI context not available".to_string()))?;

		Injected::<T>::resolve_uncached(ctx)
			.await
			.map(|injected| injected.into_inner())
			.map_err(|_| WebSocketError::Internal("dependency resolution failed".to_string()))
	}

	/// Try to resolve a dependency, returning None if DI context is not available
	///
	/// This is useful for optional dependencies or when you want to gracefully
	/// handle the case where DI is not configured.
	///
	/// # Examples
	///
	/// ```ignore
	/// if let Some(cache) = ctx.try_resolve::<CacheService>().await {
	///     // Use cache
	/// } else {
	///     // Fallback without cache
	/// }
	/// ```
	#[cfg(feature = "di")]
	pub async fn try_resolve<T>(&self) -> Option<T>
	where
		T: Injectable + Clone + Send + Sync + 'static,
	{
		let ctx = self.di_context.as_ref()?;

		Injected::<T>::resolve(ctx)
			.await
			.ok()
			.map(|injected| injected.into_inner())
	}
}

/// WebSocket consumer trait
///
/// Consumers handle the lifecycle of WebSocket connections and messages.
#[async_trait]
pub trait WebSocketConsumer: Send + Sync {
	/// Called when a WebSocket connection is established
	async fn on_connect(&self, context: &mut ConsumerContext) -> WebSocketResult<()>;

	/// Called when a message is received
	async fn on_message(
		&self,
		context: &mut ConsumerContext,
		message: Message,
	) -> WebSocketResult<()>;

	/// Called when a WebSocket connection is closed
	async fn on_disconnect(&self, context: &mut ConsumerContext) -> WebSocketResult<()>;
}

/// Echo consumer that echoes all messages back to the sender
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::consumers::{EchoConsumer, WebSocketConsumer, ConsumerContext};
/// use reinhardt_websockets::{Message, WebSocketConnection};
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let consumer = EchoConsumer::new();
/// let (tx, mut rx) = mpsc::unbounded_channel();
/// let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
/// let mut context = ConsumerContext::new(conn);
///
/// let msg = Message::text("Hello".to_string());
/// consumer.on_message(&mut context, msg).await.unwrap();
///
/// let received = rx.recv().await.unwrap();
/// match received {
///     Message::Text { data } => assert_eq!(data, "Echo: Hello"),
///     _ => panic!("Expected text message"),
/// }
/// # });
/// ```
pub struct EchoConsumer {
	prefix: String,
}

impl EchoConsumer {
	/// Create a new echo consumer
	pub fn new() -> Self {
		Self {
			prefix: "Echo".to_string(),
		}
	}

	/// Create a new echo consumer with custom prefix
	pub fn with_prefix(prefix: String) -> Self {
		Self { prefix }
	}
}

impl Default for EchoConsumer {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl WebSocketConsumer for EchoConsumer {
	async fn on_connect(&self, context: &mut ConsumerContext) -> WebSocketResult<()> {
		context
			.connection
			.send_text(format!("{}: Connection established", self.prefix))
			.await
	}

	async fn on_message(
		&self,
		context: &mut ConsumerContext,
		message: Message,
	) -> WebSocketResult<()> {
		match message {
			Message::Text { data } => {
				context
					.connection
					.send_text(format!("{}: {}", self.prefix, data))
					.await
			}
			Message::Binary { data } => {
				// Validate binary payload and attempt UTF-8 conversion
				match String::from_utf8(data.clone()) {
					Ok(text) => {
						context
							.connection
							.send_text(format!("{}: {}", self.prefix, text))
							.await
					}
					Err(_) => {
						// Non-UTF-8 binary: echo back a summary with byte count
						context
							.connection
							.send_text(format!("{}: binary({} bytes)", self.prefix, data.len()))
							.await
					}
				}
			}
			Message::Close { code, reason } => {
				// Acknowledge close and ensure cleanup
				context
					.connection
					.close_with_reason(code, reason)
					.await
					.ok();
				Ok(())
			}
			_ => Ok(()),
		}
	}

	async fn on_disconnect(&self, _context: &mut ConsumerContext) -> WebSocketResult<()> {
		Ok(())
	}
}

/// Broadcast consumer that broadcasts messages to all connections in a group
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::consumers::{BroadcastConsumer, WebSocketConsumer, ConsumerContext};
/// use reinhardt_websockets::{Message, WebSocketConnection};
/// use reinhardt_websockets::room::Room;
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let room = Arc::new(Room::new("chat".to_string()));
/// let consumer = BroadcastConsumer::new(room.clone());
///
/// let (tx1, mut rx1) = mpsc::unbounded_channel();
/// let (tx2, mut rx2) = mpsc::unbounded_channel();
///
/// let conn1 = Arc::new(WebSocketConnection::new("user1".to_string(), tx1));
/// let conn2 = Arc::new(WebSocketConnection::new("user2".to_string(), tx2));
///
/// room.join("user1".to_string(), conn1.clone()).await.unwrap();
/// room.join("user2".to_string(), conn2.clone()).await.unwrap();
///
/// let mut context = ConsumerContext::new(conn1);
/// let msg = Message::text("Hello everyone".to_string());
///
/// consumer.on_message(&mut context, msg).await.unwrap();
///
/// // Both connections should receive the broadcast
/// assert!(rx1.try_recv().is_ok());
/// assert!(rx2.try_recv().is_ok());
/// # });
/// ```
pub struct BroadcastConsumer {
	room: Arc<crate::room::Room>,
}

impl BroadcastConsumer {
	/// Create a new broadcast consumer
	pub fn new(room: Arc<crate::room::Room>) -> Self {
		Self { room }
	}
}

#[async_trait]
impl WebSocketConsumer for BroadcastConsumer {
	async fn on_connect(&self, context: &mut ConsumerContext) -> WebSocketResult<()> {
		let client_id = context.connection.id().to_string();
		self.room
			.join(client_id.clone(), context.connection.clone())
			.await
			.map_err(|e| crate::connection::WebSocketError::Connection(e.to_string()))?;

		context
			.connection
			.send_text("Joined broadcast room".to_string())
			.await
	}

	async fn on_message(
		&self,
		_context: &mut ConsumerContext,
		message: Message,
	) -> WebSocketResult<()> {
		let result = self.room.broadcast(message).await;
		if result.is_complete_failure() {
			return Err(crate::connection::WebSocketError::Send(
				"broadcast failed for all clients".to_string(),
			));
		}
		Ok(())
	}

	async fn on_disconnect(&self, context: &mut ConsumerContext) -> WebSocketResult<()> {
		let client_id = context.connection.id();
		// Best-effort leave: the client may already have been removed by
		// broadcast failure cleanup, so ignore ClientNotFound errors.
		let _ = self.room.leave(client_id).await;

		// Ensure the connection is marked as closed even on abnormal disconnect
		context.connection.force_close().await;

		Ok(())
	}
}

/// JSON consumer that parses and serializes JSON messages
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::consumers::{JsonConsumer, WebSocketConsumer, ConsumerContext};
/// use reinhardt_websockets::{Message, WebSocketConnection};
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, Debug, PartialEq)]
/// struct ChatMessage {
///     user: String,
///     text: String,
/// }
///
/// # tokio_test::block_on(async {
/// let consumer = JsonConsumer::new();
/// let (tx, mut rx) = mpsc::unbounded_channel();
/// let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
/// let mut context = ConsumerContext::new(conn);
///
/// let msg = ChatMessage {
///     user: "Alice".to_string(),
///     text: "Hello".to_string(),
/// };
///
/// let json_msg = Message::json(&msg).unwrap();
/// consumer.on_message(&mut context, json_msg).await.unwrap();
/// # });
/// ```
pub struct JsonConsumer;

impl JsonConsumer {
	/// Create a new JSON consumer
	pub fn new() -> Self {
		Self
	}
}

impl Default for JsonConsumer {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl WebSocketConsumer for JsonConsumer {
	async fn on_connect(&self, context: &mut ConsumerContext) -> WebSocketResult<()> {
		context
			.connection
			.send_json(&serde_json::json!({
				"type": "connection",
				"status": "connected"
			}))
			.await
	}

	async fn on_message(
		&self,
		context: &mut ConsumerContext,
		message: Message,
	) -> WebSocketResult<()> {
		match message {
			Message::Text { data } => {
				// Try to parse as JSON
				let json: serde_json::Value = serde_json::from_str(&data)
					.map_err(|e| crate::connection::WebSocketError::Protocol(e.to_string()))?;

				// Echo back with metadata
				let response = serde_json::json!({
					"type": "echo",
					"data": json,
					"timestamp": chrono::Utc::now().to_rfc3339()
				});

				context.connection.send_json(&response).await
			}
			Message::Binary { data } => {
				// Validate that binary data is valid UTF-8 JSON
				let text = String::from_utf8(data).map_err(|e| {
					crate::connection::WebSocketError::BinaryPayload(format!(
						"binary payload is not valid UTF-8: {}",
						e
					))
				})?;

				let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
					crate::connection::WebSocketError::BinaryPayload(format!(
						"binary payload is not valid JSON: {}",
						e
					))
				})?;

				let response = serde_json::json!({
					"type": "echo",
					"data": json,
					"source": "binary",
					"timestamp": chrono::Utc::now().to_rfc3339()
				});

				context.connection.send_json(&response).await
			}
			_ => Ok(()),
		}
	}

	async fn on_disconnect(&self, _context: &mut ConsumerContext) -> WebSocketResult<()> {
		Ok(())
	}
}

/// Consumer chain for composing multiple consumers
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::consumers::{ConsumerChain, EchoConsumer, ConsumerContext, WebSocketConsumer};
/// use reinhardt_websockets::WebSocketConnection;
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let mut chain = ConsumerChain::new();
/// chain.add_consumer(Box::new(EchoConsumer::new()));
///
/// let (tx, _rx) = mpsc::unbounded_channel();
/// let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
/// let mut context = ConsumerContext::new(conn);
///
/// assert!(chain.on_connect(&mut context).await.is_ok());
/// # });
/// ```
pub struct ConsumerChain {
	consumers: Vec<Box<dyn WebSocketConsumer>>,
}

impl ConsumerChain {
	/// Create a new consumer chain
	pub fn new() -> Self {
		Self {
			consumers: Vec::new(),
		}
	}

	/// Add a consumer to the chain
	pub fn add_consumer(&mut self, consumer: Box<dyn WebSocketConsumer>) {
		self.consumers.push(consumer);
	}
}

impl Default for ConsumerChain {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl WebSocketConsumer for ConsumerChain {
	async fn on_connect(&self, context: &mut ConsumerContext) -> WebSocketResult<()> {
		for consumer in &self.consumers {
			consumer.on_connect(context).await?;
		}
		Ok(())
	}

	async fn on_message(
		&self,
		context: &mut ConsumerContext,
		message: Message,
	) -> WebSocketResult<()> {
		for consumer in &self.consumers {
			consumer.on_message(context, message.clone()).await?;
		}
		Ok(())
	}

	async fn on_disconnect(&self, context: &mut ConsumerContext) -> WebSocketResult<()> {
		for consumer in &self.consumers {
			consumer.on_disconnect(context).await?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use tokio::sync::mpsc;

	#[rstest]
	#[tokio::test]
	async fn test_consumer_context_creation() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));

		// Act
		let context = ConsumerContext::new(conn);

		// Assert
		assert_eq!(context.connection.id(), "test");
	}

	#[rstest]
	#[tokio::test]
	async fn test_consumer_context_metadata() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));

		// Act
		let context =
			ConsumerContext::new(conn).with_metadata("user_id".to_string(), "123".to_string());

		// Assert
		assert_eq!(context.get_metadata("user_id").unwrap(), "123");
	}

	#[rstest]
	#[tokio::test]
	async fn test_echo_consumer_connect() {
		// Arrange
		let consumer = EchoConsumer::new();
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act
		consumer.on_connect(&mut context).await.unwrap();

		// Assert
		let msg = rx.recv().await.unwrap();
		match msg {
			Message::Text { data } => assert!(data.contains("Connection established")),
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_echo_consumer_message() {
		// Arrange
		let consumer = EchoConsumer::new();
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act
		let msg = Message::text("Hello".to_string());
		consumer.on_message(&mut context, msg).await.unwrap();

		// Assert
		let received = rx.recv().await.unwrap();
		match received {
			Message::Text { data } => assert_eq!(data, "Echo: Hello"),
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_echo_consumer_binary_utf8_message() {
		// Arrange
		let consumer = EchoConsumer::new();
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act - send a valid UTF-8 binary message
		let msg = Message::binary(b"Hello binary".to_vec());
		consumer.on_message(&mut context, msg).await.unwrap();

		// Assert
		let received = rx.recv().await.unwrap();
		match received {
			Message::Text { data } => assert_eq!(data, "Echo: Hello binary"),
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_echo_consumer_binary_non_utf8_message() {
		// Arrange
		let consumer = EchoConsumer::new();
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act - send a non-UTF-8 binary message
		let msg = Message::binary(vec![0xFF, 0xFE, 0xFD]);
		consumer.on_message(&mut context, msg).await.unwrap();

		// Assert
		let received = rx.recv().await.unwrap();
		match received {
			Message::Text { data } => assert_eq!(data, "Echo: binary(3 bytes)"),
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_echo_consumer_handles_close_message() {
		// Arrange
		let consumer = EchoConsumer::new();
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn.clone());

		// Act
		let msg = Message::Close {
			code: 1000,
			reason: "Normal closure".to_string(),
		};
		consumer.on_message(&mut context, msg).await.unwrap();

		// Assert - connection should be closed
		assert!(conn.is_closed().await);

		// The close frame should have been sent
		let received = rx.recv().await.unwrap();
		assert!(matches!(received, Message::Close { code: 1000, .. }));
	}

	#[rstest]
	#[tokio::test]
	async fn test_json_consumer_connect() {
		// Arrange
		let consumer = JsonConsumer::new();
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act
		consumer.on_connect(&mut context).await.unwrap();

		// Assert
		let msg = rx.recv().await.unwrap();
		match msg {
			Message::Text { data } => {
				let json: serde_json::Value = serde_json::from_str(&data).unwrap();
				assert_eq!(json["status"], "connected");
			}
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_json_consumer_binary_valid_json() {
		// Arrange
		let consumer = JsonConsumer::new();
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act - send valid JSON as binary
		let msg = Message::binary(br#"{"key":"value"}"#.to_vec());
		consumer.on_message(&mut context, msg).await.unwrap();

		// Assert
		let received = rx.recv().await.unwrap();
		match received {
			Message::Text { data } => {
				let json: serde_json::Value = serde_json::from_str(&data).unwrap();
				assert_eq!(json["source"], "binary");
				assert_eq!(json["data"]["key"], "value");
			}
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_json_consumer_binary_invalid_utf8_returns_error() {
		// Arrange
		let consumer = JsonConsumer::new();
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act - send non-UTF-8 binary
		let msg = Message::binary(vec![0xFF, 0xFE]);
		let result = consumer.on_message(&mut context, msg).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, WebSocketError::BinaryPayload(_)));
		assert!(err.to_string().contains("not valid UTF-8"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_json_consumer_binary_invalid_json_returns_error() {
		// Arrange
		let consumer = JsonConsumer::new();
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act - send valid UTF-8 but invalid JSON as binary
		let msg = Message::binary(b"not json at all".to_vec());
		let result = consumer.on_message(&mut context, msg).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, WebSocketError::BinaryPayload(_)));
		assert!(err.to_string().contains("not valid JSON"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_consumer_disconnect_cleanup() {
		// Arrange
		let room = Arc::new(crate::room::Room::new("cleanup".to_string()));
		let consumer = BroadcastConsumer::new(room.clone());
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("user1".to_string(), tx));
		room.join("user1".to_string(), conn.clone()).await.unwrap();
		let mut context = ConsumerContext::new(conn.clone());

		// Act
		consumer.on_disconnect(&mut context).await.unwrap();

		// Assert - connection is force-closed and removed from room
		assert!(conn.is_closed().await);
		assert!(!room.has_client("user1").await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_consumer_disconnect_tolerates_already_removed() {
		// Arrange - client not in room (e.g., already removed by broadcast cleanup)
		let room = Arc::new(crate::room::Room::new("tolerant".to_string()));
		let consumer = BroadcastConsumer::new(room.clone());
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("ghost".to_string(), tx));
		let mut context = ConsumerContext::new(conn.clone());

		// Act - should not error even though client is not in the room
		let result = consumer.on_disconnect(&mut context).await;

		// Assert
		assert!(result.is_ok());
		assert!(conn.is_closed().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_consumer_chain() {
		// Arrange
		let mut chain = ConsumerChain::new();
		chain.add_consumer(Box::new(EchoConsumer::with_prefix("Consumer1".to_string())));

		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("test".to_string(), tx));
		let mut context = ConsumerContext::new(conn);

		// Act & Assert
		assert!(chain.on_connect(&mut context).await.is_ok());
	}
}
