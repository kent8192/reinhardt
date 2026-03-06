//! Link column type implementation

use crate::builder::html::{a, td};
use crate::dom::Element;
use crate::tables::column::Column as ColumnTrait;
use std::any::Any;

/// Column that renders values as hyperlinks
///
/// This column type renders values as HTML anchor tags with customizable URLs.
/// The URL pattern can include placeholders like `{id}` that will be replaced
/// with actual values.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_pages::tables::columns::LinkColumn;
///
/// // Simple link
/// let column = LinkColumn::new("name", "Name", "/users/{id}");
///
/// // With custom text
/// let column = LinkColumn::with_text("email", "Email", "/users/{id}", "View Profile");
/// ```
pub struct LinkColumn {
	name: String,
	label: String,
	url_pattern: String,
	text_override: Option<String>,
	orderable: bool,
	visible: bool,
}

impl LinkColumn {
	/// Creates a new link column
	///
	/// # Arguments
	///
	/// * `name` - The internal field name
	/// * `label` - The display label
	/// * `url_pattern` - URL pattern with placeholders (e.g., "/users/{id}")
	pub fn new(
		name: impl Into<String>,
		label: impl Into<String>,
		url_pattern: impl Into<String>,
	) -> Self {
		Self {
			name: name.into(),
			label: label.into(),
			url_pattern: url_pattern.into(),
			text_override: None,
			orderable: true,
			visible: true,
		}
	}

	/// Creates a new link column with custom link text
	///
	/// # Arguments
	///
	/// * `name` - The internal field name
	/// * `label` - The display label
	/// * `url_pattern` - URL pattern with placeholders (e.g., "/users/{id}")
	/// * `text` - Custom text to display instead of the value
	pub fn with_text(
		name: impl Into<String>,
		label: impl Into<String>,
		url_pattern: impl Into<String>,
		text: impl Into<String>,
	) -> Self {
		Self {
			name: name.into(),
			label: label.into(),
			url_pattern: url_pattern.into(),
			text_override: Some(text.into()),
			orderable: true,
			visible: true,
		}
	}

	/// Sets whether this column is orderable
	pub fn orderable(mut self, orderable: bool) -> Self {
		self.orderable = orderable;
		self
	}

	/// Sets whether this column is visible
	pub fn visible(mut self, visible: bool) -> Self {
		self.visible = visible;
		self
	}
}

impl ColumnTrait for LinkColumn {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> &str {
		&self.label
	}

	fn render(&self, value: &dyn Any) -> Element {
		let value_str = if let Some(s) = value.downcast_ref::<String>() {
			s.as_str().to_string()
		} else if let Some(s) = value.downcast_ref::<&str>() {
			(*s).to_string()
		} else if let Some(n) = value.downcast_ref::<i32>() {
			n.to_string()
		} else if let Some(n) = value.downcast_ref::<i64>() {
			n.to_string()
		} else {
			String::new()
		};

		// Replace {field_name} placeholders in URL pattern
		let url = self
			.url_pattern
			.replace(&format!("{{{}}}", self.name), &value_str);

		let text = self.text_override.as_deref().unwrap_or(&value_str);
		let link = a().attr("href", &url).text(text).build();
		td().child(link).build()
	}

	fn is_orderable(&self) -> bool {
		self.orderable
	}

	fn is_visible(&self) -> bool {
		self.visible
	}
}
