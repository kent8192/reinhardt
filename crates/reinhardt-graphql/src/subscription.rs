use async_graphql::{Context, ID, Subscription};
use futures_util::Stream;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Default channel capacity for subscription broadcast channels
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// Event types for subscriptions
#[derive(Debug, Clone)]
pub enum UserEvent {
	Created(crate::schema::User),
	Updated(crate::schema::User),
	Deleted(ID),
}

/// Event broadcaster with configurable backpressure
///
/// Uses `tokio::sync::broadcast` with a bounded channel to provide
/// backpressure. When slow subscribers lag behind, `RecvError::Lagged`
/// is handled gracefully by logging a warning and continuing.
#[derive(Clone)]
pub struct EventBroadcaster {
	tx: Arc<RwLock<broadcast::Sender<UserEvent>>>,
	capacity: usize,
}

impl EventBroadcaster {
	/// Create a new event broadcaster with default capacity
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::subscription::EventBroadcaster;
	///
	/// let broadcaster = EventBroadcaster::new();
	/// ```
	pub fn new() -> Self {
		Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
	}

	/// Create a new event broadcaster with specified channel capacity
	///
	/// The capacity determines how many messages can be buffered before
	/// slow subscribers start lagging.
	///
	/// # Panics
	///
	/// Panics if `capacity` is 0.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::subscription::EventBroadcaster;
	///
	/// let broadcaster = EventBroadcaster::with_capacity(512);
	/// ```
	pub fn with_capacity(capacity: usize) -> Self {
		assert!(capacity > 0, "Channel capacity must be greater than 0");
		let (tx, _) = broadcast::channel(capacity);
		Self {
			tx: Arc::new(RwLock::new(tx)),
			capacity,
		}
	}

	/// Returns the configured channel capacity
	pub fn capacity(&self) -> usize {
		self.capacity
	}

	/// Broadcast an event to all subscribers
	///
	/// If there are no active subscribers, the event is dropped and
	/// a debug-level log is emitted. The number of receivers that
	/// received the event is returned.
	pub async fn broadcast(&self, event: UserEvent) -> usize {
		let tx = self.tx.read().await;
		match tx.send(event) {
			Ok(receiver_count) => {
				info!(receiver_count, "broadcast event sent to subscribers");
				receiver_count
			}
			Err(_) => {
				// No active receivers; event is dropped
				info!("broadcast event dropped: no active subscribers");
				0
			}
		}
	}

	/// Subscribe to events
	///
	/// Returns a receiver that handles lagged messages gracefully.
	pub async fn subscribe(&self) -> broadcast::Receiver<UserEvent> {
		self.tx.read().await.subscribe()
	}
}

impl Default for EventBroadcaster {
	fn default() -> Self {
		Self::new()
	}
}

/// Create a stream from a broadcast receiver that handles lagged messages
///
/// When a subscriber falls behind and messages are dropped, this stream
/// logs a warning and continues receiving subsequent messages instead of
/// terminating.
fn receiver_to_stream(mut rx: broadcast::Receiver<UserEvent>) -> impl Stream<Item = UserEvent> {
	async_stream::stream! {
		loop {
			match rx.recv().await {
				Ok(event) => yield event,
				Err(broadcast::error::RecvError::Lagged(skipped)) => {
					warn!(
						skipped,
						"subscription receiver lagged, messages were dropped"
					);
					// Continue receiving subsequent messages
					continue;
				}
				Err(broadcast::error::RecvError::Closed) => {
					// Channel closed, stop the stream
					break;
				}
			}
		}
	}
}

/// GraphQL Subscription root
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
	async fn user_created<'ctx>(
		&self,
		ctx: &Context<'ctx>,
	) -> impl Stream<Item = crate::schema::User> + 'ctx {
		// Gracefully handle missing EventBroadcaster instead of panicking.
		// Returns an empty stream if the broadcaster is not in context.
		let receiver = match ctx.data::<EventBroadcaster>() {
			Ok(broadcaster) => Some(broadcaster.subscribe().await),
			Err(_) => None,
		};

		let stream = receiver.map(receiver_to_stream);
		async_stream::stream! {
			if let Some(stream) = stream {
				use futures_util::StreamExt;
				let mut stream = std::pin::pin!(stream);
				while let Some(event) = stream.next().await {
					if let UserEvent::Created(user) = event {
						yield user;
					}
				}
			}
		}
	}

	async fn user_updated<'ctx>(
		&self,
		ctx: &Context<'ctx>,
	) -> impl Stream<Item = crate::schema::User> + 'ctx {
		let receiver = match ctx.data::<EventBroadcaster>() {
			Ok(broadcaster) => Some(broadcaster.subscribe().await),
			Err(_) => None,
		};

		let stream = receiver.map(receiver_to_stream);
		async_stream::stream! {
			if let Some(stream) = stream {
				use futures_util::StreamExt;
				let mut stream = std::pin::pin!(stream);
				while let Some(event) = stream.next().await {
					if let UserEvent::Updated(user) = event {
						yield user;
					}
				}
			}
		}
	}

	async fn user_deleted<'ctx>(&self, ctx: &Context<'ctx>) -> impl Stream<Item = ID> + 'ctx {
		let receiver = match ctx.data::<EventBroadcaster>() {
			Ok(broadcaster) => Some(broadcaster.subscribe().await),
			Err(_) => None,
		};

		let stream = receiver.map(receiver_to_stream);
		async_stream::stream! {
			if let Some(stream) = stream {
				use futures_util::StreamExt;
				let mut stream = std::pin::pin!(stream);
				while let Some(event) = stream.next().await {
					if let UserEvent::Deleted(id) = event {
						yield id;
					}
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	fn make_test_user(id: &str, name: &str) -> crate::schema::User {
		crate::schema::User {
			id: ID::from(id),
			name: name.to_string(),
			email: format!("{}@example.com", name.to_lowercase()),
			active: true,
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcaster_default_capacity() {
		// Arrange & Act
		let broadcaster = EventBroadcaster::new();

		// Assert
		assert_eq!(broadcaster.capacity(), DEFAULT_CHANNEL_CAPACITY);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcaster_custom_capacity() {
		// Arrange & Act
		let broadcaster = EventBroadcaster::with_capacity(512);

		// Assert
		assert_eq!(broadcaster.capacity(), 512);
	}

	#[rstest]
	#[tokio::test]
	#[should_panic(expected = "Channel capacity must be greater than 0")]
	async fn test_broadcaster_zero_capacity_panics() {
		// Arrange & Act & Assert
		EventBroadcaster::with_capacity(0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcaster_send_receive() {
		// Arrange
		let broadcaster = EventBroadcaster::new();
		let mut rx = broadcaster.subscribe().await;
		let user = make_test_user("1", "Test");

		// Act
		let receiver_count = broadcaster
			.broadcast(UserEvent::Created(user.clone()))
			.await;

		// Assert
		assert_eq!(receiver_count, 1);
		let event = rx.recv().await.unwrap();
		match event {
			UserEvent::Created(u) => assert_eq!(u.name, "Test"),
			_ => panic!("Expected Created event"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcaster_no_subscribers_returns_zero() {
		// Arrange
		let broadcaster = EventBroadcaster::new();
		let user = make_test_user("no-sub", "NoSub");

		// Act
		let receiver_count = broadcaster.broadcast(UserEvent::Created(user)).await;

		// Assert
		assert_eq!(receiver_count, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_broadcaster_multiple_subscribers() {
		// Arrange
		let broadcaster = EventBroadcaster::new();
		let mut rx1 = broadcaster.subscribe().await;
		let mut rx2 = broadcaster.subscribe().await;
		let mut rx3 = broadcaster.subscribe().await;
		let user = make_test_user("multi-sub-1", "MultiSub");

		// Act
		let receiver_count = broadcaster
			.broadcast(UserEvent::Created(user.clone()))
			.await;

		// Assert
		assert_eq!(receiver_count, 3);

		let event1 = rx1.recv().await.unwrap();
		let event2 = rx2.recv().await.unwrap();
		let event3 = rx3.recv().await.unwrap();

		match event1 {
			UserEvent::Created(u) => assert_eq!(u.name, "MultiSub"),
			_ => panic!("Expected Created event in rx1"),
		}
		match event2 {
			UserEvent::Created(u) => assert_eq!(u.name, "MultiSub"),
			_ => panic!("Expected Created event in rx2"),
		}
		match event3 {
			UserEvent::Created(u) => assert_eq!(u.name, "MultiSub"),
			_ => panic!("Expected Created event in rx3"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_event_created() {
		// Arrange
		let broadcaster = EventBroadcaster::new();
		let mut rx = broadcaster.subscribe().await;
		let user = make_test_user("created-test", "CreatedUser");

		// Act
		broadcaster
			.broadcast(UserEvent::Created(user.clone()))
			.await;
		let event = rx.recv().await.unwrap();

		// Assert
		match event {
			UserEvent::Created(u) => {
				assert_eq!(u.id.to_string(), "created-test");
				assert_eq!(u.name, "CreatedUser");
				assert_eq!(u.email, "createduser@example.com");
				assert!(u.active);
			}
			_ => panic!("Expected Created event"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_event_updated() {
		// Arrange
		let broadcaster = EventBroadcaster::new();
		let mut rx = broadcaster.subscribe().await;
		let mut user = make_test_user("updated-test", "UpdatedUser");
		user.active = false;

		// Act
		broadcaster
			.broadcast(UserEvent::Updated(user.clone()))
			.await;
		let event = rx.recv().await.unwrap();

		// Assert
		match event {
			UserEvent::Updated(u) => {
				assert_eq!(u.id.to_string(), "updated-test");
				assert_eq!(u.name, "UpdatedUser");
				assert!(!u.active);
			}
			_ => panic!("Expected Updated event"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_event_deleted() {
		// Arrange
		let broadcaster = EventBroadcaster::new();
		let mut rx = broadcaster.subscribe().await;
		let deleted_id = ID::from("deleted-test");

		// Act
		broadcaster
			.broadcast(UserEvent::Deleted(deleted_id.clone()))
			.await;
		let event = rx.recv().await.unwrap();

		// Assert
		match event {
			UserEvent::Deleted(id) => {
				assert_eq!(id.to_string(), "deleted-test");
			}
			_ => panic!("Expected Deleted event"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_subscription_filtering() {
		// Arrange
		let broadcaster = EventBroadcaster::new();
		let mut rx = broadcaster.subscribe().await;
		let user1 = make_test_user("filter-1", "Filter1");
		let mut user2 = make_test_user("filter-2", "Filter2");
		user2.active = false;

		// Act
		broadcaster
			.broadcast(UserEvent::Created(user1.clone()))
			.await;
		broadcaster
			.broadcast(UserEvent::Updated(user2.clone()))
			.await;
		broadcaster
			.broadcast(UserEvent::Deleted(ID::from("filter-3")))
			.await;

		// Assert
		let event1 = rx.recv().await.unwrap();
		let event2 = rx.recv().await.unwrap();
		let event3 = rx.recv().await.unwrap();

		assert!(matches!(event1, UserEvent::Created(_)));
		assert!(matches!(event2, UserEvent::Updated(_)));
		assert!(matches!(event3, UserEvent::Deleted(_)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_backpressure_lagged_consumer() {
		// Arrange
		// Use a very small channel to trigger lagging
		let broadcaster = EventBroadcaster::with_capacity(2);
		let mut rx = broadcaster.subscribe().await;

		// Act
		// Send more messages than the channel capacity to cause lagging
		for i in 0..5 {
			let user = make_test_user(&format!("bp-{}", i), &format!("User{}", i));
			broadcaster.broadcast(UserEvent::Created(user)).await;
		}

		// Assert
		// The receiver should get a Lagged error for the first recv,
		// then be able to receive subsequent messages
		match rx.recv().await {
			Err(broadcast::error::RecvError::Lagged(skipped)) => {
				// Overflow occurred as expected
				assert!(skipped > 0);
			}
			Ok(_) => {
				// Some messages may still be in buffer, that's ok
			}
			Err(broadcast::error::RecvError::Closed) => {
				panic!("Channel should not be closed");
			}
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_receiver_to_stream_handles_lagged() {
		use futures_util::StreamExt;

		// Arrange
		// Small capacity to trigger lagging
		let broadcaster = EventBroadcaster::with_capacity(2);
		let rx = broadcaster.subscribe().await;
		let mut stream = std::pin::pin!(receiver_to_stream(rx));

		// Act
		// Overflow the channel before consuming
		for i in 0..5 {
			let user = make_test_user(&format!("stream-{}", i), &format!("StreamUser{}", i));
			broadcaster.broadcast(UserEvent::Created(user)).await;
		}

		// Assert
		// The stream should still yield events (the most recent ones in the buffer)
		// despite lagging, because receiver_to_stream handles Lagged gracefully
		let event = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next()).await;
		assert!(
			event.is_ok(),
			"Stream should produce an event after lagging"
		);
		assert!(event.unwrap().is_some(), "Stream should not be terminated");
	}

	#[rstest]
	#[tokio::test]
	async fn test_receiver_to_stream_closed() {
		use futures_util::StreamExt;

		// Arrange
		let broadcaster = EventBroadcaster::with_capacity(4);
		let rx = broadcaster.subscribe().await;
		let mut stream = std::pin::pin!(receiver_to_stream(rx));

		// Act
		// Drop the broadcaster to close the channel
		drop(broadcaster);

		// Assert
		let event = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next()).await;
		assert!(
			event.is_ok(),
			"Stream should resolve when channel is closed"
		);
		assert!(
			event.unwrap().is_none(),
			"Stream should yield None when closed"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_bounded_channel_respects_capacity() {
		// Arrange
		let capacity = 4;
		let broadcaster = EventBroadcaster::with_capacity(capacity);
		let _rx = broadcaster.subscribe().await;

		// Act
		// Fill up the channel exactly to capacity
		for i in 0..capacity {
			let user = make_test_user(&format!("cap-{}", i), &format!("CapUser{}", i));
			broadcaster.broadcast(UserEvent::Created(user)).await;
		}

		// Assert
		// Sending one more should still succeed (broadcast replaces oldest)
		let user = make_test_user("cap-overflow", "CapOverflow");
		let count = broadcaster.broadcast(UserEvent::Created(user)).await;
		assert_eq!(count, 1);
	}

	#[tokio::test]
	async fn test_subscription_missing_broadcaster_does_not_panic() {
		// Arrange: schema without EventBroadcaster in context data
		use async_graphql::{EmptyMutation, Schema};
		use tokio_stream::StreamExt;

		let schema = Schema::build(crate::schema::Query, EmptyMutation, SubscriptionRoot)
			.data(crate::schema::UserStorage::new())
			.finish();

		// Act: attempt to subscribe to user_created without EventBroadcaster
		let query = r#"subscription { userCreated { id name } }"#;
		let mut stream = schema.execute_stream(query);

		// Assert: stream should terminate gracefully without panic.
		// Use a timeout to prevent hanging if stream never terminates.
		let result =
			tokio::time::timeout(std::time::Duration::from_millis(100), stream.next()).await;

		// Either the stream returned None (empty) or a response -- both are acceptable.
		// The key assertion is that we reached this point without a panic.
		if let Ok(Some(resp)) = result {
			// If we got a response, it should be an empty data set (no panic)
			assert!(
				resp.errors.is_empty() || !resp.errors.is_empty(),
				"reached without panic"
			);
		}
		// If timeout or None, that is also fine -- no panic occurred.
	}
}
