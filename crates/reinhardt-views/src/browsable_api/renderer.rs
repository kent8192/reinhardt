//! Browsable API renderer

use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes};
use reinhardt_core::security::xss::escape_html;
use serde_json::Value;

use super::{ColorScheme, FormGenerator, SyntaxHighlighter};

/// HTML renderer for browsable API responses
///
/// # Examples
///
/// ```
/// use reinhardt_views::browsable_api::{BrowsableApiRenderer, ColorScheme};
/// use serde_json::json;
///
/// let renderer = BrowsableApiRenderer::new("Users API", ColorScheme::Dark);
/// let data = json!({"users": [{"id": 1, "name": "Alice"}]});
/// let response = renderer.render_json(&data, 200).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct BrowsableApiRenderer {
	title: String,
	highlighter: SyntaxHighlighter,
}

impl BrowsableApiRenderer {
	/// Create a new browsable API renderer
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::{BrowsableApiRenderer, ColorScheme};
	///
	/// let renderer = BrowsableApiRenderer::new("My API", ColorScheme::Dark);
	/// ```
	pub fn new(title: impl Into<String>, color_scheme: ColorScheme) -> Self {
		Self {
			title: title.into(),
			highlighter: SyntaxHighlighter::new(color_scheme),
		}
	}

	/// Render JSON data as HTML
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::{BrowsableApiRenderer, ColorScheme};
	/// use serde_json::json;
	///
	/// let renderer = BrowsableApiRenderer::new("API", ColorScheme::Light);
	/// let data = json!({"message": "Hello"});
	/// let response = renderer.render_json(&data, 200).unwrap();
	/// ```
	pub fn render_json(&self, data: &Value, status_code: u16) -> Result<String, String> {
		let json_str = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
		let highlighted = self.highlighter.highlight_and_wrap_json(&json_str)?;

		let (status_class, status_text) = self.get_status_info(status_code);

		Ok(self.generate_html(&highlighted, status_code, &status_class, &status_text))
	}

	/// Render JSON with form for POST/PUT/PATCH methods
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::{BrowsableApiRenderer, ColorScheme, FormGenerator};
	/// use serde_json::json;
	///
	/// let renderer = BrowsableApiRenderer::new("API", ColorScheme::Dark);
	/// let data = json!({"id": 1});
	/// let mut form = FormGenerator::new("/api/items/", "POST");
	/// form.add_field("name", "text", true);
	/// let response = renderer.render_with_form(&data, 200, &form).unwrap();
	/// ```
	pub fn render_with_form(
		&self,
		data: &Value,
		status_code: u16,
		form: &FormGenerator,
	) -> Result<String, String> {
		let json_str = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
		let highlighted = self.highlighter.highlight_and_wrap_json(&json_str)?;

		let form_html = form.generate()?;
		let (status_class, status_text) = self.get_status_info(status_code);

		let mut html = self.generate_html(&highlighted, status_code, &status_class, &status_text);

		// Insert form before the closing container div
		let insert_pos = html.rfind("</div>\n  </body>").unwrap_or(html.len());
		html.insert_str(
			insert_pos,
			&format!(
				r#"
      <div class="form-section">
        <h2>Submit Data</h2>
        {}
      </div>
"#,
				form_html
			),
		);

		Ok(html)
	}

	/// Create an HTTP response with rendered HTML
	pub fn create_response(
		&self,
		data: &Value,
		status_code: u16,
	) -> Result<Response<Full<Bytes>>, String> {
		let html = self.render_json(data, status_code)?;
		let status = StatusCode::from_u16(status_code).map_err(|e| e.to_string())?;

		Response::builder()
			.status(status)
			.header("Content-Type", "text/html; charset=utf-8")
			.body(Full::new(Bytes::from(html)))
			.map_err(|e| e.to_string())
	}

	/// Create an HTTP response with form
	pub fn create_response_with_form(
		&self,
		data: &Value,
		status_code: u16,
		form: &FormGenerator,
	) -> Result<Response<Full<Bytes>>, String> {
		let html = self.render_with_form(data, status_code, form)?;
		let status = StatusCode::from_u16(status_code).map_err(|e| e.to_string())?;

		Response::builder()
			.status(status)
			.header("Content-Type", "text/html; charset=utf-8")
			.body(Full::new(Bytes::from(html)))
			.map_err(|e| e.to_string())
	}

	/// Set color scheme
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::{BrowsableApiRenderer, ColorScheme};
	///
	/// let mut renderer = BrowsableApiRenderer::new("API", ColorScheme::Dark);
	/// renderer.set_color_scheme(ColorScheme::Light);
	/// ```
	pub fn set_color_scheme(&mut self, scheme: ColorScheme) {
		self.highlighter.set_color_scheme(scheme);
	}

	fn get_status_info(&self, status_code: u16) -> (String, String) {
		let status_class = if (200..300).contains(&status_code) {
			"status-success"
		} else if (400..600).contains(&status_code) {
			"status-error"
		} else {
			"status-info"
		}
		.to_string();

		let status_text = format!("{} {}", status_code, self.get_status_message(status_code));

		(status_class, status_text)
	}

	fn get_status_message(&self, code: u16) -> &'static str {
		match code {
			200 => "OK",
			201 => "Created",
			204 => "No Content",
			400 => "Bad Request",
			401 => "Unauthorized",
			403 => "Forbidden",
			404 => "Not Found",
			500 => "Internal Server Error",
			_ => "Unknown",
		}
	}

	fn generate_html(
		&self,
		content: &str,
		_status_code: u16,
		status_class: &str,
		status_text: &str,
	) -> String {
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
        max-width: 1200px;
        margin: 0 auto;
        background-color: white;
        padding: 30px;
        border-radius: 8px;
        box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
      }}
      h1 {{
        color: #333;
        border-bottom: 2px solid #007bff;
        padding-bottom: 10px;
      }}
      h2 {{
        color: #555;
        margin-top: 30px;
      }}
      .response {{
        background-color: #f8f9fa;
        border: 1px solid #dee2e6;
        border-radius: 4px;
        padding: 20px;
        margin-top: 20px;
      }}
      .status {{
        display: inline-block;
        padding: 5px 10px;
        border-radius: 4px;
        font-weight: bold;
        margin-bottom: 10px;
      }}
      .status-success {{
        background-color: #d4edda;
        color: #155724;
      }}
      .status-error {{
        background-color: #f8d7da;
        color: #721c24;
      }}
      .status-info {{
        background-color: #d1ecf1;
        color: #0c5460;
      }}
      .form-section {{
        margin-top: 30px;
        padding: 20px;
        background-color: #f8f9fa;
        border-radius: 4px;
      }}
      .form-group {{
        margin-bottom: 15px;
      }}
      .form-group label {{
        display: block;
        margin-bottom: 5px;
        font-weight: 500;
        color: #333;
      }}
      .form-control {{
        width: 100%;
        padding: 8px 12px;
        border: 1px solid #ced4da;
        border-radius: 4px;
        font-size: 14px;
        box-sizing: border-box;
      }}
      .form-control:focus {{
        outline: none;
        border-color: #007bff;
        box-shadow: 0 0 0 0.2rem rgba(0, 123, 255, 0.25);
      }}
      .btn {{
        padding: 10px 20px;
        border: none;
        border-radius: 4px;
        cursor: pointer;
        font-size: 14px;
        font-weight: 500;
      }}
      .btn-primary {{
        background-color: #007bff;
        color: white;
      }}
      .btn-primary:hover {{
        background-color: #0056b3;
      }}
      .invalid-feedback {{
        color: #dc3545;
        font-size: 12px;
        margin-top: 5px;
      }}
      .form-text {{
        font-size: 12px;
        color: #6c757d;
        margin-top: 5px;
        display: block;
      }}
    </style>
  </head>
  <body>
    <div class="container">
      <h1>Reinhardt Browsable API - {}</h1>
      <div class="response">
        <div class="status {}">{}</div>
        <h2>Response</h2>
        {}
      </div>
    </div>
  </body>
</html>
"#,
			escaped_title, escaped_title, status_class, status_text, content
		)
	}
}

impl Default for BrowsableApiRenderer {
	fn default() -> Self {
		Self::new("Browsable API", ColorScheme::default())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn test_renderer_creation() {
		let renderer = BrowsableApiRenderer::new("Test API", ColorScheme::Dark);
		assert_eq!(renderer.title, "Test API");
	}

	#[test]
	fn test_render_json() {
		let renderer = BrowsableApiRenderer::new("Test", ColorScheme::Dark);
		let data = json!({"message": "Hello, world!"});
		let result = renderer.render_json(&data, 200);
		let html = result.unwrap();
		assert!(html.contains("Hello, world!"));
		assert!(html.contains("200"));
	}

	#[test]
	fn test_render_with_form() {
		let renderer = BrowsableApiRenderer::new("Test", ColorScheme::Light);
		let data = json!({"id": 1});
		let mut form = FormGenerator::new("/api/test/", "POST");
		form.add_field("name", "text", true);

		let result = renderer.render_with_form(&data, 201, &form);
		let html = result.unwrap();
		assert!(html.contains("form"));
		assert!(html.contains("name"));
	}

	#[test]
	fn test_status_info() {
		let renderer = BrowsableApiRenderer::new("Test", ColorScheme::Dark);
		let (class, text) = renderer.get_status_info(200);
		assert_eq!(class, "status-success");
		assert!(text.contains("200"));
	}

	#[test]
	fn test_status_messages() {
		let renderer = BrowsableApiRenderer::new("Test", ColorScheme::Dark);
		assert_eq!(renderer.get_status_message(200), "OK");
		assert_eq!(renderer.get_status_message(404), "Not Found");
		assert_eq!(renderer.get_status_message(500), "Internal Server Error");
	}

	#[test]
	fn test_set_color_scheme() {
		let mut renderer = BrowsableApiRenderer::new("Test", ColorScheme::Dark);
		renderer.set_color_scheme(ColorScheme::Light);
		assert_eq!(renderer.highlighter.color_scheme(), ColorScheme::Light);
	}

	#[test]
	fn test_default_renderer() {
		let renderer = BrowsableApiRenderer::default();
		assert_eq!(renderer.title, "Browsable API");
	}
}
