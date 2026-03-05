use crate::field::{FieldError, FieldResult, FormField, Widget};

/// URLField for URL input
pub struct URLField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub max_length: Option<usize>,
}

impl URLField {
	/// Create a new URLField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::URLField;
	///
	/// let field = URLField::new("website".to_string());
	/// assert_eq!(field.name, "website");
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
			max_length: Some(200),
		}
	}

	fn validate_url(url: &str) -> bool {
		// Basic URL validation
		let url_regex = regex::Regex::new(
            r"^https?://(?:(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}|localhost|\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})(?::\d+)?(?:/[^\s]*)?$"
        ).unwrap();

		url_regex.is_match(url)
	}
}

impl FormField for URLField {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> Option<&str> {
		self.label.as_deref()
	}

	fn required(&self) -> bool {
		self.required
	}

	fn help_text(&self) -> Option<&str> {
		self.help_text.as_deref()
	}

	fn widget(&self) -> &Widget {
		&self.widget
	}

	fn initial(&self) -> Option<&serde_json::Value> {
		self.initial.as_ref()
	}

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value> {
		match value {
			None if self.required => Err(FieldError::required(None)),
			None => Ok(serde_json::Value::String(String::new())),
			Some(v) => {
				let s = v
					.as_str()
					.ok_or_else(|| FieldError::Invalid("Expected string".to_string()))?;

				let s = s.trim();

				if s.is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::String(String::new()));
				}

				// Check length using character count (not byte count)
				// for correct multi-byte character handling
				let char_count = s.chars().count();
				if let Some(max) = self.max_length
					&& char_count > max
				{
					return Err(FieldError::Validation(format!(
						"Ensure this value has at most {} characters (it has {})",
						max, char_count
					)));
				}

				// Validate URL format
				if !Self::validate_url(s) {
					return Err(FieldError::Validation("Enter a valid URL".to_string()));
				}

				Ok(serde_json::Value::String(s.to_string()))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_urlfield_valid() {
		let field = URLField::new("website".to_string());

		assert_eq!(
			field
				.clean(Some(&serde_json::json!("https://example.com")))
				.unwrap(),
			serde_json::json!("https://example.com")
		);
		assert_eq!(
			field
				.clean(Some(&serde_json::json!("http://test.org/path")))
				.unwrap(),
			serde_json::json!("http://test.org/path")
		);
	}

	#[test]
	fn test_urlfield_invalid() {
		let field = URLField::new("website".to_string());

		assert!(matches!(
			field.clean(Some(&serde_json::json!("not a url"))),
			Err(FieldError::Validation(_))
		));
		assert!(matches!(
			field.clean(Some(&serde_json::json!("ftp://example.com"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_urlfield_optional() {
		let mut field = URLField::new("website".to_string());
		field.required = false;

		assert_eq!(field.clean(None).unwrap(), serde_json::json!(""));
		assert_eq!(
			field.clean(Some(&serde_json::json!(""))).unwrap(),
			serde_json::json!("")
		);
	}
}
