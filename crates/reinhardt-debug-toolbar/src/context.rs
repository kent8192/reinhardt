//! Toolbar context and task-local storage
//!
//! This module provides per-request context storage using Tokio's task-local storage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// Task-local storage for toolbar context
tokio::task_local! {
	/// Task-local storage for toolbar context
	///
	/// This context is available to all code running within the same async task.
	/// Automatically cleaned up when the task completes.
	pub static TOOLBAR_CONTEXT: ToolbarContext;
}

/// Helper macro to safely access toolbar context
///
/// Returns `None` if context is not available (toolbar disabled).
#[macro_export]
macro_rules! with_toolbar_context {
	($expr:expr) => {
		$crate::context::TOOLBAR_CONTEXT.try_with(|ctx| $expr).ok()
	};
}

/// Per-request debug toolbar context
#[derive(Debug, Clone)]
pub struct ToolbarContext {
	/// Request start time (for total duration calculation)
	pub start_time: Instant,

	/// Request information (method, path, headers, etc.)
	pub request_info: RequestInfo,

	/// SQL queries executed during request (thread-safe)
	pub sql_queries: Arc<Mutex<Vec<SqlQuery>>>,

	/// Templates rendered during request (thread-safe)
	pub templates: Arc<Mutex<Vec<TemplateInfo>>>,

	/// Cache operations performed during request (thread-safe)
	pub cache_ops: Arc<Mutex<Vec<CacheOperation>>>,

	/// Performance markers (middleware, handlers, etc.)
	pub performance_markers: Arc<Mutex<Vec<PerformanceMarker>>>,

	/// List of enabled panel IDs
	pub enabled_panels: Vec<String>,
}

impl ToolbarContext {
	/// Create new toolbar context
	pub fn new(request_info: RequestInfo) -> Self {
		Self {
			start_time: Instant::now(),
			request_info,
			sql_queries: Arc::new(Mutex::new(Vec::new())),
			templates: Arc::new(Mutex::new(Vec::new())),
			cache_ops: Arc::new(Mutex::new(Vec::new())),
			performance_markers: Arc::new(Mutex::new(Vec::new())),
			enabled_panels: Vec::new(),
		}
	}

	/// Record SQL query
	pub fn record_sql_query(&self, query: SqlQuery) {
		let mut queries = self.sql_queries.lock().unwrap();

		// Limit query count to prevent memory exhaustion
		const MAX_QUERIES: usize = 1000;
		if queries.len() >= MAX_QUERIES {
			queries.remove(0); // Remove oldest
		}

		queries.push(query);
	}

	/// Record template rendering
	pub fn record_template(&self, template: TemplateInfo) {
		let mut templates = self.templates.lock().unwrap();

		// Limit template count
		const MAX_TEMPLATES: usize = 100;
		if templates.len() >= MAX_TEMPLATES {
			templates.remove(0);
		}

		templates.push(template);
	}

	/// Record cache operation
	pub fn record_cache_op(&self, op: CacheOperation) {
		let mut ops = self.cache_ops.lock().unwrap();

		// Limit cache operation count
		const MAX_CACHE_OPS: usize = 1000;
		if ops.len() >= MAX_CACHE_OPS {
			ops.remove(0);
		}

		ops.push(op);
	}

	/// Add performance marker
	pub fn add_marker(&self, marker: PerformanceMarker) {
		let mut markers = self.performance_markers.lock().unwrap();

		// Limit marker count
		const MAX_MARKERS: usize = 500;
		if markers.len() >= MAX_MARKERS {
			markers.remove(0);
		}

		markers.push(marker);
	}

	/// Get elapsed time since request start
	pub fn elapsed(&self) -> Duration {
		self.start_time.elapsed()
	}

	/// Get snapshot of SQL queries
	pub fn get_sql_queries(&self) -> Vec<SqlQuery> {
		self.sql_queries.lock().unwrap().clone()
	}

	/// Get snapshot of templates
	pub fn get_templates(&self) -> Vec<TemplateInfo> {
		self.templates.lock().unwrap().clone()
	}

	/// Get snapshot of cache operations
	pub fn get_cache_ops(&self) -> Vec<CacheOperation> {
		self.cache_ops.lock().unwrap().clone()
	}

	/// Get snapshot of performance markers
	pub fn get_performance_markers(&self) -> Vec<PerformanceMarker> {
		self.performance_markers.lock().unwrap().clone()
	}
}

/// Request information
#[derive(Debug, Clone, Serialize)]
pub struct RequestInfo {
	/// HTTP method (GET, POST, etc.)
	pub method: String,

	/// Request path
	pub path: String,

	/// Query string
	pub query: Option<String>,

	/// Request headers
	pub headers: Vec<(String, String)>,

	/// Client IP address
	pub client_ip: String,

	/// Request timestamp
	pub timestamp: DateTime<Utc>,
}

/// SQL query information
#[derive(Debug, Clone, Serialize)]
pub struct SqlQuery {
	/// SQL query text
	pub sql: String,

	/// Query parameters
	pub params: Vec<String>,

	/// Execution duration
	#[serde(serialize_with = "serialize_duration_ms")]
	pub duration: Duration,

	/// Stack trace (captured in debug mode only)
	pub stack_trace: String,

	/// Query timestamp
	pub timestamp: DateTime<Utc>,

	/// Database connection name
	pub connection: Option<String>,
}

/// Template rendering information
#[derive(Debug, Clone, Serialize)]
pub struct TemplateInfo {
	/// Template name
	pub name: String,

	/// Rendering duration
	#[serde(serialize_with = "serialize_duration_ms")]
	pub render_duration: Duration,

	/// Template context data
	pub context_data: serde_json::Value,

	/// Parent template (for inheritance)
	pub parent: Option<String>,

	/// Rendering timestamp
	pub timestamp: DateTime<Utc>,
}

/// Cache operation information
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum CacheOperation {
	/// Cache GET operation
	Get {
		/// Cache key
		key: String,
		/// Whether the key was found (hit) or not (miss)
		hit: bool,
		/// Operation duration
		#[serde(serialize_with = "serialize_duration_ms")]
		duration: Duration,
		/// Operation timestamp
		timestamp: DateTime<Utc>,
	},
	/// Cache SET operation
	Set {
		/// Cache key
		key: String,
		/// Operation duration
		#[serde(serialize_with = "serialize_duration_ms")]
		duration: Duration,
		/// Operation timestamp
		timestamp: DateTime<Utc>,
	},
	/// Cache DELETE operation
	Delete {
		/// Cache key
		key: String,
		/// Operation duration
		#[serde(serialize_with = "serialize_duration_ms")]
		duration: Duration,
		/// Operation timestamp
		timestamp: DateTime<Utc>,
	},
}

/// Performance marker
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceMarker {
	/// Marker name (e.g., "AuthMiddleware", "Handler")
	pub name: String,

	/// Start time relative to request start
	#[serde(serialize_with = "serialize_duration_ms")]
	pub start: Duration,

	/// End time relative to request start
	#[serde(serialize_with = "serialize_duration_ms")]
	pub end: Duration,

	/// Marker category (for color-coding)
	pub category: MarkerCategory,
}

/// Performance marker category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarkerCategory {
	/// Middleware
	Middleware,
	/// Handler
	Handler,
	/// Database
	Database,
	/// Cache
	Cache,
	/// Template
	Template,
	/// Other
	Other,
}

/// Serialize Duration as milliseconds
fn serialize_duration_ms<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
	S: serde::Serializer,
{
	serializer.serialize_f64(duration.as_secs_f64() * 1000.0)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_toolbar_context_creation() {
		let request_info = RequestInfo {
			method: "GET".to_string(),
			path: "/test".to_string(),
			query: None,
			headers: vec![],
			client_ip: "127.0.0.1".to_string(),
			timestamp: Utc::now(),
		};
		let ctx = ToolbarContext::new(request_info);

		assert_eq!(ctx.get_sql_queries().len(), 0);
		assert_eq!(ctx.get_templates().len(), 0);
		assert_eq!(ctx.get_cache_ops().len(), 0);
		assert_eq!(ctx.get_performance_markers().len(), 0);
	}

	#[rstest]
	fn test_sql_query_recording() {
		let request_info = RequestInfo {
			method: "GET".to_string(),
			path: "/test".to_string(),
			query: None,
			headers: vec![],
			client_ip: "127.0.0.1".to_string(),
			timestamp: Utc::now(),
		};
		let ctx = ToolbarContext::new(request_info);

		let query = SqlQuery {
			sql: "SELECT * FROM users".to_string(),
			params: vec![],
			duration: Duration::from_millis(10),
			stack_trace: String::new(),
			timestamp: Utc::now(),
			connection: None,
		};

		ctx.record_sql_query(query);
		assert_eq!(ctx.get_sql_queries().len(), 1);
	}

	#[rstest]
	fn test_bounded_buffer() {
		let request_info = RequestInfo {
			method: "GET".to_string(),
			path: "/test".to_string(),
			query: None,
			headers: vec![],
			client_ip: "127.0.0.1".to_string(),
			timestamp: Utc::now(),
		};
		let ctx = ToolbarContext::new(request_info);

		// Record 1001 queries (exceeds MAX_QUERIES = 1000)
		for i in 0..1001 {
			let query = SqlQuery {
				sql: format!("SELECT * FROM users WHERE id = {}", i),
				params: vec![],
				duration: Duration::from_millis(10),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			};
			ctx.record_sql_query(query);
		}

		// Should be limited to 1000
		assert_eq!(ctx.get_sql_queries().len(), 1000);
	}
}
