//! Test data builders for reinhardt-debug-toolbar tests
//!
//! This module provides fluent builder APIs for creating test data.

use chrono::Utc;
use reinhardt_debug_toolbar::context::{
	CacheOperation, MarkerCategory, PerformanceMarker, SqlQuery, TemplateInfo,
};
use std::time::Duration;

/// Builder for creating SqlQuery test data
///
/// # Example
///
/// ```rust
/// use reinhardt_debug_toolbar_tests::builders::SqlQueryBuilder;
///
/// let query = SqlQueryBuilder::new()
///     .sql("SELECT * FROM users")
///     .duration(Duration::from_millis(50))
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
pub struct SqlQueryBuilder {
	sql: String,
	params: Vec<String>,
	duration: Duration,
	stack_trace: String,
	timestamp: chrono::DateTime<chrono::Utc>,
	connection: Option<String>,
}

impl SqlQueryBuilder {
	/// Create a new SqlQueryBuilder with default values
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the SQL query text
	pub fn sql(mut self, sql: impl Into<String>) -> Self {
		self.sql = sql.into();
		self
	}

	/// Set query parameters
	pub fn params(mut self, params: Vec<String>) -> Self {
		self.params = params;
		self
	}

	/// Set execution duration
	pub fn duration(mut self, duration: Duration) -> Self {
		self.duration = duration;
		self
	}

	/// Set stack trace
	pub fn stack_trace(mut self, stack_trace: impl Into<String>) -> Self {
		self.stack_trace = stack_trace.into();
		self
	}

	/// Set timestamp
	pub fn timestamp(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
		self.timestamp = timestamp;
		self
	}

	/// Set connection name
	pub fn connection(mut self, connection: impl Into<String>) -> Self {
		self.connection = Some(connection.into());
		self
	}

	/// Build the SqlQuery
	pub fn build(self) -> SqlQuery {
		SqlQuery {
			sql: self.sql,
			params: self.params,
			duration: self.duration,
			stack_trace: self.stack_trace,
			timestamp: self.timestamp,
			connection: self.connection,
		}
	}
}

/// Builder for creating TemplateInfo test data
///
/// # Example
///
/// ```rust
/// use reinhardt_debug_toolbar_tests::builders::TemplateInfoBuilder;
/// use std::time::Duration;
///
/// let template = TemplateInfoBuilder::new()
///     .name("base.html")
///     .render_duration(Duration::from_millis(10))
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
pub struct TemplateInfoBuilder {
	name: String,
	render_duration: Duration,
	context_data: serde_json::Value,
	parent: Option<String>,
	timestamp: chrono::DateTime<chrono::Utc>,
}

impl TemplateInfoBuilder {
	/// Create a new TemplateInfoBuilder with default values
	pub fn new() -> Self {
		Self::default()
	}

	/// Set template name
	pub fn name(mut self, name: impl Into<String>) -> Self {
		self.name = name.into();
		self
	}

	/// Set rendering duration
	pub fn render_duration(mut self, duration: Duration) -> Self {
		self.render_duration = duration;
		self
	}

	/// Set context data
	pub fn context_data(mut self, data: serde_json::Value) -> Self {
		self.context_data = data;
		self
	}

	/// Set parent template
	pub fn parent(mut self, parent: impl Into<String>) -> Self {
		self.parent = Some(parent.into());
		self
	}

	/// Set timestamp
	pub fn timestamp(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
		self.timestamp = timestamp;
		self
	}

	/// Build the TemplateInfo
	pub fn build(self) -> TemplateInfo {
		TemplateInfo {
			name: self.name,
			render_duration: self.render_duration,
			context_data: self.context_data,
			parent: self.parent,
			timestamp: self.timestamp,
		}
	}
}

/// Builder for creating CacheOperation test data
///
/// # Example
///
/// ```rust
/// use reinhardt_debug_toolbar_tests::builders::CacheOperationBuilder;
/// use std::time::Duration;
///
/// let get_op = CacheOperationBuilder::get()
///     .key("user:123")
///     .hit(true)
///     .duration(Duration::from_millis(1))
///     .build();
///
/// let set_op = CacheOperationBuilder::set()
///     .key("user:123")
///     .duration(Duration::from_millis(2))
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct CacheOperationBuilder {
	operation: CacheOperationBuilderType,
}

#[derive(Debug, Clone)]
enum CacheOperationBuilderType {
	Get {
		key: String,
		hit: bool,
		duration: Duration,
		timestamp: chrono::DateTime<chrono::Utc>,
	},
	Set {
		key: String,
		duration: Duration,
		timestamp: chrono::DateTime<chrono::Utc>,
	},
	Delete {
		key: String,
		duration: Duration,
		timestamp: chrono::DateTime<chrono::Utc>,
	},
}

impl CacheOperationBuilder {
	/// Create a GET cache operation builder
	pub fn get() -> Self {
		Self {
			operation: CacheOperationBuilderType::Get {
				key: String::new(),
				hit: false,
				duration: Duration::default(),
				timestamp: Utc::now(),
			},
		}
	}

	/// Create a SET cache operation builder
	pub fn set() -> Self {
		Self {
			operation: CacheOperationBuilderType::Set {
				key: String::new(),
				duration: Duration::default(),
				timestamp: Utc::now(),
			},
		}
	}

	/// Create a DELETE cache operation builder
	pub fn delete() -> Self {
		Self {
			operation: CacheOperationBuilderType::Delete {
				key: String::new(),
				duration: Duration::default(),
				timestamp: Utc::now(),
			},
		}
	}

	/// Set cache key
	pub fn key(mut self, key: impl Into<String>) -> Self {
		match &mut self.operation {
			CacheOperationBuilderType::Get { key: k, .. } => *k = key.into(),
			CacheOperationBuilderType::Set { key: k, .. } => *k = key.into(),
			CacheOperationBuilderType::Delete { key: k, .. } => *k = key.into(),
		}
		self
	}

	/// Set hit status (for GET operations only)
	pub fn hit(mut self, hit: bool) -> Self {
		if let CacheOperationBuilderType::Get { hit: h, .. } = &mut self.operation {
			*h = hit;
		}
		self
	}

	/// Set operation duration
	pub fn duration(mut self, duration: Duration) -> Self {
		match &mut self.operation {
			CacheOperationBuilderType::Get { duration: d, .. } => *d = duration,
			CacheOperationBuilderType::Set { duration: d, .. } => *d = duration,
			CacheOperationBuilderType::Delete { duration: d, .. } => *d = duration,
		}
		self
	}

	/// Set timestamp
	pub fn timestamp(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
		match &mut self.operation {
			CacheOperationBuilderType::Get { timestamp: t, .. } => *t = timestamp,
			CacheOperationBuilderType::Set { timestamp: t, .. } => *t = timestamp,
			CacheOperationBuilderType::Delete { timestamp: t, .. } => *t = timestamp,
		}
		self
	}

	/// Build the CacheOperation
	pub fn build(self) -> CacheOperation {
		match self.operation {
			CacheOperationBuilderType::Get {
				key,
				hit,
				duration,
				timestamp,
			} => CacheOperation::Get {
				key,
				hit,
				duration,
				timestamp,
			},
			CacheOperationBuilderType::Set {
				key,
				duration,
				timestamp,
			} => CacheOperation::Set {
				key,
				duration,
				timestamp,
			},
			CacheOperationBuilderType::Delete {
				key,
				duration,
				timestamp,
			} => CacheOperation::Delete {
				key,
				duration,
				timestamp,
			},
		}
	}
}

/// Builder for creating PerformanceMarker test data
///
/// # Example
///
/// ```rust
/// use reinhardt_debug_toolbar_tests::builders::PerformanceMarkerBuilder;
/// use reinhardt_debug_toolbar::context::MarkerCategory;
/// use std::time::Duration;
///
/// let marker = PerformanceMarkerBuilder::new()
///     .name("AuthMiddleware")
///     .category(MarkerCategory::Middleware)
///     .start(Duration::from_millis(0))
///     .end(Duration::from_millis(5))
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct PerformanceMarkerBuilder {
	name: String,
	start: Duration,
	end: Duration,
	category: MarkerCategory,
}

impl PerformanceMarkerBuilder {
	/// Create a new PerformanceMarkerBuilder with default values
	pub fn new() -> Self {
		Self {
			name: String::new(),
			start: Duration::default(),
			end: Duration::default(),
			category: MarkerCategory::Other,
		}
	}

	/// Set marker name
	pub fn name(mut self, name: impl Into<String>) -> Self {
		self.name = name.into();
		self
	}

	/// Set start time (relative to request start)
	pub fn start(mut self, start: Duration) -> Self {
		self.start = start;
		self
	}

	/// Set end time (relative to request start)
	pub fn end(mut self, end: Duration) -> Self {
		self.end = end;
		self
	}

	/// Set marker category
	pub fn category(mut self, category: MarkerCategory) -> Self {
		self.category = category;
		self
	}

	/// Build the PerformanceMarker
	pub fn build(self) -> PerformanceMarker {
		PerformanceMarker {
			name: self.name,
			start: self.start,
			end: self.end,
			category: self.category,
		}
	}
}
