//! Integration tests for reinhardt-graphql

use async_graphql::{ID, Schema};
use futures::StreamExt;
use reinhardt_graphql::{
	EventBroadcaster, Mutation, Query, SubscriptionRoot, User, UserEvent, UserStorage,
	create_schema,
};

#[tokio::test]
async fn test_full_graphql_workflow() {
	// Setup
	let storage = UserStorage::new();
	let schema = create_schema(storage);

	// Step 1: Query empty users list
	let query = r#"
        {
            users {
                id
                name
            }
        }
    "#;
	let result = schema.execute(query).await;
	let data = result.data.into_json().unwrap();
	assert_eq!(data["users"].as_array().unwrap().len(), 0);

	// Step 2: Create a user via mutation
	let mutation = r#"
        mutation {
            createUser(input: { name: "Alice", email: "alice@example.com" }) {
                id
                name
                email
                active
            }
        }
    "#;
	let result = schema.execute(mutation).await;
	let data = result.data.into_json().unwrap();
	let user_id = data["createUser"]["id"].as_str().unwrap();
	assert_eq!(data["createUser"]["name"], "Alice");

	// Step 3: Query the created user
	let query = format!(
		r#"
        {{
            user(id: "{}") {{
                id
                name
                email
                active
            }}
        }}
        "#,
		user_id
	);
	let result = schema.execute(&query).await;
	let data = result.data.into_json().unwrap();
	assert_eq!(data["user"]["name"], "Alice");
	assert_eq!(data["user"]["email"], "alice@example.com");
	assert!(data["user"]["active"].as_bool().unwrap());

	// Step 4: Update user status
	let mutation = format!(
		r#"
        mutation {{
            updateUserStatus(id: "{}", active: false) {{
                id
                active
            }}
        }}
        "#,
		user_id
	);
	let result = schema.execute(&mutation).await;
	let data = result.data.into_json().unwrap();
	assert!(!data["updateUserStatus"]["active"].as_bool().unwrap());

	// Step 5: Verify update
	let query = format!(
		r#"
        {{
            user(id: "{}") {{
                active
            }}
        }}
        "#,
		user_id
	);
	let result = schema.execute(&query).await;
	let data = result.data.into_json().unwrap();
	assert!(!data["user"]["active"].as_bool().unwrap());

	// Step 6: Query all users
	let query = r#"
        {
            users {
                id
                name
            }
        }
    "#;
	let result = schema.execute(query).await;
	let data = result.data.into_json().unwrap();
	assert_eq!(data["users"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_concurrent_operations() {
	let storage = UserStorage::new();
	let schema = create_schema(storage);

	// Create multiple users concurrently
	let handles: Vec<_> = (0..5)
		.map(|i| {
			let schema = schema.clone();
			tokio::spawn(async move {
				let mutation = format!(
					r#"
                    mutation {{
                        createUser(input: {{ name: "User{}", email: "user{}@example.com" }}) {{
                            id
                            name
                        }}
                    }}
                    "#,
					i, i
				);
				schema.execute(&mutation).await
			})
		})
		.collect();

	for handle in handles {
		let result = handle.await.unwrap();
		assert!(result.errors.is_empty());
	}

	// Query all users
	let query = r#"
        {
            users {
                id
                name
            }
        }
    "#;
	let result = schema.execute(query).await;
	let data = result.data.into_json().unwrap();
	assert_eq!(data["users"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn test_schema_introspection() {
	let storage = UserStorage::new();
	let schema = create_schema(storage);

	// Query schema types
	let query = r#"
        {
            __schema {
                types {
                    name
                    kind
                }
            }
        }
    "#;
	let result = schema.execute(query).await;
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	assert!(data["__schema"]["types"].is_array());

	// Query specific type
	let query = r#"
        {
            __type(name: "User") {
                name
                kind
                fields {
                    name
                    type {
                        name
                    }
                }
            }
        }
    "#;
	let result = schema.execute(query).await;
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	assert_eq!(data["__type"]["name"], "User");

	// Verify User has expected fields
	let fields = data["__type"]["fields"].as_array().unwrap();
	let field_names: Vec<&str> = fields.iter().map(|f| f["name"].as_str().unwrap()).collect();
	assert!(field_names.contains(&"id"));
	assert!(field_names.contains(&"name"));
	assert!(field_names.contains(&"email"));
	assert!(field_names.contains(&"active"));
}

#[tokio::test]
async fn test_graphql_integration_error_handling() {
	let storage = UserStorage::new();
	let schema = create_schema(storage);

	// Test invalid query syntax
	let query = r#"
        {
            user(id: "test"
        }
    "#;
	let result = schema.execute(query).await;
	assert!(!result.errors.is_empty());

	// Test invalid field
	let query = r#"
        {
            users {
                nonExistentField
            }
        }
    "#;
	let result = schema.execute(query).await;
	assert!(!result.errors.is_empty());

	// Test missing required argument
	let mutation = r#"
        mutation {
            createUser(input: { name: "Test" }) {
                id
            }
        }
    "#;
	let result = schema.execute(mutation).await;
	assert!(!result.errors.is_empty());
}

#[tokio::test]
async fn test_complex_queries() {
	let storage = UserStorage::new();

	// Pre-populate with users
	for i in 0..3 {
		storage
			.add_user(User {
				id: ID::from(format!("user-{}", i)),
				name: format!("User{}", i),
				email: format!("user{}@example.com", i),
				active: i % 2 == 0,
			})
			.await;
	}

	let schema = create_schema(storage);

	// Query with multiple fields and introspection
	let query = r#"
        {
            users {
                id
                name
                email
                active
            }
            hello(name: "Complex")
            __typename
        }
    "#;
	let result = schema.execute(query).await;
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();

	assert_eq!(data["users"].as_array().unwrap().len(), 3);
	assert_eq!(data["hello"], "Hello, Complex!");
	assert_eq!(data["__typename"], "Query");
}

#[tokio::test]
async fn test_batched_queries() {
	let storage = UserStorage::new();
	let schema = create_schema(storage);

	// Create users first
	for i in 0..3 {
		let mutation = format!(
			r#"
            mutation {{
                createUser(input: {{ name: "BatchUser{}", email: "batch{}@example.com" }}) {{
                    id
                }}
            }}
            "#,
			i, i
		);
		schema.execute(&mutation).await;
	}

	// Execute multiple queries in sequence (simulating batch)
	let queries = vec![
		r#"{ hello(name: "Batch1") }"#,
		r#"{ hello(name: "Batch2") }"#,
		r#"{ users { id } }"#,
	];

	for query in queries {
		let result = schema.execute(query).await;
		assert!(result.errors.is_empty());
	}
}

#[tokio::test]
async fn test_subscription_lifecycle() {
	// Create schema with subscription support
	let storage = UserStorage::new();
	let broadcaster = EventBroadcaster::new();

	let schema = Schema::build(Query, Mutation, SubscriptionRoot)
		.data(storage)
		.data(broadcaster.clone())
		.finish();

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

	// Execute subscription and get stream
	let mut stream = schema.execute_stream(subscription_query);

	// Broadcast a user created event in a separate task
	let broadcaster_clone = broadcaster.clone();
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		let user = User {
			id: ID::from("sub-test-1"),
			name: "Subscription Test".to_string(),
			email: "sub@test.com".to_string(),
			active: true,
		};

		broadcaster_clone.broadcast(UserEvent::Created(user)).await;
	});

	// Wait for the event
	let event = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next())
		.await
		.expect("Timeout waiting for subscription event")
		.expect("Stream should yield an event");

	// Verify event data
	assert!(event.errors.is_empty(), "Event should not have errors");
	let data = event.data.into_json().unwrap();

	assert!(
		data["userCreated"]["id"]
			.as_str()
			.unwrap()
			.contains("sub-test-1")
	);
	assert_eq!(data["userCreated"]["name"], "Subscription Test");
	assert_eq!(data["userCreated"]["email"], "sub@test.com");
	assert!(data["userCreated"]["active"].as_bool().unwrap());
}

#[tokio::test]
async fn test_context_propagation() {
	let storage = UserStorage::new();
	let schema = create_schema(storage);

	// Create user
	let mutation = r#"
        mutation {
            createUser(input: { name: "ContextUser", email: "context@example.com" }) {
                id
                name
            }
        }
    "#;
	let result = schema.execute(mutation).await;
	assert!(result.errors.is_empty());

	// Query should work without explicit context
	let query = r#"
        {
            users {
                name
            }
        }
    "#;
	let result = schema.execute(query).await;
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	assert!(!data["users"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_mutation_rollback_on_error() {
	let storage = UserStorage::new();
	let schema = create_schema(storage.clone());

	// Count users before
	let initial_count = storage.list_users().await.len();

	// Try to create user with invalid input (missing required field)
	let mutation = r#"
        mutation {
            createUser(input: { name: "Test" }) {
                id
            }
        }
    "#;
	let result = schema.execute(mutation).await;
	assert!(!result.errors.is_empty());

	// Verify user count unchanged
	let final_count = storage.list_users().await.len();
	assert_eq!(initial_count, final_count);
}

#[tokio::test]
async fn test_query_with_variables() {
	let storage = UserStorage::new();
	storage
		.add_user(User {
			id: ID::from("var-test-1"),
			name: "VarUser".to_string(),
			email: "varuser@example.com".to_string(),
			active: true,
		})
		.await;

	let schema = create_schema(storage);

	// Query with variables
	let query = r#"
        query GetUser($userId: ID!) {
            user(id: $userId) {
                id
                name
            }
        }
    "#;

	let result =
		schema
			.execute(async_graphql::Request::new(query).variables(
				async_graphql::Variables::from_json(serde_json::json!({
					"userId": "var-test-1"
				})),
			))
			.await;

	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	assert_eq!(data["user"]["name"], "VarUser");
}

#[tokio::test]
async fn test_query_aliases() {
	let storage = UserStorage::new();
	let schema = create_schema(storage);

	// Query with aliases
	let query = r#"
        {
            greeting1: hello(name: "Alice")
            greeting2: hello(name: "Bob")
            allUsers: users {
                id
            }
        }
    "#;

	let result = schema.execute(query).await;
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	assert_eq!(data["greeting1"], "Hello, Alice!");
	assert_eq!(data["greeting2"], "Hello, Bob!");
	assert!(data["allUsers"].is_array());
}

#[tokio::test]
async fn test_multiple_subscriptions() {
	// Create schema with subscription support
	let storage = UserStorage::new();
	let broadcaster = EventBroadcaster::new();

	let schema = Schema::build(Query, Mutation, SubscriptionRoot)
		.data(storage)
		.data(broadcaster.clone())
		.finish();

	// Create multiple different subscription types
	let sub1_query = r#"subscription { userCreated { id name } }"#;
	let sub2_query = r#"subscription { userUpdated { id name active } }"#;
	let sub3_query = r#"subscription { userDeleted }"#;

	let mut stream1 = schema.execute_stream(sub1_query);
	let mut stream2 = schema.execute_stream(sub2_query);
	let mut stream3 = schema.execute_stream(sub3_query);

	// Broadcast mixed events in a separate task
	let broadcaster_clone = broadcaster.clone();
	tokio::spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		// Send Created event (should be received by stream1)
		broadcaster_clone
			.broadcast(UserEvent::Created(User {
				id: ID::from("multi-1"),
				name: "Created User".to_string(),
				email: "created@example.com".to_string(),
				active: true,
			}))
			.await;

		tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

		// Send Updated event (should be received by stream2)
		broadcaster_clone
			.broadcast(UserEvent::Updated(User {
				id: ID::from("multi-2"),
				name: "Updated User".to_string(),
				email: "updated@example.com".to_string(),
				active: false,
			}))
			.await;

		tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

		// Send Deleted event (should be received by stream3)
		broadcaster_clone
			.broadcast(UserEvent::Deleted(ID::from("multi-3")))
			.await;
	});

	// Wait for and verify Created event on stream1
	let event1 = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream1.next())
		.await
		.expect("Timeout waiting for created event")
		.expect("Stream1 should yield an event");

	assert!(event1.errors.is_empty());
	let data1 = event1.data.into_json().unwrap();
	assert!(
		data1["userCreated"]["id"]
			.as_str()
			.unwrap()
			.contains("multi-1")
	);
	assert_eq!(data1["userCreated"]["name"], "Created User");

	// Wait for and verify Updated event on stream2
	let event2 = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream2.next())
		.await
		.expect("Timeout waiting for updated event")
		.expect("Stream2 should yield an event");

	assert!(event2.errors.is_empty());
	let data2 = event2.data.into_json().unwrap();
	assert!(
		data2["userUpdated"]["id"]
			.as_str()
			.unwrap()
			.contains("multi-2")
	);
	assert_eq!(data2["userUpdated"]["name"], "Updated User");
	assert!(!data2["userUpdated"]["active"].as_bool().unwrap());

	// Wait for and verify Deleted event on stream3
	let event3 = tokio::time::timeout(tokio::time::Duration::from_secs(2), stream3.next())
		.await
		.expect("Timeout waiting for deleted event")
		.expect("Stream3 should yield an event");

	assert!(event3.errors.is_empty());
	let data3 = event3.data.into_json().unwrap();
	assert!(data3["userDeleted"].as_str().unwrap().contains("multi-3"));
}
