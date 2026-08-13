use chrono::{DateTime, SecondsFormat, Utc};
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error, Result};
use reinhardt_db::orm::execution::convert_values;
use reinhardt_db::orm::{DatabaseBackend, DatabaseConnection, OrmExecutor, QueryValue, Row};
use reinhardt_query::prelude::{
	Alias, Expr, ExprTrait, Func, MySqlQueryBuilder, Order, PostgresQueryBuilder, Query,
	QueryStatementBuilder, SqliteQueryBuilder, Value,
};

static HISTORY_SCHEMA_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const POSTGRES_SCHEMA: &[&str] = &[
	"CREATE TABLE IF NOT EXISTS reinhardt_admin_history (\
		id BIGSERIAL PRIMARY KEY, \
		occurred_at VARCHAR(35) NOT NULL, \
		actor TEXT NOT NULL, \
		action_name TEXT NOT NULL, \
		model_name TEXT NOT NULL, \
		model_identity BYTEA NOT NULL, \
		object_id TEXT NOT NULL, \
		object_identity BYTEA NOT NULL, \
		object_repr TEXT NOT NULL, \
		changed_fields TEXT NOT NULL, \
		affected_count BIGINT NOT NULL, \
		success BOOLEAN NOT NULL\
	)",
	"CREATE INDEX IF NOT EXISTS reinhardt_admin_history_object_idx \
		ON reinhardt_admin_history (model_identity, object_identity, occurred_at DESC, id DESC)",
];

const MYSQL_SCHEMA: &[&str] = &["CREATE TABLE IF NOT EXISTS reinhardt_admin_history (\
		id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY, \
		occurred_at VARCHAR(35) NOT NULL, \
		actor TEXT NOT NULL, \
		action_name TEXT NOT NULL, \
		model_name TEXT NOT NULL, \
		model_identity BLOB NOT NULL, \
		object_id TEXT NOT NULL, \
		object_identity BLOB NOT NULL, \
		object_repr TEXT NOT NULL, \
		changed_fields TEXT NOT NULL, \
		affected_count BIGINT UNSIGNED NOT NULL, \
		success BOOLEAN NOT NULL, \
		INDEX reinhardt_admin_history_object_idx \
			(model_identity(255) ASC, object_identity(255) ASC, occurred_at ASC, id ASC)\
	) ENGINE=InnoDB CHARACTER SET utf8mb4"];

const SQLITE_SCHEMA: &[&str] = &[
	"CREATE TABLE IF NOT EXISTS reinhardt_admin_history (\
		id INTEGER PRIMARY KEY AUTOINCREMENT, \
		occurred_at VARCHAR(35) NOT NULL, \
		actor TEXT NOT NULL, \
		action_name TEXT NOT NULL, \
		model_name TEXT NOT NULL, \
		model_identity BLOB NOT NULL, \
		object_id TEXT NOT NULL, \
		object_identity BLOB NOT NULL, \
		object_repr TEXT NOT NULL, \
		changed_fields TEXT NOT NULL, \
		affected_count INTEGER NOT NULL, \
		success BOOLEAN NOT NULL\
	)",
	"CREATE INDEX IF NOT EXISTS reinhardt_admin_history_object_idx \
		ON reinhardt_admin_history (model_identity, object_identity, occurred_at DESC, id DESC)",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewHistoryEvent {
	pub(crate) occurred_at: DateTime<Utc>,
	pub(crate) actor: String,
	pub(crate) action_name: String,
	pub(crate) model_name: String,
	pub(crate) table_name: String,
	pub(crate) object_id: String,
	pub(crate) object_repr: String,
	pub(crate) changed_fields: Vec<String>,
	pub(crate) affected_count: u64,
	pub(crate) success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredHistoryEvent {
	pub(crate) id: i64,
	pub(crate) occurred_at: DateTime<Utc>,
	pub(crate) actor: String,
	pub(crate) action_name: String,
	pub(crate) model_name: String,
	pub(crate) object_id: String,
	pub(crate) object_repr: String,
	pub(crate) changed_fields: Vec<String>,
	pub(crate) affected_count: u64,
	pub(crate) success: bool,
}

fn history_schema_statements(backend: DatabaseBackend) -> &'static [&'static str] {
	match backend {
		DatabaseBackend::Postgres => POSTGRES_SCHEMA,
		DatabaseBackend::MySql => MYSQL_SCHEMA,
		DatabaseBackend::Sqlite => SQLITE_SCHEMA,
	}
}

fn serialization_error(error: impl std::fmt::Display) -> Error {
	DatabaseError::new(DatabaseErrorKind::Serialization, error.to_string()).into()
}

fn model_identity(model_name: &str, table_name: &str) -> Vec<u8> {
	let mut identity = Vec::with_capacity(16 + model_name.len() + table_name.len());
	identity.extend_from_slice(&(model_name.len() as u64).to_be_bytes());
	identity.extend_from_slice(model_name.as_bytes());
	identity.extend_from_slice(&(table_name.len() as u64).to_be_bytes());
	identity.extend_from_slice(table_name.as_bytes());
	identity
}

fn insert_history_values(event: &NewHistoryEvent) -> Result<Vec<Value>> {
	let mut changed_fields = event.changed_fields.clone();
	changed_fields.sort_unstable();
	changed_fields.dedup();
	let changed_fields = serde_json::to_string(&changed_fields).map_err(serialization_error)?;
	let affected_count = i64::try_from(event.affected_count).map_err(serialization_error)?;

	Ok(vec![
		Value::String(Some(Box::new(
			event
				.occurred_at
				.to_rfc3339_opts(SecondsFormat::Micros, true),
		))),
		Value::String(Some(Box::new(event.actor.clone()))),
		Value::String(Some(Box::new(event.action_name.clone()))),
		Value::String(Some(Box::new(event.model_name.clone()))),
		Value::Bytes(Some(Box::new(model_identity(
			&event.model_name,
			&event.table_name,
		)))),
		Value::String(Some(Box::new(event.object_id.clone()))),
		Value::Bytes(Some(Box::new(event.object_id.as_bytes().to_vec()))),
		Value::String(Some(Box::new(event.object_repr.clone()))),
		Value::String(Some(Box::new(changed_fields))),
		Value::BigInt(Some(affected_count)),
		Value::Bool(Some(event.success)),
	])
}

fn build_insert_history_query(
	backend: DatabaseBackend,
	event: &NewHistoryEvent,
) -> Result<(String, Vec<QueryValue>)> {
	let mut query = Query::insert()
		.into_table(Alias::new("reinhardt_admin_history"))
		.columns([
			"occurred_at",
			"actor",
			"action_name",
			"model_name",
			"model_identity",
			"object_id",
			"object_identity",
			"object_repr",
			"changed_fields",
			"affected_count",
			"success",
		])
		.to_owned();
	query
		.values(insert_history_values(event)?)
		.map_err(serialization_error)?;
	let (sql, values) = match backend {
		DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
		DatabaseBackend::MySql => query.build(MySqlQueryBuilder),
		DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
	};
	Ok((sql, convert_values(values)))
}

fn build_list_object_history_query(
	backend: DatabaseBackend,
	model_name: &str,
	table_name: &str,
	object_id: &str,
	offset: u64,
	limit: u64,
) -> Result<(String, Vec<QueryValue>)> {
	let query = Query::select()
		.columns([
			"id",
			"occurred_at",
			"actor",
			"action_name",
			"model_name",
			"object_id",
			"object_repr",
			"changed_fields",
			"affected_count",
			"success",
		])
		.from(Alias::new("reinhardt_admin_history"))
		.and_where(
			Expr::col(Alias::new("model_identity")).eq(Value::Bytes(Some(Box::new(
				model_identity(model_name, table_name),
			)))),
		)
		.and_where(
			Expr::col(Alias::new("object_identity"))
				.eq(Value::Bytes(Some(Box::new(object_id.as_bytes().to_vec())))),
		)
		.order_by(Alias::new("occurred_at"), Order::Desc)
		.order_by(Alias::new("id"), Order::Desc)
		.limit(i64::try_from(limit).map_err(serialization_error)?)
		.offset(i64::try_from(offset).map_err(serialization_error)?)
		.to_owned();
	let (sql, values) = match backend {
		DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
		DatabaseBackend::MySql => query.build(MySqlQueryBuilder),
		DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
	};
	Ok((sql, convert_values(values)))
}

fn build_count_object_history_query(
	backend: DatabaseBackend,
	model_name: &str,
	table_name: &str,
	object_id: &str,
) -> (String, Vec<QueryValue>) {
	let query = Query::select()
		.expr_as(
			Func::count(Expr::asterisk().into_simple_expr()),
			Alias::new("count"),
		)
		.from(Alias::new("reinhardt_admin_history"))
		.and_where(
			Expr::col(Alias::new("model_identity")).eq(Value::Bytes(Some(Box::new(
				model_identity(model_name, table_name),
			)))),
		)
		.and_where(
			Expr::col(Alias::new("object_identity"))
				.eq(Value::Bytes(Some(Box::new(object_id.as_bytes().to_vec())))),
		)
		.to_owned();
	let (sql, values) = match backend {
		DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
		DatabaseBackend::MySql => query.build(MySqlQueryBuilder),
		DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
	};
	(sql, convert_values(values))
}

fn stored_history_event_from_row(row: Row) -> Result<StoredHistoryEvent> {
	let occurred_at: String = row.get("occurred_at")?;
	let changed_fields: String = row.get("changed_fields")?;
	let affected_count: i64 = row.get("affected_count")?;

	Ok(StoredHistoryEvent {
		id: row.get("id")?,
		occurred_at: DateTime::parse_from_rfc3339(&occurred_at)
			.map_err(serialization_error)?
			.with_timezone(&Utc),
		actor: row.get("actor")?,
		action_name: row.get("action_name")?,
		model_name: row.get("model_name")?,
		object_id: row.get("object_id")?,
		object_repr: row.get("object_repr")?,
		changed_fields: serde_json::from_str(&changed_fields).map_err(serialization_error)?,
		affected_count: u64::try_from(affected_count).map_err(serialization_error)?,
		success: row.get("success")?,
	})
}

/// Initializes the admin history table and index during application setup.
///
/// Call this before serving admin requests, or provision the same schema from
/// an application migration. Request handlers only insert and read history.
pub async fn initialize_admin_history_schema(connection: &mut DatabaseConnection) -> Result<()> {
	// MySQL may implicitly commit DDL, so schema initialization must stay outside
	// mutation transactions.
	let _lock = HISTORY_SCHEMA_LOCK.lock().await;
	for statement in history_schema_statements(connection.backend()) {
		connection.execute(statement, Vec::new()).await?;
	}
	Ok(())
}

pub(crate) async fn insert_history_event<E>(executor: &mut E, event: &NewHistoryEvent) -> Result<()>
where
	E: OrmExecutor + ?Sized,
{
	let (sql, params) = build_insert_history_query(executor.backend(), event)?;
	executor.execute(&sql, params).await?;
	Ok(())
}

pub(crate) async fn list_object_history<E>(
	executor: &mut E,
	model_name: &str,
	table_name: &str,
	object_id: &str,
	offset: u64,
	limit: u64,
) -> Result<Vec<StoredHistoryEvent>>
where
	E: OrmExecutor + ?Sized,
{
	let (sql, params) = build_list_object_history_query(
		executor.backend(),
		model_name,
		table_name,
		object_id,
		offset,
		limit,
	)?;
	let rows = executor.fetch_all(&sql, params).await?;
	rows.into_iter()
		.map(stored_history_event_from_row)
		.collect()
}

pub(crate) async fn count_object_history<E>(
	executor: &mut E,
	model_name: &str,
	table_name: &str,
	object_id: &str,
) -> Result<u64>
where
	E: OrmExecutor + ?Sized,
{
	let (sql, params) =
		build_count_object_history_query(executor.backend(), model_name, table_name, object_id);
	let row = executor.fetch_one(&sql, params).await?;
	let count: i64 = row.get("count")?;
	u64::try_from(count).map_err(serialization_error)
}

#[cfg(test)]
mod tests {
	use chrono::{TimeZone, Utc};
	use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
	use reinhardt_db::backends::types::{QueryValue, Row};
	use reinhardt_db::orm::{DatabaseBackend, DatabaseConnectionLease};
	use rstest::rstest;

	use super::*;

	#[rstest]
	#[case::postgres(DatabaseBackend::Postgres, 2, "BIGSERIAL PRIMARY KEY")]
	#[case::mysql(DatabaseBackend::MySql, 1, "AUTO_INCREMENT PRIMARY KEY")]
	#[case::sqlite(DatabaseBackend::Sqlite, 2, "INTEGER PRIMARY KEY AUTOINCREMENT")]
	fn history_schema_is_backend_aware_and_keeps_deleted_objects(
		#[case] backend: DatabaseBackend,
		#[case] expected_statement_count: usize,
		#[case] expected_identity: &str,
	) {
		// Act
		let statements = history_schema_statements(backend);
		let schema = statements.join("\n");

		// Assert
		assert_eq!(statements.len(), expected_statement_count);
		assert!(schema.contains("CREATE TABLE IF NOT EXISTS reinhardt_admin_history"));
		assert!(schema.contains(expected_identity));
		assert!(schema.contains("reinhardt_admin_history_object_idx"));
		assert!(schema.contains("model_identity"));
		assert!(schema.contains("object_identity"));
		assert!(schema.contains("object_id TEXT NOT NULL"));
		assert!(schema.contains("actor TEXT NOT NULL"));
		assert!(schema.contains("action_name TEXT NOT NULL"));
		assert!(schema.contains("model_name TEXT NOT NULL"));
		assert!(!schema.contains("VARCHAR(255)"));
		match backend {
			DatabaseBackend::Postgres => {
				assert!(schema.contains("model_identity BYTEA NOT NULL"));
				assert!(schema.contains("object_identity BYTEA NOT NULL"));
			}
			DatabaseBackend::MySql => {
				assert!(schema.contains("model_identity BLOB NOT NULL"));
				assert!(schema.contains("object_identity BLOB NOT NULL"));
			}
			DatabaseBackend::Sqlite => {
				assert!(schema.contains("model_identity BLOB NOT NULL"));
				assert!(schema.contains("object_identity BLOB NOT NULL"));
			}
		}
		assert!(!schema.contains("FOREIGN KEY"));
		assert!(!schema.contains("old_values"));
		assert!(!schema.contains("new_values"));
	}

	#[rstest]
	fn mysql_history_identity_is_mariadb_compatible() {
		// Act
		let schema = history_schema_statements(DatabaseBackend::MySql).join("\n");

		// Assert
		assert!(schema.contains("model_identity BLOB NOT NULL"));
		assert!(schema.contains("object_identity BLOB NOT NULL"));
		assert!(schema.contains("model_identity(255) ASC"));
		assert!(schema.contains("object_identity(255) ASC"));
		assert!(schema.contains("ENGINE=InnoDB"));
		assert!(!schema.contains("0900"));
	}

	#[rstest]
	#[case::postgres(DatabaseBackend::Postgres, "$1", "$11")]
	#[case::mysql(DatabaseBackend::MySql, "?", "?")]
	#[case::sqlite(DatabaseBackend::Sqlite, "?", "?")]
	fn insert_is_backend_aware_and_serializes_only_changed_field_names(
		#[case] backend: DatabaseBackend,
		#[case] first_placeholder: &str,
		#[case] last_placeholder: &str,
	) {
		// Arrange
		let event = NewHistoryEvent {
			occurred_at: Utc.with_ymd_and_hms(2026, 8, 9, 1, 2, 3).unwrap(),
			actor: "staff-7".to_owned(),
			action_name: "update".to_owned(),
			model_name: "blog.Post".to_owned(),
			table_name: "blog_posts".to_owned(),
			object_id: "42".to_owned(),
			object_repr: "Post (42)".to_owned(),
			changed_fields: vec!["title".to_owned(), "status".to_owned(), "title".to_owned()],
			affected_count: 1,
			success: true,
		};

		// Act
		let (sql, params) = build_insert_history_query(backend, &event).unwrap();

		// Assert
		assert!(sql.contains(first_placeholder));
		assert!(sql.contains(last_placeholder));
		assert_eq!(params.len(), 11);
		assert!(matches!(params[4], QueryValue::Bytes(_)));
		assert!(matches!(params[6], QueryValue::Bytes(_)));
		assert_eq!(
			params[8],
			QueryValue::String("[\"status\",\"title\"]".to_owned())
		);
		assert!(!format!("{params:?}").contains("secret-before-value"));
	}

	#[rstest]
	#[case::postgres(DatabaseBackend::Postgres, "$1", "$4")]
	#[case::mysql(DatabaseBackend::MySql, "?", "?")]
	#[case::sqlite(DatabaseBackend::Sqlite, "?", "?")]
	fn object_history_query_is_exact_and_stably_ordered(
		#[case] backend: DatabaseBackend,
		#[case] first_placeholder: &str,
		#[case] last_placeholder: &str,
	) {
		// Act
		let (sql, params) =
			build_list_object_history_query(backend, "blog.Post", "blog_posts", "42", 20, 10)
				.unwrap();

		// Assert
		assert!(sql.contains(first_placeholder));
		assert!(sql.contains(&format!("OFFSET {last_placeholder}")));
		assert!(sql.contains("model_identity"));
		assert!(sql.contains("object_identity"));
		assert!(sql.contains("occurred_at"));
		assert!(sql.contains("DESC"));
		assert_eq!(
			params,
			vec![
				QueryValue::Bytes(vec![
					0, 0, 0, 0, 0, 0, 0, 9, 98, 108, 111, 103, 46, 80, 111, 115, 116, 0, 0, 0, 0,
					0, 0, 0, 10, 98, 108, 111, 103, 95, 112, 111, 115, 116, 115
				]),
				QueryValue::Bytes(b"42".to_vec()),
				QueryValue::Int(10),
				QueryValue::Int(20),
			]
		);
	}

	#[rstest]
	fn binary_identity_keys_distinguish_table_and_exact_object_text() {
		// Arrange
		let base = (model_identity("Post", "posts"), b"Object".to_vec());

		// Act
		let other_table = (model_identity("Post", "archived_posts"), b"Object".to_vec());
		let different_case = (model_identity("Post", "posts"), b"object".to_vec());
		let trailing_space = (model_identity("Post", "posts"), b"Object ".to_vec());
		let ambiguous_left = (model_identity("ab", "c"), b"Object".to_vec());
		let ambiguous_right = (model_identity("a", "bc"), b"Object".to_vec());

		// Assert
		assert_ne!(base.0, other_table.0);
		assert_ne!(base.1, different_case.1);
		assert_ne!(base.1, trailing_space.1);
		assert_ne!(ambiguous_left.0, ambiguous_right.0);
	}

	#[rstest]
	fn stored_event_decodes_json_changed_fields() {
		// Arrange
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Int(7));
		row.insert(
			"occurred_at".to_owned(),
			QueryValue::String("2026-08-09T01:02:03.000000Z".to_owned()),
		);
		row.insert("actor".to_owned(), QueryValue::String("staff-7".to_owned()));
		row.insert(
			"action_name".to_owned(),
			QueryValue::String("delete".to_owned()),
		);
		row.insert(
			"model_name".to_owned(),
			QueryValue::String("blog.Post".to_owned()),
		);
		row.insert("object_id".to_owned(), QueryValue::String("42".to_owned()));
		row.insert(
			"object_repr".to_owned(),
			QueryValue::String("Post (42)".to_owned()),
		);
		row.insert(
			"changed_fields".to_owned(),
			QueryValue::String("[\"status\",\"title\"]".to_owned()),
		);
		row.insert("affected_count".to_owned(), QueryValue::Int(1));
		row.insert("success".to_owned(), QueryValue::Bool(true));

		// Act
		let event = stored_history_event_from_row(row).unwrap();

		// Assert
		assert_eq!(event.id, 7);
		assert_eq!(event.model_name, "blog.Post");
		assert_eq!(event.object_id, "42");
		assert_eq!(event.changed_fields, vec!["status", "title"]);
		assert!(event.success);
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_history_is_persistent_exact_and_stably_ordered_without_an_object_row() {
		// Arrange
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("in-memory SQLite must connect");
		let lease =
			DatabaseConnectionLease::register(owner).expect("SQLite connection must register");
		let mut connection = lease.handle();
		initialize_admin_history_schema(&mut connection)
			.await
			.expect("history schema must initialize");
		let occurred_at = Utc.with_ymd_and_hms(2026, 8, 9, 1, 2, 3).unwrap();
		for (model_name, object_id, action_name) in [
			("blog.Post", "42", "CREATE"),
			("blog.Post", "420", "CREATE"),
			("blog.Post", "42", "DELETE"),
		] {
			insert_history_event(
				&mut connection,
				&NewHistoryEvent {
					occurred_at,
					actor: "staff-7".to_string(),
					action_name: action_name.to_string(),
					model_name: model_name.to_string(),
					table_name: "blog_posts".to_string(),
					object_id: object_id.to_string(),
					object_repr: format!("{model_name} ({object_id})"),
					changed_fields: vec!["status".to_string(), "name".to_string()],
					affected_count: 1,
					success: true,
				},
			)
			.await
			.expect("history event must persist");
		}

		// Act
		let events = list_object_history(&mut connection, "blog.Post", "blog_posts", "42", 0, 25)
			.await
			.expect("history must be queryable without an object table");
		let count = count_object_history(&mut connection, "blog.Post", "blog_posts", "42")
			.await
			.expect("history count must be queryable");

		// Assert
		assert_eq!(count, 2);
		assert_eq!(
			events.iter().map(|event| event.id).collect::<Vec<_>>(),
			[3, 1]
		);
		assert_eq!(events[0].action_name, "DELETE");
		assert_eq!(events[0].changed_fields, ["name", "status"]);
		assert!(events.iter().all(|event| event.object_id == "42"));
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_history_accepts_audit_metadata_longer_than_255_characters() {
		// Arrange
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("in-memory SQLite must connect");
		let lease =
			DatabaseConnectionLease::register(owner).expect("SQLite connection must register");
		let mut connection = lease.handle();
		initialize_admin_history_schema(&mut connection)
			.await
			.expect("history schema must initialize");
		let actor = "a".repeat(256);
		let action_name = "b".repeat(256);
		let model_name = "c".repeat(256);
		let event = NewHistoryEvent {
			occurred_at: Utc.with_ymd_and_hms(2026, 8, 9, 1, 2, 3).unwrap(),
			actor: actor.clone(),
			action_name: action_name.clone(),
			model_name: model_name.clone(),
			table_name: "blog_posts".to_owned(),
			object_id: "42".to_owned(),
			object_repr: "Post (42)".to_owned(),
			changed_fields: Vec::new(),
			affected_count: 1,
			success: true,
		};

		// Act
		insert_history_event(&mut connection, &event)
			.await
			.expect("long audit metadata must be accepted");
		let events = list_object_history(&mut connection, &model_name, "blog_posts", "42", 0, 1)
			.await
			.expect("long audit metadata must remain queryable");

		// Assert
		assert_eq!(events.len(), 1);
		assert_eq!(events[0].actor, actor);
		assert_eq!(events[0].action_name, action_name);
		assert_eq!(events[0].model_name, model_name);
	}
}
