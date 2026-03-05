use crate::field::{FieldError, FieldResult, FormField, Widget};
use regex::Regex;
use std::net::Ipv6Addr;
use std::sync::OnceLock;

/// RegexField for pattern-based validation
///
/// Compiled regex is cached using `OnceLock` to avoid repeated
/// compilation which could lead to ReDoS via allocation overhead.
pub struct RegexField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	/// Cached compiled regex to prevent repeated compilation (ReDoS mitigation)
	regex_cache: OnceLock<Regex>,
	/// Raw pattern string stored for lazy compilation
	pattern: String,
	pub error_message: String,
	pub max_length: Option<usize>,
	pub min_length: Option<usize>,
}

impl RegexField {
	/// Create a new RegexField
	///
	/// The regex is compiled lazily on first use and cached for subsequent calls.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::RegexField;
	///
	/// let field = RegexField::new("pattern".to_string(), r"^\d+$").unwrap();
	/// assert_eq!(field.name, "pattern");
	/// ```
	pub fn new(name: String, pattern: &str) -> Result<Self, regex::Error> {
		// Validate the pattern eagerly so callers get errors at construction time
		let compiled = Regex::new(pattern)?;
		let cache = OnceLock::new();
		cache
			.set(compiled)
			.unwrap_or_else(|_| panic!("OnceLock should be empty at construction"));
		Ok(Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
			regex_cache: cache,
			pattern: pattern.to_string(),
			error_message: "Enter a valid value".to_string(),
			max_length: None,
			min_length: None,
		})
	}

	/// Get the cached compiled regex
	fn regex(&self) -> &Regex {
		self.regex_cache.get_or_init(|| {
			Regex::new(&self.pattern).expect("Pattern was validated at construction")
		})
	}
	pub fn with_error_message(mut self, message: String) -> Self {
		self.error_message = message;
		self
	}
}

impl FormField for RegexField {
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
			None => Ok(serde_json::Value::Null),
			Some(v) => {
				let s = v
					.as_str()
					.ok_or_else(|| FieldError::invalid(None, "Expected string"))?;

				if s.is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::Null);
				}

				// Length validation using character count (not byte count)
				// for correct multi-byte character handling
				let char_count = s.chars().count();
				if let Some(max) = self.max_length
					&& char_count > max
				{
					return Err(FieldError::validation(
						None,
						&format!("Ensure this value has at most {} characters", max),
					));
				}

				if let Some(min) = self.min_length
					&& char_count < min
				{
					return Err(FieldError::validation(
						None,
						&format!("Ensure this value has at least {} characters", min),
					));
				}

				// Regex validation (uses cached compiled regex)
				if !self.regex().is_match(s) {
					return Err(FieldError::validation(None, &self.error_message));
				}

				Ok(serde_json::Value::String(s.to_string()))
			}
		}
	}
}

/// SlugField for URL-safe slugs
pub struct SlugField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub max_length: Option<usize>,
	pub allow_unicode: bool,
}

impl SlugField {
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
			max_length: Some(50),
			allow_unicode: false,
		}
	}

	fn is_valid_slug(&self, s: &str) -> bool {
		if self.allow_unicode {
			s.chars().all(|c| {
				c.is_alphanumeric() || c == '-' || c == '_' || (!c.is_ascii() && c.is_alphabetic())
			})
		} else {
			s.chars()
				.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
		}
	}
}

impl FormField for SlugField {
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
			None => Ok(serde_json::Value::Null),
			Some(v) => {
				let s = v
					.as_str()
					.ok_or_else(|| FieldError::invalid(None, "Expected string"))?;

				if s.is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::Null);
				}

				// Use character count for correct multi-byte handling
				if let Some(max) = self.max_length
					&& s.chars().count() > max
				{
					return Err(FieldError::validation(
						None,
						&format!("Ensure this value has at most {} characters", max),
					));
				}

				if !self.is_valid_slug(s) {
					let msg = if self.allow_unicode {
						"Enter a valid slug consisting of Unicode letters, numbers, underscores, or hyphens"
					} else {
						"Enter a valid slug consisting of letters, numbers, underscores or hyphens"
					};
					return Err(FieldError::validation(None, msg));
				}

				Ok(serde_json::Value::String(s.to_string()))
			}
		}
	}
}

/// GenericIPAddressField for IPv4 and IPv6 addresses
pub struct GenericIPAddressField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub protocol: IPProtocol,
}

#[derive(Debug, Clone, Copy)]
pub enum IPProtocol {
	Both,
	IPv4,
	IPv6,
}

impl GenericIPAddressField {
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
			protocol: IPProtocol::Both,
		}
	}

	fn is_valid_ipv4(&self, s: &str) -> bool {
		let parts: Vec<&str> = s.split('.').collect();
		if parts.len() != 4 {
			return false;
		}

		parts.iter().all(|part| {
			part.parse::<u8>()
				.map(|_| !part.starts_with('0') || part.len() == 1)
				.unwrap_or(false)
		})
	}

	fn is_valid_ipv6(&self, s: &str) -> bool {
		// Use std::net::Ipv6Addr for comprehensive IPv6 validation,
		// covering compressed (::1), IPv4-mapped (::ffff:192.0.2.1),
		// and all other valid IPv6 address formats.
		s.parse::<Ipv6Addr>().is_ok()
	}
}

impl FormField for GenericIPAddressField {
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
			None => Ok(serde_json::Value::Null),
			Some(v) => {
				let s = v
					.as_str()
					.ok_or_else(|| FieldError::invalid(None, "Expected string"))?;

				if s.is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::Null);
				}

				let is_valid = match self.protocol {
					IPProtocol::IPv4 => self.is_valid_ipv4(s),
					IPProtocol::IPv6 => self.is_valid_ipv6(s),
					IPProtocol::Both => self.is_valid_ipv4(s) || self.is_valid_ipv6(s),
				};

				if !is_valid {
					return Err(FieldError::validation(None, "Enter a valid IP address"));
				}

				Ok(serde_json::Value::String(s.to_string()))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_regex_field() {
		let field = RegexField::new("code".to_string(), r"^[A-Z]{3}\d{3}$").unwrap();

		assert!(field.clean(Some(&serde_json::json!("ABC123"))).is_ok());
		assert!(matches!(
			field.clean(Some(&serde_json::json!("abc123"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_forms_regex_field_slug() {
		let field = SlugField::new("slug".to_string());

		assert!(field.clean(Some(&serde_json::json!("my-slug"))).is_ok());
		assert!(field.clean(Some(&serde_json::json!("my_slug"))).is_ok());
		assert!(matches!(
			field.clean(Some(&serde_json::json!("my slug"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_ip_field_ipv4() {
		let mut field = GenericIPAddressField::new("ip".to_string());
		field.protocol = IPProtocol::IPv4;

		assert!(field.clean(Some(&serde_json::json!("192.168.1.1"))).is_ok());
		assert!(matches!(
			field.clean(Some(&serde_json::json!("999.999.999.999"))),
			Err(FieldError::Validation(_))
		));
	}

	#[test]
	fn test_ip_field_ipv6() {
		let mut field = GenericIPAddressField::new("ip".to_string());
		field.protocol = IPProtocol::IPv6;

		assert!(
			field
				.clean(Some(&serde_json::json!(
					"2001:0db8:85a3:0000:0000:8a2e:0370:7334"
				)))
				.is_ok()
		);
		assert!(field.clean(Some(&serde_json::json!("::1"))).is_ok());
	}

	#[rstest]
	#[case("::1", true)]
	#[case("::", true)]
	#[case("::ffff:192.0.2.1", true)]
	#[case("2001:db8::1", true)]
	#[case("fe80::1%eth0", false)]
	#[case("2001:db8:85a3::8a2e:370:7334", true)]
	#[case("::ffff:10.0.0.1", true)]
	#[case("2001:db8::", true)]
	#[case("::192.168.1.1", true)]
	#[case("not-an-ip", false)]
	#[case("2001:db8::g1", false)]
	#[case("12345::1", false)]
	fn test_ipv6_comprehensive_validation(#[case] input: &str, #[case] should_accept: bool) {
		// Arrange
		let mut field = GenericIPAddressField::new("ip".to_string());
		field.protocol = IPProtocol::IPv6;

		// Act
		let result = field.clean(Some(&serde_json::json!(input)));

		// Assert
		if should_accept {
			assert!(
				result.is_ok(),
				"Expected valid IPv6 '{}' to be accepted, got: {:?}",
				input,
				result,
			);
		} else {
			assert!(
				result.is_err(),
				"Expected invalid IPv6 '{}' to be rejected, got: {:?}",
				input,
				result,
			);
		}
	}
}
