use futures::{Stream, StreamExt};
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind};
use reinhardt_db::backends::types::{
	DatabaseType, QueryResult, QueryValue, Row, RowStream, TransactionExecutor,
};
#[cfg(feature = "sqlite")]
use reinhardt_db::orm::DatabaseConnectionLease;
use reinhardt_db::orm::{Model, QuerySet};
use reinhardt_query::prelude::{
	Alias, ColumnDef, Expr, Query, QueryStatementBuilder, SqliteQueryBuilder,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{
	Arc,
	atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::task::{Context, Poll};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct StreamedArticle {
	id: Option<i64>,
	title: String,
}

#[derive(Clone)]
struct StreamedArticleFields;

impl reinhardt_db::orm::model::FieldSelector for StreamedArticleFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for StreamedArticle {
	type PrimaryKey = i64;
	type Fields = StreamedArticleFields;
	type Objects = reinhardt_db::orm::Manager<Self>;

	fn table_name() -> &'static str {
		"streamed_articles"
	}

	fn new_fields() -> Self::Fields {
		StreamedArticleFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

fn article_row(id: i64, title: QueryValue) -> Row {
	let mut row = Row::new();
	row.insert("id".to_owned(), QueryValue::Int(id));
	row.insert("title".to_owned(), title);
	row
}

struct BoundedMockStream {
	source: VecDeque<reinhardt_core::exception::Result<Row>>,
	buffer: VecDeque<reinhardt_core::exception::Result<Row>>,
	bound: usize,
	pending_when_empty: bool,
	max_buffered: Arc<AtomicUsize>,
	dropped: Arc<AtomicBool>,
}

impl Stream for BoundedMockStream {
	type Item = reinhardt_core::exception::Result<Row>;

	fn poll_next(mut self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		if self.buffer.is_empty() {
			while self.buffer.len() < self.bound {
				let Some(row) = self.source.pop_front() else {
					break;
				};
				self.buffer.push_back(row);
			}
			self.max_buffered
				.fetch_max(self.buffer.len(), Ordering::SeqCst);
		}
		if self.buffer.is_empty() && self.pending_when_empty {
			return Poll::Pending;
		}
		Poll::Ready(self.buffer.pop_front())
	}
}

impl Drop for BoundedMockStream {
	fn drop(&mut self) {
		self.dropped.store(true, Ordering::SeqCst);
	}
}

struct StreamingExecutor {
	rows: VecDeque<reinhardt_core::exception::Result<Row>>,
	stream_calls: usize,
	pending_when_empty: bool,
	max_buffered: Arc<AtomicUsize>,
	dropped: Arc<AtomicBool>,
}

impl StreamingExecutor {
	fn new(rows: Vec<reinhardt_core::exception::Result<Row>>) -> Self {
		Self {
			rows: rows.into(),
			stream_calls: 0,
			pending_when_empty: false,
			max_buffered: Arc::new(AtomicUsize::new(0)),
			dropped: Arc::new(AtomicBool::new(false)),
		}
	}

	fn pending_after(rows: Vec<reinhardt_core::exception::Result<Row>>) -> Self {
		Self {
			pending_when_empty: true,
			..Self::new(rows)
		}
	}
}

#[async_trait::async_trait]
impl TransactionExecutor for StreamingExecutor {
	fn backend(&self) -> DatabaseType {
		DatabaseType::Sqlite
	}

	async fn execute(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		panic!("streaming QuerySet test does not execute mutations")
	}

	async fn fetch_one(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Row> {
		panic!("streaming QuerySet test does not fetch one row")
	}

	async fn fetch_all(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		panic!("streaming QuerySet must never call fetch_all")
	}

	fn fetch_stream<'a>(
		&'a mut self,
		_sql: String,
		_params: Vec<QueryValue>,
		chunk_size: usize,
	) -> reinhardt_core::exception::Result<RowStream<'a>> {
		self.stream_calls += 1;
		Ok(Box::pin(BoundedMockStream {
			source: std::mem::take(&mut self.rows),
			buffer: VecDeque::new(),
			bound: chunk_size,
			pending_when_empty: self.pending_when_empty,
			max_buffered: Arc::clone(&self.max_buffered),
			dropped: Arc::clone(&self.dropped),
		}))
	}

	async fn fetch_optional(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		panic!("streaming QuerySet test does not fetch an optional row")
	}

	async fn commit(self: Box<Self>) -> reinhardt_core::exception::Result<()> {
		Ok(())
	}

	async fn rollback(self: Box<Self>) -> reinhardt_core::exception::Result<()> {
		Ok(())
	}
}

struct UnsupportedExecutor;

#[async_trait::async_trait]
impl TransactionExecutor for UnsupportedExecutor {
	fn backend(&self) -> DatabaseType {
		DatabaseType::Sqlite
	}

	async fn execute(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		panic!("unsupported streaming test does not execute mutations")
	}

	async fn fetch_one(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Row> {
		panic!("unsupported streaming test does not fetch one row")
	}

	async fn fetch_all(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		panic!("unsupported streaming test does not fetch all rows")
	}

	async fn fetch_optional(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		panic!("unsupported streaming test does not fetch an optional row")
	}

	async fn commit(self: Box<Self>) -> reinhardt_core::exception::Result<()> {
		Ok(())
	}

	async fn rollback(self: Box<Self>) -> reinhardt_core::exception::Result<()> {
		Ok(())
	}
}

#[rstest]
#[tokio::test]
async fn iterator_with_executor_bounds_buffer_and_decodes_rows() {
	// Arrange
	let rows = (1..=7)
		.map(|id| Ok(article_row(id, QueryValue::String(format!("article-{id}")))))
		.collect();
	let mut executor = StreamingExecutor::new(rows);

	// Act
	let models = QuerySet::<StreamedArticle>::new()
		.iterator_with_executor(&mut executor, 3)
		.unwrap()
		.collect::<Vec<_>>()
		.await
		.into_iter()
		.collect::<reinhardt_core::exception::Result<Vec<_>>>()
		.unwrap();

	// Assert
	assert_eq!(models.len(), 7);
	assert_eq!(models[0].title, "article-1");
	assert_eq!(executor.stream_calls, 1);
	assert_eq!(executor.max_buffered.load(Ordering::SeqCst), 3);
	assert!(executor.dropped.load(Ordering::SeqCst));
}

#[rstest]
#[tokio::test]
async fn iterator_with_executor_releases_stream_on_early_drop() {
	// Arrange
	let mut executor = StreamingExecutor::new(vec![
		Ok(article_row(1, QueryValue::String("first".to_owned()))),
		Ok(article_row(2, QueryValue::String("second".to_owned()))),
	]);
	let dropped = Arc::clone(&executor.dropped);
	let mut stream = QuerySet::<StreamedArticle>::new()
		.iterator_with_executor(&mut executor, 1)
		.unwrap();

	// Act
	assert_eq!(stream.next().await.unwrap().unwrap().title, "first");
	drop(stream);

	// Assert
	assert!(dropped.load(Ordering::SeqCst));
}

#[rstest]
#[tokio::test]
async fn iterator_with_executor_releases_stream_after_cancelled_poll() {
	// Arrange
	let mut executor = StreamingExecutor::pending_after(vec![Ok(article_row(
		1,
		QueryValue::String("first".to_owned()),
	))]);
	let dropped = Arc::clone(&executor.dropped);
	let mut stream = QuerySet::<StreamedArticle>::new()
		.iterator_with_executor(&mut executor, 1)
		.unwrap();

	// Act
	assert_eq!(stream.next().await.unwrap().unwrap().title, "first");
	let cancelled = tokio::time::timeout(std::time::Duration::from_millis(10), stream.next()).await;
	// Assert
	assert!(cancelled.is_err());
	assert!(!dropped.load(Ordering::SeqCst));
	drop(stream);

	assert!(dropped.load(Ordering::SeqCst));
}

#[rstest]
#[tokio::test]
async fn iterator_with_executor_surfaces_midstream_backend_error() {
	// Arrange
	let backend_error = DatabaseError::new(DatabaseErrorKind::Query, "stream interrupted");
	let mut executor = StreamingExecutor::new(vec![
		Ok(article_row(1, QueryValue::String("first".to_owned()))),
		Err(backend_error.into()),
		Ok(article_row(2, QueryValue::String("unreachable".to_owned()))),
	]);
	let mut stream = QuerySet::<StreamedArticle>::new()
		.iterator_with_executor(&mut executor, 2)
		.unwrap();

	// Act
	assert_eq!(stream.next().await.unwrap().unwrap().title, "first");
	// Assert
	let error = stream.next().await.unwrap().unwrap_err();
	assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Query));
	assert!(stream.next().await.is_none());
}

#[rstest]
#[tokio::test]
async fn iterator_with_executor_surfaces_midstream_decode_error() {
	// Arrange
	let mut executor = StreamingExecutor::new(vec![
		Ok(article_row(1, QueryValue::String("first".to_owned()))),
		Ok(article_row(2, QueryValue::Int(42))),
	]);
	let mut stream = QuerySet::<StreamedArticle>::new()
		.iterator_with_executor(&mut executor, 2)
		.unwrap();

	// Act
	assert_eq!(stream.next().await.unwrap().unwrap().title, "first");
	// Assert
	let error = stream.next().await.unwrap().unwrap_err();
	assert_eq!(
		error.database_kind(),
		Some(DatabaseErrorKind::Serialization)
	);
	assert!(stream.next().await.is_none());
}

#[rstest]
#[tokio::test]
async fn iterator_with_executor_handles_empty_and_none_querysets() {
	// Arrange
	let mut empty_executor = StreamingExecutor::new(Vec::new());
	// Act
	let rows = QuerySet::<StreamedArticle>::new()
		.iterator_with_executor(&mut empty_executor, 4)
		.unwrap()
		.collect::<Vec<_>>()
		.await;
	// Assert
	assert_eq!(rows.len(), 0);
	assert_eq!(empty_executor.stream_calls, 1);

	let mut none_executor = StreamingExecutor::new(vec![Ok(article_row(
		1,
		QueryValue::String("must not execute".to_owned()),
	))]);
	let rows = QuerySet::<StreamedArticle>::new()
		.none()
		.iterator_with_executor(&mut none_executor, 4)
		.unwrap()
		.collect::<Vec<_>>()
		.await;
	assert_eq!(rows.len(), 0);
	assert_eq!(none_executor.stream_calls, 0);
}

#[rstest]
fn iterator_with_executor_rejects_invalid_chunks_and_unsupported_executors() {
	// Arrange
	let mut streaming_executor = StreamingExecutor::new(Vec::new());
	// Act
	let error = match QuerySet::<StreamedArticle>::new()
		.iterator_with_executor(&mut streaming_executor, 0)
	{
		Ok(_) => panic!("zero chunk size must be rejected"),
		Err(error) => error,
	};
	// Assert
	assert_eq!(
		error.database_kind(),
		Some(DatabaseErrorKind::Configuration)
	);
	assert_eq!(streaming_executor.stream_calls, 0);

	let mut unsupported_executor = UnsupportedExecutor;
	let error = match QuerySet::<StreamedArticle>::new()
		.iterator_with_executor(&mut unsupported_executor, 1)
	{
		Ok(_) => panic!("unsupported executor must be rejected"),
		Err(error) => error,
	};
	assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Unsupported));
}

#[cfg(feature = "sqlite")]
#[rstest]
#[tokio::test]
async fn sqlite_queryset_iterator_delivers_rows_without_query_cache() {
	// Arrange
	let owner = reinhardt_db::backends::DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	let create_table = Query::create_table()
		.table(Alias::new("streamed_articles"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.primary_key(true),
		)
		.col(ColumnDef::new(Alias::new("title")).string().not_null(true))
		.to_string(SqliteQueryBuilder);
	owner.execute(&create_table, vec![]).await.unwrap();
	for (id, title) in [(1, "first"), (2, "second"), (3, "third")] {
		let insert = Query::insert()
			.into_table(Alias::new("streamed_articles"))
			.columns([Alias::new("id"), Alias::new("title")])
			.values_panic([Expr::val(id), Expr::val(title)])
			.to_string(SqliteQueryBuilder);
		owner.execute(&insert, vec![]).await.unwrap();
	}
	let lease = DatabaseConnectionLease::register(owner).unwrap();
	let mut connection = lease.handle();

	// Act
	let models = QuerySet::<StreamedArticle>::new()
		.iterator_with_db(&mut connection, 1)
		.unwrap()
		.collect::<Vec<_>>()
		.await
		.into_iter()
		.collect::<reinhardt_core::exception::Result<Vec<_>>>()
		.unwrap();

	// Assert
	assert_eq!(
		models,
		vec![
			StreamedArticle {
				id: Some(1),
				title: "first".to_owned(),
			},
			StreamedArticle {
				id: Some(2),
				title: "second".to_owned(),
			},
			StreamedArticle {
				id: Some(3),
				title: "third".to_owned(),
			},
		]
	);
}
