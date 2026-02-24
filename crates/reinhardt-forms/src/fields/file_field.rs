use crate::field::{FieldError, FieldResult, FormField, Widget};

/// Default maximum file size: 10 MB
const DEFAULT_FILE_MAX_SIZE: u64 = 10 * 1024 * 1024;

/// Default maximum image file size: 5 MB
const DEFAULT_IMAGE_MAX_SIZE: u64 = 5 * 1024 * 1024;

/// FileField for file upload
pub struct FileField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub max_length: Option<usize>,
	pub allow_empty_file: bool,
	/// Maximum file size in bytes. Defaults to 10 MB.
	pub max_size: u64,
}

impl FileField {
	/// Create a new FileField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::FileField;
	///
	/// let field = FileField::new("upload".to_string());
	/// assert_eq!(field.name, "upload");
	/// assert_eq!(field.max_size, 10 * 1024 * 1024);
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::FileInput,
			initial: None,
			max_length: None,
			allow_empty_file: false,
			max_size: DEFAULT_FILE_MAX_SIZE,
		}
	}

	/// Set the maximum file size in bytes.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::FileField;
	///
	/// let field = FileField::new("upload".to_string()).with_max_size(5 * 1024 * 1024);
	/// assert_eq!(field.max_size, 5 * 1024 * 1024);
	/// ```
	pub fn with_max_size(mut self, max_size: u64) -> Self {
		self.max_size = max_size;
		self
	}
}

impl FormField for FileField {
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
				// Expect an object with filename and optional size
				let obj = v
					.as_object()
					.ok_or_else(|| FieldError::Invalid("Expected object".to_string()))?;

				let filename = obj
					.get("filename")
					.and_then(|f| f.as_str())
					.ok_or_else(|| FieldError::Invalid("Missing filename".to_string()))?;

				if filename.is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::Null);
				}

				// Check filename length
				if let Some(max) = self.max_length
					&& filename.len() > max
				{
					return Err(FieldError::Validation(format!(
						"Filename is too long (max {} characters)",
						max
					)));
				}

				// Check file size limit before further processing
				if let Some(size) = obj.get("size").and_then(|s| s.as_u64()) {
					if size > self.max_size {
						return Err(FieldError::Validation(format!(
							"File size {} bytes exceeds maximum allowed size of {} bytes",
							size, self.max_size
						)));
					}

					// Check for empty file
					if !self.allow_empty_file && size == 0 {
						return Err(FieldError::Validation(
							"The submitted file is empty".to_string(),
						));
					}
				} else if !self.allow_empty_file {
					// No size field present and empty files not allowed
					return Err(FieldError::Validation(
						"The submitted file is empty".to_string(),
					));
				}

				Ok(v.clone())
			}
		}
	}
}

/// ImageField for image upload with additional validation
pub struct ImageField {
	pub name: String,
	pub label: Option<String>,
	pub required: bool,
	pub help_text: Option<String>,
	pub widget: Widget,
	pub initial: Option<serde_json::Value>,
	pub max_length: Option<usize>,
	pub allow_empty_file: bool,
	/// Maximum file size in bytes. Defaults to 5 MB.
	pub max_size: u64,
}

impl ImageField {
	/// Create a new ImageField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::ImageField;
	///
	/// let field = ImageField::new("photo".to_string());
	/// assert_eq!(field.name, "photo");
	/// assert_eq!(field.max_size, 5 * 1024 * 1024);
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			label: None,
			required: true,
			help_text: None,
			widget: Widget::FileInput,
			initial: None,
			max_length: None,
			allow_empty_file: false,
			max_size: DEFAULT_IMAGE_MAX_SIZE,
		}
	}

	/// Set the maximum file size in bytes.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::fields::ImageField;
	///
	/// let field = ImageField::new("photo".to_string()).with_max_size(2 * 1024 * 1024);
	/// assert_eq!(field.max_size, 2 * 1024 * 1024);
	/// ```
	pub fn with_max_size(mut self, max_size: u64) -> Self {
		self.max_size = max_size;
		self
	}

	fn is_valid_image_extension(filename: &str) -> bool {
		// NOTE: SVG is intentionally excluded due to Stored XSS risk.
		// SVG files can contain arbitrary JavaScript that executes when served
		// with Content-Type: image/svg+xml. Use opt-in validation if SVG support
		// is required, with appropriate sanitization or Content-Disposition headers.
		let valid_extensions = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];
		filename
			.rsplit('.')
			.next()
			.map(|ext| valid_extensions.contains(&ext.to_lowercase().as_str()))
			.unwrap_or(false)
	}
}

impl FormField for ImageField {
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
				let obj = v
					.as_object()
					.ok_or_else(|| FieldError::Invalid("Expected object".to_string()))?;

				let filename = obj
					.get("filename")
					.and_then(|f| f.as_str())
					.ok_or_else(|| FieldError::Invalid("Missing filename".to_string()))?;

				if filename.is_empty() {
					if self.required {
						return Err(FieldError::required(None));
					}
					return Ok(serde_json::Value::Null);
				}

				// Validate image extension
				if !Self::is_valid_image_extension(filename) {
					return Err(FieldError::Validation(
						"Upload a valid image. The file you uploaded was either not an image or a corrupted image".to_string(),
					));
				}

				// Check filename length
				if let Some(max) = self.max_length
					&& filename.len() > max
				{
					return Err(FieldError::Validation(format!(
						"Filename is too long (max {} characters)",
						max
					)));
				}

				// Check file size limit before further processing
				if let Some(size) = obj.get("size").and_then(|s| s.as_u64()) {
					if size > self.max_size {
						return Err(FieldError::Validation(format!(
							"File size {} bytes exceeds maximum allowed size of {} bytes",
							size, self.max_size
						)));
					}

					// Check for empty file
					if !self.allow_empty_file && size == 0 {
						return Err(FieldError::Validation(
							"The submitted file is empty".to_string(),
						));
					}
				} else if !self.allow_empty_file {
					// No size field present and empty files not allowed
					return Err(FieldError::Validation(
						"The submitted file is empty".to_string(),
					));
				}

				Ok(v.clone())
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// =========================================================================
	// FileField Tests
	// =========================================================================

	// ---- Happy Path ----

	#[rstest]
	fn test_filefield_valid() {
		// Arrange
		let field = FileField::new("document".to_string());
		let file = serde_json::json!({
			"filename": "test.pdf",
			"size": 1024
		});

		// Act
		let result = field.clean(Some(&file));

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_filefield_default_max_size() {
		// Arrange & Act
		let field = FileField::new("document".to_string());

		// Assert
		assert_eq!(field.max_size, 10 * 1024 * 1024);
	}

	#[rstest]
	fn test_filefield_custom_max_size() {
		// Arrange & Act
		let field = FileField::new("document".to_string()).with_max_size(5 * 1024 * 1024);

		// Assert
		assert_eq!(field.max_size, 5 * 1024 * 1024);
	}

	#[rstest]
	fn test_filefield_within_size_limit() {
		// Arrange
		let field = FileField::new("document".to_string()).with_max_size(1024);
		let file = serde_json::json!({
			"filename": "test.pdf",
			"size": 1024
		});

		// Act
		let result = field.clean(Some(&file));

		// Assert
		assert!(result.is_ok());
	}

	// ---- Error Cases ----

	#[rstest]
	fn test_filefield_exceeds_size_limit() {
		// Arrange
		let field = FileField::new("document".to_string()).with_max_size(1024);
		let file = serde_json::json!({
			"filename": "test.pdf",
			"size": 1025
		});

		// Act
		let result = field.clean(Some(&file));

		// Assert
		assert!(
			matches!(result, Err(FieldError::Validation(ref msg)) if msg.contains("exceeds maximum"))
		);
	}

	#[rstest]
	fn test_filefield_exceeds_default_size_limit() {
		// Arrange
		let field = FileField::new("document".to_string());
		let over_10mb = 10 * 1024 * 1024 + 1;
		let file = serde_json::json!({
			"filename": "huge.bin",
			"size": over_10mb
		});

		// Act
		let result = field.clean(Some(&file));

		// Assert
		assert!(
			matches!(result, Err(FieldError::Validation(ref msg)) if msg.contains("exceeds maximum"))
		);
	}

	#[rstest]
	fn test_filefield_empty() {
		// Arrange
		let field = FileField::new("document".to_string());
		let file = serde_json::json!({
			"filename": "test.pdf",
			"size": 0
		});

		// Act & Assert
		assert!(matches!(
			field.clean(Some(&file)),
			Err(FieldError::Validation(_))
		));
	}

	#[rstest]
	fn test_filefield_no_size_field_rejects_when_empty_not_allowed() {
		// Arrange
		let field = FileField::new("document".to_string());
		let file = serde_json::json!({
			"filename": "test.pdf"
		});

		// Act
		let result = field.clean(Some(&file));

		// Assert
		assert!(matches!(result, Err(FieldError::Validation(ref msg)) if msg.contains("empty")));
	}

	// ---- Boundary Value Analysis ----

	#[rstest]
	#[case(1023, true)] // max_size - 1
	#[case(1024, true)] // max_size (boundary)
	#[case(1025, false)] // max_size + 1
	fn test_filefield_size_boundary(#[case] size: u64, #[case] valid: bool) {
		// Arrange
		let field = FileField::new("document".to_string()).with_max_size(1024);
		let file = serde_json::json!({
			"filename": "test.pdf",
			"size": size
		});

		// Act & Assert
		assert_eq!(field.clean(Some(&file)).is_ok(), valid);
	}

	// ---- Decision Table ----

	#[rstest]
	#[case(1024, 512, true)] // max_size=1024, size=512 -> OK
	#[case(1024, 1024, true)] // max_size=1024, size=1024 -> OK (at limit)
	#[case(1024, 2048, false)] // max_size=1024, size=2048 -> Error
	#[case(0, 1, false)] // max_size=0, size=1 -> Error (zero tolerance)
	fn test_filefield_size_decision_table(
		#[case] max_size: u64,
		#[case] file_size: u64,
		#[case] expected_ok: bool,
	) {
		// Arrange
		let field = FileField::new("document".to_string()).with_max_size(max_size);
		let file = serde_json::json!({
			"filename": "test.pdf",
			"size": file_size
		});

		// Act & Assert
		assert_eq!(field.clean(Some(&file)).is_ok(), expected_ok);
	}

	// =========================================================================
	// ImageField Tests
	// =========================================================================

	// ---- Happy Path ----

	#[rstest]
	fn test_imagefield_valid() {
		// Arrange
		let field = ImageField::new("photo".to_string());
		let file = serde_json::json!({
			"filename": "test.jpg",
			"size": 1024
		});

		// Act & Assert
		assert!(field.clean(Some(&file)).is_ok());
	}

	#[rstest]
	fn test_imagefield_default_max_size() {
		// Arrange & Act
		let field = ImageField::new("photo".to_string());

		// Assert
		assert_eq!(field.max_size, 5 * 1024 * 1024);
	}

	#[rstest]
	fn test_imagefield_custom_max_size() {
		// Arrange & Act
		let field = ImageField::new("photo".to_string()).with_max_size(2 * 1024 * 1024);

		// Assert
		assert_eq!(field.max_size, 2 * 1024 * 1024);
	}

	// ---- Error Cases ----

	#[rstest]
	fn test_imagefield_invalid_extension() {
		// Arrange
		let field = ImageField::new("photo".to_string());
		let file = serde_json::json!({
			"filename": "test.pdf",
			"size": 1024
		});

		// Act & Assert
		assert!(matches!(
			field.clean(Some(&file)),
			Err(FieldError::Validation(_))
		));
	}

	#[rstest]
	fn test_imagefield_rejects_svg_for_xss_prevention() {
		// Arrange
		let field = ImageField::new("photo".to_string());
		// SVG files are rejected due to Stored XSS vulnerability risk
		let svg_file = serde_json::json!({
			"filename": "malicious.svg",
			"size": 1024
		});

		// Act & Assert
		assert!(
			matches!(field.clean(Some(&svg_file)), Err(FieldError::Validation(_))),
			"SVG files should be rejected to prevent Stored XSS attacks"
		);
	}

	#[rstest]
	fn test_imagefield_exceeds_size_limit() {
		// Arrange
		let field = ImageField::new("photo".to_string()).with_max_size(1024);
		let file = serde_json::json!({
			"filename": "large.jpg",
			"size": 1025
		});

		// Act
		let result = field.clean(Some(&file));

		// Assert
		assert!(
			matches!(result, Err(FieldError::Validation(ref msg)) if msg.contains("exceeds maximum"))
		);
	}

	#[rstest]
	fn test_imagefield_exceeds_default_size_limit() {
		// Arrange
		let field = ImageField::new("photo".to_string());
		let over_5mb = 5 * 1024 * 1024 + 1;
		let file = serde_json::json!({
			"filename": "huge.png",
			"size": over_5mb
		});

		// Act
		let result = field.clean(Some(&file));

		// Assert
		assert!(
			matches!(result, Err(FieldError::Validation(ref msg)) if msg.contains("exceeds maximum"))
		);
	}

	// ---- Boundary Value Analysis ----

	#[rstest]
	#[case(2047, true)] // max_size - 1
	#[case(2048, true)] // max_size (boundary)
	#[case(2049, false)] // max_size + 1
	fn test_imagefield_size_boundary(#[case] size: u64, #[case] valid: bool) {
		// Arrange
		let field = ImageField::new("photo".to_string()).with_max_size(2048);
		let file = serde_json::json!({
			"filename": "photo.jpg",
			"size": size
		});

		// Act & Assert
		assert_eq!(field.clean(Some(&file)).is_ok(), valid);
	}
}
