//! WebSocket integration tests
//!
//! These tests verify the integration between reinhardt-websockets and other crates.
//! Based on FastAPI WebSocket test patterns.

use reinhardt_websockets::{Message, RoomManager, WebSocketConnection};
use rstest::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

/// Fixture: Create a new WebSocket room manager for testing
#[fixture]
fn websocket_manager() -> Arc<RoomManager> {
	Arc::new(RoomManager::new())
}

/// Integration test: WebSocket with room manager and multiple connections
#[rstest]
#[tokio::test]
async fn test_websocket_room_manager_integration(websocket_manager: Arc<RoomManager>) {
	let manager = websocket_manager;

	// Create multiple connections
	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, mut rx2) = mpsc::unbounded_channel();
	let (tx3, mut rx3) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("user1".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("user2".to_string(), tx2));
	let conn3 = Arc::new(WebSocketConnection::new("user3".to_string(), tx3));

	// Create rooms
	manager.create_room("room1".to_string()).await;
	manager.create_room("room2".to_string()).await;

	// Join different rooms
	manager
		.join_room("room1".to_string(), conn1.clone())
		.await
		.unwrap();
	manager
		.join_room("room1".to_string(), conn2.clone())
		.await
		.unwrap();
	manager
		.join_room("room2".to_string(), conn3.clone())
		.await
		.unwrap();

	// Verify room sizes
	assert_eq!(manager.get_room_size("room1").await, 2);
	assert_eq!(manager.get_room_size("room2").await, 1);

	// Broadcast to room1
	manager
		.broadcast_to_room("room1", Message::text("Hello room1".to_string()))
		.await
		.unwrap();

	// Verify only room1 members received the message
	let msg1 = timeout(Duration::from_millis(100), rx1.recv())
		.await
		.unwrap()
		.unwrap();
	let msg2 = timeout(Duration::from_millis(100), rx2.recv())
		.await
		.unwrap()
		.unwrap();

	assert!(matches!(msg1, Message::Text { .. }));
	assert!(matches!(msg2, Message::Text { .. }));

	// room2 should not receive the message
	assert!(
		timeout(Duration::from_millis(50), rx3.recv())
			.await
			.is_err()
	);
}

/// Integration test: WebSocket chat room with disconnect notifications
#[rstest]
#[tokio::test]
async fn test_websocket_chat_room_with_disconnections(websocket_manager: Arc<RoomManager>) {
	let manager = websocket_manager;
	let room_name = "chat_room";

	// Create three connections
	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, mut rx2) = mpsc::unbounded_channel();
	let (tx3, _rx3) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("alice".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("bob".to_string(), tx2));
	let conn3 = Arc::new(WebSocketConnection::new("charlie".to_string(), tx3));

	// Create the chat room
	manager.create_room(room_name.to_string()).await;

	// All join the chat room
	manager
		.join_room(room_name.to_string(), conn1.clone())
		.await
		.unwrap();
	manager
		.join_room(room_name.to_string(), conn2.clone())
		.await
		.unwrap();
	manager
		.join_room(room_name.to_string(), conn3.clone())
		.await
		.unwrap();

	assert_eq!(manager.get_room_size(room_name).await, 3);

	// Charlie disconnects
	conn3.close().await.unwrap();
	manager.leave_room(room_name, "charlie").await.unwrap();

	// Broadcast disconnect notification
	manager
		.broadcast_to_room(
			room_name,
			Message::text("charlie left the chat".to_string()),
		)
		.await
		.unwrap();

	// Verify remaining users received the notification
	assert_eq!(manager.get_room_size(room_name).await, 2);

	let msg1 = timeout(Duration::from_millis(100), rx1.recv())
		.await
		.unwrap()
		.unwrap();
	let msg2 = timeout(Duration::from_millis(100), rx2.recv())
		.await
		.unwrap()
		.unwrap();

	match (msg1, msg2) {
		(Message::Text { data: d1 }, Message::Text { data: d2 }) => {
			assert_eq!(d1, "charlie left the chat");
			assert_eq!(d2, "charlie left the chat");
		}
		_ => panic!("Expected text messages"),
	}
}

/// Integration test: WebSocket broadcast to all rooms
#[rstest]
#[tokio::test]
async fn test_websocket_global_broadcast_integration(websocket_manager: Arc<RoomManager>) {
	let manager = websocket_manager;

	// Create connections in different rooms
	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, mut rx2) = mpsc::unbounded_channel();
	let (tx3, mut rx3) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("user1".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("user2".to_string(), tx2));
	let conn3 = Arc::new(WebSocketConnection::new("user3".to_string(), tx3));

	// Create rooms
	manager.create_room("room_a".to_string()).await;
	manager.create_room("room_b".to_string()).await;
	manager.create_room("room_c".to_string()).await;

	manager
		.join_room("room_a".to_string(), conn1.clone())
		.await
		.unwrap();
	manager
		.join_room("room_b".to_string(), conn2.clone())
		.await
		.unwrap();
	manager
		.join_room("room_c".to_string(), conn3.clone())
		.await
		.unwrap();

	// Global broadcast
	let global_message = Message::text("Server maintenance in 5 minutes".to_string());
	let result = manager.broadcast_to_all(global_message.clone()).await;
	assert!(result.is_complete_success(), "Broadcast should succeed");

	// All users should receive the message
	let msg1 = timeout(Duration::from_millis(100), rx1.recv())
		.await
		.unwrap()
		.unwrap();
	let msg2 = timeout(Duration::from_millis(100), rx2.recv())
		.await
		.unwrap()
		.unwrap();
	let msg3 = timeout(Duration::from_millis(100), rx3.recv())
		.await
		.unwrap()
		.unwrap();

	assert_eq!(msg1, global_message);
	assert_eq!(msg2, global_message);
	assert_eq!(msg3, global_message);
}

/// Integration test: WebSocket with JSON message serialization
#[rstest]
#[tokio::test]
async fn test_websocket_json_integration(websocket_manager: Arc<RoomManager>) {
	#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone)]
	struct ChatMessage {
		user: String,
		content: String,
		timestamp: u64,
	}

	let manager = websocket_manager;

	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, mut rx2) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("sender".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("receiver".to_string(), tx2));

	// Create chat room
	manager.create_room("chat".to_string()).await;

	manager
		.join_room("chat".to_string(), conn1.clone())
		.await
		.unwrap();
	manager
		.join_room("chat".to_string(), conn2.clone())
		.await
		.unwrap();

	// Send JSON message
	let chat_msg = ChatMessage {
		user: "sender".to_string(),
		content: "Hello in JSON!".to_string(),
		timestamp: 1234567890,
	};

	let json_message = Message::json(&chat_msg).unwrap();
	manager
		.broadcast_to_room("chat", json_message)
		.await
		.unwrap();

	// Verify receivers got the message
	let received1 = timeout(Duration::from_millis(100), rx1.recv())
		.await
		.unwrap()
		.unwrap();
	let received2 = timeout(Duration::from_millis(100), rx2.recv())
		.await
		.unwrap()
		.unwrap();

	let parsed1: ChatMessage = received1.parse_json().unwrap();
	let parsed2: ChatMessage = received2.parse_json().unwrap();

	assert_eq!(parsed1, chat_msg);
	assert_eq!(parsed2, chat_msg);
}

/// Integration test: WebSocket with binary data transfer
#[rstest]
#[tokio::test]
async fn test_websocket_binary_integration(websocket_manager: Arc<RoomManager>) {
	let manager = websocket_manager;

	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, mut rx2) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("uploader".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("downloader".to_string(), tx2));

	// Create the file transfer room
	manager.create_room("file_transfer".to_string()).await;

	manager
		.join_room("file_transfer".to_string(), conn1.clone())
		.await
		.unwrap();
	manager
		.join_room("file_transfer".to_string(), conn2.clone())
		.await
		.unwrap();

	// Simulate file transfer with binary data
	let file_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header simulation
	let binary_msg = Message::binary(file_data.clone());

	manager
		.broadcast_to_room("file_transfer", binary_msg)
		.await
		.unwrap();

	// Verify binary data received
	let received1 = timeout(Duration::from_millis(100), rx1.recv())
		.await
		.unwrap()
		.unwrap();
	let received2 = timeout(Duration::from_millis(100), rx2.recv())
		.await
		.unwrap()
		.unwrap();

	match (received1, received2) {
		(Message::Binary { data: d1 }, Message::Binary { data: d2 }) => {
			assert_eq!(d1, file_data);
			assert_eq!(d2, file_data);
		}
		_ => panic!("Expected binary messages"),
	}
}

/// Integration test: WebSocket connection lifecycle
#[rstest]
#[tokio::test]
async fn test_websocket_connection_lifecycle(websocket_manager: Arc<RoomManager>) {
	let manager = websocket_manager;

	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("lifecycle_test".to_string(), tx));

	// Create test room
	manager.create_room("test_room".to_string()).await;

	// Connect
	manager
		.join_room("test_room".to_string(), conn.clone())
		.await
		.unwrap();
	assert_eq!(manager.get_room_size("test_room").await, 1);
	assert!(!conn.is_closed().await);

	// Send some messages
	conn.send_text("Message 1".to_string()).await.unwrap();
	conn.send_text("Message 2".to_string()).await.unwrap();

	// Verify messages received
	let msg1 = rx.recv().await.unwrap();
	let msg2 = rx.recv().await.unwrap();
	assert!(matches!(msg1, Message::Text { .. }));
	assert!(matches!(msg2, Message::Text { .. }));

	// Disconnect
	conn.close().await.unwrap();
	manager
		.leave_room("test_room", "lifecycle_test")
		.await
		.unwrap();

	// Verify closed state
	assert!(conn.is_closed().await);
	assert_eq!(manager.get_room_size("test_room").await, 0);

	// Close message should be received
	let close_msg = rx.recv().await.unwrap();
	assert!(matches!(close_msg, Message::Close { .. }));
}

/// Integration test: Multiple rooms management
#[rstest]
#[tokio::test]
async fn test_websocket_multiple_rooms_management(websocket_manager: Arc<RoomManager>) {
	let manager = websocket_manager;

	// Create connections
	let connections: Vec<_> = (0..5)
		.map(|i| {
			let (tx, rx) = mpsc::unbounded_channel();
			let conn = Arc::new(WebSocketConnection::new(format!("user{}", i), tx));
			(conn, rx)
		})
		.collect();

	// Create rooms first
	manager.create_room("room0".to_string()).await;
	manager.create_room("room1".to_string()).await;
	manager.create_room("room2".to_string()).await;

	// Distribute users across rooms
	for (i, (conn, _rx)) in connections.iter().enumerate() {
		let room = format!("room{}", i % 3); // 3 rooms
		manager.join_room(room, conn.clone()).await.unwrap();
	}

	// Verify room distribution
	let all_rooms = manager.get_all_rooms().await;
	assert_eq!(all_rooms.len(), 3);

	assert_eq!(manager.get_room_size("room0").await, 2); // users 0, 3
	assert_eq!(manager.get_room_size("room1").await, 2); // users 1, 4
	assert_eq!(manager.get_room_size("room2").await, 1); // user 2
}

/// Integration test: Error handling in broadcast scenarios
#[rstest]
#[tokio::test]
async fn test_websocket_broadcast_error_handling(websocket_manager: Arc<RoomManager>) {
	let manager = websocket_manager;

	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, _rx2) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("active".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("closed".to_string(), tx2));

	// Create the mixed room
	manager.create_room("mixed".to_string()).await;

	manager
		.join_room("mixed".to_string(), conn1.clone())
		.await
		.unwrap();
	manager
		.join_room("mixed".to_string(), conn2.clone())
		.await
		.unwrap();

	// Close one connection
	conn2.close().await.unwrap();

	// Broadcast should still work for active connections
	manager
		.broadcast_to_room("mixed", Message::text("Test message".to_string()))
		.await
		.unwrap();

	// Active connection should receive the message
	let msg = timeout(Duration::from_millis(100), rx1.recv())
		.await
		.unwrap()
		.unwrap();

	assert!(matches!(msg, Message::Text { .. }));
}
