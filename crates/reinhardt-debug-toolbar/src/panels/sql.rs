//! SQL query debugging panel

use crate::context::ToolbarContext;
use crate::error::ToolbarResult;
use crate::panels::{Panel, PanelStats};
use crate::utils::sql_normalization::{detect_n_plus_one, normalize_sql};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

/// SQL query debugging panel
pub struct SqlPanel {
	/// SQL warning threshold in milliseconds
	warning_threshold_ms: u64,
}

impl SqlPanel {
	/// Create new SQL panel with default threshold (100ms)
	pub fn new() -> Self {
		Self {
			warning_threshold_ms: 100,
		}
	}

	/// Create SQL panel with custom warning threshold
	pub fn with_threshold(warning_threshold_ms: u64) -> Self {
		Self {
			warning_threshold_ms,
		}
	}
}

impl Default for SqlPanel {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Panel for SqlPanel {
	fn id(&self) -> &'static str {
		"sql"
	}

	fn name(&self) -> &'static str {
		"SQL"
	}

	fn priority(&self) -> i32 {
		90 // High priority panel
	}

	async fn generate_stats(&self, ctx: &ToolbarContext) -> ToolbarResult<PanelStats> {
		let queries = ctx.sql_queries.lock().unwrap();

		// Calculate statistics
		let total_queries = queries.len();
		let total_time: Duration = queries.iter().map(|q| q.duration).sum();

		// Detect duplicates
		let mut normalized_counts: HashMap<String, usize> = HashMap::new();
		for query in queries.iter() {
			let normalized = normalize_sql(&query.sql);
			*normalized_counts.entry(normalized).or_insert(0) += 1;
		}
		let duplicate_count = normalized_counts
			.values()
			.filter(|&&count| count > 1)
			.count();

		// Detect slow queries
		let slow_queries: Vec<_> = queries
			.iter()
			.filter(|q| q.duration.as_millis() as u64 >= self.warning_threshold_ms)
			.collect();

		// Detect N+1 patterns
		let n_plus_one_patterns = detect_n_plus_one(&queries);

		// Build data payload
		let queries_data: Vec<serde_json::Value> = queries
			.iter()
			.enumerate()
			.map(|(idx, q)| {
				let normalized = normalize_sql(&q.sql);
				let is_duplicate = normalized_counts[&normalized] > 1;
				let is_slow = q.duration.as_millis() as u64 >= self.warning_threshold_ms;
				let is_n_plus_one = n_plus_one_patterns.contains(&normalized);

				serde_json::json!({
					"index": idx,
					"sql": q.sql,
					"duration_ms": q.duration.as_millis(),
					"normalized": normalized,
					"is_duplicate": is_duplicate,
					"is_slow": is_slow,
					"is_n_plus_one": is_n_plus_one,
					"stack_trace": q.stack_trace,
				})
			})
			.collect();

		let data = serde_json::json!({
			"total_queries": total_queries,
			"total_time_ms": total_time.as_millis(),
			"duplicate_count": duplicate_count,
			"slow_queries_count": slow_queries.len(),
			"n_plus_one_count": n_plus_one_patterns.len(),
			"warning_threshold_ms": self.warning_threshold_ms,
			"queries": queries_data,
		});

		let summary = format!("{} queries in {}ms", total_queries, total_time.as_millis());

		Ok(PanelStats {
			panel_id: self.id().to_string(),
			panel_name: self.name().to_string(),
			data,
			summary,
			rendered_html: None,
		})
	}

	fn render(&self, stats: &PanelStats) -> ToolbarResult<String> {
		let data = &stats.data;

		let total_queries = data["total_queries"].as_u64().unwrap_or(0);
		let total_time_ms = data["total_time_ms"].as_u64().unwrap_or(0);
		let duplicate_count = data["duplicate_count"].as_u64().unwrap_or(0);
		let slow_queries_count = data["slow_queries_count"].as_u64().unwrap_or(0);
		let n_plus_one_count = data["n_plus_one_count"].as_u64().unwrap_or(0);
		let warning_threshold_ms = data["warning_threshold_ms"].as_u64().unwrap_or(100);

		let empty_queries = vec![];
		let queries = data["queries"].as_array().unwrap_or(&empty_queries);

		// Generate warnings section
		let mut warnings = Vec::new();
		if duplicate_count > 0 {
			warnings.push(format!(
				"<div class='djdt-warning'>⚠️ {} duplicate queries detected</div>",
				duplicate_count
			));
		}
		if slow_queries_count > 0 {
			warnings.push(format!(
				"<div class='djdt-warning'>⚠️ {} slow queries (>{}ms)</div>",
				slow_queries_count, warning_threshold_ms
			));
		}
		if n_plus_one_count > 0 {
			warnings.push(format!(
				"<div class='djdt-warning'>⚠️ {} potential N+1 query patterns detected</div>",
				n_plus_one_count
			));
		}
		let warnings_html = warnings.join("");

		// Generate queries table
		let queries_html: String = queries
			.iter()
			.map(|q| {
				let index = q["index"].as_u64().unwrap_or(0);
				let sql = q["sql"].as_str().unwrap_or("");
				let duration_ms = q["duration_ms"].as_u64().unwrap_or(0);
				let is_duplicate = q["is_duplicate"].as_bool().unwrap_or(false);
				let is_slow = q["is_slow"].as_bool().unwrap_or(false);
				let is_n_plus_one = q["is_n_plus_one"].as_bool().unwrap_or(false);

				let mut badges = Vec::new();
				if is_duplicate {
					badges.push("<span class='djdt-badge djdt-badge-warning'>DUPLICATE</span>");
				}
				if is_slow {
					badges.push("<span class='djdt-badge djdt-badge-danger'>SLOW</span>");
				}
				if is_n_plus_one {
					badges.push("<span class='djdt-badge djdt-badge-danger'>N+1</span>");
				}
				let badges_html = badges.join(" ");

				format!(
					r#"
					<tr>
						<td>#{}</td>
						<td><code>{}</code> {}</td>
						<td>{}ms</td>
					</tr>
					"#,
					index + 1,
					html_escape(sql),
					badges_html,
					duration_ms
				)
			})
			.collect::<Vec<_>>()
			.join("");

		Ok(format!(
			r#"
			<div class="djdt-panel-content">
				<h3>SQL Queries</h3>
				<div class="djdt-summary">
					<p><strong>Total Queries:</strong> {}</p>
					<p><strong>Total Time:</strong> {}ms</p>
					<p><strong>Average Time:</strong> {}ms</p>
				</div>
				{}
				<table class="djdt-table">
					<thead>
						<tr>
							<th>#</th>
							<th>Query</th>
							<th>Time</th>
						</tr>
					</thead>
					<tbody>{}</tbody>
				</table>
			</div>
			"#,
			total_queries,
			total_time_ms,
			if total_queries > 0 {
				total_time_ms / total_queries
			} else {
				0
			},
			warnings_html,
			queries_html
		))
	}
}

/// Simple HTML escape
fn html_escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::context::{RequestInfo, SqlQuery};
	use chrono::Utc;
	use rstest::*;

	#[rstest]
	#[tokio::test]
	async fn test_sql_panel_generate_stats() {
		let request_info = RequestInfo {
			method: "GET".to_string(),
			path: "/test".to_string(),
			query: None,
			headers: vec![],
			client_ip: "127.0.0.1".to_string(),
			timestamp: Utc::now(),
		};
		let ctx = ToolbarContext::new(request_info);

		// Add some test queries
		{
			let mut queries = ctx.sql_queries.lock().unwrap();
			queries.push(SqlQuery {
				sql: "SELECT * FROM users WHERE id = 1".to_string(),
				params: vec![],
				duration: Duration::from_millis(50),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			});
			queries.push(SqlQuery {
				sql: "SELECT * FROM users WHERE id = 2".to_string(),
				params: vec![],
				duration: Duration::from_millis(150), // Slow query
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			});
			queries.push(SqlQuery {
				sql: "SELECT * FROM users WHERE id = 3".to_string(),
				params: vec![],
				duration: Duration::from_millis(30),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			});
		}

		let panel = SqlPanel::new();
		let stats = panel.generate_stats(&ctx).await.unwrap();

		assert_eq!(stats.panel_id, "sql");
		assert_eq!(stats.panel_name, "SQL");
		assert_eq!(stats.data["total_queries"].as_u64().unwrap(), 3);
		assert_eq!(stats.data["duplicate_count"].as_u64().unwrap(), 1); // All normalize to same query pattern
		assert_eq!(stats.data["slow_queries_count"].as_u64().unwrap(), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_sql_panel_n_plus_one_detection() {
		let request_info = RequestInfo {
			method: "GET".to_string(),
			path: "/test".to_string(),
			query: None,
			headers: vec![],
			client_ip: "127.0.0.1".to_string(),
			timestamp: Utc::now(),
		};
		let ctx = ToolbarContext::new(request_info);

		// Add N+1 pattern queries
		{
			let mut queries = ctx.sql_queries.lock().unwrap();
			queries.push(SqlQuery {
				sql: "SELECT * FROM users".to_string(),
				params: vec![],
				duration: Duration::from_millis(50),
				stack_trace: String::new(),
				timestamp: Utc::now(),
				connection: None,
			});
			for i in 1..=5 {
				queries.push(SqlQuery {
					sql: format!("SELECT * FROM posts WHERE user_id = {}", i),
					params: vec![],
					duration: Duration::from_millis(10),
					stack_trace: String::new(),
					timestamp: Utc::now(),
					connection: None,
				});
			}
		}

		let panel = SqlPanel::new();
		let stats = panel.generate_stats(&ctx).await.unwrap();

		assert_eq!(stats.data["total_queries"].as_u64().unwrap(), 6);
		assert_eq!(stats.data["n_plus_one_count"].as_u64().unwrap(), 1);
	}

	#[rstest]
	fn test_html_escape() {
		assert_eq!(
			html_escape("<script>alert('xss')</script>"),
			"&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
		);
		assert_eq!(html_escape("foo & bar"), "foo &amp; bar");
	}
}
