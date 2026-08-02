//! ORM event instrumentation for performance monitoring and debugging
//!
//! This module provides instrumentation capabilities for tracking database operations,
//! including query execution, transactions, and connection events.

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Event listener for database operations
#[async_trait]
pub trait EventListener: Send + Sync + 'static {
	/// Called when a query starts execution
	async fn on_query_start(&self, _query: &str) {}

	/// Called when a query completes execution
	async fn on_query_end(&self, _query: &str, _duration: Duration) {}

	/// Called when a transaction starts
	async fn on_transaction_start(&self) {}

	/// Called when a transaction ends
	async fn on_transaction_end(&self, _committed: bool) {}

	/// Called when a connection is acquired from the pool
	async fn on_connection_acquire(&self) {}

	/// Called when a connection is released back to the pool
	async fn on_connection_release(&self) {}

	/// Called when a query fails
	async fn on_query_error(&self, _query: &str, _error: &str) {}

	/// Called when a transaction fails
	async fn on_transaction_error(&self, _error: &str) {}
}

type SharedEventListener = Arc<dyn EventListener + 'static>;

/// Metrics collected for a single query execution
#[derive(Debug, Clone)]
pub struct QueryMetrics {
	/// The SQL query that was executed
	pub query: String,
	/// Execution duration
	pub duration: Duration,
	/// Timestamp when the query started
	pub started_at: Instant,
	/// Whether the query succeeded
	pub success: bool,
	/// Error message if the query failed
	pub error: Option<String>,
}

impl QueryMetrics {
	/// Creates a new QueryMetrics instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::QueryMetrics;
	/// use std::time::Duration;
	///
	/// let metrics = QueryMetrics::new(
	///     "SELECT * FROM users".to_string(),
	///     Duration::from_millis(50)
	/// );
	/// assert_eq!(metrics.query, "SELECT * FROM users");
	/// assert!(metrics.success);
	/// ```
	pub fn new(query: String, duration: Duration) -> Self {
		Self {
			query,
			duration,
			started_at: Instant::now(),
			success: true,
			error: None,
		}
	}

	/// Creates a QueryMetrics instance for a failed query
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::QueryMetrics;
	/// use std::time::Duration;
	///
	/// let metrics = QueryMetrics::with_error(
	///     "SELECT * FROM invalid".to_string(),
	///     Duration::from_millis(10),
	///     "table not found".to_string()
	/// );
	/// assert!(!metrics.success);
	/// assert_eq!(metrics.error.as_deref(), Some("table not found"));
	/// ```
	pub fn with_error(query: String, duration: Duration, error: String) -> Self {
		Self {
			query,
			duration,
			started_at: Instant::now(),
			success: false,
			error: Some(error),
		}
	}
}

/// Metrics collected for a transaction
#[derive(Debug, Clone)]
pub struct TransactionMetrics {
	/// Total duration of the transaction
	pub duration: Duration,
	/// Number of queries executed in the transaction
	pub query_count: usize,
	/// Whether the transaction was committed
	pub committed: bool,
	/// Timestamp when the transaction started
	pub started_at: Instant,
}

impl TransactionMetrics {
	/// Creates a new TransactionMetrics instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::TransactionMetrics;
	/// use std::time::Duration;
	///
	/// let metrics = TransactionMetrics::new(Duration::from_secs(1), 5, true);
	/// assert_eq!(metrics.query_count, 5);
	/// assert!(metrics.committed);
	/// ```
	pub fn new(duration: Duration, query_count: usize, committed: bool) -> Self {
		Self {
			duration,
			query_count,
			committed,
			started_at: Instant::now(),
		}
	}
}

/// Aggregated statistics for database operations
#[derive(Debug, Clone, Default)]
pub struct Statistics {
	/// Total number of queries executed
	pub total_queries: usize,
	/// Total number of successful queries
	pub successful_queries: usize,
	/// Total number of failed queries
	pub failed_queries: usize,
	/// Total query execution time
	pub total_duration: Duration,
	/// Average query execution time
	pub avg_duration: Duration,
	/// Minimum query execution time
	pub min_duration: Option<Duration>,
	/// Maximum query execution time
	pub max_duration: Option<Duration>,
	/// Total number of transactions
	pub total_transactions: usize,
	/// Total number of committed transactions
	pub committed_transactions: usize,
	/// Total number of rolled back transactions
	pub rolled_back_transactions: usize,
}

impl Statistics {
	/// Updates statistics with a new query metric
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::{Statistics, QueryMetrics};
	/// use std::time::Duration;
	///
	/// let mut stats = Statistics::default();
	/// let metrics = QueryMetrics::new("SELECT 1".to_string(), Duration::from_millis(10));
	/// stats.record_query(&metrics);
	///
	/// assert_eq!(stats.total_queries, 1);
	/// assert_eq!(stats.successful_queries, 1);
	/// ```
	pub fn record_query(&mut self, metrics: &QueryMetrics) {
		self.total_queries += 1;

		if metrics.success {
			self.successful_queries += 1;
		} else {
			self.failed_queries += 1;
		}

		self.total_duration += metrics.duration;

		if self.total_queries > 0 {
			self.avg_duration = self.total_duration / self.total_queries as u32;
		}

		match self.min_duration {
			None => self.min_duration = Some(metrics.duration),
			Some(min) if metrics.duration < min => self.min_duration = Some(metrics.duration),
			_ => {}
		}

		match self.max_duration {
			None => self.max_duration = Some(metrics.duration),
			Some(max) if metrics.duration > max => self.max_duration = Some(metrics.duration),
			_ => {}
		}
	}

	/// Updates statistics with a new transaction metric
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::{Statistics, TransactionMetrics};
	/// use std::time::Duration;
	///
	/// let mut stats = Statistics::default();
	/// let metrics = TransactionMetrics::new(Duration::from_secs(1), 5, true);
	/// stats.record_transaction(&metrics);
	///
	/// assert_eq!(stats.total_transactions, 1);
	/// assert_eq!(stats.committed_transactions, 1);
	/// ```
	pub fn record_transaction(&mut self, metrics: &TransactionMetrics) {
		self.total_transactions += 1;

		if metrics.committed {
			self.committed_transactions += 1;
		} else {
			self.rolled_back_transactions += 1;
		}
	}
}

/// Main instrumentation system for tracking database operations
pub struct Instrumentation {
	listeners: Arc<DashMap<String, SharedEventListener>>,
	query_metrics: Arc<DashMap<String, Vec<QueryMetrics>>>,
	transaction_metrics: Arc<DashMap<String, Vec<TransactionMetrics>>>,
	statistics: Arc<parking_lot::RwLock<Statistics>>,
}

impl Instrumentation {
	/// Creates a new Instrumentation instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// let instrumentation = Instrumentation::new();
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			listeners: Arc::new(DashMap::new()),
			query_metrics: Arc::new(DashMap::new()),
			transaction_metrics: Arc::new(DashMap::new()),
			statistics: Arc::new(parking_lot::RwLock::new(Statistics::default())),
		}
	}

	/// Adds an event listener with a unique identifier
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::{Instrumentation, EventListener};
	/// use std::sync::Arc;
	/// use std::time::Duration;
	/// use async_trait::async_trait;
	///
	/// struct MyListener;
	///
	/// #[async_trait]
	/// impl EventListener for MyListener {
	///     async fn on_query_start(&self, _query: &str) {}
	/// }
	///
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.add_listener("my_listener".to_string(), Arc::new(MyListener));
	/// assert_eq!(instrumentation.listener_count(), 1);
	/// ```
	pub fn add_listener(&self, id: String, listener: SharedEventListener) {
		self.listeners.insert(id, listener);
	}

	/// Removes an event listener by its identifier
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::{Instrumentation, EventListener};
	/// use std::sync::Arc;
	/// use async_trait::async_trait;
	///
	/// struct MyListener;
	///
	/// #[async_trait]
	/// impl EventListener for MyListener {}
	///
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.add_listener("my_listener".to_string(), Arc::new(MyListener));
	/// instrumentation.remove_listener("my_listener");
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// ```
	pub fn remove_listener(&self, id: &str) {
		self.listeners.remove(id);
	}

	/// Gets the number of registered listeners
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// let instrumentation = Instrumentation::new();
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// ```
	pub fn listener_count(&self) -> usize {
		self.listeners.len()
	}

	fn listener_snapshot(&self) -> Vec<SharedEventListener> {
		self.listeners
			.iter()
			.map(|entry| Arc::clone(entry.value()))
			.collect()
	}

	async fn notify_query_end_listeners(&self, query: &str, duration: Duration) {
		let listeners = self.listener_snapshot();
		for listener in listeners {
			listener.on_query_end(query, duration).await;
		}
	}

	async fn notify_query_error_listeners(&self, query: &str, error: &str) {
		let listeners = self.listener_snapshot();
		for listener in listeners {
			listener.on_query_error(query, error).await;
		}
	}

	/// Notifies all listeners that a query has started
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// # tokio_test::block_on(async {
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.query_start("SELECT * FROM users").await;
	/// // Verify the method executes successfully (no panic)
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// # });
	/// ```
	pub async fn query_start(&self, query: &str) {
		let listeners = self.listener_snapshot();
		for listener in listeners {
			listener.on_query_start(query).await;
		}
	}

	/// Notifies all listeners that a query has ended
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.query_end("SELECT * FROM users", Duration::from_millis(50)).await;
	/// // Verify the method executes successfully and records metrics
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// # });
	/// ```
	pub async fn query_end(&self, query: &str, duration: Duration) {
		self.query_end_with_params(query, &[], duration).await;
	}

	/// Notifies all listeners that a query has ended with bound parameter context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.query_end_with_params(
	///     "SELECT * FROM users WHERE id = $1",
	///     &["1".to_string()],
	///     Duration::from_millis(50)
	/// ).await;
	/// // Verify the method executes successfully and records metrics
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// # });
	/// ```
	pub async fn query_end_with_params(&self, query: &str, params: &[String], duration: Duration) {
		let metrics = QueryMetrics::new(query.to_string(), duration);

		self.statistics.write().record_query(&metrics);

		self.query_metrics
			.entry(query.to_string())
			.or_default()
			.push(metrics);

		super::n_plus_one::record_query(query, params, duration);

		self.notify_query_end_listeners(query, duration).await;
	}

	pub(crate) async fn orm_query_end_with_params(
		&self,
		query: &str,
		params: &[String],
		duration: Duration,
	) {
		self.orm_query_end_with_params_sync(query, params, duration);
	}

	/// Records an ORM query without requiring an async context.
	///
	/// Streaming query accounting invokes this from a drop guard so partially
	/// consumed streams remain visible to N+1 detection.
	pub(crate) fn orm_query_end_with_params_sync(
		&self,
		query: &str,
		params: &[String],
		duration: Duration,
	) {
		super::n_plus_one::record_query(query, params, duration);
	}

	pub(crate) async fn orm_query_error(&self, _query: &str, _error: &str) {}

	/// Notifies all listeners that a query has failed
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.query_error(
	///     "SELECT * FROM invalid",
	///     "table not found",
	///     Duration::from_millis(10)
	/// ).await;
	/// // Verify the method executes successfully and records error metrics
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// # });
	/// ```
	pub async fn query_error(&self, query: &str, error: &str, duration: Duration) {
		let metrics = QueryMetrics::with_error(query.to_string(), duration, error.to_string());

		self.statistics.write().record_query(&metrics);

		self.query_metrics
			.entry(query.to_string())
			.or_default()
			.push(metrics);

		self.notify_query_error_listeners(query, error).await;
	}

	/// Notifies all listeners that a transaction has started
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// # tokio_test::block_on(async {
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.transaction_start().await;
	/// // Verify the method executes successfully (no panic)
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// # });
	/// ```
	pub async fn transaction_start(&self) {
		let listeners = self.listener_snapshot();
		for listener in listeners {
			listener.on_transaction_start().await;
		}
	}

	/// Notifies all listeners that a transaction has ended
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// # tokio_test::block_on(async {
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.transaction_end(true).await;
	/// // Verify the method executes successfully (no panic)
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// # });
	/// ```
	pub async fn transaction_end(&self, committed: bool) {
		let listeners = self.listener_snapshot();
		for listener in listeners {
			listener.on_transaction_end(committed).await;
		}
	}

	/// Records transaction completion with metrics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.record_transaction(
	///     "tx-1",
	///     Duration::from_secs(1),
	///     5,
	///     true
	/// ).await;
	/// // Verify the method executes successfully and records transaction metrics
	/// assert_eq!(instrumentation.listener_count(), 0);
	/// # });
	/// ```
	pub async fn record_transaction(
		&self,
		transaction_id: &str,
		duration: Duration,
		query_count: usize,
		committed: bool,
	) {
		let metrics = TransactionMetrics::new(duration, query_count, committed);

		self.statistics.write().record_transaction(&metrics);

		self.transaction_metrics
			.entry(transaction_id.to_string())
			.or_default()
			.push(metrics);

		self.transaction_end(committed).await;
	}

	/// Gets the current statistics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// let instrumentation = Instrumentation::new();
	/// let stats = instrumentation.statistics();
	/// assert_eq!(stats.total_queries, 0);
	/// ```
	pub fn statistics(&self) -> Statistics {
		self.statistics.read().clone()
	}

	/// Clears all collected metrics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// let instrumentation = Instrumentation::new();
	/// instrumentation.clear_metrics();
	/// let stats = instrumentation.statistics();
	/// assert_eq!(stats.total_queries, 0);
	/// ```
	pub fn clear_metrics(&self) {
		self.query_metrics.clear();
		self.transaction_metrics.clear();
		*self.statistics.write() = Statistics::default();
	}

	/// Gets query metrics for a specific query
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// let instrumentation = Instrumentation::new();
	/// let metrics = instrumentation.query_metrics("SELECT * FROM users");
	/// assert!(metrics.is_empty());
	/// ```
	pub fn query_metrics(&self, query: &str) -> Vec<QueryMetrics> {
		self.query_metrics
			.get(query)
			.map(|entry| entry.value().clone())
			.unwrap_or_default()
	}

	/// Gets transaction metrics for a specific transaction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::instrumentation::Instrumentation;
	///
	/// let instrumentation = Instrumentation::new();
	/// let metrics = instrumentation.transaction_metrics("tx-1");
	/// assert!(metrics.is_empty());
	/// ```
	pub fn transaction_metrics(&self, transaction_id: &str) -> Vec<TransactionMetrics> {
		self.transaction_metrics
			.get(transaction_id)
			.map(|entry| entry.value().clone())
			.unwrap_or_default()
	}
}

impl Default for Instrumentation {
	fn default() -> Self {
		Self::new()
	}
}

/// Global instrumentation singleton
static INSTRUMENTATION: once_cell::sync::Lazy<Instrumentation> =
	once_cell::sync::Lazy::new(Instrumentation::new);

/// Gets the global instrumentation instance
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::instrumentation::instrumentation;
///
/// let instr = instrumentation();
/// assert_eq!(instr.listener_count(), 0);
/// ```
pub fn instrumentation() -> &'static Instrumentation {
	&INSTRUMENTATION
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::{NPlusOneConfig, NPlusOneScope};
	use std::sync::atomic::{AtomicUsize, Ordering};

	struct TestListener {
		query_start_count: Arc<AtomicUsize>,
		query_end_count: Arc<AtomicUsize>,
		query_error_count: Arc<AtomicUsize>,
		transaction_start_count: Arc<AtomicUsize>,
		transaction_end_count: Arc<AtomicUsize>,
	}

	fn low_threshold_config() -> NPlusOneConfig {
		let mut config = NPlusOneConfig::default();
		config.threshold = 3;
		config.min_distinct_params = 3;
		config
	}

	#[async_trait]
	impl EventListener for TestListener {
		async fn on_query_start(&self, _query: &str) {
			self.query_start_count.fetch_add(1, Ordering::SeqCst);
		}

		async fn on_query_end(&self, _query: &str, _duration: Duration) {
			self.query_end_count.fetch_add(1, Ordering::SeqCst);
		}

		async fn on_query_error(&self, _query: &str, _error: &str) {
			self.query_error_count.fetch_add(1, Ordering::SeqCst);
		}

		async fn on_transaction_start(&self) {
			self.transaction_start_count.fetch_add(1, Ordering::SeqCst);
		}

		async fn on_transaction_end(&self, _committed: bool) {
			self.transaction_end_count.fetch_add(1, Ordering::SeqCst);
		}
	}

	#[tokio::test]
	async fn test_query_start_notification() {
		let instr = Instrumentation::new();

		let query_start = Arc::new(AtomicUsize::new(0));

		let listener = Arc::new(TestListener {
			query_start_count: query_start.clone(),
			query_end_count: Arc::new(AtomicUsize::new(0)),
			query_error_count: Arc::new(AtomicUsize::new(0)),
			transaction_start_count: Arc::new(AtomicUsize::new(0)),
			transaction_end_count: Arc::new(AtomicUsize::new(0)),
		});

		instr.add_listener("test".to_string(), listener);
		instr.query_start("SELECT * FROM users").await;

		assert_eq!(query_start.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_query_end_notification() {
		let instr = Instrumentation::new();

		let query_end = Arc::new(AtomicUsize::new(0));

		let listener = Arc::new(TestListener {
			query_start_count: Arc::new(AtomicUsize::new(0)),
			query_end_count: query_end.clone(),
			query_error_count: Arc::new(AtomicUsize::new(0)),
			transaction_start_count: Arc::new(AtomicUsize::new(0)),
			transaction_end_count: Arc::new(AtomicUsize::new(0)),
		});

		instr.add_listener("test".to_string(), listener);
		instr
			.query_end("SELECT * FROM users", Duration::from_millis(50))
			.await;

		assert_eq!(query_end.load(Ordering::SeqCst), 1);

		let stats = instr.statistics();
		assert_eq!(stats.total_queries, 1);
		assert_eq!(stats.successful_queries, 1);
	}

	#[tokio::test]
	async fn test_query_end_with_params_records_n_plus_one_scope() {
		let instrumentation = Instrumentation::new();

		let (_, report) = NPlusOneScope::warn("instrumentation", low_threshold_config())
			.run_with_report(async {
				for id in ["1", "2", "3"] {
					instrumentation
						.query_end_with_params(
							"SELECT * FROM posts WHERE author_id = $1",
							&[id.to_string()],
							Duration::from_millis(1),
						)
						.await;
				}
			})
			.await;

		assert_eq!(report.findings.len(), 1);
	}

	#[tokio::test]
	async fn test_orm_query_end_with_params_does_not_retain_metrics() {
		let instrumentation = Instrumentation::new();
		let query_end = Arc::new(AtomicUsize::new(0));
		let query = "SELECT * FROM posts WHERE author_id = $1";

		let listener = Arc::new(TestListener {
			query_start_count: Arc::new(AtomicUsize::new(0)),
			query_end_count: query_end.clone(),
			query_error_count: Arc::new(AtomicUsize::new(0)),
			transaction_start_count: Arc::new(AtomicUsize::new(0)),
			transaction_end_count: Arc::new(AtomicUsize::new(0)),
		});

		instrumentation.add_listener("test".to_string(), listener);
		instrumentation
			.orm_query_end_with_params(query, &["1".to_string()], Duration::from_millis(1))
			.await;

		assert_eq!(query_end.load(Ordering::SeqCst), 0);
		assert!(instrumentation.query_metrics(query).is_empty());
		assert_eq!(instrumentation.statistics().total_queries, 0);
	}

	#[tokio::test]
	async fn test_orm_query_end_with_params_records_n_plus_one_scope() {
		let instrumentation = Instrumentation::new();

		let (_, report) = NPlusOneScope::warn("instrumentation", low_threshold_config())
			.run_with_report(async {
				for id in ["1", "2", "3"] {
					instrumentation
						.orm_query_end_with_params(
							"SELECT * FROM posts WHERE author_id = $1",
							&[id.to_string()],
							Duration::from_millis(1),
						)
						.await;
				}
			})
			.await;

		assert_eq!(report.findings.len(), 1);
	}

	#[tokio::test]
	async fn test_orm_query_error_does_not_retain_metrics() {
		let instr = Instrumentation::new();
		let query_error = Arc::new(AtomicUsize::new(0));
		let query = "SELECT * FROM posts WHERE author_id = 1";

		let listener = Arc::new(TestListener {
			query_start_count: Arc::new(AtomicUsize::new(0)),
			query_end_count: Arc::new(AtomicUsize::new(0)),
			query_error_count: query_error.clone(),
			transaction_start_count: Arc::new(AtomicUsize::new(0)),
			transaction_end_count: Arc::new(AtomicUsize::new(0)),
		});

		instr.add_listener("test".to_string(), listener);
		instr.orm_query_error(query, "table not found").await;

		assert_eq!(query_error.load(Ordering::SeqCst), 0);
		assert!(instr.query_metrics(query).is_empty());
		assert_eq!(instr.statistics().total_queries, 0);
	}

	#[tokio::test]
	async fn test_query_end_with_params_ignores_same_bind_signature() {
		let instrumentation = Instrumentation::new();

		let (_, report) = NPlusOneScope::warn("instrumentation", low_threshold_config())
			.run_with_report(async {
				for _ in 0..3 {
					instrumentation
						.query_end_with_params(
							"SELECT * FROM posts WHERE author_id = $1",
							&["1".to_string()],
							Duration::from_millis(1),
						)
						.await;
				}
			})
			.await;

		assert!(report.findings.is_empty());
	}

	#[tokio::test]
	async fn test_query_end_with_params_does_not_report_single_join_query() {
		let instrumentation = Instrumentation::new();

		let (_, report) = NPlusOneScope::warn("instrumentation", low_threshold_config())
			.run_with_report(async {
				instrumentation
					.query_end_with_params(
						"SELECT posts.*, authors.* FROM posts JOIN authors ON posts.author_id = authors.id",
						&[],
						Duration::from_millis(3),
					)
					.await;
			})
			.await;

		assert!(report.findings.is_empty());
	}

	#[tokio::test]
	async fn test_query_end_with_params_does_not_report_single_batched_query() {
		let instrumentation = Instrumentation::new();

		let (_, report) = NPlusOneScope::warn("instrumentation", low_threshold_config())
			.run_with_report(async {
				instrumentation
					.query_end_with_params(
						"SELECT * FROM posts WHERE author_id IN ($1, $2, $3)",
						&["1".to_string(), "2".to_string(), "3".to_string()],
						Duration::from_millis(3),
					)
					.await;
			})
			.await;

		assert!(report.findings.is_empty());
	}

	#[tokio::test]
	async fn test_query_end_with_params_respects_ignored_fingerprint() {
		let instrumentation = Instrumentation::new();
		let mut config = low_threshold_config();
		config
			.ignored_fingerprints
			.insert("SELECT * FROM posts WHERE author_id = ?".to_string());

		let (_, report) = NPlusOneScope::warn("instrumentation", config)
			.run_with_report(async {
				for id in ["1", "2", "3"] {
					instrumentation
						.query_end_with_params(
							"SELECT * FROM posts WHERE author_id = $1",
							&[id.to_string()],
							Duration::from_millis(1),
						)
						.await;
				}
			})
			.await;

		assert!(report.findings.is_empty());
	}

	#[tokio::test]
	async fn test_query_end_with_params_reports_dropped_samples() {
		let instrumentation = Instrumentation::new();
		let mut config = low_threshold_config();
		config.max_records_per_scope = 2;

		let (_, report) = NPlusOneScope::warn("instrumentation", config)
			.run_with_report(async {
				for id in ["1", "2", "3"] {
					instrumentation
						.query_end_with_params(
							"SELECT * FROM posts WHERE author_id = $1",
							&[id.to_string()],
							Duration::from_millis(1),
						)
						.await;
				}
			})
			.await;

		assert_eq!(report.total_recorded_queries, 2);
		assert_eq!(report.dropped_sample_count, 1);
		assert!(report.findings.is_empty());
	}

	#[tokio::test]
	async fn test_query_error_notification() {
		let instr = Instrumentation::new();

		let query_error = Arc::new(AtomicUsize::new(0));

		let listener = Arc::new(TestListener {
			query_start_count: Arc::new(AtomicUsize::new(0)),
			query_end_count: Arc::new(AtomicUsize::new(0)),
			query_error_count: query_error.clone(),
			transaction_start_count: Arc::new(AtomicUsize::new(0)),
			transaction_end_count: Arc::new(AtomicUsize::new(0)),
		});

		instr.add_listener("test".to_string(), listener);
		instr
			.query_error(
				"SELECT * FROM invalid",
				"table not found",
				Duration::from_millis(10),
			)
			.await;

		assert_eq!(query_error.load(Ordering::SeqCst), 1);

		let stats = instr.statistics();
		assert_eq!(stats.total_queries, 1);
		assert_eq!(stats.failed_queries, 1);
	}

	#[tokio::test]
	async fn test_transaction_notifications() {
		let instr = Instrumentation::new();

		let tx_start = Arc::new(AtomicUsize::new(0));
		let tx_end = Arc::new(AtomicUsize::new(0));

		let listener = Arc::new(TestListener {
			query_start_count: Arc::new(AtomicUsize::new(0)),
			query_end_count: Arc::new(AtomicUsize::new(0)),
			query_error_count: Arc::new(AtomicUsize::new(0)),
			transaction_start_count: tx_start.clone(),
			transaction_end_count: tx_end.clone(),
		});

		instr.add_listener("test".to_string(), listener);

		instr.transaction_start().await;
		assert_eq!(tx_start.load(Ordering::SeqCst), 1);

		instr.transaction_end(true).await;
		assert_eq!(tx_end.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_record_transaction() {
		let instr = Instrumentation::new();

		instr
			.record_transaction("tx-1", Duration::from_secs(1), 5, true)
			.await;

		let stats = instr.statistics();
		assert_eq!(stats.total_transactions, 1);
		assert_eq!(stats.committed_transactions, 1);
	}

	#[tokio::test]
	async fn test_statistics_aggregation() {
		let instr = Instrumentation::new();

		instr.query_end("SELECT 1", Duration::from_millis(10)).await;
		instr.query_end("SELECT 2", Duration::from_millis(20)).await;
		instr.query_end("SELECT 3", Duration::from_millis(30)).await;

		let stats = instr.statistics();
		assert_eq!(stats.total_queries, 3);
		assert_eq!(stats.successful_queries, 3);
		assert_eq!(stats.min_duration, Some(Duration::from_millis(10)));
		assert_eq!(stats.max_duration, Some(Duration::from_millis(30)));
		assert_eq!(stats.avg_duration, Duration::from_millis(20));
	}

	#[tokio::test]
	async fn test_listener_management() {
		let instr = Instrumentation::new();

		let listener = Arc::new(TestListener {
			query_start_count: Arc::new(AtomicUsize::new(0)),
			query_end_count: Arc::new(AtomicUsize::new(0)),
			query_error_count: Arc::new(AtomicUsize::new(0)),
			transaction_start_count: Arc::new(AtomicUsize::new(0)),
			transaction_end_count: Arc::new(AtomicUsize::new(0)),
		});

		instr.add_listener("test1".to_string(), listener.clone());
		assert_eq!(instr.listener_count(), 1);

		instr.add_listener("test2".to_string(), listener);
		assert_eq!(instr.listener_count(), 2);

		instr.remove_listener("test1");
		assert_eq!(instr.listener_count(), 1);
	}

	#[tokio::test]
	async fn test_clear_metrics() {
		let instr = Instrumentation::new();

		instr.query_end("SELECT 1", Duration::from_millis(10)).await;

		let stats_before = instr.statistics();
		assert_eq!(stats_before.total_queries, 1);

		instr.clear_metrics();

		let stats_after = instr.statistics();
		assert_eq!(stats_after.total_queries, 0);
	}

	#[tokio::test]
	async fn test_query_metrics_retrieval() {
		let instr = Instrumentation::new();

		let query = "SELECT * FROM users";
		instr.query_end(query, Duration::from_millis(50)).await;

		let metrics = instr.query_metrics(query);
		assert_eq!(metrics.len(), 1);
		assert_eq!(metrics[0].query, query);
		assert!(metrics[0].success);
	}

	#[tokio::test]
	async fn test_transaction_metrics_retrieval() {
		let instr = Instrumentation::new();

		instr
			.record_transaction("tx-1", Duration::from_secs(1), 5, true)
			.await;

		let metrics = instr.transaction_metrics("tx-1");
		assert_eq!(metrics.len(), 1);
		assert_eq!(metrics[0].query_count, 5);
		assert!(metrics[0].committed);
	}

	#[tokio::test]
	async fn test_global_instrumentation() {
		let instr = instrumentation();
		instr.clear_metrics();

		instr.query_end("SELECT 1", Duration::from_millis(10)).await;

		let stats = instr.statistics();
		assert!(stats.total_queries >= 1);
	}
}
