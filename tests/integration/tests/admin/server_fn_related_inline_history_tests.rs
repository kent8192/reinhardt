//! Integration tests for atomic parent and related-inline history persistence.

use super::server_fn_helpers::{
	AdminDatabaseDepends, AdminSiteDepends, TEST_CSRF_TOKEN, make_auth_user, make_staff_request,
	setup_admin_history_schema,
};
use reinhardt_admin::core::{AdminDatabase, AdminSite, AdminUser, InlineModelAdmin, ModelAdmin};
use reinhardt_admin::server::{create_record, get_history, update_record};
use reinhardt_admin::types::{AdminHistoryEntry, HistoryResponse, MutationRequest};
use reinhardt_db::associations::ForeignKeyField;
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::orm::DatabaseConnectionLease;
use reinhardt_di::KeyedDepends;
use reinhardt_macros::model;
use reinhardt_pages::server_fn::{ServerFnError, ServerFnErrorKind};
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

const PARENT_MODEL: &str = "RelatedInlineParent";
const CHILD_MODEL: &str = "RelatedInlineChild";
const CHILD_INLINE_IDENTITY: &str = "Line Item";
const PARENT_TABLE: &str = "related_inline_history_parents";
const CHILD_TABLE: &str = "related_inline_history_children";
const INLINE_KEY: &str = "related_inline_history_children-parent_id";

#[model(
	app_label = "admin_related_inline_history",
	table_name = "related_inline_history_parents",
	form = true,
	info = false
)]
#[derive(Clone, Deserialize, Serialize)]
struct RelatedInlineParent {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 100)]
	name: String,
}

#[model(
	app_label = "admin_related_inline_history",
	table_name = "related_inline_history_children",
	form = true,
	info = false
)]
#[derive(Clone, Deserialize, Serialize)]
struct RelatedInlineChild {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[rel(foreign_key, related_name = "children")]
	parent: ForeignKeyField<RelatedInlineParent>,
	#[field(max_length = 100)]
	name: String,
	position: i64,
}

#[derive(Clone, Copy)]
enum RelatedInlineAdminKind {
	Parent,
	Child,
}

struct RelatedInlineModelAdmin {
	kind: RelatedInlineAdminKind,
}

impl RelatedInlineModelAdmin {
	fn parent() -> Self {
		Self {
			kind: RelatedInlineAdminKind::Parent,
		}
	}

	fn child() -> Self {
		Self {
			kind: RelatedInlineAdminKind::Child,
		}
	}
}

#[async_trait::async_trait]
impl ModelAdmin for RelatedInlineModelAdmin {
	fn model_name(&self) -> &str {
		match self.kind {
			RelatedInlineAdminKind::Parent => PARENT_MODEL,
			RelatedInlineAdminKind::Child => CHILD_MODEL,
		}
	}

	fn table_name(&self) -> &str {
		match self.kind {
			RelatedInlineAdminKind::Parent => PARENT_TABLE,
			RelatedInlineAdminKind::Child => CHILD_TABLE,
		}
	}

	fn fields(&self) -> Option<Vec<&str>> {
		match self.kind {
			RelatedInlineAdminKind::Parent => Some(vec!["id", "name"]),
			RelatedInlineAdminKind::Child => Some(vec!["id", "parent_id", "name", "position"]),
		}
	}

	fn inlines(&self) -> Vec<InlineModelAdmin> {
		match self.kind {
			RelatedInlineAdminKind::Parent => vec![
				InlineModelAdmin::new::<RelatedInlineParent, RelatedInlineChild>(
					CHILD_INLINE_IDENTITY,
					"parent_id",
					&["name", "position"],
				)
				.expect("related inline configuration must be valid")
				.can_delete(true),
			],
			RelatedInlineAdminKind::Child => Vec::new(),
		}
	}

	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_delete_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}
}

type RelatedInlineContext = (
	AdminSiteDepends,
	AdminDatabaseDepends,
	sqlx::PgPool,
	DatabaseConnectionLease,
);

#[fixture]
async fn related_inline_context(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> RelatedInlineContext {
	let (pool, _) = shared_db_pool.await;
	sqlx::raw_sql(
		"CREATE TABLE related_inline_history_parents (\
			id BIGSERIAL PRIMARY KEY, \
			name VARCHAR(100) NOT NULL\
		); \
		CREATE TABLE related_inline_history_children (\
			id BIGSERIAL PRIMARY KEY, \
			parent_id BIGINT NOT NULL REFERENCES related_inline_history_parents(id), \
			name VARCHAR(100) NOT NULL, \
			position BIGINT NOT NULL\
		)",
	)
	.execute(&pool)
	.await
	.expect("related inline history tables must be created");

	let backend = Arc::new(PostgresBackend::new(pool.clone()));
	let connection = BackendsConnection::new(backend);
	let lease = DatabaseConnectionLease::register(connection)
		.expect("related inline database connection must register");
	let mut history_connection = lease.handle();
	setup_admin_history_schema(&mut history_connection).await;
	let db = AdminDatabase::new(lease.handle());
	let site = AdminSite::new("Related Inline History Test Admin");
	site.register(PARENT_MODEL, RelatedInlineModelAdmin::parent())
		.expect("related inline parent admin must register");
	site.register(CHILD_MODEL, RelatedInlineModelAdmin::child())
		.expect("related inline child admin must register");

	(
		KeyedDepends::from_value(site),
		KeyedDepends::from_value(db),
		pool,
		lease,
	)
}

fn mutation(data: HashMap<String, Value>) -> MutationRequest {
	MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_string(),
		data,
	}
}

fn inline_control(index: usize, field: &str) -> String {
	format!("__reinhardt_inlines.{INLINE_KEY}.{index}.{field}")
}

fn parent_with_children_request(parent_name: &str, children: &[(&str, i64)]) -> MutationRequest {
	let mut data = HashMap::from([("name".to_string(), json!(parent_name))]);
	for (index, (name, position)) in children.iter().enumerate() {
		data.insert(inline_control(index, "name"), json!(name));
		data.insert(inline_control(index, "position"), json!(position));
	}
	mutation(data)
}

async fn create_parent_with_children(
	context: &RelatedInlineContext,
	parent_name: &str,
	children: &[(&str, i64)],
) -> (String, Vec<(i64, String, i64)>) {
	let (site, db, pool, _) = context;
	let response = create_record(
		PARENT_MODEL.to_lowercase(),
		parent_with_children_request(parent_name, children),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("parent and related children must be created");
	let parent_id = response
		.affected
		.expect("created parent identity must be returned")
		.to_string();
	let rows = sqlx::query_as::<_, (i64, String, i64)>(
		"SELECT id, name, position FROM related_inline_history_children \
		 WHERE parent_id = $1 ORDER BY id",
	)
	.bind(
		parent_id
			.parse::<i64>()
			.expect("parent identity must be i64"),
	)
	.fetch_all(pool)
	.await
	.expect("created related children must be queryable");
	(parent_id, rows)
}

async fn object_history_result(
	context: &RelatedInlineContext,
	model_name: &str,
	object_id: &str,
) -> Result<HistoryResponse, ServerFnError> {
	let (site, db, _, _) = context;
	get_history(
		model_name.to_lowercase(),
		object_id.to_string(),
		1,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
}

async fn object_history(
	context: &RelatedInlineContext,
	model_name: &str,
	object_id: &str,
) -> HistoryResponse {
	object_history_result(context, model_name, object_id)
		.await
		.expect("related object history must be queryable")
}

fn assert_object_not_found(error: ServerFnError) {
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(404));
	assert_eq!(error.user_message(), "Object not found");
}

async fn history_actions(pool: &sqlx::PgPool, model_name: &str, object_id: &str) -> Vec<String> {
	sqlx::query_scalar(
		"SELECT action_name FROM reinhardt_admin_history \
		 WHERE model_name = $1 AND object_id = $2 ORDER BY id DESC",
	)
	.bind(model_name)
	.bind(object_id)
	.fetch_all(pool)
	.await
	.expect("history actions must be independently queryable")
}

fn assert_history(
	response: &HistoryResponse,
	model_name: &str,
	object_id: &str,
	expected: &[(&str, &[&str])],
) {
	assert_eq!(response.model_name, model_name);
	assert_eq!(response.object_id, object_id);
	assert_eq!(response.count, expected.len() as u64);
	assert_eq!(response.results.len(), expected.len());
	assert_eq!(response.page, 1);
	assert_eq!(response.page_size, 25);
	assert_eq!(response.total_pages, 1);
	assert_eq!(
		response
			.results
			.iter()
			.map(|event| (
				event.action_name.as_str(),
				event
					.changed_fields
					.iter()
					.map(String::as_str)
					.collect::<Vec<_>>(),
			))
			.collect::<Vec<_>>(),
		expected
			.iter()
			.map(|(action, fields)| (*action, fields.to_vec()))
			.collect::<Vec<_>>()
	);
	for event in &response.results {
		assert_history_identity(event, model_name, object_id);
	}
}

fn assert_history_identity(event: &AdminHistoryEntry, model_name: &str, object_id: &str) {
	assert_eq!(event.actor, "test_staff");
	assert_eq!(event.model_name, model_name);
	assert_eq!(event.object_id, object_id);
	assert_eq!(event.object_repr, format!("{model_name} ({object_id})"));
	assert_eq!(event.affected_count, 1);
	assert!(event.success);
}

#[rstest]
#[tokio::test]
async fn related_inline_create_then_update_writes_canonical_per_object_history(
	#[future] related_inline_context: RelatedInlineContext,
) {
	// Arrange
	let context = related_inline_context.await;

	// Act
	let (parent_id, initial_children) = create_parent_with_children(
		&context,
		"parent-before",
		&[("first-before", 1), ("second-before", 2)],
	)
	.await;
	let first_id = initial_children[0].0.to_string();
	let deleted_id = initial_children[1].0.to_string();
	let mut update_data = HashMap::from([("name".to_string(), json!("parent-after"))]);
	update_data.insert(inline_control(0, "__id"), json!(first_id.clone()));
	update_data.insert(inline_control(0, "name"), json!("first-after"));
	update_data.insert(inline_control(0, "position"), json!(10));
	update_data.insert(inline_control(1, "__id"), json!(deleted_id.clone()));
	update_data.insert(inline_control(1, "__delete"), json!(true));
	update_data.insert(inline_control(2, "name"), json!("third-created"));
	update_data.insert(inline_control(2, "position"), json!(3));
	let (site, db, pool, _) = &context;
	let update = update_record(
		PARENT_MODEL.to_lowercase(),
		parent_id.clone(),
		mutation(update_data),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("parent and related inline update must commit");
	let parent_name: String =
		sqlx::query_scalar("SELECT name FROM related_inline_history_parents WHERE id = $1")
			.bind(
				parent_id
					.parse::<i64>()
					.expect("parent identity must be i64"),
			)
			.fetch_one(pool)
			.await
			.expect("updated parent must be queryable");
	let children = sqlx::query_as::<_, (i64, String, i64)>(
		"SELECT id, name, position FROM related_inline_history_children \
		 WHERE parent_id = $1 ORDER BY id",
	)
	.bind(
		parent_id
			.parse::<i64>()
			.expect("parent identity must be i64"),
	)
	.fetch_all(pool)
	.await
	.expect("updated related children must be queryable");
	let created_id = children
		.iter()
		.find(|(_, name, _)| name == "third-created")
		.map(|(id, _, _)| id.to_string())
		.expect("new related child identity must be returned by the database");
	let parent_history = object_history(&context, PARENT_MODEL, &parent_id).await;
	let updated_history = object_history(&context, CHILD_MODEL, &first_id).await;
	let deleted_history_result = object_history_result(&context, CHILD_MODEL, &deleted_id).await;
	let deleted_history = history_actions(pool, CHILD_MODEL, &deleted_id).await;
	let created_history = object_history(&context, CHILD_MODEL, &created_id).await;
	let deleted_history_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM reinhardt_admin_history WHERE model_name = $1 AND object_id = $2",
	)
	.bind(CHILD_MODEL)
	.bind(&deleted_id)
	.fetch_one(pool)
	.await
	.expect("deleted child history must remain persisted");

	// Assert
	assert_eq!(update.affected, Some(1));
	assert_eq!(
		initial_children
			.iter()
			.map(|(_, name, position)| (name.as_str(), *position))
			.collect::<Vec<_>>(),
		[("first-before", 1), ("second-before", 2)]
	);
	assert_eq!(parent_name, "parent-after");
	assert_eq!(
		children,
		vec![
			(
				first_id.parse::<i64>().expect("child identity must be i64"),
				"first-after".to_string(),
				10,
			),
			(
				created_id
					.parse::<i64>()
					.expect("child identity must be i64"),
				"third-created".to_string(),
				3,
			),
		]
	);
	assert_history(
		&parent_history,
		PARENT_MODEL,
		&parent_id,
		&[("UPDATE", &["name"]), ("CREATE", &["name"])],
	);
	assert_history(
		&updated_history,
		CHILD_MODEL,
		&first_id,
		&[
			("UPDATE", &["name", "position"]),
			("CREATE", &["name", "position"]),
		],
	);
	assert_object_not_found(
		deleted_history_result.expect_err("deleted child history must be scoped out"),
	);
	assert_eq!(deleted_history, ["DELETE", "CREATE"]);
	assert_eq!(deleted_history_count, 2);
	assert_history(
		&created_history,
		CHILD_MODEL,
		&created_id,
		&[("CREATE", &["name", "position"])],
	);
}

#[rstest]
#[tokio::test]
async fn child_history_failure_rolls_back_parent_child_and_request_histories(
	#[future] related_inline_context: RelatedInlineContext,
) {
	// Arrange
	let context = related_inline_context.await;
	let (parent_id, children) =
		create_parent_with_children(&context, "parent-baseline", &[("child-baseline", 1)]).await;
	let child_id = children[0].0.to_string();
	let (site, db, pool, _) = &context;
	sqlx::query(
		"ALTER TABLE reinhardt_admin_history \
		 ADD CONSTRAINT related_inline_child_history_reject \
		 CHECK (model_name <> 'RelatedInlineChild') NOT VALID",
	)
	.execute(pool)
	.await
	.expect("child-only history rejection constraint must install");
	let mut update_data = HashMap::from([("name".to_string(), json!("parent-rolled-back"))]);
	update_data.insert(inline_control(0, "__id"), json!(child_id.clone()));
	update_data.insert(inline_control(0, "name"), json!("child-rolled-back"));
	update_data.insert(inline_control(0, "position"), json!(99));

	// Act
	let result = update_record(
		PARENT_MODEL.to_lowercase(),
		parent_id.clone(),
		mutation(update_data),
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await;
	let parent_name: String =
		sqlx::query_scalar("SELECT name FROM related_inline_history_parents WHERE id = $1")
			.bind(
				parent_id
					.parse::<i64>()
					.expect("parent identity must be i64"),
			)
			.fetch_one(pool)
			.await
			.expect("rolled-back parent must remain queryable");
	let child: (String, i64) =
		sqlx::query_as("SELECT name, position FROM related_inline_history_children WHERE id = $1")
			.bind(child_id.parse::<i64>().expect("child identity must be i64"))
			.fetch_one(pool)
			.await
			.expect("rolled-back child must remain queryable");
	let total_history: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reinhardt_admin_history")
		.fetch_one(pool)
		.await
		.expect("history count must be queryable");
	let parent_history = object_history(&context, PARENT_MODEL, &parent_id).await;
	let child_history = object_history(&context, CHILD_MODEL, &child_id).await;

	// Assert
	result.expect_err("child history failure must reject the combined request");
	assert_eq!(parent_name, "parent-baseline");
	assert_eq!(child, ("child-baseline".to_string(), 1));
	assert_eq!(total_history, 2);
	assert_history(
		&parent_history,
		PARENT_MODEL,
		&parent_id,
		&[("CREATE", &["name"])],
	);
	assert_history(
		&child_history,
		CHILD_MODEL,
		&child_id,
		&[("CREATE", &["name", "position"])],
	);
}
