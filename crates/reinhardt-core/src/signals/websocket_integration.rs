//! WebSocket Integration - Real-time signal propagation to clients
//!
//! This module provides WebSocket integration for signals, allowing real-time
//! signal propagation to connected WebSocket clients.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt_core::signals::websocket_integration::WebSocketSignalBridge;
//! use reinhardt_core::signals::post_save;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), reinhardt_core::signals::SignalError> {
//! # #[derive(Clone, serde::Serialize)]
//! # struct User;
//! # let user = User;
//! // Create a WebSocket bridge
//! let bridge = WebSocketSignalBridge::new();
//!
//! // Connect signals to WebSocket broadcast
//! bridge.connect_signal(post_save::<User>(), "user.saved").await;
//!
//! // When a signal is emitted, it will be broadcast to WebSocket clients
//! post_save::<User>().send(user).await?;
//! # Ok(())
//! # }
//! ```

use super::error::SignalError;
use super::signal::Signal;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// WebSocket message format for signal events
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::websocket_integration::WebSocketMessage;
/// use serde_json::json;
///
/// let msg = WebSocketMessage::new("user.created", json!({"id": 123}));
/// assert_eq!(msg.event_type, "user.created");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage<T> {
	/// Event type identifier
	pub event_type: String,
	/// Event payload
	pub payload: T,
	/// Message timestamp (Unix timestamp in milliseconds)
	pub timestamp: u64,
}

impl<T> WebSocketMessage<T> {
	/// Create a new WebSocket message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::WebSocketMessage;
	///
	/// let msg = WebSocketMessage::new("notification", "Hello");
	/// assert_eq!(msg.event_type, "notification");
	/// assert_eq!(msg.payload, "Hello");
	/// ```
	pub fn new(event_type: impl Into<String>, payload: T) -> Self {
		use std::time::{SystemTime, UNIX_EPOCH};

		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap()
			.as_millis() as u64;

		Self {
			event_type: event_type.into(),
			payload,
			timestamp,
		}
	}
}

/// WebSocket client connection trait
///
/// Implement this trait to integrate with your WebSocket server
pub trait WebSocketClient: Send + Sync {
	/// Send a message to this client
	fn send_message(&self, message: String) -> Result<(), SignalError>;

	/// Get the client ID
	fn client_id(&self) -> &str;

	/// Check if the client is still connected
	fn is_connected(&self) -> bool;
}

/// In-memory WebSocket client for testing
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::websocket_integration::{MockWebSocketClient, WebSocketClient};
///
/// let client = MockWebSocketClient::new("client-1");
/// assert_eq!(client.client_id(), "client-1");
/// assert!(client.is_connected());
/// ```
pub struct MockWebSocketClient {
	id: String,
	messages: Arc<RwLock<Vec<String>>>,
	connected: Arc<RwLock<bool>>,
}

impl MockWebSocketClient {
	/// Create a new mock WebSocket client
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::MockWebSocketClient;
	///
	/// let client = MockWebSocketClient::new("test-client");
	/// ```
	pub fn new(id: impl Into<String>) -> Self {
		Self {
			id: id.into(),
			messages: Arc::new(RwLock::new(Vec::new())),
			connected: Arc::new(RwLock::new(true)),
		}
	}

	/// Get all messages received by this client
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::{MockWebSocketClient, WebSocketClient};
	///
	/// let client = MockWebSocketClient::new("test");
	/// client.send_message("Hello".to_string()).unwrap();
	///
	/// let messages = client.messages();
	/// assert_eq!(messages.len(), 1);
	/// assert_eq!(messages[0], "Hello");
	/// ```
	pub fn messages(&self) -> Vec<String> {
		self.messages.read().clone()
	}

	/// Disconnect this client
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::{MockWebSocketClient, WebSocketClient};
	///
	/// let client = MockWebSocketClient::new("test");
	/// assert!(client.is_connected());
	///
	/// client.disconnect();
	/// assert!(!client.is_connected());
	/// ```
	pub fn disconnect(&self) {
		*self.connected.write() = false;
	}
}

impl WebSocketClient for MockWebSocketClient {
	fn send_message(&self, message: String) -> Result<(), SignalError> {
		if !self.is_connected() {
			return Err(SignalError::new("Client is disconnected"));
		}
		self.messages.write().push(message);
		Ok(())
	}

	fn client_id(&self) -> &str {
		&self.id
	}

	fn is_connected(&self) -> bool {
		*self.connected.read()
	}
}

/// WebSocket signal bridge
///
/// Bridges signals to WebSocket clients, broadcasting signal events
/// to connected clients in real-time
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::websocket_integration::WebSocketSignalBridge;
///
/// let bridge = WebSocketSignalBridge::new();
/// ```
pub struct WebSocketSignalBridge {
	clients: Arc<RwLock<HashMap<String, Arc<dyn WebSocketClient>>>>,
}

impl WebSocketSignalBridge {
	/// Create a new WebSocket signal bridge
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::WebSocketSignalBridge;
	///
	/// let bridge = WebSocketSignalBridge::new();
	/// ```
	pub fn new() -> Self {
		Self {
			clients: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Add a WebSocket client to the bridge
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::{WebSocketSignalBridge, MockWebSocketClient};
	/// use std::sync::Arc;
	///
	/// let bridge = WebSocketSignalBridge::new();
	/// let client = Arc::new(MockWebSocketClient::new("client-1"));
	/// bridge.add_client(client);
	///
	/// assert_eq!(bridge.client_count(), 1);
	/// ```
	pub fn add_client(&self, client: Arc<dyn WebSocketClient>) {
		self.clients
			.write()
			.insert(client.client_id().to_string(), client);
	}

	/// Remove a WebSocket client from the bridge
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::{WebSocketSignalBridge, MockWebSocketClient};
	/// use std::sync::Arc;
	///
	/// let bridge = WebSocketSignalBridge::new();
	/// let client = Arc::new(MockWebSocketClient::new("client-1"));
	/// bridge.add_client(client);
	///
	/// bridge.remove_client("client-1");
	/// assert_eq!(bridge.client_count(), 0);
	/// ```
	pub fn remove_client(&self, client_id: &str) {
		self.clients.write().remove(client_id);
	}

	/// Get the number of connected clients
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::WebSocketSignalBridge;
	///
	/// let bridge = WebSocketSignalBridge::new();
	/// assert_eq!(bridge.client_count(), 0);
	/// ```
	pub fn client_count(&self) -> usize {
		self.clients.read().len()
	}

	/// Broadcast a message to all connected clients
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::{WebSocketSignalBridge, MockWebSocketClient};
	/// use std::sync::Arc;
	///
	/// let bridge = WebSocketSignalBridge::new();
	/// let client = Arc::new(MockWebSocketClient::new("client-1"));
	/// bridge.add_client(client.clone());
	///
	/// bridge.broadcast("Hello all".to_string()).unwrap();
	///
	/// let messages = client.messages();
	/// assert_eq!(messages.len(), 1);
	/// ```
	pub fn broadcast(&self, message: String) -> Result<(), SignalError> {
		let clients = self.clients.read();
		let mut errors = Vec::new();

		for client in clients.values() {
			if client.is_connected()
				&& let Err(e) = client.send_message(message.clone())
			{
				errors.push(e);
			}
		}

		if !errors.is_empty() {
			return Err(SignalError::new(format!(
				"Failed to send to {} clients",
				errors.len()
			)));
		}

		Ok(())
	}

	/// Connect a signal to WebSocket broadcast
	///
	/// When the signal is emitted, it will be serialized and broadcast to all clients
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_core::signals::websocket_integration::WebSocketSignalBridge;
	/// use reinhardt_core::signals::post_save;
	/// # use serde::{Serialize, Deserialize};
	///
	/// # #[tokio::main]
	/// # async fn main() {
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// let bridge = WebSocketSignalBridge::new();
	/// bridge.connect_signal(post_save::<User>(), "user.saved").await;
	/// # }
	/// ```
	pub async fn connect_signal<T>(&self, signal: Signal<T>, event_type: impl Into<String>)
	where
		T: Serialize + Send + Sync + 'static,
	{
		let clients = Arc::clone(&self.clients);
		let event_type = event_type.into();

		signal.connect(move |instance| {
			let clients = Arc::clone(&clients);
			let event_type = event_type.clone();

			async move {
				let message = WebSocketMessage::new(&event_type, &*instance);
				let json = serde_json::to_string(&message)
					.map_err(|e| SignalError::new(format!("Serialization error: {}", e)))?;

				let clients_read = clients.read();
				for client in clients_read.values() {
					if client.is_connected()
						&& let Err(e) = client.send_message(json.clone())
					{
						eprintln!("Failed to send WebSocket message: {}", e);
					}
				}

				Ok(())
			}
		});
	}

	/// Clean up disconnected clients
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::{WebSocketSignalBridge, MockWebSocketClient};
	/// use std::sync::Arc;
	///
	/// let bridge = WebSocketSignalBridge::new();
	/// let client = Arc::new(MockWebSocketClient::new("client-1"));
	/// bridge.add_client(client.clone());
	///
	/// client.disconnect();
	/// bridge.cleanup_disconnected();
	///
	/// assert_eq!(bridge.client_count(), 0);
	/// ```
	pub fn cleanup_disconnected(&self) {
		let mut clients = self.clients.write();
		clients.retain(|_, client| client.is_connected());
	}
}

impl Default for WebSocketSignalBridge {
	fn default() -> Self {
		Self::new()
	}
}

impl Clone for WebSocketSignalBridge {
	fn clone(&self) -> Self {
		Self {
			clients: Arc::clone(&self.clients),
		}
	}
}

impl fmt::Debug for WebSocketSignalBridge {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("WebSocketSignalBridge")
			.field("client_count", &self.client_count())
			.finish()
	}
}

/// Typed WebSocket signal broadcaster
///
/// A type-safe wrapper for broadcasting specific signal types to WebSocket clients
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::websocket_integration::{WebSocketSignalBridge, TypedWebSocketBroadcaster};
///
/// let bridge = WebSocketSignalBridge::new();
/// let broadcaster = TypedWebSocketBroadcaster::<String>::new(bridge, "string_event");
/// ```
pub struct TypedWebSocketBroadcaster<T>
where
	T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
	bridge: WebSocketSignalBridge,
	event_type: String,
	_phantom: PhantomData<T>,
}

impl<T> TypedWebSocketBroadcaster<T>
where
	T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
	/// Create a new typed broadcaster
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::{WebSocketSignalBridge, TypedWebSocketBroadcaster};
	///
	/// let bridge = WebSocketSignalBridge::new();
	/// let broadcaster = TypedWebSocketBroadcaster::<String>::new(bridge, "test");
	/// ```
	pub fn new(bridge: WebSocketSignalBridge, event_type: impl Into<String>) -> Self {
		Self {
			bridge,
			event_type: event_type.into(),
			_phantom: PhantomData,
		}
	}

	/// Broadcast a typed message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::websocket_integration::{WebSocketSignalBridge, TypedWebSocketBroadcaster, MockWebSocketClient};
	/// use std::sync::Arc;
	///
	/// let bridge = WebSocketSignalBridge::new();
	/// let client = Arc::new(MockWebSocketClient::new("client-1"));
	/// bridge.add_client(client.clone());
	///
	/// let broadcaster = TypedWebSocketBroadcaster::new(bridge, "message");
	/// broadcaster.broadcast("Hello".to_string()).unwrap();
	///
	/// let messages = client.messages();
	/// assert_eq!(messages.len(), 1);
	/// ```
	pub fn broadcast(&self, payload: T) -> Result<(), SignalError> {
		let message = WebSocketMessage::new(&self.event_type, payload);
		let json = serde_json::to_string(&message)
			.map_err(|e| SignalError::new(format!("Serialization error: {}", e)))?;

		self.bridge.broadcast(json)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::Arc;

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestPayload {
		message: String,
	}

	#[test]
	fn test_websocket_message_creation() {
		let msg = WebSocketMessage::new("test", "payload");
		assert_eq!(msg.event_type, "test");
		assert_eq!(msg.payload, "payload");
		assert!(msg.timestamp > 0);
	}

	#[test]
	fn test_mock_websocket_client() {
		let client = MockWebSocketClient::new("test-client");
		assert_eq!(client.client_id(), "test-client");
		assert!(client.is_connected());

		client.send_message("Hello".to_string()).unwrap();
		let messages = client.messages();
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0], "Hello");
	}

	#[test]
	fn test_mock_client_disconnect() {
		let client = MockWebSocketClient::new("test");
		client.disconnect();
		assert!(!client.is_connected());

		let result = client.send_message("test".to_string());
		assert!(result.is_err());
	}

	#[test]
	fn test_websocket_bridge_add_remove_client() {
		let bridge = WebSocketSignalBridge::new();
		let client = Arc::new(MockWebSocketClient::new("client-1"));

		bridge.add_client(client.clone());
		assert_eq!(bridge.client_count(), 1);

		bridge.remove_client("client-1");
		assert_eq!(bridge.client_count(), 0);
	}

	#[test]
	fn test_websocket_bridge_broadcast() {
		let bridge = WebSocketSignalBridge::new();

		let client1 = Arc::new(MockWebSocketClient::new("client-1"));
		let client2 = Arc::new(MockWebSocketClient::new("client-2"));

		bridge.add_client(client1.clone());
		bridge.add_client(client2.clone());

		bridge.broadcast("Test message".to_string()).unwrap();

		assert_eq!(client1.messages().len(), 1);
		assert_eq!(client2.messages().len(), 1);
		assert_eq!(client1.messages()[0], "Test message");
	}

	#[tokio::test]
	async fn test_websocket_bridge_connect_signal() {
		let bridge = WebSocketSignalBridge::new();
		let client = Arc::new(MockWebSocketClient::new("client-1"));
		bridge.add_client(client.clone());

		let signal = Signal::new(crate::signals::SignalName::custom("test_signal"));
		bridge.connect_signal(signal.clone(), "test.event").await;

		signal.send("test payload".to_string()).await.unwrap();

		let messages = client.messages();
		assert_eq!(messages.len(), 1);

		let parsed: WebSocketMessage<String> = serde_json::from_str(&messages[0]).unwrap();
		assert_eq!(parsed.event_type, "test.event");
	}

	#[test]
	fn test_websocket_bridge_cleanup_disconnected() {
		let bridge = WebSocketSignalBridge::new();

		let client1 = Arc::new(MockWebSocketClient::new("client-1"));
		let client2 = Arc::new(MockWebSocketClient::new("client-2"));

		bridge.add_client(client1.clone());
		bridge.add_client(client2.clone());

		assert_eq!(bridge.client_count(), 2);

		client1.disconnect();
		bridge.cleanup_disconnected();

		assert_eq!(bridge.client_count(), 1);
	}

	#[test]
	fn test_typed_websocket_broadcaster() {
		let bridge = WebSocketSignalBridge::new();
		let client = Arc::new(MockWebSocketClient::new("client-1"));
		bridge.add_client(client.clone());

		let broadcaster = TypedWebSocketBroadcaster::new(bridge, "typed.event");

		let payload = TestPayload {
			message: "Hello".to_string(),
		};

		broadcaster.broadcast(payload.clone()).unwrap();

		let messages = client.messages();
		assert_eq!(messages.len(), 1);

		let parsed: WebSocketMessage<TestPayload> = serde_json::from_str(&messages[0]).unwrap();
		assert_eq!(parsed.event_type, "typed.event");
		assert_eq!(parsed.payload, payload);
	}
}
