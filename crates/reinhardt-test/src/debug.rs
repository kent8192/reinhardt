use reinhardt_core::security::escape_html_content;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Debug panel information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugPanel {
	pub title: String,
	pub content: Vec<DebugEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DebugEntry {
	KeyValue {
		key: String,
		value: String,
	},
	Table {
		headers: Vec<String>,
		rows: Vec<Vec<String>>,
	},
	Code {
		language: String,
		code: String,
	},
	Text {
		text: String,
	},
}

/// Request/Response timing information
#[derive(Debug, Clone, Serialize)]
pub struct TimingInfo {
	pub total_time: Duration,
	pub sql_time: Duration,
	pub sql_queries: usize,
	pub cache_hits: usize,
	pub cache_misses: usize,
}

/// SQL query record
#[derive(Debug, Clone, Serialize)]
pub struct SqlQuery {
	pub query: String,
	pub duration: Duration,
	pub stack_trace: Vec<String>,
}

/// Debug toolbar
pub struct DebugToolbar {
	panels: Arc<RwLock<HashMap<String, DebugPanel>>>,
	timing: Arc<RwLock<TimingInfo>>,
	sql_queries: Arc<RwLock<Vec<SqlQuery>>>,
	start_time: Instant,
	enabled: bool,
}

impl DebugToolbar {
	/// Create a new debug toolbar
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	///
	/// let toolbar = DebugToolbar::new();
	/// assert!(toolbar.is_enabled());
	/// ```
	pub fn new() -> Self {
		Self {
			panels: Arc::new(RwLock::new(HashMap::new())),
			timing: Arc::new(RwLock::new(TimingInfo {
				total_time: Duration::from_secs(0),
				sql_time: Duration::from_secs(0),
				sql_queries: 0,
				cache_hits: 0,
				cache_misses: 0,
			})),
			sql_queries: Arc::new(RwLock::new(Vec::new())),
			start_time: Instant::now(),
			enabled: true,
		}
	}
	/// Enable or disable the debug toolbar
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	///
	/// let mut toolbar = DebugToolbar::new();
	/// toolbar.set_enabled(false);
	/// assert!(!toolbar.is_enabled());
	/// ```
	pub fn set_enabled(&mut self, enabled: bool) {
		self.enabled = enabled;
	}
	/// Check if the debug toolbar is enabled
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	///
	/// let toolbar = DebugToolbar::new();
	/// assert!(toolbar.is_enabled());
	/// ```
	pub fn is_enabled(&self) -> bool {
		self.enabled
	}
	/// Add a debug panel
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::{DebugToolbar, DebugPanel, DebugEntry};
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// let panel = DebugPanel {
	///     title: "Test Panel".to_string(),
	///     content: vec![DebugEntry::Text { text: "Hello".to_string() }],
	/// };
	/// toolbar.add_panel("test".to_string(), panel).await;
	/// let panels = toolbar.get_panels().await;
	/// assert!(panels.contains_key("test"));
	/// # });
	/// ```
	pub async fn add_panel(&self, id: String, panel: DebugPanel) {
		if !self.enabled {
			return;
		}
		self.panels.write().await.insert(id, panel);
	}
	/// Record SQL query
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// toolbar.record_sql_query("SELECT * FROM users".to_string(), Duration::from_millis(10)).await;
	/// let timing = toolbar.get_timing().await;
	/// assert_eq!(timing.sql_queries, 1);
	/// assert!(timing.sql_time >= Duration::from_millis(10));
	/// # });
	/// ```
	pub async fn record_sql_query(&self, query: String, duration: Duration) {
		if !self.enabled {
			return;
		}

		let sql_query = SqlQuery {
			query,
			duration,
			stack_trace: vec![],
		};

		self.sql_queries.write().await.push(sql_query);

		let mut timing = self.timing.write().await;
		timing.sql_queries += 1;
		timing.sql_time += duration;
	}
	/// Record cache hit
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// toolbar.record_cache_hit().await;
	/// let timing = toolbar.get_timing().await;
	/// assert_eq!(timing.cache_hits, 1);
	/// # });
	/// ```
	pub async fn record_cache_hit(&self) {
		if !self.enabled {
			return;
		}
		self.timing.write().await.cache_hits += 1;
	}
	/// Record cache miss
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// toolbar.record_cache_miss().await;
	/// let timing = toolbar.get_timing().await;
	/// assert_eq!(timing.cache_misses, 1);
	/// # });
	/// ```
	pub async fn record_cache_miss(&self) {
		if !self.enabled {
			return;
		}
		self.timing.write().await.cache_misses += 1;
	}
	/// Finalize timing information
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// // Simulate some work
	/// tokio::time::sleep(Duration::from_millis(10)).await;
	/// toolbar.finalize().await;
	/// let timing = toolbar.get_timing().await;
	/// assert!(timing.total_time >= Duration::from_millis(10));
	/// # });
	/// ```
	pub async fn finalize(&self) {
		if !self.enabled {
			return;
		}
		self.timing.write().await.total_time = self.start_time.elapsed();
	}
	/// Get all panels
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::{DebugToolbar, DebugPanel, DebugEntry};
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// let panel = DebugPanel {
	///     title: "Test".to_string(),
	///     content: vec![],
	/// };
	/// toolbar.add_panel("test".to_string(), panel).await;
	/// let panels = toolbar.get_panels().await;
	/// assert_eq!(panels.len(), 1);
	/// # });
	/// ```
	pub async fn get_panels(&self) -> HashMap<String, DebugPanel> {
		self.panels.read().await.clone()
	}
	/// Get timing info
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// let timing = toolbar.get_timing().await;
	/// assert_eq!(timing.sql_queries, 0);
	/// assert_eq!(timing.cache_hits, 0);
	/// # });
	/// ```
	pub async fn get_timing(&self) -> TimingInfo {
		self.timing.read().await.clone()
	}
	/// Get SQL queries
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// toolbar.record_sql_query("SELECT 1".to_string(), Duration::from_millis(5)).await;
	/// let queries = toolbar.get_sql_queries().await;
	/// assert_eq!(queries.len(), 1);
	/// assert_eq!(queries[0].query, "SELECT 1");
	/// # });
	/// ```
	pub async fn get_sql_queries(&self) -> Vec<SqlQuery> {
		self.sql_queries.read().await.clone()
	}
	/// Render as HTML
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_test::debug::DebugToolbar;
	///
	/// # tokio_test::block_on(async {
	/// let toolbar = DebugToolbar::new();
	/// toolbar.finalize().await;
	/// let html = toolbar.render_html().await;
	/// assert!(html.contains("debug-toolbar"));
	/// assert!(html.contains("Timing"));
	/// # });
	/// ```
	pub async fn render_html(&self) -> String {
		if !self.enabled {
			return String::new();
		}

		let panels = self.get_panels().await;
		let timing = self.get_timing().await;
		let queries = self.get_sql_queries().await;

		format!(
			r#"
<div class="debug-toolbar">
    <div class="timing">
        <h3>Timing</h3>
        <p>Total: {:?}</p>
        <p>SQL: {:?} ({} queries)</p>
        <p>Cache: {} hits, {} misses</p>
    </div>
    <div class="sql-queries">
        <h3>SQL Queries</h3>
        <ul>
            {}
        </ul>
    </div>
    <div class="panels">
        {}
    </div>
</div>
"#,
			timing.total_time,
			timing.sql_time,
			timing.sql_queries,
			timing.cache_hits,
			timing.cache_misses,
			queries
				.iter()
				.map(|q| format!(
					"<li>{} ({:?})</li>",
					escape_html_content(&q.query),
					q.duration
				))
				.collect::<Vec<_>>()
				.join("\n"),
			panels
				.iter()
				.map(|(id, panel)| format!(
					"<div class='panel' id='{}'><h3>{}</h3></div>",
					escape_html_content(id),
					escape_html_content(&panel.title)
				))
				.collect::<Vec<_>>()
				.join("\n")
		)
	}
}

impl Default for DebugToolbar {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_debug_toolbar() {
		let toolbar = DebugToolbar::new();

		toolbar
			.record_sql_query("SELECT * FROM users".to_string(), Duration::from_millis(10))
			.await;
		toolbar.record_cache_hit().await;
		toolbar.finalize().await;

		let timing = toolbar.get_timing().await;
		assert_eq!(timing.sql_queries, 1);
		assert_eq!(timing.cache_hits, 1);

		let queries = toolbar.get_sql_queries().await;
		assert_eq!(queries.len(), 1);
	}

	#[tokio::test]
	async fn test_debug_panel() {
		let toolbar = DebugToolbar::new();

		let panel = DebugPanel {
			title: "Test Panel".to_string(),
			content: vec![DebugEntry::KeyValue {
				key: "key".to_string(),
				value: "value".to_string(),
			}],
		};

		toolbar.add_panel("test".to_string(), panel).await;

		let panels = toolbar.get_panels().await;
		assert_eq!(panels.len(), 1);
		assert!(panels.contains_key("test"));
	}

	#[test]
	fn test_escape_html_content() {
		assert_eq!(escape_html_content("hello"), "hello");
		assert_eq!(
			escape_html_content("<script>alert('xss')</script>"),
			"&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
		);
		assert_eq!(escape_html_content("a & b"), "a &amp; b");
		assert_eq!(
			escape_html_content(r#"key="value""#),
			"key=&quot;value&quot;"
		);
	}

	#[tokio::test]
	async fn test_render_html_escapes_sql_queries() {
		let toolbar = DebugToolbar::new();
		toolbar
			.record_sql_query(
				"SELECT * FROM users WHERE name = '<script>alert(1)</script>'".to_string(),
				Duration::from_millis(1),
			)
			.await;
		toolbar.finalize().await;

		let html = toolbar.render_html().await;
		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
	}

	#[tokio::test]
	async fn test_render_html_escapes_panel_content() {
		let toolbar = DebugToolbar::new();
		let panel = DebugPanel {
			title: "<img src=x onerror=alert(1)>".to_string(),
			content: vec![],
		};
		toolbar.add_panel("<script>".to_string(), panel).await;
		toolbar.finalize().await;

		let html = toolbar.render_html().await;
		assert!(!html.contains("<script>"));
		assert!(!html.contains("<img src=x"));
		assert!(html.contains("&lt;script&gt;"));
		assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
	}

	#[tokio::test]
	async fn test_disabled_toolbar() {
		let mut toolbar = DebugToolbar::new();
		toolbar.set_enabled(false);

		toolbar
			.record_sql_query("SELECT 1".to_string(), Duration::from_millis(1))
			.await;

		let timing = toolbar.get_timing().await;
		assert_eq!(timing.sql_queries, 0);
	}
}
