use reinhardt_auth::sessions::backends::cache::SessionBackend;
use reinhardt_auth::sessions::backends::database::DatabaseSessionBackend;
use reinhardt_auth::sessions::cleanup::CleanupableBackend;
use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
use serial_test::serial;

#[tokio::test]
#[serial(sessions_db)]
async fn injected_connection_handles_session_lifecycle_without_global_orm_connection() {
	let connection = BackendsConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	let backend = DatabaseSessionBackend::from_connection(connection).unwrap();
	let session_key = "injected-connection";
	let session_data = serde_json::json!({"user_id": 42});

	backend.create_table().await.unwrap();
	backend
		.save(session_key, &session_data, Some(60))
		.await
		.unwrap();

	let loaded: Option<serde_json::Value> = backend.load(session_key).await.unwrap();
	assert_eq!(loaded, Some(session_data));
	assert!(backend.exists(session_key).await.unwrap());

	backend.delete(session_key).await.unwrap();

	assert!(!backend.exists(session_key).await.unwrap());
}

#[tokio::test]
#[serial(sessions_db)]
async fn injected_connection_counts_prefix_in_database() {
	let connection = BackendsConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	let backend = DatabaseSessionBackend::from_connection(connection).unwrap();
	backend.create_table().await.unwrap();

	for key in [
		"tenant:one",
		"tenant:two",
		"tenant:three",
		"tenant_123:one",
		"tenantX123:one",
		"other:one",
	] {
		backend
			.save(key, &serde_json::json!({}), Some(60))
			.await
			.unwrap();
	}

	assert_eq!(backend.count_keys_with_prefix("tenant:").await.unwrap(), 3);
	assert_eq!(
		backend.count_keys_with_prefix("tenant_123:").await.unwrap(),
		1
	);
}
