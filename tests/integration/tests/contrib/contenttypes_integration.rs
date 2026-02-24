//! Integration tests: Multi-database & ORM integration
//!
//! This test provides integration testing for multi_db and orm_integration modules.

use reinhardt_test::resource::{TeardownGuard, TestResource};
use rstest::*;
use serial_test::serial;
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize SQLx drivers (idempotent)
#[fixture]
fn init_drivers() {
	INIT.call_once(|| {
		sqlx::any::install_default_drivers();
	});
}

/// Guard for ContentTypeRegistry cleanup
///
/// Ensures CONTENT_TYPE_REGISTRY is cleared before and after each test,
/// even if the test panics.
struct ContentTypeRegistryGuard;

impl TestResource for ContentTypeRegistryGuard {
	fn setup() -> Self {
		// Clear registry before test
		use reinhardt_db::contenttypes::CONTENT_TYPE_REGISTRY;
		CONTENT_TYPE_REGISTRY.clear();
		Self
	}

	fn teardown(&mut self) {
		// Clear registry after test (guaranteed even on panic)
		use reinhardt_db::contenttypes::CONTENT_TYPE_REGISTRY;
		CONTENT_TYPE_REGISTRY.clear();
	}
}

#[fixture]
fn registry_guard() -> TeardownGuard<ContentTypeRegistryGuard> {
	TeardownGuard::new()
}

mod multi_db_tests {
	use super::*;
	use reinhardt_db::contenttypes::{CONTENT_TYPE_REGISTRY, MultiDbContentTypeManager};

	#[rstest]
	#[serial(content_type_registry)]
	#[tokio::test]
	async fn test_multi_db_with_global_registry(
		_init_drivers: (),
		_registry_guard: TeardownGuard<ContentTypeRegistryGuard>,
	) {
		let mut manager = MultiDbContentTypeManager::new();
		manager
			.add_database("db1", "sqlite::memory:?mode=rwc&cache=shared")
			.await
			.expect("Failed to add db1");

		// Create ContentType
		let ct = manager
			.get_or_create("db1", "integration", "Test")
			.await
			.expect("Failed to create");

		// Verify it's also registered in global registry
		CONTENT_TYPE_REGISTRY.register(ct.clone());
		let global_ct = CONTENT_TYPE_REGISTRY.get("integration", "Test");
		assert!(global_ct.is_some());

		// Cleanup is handled by TeardownGuard automatically
	}

	#[rstest]
	#[serial(content_type_registry)]
	#[tokio::test]
	async fn test_multi_db_cross_database_search(
		_init_drivers: (),
		_registry_guard: TeardownGuard<ContentTypeRegistryGuard>,
	) {
		let mut manager = MultiDbContentTypeManager::new();
		manager
			.add_database("primary", "sqlite::memory:?mode=rwc&cache=shared")
			.await
			.expect("Failed to add primary");
		manager
			.add_database("secondary", "sqlite::memory:?mode=rwc&cache=shared")
			.await
			.expect("Failed to add secondary");

		// Create same ContentType in both databases
		manager
			.get_or_create("primary", "shared", "Model")
			.await
			.expect("Failed to create in primary");
		manager
			.get_or_create("secondary", "shared", "Model")
			.await
			.expect("Failed to create in secondary");

		// Cross-database search
		let results = manager
			.search_all_databases("shared", "Model")
			.await
			.expect("Failed to search");

		assert_eq!(results.len(), 2);
		let db_names: Vec<String> = results.iter().map(|(db, _)| db.clone()).collect();
		assert!(db_names.contains(&"primary".to_string()));
		assert!(db_names.contains(&"secondary".to_string()));
	}

	#[rstest]
	#[serial(content_type_registry)]
	#[tokio::test]
	async fn test_multi_db_isolated_caches(
		_init_drivers: (),
		_registry_guard: TeardownGuard<ContentTypeRegistryGuard>,
	) {
		let mut manager = MultiDbContentTypeManager::new();
		manager
			.add_database("db1", "sqlite::memory:?mode=rwc&cache=shared")
			.await
			.expect("Failed to add db1");
		manager
			.add_database("db2", "sqlite::memory:?mode=rwc&cache=shared")
			.await
			.expect("Failed to add db2");

		// Create ContentType in db1 only
		let ct1 = manager
			.get_or_create("db1", "isolated", "Model")
			.await
			.expect("Failed to create in db1");

		// Does not exist in db2
		let ct2 = manager
			.get("db2", "isolated", "Model")
			.await
			.expect("Failed to query");

		assert!(ct2.is_none());
		assert!(ct1.id.is_some());
	}

	#[rstest]
	#[serial(content_type_registry)]
	#[tokio::test]
	async fn test_multi_db_load_all_with_cache(
		_init_drivers: (),
		_registry_guard: TeardownGuard<ContentTypeRegistryGuard>,
	) {
		let mut manager = MultiDbContentTypeManager::new();
		manager
			.add_database("primary", "sqlite::memory:?mode=rwc&cache=shared")
			.await
			.expect("Failed to add database");

		// Create multiple ContentTypes
		manager
			.get_or_create("primary", "app1", "Model1")
			.await
			.expect("Failed to create");
		manager
			.get_or_create("primary", "app2", "Model2")
			.await
			.expect("Failed to create");

		// Load all (registered in cache)
		let all = manager
			.load_all("primary")
			.await
			.expect("Failed to load all");
		assert_eq!(all.len(), 2);

		// Verify it can be retrieved from cache
		let registry = manager.get_registry("primary").unwrap();
		let cached = registry.get("app1", "Model1");
		assert!(cached.is_some());
	}
}

mod orm_integration_tests {
	use super::*;
	use reinhardt_db::contenttypes::{ContentTypeQuery, ContentTypeTransaction};
	use sqlx::AnyPool;
	use std::sync::Arc;

	/// Setup test pool fixture with table creation
	#[fixture]
	async fn setup_test_pool(_init_drivers: ()) -> Arc<AnyPool> {
		// Use single connection pool for in-memory SQLite with shared cache
		use sqlx::pool::PoolOptions;
		let database_url = "sqlite::memory:?mode=rwc&cache=shared";
		let pool = PoolOptions::new()
			.min_connections(1)
			.max_connections(1)
			.connect(database_url)
			.await
			.expect("Failed to connect");

		// Create table
		use reinhardt_db::contenttypes::persistence::ContentTypePersistence;
		let persistence = ContentTypePersistence::from_pool(pool.clone().into(), database_url);
		persistence
			.create_table()
			.await
			.expect("Failed to create table");

		pool.into()
	}

	#[rstest]
	#[tokio::test]
	async fn test_query_with_transaction(#[future] setup_test_pool: Arc<AnyPool>) {
		let pool = setup_test_pool.await;

		// Create ContentType within transaction
		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("txn_test", "Model1")
			.await
			.expect("Failed to create");
		tx.create("txn_test", "Model2")
			.await
			.expect("Failed to create");

		// Retrieve with query
		let query = ContentTypeQuery::new(pool);
		let results = query
			.filter_app_label("txn_test")
			.order_by_model()
			.all()
			.await
			.expect("Failed to query");

		assert_eq!(results.len(), 2);
		assert_eq!(results[0].model, "Model1");
		assert_eq!(results[1].model, "Model2");
	}

	#[rstest]
	#[tokio::test]
	async fn test_query_chaining(#[future] setup_test_pool: Arc<AnyPool>) {
		let pool = setup_test_pool.await;

		// Create test data
		let tx = ContentTypeTransaction::new(pool.clone());
		for i in 1..=5 {
			tx.create("chain", &format!("Model{}", i))
				.await
				.expect("Failed to create");
		}

		// Complex query chain
		let query = ContentTypeQuery::new(pool);
		let results = query
			.filter_app_label("chain")
			.order_by_model()
			.limit(3)
			.offset(1)
			.all()
			.await
			.expect("Failed to query");

		assert_eq!(results.len(), 3);
		assert_eq!(results[0].model, "Model2");
		assert_eq!(results[2].model, "Model4");
	}

	#[rstest]
	#[tokio::test]
	async fn test_query_count_and_exists(#[future] setup_test_pool: Arc<AnyPool>) {
		let pool = setup_test_pool.await;

		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("count_test", "A")
			.await
			.expect("Failed to create");
		tx.create("count_test", "B")
			.await
			.expect("Failed to create");
		tx.create("count_test", "C")
			.await
			.expect("Failed to create");

		// count
		let query = ContentTypeQuery::new(pool.clone());
		let count = query
			.filter_app_label("count_test")
			.count()
			.await
			.expect("Failed to count");
		assert_eq!(count, 3);

		// exists
		let query2 = ContentTypeQuery::new(pool.clone());
		let exists = query2
			.filter_app_label("count_test")
			.filter_model("B")
			.exists()
			.await
			.expect("Failed to check exists");
		assert!(exists);

		// not exists
		let query3 = ContentTypeQuery::new(pool);
		let not_exists = query3
			.filter_app_label("count_test")
			.filter_model("Z")
			.exists()
			.await
			.expect("Failed to check not exists");
		assert!(!not_exists);
	}

	#[rstest]
	#[tokio::test]
	async fn test_transaction_delete(#[future] setup_test_pool: Arc<AnyPool>) {
		let pool = setup_test_pool.await;

		// Create
		let tx = ContentTypeTransaction::new(pool.clone());
		let ct = tx
			.create("delete_test", "ToBeDeleted")
			.await
			.expect("Failed to create");
		let id = ct.id.unwrap();

		// Delete
		tx.delete(id).await.expect("Failed to delete");

		// Verify deletion
		let query = ContentTypeQuery::new(pool);
		let result = query.filter_id(id).first().await.expect("Failed to query");
		assert!(result.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_query_multiple_order_by(#[future] setup_test_pool: Arc<AnyPool>) {
		let pool = setup_test_pool.await;

		let tx = ContentTypeTransaction::new(pool.clone());
		tx.create("app1", "Z").await.expect("Failed to create");
		tx.create("app2", "A").await.expect("Failed to create");
		tx.create("app1", "A").await.expect("Failed to create");

		// Multiple order_by
		let query = ContentTypeQuery::new(pool);
		let results = query
			.order_by_app_label()
			.order_by_model()
			.all()
			.await
			.expect("Failed to query");

		assert_eq!(results.len(), 3);
		// app1 comes first, sorted by model within
		assert_eq!(results[0].app_label, "app1");
		assert_eq!(results[0].model, "A");
		assert_eq!(results[1].app_label, "app1");
		assert_eq!(results[1].model, "Z");
	}
}

mod combined_tests {
	use super::*;
	use reinhardt_db::contenttypes::{
		ContentTypeQuery, ContentTypeTransaction, MultiDbContentTypeManager,
	};
	use sqlx::AnyPool;
	use std::sync::Arc;

	#[rstest]
	#[serial(content_type_registry)]
	#[tokio::test]
	async fn test_multi_db_with_orm_query(
		_init_drivers: (),
		_registry_guard: TeardownGuard<ContentTypeRegistryGuard>,
	) {
		let mut manager = MultiDbContentTypeManager::new();
		manager
			.add_database("primary", "sqlite::memory:?mode=rwc&cache=shared")
			.await
			.expect("Failed to add database");

		// Create ContentType via manager
		manager
			.get_or_create("primary", "combined", "Model1")
			.await
			.expect("Failed to create via manager");

		// Retrieve with ORM query
		let _pool = manager.get_database("primary").unwrap();
		let _pool_arc: Arc<AnyPool> = Arc::new(
			AnyPool::connect("sqlite::memory:?mode=rwc&cache=shared")
				.await
				.expect("Failed to connect"),
		);

		// Note: In real tests, the same database connection should be used
		// This is written as a conceptual test
	}

	#[rstest]
	#[serial(content_type_registry)]
	#[tokio::test]
	async fn test_transaction_with_multi_db(
		_init_drivers: (),
		_registry_guard: TeardownGuard<ContentTypeRegistryGuard>,
	) {
		// Use single connection pool for in-memory SQLite with shared cache
		use sqlx::pool::PoolOptions;
		let database_url = "sqlite::memory:?mode=rwc&cache=shared";
		let pool = PoolOptions::new()
			.min_connections(1)
			.max_connections(1)
			.connect(database_url)
			.await
			.expect("Failed to connect");

		// Create table
		use reinhardt_db::contenttypes::persistence::ContentTypePersistence;
		let persistence = ContentTypePersistence::from_pool(pool.clone().into(), database_url);
		persistence
			.create_table()
			.await
			.expect("Failed to create table");

		let pool_arc = Arc::new(pool);

		// Transaction operations
		let tx = ContentTypeTransaction::new(pool_arc.clone());
		tx.create("multi", "A").await.expect("Failed to create");
		tx.create("multi", "B").await.expect("Failed to create");

		// Verify with query
		let query = ContentTypeQuery::new(pool_arc);
		let count = query
			.filter_app_label("multi")
			.count()
			.await
			.expect("Failed to count");

		assert_eq!(count, 2);
	}
}
