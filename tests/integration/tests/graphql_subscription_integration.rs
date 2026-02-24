//! GraphQL Subscription integration tests
//!
//! Tests GraphQL subscriptions (broadcaster/receiver model), multiple client subscriptions,
//! event broadcasting, disconnection handling, and subscription lifecycle.

use async_graphql::{ID, Schema};
use reinhardt_graphql::{EventBroadcaster, SubscriptionRoot, User, UserEvent, UserStorage};
use std::time::Duration;
use tokio::time::timeout;

/// Helper: Create subscription schema
fn create_subscription_schema(
	broadcaster: EventBroadcaster,
) -> Schema<reinhardt_graphql::Query, reinhardt_graphql::Mutation, SubscriptionRoot> {
	let storage = UserStorage::new();
	Schema::build(
		reinhardt_graphql::Query,
		reinhardt_graphql::Mutation,
		SubscriptionRoot,
	)
	.data(storage)
	.data(broadcaster)
	.finish()
}

/// Test: Basic subscription creation and event reception
#[tokio::test]
async fn test_basic_subscription() {
	let broadcaster = EventBroadcaster::new();
	let _schema = create_subscription_schema(broadcaster.clone());

	// Subscribe to user_created events
	let mut rx = broadcaster.subscribe().await;

	// Broadcast a user created event
	let user = User {
		id: ID::from("sub-test-1"),
		name: "SubscriptionUser".to_string(),
		email: "sub@example.com".to_string(),
		active: true,
	};

	broadcaster
		.broadcast(UserEvent::Created(user.clone()))
		.await;

	// Receive event with timeout
	let event = timeout(Duration::from_secs(2), rx.recv())
		.await
		.expect("Timeout waiting for event")
		.expect("Event receive error");

	match event {
		UserEvent::Created(received_user) => {
			assert_eq!(received_user.id, user.id);
			assert_eq!(received_user.name, user.name);
			assert_eq!(received_user.email, user.email);
		}
		_ => panic!("Expected Created event"),
	}
}

/// Test: Multiple clients subscribing simultaneously
#[tokio::test]
async fn test_multiple_client_subscriptions() {
	let broadcaster = EventBroadcaster::new();
	let _schema = create_subscription_schema(broadcaster.clone());

	// Create 3 subscribers
	let mut rx1 = broadcaster.subscribe().await;
	let mut rx2 = broadcaster.subscribe().await;
	let mut rx3 = broadcaster.subscribe().await;

	// Broadcast event
	let user = User {
		id: ID::from("multi-client-1"),
		name: "MultiClient".to_string(),
		email: "multiclient@example.com".to_string(),
		active: true,
	};

	broadcaster
		.broadcast(UserEvent::Created(user.clone()))
		.await;

	// All clients should receive the event
	for (i, rx) in [&mut rx1, &mut rx2, &mut rx3].iter_mut().enumerate() {
		let event = timeout(Duration::from_secs(2), rx.recv())
			.await
			.unwrap_or_else(|_| panic!("Client {} timeout", i + 1))
			.unwrap_or_else(|_| panic!("Client {} receive error", i + 1));

		match event {
			UserEvent::Created(ref received_user) => {
				assert_eq!(received_user.id, user.id);
				assert_eq!(received_user.name, "MultiClient");
			}
			_ => panic!("Client {} expected Created event", i + 1),
		}
	}
}

/// Test: Event filtering (only receive subscribed event types)
#[tokio::test]
async fn test_event_filtering_by_type() {
	let broadcaster = EventBroadcaster::new();
	let _schema = create_subscription_schema(broadcaster.clone());

	let mut rx = broadcaster.subscribe().await;

	// Broadcast different event types
	let created_user = User {
		id: ID::from("created-1"),
		name: "Created".to_string(),
		email: "created@example.com".to_string(),
		active: true,
	};

	let updated_user = User {
		id: ID::from("updated-1"),
		name: "Updated".to_string(),
		email: "updated@example.com".to_string(),
		active: false,
	};

	broadcaster
		.broadcast(UserEvent::Created(created_user))
		.await;
	broadcaster
		.broadcast(UserEvent::Updated(updated_user))
		.await;
	broadcaster
		.broadcast(UserEvent::Deleted(ID::from("deleted-1")))
		.await;

	// Receive all 3 events
	let event1 = rx.recv().await.unwrap();
	let event2 = rx.recv().await.unwrap();
	let event3 = rx.recv().await.unwrap();

	// Verify event types
	match event1 {
		UserEvent::Created(_) => {}
		_ => panic!("First event should be Created"),
	}

	match event2 {
		UserEvent::Updated(_) => {}
		_ => panic!("Second event should be Updated"),
	}

	match event3 {
		UserEvent::Deleted(_) => {}
		_ => panic!("Third event should be Deleted"),
	}
}

/// Test: Subscription with rapid event broadcast
#[tokio::test]
async fn test_rapid_event_broadcast() {
	let broadcaster = EventBroadcaster::new();
	let _schema = create_subscription_schema(broadcaster.clone());

	let mut rx = broadcaster.subscribe().await;

	// Broadcast 10 events rapidly
	for i in 0..10 {
		let user = User {
			id: ID::from(format!("rapid-{}", i)),
			name: format!("RapidUser{}", i),
			email: format!("rapid{}@example.com", i),
			active: true,
		};
		broadcaster.broadcast(UserEvent::Created(user)).await;
	}

	// Receive all 10 events
	for i in 0..10 {
		let event = timeout(Duration::from_secs(2), rx.recv())
			.await
			.unwrap_or_else(|_| panic!("Event {} timeout", i))
			.unwrap_or_else(|_| panic!("Event {} receive error", i));

		match event {
			UserEvent::Created(user) => {
				assert_eq!(user.id.to_string(), format!("rapid-{}", i));
			}
			_ => panic!("Expected Created event {}", i),
		}
	}
}

/// Test: Late subscriber (should not receive old events)
#[tokio::test]
async fn test_late_subscriber() {
	let broadcaster = EventBroadcaster::new();
	let _schema = create_subscription_schema(broadcaster.clone());

	// Broadcast event before subscription
	let early_user = User {
		id: ID::from("early-1"),
		name: "Early".to_string(),
		email: "early@example.com".to_string(),
		active: true,
	};
	broadcaster.broadcast(UserEvent::Created(early_user)).await;

	// Subscribe after event
	let mut rx = broadcaster.subscribe().await;

	// Broadcast new event
	let late_user = User {
		id: ID::from("late-1"),
		name: "Late".to_string(),
		email: "late@example.com".to_string(),
		active: true,
	};
	broadcaster
		.broadcast(UserEvent::Created(late_user.clone()))
		.await;

	// Should only receive the late event
	let event = timeout(Duration::from_secs(2), rx.recv())
		.await
		.expect("Timeout")
		.expect("Receive error");

	match event {
		UserEvent::Created(user) => {
			assert_eq!(user.id, late_user.id);
			assert_eq!(user.name, "Late");
		}
		_ => panic!("Expected late Created event"),
	}
}

/// Test: Subscription lifecycle (subscribe → unsubscribe → resubscribe)
#[tokio::test]
async fn test_subscription_lifecycle() {
	let broadcaster = EventBroadcaster::new();
	let _schema = create_subscription_schema(broadcaster.clone());

	// First subscription
	{
		let mut rx1 = broadcaster.subscribe().await;

		let user1 = User {
			id: ID::from("lifecycle-1"),
			name: "First".to_string(),
			email: "first@example.com".to_string(),
			active: true,
		};
		broadcaster
			.broadcast(UserEvent::Created(user1.clone()))
			.await;

		let event = rx1.recv().await.unwrap();
		match event {
			UserEvent::Created(user) => assert_eq!(user.id, user1.id),
			_ => panic!("Expected Created event"),
		}
		// rx1 dropped (unsubscribed)
	}

	// Resubscribe
	let mut rx2 = broadcaster.subscribe().await;

	let user2 = User {
		id: ID::from("lifecycle-2"),
		name: "Second".to_string(),
		email: "second@example.com".to_string(),
		active: true,
	};
	broadcaster
		.broadcast(UserEvent::Created(user2.clone()))
		.await;

	let event = rx2.recv().await.unwrap();
	match event {
		UserEvent::Created(user) => assert_eq!(user.id, user2.id),
		_ => panic!("Expected Created event"),
	}
}

/// Test: Concurrent subscriptions with different event types
#[tokio::test]
async fn test_concurrent_subscriptions_mixed_events() {
	let broadcaster = EventBroadcaster::new();
	let _schema = create_subscription_schema(broadcaster.clone());

	let mut rx1 = broadcaster.subscribe().await;
	let mut rx2 = broadcaster.subscribe().await;

	// Broadcast mixed events
	let created_user = User {
		id: ID::from("mixed-created"),
		name: "Created".to_string(),
		email: "created@example.com".to_string(),
		active: true,
	};

	let updated_user = User {
		id: ID::from("mixed-updated"),
		name: "Updated".to_string(),
		email: "updated@example.com".to_string(),
		active: false,
	};

	broadcaster
		.broadcast(UserEvent::Created(created_user.clone()))
		.await;
	broadcaster
		.broadcast(UserEvent::Updated(updated_user.clone()))
		.await;

	// Both receivers should get both events
	for rx in [&mut rx1, &mut rx2].iter_mut() {
		let event1 = rx.recv().await.unwrap();
		let event2 = rx.recv().await.unwrap();

		match event1 {
			UserEvent::Created(user) => assert_eq!(user.id, created_user.id),
			_ => panic!("First event should be Created"),
		}

		match event2 {
			UserEvent::Updated(user) => assert_eq!(user.id, updated_user.id),
			_ => panic!("Second event should be Updated"),
		}
	}
}

/// Test: No events timeout (verify no spurious events)
#[tokio::test]
async fn test_no_events_timeout() {
	let broadcaster = EventBroadcaster::new();
	let _schema = create_subscription_schema(broadcaster.clone());

	let mut rx = broadcaster.subscribe().await;

	// Try to receive with short timeout (should timeout)
	let result = timeout(Duration::from_millis(100), rx.recv()).await;

	assert!(
		result.is_err(),
		"Should timeout when no events are broadcast"
	);
}
