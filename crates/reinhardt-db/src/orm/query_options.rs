//! # Query Options
//!
//! Advanced query execution options inspired by SQLAlchemy.
//!
//! This module provides options for controlling query execution behavior:
//! - Load options for relationship loading strategies
//! - Execution options for query hints and optimizations
//! - Population options for updating existing objects
//! - Locking options for SELECT FOR UPDATE
//!
//! This module is inspired by SQLAlchemy's query.py and execution_options
//! Copyright 2005-2025 SQLAlchemy authors and contributors
//! Licensed under MIT License. See THIRD-PARTY-NOTICES for details.

use crate::orm::loading::LoadOption;
use std::collections::HashMap;

/// Query execution options
/// Controls how queries are executed and results are processed
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ExecutionOptions {
	/// Compiled SQL cache options
	pub compiled_cache: Option<CompiledCacheOption>,
	/// Isolation level for this query
	pub isolation_level: Option<IsolationLevel>,
	/// Query timeout in seconds
	pub timeout: Option<u64>,
	/// Whether to use autocommit
	pub autocommit: bool,
	/// Schema name to use
	pub schema_translate_map: HashMap<String, String>,
	/// Custom execution options
	pub custom: HashMap<String, String>,
}

impl ExecutionOptions {
	/// Create new execution options with defaults
	pub fn new() -> Self {
		Self {
			compiled_cache: None,
			isolation_level: None,
			timeout: None,
			autocommit: false,
			schema_translate_map: HashMap::new(),
			custom: HashMap::new(),
		}
	}
	/// Set compiled cache option
	pub fn with_compiled_cache(mut self, option: CompiledCacheOption) -> Self {
		self.compiled_cache = Some(option);
		self
	}
	/// Set isolation level
	pub fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
		self.isolation_level = Some(level);
		self
	}
	/// Set query timeout
	pub fn with_timeout(mut self, seconds: u64) -> Self {
		self.timeout = Some(seconds);
		self
	}
	/// Enable autocommit
	///
	pub fn autocommit(mut self) -> Self {
		self.autocommit = true;
		self
	}
	/// Add schema translation
	///
	pub fn translate_schema(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
		self.schema_translate_map.insert(from.into(), to.into());
		self
	}
	/// Add custom option
	pub fn with_custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.custom.insert(key.into(), value.into());
		self
	}
}

impl Default for ExecutionOptions {
	fn default() -> Self {
		Self::new()
	}
}

/// Compiled cache options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompiledCacheOption {
	/// Use compiled cache
	Use,
	/// Don't use compiled cache
	NoCache,
	/// Clear cache before execution
	Clear,
}

/// Isolation level for transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
	ReadUncommitted,
	ReadCommitted,
	RepeatableRead,
	Serializable,
}

impl IsolationLevel {
	/// Convert to SQL string
	///
	pub fn to_sql(&self) -> &'static str {
		match self {
			IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
			IsolationLevel::ReadCommitted => "READ COMMITTED",
			IsolationLevel::RepeatableRead => "REPEATABLE READ",
			IsolationLevel::Serializable => "SERIALIZABLE",
		}
	}
}

/// Query options combining load and execution options
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct QueryOptions {
	/// Load options for relationships
	pub load_options: Vec<LoadOption>,
	/// Execution options
	pub execution_options: ExecutionOptions,
	/// Populate existing objects in session
	pub populate_existing: bool,
	/// Use SELECT FOR UPDATE
	pub with_for_update: ForUpdateMode,
	/// Yield per batch size
	pub yield_per: Option<usize>,
}

impl QueryOptions {
	/// Create new query options
	pub fn new() -> Self {
		Self {
			load_options: Vec::new(),
			execution_options: ExecutionOptions::new(),
			populate_existing: false,
			with_for_update: ForUpdateMode::None,
			yield_per: None,
		}
	}
	/// Add a load option
	///
	pub fn add_load_option(mut self, option: LoadOption) -> Self {
		self.load_options.push(option);
		self
	}
	/// Add multiple load options
	pub fn with_load_options(mut self, options: Vec<LoadOption>) -> Self {
		self.load_options.extend(options);
		self
	}
	/// Set execution options
	pub fn with_execution_options(mut self, options: ExecutionOptions) -> Self {
		self.execution_options = options;
		self
	}
	/// Enable populate existing (refresh objects already in session)
	/// SQLAlchemy: query.populate_existing()
	///
	pub fn populate_existing(mut self) -> Self {
		self.populate_existing = true;
		self
	}
	/// Enable SELECT FOR UPDATE
	/// SQLAlchemy: query.with_for_update()
	pub fn with_for_update(mut self, mode: ForUpdateMode) -> Self {
		self.with_for_update = mode;
		self
	}
	/// Set yield per batch size
	/// SQLAlchemy: query.yield_per(100)
	pub fn with_yield_per(mut self, batch_size: usize) -> Self {
		self.yield_per = Some(batch_size);
		self
	}
	/// Generate SQL hints for query
	///
	pub fn sql_hints(&self) -> Vec<String> {
		let mut hints = Vec::new();

		// Add load option hints
		for opt in &self.load_options {
			if let Some(hint) = opt.strategy().sql_hint() {
				hints.push(hint.to_string());
			}
		}

		// Add FOR UPDATE hint
		if self.with_for_update != ForUpdateMode::None {
			hints.push(self.with_for_update.to_sql().to_string());
		}

		hints
	}
}

impl Default for QueryOptions {
	fn default() -> Self {
		Self::new()
	}
}

/// FOR UPDATE locking modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForUpdateMode {
	/// No locking
	None,
	/// Standard FOR UPDATE
	Standard,
	/// FOR UPDATE NOWAIT
	NoWait,
	/// FOR UPDATE SKIP LOCKED
	SkipLocked,
	/// FOR SHARE (read lock)
	Share,
	/// FOR KEY SHARE
	KeyShare,
	/// FOR NO KEY UPDATE
	NoKeyUpdate,
}

impl ForUpdateMode {
	/// Convert to SQL clause
	///
	pub fn to_sql(&self) -> &'static str {
		match self {
			ForUpdateMode::None => "",
			ForUpdateMode::Standard => "FOR UPDATE",
			ForUpdateMode::NoWait => "FOR UPDATE NOWAIT",
			ForUpdateMode::SkipLocked => "FOR UPDATE SKIP LOCKED",
			ForUpdateMode::Share => "FOR SHARE",
			ForUpdateMode::KeyShare => "FOR KEY SHARE",
			ForUpdateMode::NoKeyUpdate => "FOR NO KEY UPDATE",
		}
	}
	/// Check if this is a locking mode
	///
	pub fn is_locking(&self) -> bool {
		*self != ForUpdateMode::None
	}
}

/// Builder for query options with fluent API
#[non_exhaustive]
pub struct QueryOptionsBuilder {
	options: QueryOptions,
}

impl QueryOptionsBuilder {
	/// Create new builder
	pub fn new() -> Self {
		Self {
			options: QueryOptions::new(),
		}
	}
	/// Add load option
	pub fn load(mut self, option: LoadOption) -> Self {
		self.options.load_options.push(option);
		self
	}
	/// Enable populate existing
	///
	pub fn populate_existing(mut self) -> Self {
		self.options.populate_existing = true;
		self
	}
	/// Set FOR UPDATE mode
	///
	pub fn for_update(mut self, mode: ForUpdateMode) -> Self {
		self.options.with_for_update = mode;
		self
	}
	/// Set execution timeout
	///
	pub fn timeout(mut self, seconds: u64) -> Self {
		self.options.execution_options.timeout = Some(seconds);
		self
	}
	/// Set isolation level
	///
	pub fn isolation_level(mut self, level: IsolationLevel) -> Self {
		self.options.execution_options.isolation_level = Some(level);
		self
	}
	/// Set yield per batch size
	///
	pub fn yield_per(mut self, batch_size: usize) -> Self {
		self.options.yield_per = Some(batch_size);
		self
	}
	/// Build the query options
	///
	pub fn build(self) -> QueryOptions {
		self.options
	}
}

impl Default for QueryOptionsBuilder {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::loading::{joinedload, selectinload};

	#[test]
	fn test_execution_options() {
		let opts = ExecutionOptions::new()
			.with_timeout(30)
			.with_isolation_level(IsolationLevel::Serializable)
			.autocommit();

		assert_eq!(opts.timeout, Some(30));
		assert_eq!(opts.isolation_level, Some(IsolationLevel::Serializable));
		assert!(opts.autocommit);
	}

	#[test]
	fn test_isolation_level_sql() {
		assert_eq!(IsolationLevel::ReadUncommitted.to_sql(), "READ UNCOMMITTED");
		assert_eq!(IsolationLevel::ReadCommitted.to_sql(), "READ COMMITTED");
		assert_eq!(IsolationLevel::RepeatableRead.to_sql(), "REPEATABLE READ");
		assert_eq!(IsolationLevel::Serializable.to_sql(), "SERIALIZABLE");
	}

	#[test]
	fn test_query_options() {
		let opts = QueryOptions::new()
			.add_load_option(joinedload("posts"))
			.add_load_option(selectinload("comments"))
			.populate_existing()
			.with_for_update(ForUpdateMode::NoWait);

		assert_eq!(opts.load_options.len(), 2);
		assert!(opts.populate_existing);
		assert_eq!(opts.with_for_update, ForUpdateMode::NoWait);
	}

	#[test]
	fn test_for_update_modes() {
		assert_eq!(ForUpdateMode::Standard.to_sql(), "FOR UPDATE");
		assert_eq!(ForUpdateMode::NoWait.to_sql(), "FOR UPDATE NOWAIT");
		assert_eq!(ForUpdateMode::SkipLocked.to_sql(), "FOR UPDATE SKIP LOCKED");
		assert_eq!(ForUpdateMode::Share.to_sql(), "FOR SHARE");
		assert_eq!(ForUpdateMode::KeyShare.to_sql(), "FOR KEY SHARE");
		assert_eq!(ForUpdateMode::NoKeyUpdate.to_sql(), "FOR NO KEY UPDATE");

		assert!(!ForUpdateMode::None.is_locking());
		assert!(ForUpdateMode::Standard.is_locking());
	}

	#[test]
	fn test_query_options_builder() {
		let opts = QueryOptionsBuilder::new()
			.load(joinedload("author"))
			.populate_existing()
			.for_update(ForUpdateMode::SkipLocked)
			.timeout(60)
			.isolation_level(IsolationLevel::RepeatableRead)
			.yield_per(100)
			.build();

		assert_eq!(opts.load_options.len(), 1);
		assert!(opts.populate_existing);
		assert_eq!(opts.with_for_update, ForUpdateMode::SkipLocked);
		assert_eq!(opts.execution_options.timeout, Some(60));
		assert_eq!(
			opts.execution_options.isolation_level,
			Some(IsolationLevel::RepeatableRead)
		);
		assert_eq!(opts.yield_per, Some(100));
	}

	#[test]
	fn test_query_options_sql_hints() {
		let opts = QueryOptions::new()
			.add_load_option(joinedload("posts"))
			.add_load_option(selectinload("comments"))
			.with_for_update(ForUpdateMode::NoWait);

		let hints = opts.sql_hints();
		assert!(hints.len() >= 2); // At least load hints + FOR UPDATE
		assert!(hints.iter().any(|h| h.contains("FOR UPDATE NOWAIT")));
	}

	#[test]
	fn test_schema_translation() {
		let opts = ExecutionOptions::new()
			.translate_schema("old_schema", "new_schema")
			.translate_schema("test", "production");

		assert_eq!(opts.schema_translate_map.len(), 2);
		assert_eq!(
			opts.schema_translate_map.get("old_schema"),
			Some(&"new_schema".to_string())
		);
	}

	#[test]
	fn test_custom_options() {
		let opts = ExecutionOptions::new()
			.with_custom("max_rows", "1000")
			.with_custom("enable_cache", "true");

		assert_eq!(opts.custom.len(), 2);
		assert_eq!(opts.custom.get("max_rows"), Some(&"1000".to_string()));
	}

	#[test]
	fn test_compiled_cache_options() {
		let opts = ExecutionOptions::new().with_compiled_cache(CompiledCacheOption::Use);

		assert_eq!(opts.compiled_cache, Some(CompiledCacheOption::Use));

		let opts2 = ExecutionOptions::new().with_compiled_cache(CompiledCacheOption::NoCache);

		assert_eq!(opts2.compiled_cache, Some(CompiledCacheOption::NoCache));
	}
}
