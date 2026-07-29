//! Behavioral tests for typed ORM upsert builders.

use std::collections::{HashMap, VecDeque};

use async_trait::async_trait;
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error, Result};
use reinhardt_db::orm::connection::{DatabaseBackend, OrmExecutor, QueryResult, QueryValue, Row};
use reinhardt_db::orm::custom_manager::CustomManager;
use reinhardt_db::orm::expressions::FieldRef;
use reinhardt_db::orm::inspection::FieldInfo;
use reinhardt_db::orm::manager::Manager;
use reinhardt_db::orm::model::{FieldSelector, Model};
use reinhardt_db::orm::upsert::UpsertWrite;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Article {
	id: Option<i64>,
	#[serde(rename(deserialize = "article_slug"))]
	slug: String,
	#[serde(rename(deserialize = "article_rank"))]
	rank: i32,
}

#[derive(Clone)]
struct ArticleFields;

impl FieldSelector for ArticleFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for Article {
	type PrimaryKey = i64;
	type Fields = ArticleFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"articles"
	}

	fn new_fields() -> Self::Fields {
		ArticleFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn field_metadata() -> Vec<FieldInfo> {
		vec![
			field("id", None, true, false),
			field("slug", Some("article_slug"), false, true),
			field("rank", Some("article_rank"), false, false),
		]
	}
}

impl Article {
	fn field_slug() -> FieldRef<Self, String> {
		// SAFETY: the names and type match Article's declared slug field.
		unsafe { FieldRef::from_model_field("slug", "article_slug") }
	}

	fn field_rank() -> FieldRef<Self, i32> {
		// SAFETY: the names and type match Article's declared rank field.
		unsafe { FieldRef::from_model_field("rank", "article_rank") }
	}

	fn field_id() -> FieldRef<Self, i64> {
		// SAFETY: the names and type match Article's declared primary-key field.
		unsafe { FieldRef::from_model_field("id", "id") }
	}
}

fn field(name: &str, db_column: Option<&str>, primary_key: bool, unique: bool) -> FieldInfo {
	FieldInfo {
		name: name.to_owned(),
		field_type: "test".to_owned(),
		nullable: false,
		primary_key,
		unique,
		blank: false,
		editable: true,
		storage_kind: None,
		domain: None,
		default: None,
		db_default: None,
		db_column: db_column.map(str::to_owned),
		choices: None,
		attributes: HashMap::new(),
	}
}

fn article_row(id: i64, slug: &str, rank: i32) -> Row {
	Row {
		data: HashMap::from([
			("id".to_owned(), QueryValue::Int(id)),
			(
				"article_slug".to_owned(),
				QueryValue::String(slug.to_owned()),
			),
			("article_rank".to_owned(), QueryValue::Int(i64::from(rank))),
		]),
	}
}

#[derive(Clone, Debug, PartialEq)]
struct RecordedCall {
	operation: &'static str,
	sql: String,
	params: Vec<QueryValue>,
}

struct RecordingExecutor {
	backend: DatabaseBackend,
	calls: Vec<RecordedCall>,
	execute_results: VecDeque<Result<QueryResult>>,
	fetch_all_results: VecDeque<Result<Vec<Row>>>,
}

impl RecordingExecutor {
	fn new(backend: DatabaseBackend) -> Self {
		Self {
			backend,
			calls: Vec::new(),
			execute_results: VecDeque::new(),
			fetch_all_results: VecDeque::new(),
		}
	}

	fn with_execute(mut self, result: Result<QueryResult>) -> Self {
		self.execute_results.push_back(result);
		self
	}

	fn with_fetch_all(mut self, result: Result<Vec<Row>>) -> Self {
		self.fetch_all_results.push_back(result);
		self
	}

	fn operations(&self) -> Vec<&'static str> {
		self.calls.iter().map(|call| call.operation).collect()
	}

	fn record(&mut self, operation: &'static str, sql: &str, params: Vec<QueryValue>) {
		self.calls.push(RecordedCall {
			operation,
			sql: sql.to_owned(),
			params,
		});
	}

	fn missing(operation: &str) -> Error {
		DatabaseError::new(
			DatabaseErrorKind::Query,
			format!("no queued {operation} result"),
		)
		.into()
	}
}

#[async_trait]
impl OrmExecutor for RecordingExecutor {
	fn backend(&self) -> DatabaseBackend {
		self.backend
	}

	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		self.record("execute", sql, params);
		self.execute_results
			.pop_front()
			.unwrap_or_else(|| Err(Self::missing("execute")))
	}

	async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
		Err(Self::missing("fetch_one"))
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		self.record("fetch_all", sql, params);
		self.fetch_all_results
			.pop_front()
			.unwrap_or_else(|| Err(Self::missing("fetch_all")))
	}

	async fn fetch_optional(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> Result<Option<Row>> {
		Err(Self::missing("fetch_optional"))
	}
}

#[derive(Default)]
struct MutatingManager;

impl CustomManager for MutatingManager {
	type Model = Article;

	fn new() -> Self {
		Self
	}

	fn before_upsert_write(&self, write: &mut UpsertWrite<'_, Article>) -> Result<()> {
		if let UpsertWrite::Create(create) = write {
			create.set(Article::field_rank(), 42)?;
		}
		Ok(())
	}
}

#[derive(Default)]
struct VetoManager;

impl CustomManager for VetoManager {
	type Model = Article;

	fn new() -> Self {
		Self
	}

	fn before_upsert_write(&self, _write: &mut UpsertWrite<'_, Article>) -> Result<()> {
		Err(Error::Validation("upsert vetoed".to_owned()))
	}
}

#[derive(Default)]
struct PlainManager;

impl CustomManager for PlainManager {
	type Model = Article;

	fn new() -> Self {
		Self
	}
}

#[tokio::test]
async fn get_or_create_validation_error_precedes_executor_use() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres);

	let error = Manager::<Article>::new()
		.get_or_create()
		.default(Article::field_rank(), 1)
		.execute_with(&mut executor)
		.await
		.expect_err("an empty lookup must fail validation");

	assert!(matches!(error, Error::Validation(_)));
	assert_eq!(executor.operations(), Vec::<&str>::new());
}

#[tokio::test]
async fn get_or_create_global_connection_is_acquired_after_validation() {
	let error = Manager::<Article>::new()
		.get_or_create()
		.default(Article::field_rank(), 1)
		.execute()
		.await
		.expect_err("plan validation must run before global connection lookup");

	assert!(matches!(error, Error::Validation(_)));
}

#[tokio::test]
async fn get_or_create_existing_row_skips_hook_and_insert() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(Ok(vec![article_row(1, "rust", 7)]));

	let (article, created) = VetoManager
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.default(Article::field_rank(), 1)
		.execute_with(&mut executor)
		.await
		.expect("an existing row must bypass the create hook");

	assert_eq!(
		article,
		Article {
			id: Some(1),
			slug: "rust".to_owned(),
			rank: 7
		}
	);
	assert!(!created);
	assert_eq!(executor.operations(), ["fetch_all"]);
}

#[tokio::test]
async fn get_or_create_absent_row_inserts_and_reloads() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Ok(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		}))
		.with_fetch_all(Ok(vec![article_row(2, "rust", 1)]));

	let (article, created) = Manager::<Article>::new()
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.default(Article::field_rank(), 1)
		.execute_with(&mut executor)
		.await
		.expect("an absent row must be inserted and reloaded");

	assert_eq!(article.id, Some(2));
	assert!(created);
	assert_eq!(executor.operations(), ["fetch_all", "execute", "fetch_all"]);
}

#[rstest]
#[case(DatabaseBackend::Postgres)]
#[case(DatabaseBackend::Sqlite)]
#[tokio::test]
async fn get_or_create_zero_affected_conflict_reloads_existing_row(
	#[case] backend: DatabaseBackend,
) {
	let mut executor = RecordingExecutor::new(backend)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Ok(QueryResult {
			rows_affected: 0,
			last_insert_id: None,
		}))
		.with_fetch_all(Ok(vec![article_row(3, "rust", 9)]));

	let (article, created) = Manager::<Article>::new()
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.default(Article::field_rank(), 1)
		.execute_with(&mut executor)
		.await
		.expect("a targeted conflict must reload the winning row");

	assert_eq!(article.rank, 9);
	assert!(!created);
	assert_eq!(executor.operations(), ["fetch_all", "execute", "fetch_all"]);
}

#[rstest]
#[case(Vec::new())]
#[case(vec![article_row(3, "rust", 1), article_row(4, "rust", 2)])]
#[tokio::test]
async fn get_or_create_conflict_requires_exactly_one_lookup_row(#[case] rows: Vec<Row>) {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Ok(QueryResult {
			rows_affected: 0,
			last_insert_id: None,
		}))
		.with_fetch_all(Ok(rows));

	let error = Manager::<Article>::new()
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.execute_with(&mut executor)
		.await
		.expect_err("a conflict reload mismatch must fail");

	assert!(matches!(error, Error::Conflict(_)));
	assert_eq!(executor.operations(), ["fetch_all", "execute", "fetch_all"]);
}

#[tokio::test]
async fn mysql_unique_violation_with_matching_row_is_a_lost_race() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Err(DatabaseError::new(
			DatabaseErrorKind::UniqueViolation,
			"duplicate slug",
		)
		.into()))
		.with_fetch_all(Ok(vec![article_row(5, "rust", 8)]));

	let (article, created) = Manager::<Article>::new()
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.execute_with(&mut executor)
		.await
		.expect("a matching unique violation must reload the winner");

	assert_eq!(article.id, Some(5));
	assert!(!created);
	assert_eq!(executor.operations(), ["fetch_all", "execute", "fetch_all"]);
}

#[tokio::test]
async fn mysql_unrelated_unique_violation_returns_the_original_error() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Err(DatabaseError::new(
			DatabaseErrorKind::UniqueViolation,
			"unrelated unique constraint",
		)
		.into()))
		.with_fetch_all(Ok(Vec::new()));

	let error = Manager::<Article>::new()
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.execute_with(&mut executor)
		.await
		.expect_err("an unrelated unique violation must remain unchanged");

	assert_eq!(
		error.database_kind(),
		Some(DatabaseErrorKind::UniqueViolation)
	);
	assert!(error.to_string().contains("unrelated unique constraint"));
	assert_eq!(executor.operations(), ["fetch_all", "execute", "fetch_all"]);
}

#[tokio::test]
async fn mysql_zero_affected_success_is_not_classified_as_a_race() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Ok(QueryResult {
			rows_affected: 0,
			last_insert_id: None,
		}));

	let error = Manager::<Article>::new()
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.execute_with(&mut executor)
		.await
		.expect_err("only a MySQL unique violation may enter race recovery");

	assert!(matches!(error, Error::Conflict(_)));
	assert_eq!(executor.operations(), ["fetch_all", "execute"]);
}

#[tokio::test]
async fn custom_and_standard_managers_use_the_same_create_sequence() {
	let mut standard = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Ok(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		}))
		.with_fetch_all(Ok(vec![article_row(6, "rust", 1)]));
	let mut custom = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Ok(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		}))
		.with_fetch_all(Ok(vec![article_row(6, "rust", 1)]));

	let _ = Manager::<Article>::new()
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.execute_with(&mut standard)
		.await
		.expect("standard manager path must succeed");
	let _ = PlainManager
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.execute_with(&mut custom)
		.await
		.expect("custom manager path must succeed");

	assert_eq!(standard.calls, custom.calls);
	assert_eq!(standard.operations(), ["fetch_all", "execute", "fetch_all"]);
}

#[tokio::test]
async fn create_hook_mutation_changes_insert_parameters() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(Ok(Vec::new()))
		.with_execute(Ok(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		}))
		.with_fetch_all(Ok(vec![article_row(7, "rust", 42)]));

	let (_, created) = MutatingManager
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.default(Article::field_rank(), 1)
		.execute_with(&mut executor)
		.await
		.expect("the mutated create values must be inserted");

	assert!(created);
	assert_eq!(
		executor.calls[1].params,
		vec![QueryValue::String("rust".to_owned()), QueryValue::Int(42)]
	);
}

#[tokio::test]
async fn create_hook_veto_records_no_write() {
	let mut executor =
		RecordingExecutor::new(DatabaseBackend::Postgres).with_fetch_all(Ok(Vec::new()));

	let error = VetoManager
		.get_or_create()
		.lookup(Article::field_slug(), "rust")
		.execute_with(&mut executor)
		.await
		.expect_err("the create hook must be able to veto the write");

	assert_eq!(error.to_string(), "Validation error: upsert vetoed");
	assert_eq!(executor.operations(), ["fetch_all"]);
}

#[tokio::test]
async fn get_or_create_preserves_the_first_builder_encoding_error() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres);

	let error = Manager::<Article>::new()
		.get_or_create()
		.lookup(Article::field_id(), u64::MAX)
		.lookup(Article::field_slug(), "rust")
		.execute_with(&mut executor)
		.await
		.expect_err("the first builder encoding failure must be retained");

	assert_eq!(
		error.database_kind(),
		Some(DatabaseErrorKind::Serialization)
	);
	assert!(error.to_string().contains("exceeds i64 database range"));
	assert_eq!(executor.operations(), Vec::<&str>::new());
}
