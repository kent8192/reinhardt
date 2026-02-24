//! Interactive API documentation templates

use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes};
use reinhardt_core::security::xss::{escape_html, escape_javascript};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// API endpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
	/// HTTP method (GET, POST, etc.)
	pub method: String,
	/// Endpoint path
	pub path: String,
	/// Description of the endpoint
	pub description: String,
	/// Request parameters
	pub parameters: Vec<Parameter>,
	/// Response schema
	pub response_schema: Option<String>,
	/// Example request
	pub example_request: Option<String>,
	/// Example response
	pub example_response: Option<String>,
}

/// API parameter information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
	/// Parameter name
	pub name: String,
	/// Parameter type
	pub param_type: String,
	/// Whether the parameter is required
	pub required: bool,
	/// Parameter description
	pub description: Option<String>,
}

/// Interactive API documentation renderer
///
/// # Examples
///
/// ```
/// use reinhardt_views::browsable_api::templates::{InteractiveDocsRenderer, ApiEndpoint};
///
/// let mut renderer = InteractiveDocsRenderer::new("My API Documentation");
/// let endpoint = ApiEndpoint {
///     method: "GET".to_string(),
///     path: "/api/users/".to_string(),
///     description: "List all users".to_string(),
///     parameters: vec![],
///     response_schema: Some("User[]".to_string()),
///     example_request: None,
///     example_response: Some(r#"[{"id": 1, "name": "Alice"}]"#.to_string()),
/// };
/// renderer.add_endpoint(endpoint);
/// let html = renderer.render().unwrap();
/// assert!(html.contains("My API Documentation"));
/// ```
#[derive(Debug, Clone)]
pub struct InteractiveDocsRenderer {
	title: String,
	description: Option<String>,
	endpoints: Vec<ApiEndpoint>,
	base_url: String,
}

impl InteractiveDocsRenderer {
	/// Create a new interactive docs renderer
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::InteractiveDocsRenderer;
	///
	/// let renderer = InteractiveDocsRenderer::new("API Docs");
	/// ```
	pub fn new(title: impl Into<String>) -> Self {
		Self {
			title: title.into(),
			description: None,
			endpoints: Vec::new(),
			base_url: String::new(),
		}
	}

	/// Set API description
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::InteractiveDocsRenderer;
	///
	/// let mut renderer = InteractiveDocsRenderer::new("API");
	/// renderer.set_description("RESTful API for managing users");
	/// ```
	pub fn set_description(&mut self, description: impl Into<String>) -> &mut Self {
		self.description = Some(description.into());
		self
	}

	/// Set base URL for the API
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::InteractiveDocsRenderer;
	///
	/// let mut renderer = InteractiveDocsRenderer::new("API");
	/// renderer.set_base_url("https://api.example.com");
	/// ```
	pub fn set_base_url(&mut self, base_url: impl Into<String>) -> &mut Self {
		self.base_url = base_url.into();
		self
	}

	/// Add an endpoint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::templates::{InteractiveDocsRenderer, ApiEndpoint};
	///
	/// let mut renderer = InteractiveDocsRenderer::new("API");
	/// let endpoint = ApiEndpoint {
	///     method: "POST".to_string(),
	///     path: "/api/items/".to_string(),
	///     description: "Create an item".to_string(),
	///     parameters: vec![],
	///     response_schema: None,
	///     example_request: None,
	///     example_response: None,
	/// };
	/// renderer.add_endpoint(endpoint);
	/// ```
	pub fn add_endpoint(&mut self, endpoint: ApiEndpoint) -> &mut Self {
		self.endpoints.push(endpoint);
		self
	}

	/// Group endpoints by path prefix
	fn group_endpoints(&self) -> HashMap<String, Vec<&ApiEndpoint>> {
		let mut groups: HashMap<String, Vec<&ApiEndpoint>> = HashMap::new();

		for endpoint in &self.endpoints {
			let group = endpoint
				.path
				.split('/')
				.nth(2)
				.unwrap_or("default")
				.to_string();
			groups.entry(group).or_default().push(endpoint);
		}

		groups
	}

	/// Render the interactive documentation as HTML
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::templates::{InteractiveDocsRenderer, ApiEndpoint};
	///
	/// let mut renderer = InteractiveDocsRenderer::new("API Docs");
	/// let endpoint = ApiEndpoint {
	///     method: "GET".to_string(),
	///     path: "/api/test/".to_string(),
	///     description: "Test endpoint".to_string(),
	///     parameters: vec![],
	///     response_schema: None,
	///     example_request: None,
	///     example_response: None,
	/// };
	/// renderer.add_endpoint(endpoint);
	/// let html = renderer.render().unwrap();
	/// assert!(html.contains("API Docs"));
	/// ```
	pub fn render(&self) -> Result<String, String> {
		let mut html = self.generate_header();

		// API description - escape for HTML content
		if let Some(desc) = &self.description {
			html.push_str(&format!(
				r#"      <div class="description">{}</div>"#,
				escape_html(desc)
			));
			html.push('\n');
		}

		// Group endpoints
		let groups = self.group_endpoints();

		for (group_name, endpoints) in groups {
			html.push_str(&format!(
				r#"      <div class="endpoint-group">
        <h2>{}</h2>
"#,
				escape_html(&group_name)
			));

			for endpoint in endpoints {
				html.push_str(&self.render_endpoint(endpoint));
			}

			html.push_str("      </div>\n");
		}

		html.push_str(&self.generate_footer());

		Ok(html)
	}

	/// Render a single endpoint
	fn render_endpoint(&self, endpoint: &ApiEndpoint) -> String {
		let method_class = endpoint.method.to_lowercase();
		// Escape all user-controlled values for HTML content
		let mut html = format!(
			r#"        <div class="endpoint">
          <div class="endpoint-header">
            <span class="method method-{}">{}</span>
            <span class="path">{}</span>
          </div>
          <div class="endpoint-body">
            <p class="description">{}</p>
"#,
			escape_html(&method_class),
			escape_html(&endpoint.method),
			escape_html(&endpoint.path),
			escape_html(&endpoint.description)
		);

		// Parameters
		if !endpoint.parameters.is_empty() {
			html.push_str("            <h4>Parameters:</h4>\n");
			html.push_str("            <table class=\"params-table\">\n");
			html.push_str("              <thead><tr><th>Name</th><th>Type</th><th>Required</th><th>Description</th></tr></thead>\n");
			html.push_str("              <tbody>\n");

			for param in &endpoint.parameters {
				html.push_str(&format!(
					"                <tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
					escape_html(&param.name),
					escape_html(&param.param_type),
					if param.required { "Yes" } else { "No" },
					escape_html(param.description.as_deref().unwrap_or("-"))
				));
			}

			html.push_str("              </tbody>\n");
			html.push_str("            </table>\n");
		}

		// Example request - escape for HTML content
		if let Some(example_req) = &endpoint.example_request {
			html.push_str(&format!(
				r#"            <h4>Example Request:</h4>
            <pre class="example">{}</pre>
"#,
				escape_html(example_req)
			));
		}

		// Example response - escape for HTML content
		if let Some(example_resp) = &endpoint.example_response {
			html.push_str(&format!(
				r#"            <h4>Example Response:</h4>
            <pre class="example">{}</pre>
"#,
				escape_html(example_resp)
			));
		}

		// Try it out button - CRITICAL: use escape_javascript for onclick handler
		html.push_str(&format!(
			r#"            <button class="try-it-btn" onclick="tryEndpoint('{}', '{}')">Try it out</button>
"#,
			escape_javascript(&endpoint.method),
			escape_javascript(&endpoint.path)
		));

		html.push_str("          </div>\n");
		html.push_str("        </div>\n");

		html
	}

	/// Generate HTML header
	fn generate_header(&self) -> String {
		let escaped_title = escape_html(&self.title);
		format!(
			r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{}</title>
    <style>
      body {{
        font-family: "Segoe UI", Tahoma, Geneva, Verdana, sans-serif;
        margin: 0;
        padding: 20px;
        background-color: #f5f5f5;
      }}
      .container {{
        max-width: 1400px;
        margin: 0 auto;
        background-color: white;
        padding: 30px;
        border-radius: 8px;
        box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
      }}
      h1 {{
        color: #333;
        border-bottom: 3px solid #007bff;
        padding-bottom: 15px;
        margin-bottom: 20px;
      }}
      h2 {{
        color: #555;
        margin-top: 30px;
        border-bottom: 1px solid #ddd;
        padding-bottom: 10px;
      }}
      .description {{
        color: #666;
        margin-bottom: 30px;
        line-height: 1.6;
      }}
      .endpoint-group {{
        margin-bottom: 40px;
      }}
      .endpoint {{
        border: 1px solid #dee2e6;
        border-radius: 6px;
        margin-bottom: 20px;
        overflow: hidden;
      }}
      .endpoint-header {{
        background-color: #f8f9fa;
        padding: 15px 20px;
        display: flex;
        align-items: center;
        gap: 15px;
      }}
      .method {{
        padding: 5px 12px;
        border-radius: 4px;
        font-weight: bold;
        font-size: 13px;
        text-transform: uppercase;
      }}
      .method-get {{ background-color: #61affe; color: white; }}
      .method-post {{ background-color: #49cc90; color: white; }}
      .method-put {{ background-color: #fca130; color: white; }}
      .method-patch {{ background-color: #50e3c2; color: white; }}
      .method-delete {{ background-color: #f93e3e; color: white; }}
      .path {{
        font-family: 'Courier New', monospace;
        font-size: 16px;
        color: #333;
      }}
      .endpoint-body {{
        padding: 20px;
      }}
      .endpoint-body .description {{
        color: #555;
        margin-bottom: 15px;
      }}
      .params-table {{
        width: 100%;
        border-collapse: collapse;
        margin: 15px 0;
      }}
      .params-table th {{
        background-color: #f8f9fa;
        padding: 10px;
        text-align: left;
        border-bottom: 2px solid #dee2e6;
      }}
      .params-table td {{
        padding: 10px;
        border-bottom: 1px solid #dee2e6;
      }}
      .example {{
        background-color: #282c34;
        color: #abb2bf;
        padding: 15px;
        border-radius: 4px;
        overflow-x: auto;
        font-family: 'Courier New', monospace;
        line-height: 1.5;
        margin: 10px 0;
      }}
      .try-it-btn {{
        background-color: #007bff;
        color: white;
        border: none;
        padding: 10px 20px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 14px;
        margin-top: 15px;
      }}
      .try-it-btn:hover {{
        background-color: #0056b3;
      }}
      h4 {{
        color: #333;
        margin-top: 20px;
        margin-bottom: 10px;
      }}
    </style>
    <script>
      function tryEndpoint(method, path) {{
        alert('Try it out: ' + method + ' ' + path + '\n\nThis feature is coming soon!');
      }}
    </script>
  </head>
  <body>
    <div class="container">
      <h1>{}</h1>
"#,
			escaped_title, escaped_title
		)
	}

	/// Generate HTML footer
	fn generate_footer(&self) -> String {
		r#"    </div>
  </body>
</html>
"#
		.to_string()
	}

	/// Create an HTTP response with rendered HTML
	pub fn create_response(&self) -> Result<Response<Full<Bytes>>, String> {
		let html = self.render()?;

		Response::builder()
			.status(StatusCode::OK)
			.header("Content-Type", "text/html; charset=utf-8")
			.body(Full::new(Bytes::from(html)))
			.map_err(|e| e.to_string())
	}
}

impl Default for InteractiveDocsRenderer {
	fn default() -> Self {
		Self::new("API Documentation")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_renderer_creation() {
		let renderer = InteractiveDocsRenderer::new("Test API");
		assert_eq!(renderer.title, "Test API");
		assert!(renderer.endpoints.is_empty());
	}

	#[test]
	fn test_set_description() {
		let mut renderer = InteractiveDocsRenderer::new("API");
		renderer.set_description("Test description");
		assert_eq!(renderer.description, Some("Test description".to_string()));
	}

	#[test]
	fn test_set_base_url() {
		let mut renderer = InteractiveDocsRenderer::new("API");
		renderer.set_base_url("https://example.com");
		assert_eq!(renderer.base_url, "https://example.com");
	}

	#[test]
	fn test_add_endpoint() {
		let mut renderer = InteractiveDocsRenderer::new("API");
		let endpoint = ApiEndpoint {
			method: "GET".to_string(),
			path: "/api/test/".to_string(),
			description: "Test".to_string(),
			parameters: vec![],
			response_schema: None,
			example_request: None,
			example_response: None,
		};
		renderer.add_endpoint(endpoint);
		assert_eq!(renderer.endpoints.len(), 1);
	}

	#[test]
	fn test_render_basic() {
		let mut renderer = InteractiveDocsRenderer::new("API Docs");
		let endpoint = ApiEndpoint {
			method: "GET".to_string(),
			path: "/api/users/".to_string(),
			description: "List users".to_string(),
			parameters: vec![],
			response_schema: None,
			example_request: None,
			example_response: None,
		};
		renderer.add_endpoint(endpoint);
		let result = renderer.render();
		let html = result.unwrap();
		assert!(html.contains("API Docs"));
		assert!(html.contains("/api/users/"));
	}

	#[test]
	fn test_render_with_parameters() {
		let mut renderer = InteractiveDocsRenderer::new("API");
		let endpoint = ApiEndpoint {
			method: "POST".to_string(),
			path: "/api/items/".to_string(),
			description: "Create item".to_string(),
			parameters: vec![Parameter {
				name: "name".to_string(),
				param_type: "string".to_string(),
				required: true,
				description: Some("Item name".to_string()),
			}],
			response_schema: None,
			example_request: None,
			example_response: None,
		};
		renderer.add_endpoint(endpoint);
		let html = renderer.render().unwrap();
		assert!(html.contains("Parameters"));
		assert!(html.contains("name"));
	}

	#[test]
	fn test_default_renderer() {
		let renderer = InteractiveDocsRenderer::default();
		assert_eq!(renderer.title, "API Documentation");
	}
}
