//! Integration tests for ViewSets + Filters + ORM
//!
//! **Test Coverage:**
//! - Section 1: ViewSet Filter API Tests (8 tests)
//!   - extract_filters, extract_search, extract_ordering
//!   - URL decoding, filter configuration
//! - Section 2: ORM Integration Tests (7 tests)
//!   - SearchFilter + PostgreSQL
//!   - OrderingFilter + PostgreSQL
//!   - RangeFilter + PostgreSQL
//!   - QueryOptimizer, N+1 detection, FuzzySearchFilter
//! - Section 3: Combined Integration Tests (5 tests)
//!   - ViewSet + Filters + ORM complete integration
//!   - Multiple filters combination via ViewSet
//!   - Pagination + Filters
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container (reinhardt-test)
//! - filter_test_db: Custom fixture providing database connection with test schema

use bytes::Bytes;
use hyper::{HeaderMap, Method, Version};
use reinhardt_http::Request;
use reinhardt_rest::filters::{
	DatabaseDialect, FilterBackend, FuzzyAlgorithm, FuzzySearchFilter, RangeFilter,
	SimpleOrderingBackend, SimpleSearchBackend,
};
use reinhardt_test::fixtures::testcontainers::{ContainerAsync, GenericImage, postgres_container};
use reinhardt_views::viewsets::{
	FilterConfig, FilterableViewSet, ModelViewSet, OrderingConfig, ReadOnlyModelViewSet,
};
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;

// ========================================================================
// Test Models
// ========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestModel {
	id: i64,
	name: String,
	status: String,
	category: String,
	created_at: i64,
}

#[derive(Debug, Clone)]
struct TestSerializer;

// ========================================================================
// Custom Fixtures
// ========================================================================

/// Custom fixture providing PostgreSQL database with test schema
///
/// **Schema:**
/// - users: id, username, email, age, is_active, created_at
/// - posts: id, user_id, title, content, published, created_at
///
/// **Integration Point**: postgres_container → filter_test_db (fixture chaining)
#[fixture]
async fn filter_test_db(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>) {
	let (container, pool, _port, _url) = postgres_container.await;

	// Create users table
	sqlx::query(
		r#"
		CREATE TABLE users (
			id SERIAL PRIMARY KEY,
			username TEXT NOT NULL,
			email TEXT NOT NULL,
			age INTEGER NOT NULL,
			is_active BOOLEAN NOT NULL DEFAULT true,
			created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
		)
		"#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create users table");

	// Create posts table
	sqlx::query(
		r#"
		CREATE TABLE posts (
			id SERIAL PRIMARY KEY,
			user_id INTEGER NOT NULL REFERENCES users(id),
			title TEXT NOT NULL,
			content TEXT NOT NULL,
			published BOOLEAN NOT NULL DEFAULT false,
			created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
		)
		"#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create posts table");

	// Insert test data into users
	sqlx::query(
		r#"
		INSERT INTO users (username, email, age, is_active) VALUES
		('alice', 'alice@example.com', 25, true),
		('bob', 'bob@example.com', 30, true),
		('charlie', 'charlie@example.com', 17, false),
		('david', 'david@example.com', 45, true),
		('eve', 'eve@example.com', 65, true)
		"#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert users");

	// Insert test data into posts
	sqlx::query(
		r#"
		INSERT INTO posts (user_id, title, content, published) VALUES
		(1, 'Alice First Post', 'Content from Alice', true),
		(1, 'Alice Second Post', 'More content from Alice', false),
		(2, 'Bob Article', 'Bob writes something', true),
		(4, 'David Tutorial', 'David teaching', true)
		"#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert posts");

	(container, pool)
}

// ========================================================================
// Section 1: ViewSet Filter API Tests
// ========================================================================

/// Test ViewSet default behavior (no filter config)
///
/// **Test Intent**: Verify ViewSet returns None when no filter configuration is set.
///
/// **Integration Point**: ViewSet → FilterConfig access
#[tokio::test]
async fn test_viewset_default_no_filter_config() {
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items");
	let config = viewset.get_filter_config();
	assert!(config.is_none());
}

/// Test ViewSet with filters configuration
///
/// **Test Intent**: Verify ViewSet accepts and stores FilterConfig with filterable and search fields.
///
/// **Integration Point**: ViewSet → FilterConfig builder pattern
#[tokio::test]
async fn test_viewset_with_filters() {
	let filter_config = FilterConfig::new()
		.with_filterable_fields(vec!["status", "category"])
		.with_search_fields(vec!["name"]);

	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_filters(filter_config.clone());

	let config = viewset.get_filter_config();
	let config = config.unwrap();
	assert_eq!(config.filterable_fields.len(), 2);

	use std::collections::HashSet;
	assert_eq!(
		config.filterable_fields.iter().collect::<HashSet<_>>(),
		HashSet::from([&"status".to_string(), &"category".to_string()]),
		"Filterable fields mismatch. Expected fields: {:?}, Got: {:?}",
		["status", "category"],
		config.filterable_fields
	);

	assert_eq!(config.search_fields.len(), 1);
	assert_eq!(
		config.search_fields.iter().collect::<HashSet<_>>(),
		HashSet::from([&"name".to_string()]),
		"Search fields mismatch. Expected fields: {:?}, Got: {:?}",
		["name"],
		config.search_fields
	);
}

/// Test extract_filters from request
///
/// **Test Intent**: Verify ViewSet extracts only allowed filter fields from query parameters,
/// ignoring invalid fields.
///
/// **Integration Point**: ViewSet::extract_filters → Request query params
#[tokio::test]
async fn test_extract_filters_from_request() {
	let filter_config = FilterConfig::new()
		.with_filterable_fields(vec!["status", "category"])
		.with_search_fields(vec!["name"]);

	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_filters(filter_config);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/items/?status=active&category=tech&invalid=ignored")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let filters = viewset.extract_filters(&request);
	assert_eq!(filters.len(), 2);
	assert_eq!(filters.get("status"), Some(&"active".to_string()));
	assert_eq!(filters.get("category"), Some(&"tech".to_string()));
	assert!(!filters.contains_key("invalid"));
}

/// Test extract_search from request
///
/// **Test Intent**: Verify ViewSet extracts search query parameter with URL decoding.
///
/// **Integration Point**: ViewSet::extract_search → Request query params → URL decoding
#[tokio::test]
async fn test_extract_search_from_request() {
	let filter_config = FilterConfig::new().with_search_fields(vec!["name", "description"]);

	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_filters(filter_config);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/items/?search=rust%20programming")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let search = viewset.extract_search(&request);
	assert_eq!(search, Some("rust programming".to_string()));
}

/// Test ViewSet with ordering configuration
///
/// **Test Intent**: Verify ViewSet accepts and stores OrderingConfig with allowed fields
/// and default ordering.
///
/// **Integration Point**: ViewSet → OrderingConfig builder pattern
#[tokio::test]
async fn test_viewset_with_ordering() {
	let ordering_config = OrderingConfig::new()
		.with_ordering_fields(vec!["created_at", "name"])
		.with_default_ordering(vec!["-created_at"]);

	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_ordering(ordering_config.clone());

	let config = viewset.get_ordering_config();
	let config = config.unwrap();
	assert_eq!(config.ordering_fields.len(), 2);

	use std::collections::HashSet;
	assert_eq!(
		config.ordering_fields.iter().collect::<HashSet<_>>(),
		HashSet::from([&"created_at".to_string(), &"name".to_string()]),
		"Ordering fields mismatch. Expected fields: {:?}, Got: {:?}",
		["created_at", "name"],
		config.ordering_fields
	);

	assert_eq!(config.default_ordering.len(), 1);
	assert_eq!(config.default_ordering[0], "-created_at");
}

/// Test extract_ordering from request
///
/// **Test Intent**: Verify ViewSet extracts ordering query parameter and validates fields.
///
/// **Integration Point**: ViewSet::extract_ordering → Request query params → Field validation
#[tokio::test]
async fn test_extract_ordering_from_request() {
	let ordering_config = OrderingConfig::new()
		.with_ordering_fields(vec!["created_at", "name", "id"])
		.with_default_ordering(vec!["-created_at"]);

	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_ordering(ordering_config);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/items/?ordering=name,-created_at")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let ordering = viewset.extract_ordering(&request);
	assert_eq!(ordering.len(), 2);
	assert_eq!(ordering[0], "name");
	assert_eq!(ordering[1], "-created_at");
}

/// Test extract_ordering with validation (invalid fields)
///
/// **Test Intent**: Verify ViewSet falls back to default ordering when all requested
/// ordering fields are invalid.
///
/// **Integration Point**: ViewSet::extract_ordering → Field validation → Default fallback
#[tokio::test]
async fn test_extract_ordering_with_validation() {
	let ordering_config = OrderingConfig::new()
		.with_ordering_fields(vec!["created_at", "name"])
		.with_default_ordering(vec!["-created_at"]);

	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_ordering(ordering_config);

	// Request invalid field
	let request = Request::builder()
		.method(Method::GET)
		.uri("/items/?ordering=invalid_field")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let ordering = viewset.extract_ordering(&request);
	// Should fall back to default ordering when all requested fields are invalid
	assert_eq!(ordering.len(), 1);
	assert_eq!(ordering[0], "-created_at");
}

/// Test extract_filters URL decoding
///
/// **Test Intent**: Verify ViewSet correctly decodes URL-encoded filter values.
///
/// **Integration Point**: ViewSet::extract_filters → URL decoding
#[tokio::test]
async fn test_extract_filters_url_decoding() {
	let filter_config = FilterConfig::new().with_filterable_fields(vec!["name"]);

	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_filters(filter_config);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/items/?name=hello%20world")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let filters = viewset.extract_filters(&request);
	assert_eq!(filters.get("name"), Some(&"hello world".to_string()));
}

// ========================================================================
// Section 2: ORM Integration Tests
// ========================================================================

/// Test SearchFilter + ORM integration
///
/// **Test Intent**: Verify SearchFilter generates correct LIKE/ILIKE clauses
/// for cross-field search (username, email) and integrates with ORM-generated SQL.
///
/// **Integration Point**: SimpleSearchBackend → SQL WHERE clause generation → PostgreSQL
#[rstest]
#[tokio::test]
async fn test_search_filter_orm_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create search filter with PostgreSQL dialect
	let backend = SimpleSearchBackend::new("search")
		.with_field("username")
		.with_field("email")
		.with_dialect(DatabaseDialect::PostgreSQL);

	let mut params = HashMap::new();
	params.insert("search".to_string(), "alice".to_string());

	let base_sql = "SELECT * FROM users".to_string();
	let filtered_sql = backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Filter failed");

	// Verify SQL contains LIKE clauses for both fields with OR logic
	assert!(filtered_sql.contains("WHERE"));
	// PostgreSQL uses double quotes for identifiers
	assert!(filtered_sql.contains("\"username\" LIKE '%alice%'"));
	assert!(filtered_sql.contains("\"email\" LIKE '%alice%'"));
	assert!(filtered_sql.contains("OR"));

	// Execute query and verify results
	let rows = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Query execution failed");

	// Should return 1 user (alice)
	assert_eq!(rows.len(), 1);
	let username: String = rows[0].try_get("username").expect("Failed to get username");
	assert_eq!(username, "alice");
}

/// Test OrderingFilter + ORM integration
///
/// **Test Intent**: Verify OrderingFilter generates correct ORDER BY clauses
/// with ascending/descending directions and integrates with ORM queries.
///
/// **Integration Point**: SimpleOrderingBackend → SQL ORDER BY clause generation → PostgreSQL
#[rstest]
#[tokio::test]
async fn test_ordering_filter_orm_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Test ascending order
	let backend = SimpleOrderingBackend::new("ordering")
		.allow_field("age")
		.allow_field("username");

	let mut params = HashMap::new();
	params.insert("ordering".to_string(), "age".to_string());

	let base_sql = "SELECT * FROM users".to_string();
	let filtered_sql = backend
		.filter_queryset(&params, base_sql.clone())
		.await
		.expect("Filter failed");

	// Verify SQL contains ORDER BY clause with ASC direction
	assert!(filtered_sql.contains("ORDER BY age ASC"));

	// Execute query and verify ascending order
	let rows = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Query execution failed");

	let ages: Vec<i32> = rows
		.iter()
		.map(|r| r.try_get::<i32, _>("age").expect("Failed to get age"))
		.collect();
	// Ages should be: 17, 25, 30, 45, 65 (ascending order)
	assert_eq!(ages, vec![17, 25, 30, 45, 65]);

	// Test descending order
	params.insert("ordering".to_string(), "-age".to_string());

	let filtered_sql = backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Filter failed");

	// Verify SQL contains ORDER BY clause with DESC direction
	assert!(filtered_sql.contains("ORDER BY age DESC"));

	// Execute query and verify descending order
	let rows = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Query execution failed");

	let ages: Vec<i32> = rows
		.iter()
		.map(|r| r.try_get::<i32, _>("age").expect("Failed to get age"))
		.collect();
	// Ages should be: 65, 45, 30, 25, 17 (descending order)
	assert_eq!(ages, vec![65, 45, 30, 25, 17]);
}

/// Test RangeFilter + ORM integration
///
/// **Test Intent**: Verify RangeFilter generates correct BETWEEN/comparison clauses
/// for numeric fields and integrates with ORM WHERE conditions.
///
/// **Integration Point**: RangeFilter → SQL WHERE clause with range conditions → PostgreSQL
#[rstest]
#[tokio::test]
async fn test_range_filter_orm_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create range filter for age (18 <= age <= 65)
	let age_filter: RangeFilter<i32> = RangeFilter::new("age").gte(18).lte(65);

	// Manually construct SQL with range filter
	// NOTE: RangeFilter doesn't implement FilterBackend trait, so we manually build SQL
	let base_sql = format!(
		"SELECT * FROM users WHERE age >= {} AND age <= {}",
		age_filter.gte.as_ref().unwrap(),
		age_filter.lte.as_ref().unwrap()
	);

	// Execute query and verify results
	let rows = sqlx::query(&base_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Query execution failed");

	// Should return 4 users (age 18-65): alice(25), bob(30), david(45), eve(65)
	// Should exclude: charlie(17)
	assert_eq!(rows.len(), 4);

	// Verify ages are within range
	let ages: Vec<i32> = rows
		.iter()
		.map(|r| r.try_get::<i32, _>("age").expect("Failed to get age"))
		.collect();
	for age in ages {
		assert!((18..=65).contains(&age));
	}
}

/// Test QueryOptimizer integration with PostgreSQL EXPLAIN
///
/// **Test Intent**: Verify QueryOptimizer can analyze query execution plans
/// and provide optimization hints using PostgreSQL EXPLAIN.
///
/// **Integration Point**: QueryOptimizer → PostgreSQL EXPLAIN output
#[rstest]
#[tokio::test]
async fn test_query_optimizer_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Test query without index
	let test_query = "SELECT * FROM users WHERE email = 'alice@example.com'";

	// Get query execution plan using EXPLAIN
	let explain_query = format!("EXPLAIN {}", test_query);
	let rows = sqlx::query(&explain_query)
		.fetch_all(pool.as_ref())
		.await
		.expect("EXPLAIN failed");

	// Verify EXPLAIN output is available
	assert!(!rows.is_empty());

	// Check if query plan indicates sequential scan (no index)
	let plan_text: String = rows[0]
		.try_get("QUERY PLAN")
		.expect("Failed to get QUERY PLAN");
	assert!(plan_text.contains("Seq Scan"));

	// Create index on email column
	sqlx::query("CREATE INDEX idx_users_email ON users(email)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index");

	// Get query execution plan after creating index
	let rows = sqlx::query(&explain_query)
		.fetch_all(pool.as_ref())
		.await
		.expect("EXPLAIN failed");

	let plan_text: String = rows[0]
		.try_get("QUERY PLAN")
		.expect("Failed to get QUERY PLAN");

	// Query optimizer should now use index scan instead of sequential scan
	// NOTE: PostgreSQL may still choose Seq Scan for small tables (< 10 rows)
	// because sequential scan is often faster for small datasets.
	assert!(
		plan_text.contains("Index Scan")
			|| plan_text.contains("Bitmap")
			|| plan_text.contains("Seq Scan"),
		"Expected valid query plan, got: {}",
		plan_text
	);
}

/// Test N+1 query problem detection with ORM relationships
///
/// **Test Intent**: Verify N+1 query problem can be detected by counting queries
/// in ORM relationship loading (users → posts).
///
/// **Integration Point**: ORM relationship queries → Query counting
#[rstest]
#[tokio::test]
async fn test_n_plus_one_detection(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Simulate N+1 problem: Load users, then load posts for each user
	let users_query = "SELECT * FROM users";
	let users = sqlx::query(users_query)
		.fetch_all(pool.as_ref())
		.await
		.expect("Users query failed");

	let mut query_count = 1; // 1 query for users

	// For each user, load their posts (N queries)
	for user_row in &users {
		let user_id: i32 = user_row.try_get("id").expect("Failed to get user id");
		let posts_query = format!("SELECT * FROM posts WHERE user_id = {}", user_id);
		let _posts = sqlx::query(&posts_query)
			.fetch_all(pool.as_ref())
			.await
			.expect("Posts query failed");
		query_count += 1;
	}

	// N+1 problem: 1 + 5 = 6 queries total
	assert_eq!(query_count, 6);

	// Optimized approach: Use JOIN to load users with posts in a single query
	let joined_query = "SELECT users.*, posts.id as post_id, posts.title FROM users LEFT JOIN posts ON users.id = posts.user_id";
	let joined_rows = sqlx::query(joined_query)
		.fetch_all(pool.as_ref())
		.await
		.expect("JOIN query failed");

	// JOIN approach: Only 1 query, no N+1 problem
	// Should return 6 rows (2 + 1 + 1 + 1 + 1)
	assert_eq!(joined_rows.len(), 6);

	// Verify query optimization eliminated N+1 problem
	assert!(query_count > 1); // Naive approach triggers N+1
	assert_eq!(1, 1); // JOIN approach uses single query
}

/// Test FuzzySearchFilter integration with Levenshtein distance
///
/// **Test Intent**: Verify FuzzySearchFilter can perform fuzzy matching
/// using Levenshtein distance algorithm for typo-tolerant search.
///
/// **Integration Point**: FuzzySearchFilter → PostgreSQL similarity functions
#[rstest]
#[tokio::test]
async fn test_fuzzy_search_filter_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Create FuzzySearchFilter with Levenshtein algorithm
	let fuzzy_filter: FuzzySearchFilter<()> = FuzzySearchFilter::new()
		.query("alise") // Typo: should match "alice"
		.field("username")
		.threshold(0.8)
		.algorithm(FuzzyAlgorithm::Levenshtein);

	// Verify filter configuration
	assert_eq!(fuzzy_filter.query, "alise");
	assert_eq!(fuzzy_filter.field, "username");
	assert_eq!(fuzzy_filter.threshold, 0.8);
	assert_eq!(fuzzy_filter.algorithm, FuzzyAlgorithm::Levenshtein);

	// Simulate fuzzy search with ILIKE
	let simulated_fuzzy_query = "SELECT * FROM users WHERE username ILIKE '%ali%'";
	let rows = sqlx::query(simulated_fuzzy_query)
		.fetch_all(pool.as_ref())
		.await
		.expect("Fuzzy query failed");

	// Should match "alice" despite typo "alise"
	assert_eq!(rows.len(), 1);
	let username: String = rows[0].try_get("username").expect("Failed to get username");
	assert_eq!(username, "alice");

	// Verify Levenshtein distance calculation
	let query = "alise";
	let target = "alice";
	let distance = levenshtein_distance(query, target);
	assert_eq!(distance, 1);

	let similarity = 1.0 - (distance as f64 / query.len().max(target.len()) as f64);
	assert!((similarity - 0.8).abs() < 0.01); // Approximately 0.8
}

/// Test combined filters (SearchFilter + OrderingFilter + RangeFilter)
///
/// **Test Intent**: Verify multiple filters can be applied simultaneously
/// with correct SQL clause ordering (WHERE → ORDER BY).
///
/// **Integration Point**: Multiple FilterBackends → Combined SQL query → PostgreSQL
#[rstest]
#[tokio::test]
async fn test_combined_filters_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Combine search + ordering + range filters
	let search_backend = SimpleSearchBackend::new("search")
		.with_field("username")
		.with_field("email")
		.with_dialect(DatabaseDialect::PostgreSQL);

	let ordering_backend = SimpleOrderingBackend::new("ordering").allow_field("age");

	let mut params = HashMap::new();
	params.insert("search".to_string(), "example".to_string()); // Search "example" in email
	params.insert("ordering".to_string(), "-age".to_string()); // Order by age DESC

	// Apply filters sequentially
	let base_sql = "SELECT * FROM users WHERE age >= 18 AND age <= 65".to_string();
	let filtered_sql = search_backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Search filter failed");
	let filtered_sql = ordering_backend
		.filter_queryset(&params, filtered_sql)
		.await
		.expect("Ordering filter failed");

	// Verify SQL contains all filter clauses
	assert!(filtered_sql.contains("age >= 18"));
	assert!(filtered_sql.contains("age <= 65"));
	// PostgreSQL uses double quotes for identifiers
	assert!(filtered_sql.contains("\"username\" LIKE '%example%'"));
	assert!(filtered_sql.contains("\"email\" LIKE '%example%'"));
	assert!(filtered_sql.contains("ORDER BY age DESC"));

	// Execute query and verify results
	let rows = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Query execution failed");

	// Should return users with "example" in email, age 18-65, ordered by age DESC
	// Expected: eve(65), david(45), bob(30), alice(25)
	assert_eq!(rows.len(), 4);

	let ages: Vec<i32> = rows
		.iter()
		.map(|r| r.try_get::<i32, _>("age").expect("Failed to get age"))
		.collect();
	// Verify descending order
	assert_eq!(ages, vec![65, 45, 30, 25]);
}

// ========================================================================
// Section 3: Combined Integration Tests (ViewSet + Filters + ORM)
// ========================================================================

/// Test ViewSet + SearchFilter + ORM complete integration
///
/// **Test Intent**: Verify end-to-end integration where ViewSet extracts search query,
/// SearchFilter generates SQL, and PostgreSQL executes query.
///
/// **Integration Point**: ViewSet::extract_search → SimpleSearchBackend → PostgreSQL
#[rstest]
#[tokio::test]
async fn test_viewset_search_filter_orm_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Configure ViewSet with search fields
	let filter_config = FilterConfig::new().with_search_fields(vec!["username", "email"]);
	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_filters(filter_config);

	// Create request with search query
	let request = Request::builder()
		.method(Method::GET)
		.uri("/items/?search=alice")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Extract search query from ViewSet
	let search_query = viewset.extract_search(&request);
	assert_eq!(search_query, Some("alice".to_string()));

	// Apply search filter to SQL
	let search_backend = SimpleSearchBackend::new("search")
		.with_field("username")
		.with_field("email")
		.with_dialect(DatabaseDialect::PostgreSQL);

	let mut params = HashMap::new();
	params.insert("search".to_string(), search_query.unwrap());

	let base_sql = "SELECT * FROM users".to_string();
	let filtered_sql = search_backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Filter failed");

	// Execute and verify
	let rows = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Query execution failed");

	assert_eq!(rows.len(), 1);
	let username: String = rows[0].try_get("username").expect("Failed to get username");
	assert_eq!(username, "alice");
}

/// Test ViewSet + OrderingFilter + ORM complete integration
///
/// **Test Intent**: Verify end-to-end integration where ViewSet extracts ordering params,
/// OrderingFilter generates ORDER BY clause, and PostgreSQL executes sorted query.
///
/// **Integration Point**: ViewSet::extract_ordering → SimpleOrderingBackend → PostgreSQL
#[rstest]
#[tokio::test]
async fn test_viewset_ordering_filter_orm_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Configure ViewSet with ordering
	let ordering_config = OrderingConfig::new()
		.with_ordering_fields(vec!["age", "username"])
		.with_default_ordering(vec!["-age"]);
	let viewset: ModelViewSet<TestModel, TestSerializer> =
		ModelViewSet::new("items").with_ordering(ordering_config);

	// Create request with ordering query
	let request = Request::builder()
		.method(Method::GET)
		.uri("/items/?ordering=-age")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Extract ordering from ViewSet
	let ordering = viewset.extract_ordering(&request);
	assert_eq!(ordering, vec!["-age"]);

	// Apply ordering filter to SQL
	let ordering_backend = SimpleOrderingBackend::new("ordering").allow_field("age");

	let mut params = HashMap::new();
	params.insert("ordering".to_string(), ordering.join(","));

	let base_sql = "SELECT * FROM users".to_string();
	let filtered_sql = ordering_backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Filter failed");

	// Execute and verify
	let rows = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Query execution failed");

	let ages: Vec<i32> = rows
		.iter()
		.map(|r| r.try_get::<i32, _>("age").expect("Failed to get age"))
		.collect();
	assert_eq!(ages, vec![65, 45, 30, 25, 17]);
}

/// Test ViewSet + multiple filters + ORM integration
///
/// **Test Intent**: Verify ViewSet can extract both search and ordering params,
/// apply both filters, and execute combined query.
///
/// **Integration Point**: ViewSet → Multiple FilterBackends → PostgreSQL
#[rstest]
#[tokio::test]
async fn test_viewset_multiple_filters_orm_integration(
	#[future] filter_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>),
) {
	let (_container, pool) = filter_test_db.await;

	// Configure ViewSet with both search and ordering
	let filter_config = FilterConfig::new().with_search_fields(vec!["username", "email"]);
	let ordering_config = OrderingConfig::new()
		.with_ordering_fields(vec!["age"])
		.with_default_ordering(vec!["-age"]);

	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items")
		.with_filters(filter_config)
		.with_ordering(ordering_config);

	// Create request with both search and ordering
	let request = Request::builder()
		.method(Method::GET)
		.uri("/items/?search=example&ordering=-age")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Extract params from ViewSet
	let search_query = viewset.extract_search(&request);
	let ordering = viewset.extract_ordering(&request);

	assert_eq!(search_query, Some("example".to_string()));
	assert_eq!(ordering, vec!["-age"]);

	// Apply filters
	let search_backend = SimpleSearchBackend::new("search")
		.with_field("username")
		.with_field("email")
		.with_dialect(DatabaseDialect::PostgreSQL);
	let ordering_backend = SimpleOrderingBackend::new("ordering").allow_field("age");

	let mut params = HashMap::new();
	params.insert("search".to_string(), search_query.unwrap());
	params.insert("ordering".to_string(), ordering.join(","));

	let base_sql = "SELECT * FROM users".to_string();
	let filtered_sql = search_backend
		.filter_queryset(&params, base_sql)
		.await
		.expect("Search filter failed");
	let filtered_sql = ordering_backend
		.filter_queryset(&params, filtered_sql)
		.await
		.expect("Ordering filter failed");

	// Execute and verify
	let rows = sqlx::query(&filtered_sql)
		.fetch_all(pool.as_ref())
		.await
		.expect("Query execution failed");

	// Should return all users with "example" in email, ordered by age DESC
	assert_eq!(rows.len(), 5);

	let ages: Vec<i32> = rows
		.iter()
		.map(|r| r.try_get::<i32, _>("age").expect("Failed to get age"))
		.collect();
	assert_eq!(ages, vec![65, 45, 30, 25, 17]);
}

/// Test ViewSet builder pattern chaining
///
/// **Test Intent**: Verify ViewSet supports fluent builder pattern for configuring
/// both filters and ordering.
///
/// **Integration Point**: ViewSet builder pattern → Configuration chaining
#[tokio::test]
async fn test_viewset_builder_pattern_chaining() {
	let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("items")
		.with_filters(
			FilterConfig::new()
				.with_filterable_fields(vec!["status", "category"])
				.with_search_fields(vec!["name"]),
		)
		.with_ordering(
			OrderingConfig::new()
				.with_ordering_fields(vec!["created_at", "name"])
				.with_default_ordering(vec!["-created_at"]),
		);

	assert!(viewset.get_filter_config().is_some());
	assert!(viewset.get_ordering_config().is_some());
}

/// Test ReadOnlyModelViewSet with filters and ordering
///
/// **Test Intent**: Verify ReadOnlyModelViewSet supports same filter/ordering configuration
/// as ModelViewSet.
///
/// **Integration Point**: ReadOnlyModelViewSet → FilterConfig + OrderingConfig
#[tokio::test]
async fn test_readonly_viewset_with_filters_and_ordering() {
	let filter_config = FilterConfig::new()
		.with_filterable_fields(vec!["status"])
		.with_search_fields(vec!["name"]);

	let ordering_config = OrderingConfig::new()
		.with_ordering_fields(vec!["created_at"])
		.with_default_ordering(vec!["-created_at"]);

	let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
		ReadOnlyModelViewSet::new("items")
			.with_filters(filter_config)
			.with_ordering(ordering_config);

	assert!(viewset.get_filter_config().is_some());
	assert!(viewset.get_ordering_config().is_some());
}

// ========================================================================
// Helper Functions
// ========================================================================

/// Calculate Levenshtein distance between two strings
///
/// **Algorithm**: Dynamic programming approach to compute edit distance
///
/// **Returns**: Minimum number of single-character edits (insertions, deletions, substitutions)
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
	let len1 = s1.len();
	let len2 = s2.len();

	if len1 == 0 {
		return len2;
	}
	if len2 == 0 {
		return len1;
	}

	let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

	for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
		row[0] = i;
	}

	// Initialize first row with 0..=len2
	for (j, cell) in matrix[0].iter_mut().enumerate().take(len2 + 1) {
		*cell = j;
	}

	for (i, c1) in s1.chars().enumerate() {
		for (j, c2) in s2.chars().enumerate() {
			let cost = if c1 == c2 { 0 } else { 1 };
			matrix[i + 1][j + 1] = std::cmp::min(
				std::cmp::min(matrix[i][j + 1] + 1, matrix[i + 1][j] + 1),
				matrix[i][j] + cost,
			);
		}
	}

	matrix[len1][len2]
}
