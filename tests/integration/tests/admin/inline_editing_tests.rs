//! Real SQLite integration tests for transactional inline editing.

#![cfg(feature = "sqlite")]

use super::server_fn_helpers::{TEST_CSRF_TOKEN, make_auth_user, make_staff_request};
use reinhardt::model;
use reinhardt_admin::adapters::MutationRequest;
use reinhardt_admin::core::{
	AdminDatabase, AdminDatabaseKey, AdminSite, AdminSiteKey, InlineModelAdmin, ModelAdminConfig,
	initialize_admin_history_schema,
};
use reinhardt_admin::server::{create_record, update_record};
use reinhardt_db::associations::ForeignKeyField;
use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
use reinhardt_db::orm::{DatabaseConnection, DatabaseConnectionLease, QueryValue};
use reinhardt_di::KeyedDepends;
use reinhardt_pages::server_fn::ServerFnErrorKind;
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

const PARENT_MODEL: &str = "InlineEditingParent";
const CHILD_MODEL: &str = "InlineEditingChild";
const PARENT_TABLE: &str = "issue_5991_inline_parents";
const CHILD_TABLE: &str = "issue_5991_inline_children";
const INLINE_KEY: &str = "issue_5991_inline_children-parent_id";
const DUPLICATE_CODE: &str = "duplicate";

#[model(
	app_label = "admin_inline_editing",
	table_name = "issue_5991_inline_parents",
	form = true,
	info = false
)]
#[derive(Clone, Deserialize, Serialize)]
struct InlineEditingParent {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 100)]
	name: String,
}

#[model(
	app_label = "admin_inline_editing",
	table_name = "issue_5991_inline_children",
	form = true,
	info = false
)]
#[derive(Clone, Deserialize, Serialize)]
struct InlineEditingChild {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[rel(foreign_key, related_name = "inline_editing_children")]
	parent: ForeignKeyField<InlineEditingParent>,
	#[field(max_length = 100, unique = true)]
	code: String,
}

type AdminSiteDepends = KeyedDepends<AdminSiteKey, AdminSite>;
type AdminDatabaseDepends = KeyedDepends<AdminDatabaseKey, AdminDatabase>;
type InlineTestContext = (
	AdminSiteDepends,
	AdminDatabaseDepends,
	DatabaseConnectionLease,
	DatabaseConnection,
);

async fn inline_test_context() -> InlineTestContext {
	let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("in-memory SQLite connection should initialize");
	let lease = DatabaseConnectionLease::register(owner)
		.expect("SQLite connection should remain registered for the test lifetime");
	let mut connection = lease.handle();
	initialize_admin_history_schema(&mut connection)
		.await
		.expect("history schema should be initialized");
	connection
		.execute("PRAGMA foreign_keys = ON", Vec::new())
		.await
		.expect("SQLite foreign key enforcement should be enabled");
	connection
		.execute(
			"CREATE TABLE issue_5991_inline_parents (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				name TEXT NOT NULL
			)",
			Vec::new(),
		)
		.await
		.expect("inline parent table should be created");
	connection
		.execute(
			"CREATE TABLE issue_5991_inline_children (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				parent_id INTEGER NOT NULL,
				code TEXT NOT NULL UNIQUE,
				FOREIGN KEY (parent_id) REFERENCES issue_5991_inline_parents(id)
			)",
			Vec::new(),
		)
		.await
		.expect("inline child table should be created");

	let inline = InlineModelAdmin::new::<InlineEditingParent, InlineEditingChild>(
		CHILD_MODEL,
		"parent_id",
		&["code"],
	)
	.expect("generated parent-child metadata should define the inline relation");
	let parent_admin = ModelAdminConfig::builder()
		.model_name(PARENT_MODEL)
		.table_name(PARENT_TABLE)
		.fields(vec!["name"])
		.inlines(vec![inline])
		.allow_all(true)
		.build()
		.expect("parent admin configuration should be valid");
	let child_admin = ModelAdminConfig::builder()
		.model_name(CHILD_MODEL)
		.table_name(CHILD_TABLE)
		.fields(vec!["code"])
		.allow_all(true)
		.build()
		.expect("child admin configuration should be valid");
	let site = AdminSite::new("Inline Editing Test Admin");
	site.register(PARENT_MODEL, parent_admin)
		.expect("parent admin should register");
	site.register(CHILD_MODEL, child_admin)
		.expect("child admin should register");
	let database = AdminDatabase::new(connection);

	(
		KeyedDepends::from_value(site),
		KeyedDepends::from_value(database),
		lease,
		connection,
	)
}

async fn seed_parent(connection: &DatabaseConnection, name: &str) {
	connection
		.execute(
			"INSERT INTO issue_5991_inline_parents (id, name) VALUES (1, ?)",
			vec![QueryValue::String(name.to_owned())],
		)
		.await
		.expect("seed parent should be inserted");
}

async fn seed_duplicate_child(connection: &DatabaseConnection) {
	connection
		.execute(
			"INSERT INTO issue_5991_inline_children (id, parent_id, code) VALUES (1, 1, ?)",
			vec![QueryValue::String(DUPLICATE_CODE.to_owned())],
		)
		.await
		.expect("seed child should be inserted");
}

fn mutation_request(parent_name: &str, child_codes: &[&str]) -> MutationRequest {
	let mut data = HashMap::from([("name".to_owned(), json!(parent_name))]);
	for (index, code) in child_codes.iter().enumerate() {
		data.insert(
			format!("__reinhardt_inlines.{INLINE_KEY}.{index}.code"),
			json!(code),
		);
	}
	MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_owned(),
		data,
	}
}

async fn query_rows(connection: &DatabaseConnection, sql: &str) -> Vec<Value> {
	connection
		.query(sql, Vec::new())
		.await
		.expect("SQLite verification query should succeed")
		.into_iter()
		.map(|row| row.data)
		.collect()
}

#[rstest]
#[tokio::test]
async fn inline_create_rolls_back_parent_and_earlier_child_when_later_child_fails() {
	// Arrange
	let (site, database, _lease, connection) = inline_test_context().await;
	seed_parent(&connection, "seed parent").await;
	seed_duplicate_child(&connection).await;
	let request = mutation_request("new parent", &["first child", DUPLICATE_CODE]);

	// Act
	let result = create_record(
		PARENT_MODEL.to_owned(),
		request,
		site,
		database,
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	let error = result.expect_err("the duplicate inline child should fail the request");
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(500));
	assert_eq!(error.user_message(), "Inline persistence failed");
	assert_eq!(
		query_rows(
			&connection,
			"SELECT id, name FROM issue_5991_inline_parents ORDER BY id",
		)
		.await,
		vec![json!({"id": 1, "name": "seed parent"})],
	);
	assert_eq!(
		query_rows(
			&connection,
			"SELECT id, parent_id, code FROM issue_5991_inline_children ORDER BY id",
		)
		.await,
		vec![json!({"id": 1, "parent_id": 1, "code": DUPLICATE_CODE})],
	);
}

#[rstest]
#[tokio::test]
async fn inline_update_rolls_back_parent_and_earlier_child_when_later_child_fails() {
	// Arrange
	let (site, database, _lease, connection) = inline_test_context().await;
	seed_parent(&connection, "before").await;
	seed_duplicate_child(&connection).await;
	let request = mutation_request("after", &["first child", DUPLICATE_CODE]);

	// Act
	let result = update_record(
		PARENT_MODEL.to_owned(),
		"1".to_owned(),
		request,
		site,
		database,
		make_staff_request(),
		make_auth_user(),
	)
	.await;

	// Assert
	let error = result.expect_err("the duplicate inline child should fail the request");
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(500));
	assert_eq!(error.user_message(), "Inline persistence failed");
	assert_eq!(
		query_rows(
			&connection,
			"SELECT id, name FROM issue_5991_inline_parents ORDER BY id",
		)
		.await,
		vec![json!({"id": 1, "name": "before"})],
	);
	assert_eq!(
		query_rows(
			&connection,
			"SELECT id, parent_id, code FROM issue_5991_inline_children ORDER BY id",
		)
		.await,
		vec![json!({"id": 1, "parent_id": 1, "code": DUPLICATE_CODE})],
	);
}

#[rstest]
#[tokio::test]
async fn inline_create_uses_returned_parent_identity_for_children_and_legacy_response() {
	// Arrange
	let (site, database, _lease, connection) = inline_test_context().await;
	seed_parent(&connection, "seed parent").await;
	let request = mutation_request("new parent", &["created child"]);

	// Act
	let response = create_record(
		PARENT_MODEL.to_owned(),
		request,
		site,
		database,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("parent and inline child should be created atomically");

	// Assert
	assert_eq!(response.success, true);
	assert_eq!(response.affected, Some(2));
	assert_eq!(
		query_rows(
			&connection,
			"SELECT id, name FROM issue_5991_inline_parents ORDER BY id",
		)
		.await,
		vec![
			json!({"id": 1, "name": "seed parent"}),
			json!({"id": 2, "name": "new parent"}),
		],
	);
	assert_eq!(
		query_rows(
			&connection,
			"SELECT id, parent_id, code FROM issue_5991_inline_children ORDER BY id",
		)
		.await,
		vec![json!({"id": 1, "parent_id": 2, "code": "created child"})],
	);
}
