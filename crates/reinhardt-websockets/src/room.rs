//! WebSocket room management with advanced features
//!
//! This module provides room-based WebSocket connection management,
//! including client tracking, metadata storage, and targeted messaging.

use crate::connection::{Message, WebSocketConnection, WebSocketError};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Error types for room operations
#[derive(Debug, thiserror::Error)]
pub enum RoomError {
	#[error("Client not found")]
	ClientNotFound(String),
	#[error("Room not found")]
	RoomNotFound(String),
	#[error("Client already exists")]
	ClientAlreadyExists(String),
	#[error("WebSocket error")]
	WebSocket(#[from] WebSocketError),
	#[error("Metadata error")]
	Metadata(String),
}

pub type RoomResult<T> = Result<T, RoomError>;

/// Result of a broadcast operation that tracks individual send outcomes.
///
/// This provides detailed information about which clients received the message
/// and which failed, enabling callers to take appropriate action (e.g., logging,
/// notifying administrators, or retrying).
///
/// Dead connections that fail during broadcast are automatically removed from
/// the room.
#[derive(Debug)]
pub struct BroadcastResult {
	/// Client IDs that received the message successfully
	pub successful: Vec<String>,
	/// Client IDs that failed with their respective errors
	pub failed: Vec<(String, WebSocketError)>,
}

impl BroadcastResult {
	/// Returns `true` if all sends were successful (no failures).
	pub fn is_complete_success(&self) -> bool {
		self.failed.is_empty()
	}

	/// Returns `true` if at least one send was successful.
	pub fn is_partial_success(&self) -> bool {
		!self.successful.is_empty()
	}

	/// Returns `true` if all sends failed.
	pub fn is_complete_failure(&self) -> bool {
		self.successful.is_empty() && !self.failed.is_empty()
	}

	/// Returns the number of failed sends.
	pub fn failure_count(&self) -> usize {
		self.failed.len()
	}

	/// Returns the IDs of clients that failed to receive the message.
	pub fn failed_client_ids(&self) -> Vec<&str> {
		self.failed.iter().map(|(id, _)| id.as_str()).collect()
	}
}

/// A WebSocket room that manages multiple client connections
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::room::Room;
/// use reinhardt_websockets::WebSocketConnection;
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let room = Room::new("chat_room".to_string());
///
/// let (tx, _rx) = mpsc::unbounded_channel();
/// let client = Arc::new(WebSocketConnection::new("user1".to_string(), tx));
///
/// room.join("user1".to_string(), client).await.unwrap();
/// assert_eq!(room.client_count().await, 1);
/// # });
/// ```
pub struct Room {
	id: String,
	clients: Arc<RwLock<HashMap<String, Arc<WebSocketConnection>>>>,
	metadata: Arc<RwLock<HashMap<String, Value>>>,
}

impl Room {
	/// Create a new room with the given ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	///
	/// let room = Room::new("general".to_string());
	/// assert_eq!(room.id(), "general");
	/// ```
	pub fn new(id: String) -> Self {
		Self {
			id,
			clients: Arc::new(RwLock::new(HashMap::new())),
			metadata: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Get the room ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	///
	/// let room = Room::new("lobby".to_string());
	/// assert_eq!(room.id(), "lobby");
	/// ```
	pub fn id(&self) -> &str {
		&self.id
	}

	/// Add a client to the room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("chat".to_string());
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let client = Arc::new(WebSocketConnection::new("alice".to_string(), tx));
	///
	/// room.join("alice".to_string(), client).await.unwrap();
	/// assert!(room.has_client("alice").await);
	/// # });
	/// ```
	pub async fn join(
		&self,
		client_id: String,
		client: Arc<WebSocketConnection>,
	) -> RoomResult<()> {
		let mut clients = self.clients.write().await;

		if clients.contains_key(&client_id) {
			return Err(RoomError::ClientAlreadyExists(client_id));
		}

		clients.insert(client_id, client);
		Ok(())
	}

	/// Remove a client from the room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("chat".to_string());
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let client = Arc::new(WebSocketConnection::new("bob".to_string(), tx));
	///
	/// room.join("bob".to_string(), client).await.unwrap();
	/// assert!(room.has_client("bob").await);
	///
	/// room.leave("bob").await.unwrap();
	/// assert!(!room.has_client("bob").await);
	/// # });
	/// ```
	pub async fn leave(&self, client_id: &str) -> RoomResult<()> {
		let mut clients = self.clients.write().await;

		clients
			.remove(client_id)
			.ok_or_else(|| RoomError::ClientNotFound(client_id.to_string()))?;

		Ok(())
	}

	/// Broadcast a message to all clients in the room.
	///
	/// Returns a [`BroadcastResult`] that reports which clients received the
	/// message and which failed. Dead connections that fail during broadcast
	/// are automatically removed from the room.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("chat".to_string());
	///
	/// let (tx1, _rx1) = mpsc::unbounded_channel();
	/// let (tx2, _rx2) = mpsc::unbounded_channel();
	/// let client1 = Arc::new(WebSocketConnection::new("user1".to_string(), tx1));
	/// let client2 = Arc::new(WebSocketConnection::new("user2".to_string(), tx2));
	///
	/// room.join("user1".to_string(), client1).await.unwrap();
	/// room.join("user2".to_string(), client2).await.unwrap();
	///
	/// let msg = Message::text("Hello everyone!".to_string());
	/// let result = room.broadcast(msg).await;
	///
	/// assert!(result.is_complete_success());
	/// assert_eq!(result.successful.len(), 2);
	/// # });
	/// ```
	pub async fn broadcast(&self, message: Message) -> BroadcastResult {
		let clients = self.clients.read().await;

		let mut successful = Vec::new();
		let mut failed = Vec::new();

		for (client_id, client) in clients.iter() {
			match client.send(message.clone()).await {
				Ok(()) => successful.push(client_id.clone()),
				Err(e) => failed.push((client_id.clone(), e)),
			}
		}

		// Drop read lock before acquiring write lock
		drop(clients);

		// Automatically remove dead connections from the room
		if !failed.is_empty() {
			let mut clients_write = self.clients.write().await;
			for (client_id, _) in &failed {
				clients_write.remove(client_id);
			}
		}

		BroadcastResult { successful, failed }
	}

	/// Broadcasts a message to all clients with a per-client send timeout.
	///
	/// Slow consumers that do not accept the message within the given timeout
	/// are treated as failed and automatically removed from the room, applying
	/// backpressure to prevent slow receivers from blocking the entire broadcast.
	///
	/// # Arguments
	///
	/// * `message` - The message to broadcast
	/// * `send_timeout` - Maximum time to wait for each client to accept the message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("chat".to_string());
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let client = Arc::new(WebSocketConnection::new("user1".to_string(), tx));
	///
	/// room.join("user1".to_string(), client).await.unwrap();
	///
	/// let msg = Message::text("Hello!".to_string());
	/// let result = room.broadcast_with_timeout(msg, Duration::from_secs(5)).await;
	///
	/// assert!(result.is_complete_success());
	/// # });
	/// ```
	pub async fn broadcast_with_timeout(
		&self,
		message: Message,
		send_timeout: Duration,
	) -> BroadcastResult {
		let clients = self.clients.read().await;

		let mut successful = Vec::new();
		let mut failed = Vec::new();

		for (client_id, client) in clients.iter() {
			let send_future = client.send(message.clone());
			match tokio::time::timeout(send_timeout, send_future).await {
				Ok(Ok(())) => successful.push(client_id.clone()),
				Ok(Err(e)) => failed.push((client_id.clone(), e)),
				Err(_elapsed) => {
					failed.push((
						client_id.clone(),
						WebSocketError::SlowConsumer(send_timeout),
					));
				}
			}
		}

		// Drop read lock before acquiring write lock
		drop(clients);

		// Automatically remove failed connections from the room
		if !failed.is_empty() {
			let mut clients_write = self.clients.write().await;
			for (client_id, _) in &failed {
				clients_write.remove(client_id);
			}
		}

		BroadcastResult { successful, failed }
	}

	/// Send a message to a specific client
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("private".to_string());
	///
	/// let (tx, mut rx) = mpsc::unbounded_channel();
	/// let client = Arc::new(WebSocketConnection::new("charlie".to_string(), tx));
	///
	/// room.join("charlie".to_string(), client).await.unwrap();
	///
	/// let msg = Message::text("Private message".to_string());
	/// room.send_to("charlie", msg).await.unwrap();
	///
	/// let received = rx.recv().await.unwrap();
	/// match received {
	///     Message::Text { data } => assert_eq!(data, "Private message"),
	///     _ => panic!("Expected text message"),
	/// }
	/// # });
	/// ```
	pub async fn send_to(&self, client_id: &str, message: Message) -> RoomResult<()> {
		let clients = self.clients.read().await;

		let client = clients
			.get(client_id)
			.ok_or_else(|| RoomError::ClientNotFound(client_id.to_string()))?;

		client.send(message).await?;

		Ok(())
	}

	/// Get the number of clients in the room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("game".to_string());
	/// assert_eq!(room.client_count().await, 0);
	///
	/// let (tx1, _rx1) = mpsc::unbounded_channel();
	/// let (tx2, _rx2) = mpsc::unbounded_channel();
	///
	/// let client1 = Arc::new(WebSocketConnection::new("player1".to_string(), tx1));
	/// let client2 = Arc::new(WebSocketConnection::new("player2".to_string(), tx2));
	///
	/// room.join("player1".to_string(), client1).await.unwrap();
	/// room.join("player2".to_string(), client2).await.unwrap();
	///
	/// assert_eq!(room.client_count().await, 2);
	/// # });
	/// ```
	pub async fn client_count(&self) -> usize {
		let clients = self.clients.read().await;
		clients.len()
	}

	/// Get all client IDs in the room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("meeting".to_string());
	///
	/// let (tx1, _rx1) = mpsc::unbounded_channel();
	/// let (tx2, _rx2) = mpsc::unbounded_channel();
	///
	/// let client1 = Arc::new(WebSocketConnection::new("dave".to_string(), tx1));
	/// let client2 = Arc::new(WebSocketConnection::new("eve".to_string(), tx2));
	///
	/// room.join("dave".to_string(), client1).await.unwrap();
	/// room.join("eve".to_string(), client2).await.unwrap();
	///
	/// let ids = room.client_ids().await;
	/// assert_eq!(ids.len(), 2);
	/// assert!(ids.contains(&"dave".to_string()));
	/// assert!(ids.contains(&"eve".to_string()));
	/// # });
	/// ```
	pub async fn client_ids(&self) -> Vec<String> {
		let clients = self.clients.read().await;
		clients.keys().cloned().collect()
	}

	/// Check if a client is in the room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("support".to_string());
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let client = Arc::new(WebSocketConnection::new("frank".to_string(), tx));
	///
	/// room.join("frank".to_string(), client).await.unwrap();
	///
	/// assert!(room.has_client("frank").await);
	/// assert!(!room.has_client("grace").await);
	/// # });
	/// ```
	pub async fn has_client(&self, client_id: &str) -> bool {
		let clients = self.clients.read().await;
		clients.contains_key(client_id)
	}

	/// Set metadata for the room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("config".to_string());
	///
	/// room.set_metadata("max_users", json!(10)).await.unwrap();
	/// room.set_metadata("topic", json!("General Discussion")).await.unwrap();
	///
	/// let max_users: i64 = room.get_metadata("max_users").await.unwrap().unwrap();
	/// assert_eq!(max_users, 10);
	/// # });
	/// ```
	pub async fn set_metadata<T: serde::Serialize>(&self, key: &str, value: T) -> RoomResult<()> {
		let json_value =
			serde_json::to_value(value).map_err(|e| RoomError::Metadata(e.to_string()))?;

		let mut metadata = self.metadata.write().await;
		metadata.insert(key.to_string(), json_value);

		Ok(())
	}

	/// Get metadata from the room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("data".to_string());
	///
	/// room.set_metadata("counter", json!(42)).await.unwrap();
	///
	/// let counter: i64 = room.get_metadata("counter").await.unwrap().unwrap();
	/// assert_eq!(counter, 42);
	///
	/// let missing: Option<String> = room.get_metadata("nonexistent").await.unwrap();
	/// assert!(missing.is_none());
	/// # });
	/// ```
	pub async fn get_metadata<T>(&self, key: &str) -> RoomResult<Option<T>>
	where
		T: serde::de::DeserializeOwned,
	{
		let metadata = self.metadata.read().await;

		metadata
			.get(key)
			.map(|v| serde_json::from_value(v.clone()))
			.transpose()
			.map_err(|e| RoomError::Metadata(e.to_string()))
	}

	/// Remove metadata from the room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("temp".to_string());
	///
	/// room.set_metadata("temp_key", json!("temp_value")).await.unwrap();
	/// assert!(room.get_metadata::<String>("temp_key").await.unwrap().is_some());
	///
	/// room.remove_metadata("temp_key").await;
	/// assert!(room.get_metadata::<String>("temp_key").await.unwrap().is_none());
	/// # });
	/// ```
	pub async fn remove_metadata(&self, key: &str) -> Option<Value> {
		let mut metadata = self.metadata.write().await;
		metadata.remove(key)
	}

	/// Clear all metadata
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("reset".to_string());
	///
	/// room.set_metadata("key1", json!("value1")).await.unwrap();
	/// room.set_metadata("key2", json!("value2")).await.unwrap();
	///
	/// room.clear_metadata().await;
	///
	/// assert!(room.get_metadata::<String>("key1").await.unwrap().is_none());
	/// assert!(room.get_metadata::<String>("key2").await.unwrap().is_none());
	/// # });
	/// ```
	pub async fn clear_metadata(&self) {
		let mut metadata = self.metadata.write().await;
		metadata.clear();
	}

	/// Check if room is empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::Room;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let room = Room::new("empty_check".to_string());
	/// assert!(room.is_empty().await);
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let client = Arc::new(WebSocketConnection::new("henry".to_string(), tx));
	///
	/// room.join("henry".to_string(), client).await.unwrap();
	/// assert!(!room.is_empty().await);
	/// # });
	/// ```
	pub async fn is_empty(&self) -> bool {
		let clients = self.clients.read().await;
		clients.is_empty()
	}
}

/// Manages multiple WebSocket rooms
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::room::RoomManager;
///
/// # tokio_test::block_on(async {
/// let manager = RoomManager::new();
///
/// let room = manager.create_room("lobby".to_string()).await;
/// assert_eq!(room.id(), "lobby");
/// assert_eq!(manager.room_count().await, 1);
/// # });
/// ```
pub struct RoomManager {
	rooms: Arc<RwLock<HashMap<String, Arc<Room>>>>,
}

impl RoomManager {
	/// Create a new RoomManager
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// let manager = RoomManager::new();
	/// # tokio_test::block_on(async {
	/// assert_eq!(manager.room_count().await, 0);
	/// # });
	/// ```
	pub fn new() -> Self {
		Self {
			rooms: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Create a new room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// let room = manager.create_room("game_room".to_string()).await;
	/// assert_eq!(room.id(), "game_room");
	/// # });
	/// ```
	pub async fn create_room(&self, id: String) -> Arc<Room> {
		let mut rooms = self.rooms.write().await;

		let room = Arc::new(Room::new(id.clone()));
		rooms.insert(id, room.clone());

		room
	}

	/// Get an existing room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("test".to_string()).await;
	///
	/// let room = manager.get_room("test").await;
	/// assert!(room.is_some());
	/// assert_eq!(room.unwrap().id(), "test");
	/// # });
	/// ```
	pub async fn get_room(&self, id: &str) -> Option<Arc<Room>> {
		let rooms = self.rooms.read().await;
		rooms.get(id).cloned()
	}

	/// Get or create a room
	///
	/// This method uses a single write lock to avoid TOCTOU race conditions
	/// that could occur with separate get and create operations.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	///
	/// let room1 = manager.get_or_create_room("auto".to_string()).await;
	/// let room2 = manager.get_or_create_room("auto".to_string()).await;
	///
	/// assert_eq!(room1.id(), room2.id());
	/// # });
	/// ```
	pub async fn get_or_create_room(&self, id: String) -> Arc<Room> {
		let mut rooms = self.rooms.write().await;

		if let Some(room) = rooms.get(&id) {
			return room.clone();
		}

		let room = Arc::new(Room::new(id.clone()));
		rooms.insert(id, room.clone());
		room
	}

	/// Delete a room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("temporary".to_string()).await;
	///
	/// assert!(manager.get_room("temporary").await.is_some());
	///
	/// manager.delete_room("temporary").await.unwrap();
	/// assert!(manager.get_room("temporary").await.is_none());
	/// # });
	/// ```
	pub async fn delete_room(&self, id: &str) -> RoomResult<()> {
		let mut rooms = self.rooms.write().await;

		rooms
			.remove(id)
			.ok_or_else(|| RoomError::RoomNotFound(id.to_string()))?;

		Ok(())
	}

	/// Get the number of rooms
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// assert_eq!(manager.room_count().await, 0);
	///
	/// manager.create_room("room1".to_string()).await;
	/// manager.create_room("room2".to_string()).await;
	///
	/// assert_eq!(manager.room_count().await, 2);
	/// # });
	/// ```
	pub async fn room_count(&self) -> usize {
		let rooms = self.rooms.read().await;
		rooms.len()
	}

	/// Get all room IDs
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	///
	/// manager.create_room("alpha".to_string()).await;
	/// manager.create_room("beta".to_string()).await;
	///
	/// let ids = manager.room_ids().await;
	/// assert_eq!(ids.len(), 2);
	/// assert!(ids.contains(&"alpha".to_string()));
	/// assert!(ids.contains(&"beta".to_string()));
	/// # });
	/// ```
	pub async fn room_ids(&self) -> Vec<String> {
		let rooms = self.rooms.read().await;
		rooms.keys().cloned().collect()
	}

	/// Check if a room exists
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("exists".to_string()).await;
	///
	/// assert!(manager.has_room("exists").await);
	/// assert!(!manager.has_room("missing").await);
	/// # });
	/// ```
	pub async fn has_room(&self, id: &str) -> bool {
		let rooms = self.rooms.read().await;
		rooms.contains_key(id)
	}

	/// Delete all empty rooms
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	///
	/// let empty_room = manager.create_room("empty".to_string()).await;
	/// let occupied_room = manager.create_room("occupied".to_string()).await;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let client = Arc::new(WebSocketConnection::new("user".to_string(), tx));
	/// occupied_room.join("user".to_string(), client).await.unwrap();
	///
	/// manager.cleanup_empty_rooms().await;
	///
	/// assert!(!manager.has_room("empty").await);
	/// assert!(manager.has_room("occupied").await);
	/// # });
	/// ```
	pub async fn cleanup_empty_rooms(&self) {
		let mut rooms = self.rooms.write().await;
		let empty_room_ids: Vec<String> = {
			let mut empty_ids = Vec::new();
			for (id, room) in rooms.iter() {
				if room.is_empty().await {
					empty_ids.push(id.clone());
				}
			}
			empty_ids
		};

		for id in empty_room_ids {
			rooms.remove(&id);
		}
	}

	/// Add a client to a specific room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("game".to_string()).await;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = Arc::new(WebSocketConnection::new("player1".to_string(), tx));
	///
	/// manager.join_room("game".to_string(), conn).await.unwrap();
	/// # });
	/// ```
	pub async fn join_room(
		&self,
		room_id: String,
		connection: Arc<WebSocketConnection>,
	) -> RoomResult<()> {
		let room = self
			.get_room(&room_id)
			.await
			.ok_or_else(|| RoomError::RoomNotFound(room_id.clone()))?;

		let client_id = connection.id().to_string();
		room.join(client_id, connection).await
	}

	/// Remove a client from a specific room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("chat".to_string()).await;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = Arc::new(WebSocketConnection::new("user1".to_string(), tx));
	///
	/// manager.join_room("chat".to_string(), conn).await.unwrap();
	/// manager.leave_room("chat", "user1").await.unwrap();
	/// # });
	/// ```
	pub async fn leave_room(&self, room_id: &str, user_id: &str) -> RoomResult<()> {
		let room = self
			.get_room(room_id)
			.await
			.ok_or_else(|| RoomError::RoomNotFound(room_id.to_string()))?;

		room.leave(user_id).await
	}

	/// Get the number of clients in a specific room
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("lobby".to_string()).await;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = Arc::new(WebSocketConnection::new("user1".to_string(), tx));
	///
	/// manager.join_room("lobby".to_string(), conn).await.unwrap();
	/// assert_eq!(manager.get_room_size("lobby").await, 1);
	/// # });
	/// ```
	pub async fn get_room_size(&self, room_id: &str) -> usize {
		if let Some(room) = self.get_room(room_id).await {
			room.client_count().await
		} else {
			0
		}
	}

	/// Broadcast a message to all clients in a specific room.
	///
	/// Returns a [`BroadcastResult`] describing which clients received the
	/// message and which failed. Returns [`RoomError::RoomNotFound`] if the
	/// room does not exist.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("announcement".to_string()).await;
	///
	/// let (tx, mut rx) = mpsc::unbounded_channel();
	/// let conn = Arc::new(WebSocketConnection::new("listener".to_string(), tx));
	///
	/// manager.join_room("announcement".to_string(), conn).await.unwrap();
	///
	/// let msg = Message::text("Hello everyone!".to_string());
	/// let result = manager.broadcast_to_room("announcement", msg).await.unwrap();
	/// assert!(result.is_complete_success());
	/// # });
	/// ```
	pub async fn broadcast_to_room(
		&self,
		room_id: &str,
		message: Message,
	) -> RoomResult<BroadcastResult> {
		let room = self
			.get_room(room_id)
			.await
			.ok_or_else(|| RoomError::RoomNotFound(room_id.to_string()))?;

		Ok(room.broadcast(message).await)
	}

	/// Broadcasts a message to all clients in a room with a per-client timeout.
	///
	/// Slow consumers that do not accept the message within the given timeout
	/// are treated as failed and removed from the room.
	///
	/// Returns [`RoomError::RoomNotFound`] if the room does not exist.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("timeout_test".to_string()).await;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = Arc::new(WebSocketConnection::new("user1".to_string(), tx));
	///
	/// manager.join_room("timeout_test".to_string(), conn).await.unwrap();
	///
	/// let msg = Message::text("Hello!".to_string());
	/// let result = manager
	///     .broadcast_to_room_with_timeout("timeout_test", msg, Duration::from_secs(5))
	///     .await
	///     .unwrap();
	/// assert!(result.is_complete_success());
	/// # });
	/// ```
	pub async fn broadcast_to_room_with_timeout(
		&self,
		room_id: &str,
		message: Message,
		send_timeout: Duration,
	) -> RoomResult<BroadcastResult> {
		let room = self
			.get_room(room_id)
			.await
			.ok_or_else(|| RoomError::RoomNotFound(room_id.to_string()))?;

		Ok(room.broadcast_with_timeout(message, send_timeout).await)
	}

	/// Broadcast a message to all clients in all rooms.
	///
	/// Returns a [`BroadcastResult`] aggregated across all rooms, reporting
	/// which clients received the message and which failed.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("room1".to_string()).await;
	/// manager.create_room("room2".to_string()).await;
	///
	/// let (tx1, _rx1) = mpsc::unbounded_channel();
	/// let (tx2, _rx2) = mpsc::unbounded_channel();
	///
	/// let conn1 = Arc::new(WebSocketConnection::new("user1".to_string(), tx1));
	/// let conn2 = Arc::new(WebSocketConnection::new("user2".to_string(), tx2));
	///
	/// manager.join_room("room1".to_string(), conn1).await.unwrap();
	/// manager.join_room("room2".to_string(), conn2).await.unwrap();
	///
	/// let msg = Message::text("System message".to_string());
	/// let result = manager.broadcast_to_all(msg).await;
	/// assert!(result.is_complete_success());
	/// # });
	/// ```
	pub async fn broadcast_to_all(&self, message: Message) -> BroadcastResult {
		let rooms = self.rooms.read().await;

		let mut successful = Vec::new();
		let mut failed = Vec::new();

		for room in rooms.values() {
			let result = room.broadcast(message.clone()).await;
			successful.extend(result.successful);
			failed.extend(result.failed);
		}

		BroadcastResult { successful, failed }
	}

	/// Get all room IDs (alias for room_ids for compatibility)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::room::RoomManager;
	///
	/// # tokio_test::block_on(async {
	/// let manager = RoomManager::new();
	/// manager.create_room("alpha".to_string()).await;
	/// manager.create_room("beta".to_string()).await;
	///
	/// let rooms = manager.get_all_rooms().await;
	/// assert_eq!(rooms.len(), 2);
	/// assert!(rooms.contains(&"alpha".to_string()));
	/// assert!(rooms.contains(&"beta".to_string()));
	/// # });
	/// ```
	pub async fn get_all_rooms(&self) -> Vec<String> {
		self.room_ids().await
	}
}

impl Default for RoomManager {
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
	async fn test_room_new() {
		let room = Room::new("test_room".to_string());
		assert_eq!(room.id(), "test_room");
		assert_eq!(room.client_count().await, 0);
		assert!(room.is_empty().await);
	}

	#[tokio::test]
	async fn test_room_join_client() {
		let room = Room::new("join_test".to_string());
		let (tx, _rx) = mpsc::unbounded_channel();
		let client = Arc::new(WebSocketConnection::new("client1".to_string(), tx));

		room.join("client1".to_string(), client).await.unwrap();
		assert_eq!(room.client_count().await, 1);
		assert!(room.has_client("client1").await);
	}

	#[tokio::test]
	async fn test_room_join_duplicate_client() {
		let room = Room::new("duplicate_test".to_string());
		let (tx1, _rx1) = mpsc::unbounded_channel();
		let (tx2, _rx2) = mpsc::unbounded_channel();

		let client1 = Arc::new(WebSocketConnection::new("duplicate".to_string(), tx1));
		let client2 = Arc::new(WebSocketConnection::new("duplicate".to_string(), tx2));

		room.join("duplicate".to_string(), client1).await.unwrap();
		let result = room.join("duplicate".to_string(), client2).await;

		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			RoomError::ClientAlreadyExists(_)
		));
	}

	#[tokio::test]
	async fn test_room_leave_client() {
		let room = Room::new("leave_test".to_string());
		let (tx, _rx) = mpsc::unbounded_channel();
		let client = Arc::new(WebSocketConnection::new("leaver".to_string(), tx));

		room.join("leaver".to_string(), client).await.unwrap();
		assert!(room.has_client("leaver").await);

		room.leave("leaver").await.unwrap();
		assert!(!room.has_client("leaver").await);
		assert_eq!(room.client_count().await, 0);
	}

	#[tokio::test]
	async fn test_room_leave_nonexistent_client() {
		let room = Room::new("leave_error_test".to_string());
		let result = room.leave("nonexistent").await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), RoomError::ClientNotFound(_)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_room_broadcast() {
		// Arrange
		let room = Room::new("broadcast_test".to_string());

		let (tx1, mut rx1) = mpsc::unbounded_channel();
		let (tx2, mut rx2) = mpsc::unbounded_channel();
		let (tx3, mut rx3) = mpsc::unbounded_channel();

		let client1 = Arc::new(WebSocketConnection::new("user1".to_string(), tx1));
		let client2 = Arc::new(WebSocketConnection::new("user2".to_string(), tx2));
		let client3 = Arc::new(WebSocketConnection::new("user3".to_string(), tx3));

		room.join("user1".to_string(), client1).await.unwrap();
		room.join("user2".to_string(), client2).await.unwrap();
		room.join("user3".to_string(), client3).await.unwrap();

		// Act
		let msg = Message::text("Broadcast message".to_string());
		let result = room.broadcast(msg).await;

		// Assert
		assert!(result.is_complete_success());
		assert_eq!(result.successful.len(), 3);
		assert_eq!(result.failure_count(), 0);
		assert!(matches!(rx1.try_recv(), Ok(Message::Text { .. })));
		assert!(matches!(rx2.try_recv(), Ok(Message::Text { .. })));
		assert!(matches!(rx3.try_recv(), Ok(Message::Text { .. })));
	}

	#[tokio::test]
	async fn test_room_send_to_specific_client() {
		let room = Room::new("private_msg_test".to_string());

		let (tx1, mut rx1) = mpsc::unbounded_channel();
		let (tx2, mut rx2) = mpsc::unbounded_channel();

		let client1 = Arc::new(WebSocketConnection::new("target".to_string(), tx1));
		let client2 = Arc::new(WebSocketConnection::new("other".to_string(), tx2));

		room.join("target".to_string(), client1).await.unwrap();
		room.join("other".to_string(), client2).await.unwrap();

		let msg = Message::text("Private message".to_string());
		room.send_to("target", msg).await.unwrap();

		assert!(matches!(rx1.try_recv(), Ok(Message::Text { .. })));
		assert!(rx2.try_recv().is_err());
	}

	#[tokio::test]
	async fn test_room_send_to_nonexistent_client() {
		let room = Room::new("send_error_test".to_string());
		let msg = Message::text("Test".to_string());
		let result = room.send_to("nonexistent", msg).await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), RoomError::ClientNotFound(_)));
	}

	#[tokio::test]
	async fn test_room_client_ids() {
		let room = Room::new("ids_test".to_string());

		let (tx1, _rx1) = mpsc::unbounded_channel();
		let (tx2, _rx2) = mpsc::unbounded_channel();

		let client1 = Arc::new(WebSocketConnection::new("alpha".to_string(), tx1));
		let client2 = Arc::new(WebSocketConnection::new("beta".to_string(), tx2));

		room.join("alpha".to_string(), client1).await.unwrap();
		room.join("beta".to_string(), client2).await.unwrap();

		let ids = room.client_ids().await;
		assert_eq!(ids.len(), 2);
		assert!(ids.contains(&"alpha".to_string()));
		assert!(ids.contains(&"beta".to_string()));
	}

	#[tokio::test]
	async fn test_room_metadata_set_and_get() {
		use serde_json::json;

		let room = Room::new("metadata_test".to_string());

		room.set_metadata("max_users", json!(100)).await.unwrap();
		room.set_metadata("topic", json!("General Chat"))
			.await
			.unwrap();

		let max_users: i64 = room.get_metadata("max_users").await.unwrap().unwrap();
		assert_eq!(max_users, 100);

		let topic: String = room.get_metadata("topic").await.unwrap().unwrap();
		assert_eq!(topic, "General Chat");
	}

	#[tokio::test]
	async fn test_room_metadata_get_nonexistent() {
		let room = Room::new("metadata_missing_test".to_string());
		let result: Option<String> = room.get_metadata("nonexistent").await.unwrap();
		assert!(result.is_none());
	}

	#[tokio::test]
	async fn test_room_metadata_remove() {
		use serde_json::json;

		let room = Room::new("metadata_remove_test".to_string());

		room.set_metadata("temp", json!("value")).await.unwrap();
		assert!(room.get_metadata::<String>("temp").await.unwrap().is_some());

		room.remove_metadata("temp").await;
		assert!(room.get_metadata::<String>("temp").await.unwrap().is_none());
	}

	#[tokio::test]
	async fn test_room_metadata_clear() {
		use serde_json::json;

		let room = Room::new("metadata_clear_test".to_string());

		room.set_metadata("key1", json!("value1")).await.unwrap();
		room.set_metadata("key2", json!("value2")).await.unwrap();

		room.clear_metadata().await;

		assert!(room.get_metadata::<String>("key1").await.unwrap().is_none());
		assert!(room.get_metadata::<String>("key2").await.unwrap().is_none());
	}

	#[tokio::test]
	async fn test_room_is_empty() {
		let room = Room::new("empty_test".to_string());
		assert!(room.is_empty().await);

		let (tx, _rx) = mpsc::unbounded_channel();
		let client = Arc::new(WebSocketConnection::new("user".to_string(), tx));

		room.join("user".to_string(), client).await.unwrap();
		assert!(!room.is_empty().await);

		room.leave("user").await.unwrap();
		assert!(room.is_empty().await);
	}

	#[tokio::test]
	async fn test_room_manager_new() {
		let manager = RoomManager::new();
		assert_eq!(manager.room_count().await, 0);
	}

	#[tokio::test]
	async fn test_room_manager_create_room() {
		let manager = RoomManager::new();
		let room = manager.create_room("new_room".to_string()).await;

		assert_eq!(room.id(), "new_room");
		assert_eq!(manager.room_count().await, 1);
	}

	#[tokio::test]
	async fn test_room_manager_get_room() {
		let manager = RoomManager::new();
		manager.create_room("existing".to_string()).await;

		let room = manager.get_room("existing").await;
		assert!(room.is_some());
		assert_eq!(room.unwrap().id(), "existing");

		let missing = manager.get_room("missing").await;
		assert!(missing.is_none());
	}

	#[tokio::test]
	async fn test_room_manager_get_or_create_room() {
		let manager = RoomManager::new();

		let room1 = manager.get_or_create_room("auto_room".to_string()).await;
		assert_eq!(manager.room_count().await, 1);

		let room2 = manager.get_or_create_room("auto_room".to_string()).await;
		assert_eq!(manager.room_count().await, 1);

		assert_eq!(room1.id(), room2.id());
	}

	#[tokio::test]
	async fn test_room_manager_delete_room() {
		let manager = RoomManager::new();
		manager.create_room("to_delete".to_string()).await;

		assert!(manager.has_room("to_delete").await);

		manager.delete_room("to_delete").await.unwrap();
		assert!(!manager.has_room("to_delete").await);
	}

	#[tokio::test]
	async fn test_room_manager_delete_nonexistent_room() {
		let manager = RoomManager::new();
		let result = manager.delete_room("nonexistent").await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), RoomError::RoomNotFound(_)));
	}

	#[tokio::test]
	async fn test_room_manager_room_ids() {
		let manager = RoomManager::new();

		manager.create_room("room1".to_string()).await;
		manager.create_room("room2".to_string()).await;
		manager.create_room("room3".to_string()).await;

		let ids = manager.room_ids().await;
		assert_eq!(ids.len(), 3);
		assert!(ids.contains(&"room1".to_string()));
		assert!(ids.contains(&"room2".to_string()));
		assert!(ids.contains(&"room3".to_string()));
	}

	#[tokio::test]
	async fn test_room_manager_has_room() {
		let manager = RoomManager::new();
		manager.create_room("check".to_string()).await;

		assert!(manager.has_room("check").await);
		assert!(!manager.has_room("missing").await);
	}

	#[tokio::test]
	async fn test_room_manager_cleanup_empty_rooms() {
		let manager = RoomManager::new();

		let _empty_room = manager.create_room("empty".to_string()).await;
		let occupied_room = manager.create_room("occupied".to_string()).await;

		let (tx, _rx) = mpsc::unbounded_channel();
		let client = Arc::new(WebSocketConnection::new("user".to_string(), tx));
		occupied_room
			.join("user".to_string(), client)
			.await
			.unwrap();

		manager.cleanup_empty_rooms().await;

		assert!(!manager.has_room("empty").await);
		assert!(manager.has_room("occupied").await);
		assert_eq!(manager.room_count().await, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_partial_failure_reports_failed_clients() {
		// Arrange
		let room = Room::new("partial_fail".to_string());

		let (tx_alive, mut rx_alive) = mpsc::unbounded_channel();
		let (tx_dead, _rx_dead) = mpsc::unbounded_channel::<Message>();

		let alive_client = Arc::new(WebSocketConnection::new("alive".to_string(), tx_alive));
		let dead_client = Arc::new(WebSocketConnection::new("dead".to_string(), tx_dead));

		room.join("alive".to_string(), alive_client).await.unwrap();
		room.join("dead".to_string(), dead_client).await.unwrap();

		// Simulate dead connection by dropping the receiver
		drop(_rx_dead);

		// Act
		let msg = Message::text("test message".to_string());
		let result = room.broadcast(msg).await;

		// Assert
		assert!(!result.is_complete_success());
		assert!(result.is_partial_success());
		assert!(!result.is_complete_failure());
		assert_eq!(result.successful.len(), 1);
		assert!(result.successful.contains(&"alive".to_string()));
		assert_eq!(result.failure_count(), 1);
		assert!(result.failed_client_ids().contains(&"dead"));
		assert!(matches!(rx_alive.try_recv(), Ok(Message::Text { .. })));
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_partial_failure_removes_dead_connections() {
		// Arrange
		let room = Room::new("cleanup".to_string());

		let (tx_alive, _rx_alive) = mpsc::unbounded_channel();
		let (tx_dead1, _rx_dead1) = mpsc::unbounded_channel::<Message>();
		let (tx_dead2, _rx_dead2) = mpsc::unbounded_channel::<Message>();

		let alive = Arc::new(WebSocketConnection::new("alive".to_string(), tx_alive));
		let dead1 = Arc::new(WebSocketConnection::new("dead1".to_string(), tx_dead1));
		let dead2 = Arc::new(WebSocketConnection::new("dead2".to_string(), tx_dead2));

		room.join("alive".to_string(), alive).await.unwrap();
		room.join("dead1".to_string(), dead1).await.unwrap();
		room.join("dead2".to_string(), dead2).await.unwrap();
		assert_eq!(room.client_count().await, 3);

		// Simulate dead connections
		drop(_rx_dead1);
		drop(_rx_dead2);

		// Act
		let msg = Message::text("cleanup test".to_string());
		let result = room.broadcast(msg).await;

		// Assert - dead connections are automatically removed
		assert_eq!(result.failure_count(), 2);
		assert_eq!(room.client_count().await, 1);
		assert!(room.has_client("alive").await);
		assert!(!room.has_client("dead1").await);
		assert!(!room.has_client("dead2").await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_complete_failure() {
		// Arrange
		let room = Room::new("all_dead".to_string());

		let (tx1, _rx1) = mpsc::unbounded_channel::<Message>();
		let (tx2, _rx2) = mpsc::unbounded_channel::<Message>();

		let client1 = Arc::new(WebSocketConnection::new("c1".to_string(), tx1));
		let client2 = Arc::new(WebSocketConnection::new("c2".to_string(), tx2));

		room.join("c1".to_string(), client1).await.unwrap();
		room.join("c2".to_string(), client2).await.unwrap();

		// Simulate all dead connections
		drop(_rx1);
		drop(_rx2);

		// Act
		let msg = Message::text("nobody home".to_string());
		let result = room.broadcast(msg).await;

		// Assert
		assert!(result.is_complete_failure());
		assert!(!result.is_partial_success());
		assert_eq!(result.failure_count(), 2);
		assert!(result.successful.is_empty());
		assert_eq!(room.client_count().await, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_empty_room() {
		// Arrange
		let room = Room::new("empty_broadcast".to_string());

		// Act
		let msg = Message::text("echo".to_string());
		let result = room.broadcast(msg).await;

		// Assert
		assert!(result.is_complete_success());
		assert!(result.successful.is_empty());
		assert!(result.failed.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_to_room_returns_broadcast_result() {
		// Arrange
		let manager = RoomManager::new();
		manager.create_room("results".to_string()).await;

		let (tx1, _rx1) = mpsc::unbounded_channel();
		let (tx2, _rx2) = mpsc::unbounded_channel::<Message>();
		let conn1 = Arc::new(WebSocketConnection::new("ok".to_string(), tx1));
		let conn2 = Arc::new(WebSocketConnection::new("dead".to_string(), tx2));

		manager
			.join_room("results".to_string(), conn1)
			.await
			.unwrap();
		manager
			.join_room("results".to_string(), conn2)
			.await
			.unwrap();

		// Simulate dead connection
		drop(_rx2);

		// Act
		let msg = Message::text("room broadcast".to_string());
		let result = manager.broadcast_to_room("results", msg).await.unwrap();

		// Assert
		assert!(result.is_partial_success());
		assert_eq!(result.successful.len(), 1);
		assert_eq!(result.failure_count(), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_to_all_aggregates_results() {
		// Arrange
		let manager = RoomManager::new();
		let room1 = manager.create_room("r1".to_string()).await;
		let room2 = manager.create_room("r2".to_string()).await;

		let (tx1, _rx1) = mpsc::unbounded_channel();
		let (tx2, _rx2) = mpsc::unbounded_channel::<Message>();
		let conn1 = Arc::new(WebSocketConnection::new("ok".to_string(), tx1));
		let conn2 = Arc::new(WebSocketConnection::new("dead".to_string(), tx2));

		room1.join("ok".to_string(), conn1).await.unwrap();
		room2.join("dead".to_string(), conn2).await.unwrap();

		// Simulate dead connection in room2
		drop(_rx2);

		// Act
		let msg = Message::text("global".to_string());
		let result = manager.broadcast_to_all(msg).await;

		// Assert - aggregated results from both rooms
		assert!(result.is_partial_success());
		assert_eq!(result.successful.len(), 1);
		assert_eq!(result.failure_count(), 1);
		assert!(result.successful.contains(&"ok".to_string()));
		assert!(result.failed_client_ids().contains(&"dead"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_with_timeout_succeeds_for_responsive_clients() {
		// Arrange
		let room = Room::new("timeout_ok".to_string());

		let (tx1, mut rx1) = mpsc::unbounded_channel();
		let (tx2, mut rx2) = mpsc::unbounded_channel();

		let client1 = Arc::new(WebSocketConnection::new("fast1".to_string(), tx1));
		let client2 = Arc::new(WebSocketConnection::new("fast2".to_string(), tx2));

		room.join("fast1".to_string(), client1).await.unwrap();
		room.join("fast2".to_string(), client2).await.unwrap();

		// Act
		let msg = Message::text("hello with timeout".to_string());
		let result = room
			.broadcast_with_timeout(msg, Duration::from_secs(5))
			.await;

		// Assert
		assert!(result.is_complete_success());
		assert_eq!(result.successful.len(), 2);
		assert!(matches!(rx1.try_recv(), Ok(Message::Text { .. })));
		assert!(matches!(rx2.try_recv(), Ok(Message::Text { .. })));
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_with_timeout_removes_dead_connections() {
		// Arrange
		let room = Room::new("timeout_dead".to_string());

		let (tx_alive, _rx_alive) = mpsc::unbounded_channel();
		let (tx_dead, _rx_dead) = mpsc::unbounded_channel::<Message>();

		let alive = Arc::new(WebSocketConnection::new("alive".to_string(), tx_alive));
		let dead = Arc::new(WebSocketConnection::new("dead".to_string(), tx_dead));

		room.join("alive".to_string(), alive).await.unwrap();
		room.join("dead".to_string(), dead).await.unwrap();

		// Simulate dead connection
		drop(_rx_dead);

		// Act
		let msg = Message::text("test".to_string());
		let result = room
			.broadcast_with_timeout(msg, Duration::from_secs(5))
			.await;

		// Assert
		assert!(result.is_partial_success());
		assert_eq!(result.successful.len(), 1);
		assert_eq!(result.failure_count(), 1);
		assert_eq!(room.client_count().await, 1);
		assert!(room.has_client("alive").await);
		assert!(!room.has_client("dead").await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_with_timeout_empty_room() {
		// Arrange
		let room = Room::new("timeout_empty".to_string());

		// Act
		let msg = Message::text("nobody".to_string());
		let result = room
			.broadcast_with_timeout(msg, Duration::from_secs(1))
			.await;

		// Assert
		assert!(result.is_complete_success());
		assert!(result.successful.is_empty());
		assert!(result.failed.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_to_room_with_timeout() {
		// Arrange
		let manager = RoomManager::new();
		manager.create_room("timeout_room".to_string()).await;

		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("user1".to_string(), tx));

		manager
			.join_room("timeout_room".to_string(), conn)
			.await
			.unwrap();

		// Act
		let msg = Message::text("hello".to_string());
		let result = manager
			.broadcast_to_room_with_timeout("timeout_room", msg, Duration::from_secs(5))
			.await
			.unwrap();

		// Assert
		assert!(result.is_complete_success());
		assert_eq!(result.successful.len(), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcast_to_room_with_timeout_nonexistent_room() {
		// Arrange
		let manager = RoomManager::new();

		// Act
		let msg = Message::text("hello".to_string());
		let result = manager
			.broadcast_to_room_with_timeout("missing", msg, Duration::from_secs(1))
			.await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), RoomError::RoomNotFound(_)));
	}
}
