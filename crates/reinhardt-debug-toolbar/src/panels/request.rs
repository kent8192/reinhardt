//! Request/Response information panel

use crate::context::ToolbarContext;
use crate::error::ToolbarResult;
use crate::panels::{Panel, PanelStats};
use crate::utils::sanitization::sanitize_headers;
use async_trait::async_trait;

/// Request/Response information panel
pub struct RequestPanel;

impl RequestPanel {
	/// Create new request panel
	pub fn new() -> Self {
		Self
	}
}

impl Default for RequestPanel {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Panel for RequestPanel {
	fn id(&self) -> &'static str {
		"request"
	}

	fn name(&self) -> &'static str {
		"Request"
	}

	fn priority(&self) -> i32 {
		100 // Critical panel
	}

	async fn generate_stats(&self, ctx: &ToolbarContext) -> ToolbarResult<PanelStats> {
		let request_info = &ctx.request_info;

		// Sanitize headers to remove sensitive data
		let sanitized_headers: Vec<(String, String)> = request_info
			.headers
			.iter()
			.map(|(k, v)| sanitize_headers(k, v))
			.collect();

		let data = serde_json::json!({
			"method": request_info.method,
			"path": request_info.path,
			"query": request_info.query,
			"headers": sanitized_headers,
			"client_ip": request_info.client_ip,
			"timestamp": request_info.timestamp,
		});

		let summary = format!("{} {}", request_info.method, request_info.path);

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

		let method = data["method"].as_str().unwrap_or("");
		let path = data["path"].as_str().unwrap_or("");
		let query = data["query"].as_str();
		let empty_headers = vec![];
		let headers = data["headers"].as_array().unwrap_or(&empty_headers);
		let client_ip = data["client_ip"].as_str().unwrap_or("");
		let timestamp = data["timestamp"].as_str().unwrap_or("");

		let query_html = if let Some(q) = query {
			format!(
				r#"<tr><th>Query String</th><td>{}</td></tr>"#,
				html_escape(q)
			)
		} else {
			String::new()
		};

		let headers_html: String = headers
			.iter()
			.map(|h| {
				let name = h[0].as_str().unwrap_or("");
				let value = h[1].as_str().unwrap_or("");
				format!(
					r#"<tr><td>{}</td><td>{}</td></tr>"#,
					html_escape(name),
					html_escape(value)
				)
			})
			.collect::<Vec<_>>()
			.join("");

		Ok(format!(
			r#"
			<div class="djdt-panel-content">
				<h3>Request Information</h3>
				<table class="djdt-table">
					<tr><th>Method</th><td>{}</td></tr>
					<tr><th>Path</th><td>{}</td></tr>
					{}
					<tr><th>Client IP</th><td>{}</td></tr>
					<tr><th>Timestamp</th><td>{}</td></tr>
				</table>
				<h3>Request Headers</h3>
				<table class="djdt-table">
					<thead><tr><th>Name</th><th>Value</th></tr></thead>
					<tbody>{}</tbody>
				</table>
			</div>
			"#,
			html_escape(method),
			html_escape(path),
			query_html,
			html_escape(client_ip),
			html_escape(timestamp),
			headers_html
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
	use crate::context::RequestInfo;
	use chrono::Utc;
	use rstest::*;

	#[rstest]
	#[tokio::test]
	async fn test_request_panel_generate_stats() {
		let request_info = RequestInfo {
			method: "GET".to_string(),
			path: "/test".to_string(),
			query: Some("foo=bar".to_string()),
			headers: vec![
				("Content-Type".to_string(), "application/json".to_string()),
				(
					"Authorization".to_string(),
					"Bearer secret-token".to_string(),
				),
			],
			client_ip: "127.0.0.1".to_string(),
			timestamp: Utc::now(),
		};
		let ctx = ToolbarContext::new(request_info);

		let panel = RequestPanel::new();
		let stats = panel.generate_stats(&ctx).await.unwrap();

		assert_eq!(stats.panel_id, "request");
		assert_eq!(stats.panel_name, "Request");
		assert_eq!(stats.summary, "GET /test");

		// Verify sensitive header is sanitized
		let headers = stats.data["headers"].as_array().unwrap();
		let auth_header = headers
			.iter()
			.find(|h| h[0].as_str().unwrap() == "Authorization")
			.unwrap();
		assert_eq!(auth_header[1].as_str().unwrap(), "***REDACTED***");
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
