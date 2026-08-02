//! Cross-backend integration coverage for typed QuerySet retrieval helpers.

use reinhardt::db::orm::{DatabaseConnectionLease, QuerySet};
use reinhardt::model;
use reinhardt_test::fixtures::{postgres_container, testcontainers::mysql_container};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use sqlx::{MySqlPool, PgPool};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

#[model(
	app_label = "events",
	table_name = "queryset_retrieval_events",
	get_latest_by = ("created_at", "id")
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct RetrievalEvent {
	#[field(primary_key = true)]
	id: Option<i64>,
	created_at: i64,
	#[field(max_length = 64, unique = true)]
	slug: Option<String>,
}

async fn assert_retrieval_contract(url: &str) {
	let owner = reinhardt::db::backends::DatabaseConnection::connect(url)
		.await
		.expect("backend connection should initialize");
	let lease =
		DatabaseConnectionLease::register(owner).expect("ORM connection should register for test");
	let mut connection = lease.handle();
	connection
		.execute(
			"CREATE TABLE queryset_retrieval_events (
				id BIGINT PRIMARY KEY,
				created_at BIGINT NOT NULL,
				slug VARCHAR(64) NOT NULL UNIQUE
			)",
			vec![],
		)
		.await
		.expect("retrieval event table should be created");
	connection
		.execute(
			"INSERT INTO queryset_retrieval_events (id, created_at, slug) VALUES
				(1, 100, 'first'),
				(2, 200, 'second'),
				(3, 300, 'third')",
			vec![],
		)
		.await
		.expect("retrieval event rows should be inserted");

	let queryset = QuerySet::<RetrievalEvent>::new();
	assert_eq!(
		queryset
			.latest_with_db(&mut connection)
			.await
			.expect("latest row should load")
			.id,
		Some(3)
	);
	assert_eq!(
		queryset
			.earliest_by_with_db(&mut connection, &[RetrievalEvent::ordering_created_at()],)
			.await
			.expect("earliest typed row should load")
			.id,
		Some(1)
	);

	let by_id = queryset
		.in_bulk_with_db(&mut connection, [3_i64, 1, 3, 99])
		.await
		.expect("primary-key bulk retrieval should load");
	assert_eq!(by_id.keys().copied().collect::<Vec<_>>(), vec![1, 3]);
	let values_by_id = queryset
		.clone()
		.values(&["created_at", "slug"])
		.in_bulk_with_db(&mut connection, [3_i64, 1])
		.await
		.expect("primary-key bulk retrieval should retain an omitted lookup column");
	assert_eq!(values_by_id.keys().copied().collect::<Vec<_>>(), vec![1, 3]);
	let deferred_by_id = queryset
		.clone()
		.defer(&["id"])
		.in_bulk_with_db(&mut connection, [3_i64, 1])
		.await
		.expect("primary-key bulk retrieval should retain a deferred lookup column");
	assert_eq!(
		deferred_by_id.keys().copied().collect::<Vec<_>>(),
		vec![1, 3]
	);
	let by_slug = queryset
		.in_bulk_by_with_db(
			&mut connection,
			RetrievalEvent::unique_slug(),
			[
				"third".to_string(),
				"missing".to_string(),
				"first".to_string(),
				"third".to_string(),
			],
		)
		.await
		.expect("unique-field bulk retrieval should load");
	assert_eq!(
		by_slug.keys().cloned().collect::<Vec<_>>(),
		vec!["first".to_string(), "third".to_string()]
	);
	let only_by_slug = queryset
		.clone()
		.only(&["id", "created_at"])
		.in_bulk_by_with_db(
			&mut connection,
			RetrievalEvent::unique_slug(),
			["third".to_string(), "first".to_string()],
		)
		.await
		.expect("unique-field bulk retrieval should retain an omitted lookup column");
	assert_eq!(
		only_by_slug.keys().cloned().collect::<Vec<_>>(),
		vec!["first".to_string(), "third".to_string()]
	);
	assert_eq!(
		queryset
			.clone()
			.none()
			.count_with_db(&mut connection)
			.await
			.expect("empty queryset count should short-circuit"),
		0
	);

	let transaction_latest_id = connection
		.atomic(async |transaction| {
			Ok::<_, reinhardt_core::exception::Error>(
				queryset.latest_with_db(transaction).await?.id,
			)
		})
		.await
		.expect("transaction-bound latest retrieval should load");
	assert_eq!(transaction_latest_id, Some(3));
}

#[rstest]
#[tokio::test]
#[serial(queryset_retrieval_database)]
async fn sqlite_typed_queryset_retrieval_contract() {
	assert_retrieval_contract("sqlite::memory:").await;
}

#[rstest]
#[tokio::test]
#[serial(queryset_retrieval_database)]
async fn postgres_typed_queryset_retrieval_contract(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;
	assert_retrieval_contract(&url).await;
}

#[rstest]
#[tokio::test]
#[serial(queryset_retrieval_database)]
async fn mysql_typed_queryset_retrieval_contract(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, _pool, _port, url) = mysql_container.await;
	assert_retrieval_contract(&url).await;
}
