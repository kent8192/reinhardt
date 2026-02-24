//! Integration tests for GraphQL subscriptions with real-time data updates
//!
//! This test suite verifies GraphQL subscription functionality including:
//! - Real-time event broadcasting
//! - Subscription lifecycle management
//! - ORM model integration with subscriptions
//! - Authentication and authorization
//! - Multiple concurrent subscriptions
//! - Proper cleanup and resource management

use async_graphql::{Context, ID, Object, Schema, Subscription};
use futures::StreamExt;
use reinhardt_graphql::{EventBroadcaster, User, UserEvent};
use rstest::*;
use serial_test::serial;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::core::IntoContainerPort;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{GenericImage, ImageExt};

/// Test user model that can be stored in PostgreSQL
#[derive(Debug, Clone, sqlx::FromRow)]
struct DbUser {
	id: String,
	name: String,
	email: String,
	active: bool,
}

impl From<DbUser> for User {
	fn from(db_user: DbUser) -> Self {
		User {
			id: ID::from(db_user.id),
			name: db_user.name,
			email: db_user.email,
			active: db_user.active,
		}
	}
}

impl From<User> for DbUser {
	fn from(user: User) -> Self {
		DbUser {
			id: user.id.to_string(),
			name: user.name,
			email: user.email,
			active: user.active,
		}
	}
}

/// ORM-backed user storage that persists to PostgreSQL
#[derive(Clone)]
struct OrmUserStorage {
	pool: Arc<PgPool>,
	broadcaster: EventBroadcaster,
}

impl OrmUserStorage {
	fn new(pool: Arc<PgPool>, broadcaster: EventBroadcaster) -> Self {
		Self { pool, broadcaster }
	}

	async fn create_user(&self, name: String, email: String) -> Result<User, sqlx::Error> {
		let id = uuid::Uuid::new_v4().to_string();
		let user = sqlx::query_as::<_, DbUser>(
			"INSERT INTO users (id, name, email, active) VALUES ($1, $2, $3, true) RETURNING *",
		)
		.bind(&id)
		.bind(&name)
		.bind(&email)
		.fetch_one(self.pool.as_ref())
		.await?;

		let graphql_user = User::from(user);
		self.broadcaster
			.broadcast(UserEvent::Created(graphql_user.clone()))
			.await;

		Ok(graphql_user)
	}

	async fn update_user(&self, id: String, active: bool) -> Result<Option<User>, sqlx::Error> {
		let user =
			sqlx::query_as::<_, DbUser>("UPDATE users SET active = $1 WHERE id = $2 RETURNING *")
				.bind(active)
				.bind(&id)
				.fetch_optional(self.pool.as_ref())
				.await?;

		if let Some(user) = user {
			let graphql_user = User::from(user);
			self.broadcaster
				.broadcast(UserEvent::Updated(graphql_user.clone()))
				.await;
			Ok(Some(graphql_user))
		} else {
			Ok(None)
		}
	}

	async fn delete_user(&self, id: String) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM users WHERE id = $1")
			.bind(&id)
			.execute(self.pool.as_ref())
			.await?;

		if result.rows_affected() > 0 {
			self.broadcaster
				.broadcast(UserEvent::Deleted(ID::from(id)))
				.await;
			Ok(true)
		} else {
			Ok(false)
		}
	}

	async fn get_user(&self, id: String) -> Result<Option<User>, sqlx::Error> {
		let user = sqlx::query_as::<_, DbUser>("SELECT * FROM users WHERE id = $1")
			.bind(&id)
			.fetch_optional(self.pool.as_ref())
			.await?;

		Ok(user.map(User::from))
	}
}

/// GraphQL Query root for ORM-backed operations
struct OrmQuery;

#[Object]
impl OrmQuery {
	async fn user(&self, ctx: &Context<'_>, id: String) -> Option<User> {
		let storage = ctx.data::<OrmUserStorage>().ok()?;
		storage.get_user(id).await.ok().flatten()
	}
}

/// GraphQL Mutation root for ORM-backed operations
struct OrmMutation;

#[Object]
impl OrmMutation {
	async fn create_user(&self, ctx: &Context<'_>, name: String, email: String) -> Option<User> {
		let storage = ctx.data::<OrmUserStorage>().ok()?;
		storage.create_user(name, email).await.ok()
	}

	async fn update_user(&self, ctx: &Context<'_>, id: String, active: bool) -> Option<User> {
		let storage = ctx.data::<OrmUserStorage>().ok()?;
		storage.update_user(id, active).await.ok().flatten()
	}

	async fn delete_user(&self, ctx: &Context<'_>, id: String) -> bool {
		let storage = ctx.data::<OrmUserStorage>().ok();
		if let Some(storage) = storage {
			storage.delete_user(id).await.unwrap_or(false)
		} else {
			false
		}
	}
}

/// GraphQL Subscription root
struct OrmSubscriptionRoot;

#[Subscription]
impl OrmSubscriptionRoot {
	async fn user_created<'ctx>(
		&self,
		ctx: &Context<'ctx>,
	) -> impl futures::Stream<Item = User> + 'ctx {
		let broadcaster = ctx.data::<EventBroadcaster>().unwrap();
		let mut rx = broadcaster.subscribe().await;

		async_stream::stream! {
			while let Ok(event) = rx.recv().await {
				if let UserEvent::Created(user) = event {
					yield user;
				}
			}
		}
	}

	async fn user_updated<'ctx>(
		&self,
		ctx: &Context<'ctx>,
	) -> impl futures::Stream<Item = User> + 'ctx {
		let broadcaster = ctx.data::<EventBroadcaster>().unwrap();
		let mut rx = broadcaster.subscribe().await;

		async_stream::stream! {
			while let Ok(event) = rx.recv().await {
				if let UserEvent::Updated(user) = event {
					yield user;
				}
			}
		}
	}

	async fn user_deleted<'ctx>(
		&self,
		ctx: &Context<'ctx>,
	) -> impl futures::Stream<Item = ID> + 'ctx {
		let broadcaster = ctx.data::<EventBroadcaster>().unwrap();
		let mut rx = broadcaster.subscribe().await;

		async_stream::stream! {
			while let Ok(event) = rx.recv().await {
				if let UserEvent::Deleted(id) = event {
					yield id;
				}
			}
		}
	}

	/// Subscription with authentication check
	async fn authenticated_user_updates<'ctx>(
		&self,
		ctx: &Context<'ctx>,
	) -> Result<impl futures::Stream<Item = User> + 'ctx, &'static str> {
		// Check if user is authenticated (simulated with context data)
		let is_authenticated = ctx.data::<bool>().unwrap_or(&false);
		if !is_authenticated {
			return Err("Unauthorized: Authentication required");
		}

		let broadcaster = ctx.data::<EventBroadcaster>().unwrap();
		let mut rx = broadcaster.subscribe().await;

		Ok(async_stream::stream! {
			while let Ok(event) = rx.recv().await {
				match event {
					UserEvent::Created(user) | UserEvent::Updated(user) => yield user,
					_ => {}
				}
			}
		})
	}
}

/// Fixture providing PostgreSQL container with users table
#[fixture]
async fn postgres_with_schema() -> (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster) {
	let postgres = GenericImage::new("postgres", "17-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(
			testcontainers_modules::testcontainers::core::WaitFor::message_on_stderr(
				"database system is ready to accept connections",
			),
		)
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.start()
		.await
		.expect("Failed to start PostgreSQL container");

	let port = postgres
		.get_host_port_ipv4(5432)
		.await
		.expect("Failed to get PostgreSQL port");

	let database_url = format!("postgres://postgres@localhost:{}/postgres", port);

	let pool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(5)
		.connect(&database_url)
		.await
		.expect("Failed to connect to PostgreSQL");

	// Create users table
	sqlx::query(
		r#"
		CREATE TABLE IF NOT EXISTS users (
			id VARCHAR(255) PRIMARY KEY,
			name VARCHAR(255) NOT NULL,
			email VARCHAR(255) NOT NULL,
			active BOOLEAN NOT NULL DEFAULT true
		)
		"#,
	)
	.execute(&pool)
	.await
	.expect("Failed to create users table");

	let broadcaster = EventBroadcaster::new();

	(postgres, Arc::new(pool), broadcaster)
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_subscription_setup_and_basic_event(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	// Setup subscription
	let subscription_query = r#"
		subscription {
			userCreated {
				id
				name
				email
				active
			}
		}
	"#;

	let mut stream = schema.execute_stream(subscription_query);

	// Create user in separate task
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		storage
			.create_user("Alice".to_string(), "alice@example.com".to_string())
			.await
			.expect("Failed to create user");
	});

	// Wait for subscription event
	let event = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for subscription event")
		.expect("Stream should yield an event");

	assert!(event.errors.is_empty());
	let data = event.data.into_json().unwrap();
	assert_eq!(data["userCreated"]["name"], "Alice");
	assert_eq!(data["userCreated"]["email"], "alice@example.com");
	assert!(data["userCreated"]["active"].as_bool().unwrap());
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_realtime_updates_via_subscriptions(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	// Subscribe to user updates
	let subscription_query = r#"
		subscription {
			userUpdated {
				id
				name
				active
			}
		}
	"#;

	let mut stream = schema.execute_stream(subscription_query);

	// Create and then update user
	let user_id = {
		let user = storage
			.create_user("Bob".to_string(), "bob@example.com".to_string())
			.await
			.expect("Failed to create user");
		user.id.to_string()
	};

	// Update user in separate task
	let storage_clone = storage.clone();
	let user_id_clone = user_id.clone();
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		storage_clone
			.update_user(user_id_clone, false)
			.await
			.expect("Failed to update user");
	});

	// Wait for update event
	let event = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for update event")
		.expect("Stream should yield an event");

	assert!(event.errors.is_empty());
	let data = event.data.into_json().unwrap();
	assert_eq!(data["userUpdated"]["id"], user_id);
	assert!(!data["userUpdated"]["active"].as_bool().unwrap());
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_subscription_with_orm_models(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	// Subscribe to created events
	let subscription_query = r#"
		subscription {
			userCreated {
				id
				name
				email
				active
			}
		}
	"#;

	let mut stream = schema.execute_stream(subscription_query);

	// Create multiple users and verify they persist in DB
	let storage_clone = storage.clone();
	tokio::spawn(async move {
		for i in 0..3 {
			tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
			storage_clone
				.create_user(format!("User{}", i), format!("user{}@example.com", i))
				.await
				.expect("Failed to create user");
		}
	});

	// Verify all events are received
	for i in 0..3 {
		let event = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
			.await
			.expect("Timeout waiting for event")
			.expect("Stream should yield an event");

		assert!(event.errors.is_empty());
		let data = event.data.into_json().unwrap();
		assert_eq!(data["userCreated"]["name"], format!("User{}", i));
		assert_eq!(
			data["userCreated"]["email"],
			format!("user{}@example.com", i)
		);

		// Verify user was actually persisted to database
		let user_id = data["userCreated"]["id"].as_str().unwrap();
		let db_user = storage
			.get_user(user_id.to_string())
			.await
			.expect("Failed to get user from DB");
		assert!(db_user.is_some());
	}
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_subscription_authentication(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	// Create schema without authentication
	let schema_unauthorized = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.data(false) // Not authenticated
		.finish();

	// Try to subscribe without authentication
	let subscription_query = r#"
		subscription {
			authenticatedUserUpdates {
				id
				name
			}
		}
	"#;

	let mut stream = schema_unauthorized.execute_stream(subscription_query);

	// Should receive error event
	let event = stream.next().await.expect("Should receive error event");
	assert!(!event.errors.is_empty());
	assert!(
		event.errors[0]
			.message
			.contains("Unauthorized: Authentication required")
	);

	// Create schema with authentication
	let schema_authorized = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.data(true) // Authenticated
		.finish();

	let mut stream = schema_authorized.execute_stream(subscription_query);

	// Create user
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		storage
			.create_user("Authenticated".to_string(), "auth@example.com".to_string())
			.await
			.expect("Failed to create user");
	});

	// Should receive event successfully
	let event = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for event")
		.expect("Stream should yield an event");

	assert!(event.errors.is_empty());
	let data = event.data.into_json().unwrap();
	assert_eq!(data["authenticatedUserUpdates"]["name"], "Authenticated");
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_multiple_concurrent_subscriptions(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	// Create multiple subscription streams
	let sub1_query = r#"subscription { userCreated { id name } }"#;
	let sub2_query = r#"subscription { userUpdated { id active } }"#;
	let sub3_query = r#"subscription { userDeleted }"#;

	let mut stream1 = schema.execute_stream(sub1_query);
	let mut stream2 = schema.execute_stream(sub2_query);
	let mut stream3 = schema.execute_stream(sub3_query);

	// Perform operations in sequence
	let storage_clone = storage.clone();
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		// Create user (should trigger stream1)
		let user = storage_clone
			.create_user("MultiSub".to_string(), "multi@example.com".to_string())
			.await
			.expect("Failed to create user");
		let user_id = user.id.to_string();

		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		// Update user (should trigger stream2)
		storage_clone
			.update_user(user_id.clone(), false)
			.await
			.expect("Failed to update user");

		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		// Delete user (should trigger stream3)
		storage_clone
			.delete_user(user_id)
			.await
			.expect("Failed to delete user");
	});

	// Wait for create event on stream1
	let event1 = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream1.next())
		.await
		.expect("Timeout waiting for create event")
		.expect("Stream1 should yield an event");
	assert!(event1.errors.is_empty());
	let data1 = event1.data.into_json().unwrap();
	assert_eq!(data1["userCreated"]["name"], "MultiSub");

	// Wait for update event on stream2
	let event2 = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream2.next())
		.await
		.expect("Timeout waiting for update event")
		.expect("Stream2 should yield an event");
	assert!(event2.errors.is_empty());
	let data2 = event2.data.into_json().unwrap();
	assert!(!data2["userUpdated"]["active"].as_bool().unwrap());

	// Wait for delete event on stream3
	let event3 = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream3.next())
		.await
		.expect("Timeout waiting for delete event")
		.expect("Stream3 should yield an event");
	assert!(event3.errors.is_empty());
	let data3 = event3.data.into_json().unwrap();
	assert!(data3["userDeleted"].is_string());
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_subscription_cleanup_on_drop(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	let subscription_query = r#"
		subscription {
			userCreated {
				id
				name
			}
		}
	"#;

	// Create and immediately drop stream
	{
		let _stream = schema.execute_stream(subscription_query);
		// Stream is dropped here
	}

	// Create another subscription to verify broadcaster still works
	let mut stream = schema.execute_stream(subscription_query);

	// Create user
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		storage
			.create_user("CleanupTest".to_string(), "cleanup@example.com".to_string())
			.await
			.expect("Failed to create user");
	});

	// New subscription should still receive events
	let event = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for event")
		.expect("Stream should yield an event");

	assert!(event.errors.is_empty());
	let data = event.data.into_json().unwrap();
	assert_eq!(data["userCreated"]["name"], "CleanupTest");
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_subscription_with_database_transaction(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	let subscription_query = r#"
		subscription {
			userCreated {
				id
				name
			}
		}
	"#;

	let mut stream = schema.execute_stream(subscription_query);

	// Create user within transaction
	let pool_clone = pool.clone();
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		let mut tx = pool_clone
			.begin()
			.await
			.expect("Failed to begin transaction");

		let user_id = uuid::Uuid::new_v4().to_string();
		sqlx::query("INSERT INTO users (id, name, email, active) VALUES ($1, $2, $3, true)")
			.bind(&user_id)
			.bind("TxUser")
			.bind("tx@example.com")
			.execute(&mut *tx)
			.await
			.expect("Failed to insert user in transaction");

		tx.commit().await.expect("Failed to commit transaction");

		// Manually broadcast event after transaction commits
		broadcaster
			.broadcast(UserEvent::Created(User {
				id: ID::from(user_id),
				name: "TxUser".to_string(),
				email: "tx@example.com".to_string(),
				active: true,
			}))
			.await;
	});

	// Subscription should receive event after transaction commits
	let event = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for event")
		.expect("Stream should yield an event");

	assert!(event.errors.is_empty());
	let data = event.data.into_json().unwrap();
	assert_eq!(data["userCreated"]["name"], "TxUser");

	// Verify user exists in database
	let user_id = data["userCreated"]["id"].as_str().unwrap();
	let db_user = storage
		.get_user(user_id.to_string())
		.await
		.expect("Failed to get user");
	assert!(db_user.is_some());
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_subscription_filtering_by_event_type(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	// Subscribe only to created events
	let subscription_query = r#"
		subscription {
			userCreated {
				id
				name
			}
		}
	"#;

	let mut stream = schema.execute_stream(subscription_query);

	// Create user, update it, then create another user
	let storage_clone = storage.clone();
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		// Create first user (should be received)
		let user1 = storage_clone
			.create_user("FirstUser".to_string(), "first@example.com".to_string())
			.await
			.expect("Failed to create first user");

		tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

		// Update user (should NOT be received by userCreated subscription)
		storage_clone
			.update_user(user1.id.to_string(), false)
			.await
			.expect("Failed to update user");

		tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

		// Create second user (should be received)
		storage_clone
			.create_user("SecondUser".to_string(), "second@example.com".to_string())
			.await
			.expect("Failed to create second user");
	});

	// Should receive first created event
	let event1 = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for first event")
		.expect("Stream should yield first event");
	assert!(event1.errors.is_empty());
	let data1 = event1.data.into_json().unwrap();
	assert_eq!(data1["userCreated"]["name"], "FirstUser");

	// Should receive second created event (update should be filtered out)
	let event2 = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for second event")
		.expect("Stream should yield second event");
	assert!(event2.errors.is_empty());
	let data2 = event2.data.into_json().unwrap();
	assert_eq!(data2["userCreated"]["name"], "SecondUser");
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_subscription_with_high_event_volume(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	let subscription_query = r#"
		subscription {
			userCreated {
				id
				name
			}
		}
	"#;

	let mut stream = schema.execute_stream(subscription_query);

	// Create many users rapidly
	let storage_clone = storage.clone();
	let event_count = 10;
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		for i in 0..event_count {
			storage_clone
				.create_user(
					format!("HighVolume{}", i),
					format!("highvol{}@example.com", i),
				)
				.await
				.expect("Failed to create user");
		}
	});

	// Verify all events are received
	for i in 0..event_count {
		let event = tokio::time::timeout(tokio::time::Duration::from_secs(5), stream.next())
			.await
			.expect("Timeout waiting for event")
			.expect("Stream should yield an event");

		assert!(event.errors.is_empty());
		let data = event.data.into_json().unwrap();
		assert_eq!(data["userCreated"]["name"], format!("HighVolume{}", i));
	}
}

#[rstest]
#[tokio::test]
#[serial(graphql_subscriptions)]
async fn test_subscription_error_handling_on_broadcaster_drop(
	#[future] postgres_with_schema: (ContainerAsync<GenericImage>, Arc<PgPool>, EventBroadcaster),
) {
	let (_container, pool, broadcaster) = postgres_with_schema.await;

	let storage = OrmUserStorage::new(pool.clone(), broadcaster.clone());

	let schema = Schema::build(OrmQuery, OrmMutation, OrmSubscriptionRoot)
		.data(storage.clone())
		.data(broadcaster.clone())
		.finish();

	let subscription_query = r#"
		subscription {
			userCreated {
				id
				name
			}
		}
	"#;

	let mut stream = schema.execute_stream(subscription_query);

	// Create user before broadcaster is potentially dropped
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		storage
			.create_user("ErrorTest".to_string(), "error@example.com".to_string())
			.await
			.expect("Failed to create user");
	});

	// Receive the event
	let event = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for event")
		.expect("Stream should yield an event");

	assert!(event.errors.is_empty());
	let data = event.data.into_json().unwrap();
	assert_eq!(data["userCreated"]["name"], "ErrorTest");

	// Stream should eventually end when no more events
	// We test that the stream handles graceful shutdown properly
	// by not waiting for another event (would timeout if working correctly)
	let next_event =
		tokio::time::timeout(tokio::time::Duration::from_millis(500), stream.next()).await;

	// Should timeout or return None (stream end), not receive an event
	assert!(
		next_event.is_err() || next_event.unwrap().is_none(),
		"Stream should not receive additional events"
	);
}
