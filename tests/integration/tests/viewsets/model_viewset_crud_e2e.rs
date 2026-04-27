//! End-to-end regression test for ModelViewSet/ReadOnlyModelViewSet CRUD wiring.
//!
//! Issue #3985: Prior to the fix, `ModelViewSet::dispatch` returned placeholder
//! responses (`json!([])` for list, `json!({})` for retrieve, etc.) regardless
//! of database state. The router-backed code path was therefore broken even
//! though the public API was advertised as full CRUD.
//!
//! These tests guard against that regression class by exercising the real
//! `DefaultRouter` → `ModelViewSet` → `ModelViewSetHandler` → PostgreSQL path
//! and asserting that response bodies contain real model data — never empty
//! placeholders.

use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_apps::Request;
use reinhardt_db::orm::query_types::DbBackend;
use reinhardt_macros::model;
use reinhardt_test::fixtures::testcontainers::{ContainerAsync, GenericImage, postgres_container};
use reinhardt_urls::routers::{DefaultRouter, Router};
use reinhardt_views::viewsets::{ModelViewSet, ReadOnlyModelViewSet};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::AnyPool;
use sqlx::any::install_default_drivers;
use std::sync::Arc;

#[allow(dead_code)]
#[model(table_name = "items")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

#[derive(Debug, Clone)]
struct ItemSerializer;

/// Convert the typed `PgPool` from the shared fixture into the type-erased
/// `AnyPool` that `ModelViewSetHandler::with_pool` expects, then create the
/// `items` table.
async fn pool_with_items_table(pg_url: &str) -> Arc<AnyPool> {
	install_default_drivers();
	let pool = AnyPool::connect(pg_url)
		.await
		.expect("failed to connect AnyPool to test postgres");
	sqlx::query("DROP TABLE IF EXISTS items")
		.execute(&pool)
		.await
		.expect("failed to drop items table");
	sqlx::query("CREATE TABLE items (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL)")
		.execute(&pool)
		.await
		.expect("failed to create items table");
	Arc::new(pool)
}

fn list_request(uri: &str) -> Request {
	Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap()
}

fn create_request(uri: &str, body: &'static str) -> Request {
	Request::builder()
		.method(Method::POST)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(body))
		.build()
		.unwrap()
}

#[rstest]
#[tokio::test]
async fn modelviewset_create_returns_real_data_not_placeholder(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_items_table(&pg_url).await;

	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<Item, ItemSerializer>> = Arc::new(
		ModelViewSet::new("items")
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
	);
	router.register_viewset("items", viewset);

	// Act
	let resp = router
		.route(create_request("/items/", r#"{"id":0,"name":"alpha"}"#))
		.await
		.expect("create should succeed");

	// Assert: must NOT return the placeholder `{}` body. Status must be 201.
	assert_eq!(resp.status, StatusCode::CREATED);
	let created: serde_json::Value =
		serde_json::from_slice(&resp.body).expect("create response body must be JSON");
	assert!(
		created.is_object(),
		"REGRESSION GUARD (#3985): create must return a real JSON object, not the placeholder"
	);
	assert_eq!(
		created["name"], "alpha",
		"REGRESSION GUARD (#3985): create response must echo the persisted name"
	);
}

#[rstest]
#[tokio::test]
async fn modelviewset_list_returns_real_rows_from_database(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange: seed the table directly with raw SQL so this test isolates the
	// "list returns real rows" regression from the create flow.
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_items_table(&pg_url).await;
	sqlx::query("INSERT INTO items (name) VALUES ('alpha'), ('beta')")
		.execute(pool.as_ref())
		.await
		.expect("seed items rows");

	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<Item, ItemSerializer>> = Arc::new(
		ModelViewSet::new("items")
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
	);
	router.register_viewset("items", viewset);

	// Act
	let resp = router
		.route(list_request("/items/"))
		.await
		.expect("list should succeed");

	// Assert
	assert_eq!(resp.status, StatusCode::OK);
	let list: Vec<serde_json::Value> =
		serde_json::from_slice(&resp.body).expect("list response body must be a JSON array");
	assert!(
		!list.is_empty(),
		"REGRESSION GUARD (#3985): GET /items/ must return real rows from the database, \
		 not the placeholder `[]`"
	);
	let names: Vec<&str> = list.iter().filter_map(|v| v["name"].as_str()).collect();
	assert!(names.contains(&"alpha"));
	assert!(names.contains(&"beta"));
}

#[rstest]
#[tokio::test]
async fn readonlymodelviewset_list_returns_real_rows(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_items_table(&pg_url).await;
	sqlx::query("INSERT INTO items (name) VALUES ('gamma')")
		.execute(pool.as_ref())
		.await
		.expect("seed items rows");

	let mut router = DefaultRouter::new();
	let viewset: Arc<ReadOnlyModelViewSet<Item, ItemSerializer>> = Arc::new(
		ReadOnlyModelViewSet::new("items")
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
	);
	router.register_viewset("items", viewset);

	// Act
	let resp = router
		.route(list_request("/items/"))
		.await
		.expect("list should succeed");

	// Assert
	assert_eq!(resp.status, StatusCode::OK);
	let list: Vec<serde_json::Value> =
		serde_json::from_slice(&resp.body).expect("list response body must be a JSON array");
	assert!(
		!list.is_empty(),
		"REGRESSION GUARD (#3985): ReadOnlyModelViewSet GET /items/ must return real rows, \
		 not the placeholder `[]`"
	);
	assert!(list.iter().any(|v| v["name"] == "gamma"));
}

#[rstest]
#[tokio::test]
async fn readonlymodelviewset_rejects_writes(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_items_table(&pg_url).await;

	let mut router = DefaultRouter::new();
	let viewset: Arc<ReadOnlyModelViewSet<Item, ItemSerializer>> = Arc::new(
		ReadOnlyModelViewSet::new("items")
			.with_pool(pool)
			.with_db_backend(DbBackend::Postgres),
	);
	router.register_viewset("items", viewset);

	// Act: POST should be rejected by ReadOnlyModelViewSet's dispatch.
	let result = router
		.route(create_request("/items/", r#"{"id":0,"name":"delta"}"#))
		.await;

	// Assert: this must not silently return 201 (placeholder regression).
	match result {
		Ok(resp) => assert_ne!(
			resp.status,
			StatusCode::CREATED,
			"REGRESSION GUARD (#3985): ReadOnlyModelViewSet must NOT return 201 on POST"
		),
		Err(e) => {
			let s = e.to_string();
			assert!(
				s.contains("Method") || s.contains("method"),
				"expected MethodNotAllowed-style error, got: {s}"
			);
		}
	}
}
