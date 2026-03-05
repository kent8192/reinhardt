//! HTML templates for browsable API

use reinhardt_core::security::escape_html;

/// API template generator
pub struct ApiTemplate;

impl ApiTemplate {
	/// Generate HTML for browsable API
	///
	/// All user-controlled values are HTML-escaped to prevent XSS.
	pub fn render(title: &str, data: &str, method: &str, path: &str) -> String {
		let escaped_title = escape_html(title);
		let escaped_method = escape_html(method);
		let escaped_path = escape_html(path);
		let escaped_data = escape_html(data);

		format!(
			r#"<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
    <style>
        body {{ font-family: sans-serif; margin: 20px; }}
        .header {{ background: #f5f5f5; padding: 20px; border-radius: 5px; }}
        .method {{ color: #0066cc; font-weight: bold; }}
        .path {{ color: #666; }}
        .content {{ margin-top: 20px; background: #fff; padding: 20px; border: 1px solid #ddd; }}
        pre {{ background: #f9f9f9; padding: 15px; overflow-x: auto; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>{}</h1>
        <p><span class="method">{}</span> <span class="path">{}</span></p>
    </div>
    <div class="content">
        <h2>Response</h2>
        <pre>{}</pre>
    </div>
</body>
</html>"#,
			escaped_title, escaped_title, escaped_method, escaped_path, escaped_data
		)
	}
	/// Generate error page
	///
	/// The error message is HTML-escaped to prevent XSS.
	pub fn render_error(status: u16, message: &str) -> String {
		let escaped_message = escape_html(message);

		format!(
			r#"<!DOCTYPE html>
<html>
<head>
    <title>Error {}</title>
    <style>
        body {{ font-family: sans-serif; margin: 20px; }}
        .error {{ background: #fee; padding: 20px; border-radius: 5px; border-left: 4px solid #c00; }}
    </style>
</head>
<body>
    <div class="error">
        <h1>Error {}</h1>
        <p>{}</p>
    </div>
</body>
</html>"#,
			status, status, escaped_message
		)
	}
}
