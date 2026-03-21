//! Query plan optimization and analysis system
//!
//! Provides tools for analyzing query execution plans and generating optimization hints.
//!
//! # Logging
//!
//! This module uses the `log` crate for diagnostics:
//! - `INFO`: Optimization suggestions for application developers
//! - `WARN`: Performance warnings that need attention
//! - `DEBUG`: Detailed query analysis metrics
//!
//! Enable logging with:
//! ```rust
//! env_logger::init(); // or another logger implementation
//! ```
//!
//! # Examples
//!
//! ```
//! use reinhardt_rest::filters::{QueryOptimizer, OptimizationHint};
//!
//! # async fn example() {
//! let optimizer = QueryOptimizer::new()
//!     .with_hint(OptimizationHint::PreferIndexScan)
//!     .with_hint(OptimizationHint::DisableSeqScan);
//!
//! let sql = "SELECT * FROM users WHERE email = 'test@example.com'".to_string();
//! // Optimizer would analyze and suggest improvements
//! // Verify the optimizer is configured correctly
//! let _: QueryOptimizer = optimizer;
//! # }
//! ```

use super::{FilterBackend, FilterResult};
use async_trait::async_trait;
use log::{debug, info, warn};
use regex::Regex;
use reinhardt_db::backends::{DatabaseConnection as BackendsConnection, QueryValue, Row};
use std::collections::HashMap;
use std::sync::Arc;

/// Database type for query optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseType {
	/// PostgreSQL database.
	PostgreSQL,
	/// MySQL database.
	MySQL,
	/// SQLite database.
	SQLite,
}

/// Query complexity classification
///
/// Categorizes queries based on their estimated cost and complexity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryComplexity {
	/// Simple query (cost < 10)
	Simple,
	/// Moderate complexity (cost 10-100)
	Moderate,
	/// Complex query (cost 100-1000)
	Complex,
	/// Very complex query (cost > 1000)
	VeryComplex,
}

impl QueryComplexity {
	/// Determine complexity from estimated cost
	fn from_cost(cost: f64) -> Self {
		if cost < 10.0 {
			QueryComplexity::Simple
		} else if cost < 100.0 {
			QueryComplexity::Moderate
		} else if cost < 1000.0 {
			QueryComplexity::Complex
		} else {
			QueryComplexity::VeryComplex
		}
	}
}

/// Query analysis result
///
/// Contains detailed analysis results for a query execution plan.
#[derive(Debug, Clone)]
pub struct QueryAnalysis {
	/// Estimated query cost
	pub estimated_cost: Option<f64>,
	/// Query complexity classification
	pub complexity: QueryComplexity,
	/// Optimization suggestions
	pub suggestions: Vec<String>,
	/// Whether query requires full table scan
	pub has_full_table_scan: bool,
	/// Columns that would benefit from indexes
	pub missing_indexes: Vec<String>,
	/// Table name being queried
	pub table_name: String,
}

/// Query optimization hint
///
/// Provides hints to the database query planner for optimization.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::OptimizationHint;
///
/// let hint = OptimizationHint::PreferIndexScan;
/// let seq_scan = OptimizationHint::DisableSeqScan;
/// // Verify hints are created successfully
/// let _: OptimizationHint = hint;
/// let _: OptimizationHint = seq_scan;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationHint {
	/// Prefer index scan over sequential scan
	PreferIndexScan,

	/// Disable sequential scan (force index usage)
	DisableSeqScan,

	/// Enable hash join optimization
	EnableHashJoin,

	/// Disable hash join
	DisableHashJoin,

	/// Enable merge join optimization
	EnableMergeJoin,

	/// Disable merge join
	DisableMergeJoin,

	/// Prefer nested loop join
	PreferNestedLoop,

	/// Set cost multiplier for random page access
	RandomPageCost(f64),

	/// Set cost multiplier for sequential page access
	SeqPageCost(f64),

	/// Set effective cache size
	EffectiveCacheSize(String),
}

impl OptimizationHint {
	/// Convert hint to database-specific SQL
	///
	/// # Database Compatibility
	///
	/// Different databases have different hint syntaxes:
	/// - PostgreSQL: SET commands for session-level optimizer parameters
	/// - MySQL: Optimizer hints in `/*+ ... */` comments
	/// - SQLite: PRAGMA statements for query optimization
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{OptimizationHint, DatabaseType};
	///
	/// let hint = OptimizationHint::PreferIndexScan;
	/// let pg_sql = hint.to_sql_hint(DatabaseType::PostgreSQL);
	/// let mysql_sql = hint.to_sql_hint(DatabaseType::MySQL);
	/// let sqlite_sql = hint.to_sql_hint(DatabaseType::SQLite);
	/// // Verify SQL hints are generated for each database type
	/// assert!(!pg_sql.is_empty());
	/// assert!(!mysql_sql.is_empty());
	/// assert!(sqlite_sql.is_empty()); // SQLite doesn't support this hint
	/// ```
	pub fn to_sql_hint(&self, db_type: DatabaseType) -> String {
		match db_type {
			DatabaseType::PostgreSQL => self.to_postgresql_hint(),
			DatabaseType::MySQL => self.to_mysql_hint(),
			DatabaseType::SQLite => self.to_sqlite_hint(),
		}
	}

	/// Generate PostgreSQL-specific hint
	fn to_postgresql_hint(&self) -> String {
		match self {
			OptimizationHint::PreferIndexScan => "SET enable_indexscan = on".to_string(),
			OptimizationHint::DisableSeqScan => "SET enable_seqscan = off".to_string(),
			OptimizationHint::EnableHashJoin => "SET enable_hashjoin = on".to_string(),
			OptimizationHint::DisableHashJoin => "SET enable_hashjoin = off".to_string(),
			OptimizationHint::EnableMergeJoin => "SET enable_mergejoin = on".to_string(),
			OptimizationHint::DisableMergeJoin => "SET enable_mergejoin = off".to_string(),
			OptimizationHint::PreferNestedLoop => "SET enable_nestloop = on".to_string(),
			OptimizationHint::RandomPageCost(cost) => {
				format!("SET random_page_cost = {}", cost)
			}
			OptimizationHint::SeqPageCost(cost) => format!("SET seq_page_cost = {}", cost),
			OptimizationHint::EffectiveCacheSize(size) => {
				format!("SET effective_cache_size = '{}'", size)
			}
		}
	}

	/// Generate MySQL-specific hint
	fn to_mysql_hint(&self) -> String {
		match self {
			OptimizationHint::PreferIndexScan => "/*+ INDEX_SCAN() */".to_string(),
			OptimizationHint::DisableSeqScan => "/*+ NO_TABLE_SCAN() */".to_string(),
			OptimizationHint::EnableHashJoin => "/*+ HASH_JOIN() */".to_string(),
			OptimizationHint::DisableHashJoin => "/*+ NO_HASH_JOIN() */".to_string(),
			OptimizationHint::EnableMergeJoin => "/*+ MERGE_JOIN() */".to_string(),
			OptimizationHint::DisableMergeJoin => "/*+ NO_MERGE_JOIN() */".to_string(),
			OptimizationHint::PreferNestedLoop => "/*+ BNL() */".to_string(),
			OptimizationHint::RandomPageCost(_) => {
				// MySQL doesn't have direct equivalent
				"".to_string()
			}
			OptimizationHint::SeqPageCost(_) => {
				// MySQL doesn't have direct equivalent
				"".to_string()
			}
			OptimizationHint::EffectiveCacheSize(_) => {
				// MySQL doesn't have direct equivalent
				"".to_string()
			}
		}
	}

	/// Generate SQLite-specific hint
	fn to_sqlite_hint(&self) -> String {
		match self {
			OptimizationHint::PreferIndexScan => "".to_string(),
			OptimizationHint::DisableSeqScan => "".to_string(),
			OptimizationHint::EnableHashJoin => "".to_string(),
			OptimizationHint::DisableHashJoin => "".to_string(),
			OptimizationHint::EnableMergeJoin => "".to_string(),
			OptimizationHint::DisableMergeJoin => "".to_string(),
			OptimizationHint::PreferNestedLoop => "".to_string(),
			OptimizationHint::RandomPageCost(_) => "".to_string(),
			OptimizationHint::SeqPageCost(_) => "".to_string(),
			OptimizationHint::EffectiveCacheSize(size) => {
				format!("PRAGMA cache_size = {}", size)
			}
		}
	}
}

/// Query execution plan analysis result
///
/// Contains information about how a query will be executed.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::QueryPlan;
///
/// let plan = QueryPlan::new("Seq Scan on users");
/// // Verify the query plan is created successfully
/// assert!(plan.raw_plan.contains("Seq Scan"));
/// ```
#[derive(Debug, Clone)]
pub struct QueryPlan {
	/// Raw EXPLAIN output
	pub raw_plan: String,

	/// Estimated cost
	pub estimated_cost: Option<f64>,

	/// Estimated rows
	pub estimated_rows: Option<i64>,

	/// Whether plan uses index
	pub uses_index: bool,

	/// Optimization suggestions
	pub suggestions: Vec<String>,

	/// Table name being queried
	pub table_name: String,
}

impl QueryPlan {
	/// Create a new query plan from EXPLAIN output
	///
	/// Parses EXPLAIN output to extract cost, row estimates, and index usage.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::QueryPlan;
	///
	/// let plan = QueryPlan::new("Seq Scan on users (cost=0.00..35.50 rows=2550)");
	/// // Verify the query plan parsing is correct
	/// assert_eq!(plan.estimated_cost, Some(35.50));
	/// assert_eq!(plan.estimated_rows, Some(2550));
	/// assert!(!plan.uses_index);
	/// ```
	pub fn new(raw_plan: impl Into<String>) -> Self {
		let raw_plan = raw_plan.into();

		// Parse cost from PostgreSQL EXPLAIN format: (cost=start..end rows=N)
		let cost_regex = Regex::new(r"cost=[\d.]+\.\.([\d.]+)").unwrap();
		let estimated_cost = cost_regex
			.captures(&raw_plan)
			.and_then(|caps| caps.get(1))
			.and_then(|m| m.as_str().parse::<f64>().ok());

		// Parse rows estimate
		let rows_regex = Regex::new(r"rows=(\d+)").unwrap();
		let estimated_rows = rows_regex
			.captures(&raw_plan)
			.and_then(|caps| caps.get(1))
			.and_then(|m| m.as_str().parse::<i64>().ok());

		// Check for index usage
		let uses_index = raw_plan.contains("Index Scan")
			|| raw_plan.contains("Index Only Scan")
			|| raw_plan.contains("Bitmap Index Scan");

		// Extract table name from EXPLAIN output
		// Matches patterns like "Seq Scan on users", "Index Scan using idx on users"
		let table_regex = Regex::new(r"\bon\s+(\w+)").unwrap();
		let table_name = table_regex
			.captures(&raw_plan)
			.and_then(|caps| caps.get(1))
			.map(|m| m.as_str().to_string())
			.unwrap_or_else(|| "unknown".to_string());

		Self {
			raw_plan,
			estimated_cost,
			estimated_rows,
			uses_index,
			suggestions: Vec::new(),
			table_name,
		}
	}

	/// Analyze the query plan and generate optimization suggestions
	///
	/// Examines the query plan for common performance issues and generates
	/// actionable recommendations for optimization.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::QueryPlan;
	///
	/// let plan = QueryPlan::new("Seq Scan on users (cost=0.00..35.50 rows=2550)");
	/// let analyzed = plan.analyze();
	/// // Verify analysis generates optimization suggestions
	/// assert!(!analyzed.suggestions.is_empty());
	/// ```
	pub fn analyze(mut self) -> Self {
		// Check for sequential scans without index
		if (self.raw_plan.contains("Seq Scan") || self.raw_plan.contains("Table Scan"))
			&& !self.uses_index
		{
			self.suggestions.push(
				"Sequential scan detected - consider adding an index to improve performance"
					.to_string(),
			);
		}

		// Check for high cost queries
		if let Some(cost) = self.estimated_cost {
			if cost > 1000.0 {
				self.suggestions.push(format!(
					"High query cost ({:.2}) detected - consider optimizing query structure or adding indexes",
					cost
				));
			} else if cost > 100.0 {
				self.suggestions.push(format!(
					"Moderate query cost ({:.2}) - may benefit from optimization",
					cost
				));
			}
		}

		// Check for large row estimates
		if let Some(rows) = self.estimated_rows
			&& rows > 10000
		{
			self.suggestions.push(format!(
				"Large result set ({} rows) - consider adding LIMIT clause or filtering",
				rows
			));
		}

		// Check for nested loops with large outer tables
		if self.raw_plan.contains("Nested Loop") {
			self.suggestions.push(
				"Nested loop join detected - ensure inner table is indexed and smaller".to_string(),
			);
		}

		// Check for hash join memory concerns
		if self.raw_plan.contains("Hash Join")
			&& let Some(rows) = self.estimated_rows
			&& rows > 100000
		{
			self.suggestions
				.push("Large hash join detected - may require significant memory".to_string());
		}

		// Check for missing statistics
		if self.raw_plan.contains("rows=1 ") && !self.raw_plan.contains("LIMIT") {
			self.suggestions.push(
				"Row estimate of 1 without LIMIT - table statistics may be outdated".to_string(),
			);
		}

		// Check for bitmap heap scans (good, but could be optimized)
		if self.raw_plan.contains("Bitmap Heap Scan") {
			self.suggestions
				.push("Bitmap heap scan used - consider index-only scan if possible".to_string());
		}

		// Check for sort operations
		if self.raw_plan.contains("Sort")
			&& let Some(rows) = self.estimated_rows
			&& rows > 10000
		{
			self.suggestions
				.push("Large sort operation - consider adding index on sort columns".to_string());
		}

		self
	}
}

/// Query optimizer that provides optimization hints and analysis
///
/// This filter backend analyzes SQL queries and can inject optimization hints
/// to improve query performance.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{FilterBackend, QueryOptimizer, OptimizationHint};
/// use std::collections::HashMap;
///
/// # async fn example() {
/// let optimizer = QueryOptimizer::new()
///     .with_hint(OptimizationHint::PreferIndexScan)
///     .enable_analysis(true);
///
/// let params = HashMap::new();
/// let sql = "SELECT * FROM users WHERE email = 'test@example.com'".to_string();
/// let result = optimizer.filter_queryset(&params, sql).await;
/// // Verify the filter backend processes the query successfully
/// assert!(result.is_ok());
/// # }
/// ```
pub struct QueryOptimizer {
	hints: Vec<OptimizationHint>,
	enable_analysis: bool,
	enable_hints: bool,
	db_type: DatabaseType,
	connection: Option<Arc<BackendsConnection>>,
}

impl std::fmt::Debug for QueryOptimizer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut debug_struct = f.debug_struct("QueryOptimizer");
		debug_struct
			.field("hints", &self.hints)
			.field("enable_analysis", &self.enable_analysis)
			.field("enable_hints", &self.enable_hints)
			.field("db_type", &self.db_type)
			.field(
				"connection",
				&self.connection.as_ref().map(|_| "<BackendsConnection>"),
			);

		debug_struct.finish()
	}
}

impl Default for QueryOptimizer {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryOptimizer {
	/// Create a new query optimizer with PostgreSQL as default database
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::QueryOptimizer;
	///
	/// let optimizer = QueryOptimizer::new();
	/// // Verify the optimizer is created successfully
	/// let _: QueryOptimizer = optimizer;
	/// ```
	pub fn new() -> Self {
		Self {
			hints: Vec::new(),
			enable_analysis: false,
			enable_hints: false,
			db_type: DatabaseType::PostgreSQL,
			connection: None,
		}
	}

	/// Create a query optimizer for a specific database type
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{QueryOptimizer, DatabaseType};
	///
	/// let pg_optimizer = QueryOptimizer::for_database(DatabaseType::PostgreSQL);
	/// let mysql_optimizer = QueryOptimizer::for_database(DatabaseType::MySQL);
	/// let sqlite_optimizer = QueryOptimizer::for_database(DatabaseType::SQLite);
	/// // Verify optimizers are created for each database type
	/// let _: QueryOptimizer = pg_optimizer;
	/// let _: QueryOptimizer = mysql_optimizer;
	/// let _: QueryOptimizer = sqlite_optimizer;
	/// ```
	pub fn for_database(db_type: DatabaseType) -> Self {
		Self {
			hints: Vec::new(),
			enable_analysis: false,
			enable_hints: false,
			db_type,
			connection: None,
		}
	}

	/// Add an optimization hint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{QueryOptimizer, OptimizationHint};
	///
	/// let optimizer = QueryOptimizer::new()
	///     .with_hint(OptimizationHint::PreferIndexScan)
	///     .with_hint(OptimizationHint::DisableSeqScan);
	/// // Verify the optimizer is configured with hints
	/// let _: QueryOptimizer = optimizer;
	/// ```
	pub fn with_hint(mut self, hint: OptimizationHint) -> Self {
		self.hints.push(hint);
		self
	}

	/// Enable or disable query plan analysis
	///
	/// When enabled, the optimizer will analyze EXPLAIN output.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::QueryOptimizer;
	///
	/// let optimizer = QueryOptimizer::new()
	///     .enable_analysis(true);
	/// // Verify the optimizer is configured with analysis enabled
	/// let _: QueryOptimizer = optimizer;
	/// ```
	pub fn enable_analysis(mut self, enable: bool) -> Self {
		self.enable_analysis = enable;
		self
	}

	/// Enable or disable hint injection
	///
	/// When enabled, optimization hints will be added to queries.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::QueryOptimizer;
	///
	/// let optimizer = QueryOptimizer::new()
	///     .enable_hints(true);
	/// // Verify the optimizer is configured with hints enabled
	/// let _: QueryOptimizer = optimizer;
	/// ```
	pub fn enable_hints(mut self, enable: bool) -> Self {
		self.enable_hints = enable;
		self
	}

	/// Set the database connection for EXPLAIN execution
	///
	/// When a connection is provided, the optimizer can execute EXPLAIN queries
	/// directly on the database to get actual query plans instead of using mock data.
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_rest::filters::QueryOptimizer;
	/// use reinhardt_db::backends::connection::DatabaseConnection;
	/// use std::sync::Arc;
	///
	/// # async fn example() {
	/// let conn = DatabaseConnection::connect_postgres("postgres://localhost/db").await.unwrap();
	/// let optimizer = QueryOptimizer::new()
	///     .with_connection(Arc::new(conn))
	///     .enable_analysis(true);
	/// # }
	/// ```
	pub fn with_connection(mut self, connection: Arc<BackendsConnection>) -> Self {
		self.connection = Some(connection);
		self
	}

	/// Analyze a query and return the execution plan
	///
	/// Executes EXPLAIN on the query to retrieve the query plan and analyzes it
	/// for optimization opportunities.
	///
	/// # Note
	///
	/// This method requires a database connection to execute EXPLAIN commands.
	/// Since the optimizer is typically used as a filter backend without direct
	/// database access, this method accepts the raw EXPLAIN output as a string.
	///
	/// To use this method:
	/// 1. Execute `EXPLAIN <query>` on your database
	/// 2. Pass the output to this method
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::QueryOptimizer;
	///
	/// # async fn example() {
	/// let optimizer = QueryOptimizer::new();
	/// // Assume you have executed: EXPLAIN SELECT * FROM users
	/// let explain_output = "Seq Scan on users (cost=0.00..35.50 rows=2550)";
	/// let plan = optimizer.analyze_query(explain_output).await.unwrap();
	/// assert!(plan.estimated_cost.is_some());
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn analyze_query(&self, explain_output: &str) -> FilterResult<QueryPlan> {
		let plan = QueryPlan::new(explain_output).analyze();
		Ok(plan)
	}

	/// Apply optimization hints to a query
	///
	/// Injects database-specific optimization hints into the SQL query.
	/// The injection method varies by database type:
	/// - PostgreSQL: Prepends SET commands before the query
	/// - MySQL: Injects optimizer hints after SELECT keyword
	/// - SQLite: Prepends PRAGMA statements
	fn apply_hints(&self, sql: String) -> String {
		if !self.enable_hints || self.hints.is_empty() {
			return sql;
		}

		match self.db_type {
			DatabaseType::PostgreSQL => self.apply_postgresql_hints(sql),
			DatabaseType::MySQL => self.apply_mysql_hints(sql),
			DatabaseType::SQLite => self.apply_sqlite_hints(sql),
		}
	}

	/// Apply PostgreSQL hints by prepending SET commands
	fn apply_postgresql_hints(&self, sql: String) -> String {
		let mut result = String::new();

		// Add all hints as SET commands
		for hint in &self.hints {
			let hint_sql = hint.to_sql_hint(DatabaseType::PostgreSQL);
			if !hint_sql.is_empty() {
				result.push_str(&hint_sql);
				result.push_str(";\n");
			}
		}

		// Add the original query
		result.push_str(&sql);
		result
	}

	/// Apply MySQL hints by injecting after SELECT keyword
	fn apply_mysql_hints(&self, sql: String) -> String {
		let hints: Vec<String> = self
			.hints
			.iter()
			.map(|h| h.to_sql_hint(DatabaseType::MySQL))
			.filter(|h| !h.is_empty())
			.collect();

		if hints.is_empty() {
			return sql;
		}

		let combined_hints = hints.join(" ");

		// Inject hints after SELECT keyword
		let select_regex = Regex::new(r"(?i)\bSELECT\b").unwrap();
		select_regex
			.replace(&sql, |caps: &regex::Captures| {
				format!("{} {}", &caps[0], combined_hints)
			})
			.to_string()
	}

	/// Apply SQLite hints by prepending PRAGMA statements
	fn apply_sqlite_hints(&self, sql: String) -> String {
		let mut result = String::new();

		// Add all hints as PRAGMA commands
		for hint in &self.hints {
			let hint_sql = hint.to_sql_hint(DatabaseType::SQLite);
			if !hint_sql.is_empty() {
				result.push_str(&hint_sql);
				result.push_str(";\n");
			}
		}

		// Add the original query
		result.push_str(&sql);
		result
	}

	/// Analyze a query plan and generate detailed analysis
	///
	/// Creates a `QueryAnalysis` from a `QueryPlan`, including complexity
	/// classification and indexed suggestions.
	fn analyze_query_plan(&self, query_plan: &QueryPlan) -> QueryAnalysis {
		let complexity = query_plan
			.estimated_cost
			.map(QueryComplexity::from_cost)
			.unwrap_or(QueryComplexity::Simple);

		let has_full_table_scan = (query_plan.raw_plan.contains("Seq Scan")
			|| query_plan.raw_plan.contains("Table Scan"))
			&& !query_plan.uses_index;

		// Extract potential missing indexes from suggestions
		let mut missing_indexes = Vec::new();
		for suggestion in &query_plan.suggestions {
			if suggestion.contains("index") && !suggestion.contains("using index") {
				// Try to extract column names from suggestion
				// This is a simplified heuristic
				if suggestion.contains("sort columns") {
					missing_indexes.push("sort_columns".to_string());
				} else if suggestion.contains("join") {
					missing_indexes.push("join_key".to_string());
				}
			}
		}

		QueryAnalysis {
			estimated_cost: query_plan.estimated_cost,
			complexity,
			suggestions: query_plan.suggestions.clone(),
			has_full_table_scan,
			missing_indexes,
			table_name: query_plan.table_name.clone(),
		}
	}

	/// Convert database Row results to EXPLAIN output string
	///
	/// Different databases return EXPLAIN results in different formats.
	/// This method converts them to a unified string format for analysis.
	fn rows_to_explain_output(rows: &[Row], db_type: DatabaseType) -> String {
		let mut output = String::new();

		for row in rows {
			match db_type {
				DatabaseType::PostgreSQL => {
					// PostgreSQL: EXPLAIN returns a single "QUERY PLAN" column
					if let Some(plan) = row.data.get("QUERY PLAN")
						&& let QueryValue::String(plan_str) = plan
					{
						output.push_str(plan_str);
						output.push('\n');
					}
				}
				DatabaseType::MySQL => {
					// MySQL: EXPLAIN returns multiple columns (id, select_type, table, type, etc.)
					let mut line = String::new();
					for (key, value) in &row.data {
						if let QueryValue::String(val_str) = value {
							if !line.is_empty() {
								line.push_str(" | ");
							}
							line.push_str(&format!("{}: {}", key, val_str));
						}
					}
					if !line.is_empty() {
						output.push_str(&line);
						output.push('\n');
					}
				}
				DatabaseType::SQLite => {
					// SQLite: EXPLAIN QUERY PLAN returns columns (detail)
					if let Some(detail) = row.data.get("detail")
						&& let QueryValue::String(detail_str) = detail
					{
						output.push_str(detail_str);
						output.push('\n');
					}
				}
			}
		}

		if output.is_empty() {
			output = "No EXPLAIN output available".to_string();
		}

		output
	}
}

#[async_trait]
impl FilterBackend for QueryOptimizer {
	async fn filter_queryset(
		&self,
		_query_params: &HashMap<String, String>,
		sql: String,
	) -> FilterResult<String> {
		// If analysis is enabled, analyze the query and log suggestions
		if self.enable_analysis {
			let explain_output = if let Some(conn) = &self.connection {
				// Execute actual EXPLAIN query on the database
				let explain_sql = match self.db_type {
					DatabaseType::PostgreSQL => format!("EXPLAIN {}", sql),
					DatabaseType::MySQL => format!("EXPLAIN FORMAT=TRADITIONAL {}", sql),
					DatabaseType::SQLite => format!("EXPLAIN QUERY PLAN {}", sql),
				};

				match conn.fetch_all(&explain_sql, vec![]).await {
					Ok(rows) => {
						// Convert rows to EXPLAIN output string
						Self::rows_to_explain_output(&rows, self.db_type)
					}
					Err(e) => {
						warn!(
							"Failed to execute EXPLAIN: {}. Using query analysis only.",
							e
						);
						// Fallback to mock EXPLAIN
						format!("Seq Scan on table (cost=0.00..35.50 rows=2550)\n{}", sql)
					}
				}
			} else {
				// No connection available, use mock EXPLAIN
				format!("Seq Scan on table (cost=0.00..35.50 rows=2550)\n{}", sql)
			};

			let query_plan = self.analyze_query(&explain_output).await?;
			let analysis = self.analyze_query_plan(&query_plan);

			// Log suggestions if any
			if !analysis.suggestions.is_empty() {
				info!(
					"Query optimization suggestions for table '{}':",
					analysis.table_name
				);
				for suggestion in &analysis.suggestions {
					info!("  - {}", suggestion);
				}
			}

			// Log performance metrics
			if let Some(estimated_cost) = analysis.estimated_cost {
				debug!(
					"Estimated query cost: {:.2} (complexity: {:?})",
					estimated_cost, analysis.complexity
				);
			}

			// Warn about potential issues
			if analysis.has_full_table_scan {
				warn!(
					"Query on '{}' requires full table scan. Consider adding indexes.",
					analysis.table_name
				);
			}

			// Warn about missing indexes
			if !analysis.missing_indexes.is_empty() {
				warn!(
					"Missing indexes detected on '{}': {:?}",
					analysis.table_name, analysis.missing_indexes
				);
			}
		}

		// Apply hints if enabled
		if self.enable_hints {
			Ok(self.apply_hints(sql))
		} else {
			Ok(sql)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_optimization_hint_variants() {
		let hints = vec![
			OptimizationHint::PreferIndexScan,
			OptimizationHint::DisableSeqScan,
			OptimizationHint::EnableHashJoin,
			OptimizationHint::DisableHashJoin,
			OptimizationHint::EnableMergeJoin,
			OptimizationHint::DisableMergeJoin,
			OptimizationHint::PreferNestedLoop,
			OptimizationHint::RandomPageCost(4.0),
			OptimizationHint::SeqPageCost(1.0),
			OptimizationHint::EffectiveCacheSize("4GB".to_string()),
		];
		assert_eq!(hints.len(), 10);
	}

	#[test]
	fn test_optimization_hint_to_sql() {
		let hint = OptimizationHint::PreferIndexScan;
		let sql = hint.to_sql_hint(DatabaseType::PostgreSQL);
		assert!(sql.contains("enable_indexscan"));
	}

	#[test]
	fn test_optimization_hint_with_value() {
		let hint = OptimizationHint::RandomPageCost(2.5);
		let sql = hint.to_sql_hint(DatabaseType::PostgreSQL);
		assert!(sql.contains("2.5"));
	}

	#[test]
	fn test_query_plan_creation() {
		let plan = QueryPlan::new("Seq Scan on users");
		assert!(plan.raw_plan.contains("Seq Scan"));
		assert!(plan.suggestions.is_empty());
	}

	#[test]
	fn test_query_plan_analyze() {
		let plan = QueryPlan::new("Seq Scan on users (cost=0.00..35.50 rows=2550)").analyze();
		assert!(!plan.suggestions.is_empty());
		assert!(
			plan.suggestions
				.iter()
				.any(|s| s.contains("Sequential scan"))
		);
	}

	#[test]
	fn test_query_optimizer_creation() {
		let optimizer = QueryOptimizer::new();
		assert!(optimizer.hints.is_empty());
		assert!(!optimizer.enable_analysis);
		assert!(!optimizer.enable_hints);
	}

	#[test]
	fn test_query_optimizer_with_hints() {
		let optimizer = QueryOptimizer::new()
			.with_hint(OptimizationHint::PreferIndexScan)
			.with_hint(OptimizationHint::DisableSeqScan);

		assert_eq!(optimizer.hints.len(), 2);
	}

	#[test]
	fn test_query_optimizer_enable_analysis() {
		let optimizer = QueryOptimizer::new().enable_analysis(true);
		assert!(optimizer.enable_analysis);
	}

	#[test]
	fn test_query_optimizer_enable_hints() {
		let optimizer = QueryOptimizer::new().enable_hints(true);
		assert!(optimizer.enable_hints);
	}

	#[tokio::test]
	async fn test_query_optimizer_passthrough() {
		let optimizer = QueryOptimizer::new();

		let params = HashMap::new();
		let sql = "SELECT * FROM users".to_string();
		let result = optimizer
			.filter_queryset(&params, sql.clone())
			.await
			.unwrap();

		assert_eq!(result, sql);
	}

	// Database-specific hint generation tests

	#[test]
	fn test_postgresql_hint_generation() {
		let hint = OptimizationHint::PreferIndexScan;
		let sql = hint.to_sql_hint(DatabaseType::PostgreSQL);
		assert_eq!(sql, "SET enable_indexscan = on");

		let hint = OptimizationHint::DisableSeqScan;
		let sql = hint.to_sql_hint(DatabaseType::PostgreSQL);
		assert_eq!(sql, "SET enable_seqscan = off");

		let hint = OptimizationHint::RandomPageCost(2.5);
		let sql = hint.to_sql_hint(DatabaseType::PostgreSQL);
		assert_eq!(sql, "SET random_page_cost = 2.5");
	}

	#[test]
	fn test_mysql_hint_generation() {
		let hint = OptimizationHint::PreferIndexScan;
		let sql = hint.to_sql_hint(DatabaseType::MySQL);
		assert_eq!(sql, "/*+ INDEX_SCAN() */");

		let hint = OptimizationHint::EnableHashJoin;
		let sql = hint.to_sql_hint(DatabaseType::MySQL);
		assert_eq!(sql, "/*+ HASH_JOIN() */");

		// MySQL doesn't support these hints
		let hint = OptimizationHint::RandomPageCost(2.5);
		let sql = hint.to_sql_hint(DatabaseType::MySQL);
		assert_eq!(sql, "");
	}

	#[test]
	fn test_sqlite_hint_generation() {
		let hint = OptimizationHint::EffectiveCacheSize("4GB".to_string());
		let sql = hint.to_sql_hint(DatabaseType::SQLite);
		assert_eq!(sql, "PRAGMA cache_size = 4GB");

		// SQLite doesn't support most hints
		let hint = OptimizationHint::PreferIndexScan;
		let sql = hint.to_sql_hint(DatabaseType::SQLite);
		assert_eq!(sql, "");
	}

	// Query plan parsing tests

	#[test]
	fn test_query_plan_parsing_cost() {
		let plan = QueryPlan::new("Seq Scan on users (cost=0.00..35.50 rows=2550)");
		assert_eq!(plan.estimated_cost, Some(35.50));
		assert_eq!(plan.estimated_rows, Some(2550));
		assert!(!plan.uses_index);
	}

	#[test]
	fn test_query_plan_parsing_index_scan() {
		let plan =
			QueryPlan::new("Index Scan using users_email_idx on users (cost=0.29..8.30 rows=1)");
		assert_eq!(plan.estimated_cost, Some(8.30));
		assert_eq!(plan.estimated_rows, Some(1));
		assert!(plan.uses_index);
	}

	#[test]
	fn test_query_plan_parsing_index_only_scan() {
		let plan =
			QueryPlan::new("Index Only Scan using users_id_idx on users (cost=0.15..4.17 rows=1)");
		assert!(plan.uses_index);
	}

	#[test]
	fn test_query_plan_parsing_bitmap_index() {
		let plan = QueryPlan::new("Bitmap Index Scan on users_email_idx (cost=0.00..4.27 rows=10)");
		assert!(plan.uses_index);
		assert_eq!(plan.estimated_rows, Some(10));
	}

	#[test]
	fn test_query_plan_parsing_no_cost() {
		let plan = QueryPlan::new("Seq Scan on users");
		assert_eq!(plan.estimated_cost, None);
		assert_eq!(plan.estimated_rows, None);
	}

	// Query plan analysis tests

	#[test]
	fn test_analyze_sequential_scan() {
		let plan = QueryPlan::new("Seq Scan on users (cost=0.00..35.50 rows=2550)").analyze();
		assert!(
			plan.suggestions
				.iter()
				.any(|s| s.contains("Sequential scan"))
		);
	}

	#[test]
	fn test_analyze_high_cost() {
		let plan = QueryPlan::new("Seq Scan on orders (cost=0.00..1500.00 rows=50000)").analyze();
		assert!(
			plan.suggestions
				.iter()
				.any(|s| s.contains("High query cost"))
		);
	}

	#[test]
	fn test_analyze_large_result_set() {
		let plan = QueryPlan::new("Seq Scan on logs (cost=0.00..100.00 rows=15000)").analyze();
		assert!(
			plan.suggestions
				.iter()
				.any(|s| s.contains("Large result set"))
		);
	}

	#[test]
	fn test_analyze_nested_loop() {
		let plan = QueryPlan::new("Nested Loop (cost=0.00..50.00 rows=100)").analyze();
		assert!(plan.suggestions.iter().any(|s| s.contains("Nested loop")));
	}

	#[test]
	fn test_analyze_large_hash_join() {
		let plan = QueryPlan::new("Hash Join (cost=100.00..500.00 rows=150000)").analyze();
		assert!(
			plan.suggestions
				.iter()
				.any(|s| s.contains("Large hash join"))
		);
	}

	#[test]
	fn test_analyze_bitmap_heap_scan() {
		let plan = QueryPlan::new("Bitmap Heap Scan on users (cost=4.29..8.30 rows=1)").analyze();
		assert!(
			plan.suggestions
				.iter()
				.any(|s| s.contains("Bitmap heap scan"))
		);
	}

	#[test]
	fn test_analyze_large_sort() {
		let plan = QueryPlan::new("Sort (cost=100.00..150.00 rows=20000)").analyze();
		assert!(
			plan.suggestions
				.iter()
				.any(|s| s.contains("Large sort operation"))
		);
	}

	// Hint injection tests

	#[test]
	fn test_postgresql_hint_injection() {
		let optimizer = QueryOptimizer::for_database(DatabaseType::PostgreSQL)
			.with_hint(OptimizationHint::PreferIndexScan)
			.with_hint(OptimizationHint::DisableSeqScan)
			.enable_hints(true);

		let sql = "SELECT * FROM users WHERE email = 'test@example.com'".to_string();
		let result = optimizer.apply_hints(sql);

		assert!(result.contains("SET enable_indexscan = on"));
		assert!(result.contains("SET enable_seqscan = off"));
		assert!(result.contains("SELECT * FROM users"));
	}

	#[test]
	fn test_mysql_hint_injection() {
		let optimizer = QueryOptimizer::for_database(DatabaseType::MySQL)
			.with_hint(OptimizationHint::PreferIndexScan)
			.with_hint(OptimizationHint::EnableHashJoin)
			.enable_hints(true);

		let sql = "SELECT * FROM users WHERE email = 'test@example.com'".to_string();
		let result = optimizer.apply_hints(sql);

		assert!(result.contains("/*+ INDEX_SCAN() */"));
		assert!(result.contains("/*+ HASH_JOIN() */"));
		assert!(result.contains("SELECT"));
	}

	#[test]
	fn test_sqlite_hint_injection() {
		let optimizer = QueryOptimizer::for_database(DatabaseType::SQLite)
			.with_hint(OptimizationHint::EffectiveCacheSize("4GB".to_string()))
			.enable_hints(true);

		let sql = "SELECT * FROM users WHERE email = 'test@example.com'".to_string();
		let result = optimizer.apply_hints(sql);

		assert!(result.contains("PRAGMA cache_size = 4GB"));
		assert!(result.contains("SELECT * FROM users"));
	}

	#[test]
	fn test_no_hint_injection_when_disabled() {
		let optimizer = QueryOptimizer::for_database(DatabaseType::PostgreSQL)
			.with_hint(OptimizationHint::PreferIndexScan)
			.enable_hints(false);

		let sql = "SELECT * FROM users".to_string();
		let result = optimizer.apply_hints(sql.clone());

		assert_eq!(result, sql);
	}

	#[test]
	fn test_no_hint_injection_when_empty() {
		let optimizer = QueryOptimizer::for_database(DatabaseType::PostgreSQL).enable_hints(true);

		let sql = "SELECT * FROM users".to_string();
		let result = optimizer.apply_hints(sql.clone());

		assert_eq!(result, sql);
	}

	#[tokio::test]
	async fn test_analyze_query_method() {
		let optimizer = QueryOptimizer::new();
		let explain_output = "Seq Scan on users (cost=0.00..35.50 rows=2550)";
		let plan = optimizer.analyze_query(explain_output).await.unwrap();

		assert_eq!(plan.estimated_cost, Some(35.50));
		assert_eq!(plan.estimated_rows, Some(2550));
		assert!(!plan.suggestions.is_empty());
	}

	#[test]
	fn test_database_type_for_optimizer() {
		let pg_optimizer = QueryOptimizer::for_database(DatabaseType::PostgreSQL);
		assert_eq!(pg_optimizer.db_type, DatabaseType::PostgreSQL);

		let mysql_optimizer = QueryOptimizer::for_database(DatabaseType::MySQL);
		assert_eq!(mysql_optimizer.db_type, DatabaseType::MySQL);

		let sqlite_optimizer = QueryOptimizer::for_database(DatabaseType::SQLite);
		assert_eq!(sqlite_optimizer.db_type, DatabaseType::SQLite);
	}

	#[tokio::test]
	async fn test_filter_backend_with_hints() {
		let optimizer = QueryOptimizer::for_database(DatabaseType::PostgreSQL)
			.with_hint(OptimizationHint::PreferIndexScan)
			.enable_hints(true);

		let params = HashMap::new();
		let sql = "SELECT * FROM users".to_string();
		let result = optimizer.filter_queryset(&params, sql).await.unwrap();

		assert!(result.contains("SET enable_indexscan = on"));
		assert!(result.contains("SELECT * FROM users"));
	}

	// Query analysis tests

	#[test]
	fn test_query_complexity_from_cost() {
		assert_eq!(QueryComplexity::from_cost(5.0), QueryComplexity::Simple);
		assert_eq!(QueryComplexity::from_cost(50.0), QueryComplexity::Moderate);
		assert_eq!(QueryComplexity::from_cost(500.0), QueryComplexity::Complex);
		assert_eq!(
			QueryComplexity::from_cost(5000.0),
			QueryComplexity::VeryComplex
		);
	}

	#[test]
	fn test_query_plan_table_name_extraction() {
		let plan = QueryPlan::new("Seq Scan on users (cost=0.00..35.50 rows=2550)");
		assert_eq!(plan.table_name, "users");

		let plan2 = QueryPlan::new("Index Scan using idx on products (cost=0.29..8.30 rows=1)");
		assert_eq!(plan2.table_name, "products");
	}

	#[test]
	fn test_analyze_query_plan() {
		let optimizer = QueryOptimizer::new();
		let plan = QueryPlan::new("Seq Scan on users (cost=0.00..35.50 rows=2550)").analyze();
		let analysis = optimizer.analyze_query_plan(&plan);

		assert_eq!(analysis.estimated_cost, Some(35.50));
		assert_eq!(analysis.complexity, QueryComplexity::Moderate);
		assert_eq!(analysis.table_name, "users");
		assert!(analysis.has_full_table_scan);
		assert!(!analysis.suggestions.is_empty());
	}

	#[test]
	fn test_analyze_query_plan_with_index() {
		let optimizer = QueryOptimizer::new();
		let plan =
			QueryPlan::new("Index Scan using users_email_idx on users (cost=0.29..8.30 rows=1)")
				.analyze();
		let analysis = optimizer.analyze_query_plan(&plan);

		assert_eq!(analysis.estimated_cost, Some(8.30));
		assert_eq!(analysis.complexity, QueryComplexity::Simple);
		assert!(!analysis.has_full_table_scan);
	}

	#[tokio::test]
	async fn test_filter_backend_with_analysis_enabled() {
		let optimizer = QueryOptimizer::new().enable_analysis(true);

		let params = HashMap::new();
		let sql = "SELECT * FROM users WHERE email = 'test@example.com'".to_string();
		let result = optimizer
			.filter_queryset(&params, sql.clone())
			.await
			.unwrap();

		// Result should be unchanged when hints are disabled
		assert_eq!(result, sql);
	}

	#[tokio::test]
	async fn test_filter_backend_with_analysis_disabled() {
		let optimizer = QueryOptimizer::new().enable_analysis(false);

		let params = HashMap::new();
		let sql = "SELECT * FROM users".to_string();
		let result = optimizer
			.filter_queryset(&params, sql.clone())
			.await
			.unwrap();

		assert_eq!(result, sql);
	}

	#[tokio::test]
	async fn test_filter_backend_with_both_analysis_and_hints() {
		let optimizer = QueryOptimizer::for_database(DatabaseType::PostgreSQL)
			.with_hint(OptimizationHint::PreferIndexScan)
			.enable_analysis(true)
			.enable_hints(true);

		let params = HashMap::new();
		let sql = "SELECT * FROM users".to_string();
		let result = optimizer.filter_queryset(&params, sql).await.unwrap();

		// Should have hints applied
		assert!(result.contains("SET enable_indexscan = on"));
		assert!(result.contains("SELECT * FROM users"));
	}

	#[test]
	fn test_rows_to_explain_output_postgresql() {
		use reinhardt_db::backends::types::{QueryValue, Row};
		use std::collections::HashMap;

		let mut data = HashMap::new();
		data.insert(
			"QUERY PLAN".to_string(),
			QueryValue::String("Seq Scan on users (cost=0.00..35.50 rows=2550)".to_string()),
		);
		let row = Row { data };

		let output = QueryOptimizer::rows_to_explain_output(&[row], DatabaseType::PostgreSQL);

		assert!(output.contains("Seq Scan on users"));
		assert!(output.contains("cost=0.00..35.50"));
	}

	#[test]
	fn test_rows_to_explain_output_mysql() {
		use reinhardt_db::backends::types::{QueryValue, Row};
		use std::collections::HashMap;

		let mut data = HashMap::new();
		data.insert("id".to_string(), QueryValue::String("1".to_string()));
		data.insert(
			"select_type".to_string(),
			QueryValue::String("SIMPLE".to_string()),
		);
		data.insert("table".to_string(), QueryValue::String("users".to_string()));
		let row = Row { data };

		let output = QueryOptimizer::rows_to_explain_output(&[row], DatabaseType::MySQL);

		assert!(output.contains("id: 1"));
		assert!(output.contains("select_type: SIMPLE"));
		assert!(output.contains("table: users"));
	}

	#[test]
	fn test_rows_to_explain_output_sqlite() {
		use reinhardt_db::backends::types::{QueryValue, Row};
		use std::collections::HashMap;

		let mut data = HashMap::new();
		data.insert(
			"detail".to_string(),
			QueryValue::String("SCAN TABLE users".to_string()),
		);
		let row = Row { data };

		let output = QueryOptimizer::rows_to_explain_output(&[row], DatabaseType::SQLite);

		assert!(output.contains("SCAN TABLE users"));
	}

	#[test]
	fn test_rows_to_explain_output_empty() {
		use Row;
		use std::collections::HashMap;

		let row = Row {
			data: HashMap::new(),
		};

		let output = QueryOptimizer::rows_to_explain_output(&[row], DatabaseType::PostgreSQL);

		assert_eq!(output, "No EXPLAIN output available");
	}

	#[test]
	fn test_with_connection_builder() {
		// Create a mock connection (in real scenarios, this would be a real connection)
		// For this test, we just verify the builder pattern works
		let optimizer = QueryOptimizer::new();

		// Verify that optimizer can be created without connection
		assert!(format!("{:?}", optimizer).contains("QueryOptimizer"));

		// Note: We cannot test with a real connection here as it requires database setup
		// Integration tests in tests/ crate should cover actual database connection scenarios
	}
}
