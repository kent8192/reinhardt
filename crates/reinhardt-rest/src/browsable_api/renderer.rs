use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use tera::Tera;

#[derive(Debug, thiserror::Error)]
pub enum BrowsableApiError {
	#[error("Template render error: {0}")]
	Render(String),
	#[error("Template error: {0}")]
	Template(String),
	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),
	#[error("{0}")]
	Other(String),
}

impl From<tera::Error> for BrowsableApiError {
	fn from(err: tera::Error) -> Self {
		BrowsableApiError::Render(err.to_string())
	}
}

pub type BrowsableApiResult<T> = Result<T, BrowsableApiError>;

/// Context for rendering browsable API HTML
#[derive(Debug, Clone, Serialize)]
pub struct ApiContext {
	pub title: String,
	pub description: Option<String>,
	pub endpoint: String,
	pub method: String,
	pub response_data: Value,
	pub response_status: u16,
	pub allowed_methods: Vec<String>,
	pub request_form: Option<FormContext>,
	pub headers: Vec<(String, String)>,
	/// CSRF token for form protection
	pub csrf_token: Option<String>,
}

/// Context for rendering request forms
#[derive(Debug, Clone, Serialize)]
pub struct FormContext {
	pub fields: Vec<FormField>,
	pub submit_url: String,
	pub submit_method: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FormField {
	pub name: String,
	pub label: String,
	pub field_type: String,
	pub required: bool,
	pub help_text: Option<String>,
	pub initial_value: Option<Value>,
	pub options: Option<Vec<SelectOption>>,
	pub initial_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelectOption {
	pub value: String,
	pub label: String,
}

/// Renderer for browsable API HTML responses
pub struct BrowsableApiRenderer {
	tera: Arc<Tera>,
}

impl BrowsableApiRenderer {
	/// Create a new BrowsableApiRenderer with default templates
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::browsable_api::renderer::BrowsableApiRenderer;
	/// let renderer = BrowsableApiRenderer::new();
	/// ```
	pub fn new() -> Self {
		let mut tera = Tera::default();

		// Enable autoescape for HTML (this is the default but we make it explicit)
		tera.autoescape_on(vec![".html", ".tpl"]);

		// Register template from external file
		let template_path = concat!(env!("CARGO_MANIFEST_DIR"), "/templates/api.tpl");
		if let Err(e) = tera.add_template_file(template_path, Some("api.html")) {
			// Fallback to default template if file cannot be read
			eprintln!(
				"Warning: Failed to load template file: {}. Using default template.",
				e
			);
			tera.add_raw_template("api.html", Self::default_template())
				.expect("Failed to register default template");
		}

		Self {
			tera: Arc::new(tera),
		}
	}
	/// Render API context as HTML
	///
	pub fn render(&self, context: &ApiContext) -> BrowsableApiResult<String> {
		// Convert the context to a Tera Context
		let mut tera_context = tera::Context::from_serialize(context)?;

		// Add formatted JSON
		let formatted_json = serde_json::to_string_pretty(&context.response_data)?;
		tera_context.insert("response_data_formatted", &formatted_json);

		// Process form fields to convert initial_value (serde_json::Value) to string
		// This ensures proper HTML escaping by Tera's automatic escaping
		// IMPORTANT: Use Tera-compatible struct instead of serde_json::json!()
		// to enable automatic HTML escaping
		if let Some(form) = &context.request_form {
			// Create a new FormContext with string initial values
			use serde::Serialize;

			#[derive(Serialize)]
			struct FieldWithText<'a> {
				name: &'a str,
				label: &'a str,
				field_type: &'a str,
				required: bool,
				help_text: Option<&'a str>,
				initial_value_text: Option<String>,
				options: Option<&'a Vec<SelectOption>>,
				initial_label: Option<&'a str>,
			}

			#[derive(Serialize)]
			struct FormWithText<'a> {
				fields: Vec<FieldWithText<'a>>,
				submit_url: &'a str,
				submit_method: &'a str,
			}

			let fields_with_text: Vec<FieldWithText> = form
				.fields
				.iter()
				.map(|field| {
					let initial_value_text = field.initial_value.as_ref().and_then(|v| match v {
						serde_json::Value::String(s) => Some(s.clone()),
						serde_json::Value::Number(n) => Some(n.to_string()),
						serde_json::Value::Bool(b) => Some(b.to_string()),
						serde_json::Value::Null => None,
						other => Some(other.to_string()),
					});

					FieldWithText {
						name: &field.name,
						label: &field.label,
						field_type: &field.field_type,
						required: field.required,
						help_text: field.help_text.as_deref(),
						initial_value_text,
						options: field.options.as_ref(),
						initial_label: field.initial_label.as_deref(),
					}
				})
				.collect();

			let form_with_text = FormWithText {
				fields: fields_with_text,
				submit_url: &form.submit_url,
				submit_method: &form.submit_method,
			};

			tera_context.insert("request_form_text", &form_with_text);
		}

		Ok(self.tera.render("api.html", &tera_context)?)
	}
	/// Register a custom template
	///
	pub fn register_template(&mut self, name: &str, template: &str) -> BrowsableApiResult<()> {
		let tera_mut = Arc::get_mut(&mut self.tera).ok_or_else(|| {
			BrowsableApiError::Other("Cannot modify shared template registry".to_string())
		})?;
		tera_mut
			.add_raw_template(name, template)
			.map_err(|e| BrowsableApiError::Template(e.to_string()))?;
		Ok(())
	}

	/// Default HTML template
	fn default_template() -> &'static str {
		r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ title }} - Reinhardt API</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background: #f5f5f5; }
        .container { max-width: 1200px; margin: 0 auto; background: white; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .header { padding: 20px; border-bottom: 1px solid #e0e0e0; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; border-radius: 8px 8px 0 0; }
        .header h1 { margin: 0 0 10px 0; font-size: 24px; }
        .header p { margin: 0; opacity: 0.9; }
        .content { padding: 20px; }
        .method-badge { display: inline-block; padding: 4px 12px; border-radius: 4px; font-weight: bold; font-size: 12px; margin-right: 10px; }
        .method-get { background: #4caf50; color: white; }
        .method-post { background: #2196f3; color: white; }
        .method-put { background: #ff9800; color: white; }
        .method-patch { background: #9c27b0; color: white; }
        .method-delete { background: #f44336; color: white; }
        .endpoint { font-family: monospace; background: #f5f5f5; padding: 8px 12px; border-radius: 4px; display: inline-block; margin: 10px 0; }
        .response { background: #263238; color: #aed581; padding: 20px; border-radius: 4px; overflow-x: auto; margin: 20px 0; }
        .response pre { margin: 0; white-space: pre-wrap; word-wrap: break-word; }
        .form-section { margin: 20px 0; padding: 20px; background: #f9f9f9; border-radius: 4px; }
        .form-field { margin-bottom: 15px; }
        .form-field label { display: block; margin-bottom: 5px; font-weight: 500; }
        .form-field input, .form-field textarea, .form-field select { width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px; font-size: 14px; }
        .form-field textarea { min-height: 100px; font-family: monospace; }
        .help-text { font-size: 12px; color: #666; margin-top: 4px; }
        .submit-btn { background: #667eea; color: white; border: none; padding: 10px 20px; border-radius: 4px; cursor: pointer; font-size: 14px; font-weight: 500; }
        .submit-btn:hover { background: #5568d3; }
        .allowed-methods { margin: 15px 0; }
        .allowed-methods span { margin-right: 10px; }
        .headers { margin: 20px 0; }
        .headers table { width: 100%; border-collapse: collapse; }
        .headers th, .headers td { text-align: left; padding: 8px; border-bottom: 1px solid #e0e0e0; }
        .headers th { font-weight: 500; background: #f5f5f5; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>{{ title }}</h1>
            {% if description %}<p>{{ description }}</p>{% endif %}
        </div>

        <div class="content">
            <div class="allowed-methods">
                <strong>Allowed methods:</strong>
                {% for method_name in allowed_methods %}
                <span class="method-badge method-{{ method_name | lower }}">{{ method_name }}</span>
                {% endfor %}
            </div>

            <div class="endpoint">
                <span class="method-badge method-{{ method | lower }}">{{ method }}</span>
                {{ endpoint }}
            </div>

            <h2>Response ({{ response_status }})</h2>
            <div class="response">
                <pre>{{ response_data_formatted }}</pre>
            </div>

            {% if request_form_text %}
            <div class="form-section">
                <h2>Make a Request</h2>
                <form method="{{ request_form_text.submit_method }}" action="{{ request_form_text.submit_url }}">
                    {% if csrf_token %}
                    <input type="hidden" name="csrfmiddlewaretoken" value="{{ csrf_token }}">
                    {% endif %}
                    {% for field in request_form_text.fields %}
                    <div class="form-field">
                        <label for="{{ field.name }}">
                            {{ field.label }}
                            {% if field.required %}<span style="color: red;">*</span>{% endif %}
                        </label>
                        {% if field.field_type == "select" %}
                        <select id="{{ field.name }}" name="{{ field.name }}" {% if field.required %}required{% endif %}>
                            {% if field.initial_label %}
                            <option value="" selected>{{ field.initial_label }}</option>
                            {% endif %}
                            {% for option in field.options %}
                            <option value="{{ option.value }}" {% if option.value == field.initial_value_text %}selected{% endif %}>{{ option.label }}</option>
                            {% endfor %}
                        </select>
                        {% elif field.field_type == "textarea" %}
                        <textarea id="{{ field.name }}" name="{{ field.name }}" {% if field.required %}required{% endif %}>{% if field.initial_value_text %}{{ field.initial_value_text }}{% endif %}</textarea>
                        {% else %}
                        <input type="{{ field.field_type }}" id="{{ field.name }}" name="{{ field.name }}" {% if field.required %}required{% endif %} {% if field.initial_value_text %}value="{{ field.initial_value_text }}"{% endif %}>
                        {% endif %}
                        {% if field.help_text %}<div class="help-text">{{ field.help_text }}</div>{% endif %}
                    </div>
                    {% endfor %}
                    <button type="submit" class="submit-btn">Submit</button>
                </form>
            </div>
            {% endif %}

            {% if headers %}
            <div class="headers">
                <h2>Response Headers</h2>
                <table>
                    <thead>
                        <tr>
                            <th>Header</th>
                            <th>Value</th>
                        </tr>
                    </thead>
                    <tbody>
                        {% for header in headers %}
                        <tr>
                            <td><strong>{{ header.0 }}</strong></td>
                            <td>{{ header.1 }}</td>
                        </tr>
                        {% endfor %}
                    </tbody>
                </table>
            </div>
            {% endif %}
        </div>
    </div>
</body>
</html>
"#
	}
}

impl Default for BrowsableApiRenderer {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_render_basic_context() {
		let renderer = BrowsableApiRenderer::new();
		let context = ApiContext {
			title: "User List".to_string(),
			description: Some("List all users".to_string()),
			endpoint: "/api/users/".to_string(),
			method: "GET".to_string(),
			response_data: serde_json::json!([
				{"id": 1, "name": "Alice"},
				{"id": 2, "name": "Bob"}
			]),
			response_status: 200,
			allowed_methods: vec!["GET".to_string(), "POST".to_string()],
			request_form: None,
			headers: vec![("Content-Type".to_string(), "application/json".to_string())],
			csrf_token: None,
		};

		let html = renderer.render(&context).unwrap();
		assert!(html.contains("User List"));
		// Tera autoescapes `/` as `&#x2F;` for XSS protection
		assert!(html.contains("&#x2F;api&#x2F;users&#x2F;"));
		assert!(html.contains("Alice"));
		assert!(html.contains("Bob"));
	}

	#[test]
	fn test_render_with_form() {
		let renderer = BrowsableApiRenderer::new();
		let context = ApiContext {
			title: "Create User".to_string(),
			description: None,
			endpoint: "/api/users/".to_string(),
			method: "POST".to_string(),
			response_data: serde_json::json!({"message": "Success"}),
			response_status: 201,
			allowed_methods: vec!["GET".to_string(), "POST".to_string()],
			request_form: Some(FormContext {
				fields: vec![FormField {
					name: "name".to_string(),
					label: "Name".to_string(),
					field_type: "text".to_string(),
					required: true,
					help_text: Some("Enter user name".to_string()),
					initial_value: None,
					options: None,
					initial_label: None,
				}],
				submit_url: "/api/users/".to_string(),
				submit_method: "POST".to_string(),
			}),
			headers: vec![],
			csrf_token: None,
		};

		let html = renderer.render(&context).unwrap();
		assert!(html.contains("Make a Request"));
		assert!(html.contains("name=\"name\""));
		assert!(html.contains("Enter user name"));
	}

	#[test]
	fn test_render_select_field() {
		let renderer = BrowsableApiRenderer::new();
		let context = ApiContext {
			title: "Create Post".to_string(),
			description: None,
			endpoint: "/api/posts/".to_string(),
			method: "POST".to_string(),
			response_data: serde_json::json!({}),
			response_status: 200,
			allowed_methods: vec!["POST".to_string()],
			request_form: Some(FormContext {
				fields: vec![FormField {
					name: "category".to_string(),
					label: "Category".to_string(),
					field_type: "select".to_string(),
					required: true,
					help_text: Some("Select a category".to_string()),
					initial_value: Some(serde_json::json!("tech")),
					options: Some(vec![
						SelectOption {
							value: "tech".to_string(),
							label: "Technology".to_string(),
						},
						SelectOption {
							value: "science".to_string(),
							label: "Science".to_string(),
						},
						SelectOption {
							value: "art".to_string(),
							label: "Art".to_string(),
						},
					]),
					initial_label: None,
				}],
				submit_url: "/api/posts/".to_string(),
				submit_method: "POST".to_string(),
			}),
			headers: vec![],
			csrf_token: None,
		};

		let html = renderer.render(&context).unwrap();
		assert!(html.contains("<select"));
		assert!(html.contains("name=\"category\""));
		assert!(html.contains("Technology"));
		assert!(html.contains("Science"));
		assert!(html.contains("Art"));
		assert!(html.contains("value=\"tech\""));
		assert!(html.contains("value=\"science\""));
		assert!(html.contains("value=\"art\""));
	}

	#[test]
	fn test_render_select_with_initial_label() {
		// Test: Select field with initial_label displays placeholder option
		let renderer = BrowsableApiRenderer::new();
		let context = ApiContext {
			title: "Create Item".to_string(),
			description: None,
			endpoint: "/api/items/".to_string(),
			method: "POST".to_string(),
			response_data: serde_json::json!({}),
			response_status: 200,
			allowed_methods: vec!["POST".to_string()],
			request_form: Some(FormContext {
				fields: vec![FormField {
					name: "category".to_string(),
					label: "Category".to_string(),
					field_type: "select".to_string(),
					required: false,
					help_text: Some("Choose a category".to_string()),
					initial_value: None,
					options: Some(vec![
						SelectOption {
							value: "tech".to_string(),
							label: "Technology".to_string(),
						},
						SelectOption {
							value: "science".to_string(),
							label: "Science".to_string(),
						},
					]),
					initial_label: Some("-- Select a category --".to_string()),
				}],
				submit_url: "/api/items/".to_string(),
				submit_method: "POST".to_string(),
			}),
			headers: vec![],
			csrf_token: None,
		};

		let html = renderer.render(&context).unwrap();

		// Verify select element exists
		assert!(html.contains("<select"));
		assert!(html.contains("name=\"category\""));

		// Verify initial option is rendered with empty value and selected attribute
		assert!(html.contains("-- Select a category --"));
		assert!(html.contains(r#"<option value="" selected>-- Select a category --</option>"#));

		// Verify regular options are present
		assert!(html.contains("Technology"));
		assert!(html.contains("Science"));

		// Verify initial option appears before regular options
		let initial_pos = html.find("-- Select a category --").unwrap();
		let tech_pos = html.find("Technology").unwrap();
		assert!(
			initial_pos < tech_pos,
			"Initial option should appear before regular options"
		);
	}
}
