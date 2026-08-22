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
use reinhardt_db::orm::{
	CustomManager, Filter, FilterOperator, FilterValue, Manager, QuerySet, query_types::DbBackend,
};
use reinhardt_macros::model;
use reinhardt_rest::serializers::JsonSerializer;
use reinhardt_test::fixtures::testcontainers::{ContainerAsync, GenericImage, postgres_container};
use reinhardt_urls::routers::{DefaultRouter, Router};
use reinhardt_views::viewsets::{ModelViewSet, QuerySetProvider, ReadOnlyModelViewSet, ViewError};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::AnyPool;
use sqlx::any::install_default_drivers;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Once};

#[allow(dead_code)]
#[model(app_label = "default", table_name = "items")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

type ItemSerializer = JsonSerializer<Item>;

#[derive(Default)]
struct VisibleScopedItemManager;

impl CustomManager for VisibleScopedItemManager {
	type Model = ScopedItem;

	fn new() -> Self {
		Self
	}

	fn all(&self) -> QuerySet<ScopedItem> {
		Manager::<ScopedItem>::new()
			.all()
			.filter(ScopedItem::field_is_archived().eq(false))
	}
}

#[model(
	app_label = "default",
	table_name = "scoped_items",
	manager = VisibleScopedItemManager
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScopedItem {
	#[field(primary_key = true)]
	id: Option<i64>,
	organization_id: i64,
	is_archived: bool,
	#[field(max_length = 255)]
	name: String,
}

#[model(app_label = "default", table_name = "integer_key_items")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IntegerKeyItem {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

#[model(app_label = "default", table_name = "string_key_items")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StringKeyItem {
	#[field(primary_key = true, db_column = "record_key", max_length = 32)]
	id: String,
	#[field(max_length = 255)]
	name: String,
}

#[model(app_label = "default", table_name = "uuid_key_items")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UuidKeyItem {
	#[field(primary_key = true)]
	id: uuid::Uuid,
	#[field(max_length = 255)]
	name: String,
}

#[model(app_label = "default", table_name = "timestamp_key_items")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimestampKeyItem {
	#[field(primary_key = true)]
	id: chrono::DateTime<chrono::Utc>,
	#[field(max_length = 255)]
	name: String,
}

#[derive(Clone, Copy, Debug)]
struct OrganizationId(i64);

struct ScopedItemProvider<F>(F);

impl<F> QuerySetProvider<ScopedItem> for ScopedItemProvider<F>
where
	F: Fn(&Request, QuerySet<ScopedItem>) -> std::result::Result<QuerySet<ScopedItem>, ViewError>
		+ Send
		+ Sync,
{
	fn get_queryset(
		&self,
		request: &Request,
		base: QuerySet<ScopedItem>,
	) -> std::result::Result<QuerySet<ScopedItem>, ViewError> {
		(self.0)(request, base)
	}
}

fn scoped_request(
	method: Method,
	uri: &str,
	body: &'static str,
	organization_id: Option<i64>,
) -> Request {
	let request = Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(body))
		.build()
		.unwrap();
	if let Some(organization_id) = organization_id {
		request.extensions.insert(OrganizationId(organization_id));
	}
	request
}

fn organization_queryset(
	request: &Request,
	base: QuerySet<ScopedItem>,
) -> std::result::Result<QuerySet<ScopedItem>, ViewError> {
	let organization = request
		.extensions
		.get::<OrganizationId>()
		.ok_or_else(|| ViewError::Permission("organization scope is missing".to_owned()))?;
	Ok(base.filter(Filter::new(
		"organization_id",
		FilterOperator::Eq,
		FilterValue::Integer(organization.0),
	)))
}

async fn any_pool(pg_url: &str) -> Arc<AnyPool> {
	static INSTALL_DRIVERS: Once = Once::new();
	INSTALL_DRIVERS.call_once(install_default_drivers);
	Arc::new(
		AnyPool::connect(pg_url)
			.await
			.expect("failed to connect AnyPool to test postgres"),
	)
}

/// Convert the typed `PgPool` from the shared fixture into the type-erased
/// `AnyPool` that `ModelViewSetHandler::with_pool` expects, then create the
/// `items` table.
async fn pool_with_items_table(pg_url: &str) -> Arc<AnyPool> {
	let pool = any_pool(pg_url).await;
	sqlx::query("DROP TABLE IF EXISTS items")
		.execute(pool.as_ref())
		.await
		.expect("failed to drop items table");
	sqlx::query("CREATE TABLE items (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL)")
		.execute(pool.as_ref())
		.await
		.expect("failed to create items table");
	pool
}

async fn pool_with_scoped_items_table(pg_url: &str) -> Arc<AnyPool> {
	let pool = any_pool(pg_url).await;
	sqlx::query("DROP TABLE IF EXISTS scoped_items")
		.execute(pool.as_ref())
		.await
		.expect("failed to drop scoped_items table");
	sqlx::query(
		"CREATE TABLE scoped_items (\
			id BIGSERIAL PRIMARY KEY, \
			organization_id BIGINT NOT NULL DEFAULT 1, \
			is_archived BOOLEAN NOT NULL, \
			name TEXT NULL\
		)",
	)
	.execute(pool.as_ref())
	.await
	.expect("failed to create scoped_items table");
	sqlx::query(
		"INSERT INTO scoped_items (id, organization_id, is_archived, name) VALUES \
			(1, 1, FALSE, 'own'), \
			(2, 1, TRUE, NULL), \
			(3, 2, FALSE, NULL)",
	)
	.execute(pool.as_ref())
	.await
	.expect("failed to seed scoped_items table");
	sqlx::query("SELECT setval('scoped_items_id_seq', 3)")
		.execute(pool.as_ref())
		.await
		.expect("failed to advance scoped_items sequence");
	pool
}

async fn assert_scoped_list_and_detail(router: &DefaultRouter) {
	let list = router
		.route(scoped_request(Method::GET, "/scoped-items/", "", Some(1)))
		.await
		.unwrap();
	assert_eq!(list.status, StatusCode::OK);
	let body: Vec<serde_json::Value> = serde_json::from_slice(&list.body).unwrap();
	assert_eq!(body.len(), 1);
	assert_eq!(body[0]["id"], 1);
	assert_eq!(body[0]["name"], "own");

	let own = router
		.route(scoped_request(Method::GET, "/scoped-items/1/", "", Some(1)))
		.await
		.unwrap();
	assert_eq!(own.status, StatusCode::OK);

	let archived = router
		.route(scoped_request(Method::GET, "/scoped-items/2/", "", Some(1)))
		.await
		.unwrap_err();
	assert!(matches!(
		archived,
		reinhardt_core::exception::Error::NotFound(_)
	));

	let foreign = router
		.route(scoped_request(Method::GET, "/scoped-items/3/", "", Some(1)))
		.await
		.unwrap_err();
	assert!(matches!(
		foreign,
		reinhardt_core::exception::Error::NotFound(_)
	));
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
async fn modelviewset_create_with_existing_primary_key_does_not_update_row(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_items_table(&pg_url).await;
	sqlx::query("INSERT INTO items (id, name) VALUES (1, 'original')")
		.execute(pool.as_ref())
		.await
		.unwrap();

	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<Item, ItemSerializer>> = Arc::new(
		ModelViewSet::new("items")
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
	);
	router.register_viewset("items", viewset);

	// Act
	let result = router
		.route(create_request("/items/", r#"{"id":1,"name":"replaced"}"#))
		.await;

	// Assert
	assert!(result.is_err());
	let existing_name = sqlx::query_scalar::<_, String>("SELECT name FROM items WHERE id = 1")
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(existing_name, "original");
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

#[rstest]
#[tokio::test]
async fn model_and_readonly_viewsets_queryset_provider_scope_list_and_detail_in_database(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_scoped_items_table(&pg_url).await;

	let mut model_router = DefaultRouter::new();
	let model_viewset: Arc<ModelViewSet<ScopedItem, JsonSerializer<ScopedItem>>> = Arc::new(
		ModelViewSet::new("scoped-items")
			.with_queryset_provider(ScopedItemProvider(organization_queryset))
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
	);
	model_router.register_viewset("scoped-items", model_viewset);

	let mut readonly_router = DefaultRouter::new();
	let readonly_viewset: Arc<ReadOnlyModelViewSet<ScopedItem, JsonSerializer<ScopedItem>>> =
		Arc::new(
			ReadOnlyModelViewSet::new("scoped-items")
				.with_queryset_provider(ScopedItemProvider(organization_queryset))
				.with_pool(pool)
				.with_db_backend(DbBackend::Postgres),
		);
	readonly_router.register_viewset("scoped-items", readonly_viewset);

	// Act and assert
	assert_scoped_list_and_detail(&model_router).await;
	assert_scoped_list_and_detail(&readonly_router).await;
}

#[rstest]
#[tokio::test]
async fn modelviewset_queryset_provider_blocks_cross_scope_update_and_destroy(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_scoped_items_table(&pg_url).await;
	sqlx::query(
		"INSERT INTO scoped_items (id, organization_id, is_archived, name) VALUES \
			(4, 2, FALSE, 'foreign'), \
			(5, 2, FALSE, 'foreign-delete'), \
			(6, 1, FALSE, 'own-delete')",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();

	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<ScopedItem, JsonSerializer<ScopedItem>>> = Arc::new(
		ModelViewSet::new("scoped-items")
			.with_queryset_provider(ScopedItemProvider(organization_queryset))
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
	);
	router.register_viewset("scoped-items", viewset);

	// Act
	let own_update = router
		.route(scoped_request(
			Method::PATCH,
			"/scoped-items/1/",
			r#"{"name":"updated"}"#,
			Some(1),
		))
		.await
		.unwrap();
	let own_destroy = router
		.route(scoped_request(
			Method::DELETE,
			"/scoped-items/6/",
			"",
			Some(1),
		))
		.await
		.unwrap();
	let foreign_update = router
		.route(scoped_request(
			Method::PATCH,
			"/scoped-items/4/",
			r#"{"name":"intruder"}"#,
			Some(1),
		))
		.await
		.unwrap_err();
	let foreign_destroy = router
		.route(scoped_request(
			Method::DELETE,
			"/scoped-items/5/",
			"",
			Some(1),
		))
		.await
		.unwrap_err();

	// Assert
	assert_eq!(own_update.status, StatusCode::OK);
	assert_eq!(own_destroy.status, StatusCode::NO_CONTENT);
	assert!(matches!(
		foreign_update,
		reinhardt_core::exception::Error::NotFound(_)
	));
	assert!(matches!(
		foreign_destroy,
		reinhardt_core::exception::Error::NotFound(_)
	));

	let own_name = sqlx::query_scalar::<_, String>("SELECT name FROM scoped_items WHERE id = 1")
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(own_name, "updated");
	let own_delete_count =
		sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scoped_items WHERE id = 6")
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(own_delete_count, 0);

	let foreign_name =
		sqlx::query_scalar::<_, String>("SELECT name FROM scoped_items WHERE id = 4")
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(foreign_name, "foreign");

	let foreign_count =
		sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scoped_items WHERE id = 5")
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(foreign_count, 1);
}

#[rstest]
#[tokio::test]
async fn modelviewset_scoped_update_preserves_route_primary_key(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_scoped_items_table(&pg_url).await;
	sqlx::query(
		"INSERT INTO scoped_items (id, organization_id, is_archived, name) VALUES \
			(4, 2, FALSE, 'foreign')",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();

	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<ScopedItem, JsonSerializer<ScopedItem>>> = Arc::new(
		ModelViewSet::new("scoped-items")
			.with_queryset_provider(ScopedItemProvider(organization_queryset))
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
	);
	router.register_viewset("scoped-items", viewset);

	// Act
	let response = router
		.route(scoped_request(
			Method::PATCH,
			"/scoped-items/1/",
			r#"{"id":4,"name":"updated"}"#,
			Some(1),
		))
		.await
		.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
	assert_eq!(body["id"], 1);
	assert_eq!(body["name"], "updated");

	let own_row = sqlx::query_as::<_, (i64, i64, bool, String)>(
		"SELECT id, organization_id, is_archived, name FROM scoped_items WHERE id = 1",
	)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(own_row, (1, 1, false, "updated".to_owned()));

	let foreign_row = sqlx::query_as::<_, (i64, i64, bool, String)>(
		"SELECT id, organization_id, is_archived, name FROM scoped_items WHERE id = 4",
	)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(foreign_row, (4, 2, false, "foreign".to_owned()));
}

#[rstest]
#[tokio::test]
async fn modelviewset_scoped_create_with_existing_primary_key_does_not_update_row(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_scoped_items_table(&pg_url).await;
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<ScopedItem, JsonSerializer<ScopedItem>>> = Arc::new(
		ModelViewSet::new("scoped-items")
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
	);
	router.register_viewset("scoped-items", viewset);

	// Act
	let result = router
		.route(create_request(
			"/scoped-items/",
			r#"{"id":1,"organization_id":2,"is_archived":true,"name":"replaced"}"#,
		))
		.await;

	// Assert
	assert!(result.is_err());
	let existing_row = sqlx::query_as::<_, (i64, i64, bool, String)>(
		"SELECT id, organization_id, is_archived, name FROM scoped_items WHERE id = 1",
	)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(existing_row, (1, 1, false, "own".to_owned()));
}

#[rstest]
#[tokio::test]
async fn modelviewset_detail_uses_typed_primary_key_filters(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = any_pool(&pg_url).await;
	sqlx::query("CREATE TABLE integer_key_items (id BIGINT PRIMARY KEY, name TEXT NULL)")
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query("INSERT INTO integer_key_items (id, name) VALUES (42, 'integer'), (43, NULL)")
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query("CREATE TABLE string_key_items (record_key TEXT PRIMARY KEY, name TEXT NULL)")
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query(
		"INSERT INTO string_key_items (record_key, name) VALUES ('0007', 'string'), ('poison', NULL)",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query("CREATE TABLE uuid_key_items (id UUID PRIMARY KEY, name TEXT NULL)")
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query(
		"INSERT INTO uuid_key_items (id, name) VALUES \
			('67e55044-10b1-426f-9247-bb680e5fe0c8', 'uuid'), \
			('123e4567-e89b-12d3-a456-426614174000', NULL)",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query("CREATE TABLE timestamp_key_items (id TIMESTAMPTZ PRIMARY KEY, name TEXT NULL)")
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query(
		"INSERT INTO timestamp_key_items (id, name) VALUES \
			('2026-08-19T00:00:00Z', 'timestamp'), \
			('2026-08-20T00:00:00Z', NULL)",
	)
	.execute(pool.as_ref())
	.await
	.unwrap();

	let mut integer_router = DefaultRouter::new();
	integer_router.register_viewset(
		"integer-key-items",
		Arc::new(
			ModelViewSet::<IntegerKeyItem, JsonSerializer<IntegerKeyItem>>::new(
				"integer-key-items",
			)
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
		),
	);
	let mut string_router = DefaultRouter::new();
	string_router.register_viewset(
		"string-key-items",
		Arc::new(
			ModelViewSet::<StringKeyItem, JsonSerializer<StringKeyItem>>::new("string-key-items")
				.with_pool(pool.clone())
				.with_db_backend(DbBackend::Postgres),
		),
	);
	let mut uuid_router = DefaultRouter::new();
	uuid_router.register_viewset(
		"uuid-key-items",
		Arc::new(
			ModelViewSet::<UuidKeyItem, JsonSerializer<UuidKeyItem>>::new("uuid-key-items")
				.with_pool(pool.clone())
				.with_db_backend(DbBackend::Postgres),
		),
	);
	let mut timestamp_router = DefaultRouter::new();
	timestamp_router.register_viewset(
		"timestamp-key-items",
		Arc::new(
			ModelViewSet::<TimestampKeyItem, JsonSerializer<TimestampKeyItem>>::new(
				"timestamp-key-items",
			)
			.with_pool(pool.clone())
			.with_db_backend(DbBackend::Postgres),
		),
	);

	// Act
	let integer = integer_router
		.route(scoped_request(
			Method::GET,
			"/integer-key-items/42/",
			"",
			None,
		))
		.await
		.unwrap();
	let string = string_router
		.route(scoped_request(
			Method::GET,
			"/string-key-items/0007/",
			"",
			None,
		))
		.await
		.unwrap();
	let uuid = uuid_router
		.route(scoped_request(
			Method::GET,
			"/uuid-key-items/67e55044-10b1-426f-9247-bb680e5fe0c8/",
			"",
			None,
		))
		.await
		.unwrap();
	// The router matches the raw timestamp because it does not percent-decode paths.
	let timestamp = timestamp_router
		.route(scoped_request(
			Method::GET,
			"/timestamp-key-items/2026-08-19T00:00:00Z/",
			"",
			None,
		))
		.await
		.unwrap();
	let missing_string = string_router
		.route(scoped_request(
			Method::GET,
			"/string-key-items/7/",
			"",
			None,
		))
		.await
		.unwrap_err();

	// Assert
	assert_eq!(integer.status, StatusCode::OK);
	assert_eq!(string.status, StatusCode::OK);
	assert_eq!(uuid.status, StatusCode::OK);
	assert_eq!(timestamp.status, StatusCode::OK);
	assert_eq!(
		serde_json::from_slice::<serde_json::Value>(&integer.body).unwrap()["id"],
		42
	);
	assert_eq!(
		serde_json::from_slice::<serde_json::Value>(&string.body).unwrap()["id"],
		"0007"
	);
	assert_eq!(
		serde_json::from_slice::<serde_json::Value>(&uuid.body).unwrap()["id"],
		"67e55044-10b1-426f-9247-bb680e5fe0c8"
	);
	assert_eq!(
		serde_json::from_slice::<serde_json::Value>(&timestamp.body).unwrap()["id"],
		"2026-08-19T00:00:00Z"
	);
	assert!(matches!(
		missing_string,
		reinhardt_core::exception::Error::NotFound(_)
	));

	sqlx::query("DROP TABLE integer_key_items")
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query("DROP TABLE uuid_key_items")
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query("DROP TABLE timestamp_key_items")
		.execute(pool.as_ref())
		.await
		.unwrap();

	let malformed_integer = integer_router
		.route(scoped_request(
			Method::GET,
			"/integer-key-items/not-int/",
			"",
			None,
		))
		.await
		.unwrap_err();
	let malformed_uuid = uuid_router
		.route(scoped_request(
			Method::GET,
			"/uuid-key-items/bad-uuid/",
			"",
			None,
		))
		.await
		.unwrap_err();
	let malformed_timestamp = timestamp_router
		.route(scoped_request(
			Method::GET,
			"/timestamp-key-items/bad-time/",
			"",
			None,
		))
		.await
		.unwrap_err();
	assert!(matches!(
		malformed_integer,
		reinhardt_core::exception::Error::NotFound(_)
	));
	assert!(matches!(
		malformed_uuid,
		reinhardt_core::exception::Error::NotFound(_)
	));
	assert!(matches!(
		malformed_timestamp,
		reinhardt_core::exception::Error::NotFound(_)
	));
}

#[rstest]
#[tokio::test]
async fn modelviewset_create_skips_queryset_provider_and_refetches_by_primary_key(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = pool_with_scoped_items_table(&pg_url).await;
	let calls = Arc::new(AtomicUsize::new(0));
	let hook_calls = calls.clone();
	let viewset = ModelViewSet::<ScopedItem, JsonSerializer<ScopedItem>>::new("scoped-items")
		.with_queryset_provider(ScopedItemProvider(
			move |_request: &Request, _base: QuerySet<ScopedItem>| {
				hook_calls.fetch_add(1, Ordering::SeqCst);
				Err(ViewError::Permission("hook must not run".to_owned()))
			},
		))
		.with_pool(pool)
		.with_db_backend(DbBackend::Postgres);
	let mut router = DefaultRouter::new();
	router.register_viewset("scoped-items", Arc::new(viewset));

	// Act
	let created = router
		.route(scoped_request(
			Method::POST,
			"/scoped-items/",
			r#"{"id":null,"organization_id":1,"is_archived":true,"name":"created"}"#,
			Some(1),
		))
		.await
		.unwrap();

	// Assert
	assert_eq!(created.status, StatusCode::CREATED);
	let body: serde_json::Value = serde_json::from_slice(&created.body).unwrap();
	assert_eq!(body["id"], 4);
	assert_eq!(body["name"], "created");
	assert_eq!(body["is_archived"], true);
	assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[rstest]
#[tokio::test]
async fn queryset_provider_errors_before_query_and_without_pool_fails_closed(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pg_pool, _port, pg_url) = postgres_container.await;
	let pool = any_pool(&pg_url).await;
	let mut hook_error_router = DefaultRouter::new();
	hook_error_router.register_viewset(
		"scoped-items",
		Arc::new(
			ModelViewSet::<ScopedItem, JsonSerializer<ScopedItem>>::new("scoped-items")
				.with_queryset_provider(ScopedItemProvider(
					|_request: &Request, _base: QuerySet<ScopedItem>| {
						Err(ViewError::Permission(
							"organization scope is missing".to_owned(),
						))
					},
				))
				.with_pool(pool)
				.with_db_backend(DbBackend::Postgres),
		),
	);

	let mut poolless_router = DefaultRouter::new();
	poolless_router.register_viewset(
		"poolless-items",
		Arc::new(
			ModelViewSet::<ScopedItem, JsonSerializer<ScopedItem>>::new("poolless-items")
				.with_queryset(vec![ScopedItem {
					id: Some(1),
					organization_id: 1,
					is_archived: false,
					name: "own".to_owned(),
				}])
				.with_queryset_provider(ScopedItemProvider(organization_queryset)),
		),
	);

	// Act
	let hook_error = hook_error_router
		.route(scoped_request(Method::GET, "/scoped-items/", "", None))
		.await
		.unwrap_err();
	let poolless = poolless_router
		.route(scoped_request(Method::GET, "/poolless-items/", "", Some(1)))
		.await
		.unwrap_err();

	// Assert
	assert!(matches!(
		hook_error,
		reinhardt_core::exception::Error::Authorization(_)
	));
	assert!(matches!(
		poolless,
		reinhardt_core::exception::Error::Internal(_)
	));
}
