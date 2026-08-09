use chrono::{DateTime, SecondsFormat, Utc};
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error, Result};
use reinhardt_db::orm::{DatabaseBackend, OrmExecutor, QueryValue, Row};

const POSTGRES_SCHEMA: &[&str] = &[
	"CREATE TABLE IF NOT EXISTS reinhardt_admin_history (\
		id BIGSERIAL PRIMARY KEY, \
		occurred_at VARCHAR(35) NOT NULL, \
		actor VARCHAR(255) NOT NULL, \
		action_name VARCHAR(255) NOT NULL, \
		model_name VARCHAR(255) NOT NULL, \
		model_identity BYTEA NOT NULL, \
		object_id VARCHAR(255) NOT NULL, \
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
		actor VARCHAR(255) NOT NULL, \
		action_name VARCHAR(255) NOT NULL, \
		model_name VARCHAR(255) NOT NULL, \
		model_identity VARBINARY(526) NOT NULL, \
		object_id VARCHAR(255) NOT NULL, \
		object_identity VARBINARY(1020) NOT NULL, \
		object_repr TEXT NOT NULL, \
		changed_fields TEXT NOT NULL, \
		affected_count BIGINT UNSIGNED NOT NULL, \
		success BOOLEAN NOT NULL, \
		INDEX reinhardt_admin_history_object_idx \
			(model_identity(255) ASC, object_identity(255) ASC, occurred_at ASC, id ASC)\
	) CHARACTER SET utf8mb4"];

const SQLITE_SCHEMA: &[&str] = &[
	"CREATE TABLE IF NOT EXISTS reinhardt_admin_history (\
		id INTEGER PRIMARY KEY AUTOINCREMENT, \
		occurred_at VARCHAR(35) NOT NULL, \
		actor VARCHAR(255) NOT NULL, \
		action_name VARCHAR(255) NOT NULL, \
		model_name VARCHAR(255) NOT NULL, \
		model_identity BLOB NOT NULL, \
		object_id VARCHAR(255) NOT NULL, \
		object_identity BLOB NOT NULL, \
		object_repr TEXT NOT NULL, \
		changed_fields TEXT NOT NULL, \
		affected_count INTEGER NOT NULL, \
		success BOOLEAN NOT NULL\
	)",
	"CREATE INDEX IF NOT EXISTS reinhardt_admin_history_object_idx \
		ON reinhardt_admin_history (model_identity, object_identity, occurred_at DESC, id DESC)",
];

const POSTGRES_INSERT: &str = "INSERT INTO reinhardt_admin_history (\
	occurred_at, actor, action_name, model_name, model_identity, object_id, object_identity, \
	object_repr, changed_fields, affected_count, success\
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)";

const QUESTION_MARK_INSERT: &str = "INSERT INTO reinhardt_admin_history (\
	occurred_at, actor, action_name, model_name, model_identity, object_id, object_identity, \
	object_repr, changed_fields, affected_count, success\
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

const POSTGRES_LIST: &str = "SELECT id, occurred_at, actor, action_name, model_name, object_id, \
	object_repr, changed_fields, affected_count, success \
	FROM reinhardt_admin_history \
	WHERE model_identity = $1 AND object_identity = $2 \
	ORDER BY occurred_at DESC, id DESC LIMIT $3 OFFSET $4";

const QUESTION_MARK_LIST: &str = "SELECT id, occurred_at, actor, action_name, model_name, object_id, \
	object_repr, changed_fields, affected_count, success \
	FROM reinhardt_admin_history \
	WHERE model_identity = ? AND object_identity = ? \
	ORDER BY occurred_at DESC, id DESC LIMIT ? OFFSET ?";

const POSTGRES_COUNT: &str = "SELECT COUNT(*) AS count FROM reinhardt_admin_history \
	WHERE model_identity = $1 AND object_identity = $2";

const QUESTION_MARK_COUNT: &str = "SELECT COUNT(*) AS count FROM reinhardt_admin_history \
	WHERE model_identity = ? AND object_identity = ?";

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

fn insert_history_sql(backend: DatabaseBackend) -> &'static str {
	match backend {
		DatabaseBackend::Postgres => POSTGRES_INSERT,
		DatabaseBackend::MySql | DatabaseBackend::Sqlite => QUESTION_MARK_INSERT,
	}
}

fn list_object_history_sql(backend: DatabaseBackend) -> &'static str {
	match backend {
		DatabaseBackend::Postgres => POSTGRES_LIST,
		DatabaseBackend::MySql | DatabaseBackend::Sqlite => QUESTION_MARK_LIST,
	}
}

fn count_object_history_sql(backend: DatabaseBackend) -> &'static str {
	match backend {
		DatabaseBackend::Postgres => POSTGRES_COUNT,
		DatabaseBackend::MySql | DatabaseBackend::Sqlite => QUESTION_MARK_COUNT,
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

fn insert_history_params(event: &NewHistoryEvent) -> Result<Vec<QueryValue>> {
	let mut changed_fields = event.changed_fields.clone();
	changed_fields.sort_unstable();
	changed_fields.dedup();
	let changed_fields = serde_json::to_string(&changed_fields).map_err(serialization_error)?;
	let affected_count = i64::try_from(event.affected_count).map_err(serialization_error)?;

	Ok(vec![
		QueryValue::String(
			event
				.occurred_at
				.to_rfc3339_opts(SecondsFormat::Micros, true),
		),
		QueryValue::String(event.actor.clone()),
		QueryValue::String(event.action_name.clone()),
		QueryValue::String(event.model_name.clone()),
		QueryValue::Bytes(model_identity(&event.model_name, &event.table_name)),
		QueryValue::String(event.object_id.clone()),
		QueryValue::Bytes(event.object_id.as_bytes().to_vec()),
		QueryValue::String(event.object_repr.clone()),
		QueryValue::String(changed_fields),
		QueryValue::Int(affected_count),
		QueryValue::Bool(event.success),
	])
}

fn object_identity_params(model_name: &str, table_name: &str, object_id: &str) -> Vec<QueryValue> {
	vec![
		QueryValue::Bytes(model_identity(model_name, table_name)),
		QueryValue::Bytes(object_id.as_bytes().to_vec()),
	]
}

fn object_history_params(
	model_name: &str,
	table_name: &str,
	object_id: &str,
	offset: u64,
	limit: u64,
) -> Result<Vec<QueryValue>> {
	let mut params = object_identity_params(model_name, table_name, object_id);
	params.push(QueryValue::Int(
		i64::try_from(limit).map_err(serialization_error)?,
	));
	params.push(QueryValue::Int(
		i64::try_from(offset).map_err(serialization_error)?,
	));
	Ok(params)
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

// Call this before opening an atomic mutation transaction. MySQL may implicitly
// commit DDL, so schema initialization must not run inside the mutation transaction.
pub(crate) async fn ensure_history_schema<E>(executor: &mut E) -> Result<()>
where
	E: OrmExecutor + ?Sized,
{
	for statement in history_schema_statements(executor.backend()) {
		executor.execute(statement, Vec::new()).await?;
	}
	Ok(())
}

pub(crate) async fn insert_history_event<E>(executor: &mut E, event: &NewHistoryEvent) -> Result<()>
where
	E: OrmExecutor + ?Sized,
{
	let sql = insert_history_sql(executor.backend());
	let params = insert_history_params(event)?;
	executor.execute(sql, params).await?;
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
	let sql = list_object_history_sql(executor.backend());
	let params = object_history_params(model_name, table_name, object_id, offset, limit)?;
	let rows = executor.fetch_all(sql, params).await?;
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
	let sql = count_object_history_sql(executor.backend());
	let row = executor
		.fetch_one(
			sql,
			object_identity_params(model_name, table_name, object_id),
		)
		.await?;
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
		match backend {
			DatabaseBackend::Postgres => {
				assert!(schema.contains("model_identity BYTEA NOT NULL"));
				assert!(schema.contains("object_identity BYTEA NOT NULL"));
			}
			DatabaseBackend::MySql => {
				assert!(schema.contains("model_identity VARBINARY("));
				assert!(schema.contains("object_identity VARBINARY("));
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
		assert!(schema.contains("model_identity VARBINARY(526)"));
		assert!(schema.contains("model_identity(255) ASC"));
		assert!(schema.contains("object_identity(255) ASC"));
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
		let sql = insert_history_sql(backend);
		let params = insert_history_params(&event).unwrap();

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
		let sql = list_object_history_sql(backend);
		let params = object_history_params("blog.Post", "blog_posts", "42", 20, 10).unwrap();

		// Assert
		assert!(sql.contains(&format!("model_identity = {first_placeholder}")));
		assert!(sql.contains(&format!("OFFSET {last_placeholder}")));
		assert!(sql.contains("object_identity ="));
		assert!(sql.contains("ORDER BY occurred_at DESC, id DESC"));
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
		let base = object_identity_params("Post", "posts", "Object");

		// Act
		let other_table = object_identity_params("Post", "archived_posts", "Object");
		let different_case = object_identity_params("Post", "posts", "object");
		let trailing_space = object_identity_params("Post", "posts", "Object ");
		let ambiguous_left = object_identity_params("ab", "c", "Object");
		let ambiguous_right = object_identity_params("a", "bc", "Object");

		// Assert
		assert_ne!(base[0], other_table[0]);
		assert_ne!(base[1], different_case[1]);
		assert_ne!(base[1], trailing_space[1]);
		assert_ne!(ambiguous_left[0], ambiguous_right[0]);
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
		ensure_history_schema(&mut connection)
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
}
