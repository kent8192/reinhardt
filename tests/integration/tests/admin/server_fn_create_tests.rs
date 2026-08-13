//! Integration tests for create_record server function
//!
//! Tests the create operation server function.
//! Covers regression for Issue #2946 (create() hardcodes "id" in RETURNING clause).

use super::server_fn_helpers::server_fn_context;
use reinhardt_admin::adapters::MutationRequest;
use reinhardt_admin::core::{AdminDatabase, AdminRecord, AdminSite, AdminUser, ModelAdmin};
use reinhardt_admin::server::create_record;
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::migrations::FieldType;
use reinhardt_db::migrations::model_registry::{
	FieldMetadata, ManyToManyMetadata, ModelMetadata, global_registry,
};
use reinhardt_db::orm::DatabaseConnectionLease;
use reinhardt_di::KeyedDepends;
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use rstest::*;
use serde_json::json;
use serial_test::serial;
use sqlx::Executor;
use std::collections::HashMap;
use std::sync::Arc;

use super::server_fn_helpers::{TEST_CSRF_TOKEN, make_auth_user, make_staff_request};

pub(super) struct ManyToManyContext {
	pub(super) site: super::server_fn_helpers::AdminSiteDepends,
	pub(super) db: super::server_fn_helpers::AdminDatabaseDepends,
	pub(super) pool: sqlx::PgPool,
	pub(super) _lease: DatabaseConnectionLease,
}

impl Drop for ManyToManyContext {
	fn drop(&mut self) {
		global_registry().remove_model("admin_persistence", "PersistenceArticle");
		global_registry().remove_model("admin_persistence", "PersistenceTag");
	}
}

struct PersistenceAdmin {
	model_name: &'static str,
	table_name: &'static str,
	fields: Vec<&'static str>,
	search_fields: Vec<&'static str>,
	filter_horizontal: Vec<&'static str>,
	allow_view: bool,
}

impl PersistenceAdmin {
	fn source() -> Self {
		Self {
			model_name: "PersistenceArticle",
			table_name: "admin_persistence_articles",
			fields: vec!["title", "tags"],
			search_fields: vec!["title"],
			filter_horizontal: vec!["tags"],
			allow_view: true,
		}
	}

	fn target(allow_view: bool) -> Self {
		Self {
			model_name: "PersistenceTag",
			table_name: "admin_persistence_tags",
			fields: vec!["id", "name"],
			search_fields: vec!["name"],
			filter_horizontal: Vec::new(),
			allow_view,
		}
	}
}

#[async_trait::async_trait]
impl ModelAdmin for PersistenceAdmin {
	fn model_name(&self) -> &str {
		self.model_name
	}

	fn table_name(&self) -> &str {
		self.table_name
	}

	fn list_display(&self) -> Vec<&str> {
		self.fields.clone()
	}

	fn fields(&self) -> Option<Vec<&str>> {
		Some(self.fields.clone())
	}

	fn search_fields(&self) -> Vec<&str> {
		self.search_fields.clone()
	}

	fn filter_horizontal(&self) -> Vec<&str> {
		self.filter_horizontal.clone()
	}

	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		self.allow_view
	}

	async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}
}

pub(super) async fn setup_many_to_many_context(
	pool: sqlx::PgPool,
	target_view_allowed: bool,
) -> ManyToManyContext {
	pool.execute("DROP TABLE IF EXISTS admin_persistence_articles_tags")
		.await
		.unwrap();
	pool.execute("DROP TABLE IF EXISTS admin_persistence_articles")
		.await
		.unwrap();
	pool.execute("DROP TABLE IF EXISTS admin_persistence_tags")
		.await
		.unwrap();
	pool.execute(
		"CREATE TABLE admin_persistence_articles (id SERIAL PRIMARY KEY, title VARCHAR(200) NOT NULL)",
	)
	.await
	.unwrap();
	pool.execute(
		"CREATE TABLE admin_persistence_tags (id INTEGER PRIMARY KEY, name VARCHAR(100) NOT NULL)",
	)
	.await
	.unwrap();
	pool.execute(
		"CREATE TABLE admin_persistence_articles_tags (
			marker BIGSERIAL PRIMARY KEY,
			admin_persistence_articles_id INTEGER NOT NULL,
			admin_persistence_tags_id INTEGER NOT NULL CHECK (admin_persistence_tags_id <> 99),
			UNIQUE (admin_persistence_articles_id, admin_persistence_tags_id)
		)",
	)
	.await
	.unwrap();
	pool.execute(
		"INSERT INTO admin_persistence_tags (id, name) VALUES
			(1, 'One'), (2, 'Two'), (3, 'Three'), (99, 'Rejected join')",
	)
	.await
	.unwrap();

	let mut source = ModelMetadata::new(
		"admin_persistence",
		"PersistenceArticle",
		"admin_persistence_articles",
	);
	source.add_field("id".to_string(), FieldMetadata::new(FieldType::Integer));
	source.add_field(
		"title".to_string(),
		FieldMetadata::new(FieldType::VarChar(200)),
	);
	source.add_many_to_many(ManyToManyMetadata::new(
		"tags",
		"admin_persistence.PersistenceTag",
	));
	global_registry().register_model(source);
	let mut target = ModelMetadata::new(
		"admin_persistence",
		"PersistenceTag",
		"admin_persistence_tags",
	);
	target.add_field("id".to_string(), FieldMetadata::new(FieldType::Integer));
	target.add_field(
		"name".to_string(),
		FieldMetadata::new(FieldType::VarChar(100)),
	);
	global_registry().register_model(target);

	let backend = Arc::new(PostgresBackend::new(pool.clone()));
	let lease = DatabaseConnectionLease::register(BackendsConnection::new(backend)).unwrap();
	let db = AdminDatabase::new(lease.handle());
	let site = AdminSite::new("Atomic persistence test");
	site.register("PersistenceArticle", PersistenceAdmin::source())
		.unwrap();
	site.register(
		"PersistenceTag",
		PersistenceAdmin::target(target_view_allowed),
	)
	.unwrap();
	ManyToManyContext {
		site: KeyedDepends::from_value(site),
		db: KeyedDepends::from_value(db),
		pool,
		_lease: lease,
	}
}

async fn persistence_counts(pool: &sqlx::PgPool) -> (i64, i64) {
	let parents = sqlx::query_scalar("SELECT COUNT(*) FROM admin_persistence_articles")
		.fetch_one(pool)
		.await
		.unwrap();
	let joins = sqlx::query_scalar("SELECT COUNT(*) FROM admin_persistence_articles_tags")
		.fetch_one(pool)
		.await
		.unwrap();
	(parents, joins)
}

// ==================== Happy path tests ====================

/// Verify that create_record succeeds with valid data
#[rstest]
#[tokio::test]
async fn test_create_record_happy_path(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Created Item"));
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "create_record should succeed: {:?}", result);
	let response = result.unwrap();
	assert!(response.success);
	assert!(response.affected.is_some());
}

/// Verify create_record returns valid response metadata
#[rstest]
#[tokio::test]
async fn test_create_record_returns_valid_response(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Response Metadata Test"));
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("create_record should succeed");
	assert!(response.success);
	assert!(
		response.message.contains("TestModel"),
		"Message should contain model name: {}",
		response.message
	);
	assert!(
		response.message.contains("created"),
		"Message should indicate creation: {}",
		response.message
	);
}

/// Verify created record persists to database and can be retrieved
#[rstest]
#[tokio::test]
async fn test_create_record_persists_to_database(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Persistent Record"));
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let create_result = create_record(
		"TestModel".to_string(),
		request,
		site.clone(),
		db.clone(),
		http_request,
		auth_user,
	)
	.await;
	let create_response = create_result.expect("Create should succeed");

	// Verify by reading directly from DB
	let created_id = create_response
		.affected
		.expect("Should return affected count");
	let record = db
		.get::<AdminRecord>("test_models", "id", &created_id.to_string())
		.await;

	// Assert
	assert!(
		record.is_ok(),
		"Should be able to read created record from DB"
	);
}

/// Verify create_record works with multiple fields
#[rstest]
#[tokio::test]
async fn test_create_record_multiple_fields(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Multi-Field Record"));
	data.insert("status".to_string(), json!("draft"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"Should handle multiple fields: {:?}",
		result
	);
}

/// Verify create_record handles special characters and Unicode
#[rstest]
#[tokio::test]
async fn test_create_record_special_characters(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert(
		"name".to_string(),
		json!("Special: <>&\"' \u{00e9}\u{00f1}\u{00fc} \u{65e5}\u{672c}\u{8a9e}"),
	);
	data.insert("status".to_string(), json!("active"));

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	};

	// Act
	let result = create_record(
		"TestModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"Should handle special characters: {:?}",
		result
	);
}

// ==================== Error path tests ====================

/// Verify that create_record returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_create_record_model_not_registered(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::new(),
	};

	// Act
	let result = create_record(
		"NonExistentModel".to_string(),
		request,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should return error for unregistered model"
	);
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_create_commits_parent_and_deduplicated_joins(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, true).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Atomic create")),
			("tags".to_string(), json!([1, " 1 ", 2, "2"])),
		]),
	};

	// Act
	let response = create_record(
		"PersistenceArticle".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.unwrap();

	// Assert
	let article_id = response.affected.unwrap() as i32;
	let title: String =
		sqlx::query_scalar("SELECT title FROM admin_persistence_articles WHERE id = $1")
			.bind(article_id)
			.fetch_one(&context.pool)
			.await
			.unwrap();
	let tag_ids: Vec<i32> = sqlx::query_scalar(
		"SELECT admin_persistence_tags_id FROM admin_persistence_articles_tags \
		 WHERE admin_persistence_articles_id = $1 ORDER BY admin_persistence_tags_id",
	)
	.bind(article_id)
	.fetch_all(&context.pool)
	.await
	.unwrap();
	assert_eq!(title, "Atomic create");
	assert_eq!(tag_ids, vec![1, 2]);
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_create_missing_target_rolls_back_parent_and_joins(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, true).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Must roll back")),
			("tags".to_string(), json!([1, 404])),
		]),
	};

	// Act
	let result = create_record(
		"PersistenceArticle".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	assert_eq!(persistence_counts(&context.pool).await, (0, 0));
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_create_denied_target_view_makes_no_sql_mutation(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, false).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Denied")),
			("tags".to_string(), json!([1])),
		]),
	};

	// Act
	let result = create_record(
		"PersistenceArticle".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	assert_eq!(persistence_counts(&context.pool).await, (0, 0));
}

#[rstest]
#[tokio::test]
#[serial(admin_m2m_persistence)]
async fn many_to_many_create_join_error_rolls_back_parent(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (pool, _) = shared_db_pool.await;
	let context = setup_many_to_many_context(pool, true).await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: HashMap::from([
			("title".to_string(), json!("Rejected join")),
			("tags".to_string(), json!([99])),
		]),
	};

	// Act
	let result = create_record(
		"PersistenceArticle".to_string(),
		request,
		context.site.clone(),
		context.db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	assert!(result.is_err());
	assert_eq!(persistence_counts(&context.pool).await, (0, 0));
}
