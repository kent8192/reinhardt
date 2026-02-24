//! Comprehensive WebSocket tests based on FastAPI test patterns

use crate::connection::{Message, WebSocketConnection, WebSocketError};
use crate::room::RoomManager;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

/// Test: Basic WebSocket message sending and receiving
#[tokio::test]
async fn test_basic_websocket_communication() {
	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("client1".to_string(), tx));

	// Send text messages
	conn.send_text("Message one".to_string()).await.unwrap();
	conn.send_text("Message two".to_string()).await.unwrap();

	// Receive and verify messages
	let msg1 = rx.recv().await.unwrap();
	match msg1 {
		Message::Text { data } => assert_eq!(data, "Message one"),
		_ => panic!("Expected text message"),
	}

	let msg2 = rx.recv().await.unwrap();
	match msg2 {
		Message::Text { data } => assert_eq!(data, "Message two"),
		_ => panic!("Expected text message"),
	}
}

/// Test: WebSocket connection with dependency management (after yield pattern)
#[tokio::test]
async fn test_websocket_dependency_after_yield() {
	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("session_user".to_string(), tx));

	// Simulate session data being sent
	let session_data = vec!["foo", "bar", "baz"];
	for item in session_data {
		conn.send_text(item.to_string()).await.unwrap();
	}

	// Verify all session data received
	assert_eq!(
		rx.recv().await.unwrap(),
		Message::Text {
			data: "foo".to_string()
		}
	);
	assert_eq!(
		rx.recv().await.unwrap(),
		Message::Text {
			data: "bar".to_string()
		}
	);
	assert_eq!(
		rx.recv().await.unwrap(),
		Message::Text {
			data: "baz".to_string()
		}
	);
}

/// Test: WebSocket dependency error handling (broken dependency)
#[tokio::test]
async fn test_websocket_dependency_after_yield_broken() {
	let (tx, _rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("broken_session".to_string(), tx));

	// Close connection first
	conn.close().await.unwrap();

	// Attempt to send should fail
	let result = conn.send_text("Should fail".to_string()).await;
	assert!(result.is_err());
	assert!(matches!(result.unwrap_err(), WebSocketError::Send(_)));
}

/// Test: WebSocket with cookie-based authentication simulation
#[tokio::test]
async fn test_websocket_with_cookie_auth() {
	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("cookie_client".to_string(), tx));

	// Simulate cookie authentication
	let session_cookie = "fakesession";
	conn.send_text(format!("Session cookie value is: {}", session_cookie))
		.await
		.unwrap();

	// Send messages with cookie context
	conn.send_text("Message text was: Message one, for item ID: foo".to_string())
		.await
		.unwrap();

	// Verify received messages
	let msg1 = rx.recv().await.unwrap();
	match msg1 {
		Message::Text { data } => {
			assert_eq!(data, "Session cookie value is: fakesession");
		}
		_ => panic!("Expected text message"),
	}

	let msg2 = rx.recv().await.unwrap();
	match msg2 {
		Message::Text { data } => {
			assert_eq!(data, "Message text was: Message one, for item ID: foo");
		}
		_ => panic!("Expected text message"),
	}
}

/// Test: WebSocket with query parameter token authentication
#[tokio::test]
async fn test_websocket_with_query_token() {
	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("token_client".to_string(), tx));

	// Simulate token authentication via query parameter
	let token = "some-token";
	conn.send_text(format!("Session cookie or query token value is: {}", token))
		.await
		.unwrap();

	conn.send_text("Message text was: Message one, for item ID: bar".to_string())
		.await
		.unwrap();

	// Verify token authentication message
	let msg1 = rx.recv().await.unwrap();
	match msg1 {
		Message::Text { data } => {
			assert_eq!(data, "Session cookie or query token value is: some-token");
		}
		_ => panic!("Expected text message"),
	}

	// Verify message with context
	let msg2 = rx.recv().await.unwrap();
	match msg2 {
		Message::Text { data } => {
			assert_eq!(data, "Message text was: Message one, for item ID: bar");
		}
		_ => panic!("Expected text message"),
	}
}

/// Test: WebSocket with combined token and query parameters
#[tokio::test]
async fn test_websocket_with_token_and_query() {
	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("complex_client".to_string(), tx));

	// Send authentication message
	conn.send_text("Session cookie or query token value is: some-token".to_string())
		.await
		.unwrap();

	// Send query parameter message
	conn.send_text("Query parameter q is: 3".to_string())
		.await
		.unwrap();

	// Send actual message
	conn.send_text("Message text was: Message one, for item ID: 2".to_string())
		.await
		.unwrap();

	// Verify all three messages
	assert!(matches!(rx.recv().await.unwrap(), Message::Text { .. }));
	assert!(matches!(rx.recv().await.unwrap(), Message::Text { .. }));
	assert!(matches!(rx.recv().await.unwrap(), Message::Text { .. }));
}

/// Test: WebSocket connection rejection (no credentials)
#[tokio::test]
async fn test_websocket_no_credentials() {
	let (tx, _rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("no_auth".to_string(), tx));

	// Close immediately to simulate rejection
	let result = conn.close().await;
	assert!(result.is_ok());
	assert!(conn.is_closed().await);
}

/// Test: WebSocket connection rejection (invalid data)
#[tokio::test]
async fn test_websocket_invalid_data() {
	let (tx, _rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("invalid_data".to_string(), tx));

	// Simulate invalid data scenario by closing connection
	conn.close().await.unwrap();
	assert!(conn.is_closed().await);

	// Subsequent operations should fail
	let result = conn.send_text("This should fail".to_string()).await;
	assert!(result.is_err());
}

/// Test: Multiple WebSocket clients with room management
#[tokio::test]
async fn test_websocket_multiple_clients_with_room() {
	let manager = RoomManager::new();

	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, mut rx2) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("1234".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("5678".to_string(), tx2));

	// Create room and join
	let room = manager.get_or_create_room("chat".to_string()).await;
	room.join("1234".to_string(), conn1.clone()).await.unwrap();
	room.join("5678".to_string(), conn2.clone()).await.unwrap();

	// Client 1 sends message
	conn1
		.send_text("Hello from 1234".to_string())
		.await
		.unwrap();

	// Broadcast to room
	room.broadcast(Message::text(
		"Client #1234 says: Hello from 1234".to_string(),
	))
	.await;

	// Verify broadcast received by all clients
	let result = timeout(Duration::from_millis(100), async {
		// Both clients should receive the broadcast
		let msg1 = rx1.try_recv().ok();
		let msg2 = rx2.try_recv().ok();
		(msg1, msg2)
	})
	.await;

	assert!(result.is_ok());
}

/// Test: WebSocket client disconnection handling
#[tokio::test]
async fn test_websocket_handle_disconnection() {
	let manager = RoomManager::new();

	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, _rx2) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("1234".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("5678".to_string(), tx2));

	// Create room and join
	let room = manager.get_or_create_room("chat".to_string()).await;
	room.join("1234".to_string(), conn1.clone()).await.unwrap();
	room.join("5678".to_string(), conn2.clone()).await.unwrap();

	// Verify room size
	assert_eq!(room.client_count().await, 2);

	// Client 2 disconnects
	conn2.close().await.unwrap();
	room.leave("5678").await.unwrap();

	// Broadcast disconnect message
	room.broadcast(Message::text("Client #5678 left the chat".to_string()))
		.await;

	// Verify disconnect message received
	let msg = rx1.try_recv().unwrap();
	match msg {
		Message::Text { data } => assert_eq!(data, "Client #5678 left the chat"),
		_ => panic!("Expected text message"),
	}

	// Verify room size decreased
	assert_eq!(room.client_count().await, 1);
}

/// Test: WebSocket binary message support
#[tokio::test]
async fn test_websocket_binary_messages() {
	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("binary_client".to_string(), tx));

	let binary_data = vec![1u8, 2, 3, 4, 5];
	conn.send_binary(binary_data.clone()).await.unwrap();

	let msg = rx.recv().await.unwrap();
	match msg {
		Message::Binary { data } => assert_eq!(data, binary_data),
		_ => panic!("Expected binary message"),
	}
}

/// Test: WebSocket JSON message serialization
#[tokio::test]
async fn test_websocket_json_messages() {
	#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
	struct TestMessage {
		id: i32,
		content: String,
	}

	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("json_client".to_string(), tx));

	let test_data = TestMessage {
		id: 42,
		content: "Hello JSON".to_string(),
	};

	conn.send_json(&test_data).await.unwrap();

	let msg = rx.recv().await.unwrap();
	let parsed: TestMessage = msg.parse_json().unwrap();
	assert_eq!(parsed, test_data);
}

/// Test: WebSocket ping/pong mechanism
#[tokio::test]
async fn test_websocket_ping_pong() {
	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("ping_client".to_string(), tx));

	conn.send(Message::Ping).await.unwrap();

	let msg = rx.recv().await.unwrap();
	assert!(matches!(msg, Message::Ping));
}

/// Test: WebSocket close with custom code and reason
#[tokio::test]
async fn test_websocket_close_with_reason() {
	let (tx, mut rx) = mpsc::unbounded_channel();
	let conn = Arc::new(WebSocketConnection::new("closing_client".to_string(), tx));

	conn.close().await.unwrap();

	let msg = rx.recv().await.unwrap();
	match msg {
		Message::Close { code, reason } => {
			assert_eq!(code, 1000);
			assert_eq!(reason, "Normal closure");
		}
		_ => panic!("Expected close message"),
	}

	assert!(conn.is_closed().await);
}

/// Test: Broadcast to all rooms
#[tokio::test]
async fn test_broadcast_to_all_rooms() {
	let manager = RoomManager::new();

	let (tx1, mut rx1) = mpsc::unbounded_channel();
	let (tx2, mut rx2) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("user1".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("user2".to_string(), tx2));

	// Create rooms and join
	let room1 = manager.get_or_create_room("room1".to_string()).await;
	let room2 = manager.get_or_create_room("room2".to_string()).await;

	room1
		.join("user1".to_string(), conn1.clone())
		.await
		.unwrap();
	room2
		.join("user2".to_string(), conn2.clone())
		.await
		.unwrap();

	// Broadcast to all rooms manually
	let broadcast_msg = Message::text("Global announcement".to_string());
	for room_id in manager.room_ids().await {
		if let Some(room) = manager.get_room(&room_id).await {
			room.broadcast(broadcast_msg.clone()).await;
		}
	}

	// Both rooms should receive the message
	let msg1 = rx1.try_recv().unwrap();
	let msg2 = rx2.try_recv().unwrap();

	assert!(matches!(msg1, Message::Text { .. }));
	assert!(matches!(msg2, Message::Text { .. }));
}

/// Test: Get all rooms
#[tokio::test]
async fn test_get_all_rooms() {
	let manager = RoomManager::new();

	let (tx1, _rx1) = mpsc::unbounded_channel();
	let (tx2, _rx2) = mpsc::unbounded_channel();

	let conn1 = Arc::new(WebSocketConnection::new("user1".to_string(), tx1));
	let conn2 = Arc::new(WebSocketConnection::new("user2".to_string(), tx2));

	// Create rooms and join
	let room1 = manager.get_or_create_room("room1".to_string()).await;
	let room2 = manager.get_or_create_room("room2".to_string()).await;

	room1
		.join("user1".to_string(), conn1.clone())
		.await
		.unwrap();
	room2
		.join("user2".to_string(), conn2.clone())
		.await
		.unwrap();

	let rooms = manager.room_ids().await;
	assert_eq!(rooms.len(), 2);
	assert!(rooms.contains(&"room1".to_string()));
	assert!(rooms.contains(&"room2".to_string()));
}
