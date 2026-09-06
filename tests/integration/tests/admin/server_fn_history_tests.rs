//! Integration tests for persistent per-object admin history.

use super::server_fn_helpers::{
	AdminDatabaseDepends, AdminSiteDepends, DenyAllPermissionsModelAdmin, ServerFnContext,
	StringPkContext, TEST_CSRF_TOKEN, make_auth_user, make_staff_request, server_fn_context,
	string_pk_context,
};
use reinhardt_admin::core::{AdminRecord, AdminSite};
use reinhardt_admin::server::{
	bulk_delete_records, create_record, delete_record, get_history, update_record,
};
use reinhardt_admin::types::{
	BulkDeleteRequest, HistoryResponse, MutationRequest, MutationResponse,
};
use reinhardt_db::orm::{OrmExecutor, QueryValue};
use reinhardt_di::KeyedDepends;
use reinhardt_pages::server_fn::{ServerFnError, ServerFnErrorKind};
use rstest::rstest;
use serde_json::json;
use serial_test::serial;
use std::collections::HashMap;

fn model_identity(model_name: &str, table_name: &str) -> Vec<u8> {
	let mut identity = Vec::with_capacity(16 + model_name.len() + table_name.len());
	identity.extend_from_slice(&(model_name.len() as u64).to_be_bytes());
	identity.extend_from_slice(model_name.as_bytes());
	identity.extend_from_slice(&(table_name.len() as u64).to_be_bytes());
	identity.extend_from_slice(table_name.as_bytes());
	identity
}

fn mutation(fields: &[(&str, &str)]) -> MutationRequest {
	MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data: fields
			.iter()
			.map(|(name, value)| ((*name).to_string(), json!(value)))
			.collect::<HashMap<_, _>>(),
	}
}

async fn seed_record(context: &ServerFnContext, name: &str) -> String {
	let (_, db, _) = context;
	let mut connection = *db.connection();
	let row = OrmExecutor::fetch_one(
		&mut connection,
		"INSERT INTO test_models (name, status, description) \
		 VALUES ($1, $2, $3) RETURNING id",
		vec![
			QueryValue::String(name.to_string()),
			QueryValue::String("active".to_string()),
			QueryValue::String("seed description".to_string()),
		],
	)
	.await
	.expect("test object must be seeded");
	let id: i64 = row.get("id").expect("seeded object must return id");
	id.to_string()
}

async fn update_name(
	context: &ServerFnContext,
	object_id: &str,
	name: &str,
) -> Result<MutationResponse, ServerFnError> {
	let (site, db, _) = context;
	update_record(
		"testmodel".to_string(),
		object_id.to_string(),
		mutation(&[("name", name)]),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
}

async fn query_history_result(
	context: &ServerFnContext,
	object_id: &str,
	page: u64,
) -> Result<HistoryResponse, ServerFnError> {
	let (site, db, _) = context;
	get_history(
		"testmodel".to_string(),
		object_id.to_string(),
		page,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
}

async fn query_history(context: &ServerFnContext, object_id: &str, page: u64) -> HistoryResponse {
	query_history_result(context, object_id, page)
		.await
		.expect("authorized history query must succeed")
}

async fn query_string_pk_history_result(
	context: &StringPkContext,
	object_id: &str,
) -> Result<HistoryResponse, ServerFnError> {
	let (site, db, _, _, _) = context;
	get_history(
		"stringpkmodel".to_string(),
		object_id.to_string(),
		1,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
}

async fn query_string_pk_history(context: &StringPkContext, object_id: &str) -> HistoryResponse {
	query_string_pk_history_result(context, object_id)
		.await
		.expect("authorized string primary key history query must succeed")
}

fn assert_object_not_found(error: ServerFnError) {
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(404));
	assert_eq!(error.user_message(), "Object not found");
}

async fn insert_poison_history(
	context: &ServerFnContext,
	model_name: &str,
	table_name: &str,
	object_id: &str,
) -> i64 {
	let (_, db, _) = context;
	let mut connection = *db.connection();
	let row = OrmExecutor::fetch_one(
		&mut connection,
		"INSERT INTO reinhardt_admin_history (\
			occurred_at, actor, action_name, model_name, model_identity, object_id, \
			object_identity, object_repr, changed_fields, affected_count, success\
		) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
		vec![
			QueryValue::String("2026-08-09T01:02:03.000000Z".to_string()),
			QueryValue::String("poison-actor".to_string()),
			QueryValue::String("POISON".to_string()),
			QueryValue::String(model_name.to_string()),
			QueryValue::Bytes(model_identity(model_name, table_name)),
			QueryValue::String(object_id.to_string()),
			QueryValue::Bytes(object_id.as_bytes().to_vec()),
			QueryValue::String(format!("{model_name} ({object_id})")),
			QueryValue::String("[]".to_string()),
			QueryValue::Int(1),
			QueryValue::Bool(true),
		],
	)
	.await
	.expect("poison history row must insert");
	row.get("id").expect("poison row must return id")
}

async fn exact_history_ids(context: &ServerFnContext, object_id: &str) -> Vec<i64> {
	let (_, db, _) = context;
	let mut connection = *db.connection();
	OrmExecutor::fetch_all(
		&mut connection,
		"SELECT id FROM reinhardt_admin_history \
		 WHERE model_identity = $1 AND object_identity = $2 ORDER BY id DESC",
		vec![
			QueryValue::Bytes(model_identity("TestModel", "test_models")),
			QueryValue::Bytes(object_id.as_bytes().to_vec()),
		],
	)
	.await
	.expect("history ids must be independently queryable")
	.into_iter()
	.map(|row| row.get("id").expect("history row must have id"))
	.collect()
}

async fn exact_history_actions(
	db: &AdminDatabaseDepends,
	model_name: &str,
	table_name: &str,
	object_id: &str,
) -> Vec<String> {
	let mut connection = *db.connection();
	OrmExecutor::fetch_all(
		&mut connection,
		"SELECT action_name FROM reinhardt_admin_history \
		 WHERE model_identity = $1 AND object_identity = $2 ORDER BY id DESC",
		vec![
			QueryValue::Bytes(model_identity(model_name, table_name)),
			QueryValue::Bytes(object_id.as_bytes().to_vec()),
		],
	)
	.await
	.expect("history actions must be independently queryable")
	.into_iter()
	.map(|row| {
		row.get("action_name")
			.expect("history row must have an action")
	})
	.collect()
}

#[rstest]
#[tokio::test]
async fn deleted_object_history_remains_persisted_but_is_not_queryable(
	#[future] server_fn_context: ServerFnContext,
) {
	// Arrange
	let context = server_fn_context.await;
	let (site, db, _connection_lease) = &context;
	let create_values = [
		("name", "create-secret-name"),
		("status", "draft"),
		("description", "create-secret-description"),
	];
	let created = create_record(
		"testmodel".to_string(),
		mutation(&create_values),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("create must succeed");
	let object_id = created
		.affected
		.expect("numeric test primary key must be returned")
		.to_string();
	update_name(&context, &format!("0{object_id}"), "update-secret-name")
		.await
		.expect("update must succeed");
	let visible_history = query_history(&context, &format!("0{object_id}"), 1).await;
	delete_record(
		"testmodel".to_string(),
		object_id.clone(),
		TEST_CSRF_TOKEN.to_string(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("delete must succeed");

	// Act
	let history_result = query_history_result(&context, &format!("0{object_id}"), 1).await;
	let actions = exact_history_actions(db, "TestModel", "test_models", &object_id).await;
	let serialized =
		serde_json::to_string(&visible_history).expect("visible history must serialize");

	// Assert
	assert_object_not_found(
		history_result.expect_err("deleted object history must be outside the active scope"),
	);
	assert_eq!(actions, ["DELETE", "UPDATE", "CREATE"]);
	assert_eq!(visible_history.count, 2);
	assert_eq!(visible_history.page, 1);
	assert_eq!(visible_history.page_size, 25);
	assert_eq!(visible_history.total_pages, 1);
	assert_eq!(
		visible_history
			.results
			.iter()
			.map(|entry| entry.action_name.as_str())
			.collect::<Vec<_>>(),
		["UPDATE", "CREATE"]
	);
	assert!(visible_history.results.iter().all(|entry| {
		entry.actor == "test_staff"
			&& entry.model_name == "TestModel"
			&& entry.object_id == object_id
			&& entry.object_repr == format!("TestModel ({object_id})")
			&& entry.affected_count == 1
			&& entry.success
	}));
	assert_eq!(visible_history.results[0].changed_fields, ["name"]);
	assert_eq!(
		visible_history.results[1].changed_fields,
		["description", "name", "status"]
	);
	for raw_value in [
		"create-secret-name",
		"create-secret-description",
		"update-secret-name",
	] {
		assert!(!serialized.contains(raw_value));
	}
}

#[rstest]
#[tokio::test]
#[serial(model_registry)]
async fn numeric_looking_string_primary_keys_remain_distinct_across_crud_and_history(
	#[future] string_pk_context: StringPkContext,
) {
	// Arrange
	let context = string_pk_context.await;
	let (site, db, _, _connection_lease, _registry_guard) = &context;
	for (id, name) in [("01", "leading-zero"), ("1", "plain-one")] {
		create_record(
			"stringpkmodel".to_string(),
			mutation(&[("id", id), ("name", name), ("status", "active")]),
			site.clone(),
			db.clone(),
			make_staff_request(),
			make_auth_user(),
		)
		.await
		.expect("string primary key create must succeed");
	}

	// Act
	update_record(
		"stringpkmodel".to_string(),
		"01".to_string(),
		mutation(&[("name", "updated-leading-zero")]),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("leading-zero primary key update must succeed");
	update_record(
		"stringpkmodel".to_string(),
		"1".to_string(),
		mutation(&[("name", "updated-plain-one")]),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("plain primary key update must succeed");
	delete_record(
		"stringpkmodel".to_string(),
		"01".to_string(),
		TEST_CSRF_TOKEN.to_string(),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("leading-zero primary key delete must succeed");
	let leading_zero_history = query_string_pk_history_result(&context, "01").await;
	let plain_history = query_string_pk_history(&context, "1").await;
	let leading_zero_actions =
		exact_history_actions(db, "StringPkModel", "string_pk_test_models", "01").await;
	let deleted = db
		.get::<AdminRecord>("string_pk_test_models", "id", "01")
		.await
		.expect("deleted string primary key lookup must succeed");
	let remaining = db
		.get::<AdminRecord>("string_pk_test_models", "id", "1")
		.await
		.expect("remaining string primary key lookup must succeed")
		.expect("plain primary key record must remain");

	// Assert
	assert_object_not_found(
		leading_zero_history.expect_err("deleted string primary key history must be scoped out"),
	);
	assert!(deleted.is_none());
	assert_eq!(remaining.get("name"), Some(&json!("updated-plain-one")));
	assert_eq!(plain_history.count, 2);
	assert_eq!(leading_zero_actions, ["DELETE", "UPDATE", "CREATE"]);
	assert_eq!(
		plain_history
			.results
			.iter()
			.map(|entry| (entry.object_id.as_str(), entry.action_name.as_str()))
			.collect::<Vec<_>>(),
		[("1", "UPDATE"), ("1", "CREATE")]
	);
}

#[rstest]
#[tokio::test]
async fn bulk_delete_hides_deleted_object_history(#[future] server_fn_context: ServerFnContext) {
	// Arrange
	let context = server_fn_context.await;
	let first_id = seed_record(&context, "bulk-first").await;
	let second_id = seed_record(&context, "bulk-second").await;
	let missing_id = "999999999".to_string();
	let (site, db, _connection_lease) = &context;

	// Act
	let response = bulk_delete_records(
		"testmodel".to_string(),
		BulkDeleteRequest {
			csrf_token: TEST_CSRF_TOKEN.to_string(),
			ids: vec![
				format!("0{first_id}"),
				missing_id.clone(),
				first_id.clone(),
				second_id.clone(),
			],
		},
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("bulk delete must succeed");
	let first_history_result = query_history_result(&context, &first_id, 1).await;
	let second_history_result = query_history_result(&context, &second_id, 1).await;
	let missing_history_result = query_history_result(&context, &missing_id, 1).await;
	let first_history = exact_history_actions(db, "TestModel", "test_models", &first_id).await;
	let second_history = exact_history_actions(db, "TestModel", "test_models", &second_id).await;
	let missing_history = exact_history_actions(db, "TestModel", "test_models", &missing_id).await;

	// Assert
	assert_eq!(response.deleted, 2);
	assert_object_not_found(
		first_history_result.expect_err("deleted object history must be scoped out"),
	);
	assert_object_not_found(
		second_history_result.expect_err("deleted object history must be scoped out"),
	);
	assert_object_not_found(
		missing_history_result.expect_err("missing object history must be rejected"),
	);
	assert_eq!(first_history, ["BULK_DELETE"]);
	assert_eq!(second_history, ["BULK_DELETE"]);
	assert!(missing_history.is_empty());
}

#[rstest]
#[tokio::test]
async fn history_filters_exact_identity_and_paginates_stably(
	#[future] server_fn_context: ServerFnContext,
) {
	// Arrange
	let context = server_fn_context.await;
	let object_id = seed_record(&context, "pagination-seed").await;
	for sequence in 0..26 {
		update_name(&context, &object_id, &format!("history-value-{sequence}"))
			.await
			.expect("real update must succeed");
	}
	let other_model_id =
		insert_poison_history(&context, "OtherModel", "test_models", &object_id).await;
	let neighbor_id = insert_poison_history(
		&context,
		"TestModel",
		"test_models",
		&format!("{object_id}0"),
	)
	.await;
	let (_, db, _connection_lease) = &context;
	let mut connection = *db.connection();
	OrmExecutor::execute(
		&mut connection,
		"UPDATE reinhardt_admin_history SET occurred_at = $1",
		vec![QueryValue::String(
			"2026-08-09T01:02:03.000000Z".to_string(),
		)],
	)
	.await
	.expect("timestamps must be tied for deterministic-order test");
	let expected_ids = exact_history_ids(&context, &object_id).await;

	// Act
	let first_page = query_history(&context, &object_id, 1).await;
	let second_page = query_history(&context, &object_id, 2).await;
	let actual_ids = first_page
		.results
		.iter()
		.chain(&second_page.results)
		.map(|entry| entry.id)
		.collect::<Vec<_>>();

	// Assert
	assert_eq!(expected_ids.len(), 26);
	assert_eq!(first_page.count, 26);
	assert_eq!(first_page.total_pages, 2);
	assert_eq!(first_page.results.len(), 25);
	assert_eq!(second_page.results.len(), 1);
	assert_eq!(actual_ids, expected_ids);
	assert_eq!(
		actual_ids
			.iter()
			.copied()
			.collect::<std::collections::HashSet<_>>()
			.len(),
		26
	);
	assert!(!actual_ids.contains(&other_model_id));
	assert!(!actual_ids.contains(&neighbor_id));
	assert!(
		first_page
			.results
			.iter()
			.chain(&second_page.results)
			.all(|entry| entry.action_name == "UPDATE"
				&& entry.model_name == "TestModel"
				&& entry.object_id == object_id)
	);
}

#[rstest]
#[tokio::test]
async fn history_requires_view_permission_with_existing_event(
	#[future] server_fn_context: ServerFnContext,
) {
	// Arrange
	let context = server_fn_context.await;
	let object_id = seed_record(&context, "permission-seed").await;
	update_name(&context, &object_id, "permission-event")
		.await
		.expect("authorized update must create history");
	let (_, db, _connection_lease) = &context;
	let deny_site = AdminSite::new("Deny History Site");
	deny_site
		.register(
			"TestModel",
			DenyAllPermissionsModelAdmin::test_model("test_models"),
		)
		.expect("deny model admin must register");
	let deny_site: AdminSiteDepends = KeyedDepends::from_value(deny_site);

	// Act
	let result = get_history(
		"testmodel".to_string(),
		object_id,
		1,
		deny_site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	let error = result.expect_err("history must require model view permission");
	assert!(error.to_string().to_lowercase().contains("permission"));
}

#[rstest]
#[tokio::test]
async fn audit_insert_failure_rolls_back_update(#[future] server_fn_context: ServerFnContext) {
	// Arrange
	let context = server_fn_context.await;
	let object_id = seed_record(&context, "rollback-seed").await;
	update_name(&context, &object_id, "committed-name")
		.await
		.expect("first update must commit with history");
	let (_, db, _connection_lease) = &context;
	let mut connection = *db.connection();
	OrmExecutor::execute(
		&mut connection,
		"ALTER TABLE reinhardt_admin_history \
		 ADD CONSTRAINT history_test_reject_insert CHECK (FALSE) NOT VALID",
		Vec::new(),
	)
	.await
	.expect("history fault constraint must install");

	// Act
	let result = update_name(&context, &object_id, "rolled-back-secret").await;
	let record = db
		.get::<AdminRecord>("test_models", "id", &object_id)
		.await
		.expect("object query must succeed")
		.expect("object must remain readable");
	let history = query_history(&context, &object_id, 1).await;

	// Assert
	result.expect_err("audit insert failure must fail the mutation");
	assert_eq!(record.get("name"), Some(&json!("committed-name")));
	assert_eq!(history.count, 1);
	assert_eq!(history.results.len(), 1);
	assert_eq!(history.results[0].action_name, "UPDATE");
}

#[cfg(feature = "mysql")]
#[rstest]
#[tokio::test]
async fn mysql_audit_insert_failure_rolls_back_update() {
	use super::server_fn_helpers::{
		AdminDatabaseDepends, AllPermissionsModelAdmin, setup_admin_history_schema,
	};
	use reinhardt_admin::core::AdminDatabase;
	use reinhardt_db::backends::{
		connection::DatabaseConnection as BackendsConnection, dialect::MySqlBackend,
	};
	use reinhardt_db::orm::DatabaseConnectionLease;
	use reinhardt_test::{MySqlContainer, TestDatabase};
	use sqlx::Executor;
	use std::sync::Arc;

	// Arrange
	let container = MySqlContainer::new().await;
	container
		.wait_ready()
		.await
		.expect("the MySQL container must become ready");
	let pool = sqlx::MySqlPool::connect(&container.connection_url())
		.await
		.expect("the MySQL pool must connect");
	sqlx::raw_sql(
		"CREATE TABLE test_models (\
			id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, \
			name VARCHAR(255) NOT NULL, \
			status VARCHAR(50) NOT NULL DEFAULT 'active', \
			description TEXT, \
			created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP\
		) CHARACTER SET utf8mb4",
	)
	.execute(&pool)
	.await
	.expect("MySQL test model table must be created");
	let backend = Arc::new(MySqlBackend::new(pool.clone()));
	let owner = BackendsConnection::new(backend);
	let lease = DatabaseConnectionLease::register(owner).expect("MySQL connection must register");
	let mut history_connection = lease.handle();
	setup_admin_history_schema(&mut history_connection).await;
	let site = AdminSite::new("MySQL History Test Admin");
	site.register(
		"TestModel",
		AllPermissionsModelAdmin::editable_test_model("test_models"),
	)
	.expect("MySQL TestModel must register");
	let site: AdminSiteDepends = KeyedDepends::from_value(site);
	let db: AdminDatabaseDepends = KeyedDepends::from_value(AdminDatabase::new(lease.handle()));
	let context: ServerFnContext = (site, db, lease);
	let (site, db, _) = &context;
	let created = create_record(
		"testmodel".to_string(),
		mutation(&[
			("name", "committed-name"),
			("status", "active"),
			("description", "baseline"),
		]),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("MySQL create and history insert must succeed");
	let object_id = created
		.affected
		.expect("MySQL create must return its primary key")
		.to_string();
	assert_eq!(query_history(&context, &object_id, 1).await.count, 1);
	sqlx::raw_sql(
		"CREATE TRIGGER history_test_reject_insert \
		 BEFORE INSERT ON reinhardt_admin_history \
		 FOR EACH ROW \
		 SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'forced history insert failure'",
	)
	.execute(&pool)
	.await
	.expect("MySQL history rejection trigger must install");

	// Act
	let result = update_name(&context, &object_id, "rolled-back-secret").await;
	let record = db
		.get::<AdminRecord>("test_models", "id", &object_id)
		.await
		.expect("MySQL object must remain readable")
		.expect("MySQL object must still exist");
	let history = query_history(&context, &object_id, 1).await;

	// Assert
	result.expect_err("MySQL audit insert failure must fail the update");
	assert_eq!(record.get("name"), Some(&json!("committed-name")));
	assert_eq!(history.count, 1);
	assert_eq!(history.results.len(), 1);
	assert_eq!(history.results[0].action_name, "CREATE");
}
