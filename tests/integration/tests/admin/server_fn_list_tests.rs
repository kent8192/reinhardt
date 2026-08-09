//! Integration tests for get_list server function
//!
//! Tests the list view server function with search, filters, sorting, and pagination.
//! Covers regression for Issue #2922 (sort_by not validated against allowed fields).

use super::server_fn_helpers::{
	ADMIN_TO_FIELD_SOURCE_MODEL_NAME, AllPermissionsModelAdmin, server_fn_context,
};
use reinhardt_admin::adapters::{
	DateHierarchyLevel, DateHierarchySelection, ListColumn, ListQueryParams,
};
use reinhardt_admin::core::{
	AdminDatabase, AdminDatabaseKey, AdminRecord, AdminSite, AdminSiteKey,
};
use reinhardt_admin::server::get_list;
use reinhardt_db::backends::{
	connection::DatabaseConnection as BackendsConnection,
	dialect::PostgresBackend,
	types::{DatabaseType, QueryValue, Row},
};
use reinhardt_db::orm::connection::DatabaseConnectionLease;
use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
use reinhardt_di::KeyedDepends;
use reinhardt_test::fixtures::mock::MockDatabaseBackend;
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use rstest::*;
use serde_json::json;
use sqlx::Executor;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;

use super::server_fn_helpers::{make_auth_user, make_staff_request};

fn register_date_hierarchy_metadata(
	table_name: &str,
	field_type: reinhardt_db::migrations::FieldType,
) {
	use reinhardt_db::migrations::model_registry::{FieldMetadata, ModelMetadata, global_registry};

	let mut metadata = ModelMetadata::new("admin_5993", table_name, table_name);
	metadata
		.fields
		.insert("published_on".to_string(), FieldMetadata::new(field_type));
	global_registry().register_model(metadata);
}

// ==================== Happy path tests ====================

/// Verify that get_list returns records with correct pagination metadata
#[rstest]
#[tokio::test]
async fn test_get_list_happy_path(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	for i in 0..3 {
		let mut data = HashMap::new();
		data.insert("name".to_string(), json!(format!("Item {}", i)));
		data.insert("status".to_string(), json!("active"));
		db.create::<AdminRecord>("test_models", None, data)
			.await
			.expect("Failed to create test record");
	}

	let params = ListQueryParams::default();

	// Act
	let result = get_list(
		"TestModel".to_string(),
		params,
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	assert!(result.is_ok(), "get_list should succeed: {:?}", result);
	let response = result.unwrap();
	assert_eq!(response.model_name, "TestModel");
	assert!(response.count >= 3, "Should have at least 3 records");
	assert_eq!(response.page, 1);
	assert!(response.page_size > 0);
	assert!(response.total_pages >= 1);
}

/// Verify that search filters records by search fields (OR logic)
#[rstest]
#[tokio::test]
async fn test_get_list_with_search(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("UniqueSearchTarget"));
	data.insert("status".to_string(), json!("active"));
	db.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	let params = ListQueryParams {
		search: Some("UniqueSearchTarget".to_string()),
		..Default::default()
	};

	// Act
	let result = get_list(
		"TestModel".to_string(),
		params,
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed");
	assert!(
		response.count >= 1,
		"Should find at least 1 matching record"
	);
}

/// Verify that filter by allowed field works
#[rstest]
#[tokio::test]
async fn test_get_list_with_filter(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Filter Test"));
	data.insert("status".to_string(), json!("filterable_status"));
	db.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	let mut filters = HashMap::new();
	filters.insert("status".to_string(), "filterable_status".to_string());

	let params = ListQueryParams {
		filters,
		..Default::default()
	};

	// Act
	let result = get_list(
		"TestModel".to_string(),
		params,
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed with valid filter");
	assert!(
		response.count >= 1,
		"Should find records matching the filter"
	);
}

/// Verify that the admin scope is ANDed with search and cannot be replaced by client filters.
#[rstest]
#[tokio::test]
async fn test_get_list_queryset_scope_applies_to_results_and_count(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	site.unregister("TestModel")
		.expect("Failed to unregister default TestModel admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("test_models").with_queryset_filter(Filter::new(
			"status",
			FilterOperator::Eq,
			FilterValue::String("scope-visible".to_string()),
		)),
	)
	.expect("Failed to register scoped TestModel admin");

	for status in ["scope-visible", "scope-hidden"] {
		let mut data = HashMap::new();
		data.insert("name".to_string(), json!("ScopedSearchTarget"));
		data.insert("status".to_string(), json!(status));
		db.create::<AdminRecord>("test_models", None, data)
			.await
			.expect("Failed to create scoped test record");
	}

	// Act
	let scoped = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			search: Some("ScopedSearchTarget".to_string()),
			..Default::default()
		},
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("scoped get_list should succeed");

	let mut conflicting_filters = HashMap::new();
	conflicting_filters.insert("status".to_string(), "scope-hidden".to_string());
	let conflicting = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			search: Some("ScopedSearchTarget".to_string()),
			filters: conflicting_filters,
			..Default::default()
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("conflicting client filter should produce an empty list");

	// Assert
	assert_eq!(scoped.count, 1);
	assert_eq!(scoped.results.len(), 1);
	assert_eq!(
		scoped.results[0].get("status"),
		Some(&json!("scope-visible"))
	);
	assert_eq!(conflicting.count, 0);
	assert!(conflicting.results.is_empty());
}

/// Verify that custom `to_field` relationships join physical columns and return nested data.
#[rstest]
#[tokio::test]
async fn test_get_list_select_related_uses_custom_to_field_physical_columns() {
	// Arrange
	let mut backend = MockDatabaseBackend::new();
	backend
		.expect_database_type()
		.return_const(DatabaseType::Postgres);
	backend
		.expect_fetch_all()
		.withf(|sql, params| {
			sql == "SELECT \"admin_list_select_related_to_field_sources_5992\".*, COUNT(*) OVER() AS \"__reinhardt_total_count\", \"target\".\"id\" AS \"__reinhardt_related_0__id\", \"target\".\"target_slug_column_5992\" AS \"__reinhardt_related_0__target_slug_column_5992\" FROM \"admin_list_select_related_to_field_sources_5992\" LEFT JOIN \"admin_list_select_related_to_field_targets_5992\" AS \"target\" ON \"admin_list_select_related_to_field_sources_5992\".\"source_target_slug_column_5992\" = \"target\".\"target_slug_column_5992\" ORDER BY \"admin_list_select_related_to_field_sources_5992\".\"id\" DESC LIMIT $1 OFFSET $2"
				&& params.as_slice() == [QueryValue::Int(25), QueryValue::Int(0)]
		})
		.times(1)
		.returning(|_, _| {
			let mut row = Row::new();
			row.insert("id".to_string(), QueryValue::Int(41));
			row.insert(
				"source_target_slug_column_5992".to_string(),
				QueryValue::String("target-slug-7".to_string()),
			);
			row.insert(
				"__reinhardt_total_count".to_string(),
				QueryValue::Int(1),
			);
			row.insert(
				"__reinhardt_related_0__id".to_string(),
				QueryValue::Int(7),
			);
			row.insert(
				"__reinhardt_related_0__target_slug_column_5992".to_string(),
				QueryValue::String("target-slug-7".to_string()),
			);
			Ok(vec![row])
		});
	backend.expect_fetch_one().times(0);
	let connection = BackendsConnection::new(Arc::new(backend));
	let connection_lease = DatabaseConnectionLease::register(connection)
		.expect("Failed to register mock database connection");
	let db = KeyedDepends::<AdminDatabaseKey, AdminDatabase>::from_value(AdminDatabase::new(
		connection_lease.handle(),
	));
	let site = AdminSite::new("Custom to_field list admin");
	site.register(
		ADMIN_TO_FIELD_SOURCE_MODEL_NAME,
		AllPermissionsModelAdmin::list_select_related_to_field_model(),
	)
	.expect("Failed to register custom to_field list admin");
	let site = KeyedDepends::<AdminSiteKey, AdminSite>::from_value(site);

	// Act
	let response = get_list(
		ADMIN_TO_FIELD_SOURCE_MODEL_NAME.to_string(),
		ListQueryParams {
			sort_by: Some("-id".to_string()),
			page: Some(1),
			page_size: Some(25),
			..Default::default()
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("custom to_field list query should succeed");

	// Assert
	assert_eq!(response.model_name, ADMIN_TO_FIELD_SOURCE_MODEL_NAME);
	assert_eq!(response.count, 1);
	assert_eq!(
		response.results,
		vec![HashMap::from([
			("id".to_string(), json!(41)),
			(
				"source_target_slug_column_5992".to_string(),
				json!("target-slug-7"),
			),
			(
				"target".to_string(),
				json!({
					"id": 7,
					"target_slug_column_5992": "target-slug-7"
				}),
			),
		])]
	);
}

/// Verify computed response metadata/value and the real SQL sort-field mapping.
#[tokio::test]
async fn test_get_list_computed_column_maps_sort_to_real_database_field() {
	// Arrange
	let mut backend = MockDatabaseBackend::new();
	backend
		.expect_database_type()
		.return_const(DatabaseType::Postgres);
	backend
		.expect_fetch_all()
		.withf(|sql, params| {
			sql == "SELECT \"admin_computed_columns_5993\".*, COUNT(*) OVER() AS \"__reinhardt_total_count\" FROM \"admin_computed_columns_5993\" ORDER BY \"admin_computed_columns_5993\".\"created_at\" DESC LIMIT $1 OFFSET $2"
				&& params.as_slice() == [QueryValue::Int(25), QueryValue::Int(0)]
		})
		.times(1)
		.returning(|_, _| {
			let mut row = Row::new();
			row.insert("id".to_string(), QueryValue::Int(7));
			row.insert("created_at".to_string(), QueryValue::String("2024-02-29".to_string()));
			row.insert("__reinhardt_total_count".to_string(), QueryValue::Int(1));
			Ok(vec![row])
		});
	backend.expect_fetch_one().times(0);
	let connection = BackendsConnection::new(Arc::new(backend));
	let connection_lease = DatabaseConnectionLease::register(connection)
		.expect("Failed to register mock database connection");
	let db = KeyedDepends::<AdminDatabaseKey, AdminDatabase>::from_value(AdminDatabase::new(
		connection_lease.handle(),
	));
	let site = AdminSite::new("Computed columns admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("admin_computed_columns_5993")
			.with_list_columns(vec![
				ListColumn::Field {
					field: "id".to_string(),
					label: "Record ID".to_string(),
				},
				ListColumn::Computed {
					key: "summary".to_string(),
					label: "Safe summary".to_string(),
					sort_field: Some("created_at".to_string()),
				},
			])
			.with_computed_value("summary", json!({"text": "<script>safe data</script>"})),
	)
	.expect("Failed to register computed columns admin");

	// Act
	let response = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			sort_by: Some("-summary".to_string()),
			page: Some(1),
			page_size: Some(25),
			..Default::default()
		},
		KeyedDepends::<AdminSiteKey, AdminSite>::from_value(site),
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("computed changelist query should succeed");

	// Assert
	let columns = response
		.columns
		.expect("computed column metadata should be returned");
	assert_eq!(columns.len(), 2);
	assert_eq!(columns[1].field, "summary");
	assert_eq!(columns[1].label, "Safe summary");
	assert!(columns[1].sortable);
	assert_eq!(
		response.results[0].get("summary"),
		Some(&json!({"text": "<script>safe data</script>"}))
	);
}

/// Verify computed hook failures expose only a fixed server error.
#[tokio::test]
async fn test_get_list_computed_hook_error_is_sanitized() {
	// Arrange
	let mut backend = MockDatabaseBackend::new();
	backend
		.expect_database_type()
		.return_const(DatabaseType::Postgres);
	backend.expect_fetch_all().times(1).returning(|_, _| {
		let mut row = Row::new();
		row.insert("id".to_string(), QueryValue::Int(7));
		row.insert("__reinhardt_total_count".to_string(), QueryValue::Int(1));
		Ok(vec![row])
	});
	backend.expect_fetch_one().times(0);
	let connection = BackendsConnection::new(Arc::new(backend));
	let connection_lease = DatabaseConnectionLease::register(connection)
		.expect("Failed to register mock database connection");
	let db = KeyedDepends::<AdminDatabaseKey, AdminDatabase>::from_value(AdminDatabase::new(
		connection_lease.handle(),
	));
	let site = AdminSite::new("Failing computed columns admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("admin_computed_error_5993")
			.with_list_columns(vec![
				ListColumn::Field {
					field: "id".to_string(),
					label: "ID".to_string(),
				},
				ListColumn::Computed {
					key: "secret".to_string(),
					label: "Secret".to_string(),
					sort_field: None,
				},
			])
			.with_computed_error("secret", "internal secret from computed hook"),
	)
	.expect("Failed to register failing computed columns admin");

	// Act
	let error = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			sort_by: None,
			..Default::default()
		},
		KeyedDepends::<AdminSiteKey, AdminSite>::from_value(site),
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("computed hook failure should fail the request");

	// Assert
	assert_eq!(error.status(), Some(500));
	assert_eq!(error.user_message(), "Failed to compute list column");
	assert!(!error.user_message().contains("internal secret"));
}

/// Verify month choices use the same queryset, search, filter, and parent-year scope.
#[tokio::test]
#[serial_test::serial(admin_date_hierarchy_5993)]
async fn test_get_list_date_hierarchy_choices_preserve_full_scope() {
	// Arrange
	let table_name = "admin_date_hierarchy_scope_5993";
	register_date_hierarchy_metadata(table_name, reinhardt_db::migrations::FieldType::Date);
	let mut backend = MockDatabaseBackend::new();
	backend
		.expect_database_type()
		.return_const(DatabaseType::Postgres);
	backend
		.expect_fetch_all()
		.withf(|sql, params| {
			sql.starts_with("SELECT \"admin_date_hierarchy_scope_5993\".*, COUNT(*) OVER()")
				&& sql.contains("\"status\" = $1")
				&& sql.contains("\"name\" LIKE $2")
				&& sql.contains("\"description\" LIKE $3")
				&& sql.contains("\"published_on\" >= CAST($5 AS DATE)")
				&& sql.contains("\"published_on\" < CAST($6 AS DATE)")
				&& params.as_slice()
					== [
						QueryValue::String("visible".to_string()),
						QueryValue::String("%needle%".to_string()),
						QueryValue::String("%needle%".to_string()),
						QueryValue::String("visible".to_string()),
						QueryValue::String("2024-01-01".to_string()),
						QueryValue::String("2025-01-01".to_string()),
						QueryValue::Int(100),
						QueryValue::Int(0),
					]
		})
		.times(1)
		.returning(|_, _| {
			let mut row = Row::new();
			row.insert("id".to_string(), QueryValue::Int(9));
			row.insert("__reinhardt_total_count".to_string(), QueryValue::Int(1));
			Ok(vec![row])
		});
	backend
		.expect_fetch_all()
		.withf(|sql, params| {
			sql.starts_with("SELECT DISTINCT DATE_TRUNC('month', \"published_on\")::date")
				&& sql.contains("\"status\" = $1")
				&& sql.contains("\"name\" LIKE $2")
				&& sql.contains("\"description\" LIKE $3")
				&& sql.contains("\"published_on\" >= CAST($5 AS DATE)")
				&& sql.contains("\"published_on\" < CAST($6 AS DATE)")
				&& sql.ends_with("ORDER BY \"__reinhardt_date_hierarchy\" ASC")
				&& params.as_slice()
					== [
						QueryValue::String("visible".to_string()),
						QueryValue::String("%needle%".to_string()),
						QueryValue::String("%needle%".to_string()),
						QueryValue::String("visible".to_string()),
						QueryValue::String("2024-01-01".to_string()),
						QueryValue::String("2025-01-01".to_string()),
					]
		})
		.times(1)
		.returning(|_, _| {
			let mut row = Row::new();
			row.insert(
				"__reinhardt_date_hierarchy".to_string(),
				QueryValue::String("2024-02-01".to_string()),
			);
			Ok(vec![row])
		});
	backend.expect_fetch_one().times(0);
	let connection = BackendsConnection::new(Arc::new(backend));
	let connection_lease = DatabaseConnectionLease::register(connection)
		.expect("Failed to register mock database connection");
	let db = KeyedDepends::<AdminDatabaseKey, AdminDatabase>::from_value(AdminDatabase::new(
		connection_lease.handle(),
	));
	let site = AdminSite::new("Scoped date hierarchy admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model(table_name)
			.with_queryset_filter(Filter::new(
				"status",
				FilterOperator::Eq,
				FilterValue::String("visible".to_string()),
			))
			.with_date_hierarchy("published_on"),
	)
	.expect("Failed to register scoped date hierarchy admin");
	let mut filters = HashMap::new();
	filters.insert("status".to_string(), "visible".to_string());

	// Act
	let response = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			search: Some("needle".to_string()),
			filters,
			date_hierarchy: Some(DateHierarchySelection {
				year: Some(2024),
				month: None,
				day: None,
			}),
			..Default::default()
		},
		KeyedDepends::<AdminSiteKey, AdminSite>::from_value(site),
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("scoped hierarchy query should succeed");

	// Assert
	let hierarchy = response
		.date_hierarchy
		.expect("hierarchy metadata should be returned");
	assert_eq!(hierarchy.field, "published_on");
	assert_eq!(hierarchy.next_level, Some(DateHierarchyLevel::Month));
	assert_eq!(hierarchy.choices, vec![2]);
}

/// Verify naive datetime hierarchy bounds and choices do not depend on session timezone.
#[rstest]
#[tokio::test]
#[serial_test::serial(admin_date_hierarchy_5993)]
async fn test_get_list_datetime_hierarchy_preserves_naive_calendar_in_non_utc_session(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) {
	// Arrange
	let (_fixture_pool, database_url) = shared_db_pool.await;
	let pool = PgPoolOptions::new()
		.max_connections(2)
		.after_connect(|connection, _| {
			Box::pin(async move {
				sqlx::query("SET TIME ZONE 'America/Los_Angeles'")
					.execute(&mut *connection)
					.await?;
				Ok(())
			})
		})
		.connect(&database_url)
		.await
		.expect("Failed to create non-UTC PostgreSQL pool");
	let timezone = sqlx::query_scalar::<_, String>("SHOW TIME ZONE")
		.fetch_one(&pool)
		.await
		.expect("Failed to read PostgreSQL session timezone");
	assert_eq!(timezone, "America/Los_Angeles");

	let table_name = "admin_datetime_hierarchy_timezone_5993";
	pool.execute(
		"CREATE TABLE admin_datetime_hierarchy_timezone_5993 (\
			id BIGSERIAL PRIMARY KEY, \
			published_on TIMESTAMP WITHOUT TIME ZONE NOT NULL\
		)",
	)
	.await
	.expect("Failed to create naive datetime hierarchy table");
	let previous_year = chrono::NaiveDate::from_ymd_opt(2023, 12, 31)
		.unwrap()
		.and_hms_opt(23, 30, 0)
		.unwrap();
	let selected_year = chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
		.unwrap()
		.and_hms_opt(0, 30, 0)
		.unwrap();
	sqlx::query(
		"INSERT INTO admin_datetime_hierarchy_timezone_5993 (published_on) VALUES ($1), ($2)",
	)
	.bind(previous_year)
	.bind(selected_year)
	.execute(&pool)
	.await
	.expect("Failed to insert naive datetime hierarchy rows");
	register_date_hierarchy_metadata(table_name, reinhardt_db::migrations::FieldType::DateTime);

	let backend = Arc::new(PostgresBackend::new(pool));
	let connection = BackendsConnection::new(backend);
	let connection_lease = DatabaseConnectionLease::register(connection)
		.expect("Failed to register non-UTC database connection");
	let db = KeyedDepends::<AdminDatabaseKey, AdminDatabase>::from_value(AdminDatabase::new(
		connection_lease.handle(),
	));
	let site = AdminSite::new("Naive datetime hierarchy admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model(table_name).with_date_hierarchy("published_on"),
	)
	.expect("Failed to register naive datetime hierarchy admin");

	// Act
	let response = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			date_hierarchy: Some(DateHierarchySelection {
				year: Some(2024),
				month: None,
				day: None,
			}),
			..Default::default()
		},
		KeyedDepends::<AdminSiteKey, AdminSite>::from_value(site),
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("naive datetime hierarchy should be independent of session timezone");

	// Assert
	assert_eq!(response.count, 1);
	assert_eq!(response.results.len(), 1);
	assert_eq!(response.results[0].get("id"), Some(&json!(2)));
	let hierarchy = response
		.date_hierarchy
		.expect("naive datetime hierarchy metadata should be returned");
	assert_eq!(hierarchy.next_level, Some(DateHierarchyLevel::Month));
	assert_eq!(hierarchy.choices, vec![1]);
}

/// Verify that invalid related configuration and hook errors perform no database query.
#[tokio::test]
#[serial_test::serial(admin_date_hierarchy_5993)]
async fn test_get_list_configuration_errors_perform_zero_database_queries() {
	// Arrange
	let mut backend = MockDatabaseBackend::new();
	backend
		.expect_database_type()
		.return_const(DatabaseType::Postgres);
	backend.expect_fetch_all().times(0);
	backend.expect_fetch_one().times(0);
	let connection = BackendsConnection::new(Arc::new(backend));
	let connection_lease = DatabaseConnectionLease::register(connection)
		.expect("Failed to register mock database connection");
	let db = KeyedDepends::<AdminDatabaseKey, AdminDatabase>::from_value(AdminDatabase::new(
		connection_lease.handle(),
	));

	let site = AdminSite::new("Fail-closed queryset admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("missing_admin_table")
			.with_list_select_related(vec!["owner"]),
	)
	.expect("Failed to register invalid related TestModel admin");
	let site = KeyedDepends::<AdminSiteKey, AdminSite>::from_value(site);

	// Act
	let invalid_related = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	site.unregister("TestModel")
		.expect("Failed to unregister invalid related TestModel admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("missing_admin_table")
			.with_queryset_error("queryset hook failed"),
	)
	.expect("Failed to register failing TestModel admin");
	let hook_error = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	site.unregister("TestModel")
		.expect("Failed to unregister failing TestModel admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("missing_admin_table").with_list_columns(vec![
			ListColumn::Field {
				field: "id".to_string(),
				label: "ID".to_string(),
			},
			ListColumn::Computed {
				key: "badge".to_string(),
				label: "Badge".to_string(),
				sort_field: None,
			},
		]),
	)
	.expect("Failed to register unmapped computed sort admin");
	let unmapped_sort = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			sort_by: Some("badge".to_string()),
			..Default::default()
		},
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	register_date_hierarchy_metadata(
		"admin_invalid_hierarchy_type_5993",
		reinhardt_db::migrations::FieldType::VarChar(50),
	);
	site.unregister("TestModel")
		.expect("Failed to unregister computed sort admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("admin_invalid_hierarchy_type_5993")
			.with_date_hierarchy("published_on"),
	)
	.expect("Failed to register invalid hierarchy type admin");
	let invalid_hierarchy_type = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	site.unregister("TestModel")
		.expect("Failed to unregister invalid hierarchy type admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("missing_admin_table")
			.with_date_hierarchy("published_on"),
	)
	.expect("Failed to register missing hierarchy field admin");
	let missing_hierarchy_field = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	register_date_hierarchy_metadata(
		"admin_invalid_hierarchy_selection_5993",
		reinhardt_db::migrations::FieldType::Date,
	);
	site.unregister("TestModel")
		.expect("Failed to unregister missing hierarchy field admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::test_model("admin_invalid_hierarchy_selection_5993")
			.with_date_hierarchy("published_on"),
	)
	.expect("Failed to register invalid hierarchy selection admin");
	let invalid_hierarchy_selection = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			date_hierarchy: Some(DateHierarchySelection {
				year: None,
				month: Some(2),
				day: None,
			}),
			..Default::default()
		},
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	let invalid_hierarchy_boundary = get_list(
		"TestModel".to_string(),
		ListQueryParams {
			date_hierarchy: Some(DateHierarchySelection {
				year: Some(262_142),
				month: Some(12),
				day: Some(31),
			}),
			..Default::default()
		},
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	let related_error = invalid_related.expect_err("invalid relationship should fail the request");
	assert!(
		related_error
			.to_string()
			.contains("cannot resolve model metadata")
	);
	let hook_error = hook_error.expect_err("queryset hook error should fail the request");
	assert!(hook_error.to_string().contains("queryset hook failed"));
	assert_eq!(
		unmapped_sort
			.expect_err("unmapped computed sort should fail before database access")
			.status(),
		Some(400)
	);
	assert_eq!(
		invalid_hierarchy_type
			.expect_err("invalid hierarchy type should fail before database access")
			.status(),
		Some(400)
	);
	assert_eq!(
		missing_hierarchy_field
			.expect_err("missing hierarchy field should fail before database access")
			.status(),
		Some(400)
	);
	assert_eq!(
		invalid_hierarchy_selection
			.expect_err("invalid hierarchy selection should fail before database access")
			.status(),
		Some(400)
	);
	assert_eq!(
		invalid_hierarchy_boundary
			.expect_err("invalid hierarchy boundary should fail before database access")
			.status(),
		Some(400)
	);
}

/// Verify that descending sort with "-" prefix works
#[rstest]
#[tokio::test]
async fn test_get_list_sort_descending(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	let params = ListQueryParams {
		sort_by: Some("-name".to_string()),
		..Default::default()
	};

	// Act
	let result = get_list(
		"TestModel".to_string(),
		params,
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"get_list should succeed with descending sort: {:?}",
		result
	);
}

// ==================== Validation tests ====================

/// Regression test for Issue #2922: sort_by parameter not validated against allowed fields
#[rstest]
#[tokio::test]
async fn test_get_list_sort_by_invalid_field(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	let params = ListQueryParams {
		sort_by: Some("nonexistent_column".to_string()),
		..Default::default()
	};

	// Act
	let result = get_list(
		"TestModel".to_string(),
		params,
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should reject sort_by with invalid field name"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("sort field") || err.contains("400") || err.contains("Unknown"),
		"Error should indicate invalid sort field: {}",
		err
	);
}

/// Verify that unknown filter field returns 400 error
#[rstest]
#[tokio::test]
async fn test_get_list_unknown_filter_field(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	let mut filters = HashMap::new();
	filters.insert("nonexistent_field".to_string(), "some_value".to_string());

	let params = ListQueryParams {
		filters,
		..Default::default()
	};

	// Act
	let result = get_list(
		"TestModel".to_string(),
		params,
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should reject filter with unknown field name"
	);
	let err = format!("{}", result.unwrap_err());
	assert!(
		err.contains("filter field") || err.contains("400") || err.contains("Unknown"),
		"Error should indicate unknown filter field: {}",
		err
	);
}

// ==================== Pagination tests ====================

/// Verify default pagination: page=1, page_size=25
#[rstest]
#[tokio::test]
async fn test_get_list_pagination_defaults(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act
	let result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed");
	assert_eq!(response.page, 1, "Default page should be 1");
	assert!(
		response.page_size <= 500,
		"Default page_size should not exceed MAX_PAGE_SIZE"
	);
}

/// Verify that page_size is capped at MAX_PAGE_SIZE (500)
#[rstest]
#[tokio::test]
async fn test_get_list_page_size_capped(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	let params = ListQueryParams {
		page_size: Some(10000),
		..Default::default()
	};

	// Act
	let result = get_list(
		"TestModel".to_string(),
		params,
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed with large page_size");
	assert!(
		response.page_size <= 500,
		"Page size should be capped at MAX_PAGE_SIZE(500), got {}",
		response.page_size
	);
}

/// Verify that page=0 is treated as page=1
#[rstest]
#[tokio::test]
async fn test_get_list_page_zero_treated_as_one(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	let params = ListQueryParams {
		page: Some(0),
		..Default::default()
	};

	// Act
	let result = get_list(
		"TestModel".to_string(),
		params,
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed with page=0");
	assert_eq!(response.page, 1, "Page 0 should be treated as page 1");
}

// ==================== Edge case tests ====================

/// Verify that get_list with empty table returns count=0, total_pages=1
#[rstest]
#[tokio::test]
async fn test_get_list_empty_table(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act (no records inserted)
	let result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed on empty table");
	assert_eq!(response.total_pages, 1, "Empty table should have 1 page");
}

// ==================== Contract tests ====================

/// Verify that response columns match model_admin.list_display()
#[rstest]
#[tokio::test]
async fn test_get_list_columns_match_list_display(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act
	let result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed");
	let columns = response.columns.expect("Should have columns");
	let column_names: Vec<&str> = columns.iter().map(|c| c.field.as_str()).collect();
	// model_admin_config fixture has list_display: ["id", "name", "created_at"]
	assert!(column_names.contains(&"id"), "Columns should contain 'id'");
	assert!(
		column_names.contains(&"name"),
		"Columns should contain 'name'"
	);
	assert!(
		column_names.contains(&"created_at"),
		"Columns should contain 'created_at'"
	);
}

/// Verify that response available_filters match model_admin.list_filter()
#[rstest]
#[tokio::test]
async fn test_get_list_filters_match_list_filter(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act
	let result = get_list(
		"TestModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_list should succeed");
	let filters = response
		.available_filters
		.expect("Should have available_filters");
	let filter_fields: Vec<&str> = filters.iter().map(|f| f.field.as_str()).collect();
	// model_admin_config fixture has list_filter: ["status"]
	assert!(
		filter_fields.contains(&"status"),
		"Filters should contain 'status'"
	);
}

// ==================== Error path tests ====================

/// Verify that get_list returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_get_list_model_not_registered(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let auth_user = make_auth_user();

	// Act
	let result = get_list(
		"NonExistentModel".to_string(),
		ListQueryParams::default(),
		site,
		db,
		make_staff_request(),
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should return error for unregistered model"
	);
}
