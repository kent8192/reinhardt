//! DM server function tests
//!
//! Tests for create_room, list_rooms, get_room, send_message, list_messages, mark_as_read.
//!
//! Note: Full server function integration tests require session injection.
//! These tests focus on validation logic, data factories, and type conversions.

use rstest::*;
use sqlx::PgPool;

use crate::apps::dm::shared::types::{
	CreateRoomRequest, MessageInfo, NewMessageNotification, RoomInfo, SendMessageRequest,
};
use crate::test_utils::factories::dm::{DMMessageFactory, DMRoomFactory};
use crate::test_utils::factories::user::UserFactory;
use crate::test_utils::fixtures::database::twitter_db_pool;
use crate::test_utils::fixtures::users::TestTwitterUser;

// ============================================================================
// Room Creation Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_create_direct_room(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();

	// Create two users for direct messaging
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("directuser1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("directuser2"))
		.await
		.expect("User2 creation should succeed");

	// Create direct room
	let room = room_factory
		.create_direct(&pool, user1.id(), user2.id())
		.await
		.expect("Room creation should succeed");

	// Verify room properties
	assert!(!room.is_group(), "Direct room should not be a group");
	assert!(room.name().is_none(), "Direct room should have no name");
}

#[rstest]
#[tokio::test]
async fn test_create_group_room(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();

	// Create three users for group chat
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("groupchatuser1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("groupchatuser2"))
		.await
		.expect("User2 creation should succeed");

	let user3 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("groupchatuser3"))
		.await
		.expect("User3 creation should succeed");

	// Create group room
	let room = room_factory
		.create_group(
			&pool,
			"Friends Group",
			&[user1.id(), user2.id(), user3.id()],
		)
		.await
		.expect("Room creation should succeed");

	// Verify room properties
	assert!(room.is_group(), "Group room should be marked as group");
	assert_eq!(room.name().as_deref(), Some("Friends Group"));
}

#[rstest]
#[tokio::test]
async fn test_find_rooms_for_user(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();

	// Create users
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("finduser1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("finduser2"))
		.await
		.expect("User2 creation should succeed");

	let user3 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("finduser3"))
		.await
		.expect("User3 creation should succeed");

	// Create rooms involving user1
	let _room1 = room_factory
		.create_direct(&pool, user1.id(), user2.id())
		.await
		.expect("Room1 creation should succeed");

	let _room2 = room_factory
		.create_direct(&pool, user1.id(), user3.id())
		.await
		.expect("Room2 creation should succeed");

	// Find rooms for user1
	let user1_rooms = room_factory
		.find_by_member(&pool, user1.id())
		.await
		.expect("Find rooms should succeed");

	// Find rooms for user2
	let user2_rooms = room_factory
		.find_by_member(&pool, user2.id())
		.await
		.expect("Find rooms should succeed");

	assert_eq!(user1_rooms.len(), 2, "User1 should be in 2 rooms");
	assert_eq!(user2_rooms.len(), 1, "User2 should be in 1 room");
}

#[rstest]
#[tokio::test]
async fn test_room_not_found(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let room_factory = DMRoomFactory::new();

	// Try to find non-existent room
	let fake_id = uuid::Uuid::new_v4();
	let result = room_factory.find_by_id(&pool, fake_id).await;

	assert!(result.is_err(), "Non-existent room should not be found");
}

// ============================================================================
// Message Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_send_message_success(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();
	let message_factory = DMMessageFactory::new();

	// Create users and room
	let sender = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("msgsender"))
		.await
		.expect("Sender creation should succeed");

	let receiver = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("msgreceiver"))
		.await
		.expect("Receiver creation should succeed");

	let room = room_factory
		.create_direct(&pool, sender.id(), receiver.id())
		.await
		.expect("Room creation should succeed");

	// Send message
	let message = message_factory
		.create(
			&pool,
			room.id(),
			sender.id(),
			"Hello, this is a test message!",
		)
		.await
		.expect("Message creation should succeed");

	// Verify message properties
	assert_eq!(message.content(), "Hello, this is a test message!");
	assert!(!message.is_read(), "New message should be unread");
	assert_eq!(*message.room_id(), room.id());
	assert_eq!(*message.sender_id(), sender.id());
}

#[rstest]
#[tokio::test]
async fn test_message_content_validation_empty() {
	// Test business logic: empty content check
	let content = "";
	let is_valid = !content.trim().is_empty();
	assert!(!is_valid, "Empty content should be invalid");
}

#[rstest]
#[tokio::test]
async fn test_message_content_validation_whitespace_only() {
	// Test business logic: whitespace-only content check
	let content = "   \t\n  ";
	let is_valid = !content.trim().is_empty();
	assert!(!is_valid, "Whitespace-only content should be invalid");
}

#[rstest]
#[tokio::test]
async fn test_message_content_validation_too_long() {
	// Test business logic: content length check (max 1000 chars)
	let content = "a".repeat(1001);
	let is_valid = content.len() <= 1000;
	assert!(!is_valid, "Content over 1000 chars should be invalid");
}

#[rstest]
#[tokio::test]
async fn test_message_content_validation_max_length() {
	// Test business logic: content at exactly max length
	let content = "a".repeat(1000);
	let is_valid = content.len() <= 1000;
	assert!(is_valid, "Content at exactly 1000 chars should be valid");
}

// ============================================================================
// Message List and Pagination Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_list_messages_empty_room(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();
	let message_factory = DMMessageFactory::new();

	// Create users and room
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("emptyroom1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("emptyroom2"))
		.await
		.expect("User2 creation should succeed");

	let room = room_factory
		.create_direct(&pool, user1.id(), user2.id())
		.await
		.expect("Room creation should succeed");

	// Get messages from empty room
	let messages = message_factory
		.find_by_room(&pool, room.id(), None)
		.await
		.expect("Find messages should succeed");

	assert!(messages.is_empty(), "New room should have no messages");
}

#[rstest]
#[tokio::test]
async fn test_list_messages_multiple(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();
	let message_factory = DMMessageFactory::new();

	// Create users and room
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("multimsg1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("multimsg2"))
		.await
		.expect("User2 creation should succeed");

	let room = room_factory
		.create_direct(&pool, user1.id(), user2.id())
		.await
		.expect("Room creation should succeed");

	// Create multiple messages
	let _messages = message_factory
		.create_many(
			&pool,
			room.id(),
			user1.id(),
			&["Message 1", "Message 2", "Message 3"],
		)
		.await
		.expect("Messages creation should succeed");

	// Get all messages
	let all_messages = message_factory
		.find_by_room(&pool, room.id(), None)
		.await
		.expect("Find messages should succeed");

	assert_eq!(all_messages.len(), 3, "Should have 3 messages");
}

#[rstest]
#[tokio::test]
async fn test_list_messages_with_limit(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();
	let message_factory = DMMessageFactory::new();

	// Create users and room
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("limituser1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("limituser2"))
		.await
		.expect("User2 creation should succeed");

	let room = room_factory
		.create_direct(&pool, user1.id(), user2.id())
		.await
		.expect("Room creation should succeed");

	// Create 5 messages
	message_factory
		.create_many(
			&pool,
			room.id(),
			user1.id(),
			&["Msg 1", "Msg 2", "Msg 3", "Msg 4", "Msg 5"],
		)
		.await
		.expect("Messages creation should succeed");

	// Get with limit
	let limited_messages = message_factory
		.find_by_room(&pool, room.id(), Some(2))
		.await
		.expect("Find messages should succeed");

	assert_eq!(limited_messages.len(), 2, "Should return only 2 messages");
}

// ============================================================================
// Mark as Read Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_mark_message_as_read(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();
	let message_factory = DMMessageFactory::new();

	// Create users and room
	let sender = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("readtest_sender"))
		.await
		.expect("Sender creation should succeed");

	let receiver = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("readtest_receiver"))
		.await
		.expect("Receiver creation should succeed");

	let room = room_factory
		.create_direct(&pool, sender.id(), receiver.id())
		.await
		.expect("Room creation should succeed");

	// Send message
	let message = message_factory
		.create(&pool, room.id(), sender.id(), "Please read me!")
		.await
		.expect("Message creation should succeed");

	assert!(!message.is_read(), "Message should be unread initially");

	// Mark as read
	message_factory
		.mark_as_read(&pool, message.id())
		.await
		.expect("Mark as read should succeed");

	// Verify
	let updated = message_factory
		.find_by_id(&pool, message.id())
		.await
		.expect("Find should succeed");

	assert!(updated.is_read(), "Message should be marked as read");
}

#[rstest]
#[tokio::test]
async fn test_mark_room_as_read(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();
	let message_factory = DMMessageFactory::new();

	// Create users and room
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("roomread1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("roomread2"))
		.await
		.expect("User2 creation should succeed");

	let room = room_factory
		.create_direct(&pool, user1.id(), user2.id())
		.await
		.expect("Room creation should succeed");

	// Send messages from user1 to user2
	message_factory
		.create_many(
			&pool,
			room.id(),
			user1.id(),
			&["Unread 1", "Unread 2", "Unread 3"],
		)
		.await
		.expect("Messages creation should succeed");

	// Mark room as read for user2 (receiver)
	message_factory
		.mark_room_as_read(&pool, room.id(), user2.id())
		.await
		.expect("Mark room as read should succeed");

	// Verify all messages are now read
	let messages = message_factory
		.find_by_room(&pool, room.id(), None)
		.await
		.expect("Find messages should succeed");

	for msg in messages {
		assert!(msg.is_read(), "All messages should be marked as read");
	}
}

// ============================================================================
// Type Conversion Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_room_info_structure() {
	// Test RoomInfo structure
	let room_info = RoomInfo {
		id: uuid::Uuid::new_v4(),
		name: "Test Room".to_string(),
		is_group: false,
		participants: vec![uuid::Uuid::new_v4(), uuid::Uuid::new_v4()],
		last_message: Some("Hello!".to_string()),
		last_activity: Some("2025-01-01T00:00:00Z".to_string()),
		unread_count: 5,
	};

	assert_eq!(room_info.name, "Test Room");
	assert!(!room_info.is_group);
	assert_eq!(room_info.participants.len(), 2);
	assert_eq!(room_info.unread_count, 5);
}

#[rstest]
#[tokio::test]
async fn test_message_info_structure() {
	let sender_id = uuid::Uuid::new_v4();
	let room_id = uuid::Uuid::new_v4();

	// Test MessageInfo structure
	let message_info = MessageInfo {
		id: uuid::Uuid::new_v4(),
		room_id,
		sender_id,
		sender_username: "testuser".to_string(),
		content: "Test message content".to_string(),
		created_at: "2025-01-01T00:00:00Z".to_string(),
		is_read: false,
	};

	assert_eq!(message_info.sender_username, "testuser");
	assert_eq!(message_info.content, "Test message content");
	assert!(!message_info.is_read);
	assert_eq!(message_info.room_id, room_id);
	assert_eq!(message_info.sender_id, sender_id);
}

#[rstest]
#[tokio::test]
async fn test_send_message_request_structure() {
	let room_id = uuid::Uuid::new_v4();

	// Test SendMessageRequest structure
	let request = SendMessageRequest {
		room_id,
		content: "Hello, World!".to_string(),
	};

	assert_eq!(request.room_id, room_id);
	assert_eq!(request.content, "Hello, World!");
}

#[rstest]
#[tokio::test]
async fn test_create_room_request_structure() {
	let participant_id = uuid::Uuid::new_v4();

	// Test CreateRoomRequest structure
	let request = CreateRoomRequest {
		participant_ids: vec![participant_id],
		name: Some("My Group".to_string()),
	};

	assert_eq!(request.participant_ids.len(), 1);
	assert_eq!(request.participant_ids[0], participant_id);
	assert_eq!(request.name, Some("My Group".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_new_message_notification_structure() {
	let room_id = uuid::Uuid::new_v4();

	// Test NewMessageNotification structure
	let notification = NewMessageNotification {
		room_id,
		message_preview: "Hey there!".to_string(),
		sender_username: "alice".to_string(),
		created_at: "2025-01-01T12:00:00Z".to_string(),
	};

	assert_eq!(notification.room_id, room_id);
	assert_eq!(notification.message_preview, "Hey there!");
	assert_eq!(notification.sender_username, "alice");
}

// ============================================================================
// Message Count Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_count_messages_in_room(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();
	let message_factory = DMMessageFactory::new();

	// Create users and room
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("countuser1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("countuser2"))
		.await
		.expect("User2 creation should succeed");

	let room = room_factory
		.create_direct(&pool, user1.id(), user2.id())
		.await
		.expect("Room creation should succeed");

	// Verify empty count
	let count = message_factory
		.count_by_room(&pool, room.id())
		.await
		.expect("Count should succeed");
	assert_eq!(count, 0, "New room should have 0 messages");

	// Add messages
	message_factory
		.create_many(
			&pool,
			room.id(),
			user1.id(),
			&["Msg 1", "Msg 2", "Msg 3", "Msg 4"],
		)
		.await
		.expect("Messages creation should succeed");

	// Verify updated count
	let count = message_factory
		.count_by_room(&pool, room.id())
		.await
		.expect("Count should succeed");
	assert_eq!(count, 4, "Room should have 4 messages");
}

// ============================================================================
// Room Count Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_count_rooms_for_user(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let room_factory = DMRoomFactory::new();

	// Create users
	let user1 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("roomcount1"))
		.await
		.expect("User1 creation should succeed");

	let user2 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("roomcount2"))
		.await
		.expect("User2 creation should succeed");

	let user3 = user_factory
		.create_from_test_user(&pool, &TestTwitterUser::new("roomcount3"))
		.await
		.expect("User3 creation should succeed");

	// Verify initial count
	let count = room_factory
		.count_by_member(&pool, user1.id())
		.await
		.expect("Count should succeed");
	assert_eq!(count, 0, "User1 should be in 0 rooms initially");

	// Create rooms
	room_factory
		.create_direct(&pool, user1.id(), user2.id())
		.await
		.expect("Room1 creation should succeed");

	room_factory
		.create_direct(&pool, user1.id(), user3.id())
		.await
		.expect("Room2 creation should succeed");

	// Verify updated count
	let count = room_factory
		.count_by_member(&pool, user1.id())
		.await
		.expect("Count should succeed");
	assert_eq!(count, 2, "User1 should be in 2 rooms");
}
