//! DM client hooks
//!
//! Custom React-like hooks for direct messaging functionality.
//! These hooks provide reactive state management for the DM UI components.

use crate::apps::dm::shared::types::{MessageInfo, RoomInfo};
use reinhardt::pages::reactive::Signal;
use reinhardt::pages::reactive::hooks::{
	ConnectionState, UseWebSocketOptions, WebSocketHandle, WebSocketMessage, use_effect, use_state,
	use_websocket,
};
use std::rc::Rc;
use uuid::Uuid;

/// Handle returned by `use_dm_chat` hook
///
/// Provides reactive access to chat state and functions for sending messages.
#[derive(Clone)]
pub struct DmChatHandle {
	/// Reactive list of messages in the room
	pub messages: Signal<Vec<MessageInfo>>,
	/// Loading state indicator
	pub is_loading: Signal<bool>,
	/// Error message if any
	pub error: Signal<Option<String>>,
	/// WebSocket connection handle
	pub ws: WebSocketHandle,
	/// Function to send a message
	send_message_fn: Rc<dyn Fn(String)>,
}

impl DmChatHandle {
	/// Check if the WebSocket connection is open
	pub fn is_connected(&self) -> bool {
		matches!(self.ws.connection_state().get(), ConnectionState::Open)
	}

	/// Send a text message to the room
	pub fn send_message(&self, content: String) {
		(self.send_message_fn)(content);
	}

	/// Get the current list of messages
	pub fn get_messages(&self) -> Vec<MessageInfo> {
		self.messages.get()
	}
}

/// Handle returned by `use_dm_room_list` hook
///
/// Provides reactive access to room list state.
#[derive(Clone)]
pub struct DmRoomListHandle {
	/// Reactive list of DM rooms
	pub rooms: Signal<Vec<RoomInfo>>,
	/// Loading state indicator
	pub is_loading: Signal<bool>,
	/// Error message if any
	pub error: Signal<Option<String>>,
	/// WebSocket connection for notifications (optional)
	pub ws: Option<WebSocketHandle>,
}

impl DmRoomListHandle {
	/// Get the current list of rooms
	pub fn get_rooms(&self) -> Vec<RoomInfo> {
		self.rooms.get()
	}

	/// Find a room by ID
	pub fn find_room(&self, room_id: Uuid) -> Option<RoomInfo> {
		self.rooms.get().into_iter().find(|r| r.id == room_id)
	}
}

/// Hook for managing DM chat state
///
/// Provides reactive state management for a single DM room, including:
/// - WebSocket connection for real-time message updates
/// - Initial message loading via server function
/// - Message sending functionality
///
/// # Arguments
///
/// * `room_id` - The UUID of the DM room
///
/// # Returns
///
/// A `DmChatHandle` with reactive state and control functions.
///
/// # Example
///
/// ```ignore
/// use crate::apps::dm::client::hooks::use_dm_chat;
///
/// let chat = use_dm_chat(room_id);
///
/// // Access messages reactively
/// let messages = chat.messages.get();
///
/// // Send a message
/// chat.send_message("Hello!".to_string());
/// ```
pub fn use_dm_chat(room_id: Uuid) -> DmChatHandle {
	// WebSocket connection for real-time messaging
	let ws_url = format!("/ws/dm/{}", room_id);
	let ws = use_websocket(
		&ws_url,
		UseWebSocketOptions {
			auto_reconnect: true,
			max_reconnect_attempts: Some(5),
			reconnect_delay: Some(1000),
			..Default::default()
		},
	);

	// Reactive state
	let (messages, set_messages) = use_state(Vec::<MessageInfo>::new());
	let (is_loading, set_loading) = use_state(true);
	let (error, set_error) = use_state(None::<String>);

	// Initial message loading via server function
	{
		let room_id = room_id;
		let set_messages = set_messages.clone();
		let set_loading = set_loading.clone();
		let set_error = set_error.clone();

		use_effect(move || {
			// Load initial messages using server function
			// Note: In WASM, we use wasm-bindgen-futures to spawn async tasks
			#[cfg(client)]
			{
				let set_messages = set_messages.clone();
				let set_loading = set_loading.clone();
				let set_error = set_error.clone();

				wasm_bindgen_futures::spawn_local(async move {
					match crate::apps::dm::server::server_fn::list_messages(room_id, Some(50), None)
						.await
					{
						Ok(msgs) => {
							set_messages(msgs);
							set_loading(false);
						}
						Err(e) => {
							set_error(Some(format!("Failed to load messages: {}", e)));
							set_loading(false);
						}
					}
				});
			}

			None::<fn()>
		});
	}

	// Handle incoming WebSocket messages
	{
		let ws = ws.clone();
		let messages = messages.clone();

		use_effect(move || {
			if let Some(WebSocketMessage::Text(text)) = ws.latest_message().get() {
				// Try to parse as MessageInfo
				if let Ok(msg) = serde_json::from_str::<MessageInfo>(&text) {
					// Add the new message to the list
					let mut current = messages.get();
					current.push(msg);
					messages.set(current);
				}
			}
			None::<fn()>
		});
	}

	// Create send message function
	let send_message_fn: Rc<dyn Fn(String)> = {
		let ws = ws.clone();
		let room_id = room_id;

		Rc::new(move |content: String| {
			// Create a message payload for WebSocket
			let payload = serde_json::json!({
				"type": "message",
				"room_id": room_id.to_string(),
				"content": content,
			});

			if let Ok(json_str) = serde_json::to_string(&payload) {
				let _ = ws.send_text(json_str);
			}
		})
	};

	DmChatHandle {
		messages,
		is_loading,
		error,
		ws,
		send_message_fn,
	}
}

/// Hook for managing DM room list state
///
/// Provides reactive state management for the list of DM rooms, including:
/// - Initial room list loading via server function
/// - Optional WebSocket connection for new message notifications
///
/// # Returns
///
/// A `DmRoomListHandle` with reactive state.
///
/// # Example
///
/// ```ignore
/// use crate::apps::dm::client::hooks::use_dm_room_list;
///
/// let room_list = use_dm_room_list();
///
/// // Access rooms reactively
/// for room in room_list.get_rooms() {
///     println!("{}: {} unread", room.name, room.unread_count);
/// }
/// ```
pub fn use_dm_room_list() -> DmRoomListHandle {
	// Optional WebSocket for notifications
	let ws = use_websocket(
		"/ws/dm/notifications",
		UseWebSocketOptions {
			auto_reconnect: true,
			max_reconnect_attempts: Some(3),
			reconnect_delay: Some(2000),
			..Default::default()
		},
	);

	// Reactive state
	let (rooms, set_rooms) = use_state(Vec::<RoomInfo>::new());
	let (is_loading, set_loading) = use_state(true);
	let (error, set_error) = use_state(None::<String>);

	// Initial room list loading
	{
		let set_rooms = set_rooms.clone();
		let set_loading = set_loading.clone();
		let set_error = set_error.clone();

		use_effect(move || {
			#[cfg(client)]
			{
				let set_rooms = set_rooms.clone();
				let set_loading = set_loading.clone();
				let set_error = set_error.clone();

				wasm_bindgen_futures::spawn_local(async move {
					match crate::apps::dm::server::server_fn::list_rooms().await {
						Ok(room_list) => {
							set_rooms(room_list);
							set_loading(false);
						}
						Err(e) => {
							set_error(Some(format!("Failed to load rooms: {}", e)));
							set_loading(false);
						}
					}
				});
			}

			None::<fn()>
		});
	}

	// Handle notification updates from WebSocket
	{
		let ws = ws.clone();
		let rooms = rooms.clone();

		use_effect(move || {
			if let Some(WebSocketMessage::Text(text)) = ws.latest_message().get() {
				// Parse notification message
				if let Ok(notification) = serde_json::from_str::<NewMessageNotification>(&text) {
					// Update the affected room's unread count and last message
					let mut current = rooms.get();
					if let Some(room) = current.iter_mut().find(|r| r.id == notification.room_id) {
						room.last_message = Some(notification.message_preview.clone());
						room.last_activity = Some(notification.timestamp.clone());
						room.unread_count += 1;
					}
					rooms.set(current);
				}
			}
			None::<fn()>
		});
	}

	DmRoomListHandle {
		rooms,
		is_loading,
		error,
		ws: Some(ws),
	}
}

/// Notification message for new DM messages
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NewMessageNotification {
	/// ID of the room that received the message
	pub room_id: Uuid,
	/// Preview of the message content
	pub message_preview: String,
	/// Timestamp of the message
	pub timestamp: String,
	/// Sender's username
	pub sender_username: String,
}
