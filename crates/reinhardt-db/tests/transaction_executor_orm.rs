//! Regression coverage for ORM operations backed by a caller-owned executor.
//!
//! The recorder deliberately implements only [`OrmExecutor`], proving that
//! explicit ORM APIs do not depend on transaction-only behavior or open a
//! hidden global connection.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
use reinhardt_db::associations::markers::ManyToManyConfig;
use reinhardt_db::associations::{ManyToManyField, ManyToManyManager};
use reinhardt_db::orm::annotation::{AnnotationValue, Expression, Value};
use reinhardt_db::orm::composite_pk::{CompositePrimaryKey, PkValue};
#[cfg(feature = "sqlite")]
use reinhardt_db::orm::connection::{BackendsConnection, DatabaseConnectionLease};
use reinhardt_db::orm::connection::{
	DatabaseBackend, OrmExecutor, QueryResult, QueryValue, Row, TransactionExecutor,
};
use reinhardt_db::orm::custom_manager::CustomManager;
use reinhardt_db::orm::events::{EventRegistry, EventResult, MapperEvents, set_active_registry};
use reinhardt_db::orm::execution::{QueryExecution, SelectExecution};
use reinhardt_db::orm::expressions::{F, FieldRef, UniqueFieldRef};
use reinhardt_db::orm::inspection::FieldInfo;
use reinhardt_db::orm::manager::Manager;
use reinhardt_db::orm::model::{FieldSelector, Model};
use reinhardt_db::orm::query::{FieldAssignment, Filter, FilterOperator, FilterValue, UpdateValue};
use reinhardt_db::orm::relations::GenericRelationSet;
use reinhardt_db::orm::{
	ForeignKeyAccessor, ManyToManyAccessor, NPlusOneConfig, NPlusOneScope, QuerySet,
};
use reinhardt_query::prelude::{Alias, Query};

#[cfg(feature = "sqlite")]
async fn sqlite_connection(
	url: &str,
) -> (
	DatabaseConnectionLease,
	reinhardt_db::orm::DatabaseConnection,
) {
	let owner = BackendsConnection::connect(url).await.unwrap();
	let lease = DatabaseConnectionLease::register(owner).unwrap();
	let handle = lease.handle();
	(lease, handle)
}

#[derive(Debug, Clone, PartialEq)]
struct RecordedCall {
	kind: &'static str,
	sql: String,
	params: Vec<QueryValue>,
}

#[derive(Debug)]
struct RecordingExecutor {
	backend: DatabaseBackend,
	calls: Vec<RecordedCall>,
	execute_results: VecDeque<QueryResult>,
	fetch_one_rows: VecDeque<Row>,
	fetch_all_rows: VecDeque<Vec<Row>>,
	fetch_optional_rows: VecDeque<Option<Row>>,
	fetch_optional_contexts: Vec<Option<reinhardt_db::backends::error::PgvectorOperationKind>>,
}

#[derive(Debug)]
struct RecordingTransactionExecutor {
	backend: reinhardt_db::backends::types::DatabaseType,
	calls: Vec<RecordedCall>,
	fetch_all_rows: VecDeque<Vec<Row>>,
}

impl RecordingTransactionExecutor {
	fn new(backend: reinhardt_db::backends::types::DatabaseType) -> Self {
		Self {
			backend,
			calls: Vec::new(),
			fetch_all_rows: VecDeque::new(),
		}
	}

	fn with_fetch_all(mut self, rows: Vec<Row>) -> Self {
		self.fetch_all_rows.push_back(rows);
		self
	}

	fn record(&mut self, kind: &'static str, sql: &str, params: Vec<QueryValue>) {
		self.calls.push(RecordedCall {
			kind,
			sql: sql.to_string(),
			params,
		});
	}
}

#[async_trait]
impl TransactionExecutor for RecordingTransactionExecutor {
	fn backend(&self) -> reinhardt_db::backends::types::DatabaseType {
		self.backend
	}

	async fn execute(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_db::backends::Result<QueryResult> {
		self.record("execute", sql, params);
		Ok(QueryResult {
			rows_affected: 0,
			last_insert_id: None,
		})
	}

	async fn fetch_one(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_db::backends::Result<Row> {
		self.record("fetch_one", sql, params);
		Ok(Row::new())
	}

	async fn fetch_all(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_db::backends::Result<Vec<Row>> {
		self.record("fetch_all", sql, params);
		Ok(self.fetch_all_rows.pop_front().unwrap_or_default())
	}

	async fn fetch_optional(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_db::backends::Result<Option<Row>> {
		self.record("fetch_optional", sql, params);
		Ok(None)
	}

	async fn commit(self: Box<Self>) -> reinhardt_db::backends::Result<()> {
		Ok(())
	}

	async fn rollback(self: Box<Self>) -> reinhardt_db::backends::Result<()> {
		Ok(())
	}
}

impl RecordingExecutor {
	fn new(backend: DatabaseBackend) -> Self {
		Self {
			backend,
			calls: Vec::new(),
			execute_results: VecDeque::new(),
			fetch_one_rows: VecDeque::new(),
			fetch_all_rows: VecDeque::new(),
			fetch_optional_rows: VecDeque::new(),
			fetch_optional_contexts: Vec::new(),
		}
	}

	fn with_execute_result(mut self, result: QueryResult) -> Self {
		self.execute_results.push_back(result);
		self
	}

	fn with_fetch_one(mut self, row: Row) -> Self {
		self.fetch_one_rows.push_back(row);
		self
	}

	fn with_fetch_all(mut self, rows: Vec<Row>) -> Self {
		self.fetch_all_rows.push_back(rows);
		self
	}

	fn with_fetch_optional(mut self, row: Option<Row>) -> Self {
		self.fetch_optional_rows.push_back(row);
		self
	}

	fn record(&mut self, kind: &'static str, sql: &str, params: Vec<QueryValue>) {
		self.calls.push(RecordedCall {
			kind,
			sql: sql.to_string(),
			params,
		});
	}

	fn exhausted_error(operation: &str) -> reinhardt_core::exception::Error {
		DatabaseError::new(
			DatabaseErrorKind::Query,
			format!("RecordingExecutor has no queued {operation} result"),
		)
		.into()
	}
}

#[async_trait]
impl OrmExecutor for RecordingExecutor {
	fn backend(&self) -> DatabaseBackend {
		self.backend
	}

	async fn execute(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		self.record("execute", sql, params);
		self.execute_results
			.pop_front()
			.ok_or_else(|| Self::exhausted_error("execute"))
	}

	async fn fetch_one(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Row> {
		self.record("fetch_one", sql, params);
		self.fetch_one_rows
			.pop_front()
			.ok_or_else(|| Self::exhausted_error("fetch_one"))
	}

	async fn fetch_all(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		self.record("fetch_all", sql, params);
		self.fetch_all_rows
			.pop_front()
			.ok_or_else(|| Self::exhausted_error("fetch_all"))
	}

	async fn fetch_optional(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		self.record("fetch_optional", sql, params);
		self.fetch_optional_rows
			.pop_front()
			.ok_or_else(|| Self::exhausted_error("fetch_optional"))
	}

	async fn fetch_optional_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<reinhardt_db::backends::error::PgvectorOperationKind>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		self.fetch_optional_contexts.push(context);
		self.fetch_optional(sql, params).await
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Article {
	#[serde(rename(serialize = "id", deserialize = "article_id"))]
	id: Option<i64>,
	#[serde(rename(serialize = "title", deserialize = "article_title"))]
	title: String,
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

	fn primary_key_column() -> &'static str {
		"article_id"
	}

	fn latest_by_fields() -> &'static [&'static str] {
		&["article_id"]
	}

	fn field_metadata() -> Vec<FieldInfo> {
		vec![
			FieldInfo {
				name: "id".to_string(),
				field_type: "BigIntegerField".to_string(),
				nullable: false,
				primary_key: true,
				unique: false,
				blank: false,
				editable: true,
				storage_kind: None,
				domain: None,
				default: None,
				db_default: None,
				db_column: Some("article_id".to_string()),
				choices: None,
				attributes: HashMap::new(),
			},
			FieldInfo {
				name: "title".to_string(),
				field_type: "CharField".to_string(),
				nullable: false,
				primary_key: false,
				unique: false,
				blank: false,
				editable: true,
				storage_kind: None,
				domain: None,
				default: None,
				db_default: None,
				db_column: Some("article_title".to_string()),
				choices: None,
				attributes: HashMap::new(),
			},
		]
	}
}

fn article_title_unique() -> UniqueFieldRef<Article, String> {
	fn getter(article: &Article) -> Option<String> {
		Some(article.title.clone())
	}

	// SAFETY: `article_title` is the test fixture's unique lookup column and the getter reads it.
	unsafe { UniqueFieldRef::from_model_field_with_getter("article_title", getter) }
}

struct MarkerSource;

impl std::fmt::Display for MarkerSource {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("marker-source")
	}
}

struct MarkerTarget;

impl std::fmt::Display for MarkerTarget {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("marker-target")
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MetadataFreeArticle {
	#[serde(rename(serialize = "id", deserialize = "article_id"))]
	id: Option<i64>,
	title: String,
}

#[derive(Clone)]
struct MetadataFreeArticleFields;

impl FieldSelector for MetadataFreeArticleFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for MetadataFreeArticle {
	type PrimaryKey = i64;
	type Fields = MetadataFreeArticleFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"metadata_free_articles"
	}

	fn new_fields() -> Self::Fields {
		MetadataFreeArticleFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn primary_key_column() -> &'static str {
		"article_id"
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ArticleLocale {
	article_id: i64,
	locale: String,
	title: String,
}

#[derive(Clone)]
struct ArticleLocaleFields;

impl FieldSelector for ArticleLocaleFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for ArticleLocale {
	type PrimaryKey = String;
	type Fields = ArticleLocaleFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"article_locales"
	}

	fn new_fields() -> Self::Fields {
		ArticleLocaleFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		None
	}

	fn set_primary_key(&mut self, _value: Self::PrimaryKey) {}

	fn composite_primary_key() -> Option<CompositePrimaryKey> {
		CompositePrimaryKey::new(vec!["article_id".to_string(), "locale".to_string()]).ok()
	}
}

#[derive(Default)]
struct ArticleManager;

impl CustomManager for ArticleManager {
	type Model = Article;

	fn new() -> Self {
		Self
	}
}

#[derive(Default)]
struct PrefixingArticleManager;

impl CustomManager for PrefixingArticleManager {
	type Model = Article;

	fn new() -> Self {
		Self
	}

	fn before_save(&self, model: &mut Self::Model) -> reinhardt_core::exception::Result<()> {
		model.title = format!("custom-{}", model.title);
		Ok(())
	}
}

struct ExecutorBorrowingArticleManager {
	bulk_update_calls: Arc<AtomicUsize>,
}

impl CustomManager for ExecutorBorrowingArticleManager {
	type Model = Article;

	fn new() -> Self {
		Self {
			bulk_update_calls: Arc::new(AtomicUsize::new(0)),
		}
	}

	fn get_or_create_with_conn<'a, E>(
		&'a self,
		conn: &'a mut E,
		lookup_fields: HashMap<String, String>,
		defaults: Option<HashMap<String, String>>,
	) -> impl std::future::Future<Output = reinhardt_core::exception::Result<(Self::Model, bool)>>
	+ Send
	+ 'a
	where
		E: OrmExecutor + 'a,
	{
		async move {
			let result = Manager::<Self::Model>::new()
				.get_or_create_with_conn(conn, lookup_fields, defaults)
				.await?;
			let _backend_after_await = conn.backend();
			Ok(result)
		}
	}

	fn before_bulk_update(
		&self,
		models: &mut [Self::Model],
	) -> reinhardt_core::exception::Result<()> {
		self.bulk_update_calls.fetch_add(1, Ordering::SeqCst);
		for model in models {
			model.title = format!("hooked-{}", model.title);
		}
		Ok(())
	}
}

struct VetoInsert;

#[async_trait]
impl MapperEvents for VetoInsert {
	async fn before_insert(&self, _instance_id: &str, _values: &serde_json::Value) -> EventResult {
		EventResult::Veto
	}
}

struct OrderedInsertEvents {
	events: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl MapperEvents for OrderedInsertEvents {
	async fn before_insert(&self, _instance_id: &str, _values: &serde_json::Value) -> EventResult {
		self.events
			.lock()
			.expect("event log mutex should not be poisoned")
			.push("before_insert");
		EventResult::Continue
	}

	async fn after_insert(&self, _instance_id: &str) -> EventResult {
		self.events
			.lock()
			.expect("event log mutex should not be poisoned")
			.push("after_insert");
		EventResult::Continue
	}
}

fn article_row(id: i64, title: &str) -> Row {
	let mut row = Row::new();
	row.insert("article_id".to_string(), QueryValue::Int(id));
	row.insert(
		"article_title".to_string(),
		QueryValue::String(title.to_string()),
	);
	row
}

fn metadata_free_article_row(id: i64, title: &str) -> Row {
	let mut row = Row::new();
	row.insert("article_id".to_string(), QueryValue::Int(id));
	row.insert("title".to_string(), QueryValue::String(title.to_string()));
	row
}

fn article_locale_row(article_id: i64, locale: &str, title: &str) -> Row {
	let mut row = Row::new();
	row.insert("article_id".to_string(), QueryValue::Int(article_id));
	row.insert("locale".to_string(), QueryValue::String(locale.to_string()));
	row.insert("title".to_string(), QueryValue::String(title.to_string()));
	row
}

async fn save_article(
	executor: &mut dyn OrmExecutor,
	article: &Article,
) -> reinhardt_core::exception::Result<Article> {
	let manager = Article::objects();
	if article.primary_key().is_some() {
		manager.update_with_conn(executor, article).await
	} else {
		manager.create_with_conn(executor, article).await
	}
}

#[tokio::test]
async fn dyn_orm_executor_saves_new_and_existing_articles() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_one(article_row(12, "created"))
		.with_fetch_one(article_row(12, "updated"));

	let created = save_article(
		&mut executor,
		&Article {
			id: None,
			title: "created".to_string(),
		},
	)
	.await
	.expect("dynamic executor should insert a new article");
	let updated = save_article(
		&mut executor,
		&Article {
			id: created.id,
			title: "updated".to_string(),
		},
	)
	.await
	.expect("dynamic executor should update an existing article");

	assert_eq!(created.id, Some(12));
	assert_eq!(updated.title, "updated");
	assert_eq!(executor.calls.len(), 2);
	assert_eq!(executor.calls[0].kind, "fetch_one");
	assert!(executor.calls[0].sql.starts_with("INSERT"));
	assert!(
		executor.calls[0]
			.params
			.contains(&QueryValue::String("created".to_string()))
	);
	assert_eq!(executor.calls[1].kind, "fetch_one");
	assert!(executor.calls[1].sql.starts_with("UPDATE"));
	assert!(
		executor.calls[1]
			.params
			.contains(&QueryValue::String("updated".to_string()))
	);
}

#[tokio::test]
async fn none_short_circuits_owned_and_transaction_executors() {
	let queryset = QuerySet::<Article>::new()
		.filter(Filter::new(
			"title",
			FilterOperator::Eq,
			FilterValue::String("unreachable".to_string()),
		))
		.order_by(&["-article_id"])
		.limit(1)
		.none()
		.filter(Filter::new(
			"article_id",
			FilterOperator::Eq,
			FilterValue::Int(1),
		))
		.order_by(&["article_title"])
		.limit(2);
	let mut owned_executor = RecordingExecutor::new(DatabaseBackend::Postgres);

	assert_eq!(
		queryset.all_with_db(&mut owned_executor).await.unwrap(),
		Vec::<Article>::new()
	);
	assert_eq!(
		queryset.first_with_db(&mut owned_executor).await.unwrap(),
		None
	);
	assert!(matches!(
		queryset.get_with_db(&mut owned_executor).await,
		Err(Error::NotFound(_))
	));
	assert_eq!(
		queryset.count_with_db(&mut owned_executor).await.unwrap(),
		0
	);
	assert!(!queryset.exists_with_db(&mut owned_executor).await.unwrap());
	assert!(owned_executor.calls.is_empty());

	let mut transaction_executor =
		RecordingTransactionExecutor::new(reinhardt_db::backends::types::DatabaseType::Postgres);
	assert_eq!(
		queryset
			.all_with_executor(&mut transaction_executor)
			.await
			.unwrap(),
		Vec::<Article>::new()
	);
	assert_eq!(
		queryset
			.one_with_executor(&mut transaction_executor)
			.await
			.unwrap(),
		Vec::<Article>::new()
	);
	assert_eq!(
		queryset
			.count_with_executor(&mut transaction_executor)
			.await
			.unwrap(),
		0
	);
	assert!(transaction_executor.calls.is_empty());
}

#[tokio::test]
async fn latest_and_earliest_use_typed_ordering_with_caller_owned_executors() {
	let ordering = [
		FieldRef::<Article, i64>::new("article_id").ordering(),
		FieldRef::<Article, String>::new("article_title").ordering(),
	];
	let queryset = QuerySet::<Article>::new();
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(vec![article_row(3, "latest")])
		.with_fetch_all(vec![article_row(1, "earliest")])
		.with_fetch_all(vec![article_row(4, "explicit-latest")])
		.with_fetch_all(vec![article_row(2, "explicit-earliest")]);

	assert_eq!(
		queryset.latest_with_db(&mut executor).await.unwrap().id,
		Some(3)
	);
	assert_eq!(
		queryset.earliest_with_db(&mut executor).await.unwrap().id,
		Some(1)
	);
	assert_eq!(
		queryset
			.latest_by_with_db(&mut executor, &ordering)
			.await
			.unwrap()
			.id,
		Some(4)
	);
	assert_eq!(
		queryset
			.earliest_by_with_db(&mut executor, &ordering)
			.await
			.unwrap()
			.id,
		Some(2)
	);
	assert_eq!(executor.calls.len(), 4);
	assert!(
		executor.calls[0]
			.sql
			.contains("ORDER BY \"article_id\" DESC")
	);
	assert!(
		executor.calls[1]
			.sql
			.contains("ORDER BY \"article_id\" ASC")
	);
	assert!(
		executor.calls[2]
			.sql
			.contains("ORDER BY \"article_id\" DESC, \"article_title\" DESC")
	);
	assert!(
		executor.calls[3]
			.sql
			.contains("ORDER BY \"article_id\" ASC, \"article_title\" ASC")
	);
	assert!(
		executor
			.calls
			.iter()
			.all(|call| call.sql.contains("LIMIT $1"))
	);
	assert!(
		executor
			.calls
			.iter()
			.all(|call| call.params == vec![QueryValue::Int(1)])
	);

	let mut transaction_executor =
		RecordingTransactionExecutor::new(reinhardt_db::backends::types::DatabaseType::Postgres)
			.with_fetch_all(vec![article_row(3, "latest")])
			.with_fetch_all(vec![article_row(1, "earliest")])
			.with_fetch_all(vec![article_row(4, "explicit-latest")])
			.with_fetch_all(vec![article_row(2, "explicit-earliest")]);
	assert_eq!(
		queryset
			.latest_with_executor(&mut transaction_executor)
			.await
			.unwrap()
			.id,
		Some(3)
	);
	assert_eq!(
		queryset
			.earliest_with_executor(&mut transaction_executor)
			.await
			.unwrap()
			.id,
		Some(1)
	);
	assert_eq!(
		queryset
			.latest_by_with_executor(&mut transaction_executor, &ordering)
			.await
			.unwrap()
			.id,
		Some(4)
	);
	assert_eq!(
		queryset
			.earliest_by_with_executor(&mut transaction_executor, &ordering)
			.await
			.unwrap()
			.id,
		Some(2)
	);
	assert_eq!(transaction_executor.calls.len(), 4);
	assert!(
		transaction_executor
			.calls
			.iter()
			.all(|call| call.sql.contains("LIMIT $1"))
	);
	assert!(
		transaction_executor
			.calls
			.iter()
			.all(|call| call.params == vec![QueryValue::Int(1)])
	);
}

#[tokio::test]
async fn retrieval_helpers_return_not_found_or_validation_without_extra_queries() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres).with_fetch_all(Vec::new());
	assert!(matches!(
		QuerySet::<Article>::new()
			.latest_with_db(&mut executor)
			.await,
		Err(Error::NotFound(_))
	));
	assert_eq!(executor.calls.len(), 1);

	let mut metadata_free_executor = RecordingExecutor::new(DatabaseBackend::Postgres);
	assert!(matches!(
		QuerySet::<MetadataFreeArticle>::new()
			.latest_with_db(&mut metadata_free_executor)
			.await,
		Err(Error::Validation(_))
	));
	assert!(metadata_free_executor.calls.is_empty());
	assert!(matches!(
		QuerySet::<Article>::new()
			.latest_by_with_db(&mut metadata_free_executor, &[])
			.await,
		Err(Error::Validation(_))
	));
	assert!(matches!(
		QuerySet::<MetadataFreeArticle>::new().none().latest().await,
		Err(Error::Validation(_))
	));
	assert!(matches!(
		QuerySet::<Article>::new().none().latest_by(&[]).await,
		Err(Error::Validation(_))
	));
	assert!(metadata_free_executor.calls.is_empty());
}

#[tokio::test]
async fn in_bulk_uses_one_typed_in_query_and_returns_ordered_maps() {
	let queryset = QuerySet::<Article>::new();
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(vec![article_row(3, "third"), article_row(1, "first")])
		.with_fetch_all(vec![article_row(2, "second"), article_row(1, "first")]);
	let primary = queryset
		.in_bulk_with_db(&mut executor, [3_i64, 1, 3, 99])
		.await
		.unwrap();
	assert_eq!(primary.keys().copied().collect::<Vec<_>>(), vec![1, 3]);
	let unique = queryset
		.in_bulk_by_with_db(
			&mut executor,
			article_title_unique(),
			[
				"second".to_string(),
				"first".to_string(),
				"second".to_string(),
			],
		)
		.await
		.unwrap();
	assert_eq!(
		unique.keys().cloned().collect::<Vec<_>>(),
		vec!["first".to_string(), "second".to_string()]
	);
	assert_eq!(executor.calls.len(), 2);
	assert!(executor.calls.iter().all(|call| call.kind == "fetch_all"));
	assert!(executor.calls.iter().all(|call| call.sql.contains(" IN ")));
	assert!(
		executor
			.calls
			.iter()
			.all(|call| !call.sql.contains("Expr::cust"))
	);
	assert_eq!(
		executor.calls[0].sql,
		"SELECT * FROM \"articles\" WHERE \"article_id\" IN ($1, $2, $3)"
	);
	assert_eq!(
		executor.calls[1].sql,
		"SELECT * FROM \"articles\" WHERE \"article_title\" IN ($1, $2)"
	);
	assert_eq!(
		executor.calls[0].params,
		vec![
			QueryValue::Int(1),
			QueryValue::Int(3),
			QueryValue::Int(99)
		]
	);
	assert_eq!(
		executor.calls[1].params,
		vec![
			QueryValue::String("first".to_string()),
			QueryValue::String("second".to_string())
		]
	);

	let mut transaction_executor =
		RecordingTransactionExecutor::new(reinhardt_db::backends::types::DatabaseType::Postgres)
			.with_fetch_all(vec![article_row(2, "second")])
			.with_fetch_all(vec![article_row(1, "first")]);
	assert_eq!(
		queryset
			.in_bulk_with_executor(&mut transaction_executor, [2_i64, 2])
			.await
			.unwrap()
			.keys()
			.copied()
			.collect::<Vec<_>>(),
		vec![2]
	);
	assert_eq!(
		queryset
			.in_bulk_by_with_executor(
				&mut transaction_executor,
				article_title_unique(),
				["first".to_string()],
			)
			.await
			.unwrap()
			.keys()
			.cloned()
			.collect::<Vec<_>>(),
		vec!["first".to_string()]
	);
	assert_eq!(transaction_executor.calls.len(), 2);

	let mut empty_executor = RecordingExecutor::new(DatabaseBackend::Postgres);
	assert!(
		queryset
			.in_bulk_with_db(&mut empty_executor, Vec::<i64>::new())
			.await
			.unwrap()
			.is_empty()
	);
	assert!(
		queryset
			.none()
			.in_bulk_by_with_db(
				&mut empty_executor,
				article_title_unique(),
				["unreachable".to_string()],
			)
			.await
			.unwrap()
			.is_empty()
	);
	assert!(empty_executor.calls.is_empty());
}

#[tokio::test]
async fn explicit_orm_paths_use_the_caller_owned_recording_executor() {
	let original = Article {
		id: None,
		title: "first".to_string(),
	};
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_one(article_row(1, "first"))
		.with_fetch_all(vec![article_row(1, "first")])
		.with_fetch_one(article_row(2, "second"))
		.with_fetch_one(article_row(2, "second"));

	let created = Manager::<Article>::new()
		.create_with_conn(&mut executor, &original)
		.await
		.expect("manager create should use the supplied executor");
	assert_eq!(
		created,
		Article {
			id: Some(1),
			title: "first".to_string()
		}
	);

	let records = Manager::<Article>::new()
		.all()
		.all_with_db(&mut executor)
		.await
		.expect("queryset read should use the supplied executor");
	assert_eq!(records, vec![created.clone()]);

	let mut saved = Article {
		id: None,
		title: "second".to_string(),
	};
	saved
		.save_with_conn(&mut executor)
		.await
		.expect("model save should use the supplied executor");
	assert_eq!(
		saved,
		Article {
			id: Some(2),
			title: "second".to_string()
		}
	);

	let custom_created = ArticleManager::new()
		.create_with_conn(
			&mut executor,
			&Article {
				id: None,
				title: "second".to_string(),
			},
		)
		.await
		.expect("custom-manager default should forward the supplied executor");
	assert_eq!(
		custom_created,
		Article {
			id: Some(2),
			title: "second".to_string()
		}
	);

	assert_eq!(executor.calls.len(), 4);
	assert!(
		executor
			.calls
			.iter()
			.all(|call| call.sql.contains("articles"))
	);
	assert!(
		executor
			.calls
			.iter()
			.all(|call| !call.sql.contains("LAST_INSERT_ID"))
	);
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn concrete_connection_and_atomic_transaction_support_explicit_orm_paths() {
	let (_lease, mut connection) = sqlite_connection("sqlite::memory:").await;
	connection
		.execute(
			"CREATE TABLE articles (article_id INTEGER PRIMARY KEY AUTOINCREMENT, article_title TEXT NOT NULL)",
			vec![],
		)
		.await
		.expect("schema setup should succeed");

	let created = Manager::<Article>::new()
		.create_with_conn(
			&mut connection,
			&Article {
				id: None,
				title: "connection".to_string(),
			},
		)
		.await
		.expect("explicit manager API should accept DatabaseConnection");
	assert_eq!(created.id, Some(1));

	let affected = QuerySet::<Article>::new()
		.filter(Filter::new(
			"title",
			FilterOperator::Eq,
			FilterValue::String("connection".to_string()),
		))
		.update_fields_with_conn(
			&mut connection,
			[FieldAssignment::new(
				"title",
				UpdateValue::FieldRef(F::new("id")),
			)],
		)
		.await
		.expect("partial updates must resolve Rust field names to physical database columns");
	assert_eq!(affected, 1);
	let updated = Manager::<Article>::new()
		.get(1)
		.first_with_db(&mut connection)
		.await
		.expect("the updated article should be queryable")
		.expect("the updated article should still exist");
	assert_eq!(updated.title, "1");

	let (created_in_transaction, count_in_transaction) = connection
		.atomic(async |transaction| {
			let created = Manager::<Article>::new()
				.create_with_conn(
					transaction,
					&Article {
						id: None,
						title: "transaction".to_string(),
					},
				)
				.await?;
			let count = Manager::<Article>::new()
				.count_with_conn(transaction)
				.await?;
			Ok::<_, reinhardt_core::exception::Error>((created, count))
		})
		.await
		.expect("explicit manager APIs should accept AtomicTransaction");

	assert_eq!(created_in_transaction.id, Some(2));
	assert_eq!(count_in_transaction, 2);
	assert_eq!(
		Manager::<Article>::new()
			.count_with_conn(&mut connection)
			.await
			.unwrap(),
		2
	);
}

#[tokio::test]
async fn mysql_create_uses_exact_insert_result_and_physical_primary_key_reload() {
	let model = Article {
		id: None,
		title: "mysql".to_string(),
	};
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: Some(42),
		})
		.with_fetch_one(article_row(42, "mysql"));

	let created = Manager::<Article>::new()
		.create_with_conn(&mut executor, &model)
		.await
		.expect("MySQL create should reload through the same executor");

	assert_eq!(
		created,
		Article {
			id: Some(42),
			title: "mysql".to_string()
		}
	);
	assert_eq!(executor.calls.len(), 2);
	assert_eq!(executor.calls[0].kind, "execute");
	assert!(executor.calls[0].sql.starts_with("INSERT"));
	assert!(!executor.calls[0].sql.contains("RETURNING"));
	assert!(!executor.calls[0].sql.contains("LAST_INSERT_ID"));
	assert_eq!(executor.calls[1].kind, "fetch_one");
	assert!(executor.calls[1].sql.contains("article_id"));
	assert_eq!(executor.calls[1].params, vec![QueryValue::Int(42)]);
}

#[tokio::test]
async fn postgres_and_sqlite_create_render_the_supplied_executor_dialect() {
	for (backend, placeholder) in [
		(DatabaseBackend::Postgres, "$1"),
		(DatabaseBackend::Sqlite, "?"),
	] {
		let mut executor =
			RecordingExecutor::new(backend).with_fetch_one(article_row(11, "dialect"));

		let created = Manager::<Article>::new()
			.create_with_conn(
				&mut executor,
				&Article {
					id: None,
					title: "dialect".to_string(),
				},
			)
			.await
			.expect("create should use the recorder dialect");

		assert_eq!(created.id, Some(11));
		assert_eq!(executor.calls.len(), 1);
		assert_eq!(executor.calls[0].kind, "fetch_one");
		assert!(executor.calls[0].sql.contains(placeholder));
		assert!(executor.calls[0].sql.contains("RETURNING"));
	}
}

#[tokio::test]
async fn mysql_create_with_explicit_primary_key_ignores_generated_id_and_uses_db_column() {
	let model = Article {
		id: Some(7),
		title: "explicit".to_string(),
	};
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: Some(999),
		})
		.with_fetch_one(article_row(7, "explicit"));

	let created = Manager::<Article>::new()
		.create_with_conn(&mut executor, &model)
		.await
		.expect("explicit primary key create should still insert and reload");

	assert_eq!(created, model);
	assert!(executor.calls[0].sql.contains("INSERT"));
	assert!(executor.calls[0].sql.contains("article_id"));
	assert_eq!(executor.calls[1].params, vec![QueryValue::Int(7)]);
	assert!(executor.calls[1].sql.contains("article_id"));
}

#[tokio::test]
async fn mysql_generated_id_failures_are_unsupported_before_reload() {
	for generated_id in [None, Some(0), Some(i64::MAX as u64 + 1)] {
		let mut executor =
			RecordingExecutor::new(DatabaseBackend::MySql).with_execute_result(QueryResult {
				rows_affected: 1,
				last_insert_id: generated_id,
			});

		let error = Manager::<Article>::new()
			.create_with_conn(
				&mut executor,
				&Article {
					id: None,
					title: "missing-id".to_string(),
				},
			)
			.await
			.expect_err("a missing, zero, or overflowing generated ID must fail");

		assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Unsupported));
		assert_eq!(executor.calls.len(), 1);
		assert_eq!(executor.calls[0].kind, "execute");
	}
}

#[tokio::test]
async fn get_or_create_uses_the_caller_owned_executor() {
	let mut lookup_fields = HashMap::new();
	lookup_fields.insert("title".to_string(), "lookup".to_string());
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_optional(None)
		.with_fetch_one(article_row(9, "lookup"));

	let (article, created) = Manager::<Article>::new()
		.get_or_create_with_conn(&mut executor, lookup_fields, None)
		.await
		.expect("get_or_create should use the supplied executor");

	assert!(created);
	assert_eq!(article.id, Some(9));
	assert_eq!(executor.calls.len(), 2);
	assert_eq!(executor.calls[0].kind, "fetch_optional");
	assert_eq!(executor.calls[1].kind, "fetch_one");
	assert!(
		executor
			.calls
			.iter()
			.all(|call| call.sql.contains("articles"))
	);

	let mut defaults = HashMap::new();
	defaults.insert("id".to_string(), "007".to_string());
	let mut mysql_executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_optional(None)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_one(article_row(7, "lookup"));
	let mut mysql_lookup_fields = HashMap::new();
	mysql_lookup_fields.insert("title".to_string(), "lookup".to_string());
	let (article, created) = Manager::<Article>::new()
		.get_or_create_with_conn(&mut mysql_executor, mysql_lookup_fields, Some(defaults))
		.await
		.expect("an explicit MySQL primary key from defaults must bypass generated IDs");

	assert!(created);
	assert_eq!(article.id, Some(7));
	assert_eq!(mysql_executor.calls.len(), 3);
	assert_eq!(mysql_executor.calls[0].kind, "fetch_optional");
	assert_eq!(mysql_executor.calls[1].kind, "execute");
	assert_eq!(mysql_executor.calls[2].kind, "fetch_one");
	assert!(mysql_executor.calls[1].sql.contains("article_id"));
	assert!(mysql_executor.calls[2].sql.contains("article_id"));
	assert_eq!(
		mysql_executor.calls[2].params,
		vec![QueryValue::String("007".to_string())]
	);
}

#[tokio::test]
async fn mysql_get_or_create_with_physical_primary_key_defaults_bypasses_generated_id() {
	let mut lookup_fields = HashMap::new();
	lookup_fields.insert("title".to_string(), "physical-default".to_string());
	let mut defaults = HashMap::new();
	defaults.insert("article_id".to_string(), "007".to_string());
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_optional(None)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_one(article_row(7, "physical-default"));

	let (article, created) = Manager::<Article>::new()
		.get_or_create_with_conn(&mut executor, lookup_fields, Some(defaults))
		.await
		.expect("a physical primary key from defaults must bypass generated IDs");

	assert!(created);
	assert_eq!(article.id, Some(7));
	assert_eq!(executor.calls.len(), 3);
	assert_eq!(executor.calls[0].kind, "fetch_optional");
	assert_eq!(executor.calls[1].kind, "execute");
	assert_eq!(executor.calls[2].kind, "fetch_one");
	assert!(executor.calls[1].sql.contains("article_id"));
	assert!(executor.calls[2].sql.contains("article_id"));
	assert_eq!(
		executor.calls[2].params,
		vec![QueryValue::String("007".to_string())]
	);
}

#[tokio::test]
async fn mysql_get_or_create_with_physical_primary_key_lookup_bypasses_generated_id() {
	let mut lookup_fields = HashMap::new();
	lookup_fields.insert("article_id".to_string(), "007".to_string());
	lookup_fields.insert("title".to_string(), "physical-lookup".to_string());
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_optional(None)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: Some(0),
		})
		.with_fetch_one(article_row(7, "physical-lookup"));

	let (article, created) = Manager::<Article>::new()
		.get_or_create_with_conn(&mut executor, lookup_fields, None)
		.await
		.expect("a physical primary key from lookup fields must bypass generated IDs");

	assert!(created);
	assert_eq!(article.id, Some(7));
	assert_eq!(executor.calls.len(), 3);
	assert_eq!(executor.calls[0].kind, "fetch_optional");
	assert_eq!(executor.calls[1].kind, "execute");
	assert_eq!(executor.calls[2].kind, "fetch_one");
	assert!(executor.calls[0].sql.contains("article_id"));
	assert!(executor.calls[1].sql.contains("article_id"));
	assert!(executor.calls[2].sql.contains("article_id"));
	assert_eq!(
		executor.calls[2].params,
		vec![QueryValue::String("007".to_string())]
	);
}

#[tokio::test]
async fn mysql_get_or_create_coalesces_equal_primary_key_aliases_in_defaults() {
	let lookup_fields = HashMap::from([("title".to_string(), "alias-defaults".to_string())]);
	let defaults = HashMap::from([
		("id".to_string(), "007".to_string()),
		("article_id".to_string(), "007".to_string()),
	]);
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_optional(None)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_one(article_row(7, "alias-defaults"));

	let (article, created) = Manager::<Article>::new()
		.get_or_create_with_conn(&mut executor, lookup_fields, Some(defaults))
		.await
		.expect("equal primary-key aliases should coalesce before the insert");

	assert!(created);
	assert_eq!(article.id, Some(7));
	assert_eq!(executor.calls.len(), 3);
	assert_eq!(executor.calls[1].kind, "execute");
	assert_eq!(executor.calls[1].sql.matches("`article_id`").count(), 1);
	assert_eq!(executor.calls[2].kind, "fetch_one");
	assert_eq!(
		executor.calls[2].params,
		vec![QueryValue::String("007".to_string())]
	);
}

#[tokio::test]
async fn mysql_get_or_create_defaults_override_primary_key_aliases_across_maps() {
	let lookup_fields = HashMap::from([("id".to_string(), "001".to_string())]);
	let defaults = HashMap::from([("article_id".to_string(), "007".to_string())]);
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_optional(None)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_one(article_row(7, "alias-split"));

	let (article, created) = Manager::<Article>::new()
		.get_or_create_with_conn(&mut executor, lookup_fields, Some(defaults))
		.await
		.expect("defaults should override lookup values after primary-key canonicalization");

	assert!(created);
	assert_eq!(article.id, Some(7));
	assert_eq!(executor.calls.len(), 3);
	assert_eq!(executor.calls[0].kind, "fetch_optional");
	assert_eq!(
		executor.calls[0].params,
		vec![QueryValue::String("001".to_string())]
	);
	assert_eq!(executor.calls[1].kind, "execute");
	assert_eq!(executor.calls[1].sql.matches("`article_id`").count(), 1);
	assert_eq!(
		executor.calls[1].params,
		vec![QueryValue::String("007".to_string())]
	);
	assert_eq!(
		executor.calls[2].params,
		vec![QueryValue::String("007".to_string())]
	);
}

#[tokio::test]
async fn get_or_create_rejects_conflicting_primary_key_aliases_before_executor_use() {
	let lookup_fields = HashMap::from([("title".to_string(), "alias-conflict".to_string())]);
	let defaults = HashMap::from([
		("id".to_string(), "007".to_string()),
		("article_id".to_string(), "008".to_string()),
	]);
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql);

	let error = Manager::<Article>::new()
		.get_or_create_with_conn(&mut executor, lookup_fields, Some(defaults))
		.await
		.expect_err("conflicting logical and physical primary-key aliases must be rejected");

	assert!(matches!(error, Error::Validation(_)));
	assert!(executor.calls.is_empty());
}

#[test]
fn get_or_create_sql_builders_coalesce_equal_primary_key_aliases() {
	let manager = Manager::<Article>::new();
	let lookup_fields = HashMap::from([("title".to_string(), "alias-builder".to_string())]);
	let defaults = HashMap::from([
		("id".to_string(), "007".to_string()),
		("article_id".to_string(), "007".to_string()),
	]);

	let (_, manager_insert) = manager
		.get_or_create_sql(&lookup_fields, &defaults, DatabaseBackend::MySql)
		.expect("the manager SQL builder should normalize equal primary-key aliases");
	let _ = manager
		.get_or_create_queries(&lookup_fields, &defaults)
		.expect("the manager statement builder should normalize equal primary-key aliases");
	let (_, custom_manager_insert) = CustomManager::get_or_create_sql(
		&manager,
		&lookup_fields,
		&defaults,
		DatabaseBackend::MySql,
	)
	.expect("the CustomManager SQL builder should normalize equal primary-key aliases");
	let _ = CustomManager::get_or_create_queries(&manager, &lookup_fields, &defaults)
		.expect("the CustomManager statement builder should normalize equal primary-key aliases");

	assert_eq!(manager_insert.matches("`article_id`").count(), 1);
	assert_eq!(custom_manager_insert.matches("`article_id`").count(), 1);
	assert_eq!(manager_insert, custom_manager_insert);
}

#[test]
fn get_or_create_sql_builders_reject_conflicting_primary_key_aliases() {
	let manager = Manager::<Article>::new();
	let lookup_fields = HashMap::new();
	let defaults = HashMap::from([
		("id".to_string(), "007".to_string()),
		("article_id".to_string(), "008".to_string()),
	]);

	let manager_error = manager
		.get_or_create_queries(&lookup_fields, &defaults)
		.err()
		.expect("the manager statement builder should reject conflicting aliases");
	let manager_sql_error = manager
		.get_or_create_sql(&lookup_fields, &defaults, DatabaseBackend::MySql)
		.err()
		.expect("the manager SQL builder should reject conflicting aliases");
	let custom_manager_query_error =
		CustomManager::get_or_create_queries(&manager, &lookup_fields, &defaults)
			.err()
			.expect("the CustomManager statement builder should reject conflicting aliases");
	let custom_manager_error = CustomManager::get_or_create_sql(
		&manager,
		&lookup_fields,
		&defaults,
		DatabaseBackend::MySql,
	)
	.err()
	.expect("the CustomManager SQL builder should reject conflicting aliases");

	assert!(matches!(manager_error, Error::Validation(_)));
	assert!(matches!(manager_sql_error, Error::Validation(_)));
	assert!(matches!(custom_manager_query_error, Error::Validation(_)));
	assert!(matches!(custom_manager_error, Error::Validation(_)));
}

#[test]
fn metadata_free_primary_key_override_is_used_by_get_or_create_sql_builders() {
	let manager = Manager::<MetadataFreeArticle>::new();
	let lookup_fields = HashMap::from([("title".to_string(), "manual-builder".to_string())]);
	let defaults = HashMap::from([("article_id".to_string(), "007".to_string())]);

	let (_, manager_insert) = manager
		.get_or_create_sql(&lookup_fields, &defaults, DatabaseBackend::MySql)
		.expect("the manager SQL builder should support a metadata-free primary-key override");
	let (_, custom_manager_insert) = CustomManager::get_or_create_sql(
		&manager,
		&lookup_fields,
		&defaults,
		DatabaseBackend::MySql,
	)
	.expect("the CustomManager SQL builder should support a metadata-free primary-key override");

	assert!(manager_insert.contains("`article_id`"));
	assert!(!manager_insert.contains("`id`"));
	assert_eq!(manager_insert, custom_manager_insert);
}

#[tokio::test]
async fn metadata_free_primary_key_override_is_used_by_mysql_get_or_create() {
	let manager = Manager::<MetadataFreeArticle>::new();
	let lookup_fields = HashMap::from([("title".to_string(), "manual-runtime".to_string())]);
	let defaults = Some(HashMap::from([(
		"article_id".to_string(),
		"007".to_string(),
	)]));
	let mut manager_executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_optional(None)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_one(metadata_free_article_row(7, "manual-runtime"));
	let mut custom_manager_executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_optional(None)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_one(metadata_free_article_row(7, "manual-runtime"));

	let (manager_article, manager_created) = manager
		.get_or_create_with_conn(
			&mut manager_executor,
			lookup_fields.clone(),
			defaults.clone(),
		)
		.await
		.expect("the manager runtime should support a metadata-free primary-key override");
	let (custom_manager_article, custom_manager_created) = CustomManager::get_or_create_with_conn(
		&manager,
		&mut custom_manager_executor,
		lookup_fields,
		defaults,
	)
	.await
	.expect("the CustomManager runtime should support a metadata-free primary-key override");

	assert!(manager_created);
	assert!(custom_manager_created);
	assert_eq!(manager_article.id, Some(7));
	assert_eq!(custom_manager_article.id, Some(7));
	assert!(manager_executor.calls[1].sql.contains("`article_id`"));
	assert!(!manager_executor.calls[1].sql.contains("`id`"));
	assert!(
		custom_manager_executor.calls[1]
			.sql
			.contains("`article_id`")
	);
	assert!(!custom_manager_executor.calls[1].sql.contains("`id`"));
	assert_eq!(
		manager_executor.calls[2].params,
		vec![QueryValue::String("007".to_string())]
	);
	assert_eq!(
		custom_manager_executor.calls[2].params,
		vec![QueryValue::String("007".to_string())]
	);
}

#[tokio::test]
async fn custom_manager_explicit_terminals_preserve_executor_borrows_and_hooks() {
	let manager = ExecutorBorrowingArticleManager::new();
	let mut lookup_fields = HashMap::new();
	lookup_fields.insert("title".to_string(), "custom-lookup".to_string());
	let mut get_or_create_executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_optional(None)
		.with_fetch_one(article_row(31, "custom-lookup"));

	let (article, created) = manager
		.get_or_create_with_conn(&mut get_or_create_executor, lookup_fields, None)
		.await
		.expect("an explicit CustomManager override must retain the executor across await");
	assert!(created);
	assert_eq!(article.id, Some(31));
	assert_eq!(get_or_create_executor.calls.len(), 2);

	let mut bulk_create_executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		});
	let created_models = manager
		.bulk_create_with_conn(
			&mut bulk_create_executor,
			vec![Article {
				id: None,
				title: "bulk-created".to_string(),
			}],
			None,
			false,
			false,
		)
		.await
		.expect("custom bulk_create should delegate through the supplied executor");
	assert_eq!(created_models.len(), 1);
	assert_eq!(bulk_create_executor.calls.len(), 1);
	assert_eq!(bulk_create_executor.calls[0].kind, "execute");

	let mut bulk_update_executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		});
	let updated = manager
		.bulk_update_with_conn(
			&mut bulk_update_executor,
			vec![Article {
				id: Some(31),
				title: "bulk-updated".to_string(),
			}],
			vec!["title".to_string()],
			None,
		)
		.await
		.expect("custom bulk_update should retain its hook on the explicit path");
	assert_eq!(updated, 1);
	assert_eq!(manager.bulk_update_calls.load(Ordering::SeqCst), 1);
	assert_eq!(bulk_update_executor.calls.len(), 1);
	assert!(
		bulk_update_executor.calls[0]
			.sql
			.contains("hooked-bulk-updated")
	);
}

#[tokio::test]
async fn explicit_manager_crud_and_queryset_terminals_use_one_recorder() {
	let manager = Manager::<Article>::new();
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_one(article_row(3, "created"))
		.with_fetch_one(article_row(3, "updated"))
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(4));
			row
		})
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(2));
			row
		})
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(1));
			row
		})
		.with_fetch_all(vec![article_row(3, "updated")])
		.with_fetch_all(vec![article_row(3, "updated")])
		.with_fetch_all(vec![article_row(3, "updated")])
		.with_fetch_one(article_row(4, "queryset-created"))
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		});

	let created = manager
		.create_with_conn(
			&mut executor,
			&Article {
				id: Some(3),
				title: "created".to_string(),
			},
		)
		.await
		.expect("create should use the recorder");
	let updated = manager
		.update_with_conn(
			&mut executor,
			&Article {
				id: Some(3),
				title: "updated".to_string(),
			},
		)
		.await
		.expect("update should use the recorder");
	assert_eq!(created.id, Some(3));
	assert_eq!(updated.title, "updated");
	assert_eq!(manager.count_with_conn(&mut executor).await.unwrap(), 4);

	let queryset = manager.filter(Filter::new(
		"title",
		FilterOperator::Eq,
		FilterValue::String("updated".to_string()),
	));
	let updates = HashMap::from([("title".to_string(), UpdateValue::FieldRef(F::new("id")))]);
	let (update_sql, update_params) = queryset
		.update_sql(&updates)
		.expect("update SQL should compile");
	assert_eq!(
		update_sql,
		"UPDATE \"articles\" SET \"article_title\" = \"article_id\" WHERE \"article_title\" = $1"
	);
	assert_eq!(update_params, vec!["updated"]);
	assert_eq!(queryset.count_with_db(&mut executor).await.unwrap(), 2);
	assert!(queryset.exists_with_db(&mut executor).await.unwrap());
	assert_eq!(queryset.get_with_db(&mut executor).await.unwrap(), updated);
	assert_eq!(
		queryset.first_with_db(&mut executor).await.unwrap(),
		Some(updated.clone())
	);
	assert_eq!(
		queryset.all_with_db(&mut executor).await.unwrap(),
		vec![updated.clone()]
	);
	assert_eq!(
		queryset
			.create_with_conn(
				&mut executor,
				Article {
					id: None,
					title: "queryset-created".to_string(),
				},
			)
			.await
			.unwrap()
			.id,
		Some(4)
	);
	assert_eq!(
		queryset
			.update_fields_with_conn(
				&mut executor,
				[FieldAssignment::new(
					"title",
					UpdateValue::FieldRef(F::new("id")),
				)],
			)
			.await
			.unwrap(),
		1
	);
	manager.delete_with_conn(&mut executor, 3).await.unwrap();

	assert_eq!(executor.calls.len(), 11);
	assert!(
		executor
			.calls
			.iter()
			.all(|call| call.sql.contains("articles"))
	);
	assert!(executor.calls.iter().any(|call| call.kind == "execute"));
	assert!(executor.calls.iter().any(|call| call.kind == "fetch_all"));
	assert_eq!(
		executor.calls[9].sql,
		"UPDATE \"articles\" SET \"article_title\" = \"article_id\" WHERE \"article_title\" = $1"
	);
}

#[test]
fn queryset_write_expressions_resolve_model_fields_without_rewriting_explicit_references() {
	let queryset = QuerySet::<Article>::new().filter(Filter::new(
		"title",
		FilterOperator::Eq,
		FilterValue::String("updated".to_string()),
	));

	let (expression_sql, expression_params) = queryset
		.update_fields_sql([FieldAssignment::new(
			"title",
			UpdateValue::Expression(Expression::Coalesce(vec![
				AnnotationValue::Field(F::new("title")),
				AnnotationValue::Value(Value::String("fallback".to_string())),
			])),
		)])
		.expect("expression updates should compile");
	assert_eq!(
		expression_sql,
		"UPDATE \"articles\" SET \"article_title\" = COALESCE(\"article_title\", 'fallback') WHERE \"article_title\" = $1"
	);
	assert_eq!(expression_params, vec!["updated"]);

	let explicit_physical_queryset = QuerySet::<Article>::new().filter(Filter::new(
		"article_title",
		FilterOperator::Eq,
		FilterValue::String("updated".to_string()),
	));
	let (physical_sql, physical_params) = explicit_physical_queryset
		.update_fields_sql([FieldAssignment::new(
			"title",
			UpdateValue::FieldRef(F::new("article_id")),
		)])
		.expect("physical field references should compile");
	assert_eq!(
		physical_sql,
		"UPDATE \"articles\" SET \"article_title\" = \"article_id\" WHERE \"article_title\" = $1"
	);
	assert_eq!(physical_params, vec!["updated"]);

	let (qualified_sql, qualified_params) = queryset
		.update_fields_sql([FieldAssignment::new(
			"title",
			UpdateValue::FieldRef(F::new("other.id")),
		)])
		.expect("qualified field references should compile");
	assert_eq!(
		qualified_sql,
		"UPDATE \"articles\" SET \"article_title\" = \"other\".\"id\" WHERE \"article_title\" = $1"
	);
	assert_eq!(qualified_params, vec!["updated"]);
}

#[tokio::test]
async fn queryset_count_and_exists_record_orm_instrumentation() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(2));
			row
		})
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(1));
			row
		});
	let queryset = QuerySet::<Article>::new().filter(Filter::new(
		"article_id",
		FilterOperator::Eq,
		FilterValue::Integer(7),
	));
	let mut config = NPlusOneConfig::default();
	config.threshold = usize::MAX;

	let (result, report) = NPlusOneScope::warn("queryset-count-and-exists", config)
		.run_with_report(async {
			let count = queryset.count_with_db(&mut executor).await?;
			let exists = queryset.exists_with_db(&mut executor).await?;
			Ok::<_, reinhardt_core::exception::Error>((count, exists))
		})
		.await;
	let (count, exists) = result.expect("count and exists should use the supplied executor");

	assert_eq!(count, 2);
	assert!(exists);
	assert_eq!(report.total_recorded_queries, 2);
	assert_eq!(executor.calls.len(), 2);
	assert!(executor.calls.iter().all(|call| call.kind == "fetch_one"));
}

#[tokio::test]
async fn custom_manager_override_and_model_veto_preserve_caller_owned_executor() {
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_one(article_row(8, "custom-draft"));
	let created = PrefixingArticleManager::new()
		.create_with_conn(
			&mut executor,
			&Article {
				id: None,
				title: "draft".to_string(),
			},
		)
		.await
		.expect("custom-manager override should retain the supplied executor");
	assert_eq!(created.title, "custom-draft");
	assert!(
		executor.calls[0]
			.params
			.contains(&QueryValue::String("custom-draft".to_string()))
	);

	let registry = Arc::new(EventRegistry::new());
	registry.register_mapper_listener("articles".to_string(), Arc::new(VetoInsert));
	let _guard = set_active_registry(registry);
	let mut vetoed = Article {
		id: None,
		title: "blocked".to_string(),
	};
	let mut veto_executor = RecordingExecutor::new(DatabaseBackend::Postgres);
	let error = vetoed
		.save_with_conn(&mut veto_executor)
		.await
		.expect_err("a vetoed event must stop before SQL execution");
	assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Query));
	assert!(veto_executor.calls.is_empty());
}

#[tokio::test]
async fn model_events_wrap_the_same_executor_backed_insert() {
	let events = Arc::new(Mutex::new(Vec::new()));
	let registry = Arc::new(EventRegistry::new());
	registry.register_mapper_listener(
		"articles".to_string(),
		Arc::new(OrderedInsertEvents {
			events: Arc::clone(&events),
		}),
	);
	let _guard = set_active_registry(registry);
	let mut executor =
		RecordingExecutor::new(DatabaseBackend::Postgres).with_fetch_one(article_row(15, "events"));
	let mut article = Article {
		id: None,
		title: "events".to_string(),
	};

	article
		.save_with_conn(&mut executor)
		.await
		.expect("eventful save should use the caller-owned executor");

	assert_eq!(article.id, Some(15));
	assert_eq!(
		*events
			.lock()
			.expect("event log mutex should not be poisoned"),
		vec!["before_insert", "after_insert"]
	);
	assert_eq!(executor.calls.len(), 1);
	assert_eq!(executor.calls[0].kind, "fetch_one");
}

#[tokio::test]
async fn queryset_composite_lookup_uses_the_caller_owned_executor() {
	let mut primary_key = HashMap::new();
	primary_key.insert("article_id".to_string(), PkValue::Int(17));
	primary_key.insert("locale".to_string(), PkValue::String("ja".to_string()));
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_all(vec![article_locale_row(17, "ja", "localized")]);

	let article = QuerySet::<ArticleLocale>::new()
		.get_composite_with_db(&mut executor, &primary_key)
		.await
		.expect("composite lookup should use the supplied executor");

	assert_eq!(article.article_id, 17);
	assert_eq!(article.locale, "ja");
	assert_eq!(executor.calls.len(), 1);
	assert_eq!(executor.calls[0].kind, "fetch_all");
	assert!(executor.calls[0].sql.contains("article_locales"));
}

#[tokio::test]
async fn model_save_delete_and_bulk_update_use_renamed_primary_key_on_one_executor() {
	let mut article = Article {
		id: Some(12),
		title: "before".to_string(),
	};
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_fetch_one(article_row(12, "after"))
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		});

	article
		.save_with_conn(&mut executor)
		.await
		.expect("model update save should use the recorder");
	assert_eq!(article.title, "after");
	article
		.delete_with_conn(&mut executor)
		.await
		.expect("model delete should use the recorder");
	let updated = Manager::<Article>::new()
		.bulk_update_with_conn(
			&mut executor,
			vec![article.clone()],
			vec!["title".to_string()],
			None,
		)
		.await
		.expect("bulk update should use the recorder");

	assert_eq!(updated, 1);
	assert_eq!(executor.calls.len(), 3);
	assert!(executor.calls[0].sql.starts_with("UPDATE"));
	assert!(executor.calls[1].sql.starts_with("DELETE"));
	assert!(
		executor
			.calls
			.iter()
			.all(|call| call.sql.contains("article_id"))
	);
	assert!(executor.calls[2].sql.contains("article_title"));
	assert_eq!(executor.calls[0].kind, "fetch_one");
	assert_eq!(executor.calls[1].kind, "execute");
	assert_eq!(executor.calls[2].kind, "execute");
}

#[tokio::test]
async fn bulk_update_uses_the_executor_dialect_and_database_column_names() {
	let mut executor =
		RecordingExecutor::new(DatabaseBackend::MySql).with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		});

	let updated = Manager::<Article>::new()
		.bulk_update_with_conn(
			&mut executor,
			vec![Article {
				id: Some(21),
				title: "mysql-title".to_string(),
			}],
			vec!["title".to_string()],
			None,
		)
		.await
		.expect("bulk update should render against the caller-owned MySQL executor");

	assert_eq!(updated, 1);
	assert_eq!(executor.calls.len(), 1);
	assert_eq!(executor.calls[0].kind, "execute");
	assert!(executor.calls[0].sql.contains("`articles`"));
	assert!(executor.calls[0].sql.contains("`article_id`"));
	assert!(executor.calls[0].sql.contains("`article_title`"));
	assert!(!executor.calls[0].sql.contains("\"article_title\""));
}

#[tokio::test]
async fn query_execution_uses_the_executor_dialect_and_query_row_decode_path() {
	let stmt = Query::select().from(Alias::new("articles")).to_owned();
	let execution = SelectExecution::<Article>::new(stmt);
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_one(article_row(5, "get"))
		.with_fetch_all(vec![article_row(5, "all")])
		.with_fetch_optional(Some(article_row(5, "first")))
		.with_fetch_all(vec![article_row(5, "one")])
		.with_fetch_all(vec![article_row(5, "one-or-none")])
		.with_fetch_all({
			let mut row = Row::new();
			row.insert(
				"value".to_string(),
				QueryValue::String("scalar".to_string()),
			);
			vec![row]
		})
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(6));
			row
		})
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("exists".to_string(), QueryValue::Bool(true));
			row
		});

	assert_eq!(
		execution.get_async(&mut executor, &5).await.unwrap().title,
		"get"
	);
	assert_eq!(
		execution.all_async(&mut executor).await.unwrap()[0].title,
		"all"
	);
	assert_eq!(
		execution
			.first_async(&mut executor)
			.await
			.unwrap()
			.unwrap()
			.title,
		"first"
	);
	assert_eq!(
		execution.one_async(&mut executor).await.unwrap().title,
		"one"
	);
	assert_eq!(
		execution
			.one_or_none_async(&mut executor)
			.await
			.unwrap()
			.unwrap()
			.title,
		"one-or-none"
	);
	assert_eq!(
		execution
			.scalar_async::<String, _>(&mut executor)
			.await
			.unwrap(),
		Some("scalar".to_string())
	);
	assert_eq!(execution.count_async(&mut executor).await.unwrap(), 6);
	assert!(execution.exists_async(&mut executor).await.unwrap());
	assert_eq!(executor.calls.len(), 8);
	assert!(executor.calls[0].sql.contains('?'));
	assert!(executor.calls.iter().all(|call| !call.sql.contains("$1")));
	assert_eq!(executor.calls[2].kind, "fetch_optional");
	assert_eq!(executor.fetch_optional_contexts, vec![None]);
}

#[tokio::test]
async fn bulk_create_never_uses_mysql_first_insert_id_for_each_model() {
	let models = vec![
		Article {
			id: None,
			title: "one".to_string(),
		},
		Article {
			id: None,
			title: "two".to_string(),
		},
	];
	let mut executor =
		RecordingExecutor::new(DatabaseBackend::MySql).with_execute_result(QueryResult {
			rows_affected: 2,
			last_insert_id: Some(100),
		});

	let created = Manager::<Article>::new()
		.bulk_create_with_conn(&mut executor, models.clone(), None, false, false)
		.await
		.expect("bulk create should not infer every generated ID from one result");

	assert_eq!(created, models);
	assert_eq!(executor.calls.len(), 1);
	assert_eq!(executor.calls[0].kind, "execute");
	assert!(!executor.calls[0].sql.contains("LAST_INSERT_ID"));
}

#[tokio::test]
async fn many_to_many_manager_terminals_use_the_caller_owned_executor() {
	let manager = ManyToManyManager::<(), (), i64>::new(
		1,
		"article_members".to_string(),
		"article_id".to_string(),
		"member_id".to_string(),
	);
	let mut executor = RecordingExecutor::new(DatabaseBackend::Postgres)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_all(vec![Row::new()])
		.with_fetch_all(vec![article_row(2, "related")])
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(1));
			row
		});

	manager
		.add_with_db(&mut executor, 2)
		.await
		.expect("add must use the supplied executor");
	manager
		.remove_with_db(&mut executor, 2)
		.await
		.expect("remove must use the supplied executor");
	assert!(
		manager
			.contains_with_db(&mut executor, 2)
			.await
			.expect("contains must use the supplied executor")
	);
	let related = manager
		.all_with_db(&mut executor, "articles", "article_id")
		.await
		.expect("all must use the supplied executor");
	assert_eq!(related.len(), 1);
	assert_eq!(related[0].get::<i64>("article_id"), Some(2));
	manager
		.clear_with_db(&mut executor)
		.await
		.expect("clear must use the supplied executor");
	assert_eq!(
		manager
			.count_with_db(&mut executor)
			.await
			.expect("count must use the supplied executor"),
		1
	);

	assert_eq!(executor.calls.len(), 6);
	assert_eq!(
		executor
			.calls
			.iter()
			.map(|call| call.kind)
			.collect::<Vec<_>>(),
		vec![
			"execute",
			"execute",
			"fetch_all",
			"fetch_all",
			"execute",
			"fetch_one",
		]
	);
	assert!(executor.calls.iter().all(|call| call.sql.contains("$1")));
	assert_eq!(
		executor.calls[0].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"manager add must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[1].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"manager remove must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[2].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"manager contains must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[3].params,
		vec![QueryValue::Int(1)],
		"manager all must preserve the source primary-key binder"
	);
	assert_eq!(
		executor.calls[4].params,
		vec![QueryValue::Int(1)],
		"manager clear must preserve the source primary-key binder"
	);
	assert_eq!(
		executor.calls[5].params,
		vec![QueryValue::Int(1)],
		"manager count must preserve the source primary-key binder"
	);
}

#[tokio::test]
async fn many_to_many_marker_terminals_accept_a_mutable_executor() {
	let marker = ManyToManyField::<MarkerSource, MarkerTarget>::new();
	let config = || {
		ManyToManyConfig::new(
			1,
			"article_members".to_string(),
			"article_id".to_string(),
			"member_id".to_string(),
		)
	};
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_all(vec![Row::new()])
		.with_fetch_all(vec![article_row(2, "related")])
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(1));
			row
		});

	marker
		.add_with_db(&mut executor, config(), 2)
		.await
		.expect("marker add must accept the supplied executor");
	marker
		.remove_with_db(&mut executor, config(), 2)
		.await
		.expect("marker remove must accept the supplied executor");
	assert!(
		marker
			.contains_with_db(&mut executor, config(), 2)
			.await
			.expect("marker contains must accept the supplied executor")
	);
	assert_eq!(
		marker
			.all_with_db(&mut executor, config(), "articles", "article_id")
			.await
			.expect("marker all must accept the supplied executor")
			.len(),
		1
	);
	marker
		.clear_with_db(&mut executor, config())
		.await
		.expect("marker clear must accept the supplied executor");
	assert_eq!(
		marker
			.count_with_db(&mut executor, config())
			.await
			.expect("marker count must accept the supplied executor"),
		1
	);

	assert_eq!(executor.calls.len(), 6);
	assert!(executor.calls.iter().all(|call| call.sql.contains('`')));
	assert!(executor.calls.iter().all(|call| call.sql.contains('?')));
	assert_eq!(
		executor.calls[0].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"marker add must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[1].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"marker remove must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[2].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"marker contains must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[3].params,
		vec![QueryValue::Int(1)],
		"marker all must preserve the source primary-key binder"
	);
	assert_eq!(
		executor.calls[4].params,
		vec![QueryValue::Int(1)],
		"marker clear must preserve the source primary-key binder"
	);
	assert_eq!(
		executor.calls[5].params,
		vec![QueryValue::Int(1)],
		"marker count must preserve the source primary-key binder"
	);
}

#[tokio::test]
async fn many_to_many_accessor_terminals_share_one_executor_without_starting_a_transaction() {
	let source = Article {
		id: Some(1),
		title: "source".to_string(),
	};
	let first_target = Article {
		id: Some(2),
		title: "first".to_string(),
	};
	let second_target = Article {
		id: Some(3),
		title: "second".to_string(),
	};
	let accessor = ManyToManyAccessor::<Article, Article>::new(&source, "related");
	let mut executor = RecordingExecutor::new(DatabaseBackend::Sqlite)
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_execute_result(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
		.with_fetch_all(vec![Row::new()])
		.with_fetch_all(vec![article_row(2, "first")])
		.with_fetch_all({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(1));
			vec![row]
		});

	accessor
		.add_with_conn(&mut executor, &first_target)
		.await
		.expect("add must use the caller-owned executor");
	accessor
		.remove_with_conn(&mut executor, &first_target)
		.await
		.expect("remove must use the caller-owned executor");
	assert!(
		accessor
			.contains_with_conn(&mut executor, &first_target)
			.await
			.expect("contains must use the caller-owned executor")
	);
	assert_eq!(
		accessor
			.all_with_conn(&mut executor)
			.await
			.expect("all must use the caller-owned executor"),
		vec![first_target.clone()]
	);
	accessor
		.clear_with_conn(&mut executor)
		.await
		.expect("clear must use the caller-owned executor");
	assert_eq!(
		accessor
			.count_with_conn(&mut executor)
			.await
			.expect("count must use the caller-owned executor"),
		1
	);
	accessor
		.set_with_conn(&mut executor, &[first_target.clone(), second_target])
		.await
		.expect("set must use the caller-owned executor without beginning its own transaction");

	assert_eq!(executor.calls.len(), 9);
	assert_eq!(
		executor
			.calls
			.iter()
			.map(|call| call.kind)
			.collect::<Vec<_>>(),
		vec![
			"execute",
			"execute",
			"fetch_all",
			"fetch_all",
			"execute",
			"fetch_all",
			"execute",
			"execute",
			"execute",
		]
	);
	assert!(executor.calls.iter().all(|call| call.sql.contains('?')));
	assert!(
		executor
			.calls
			.iter()
			.all(|call| !call.sql.contains("BEGIN") && !call.sql.contains("COMMIT"))
	);
	assert!(executor.calls[3].sql.contains("article_id"));
	assert_eq!(
		executor.calls[0].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"add must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[1].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"remove must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[2].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"contains must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[4].params,
		vec![QueryValue::Int(1)],
		"clear must preserve the source primary-key binder"
	);
	assert_eq!(
		executor.calls[6].params,
		vec![QueryValue::Int(1)],
		"set's clear phase must preserve the source primary-key binder"
	);
	assert_eq!(
		executor.calls[7].params,
		vec![QueryValue::Int(1), QueryValue::Int(2)],
		"set's first add phase must preserve integer primary-key binders"
	);
	assert_eq!(
		executor.calls[8].params,
		vec![QueryValue::Int(1), QueryValue::Int(3)],
		"set's second add phase must preserve integer primary-key binders"
	);
}

#[tokio::test]
async fn many_to_many_filter_by_target_uses_the_executor_and_physical_primary_key_columns() {
	let target = Article {
		id: Some(2),
		title: "target".to_string(),
	};
	let manager = Manager::<Article>::new();
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_all(vec![article_row(1, "source")]);

	let sources = ManyToManyAccessor::<Article, Article>::filter_by_target_with_conn(
		&manager,
		"related",
		&target,
		&mut executor,
	)
	.await
	.expect("filtering by a target must use the caller-owned executor");

	assert_eq!(sources.len(), 1);
	assert_eq!(sources[0].id, Some(1));
	assert_eq!(executor.calls.len(), 1);
	assert_eq!(executor.calls[0].kind, "fetch_all");
	assert!(executor.calls[0].sql.contains("`article_id`"));
	assert!(executor.calls[0].sql.contains('?'));
}

#[tokio::test]
async fn reverse_and_generic_relation_terminals_accept_a_mutable_executor() {
	let source = Article {
		id: Some(1),
		title: "source".to_string(),
	};
	let reverse = ForeignKeyAccessor::<Article, Article>::new("article_id").reverse(&source);
	let generic = GenericRelationSet::<Article>::new(4, 1, "content_type_id", "object_id");
	let mut executor = RecordingExecutor::new(DatabaseBackend::MySql)
		.with_fetch_all(vec![article_row(2, "reverse")])
		.with_fetch_all({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(1));
			vec![row]
		})
		.with_fetch_all(vec![article_row(3, "generic")])
		.with_fetch_all(vec![article_row(3, "generic-first")])
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(1));
			row
		})
		.with_fetch_one({
			let mut row = Row::new();
			row.insert("count".to_string(), QueryValue::Int(1));
			row
		});

	assert_eq!(
		reverse
			.all_with_conn(&mut executor)
			.await
			.expect("reverse all must use the caller-owned executor")[0]
			.title,
		"reverse"
	);
	assert_eq!(
		reverse
			.count_with_conn(&mut executor)
			.await
			.expect("reverse count must use the caller-owned executor"),
		1
	);
	assert_eq!(
		generic
			.all_with_db(&mut executor)
			.await
			.expect("generic relation all must use the caller-owned executor")[0]
			.title,
		"generic"
	);
	assert_eq!(
		generic
			.first_with_db(&mut executor)
			.await
			.expect("generic relation first must use the caller-owned executor")
			.expect("the recorder should return a first related model")
			.title,
		"generic-first"
	);
	assert_eq!(
		generic
			.count_with_db(&mut executor)
			.await
			.expect("generic relation count must use the caller-owned executor"),
		1
	);
	assert!(
		generic
			.exists_with_db(&mut executor)
			.await
			.expect("generic relation exists must use the caller-owned executor")
	);

	assert_eq!(executor.calls.len(), 6);
	assert!(executor.calls.iter().all(|call| call.sql.contains('`')));
	assert!(
		executor.calls[..2]
			.iter()
			.all(|call| call.sql.contains('?'))
	);
}
