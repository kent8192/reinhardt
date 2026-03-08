//! File type validator for MIME types and file extensions
//!
//! This validator provides Django-style file type validation with support for:
//! - File extension validation (e.g., `.jpg`, `.pdf`)
//! - MIME type validation (e.g., `image/jpeg`, `application/pdf`)
//! - Preset validators for common file categories (images, documents)
//!
//! # Examples
//!
//! ## Validate file extensions
//!
//! ```
//! use reinhardt_core::validators::{FileTypeValidator, Validator};
//!
//! let validator = FileTypeValidator::with_extensions(vec![
//!     "jpg".to_string(),
//!     "png".to_string(),
//! ]);
//!
//! assert!(validator.validate_filename("photo.jpg").is_ok());
//! assert!(validator.validate_filename("document.pdf").is_err());
//! ```
//!
//! ## Validate MIME types
//!
//! ```
//! use reinhardt_core::validators::{FileTypeValidator, Validator};
//!
//! let validator = FileTypeValidator::with_mime_types(vec![
//!     "image/jpeg".to_string(),
//!     "image/png".to_string(),
//! ]);
//!
//! assert!(validator.validate_mime_type("image/jpeg").is_ok());
//! assert!(validator.validate_mime_type("application/pdf").is_err());
//! ```
//!
//! ## Using preset validators
//!
//! ```
//! use reinhardt_core::validators::{FileTypeValidator, Validator};
//!
//! let validator = FileTypeValidator::images_only();
//! assert!(validator.validate_filename("photo.jpg").is_ok());
//! assert!(validator.validate_mime_type("image/png").is_ok());
//! assert!(validator.validate_filename("document.pdf").is_err());
//!
//! let validator = FileTypeValidator::documents_only();
//! assert!(validator.validate_filename("report.pdf").is_ok());
//! assert!(validator.validate_mime_type("application/msword").is_ok());
//! ```

use super::{ValidationError, ValidationResult, Validator};

/// File type validator for MIME types and file extensions
///
/// This validator can validate both file extensions and MIME types,
/// supporting whitelist-based filtering for security and type control.
pub struct FileTypeValidator {
	/// Allowed file extensions (without dot, e.g., "jpg", "pdf")
	pub allowed_extensions: Option<Vec<String>>,
	/// Allowed MIME types (e.g., "image/jpeg", "application/pdf")
	pub allowed_mime_types: Option<Vec<String>>,
}

// Common MIME type constants
impl FileTypeValidator {
	// Image MIME types

	/// MIME type for JPEG images (`image/jpeg`).
	pub const MIME_JPEG: &'static str = "image/jpeg";
	/// MIME type for PNG images (`image/png`).
	pub const MIME_PNG: &'static str = "image/png";
	/// MIME type for GIF images (`image/gif`).
	pub const MIME_GIF: &'static str = "image/gif";
	/// MIME type for WebP images (`image/webp`).
	pub const MIME_WEBP: &'static str = "image/webp";
	/// MIME type for SVG images (`image/svg+xml`).
	pub const MIME_SVG: &'static str = "image/svg+xml";
	/// MIME type for BMP images (`image/bmp`).
	pub const MIME_BMP: &'static str = "image/bmp";
	/// MIME type for TIFF images (`image/tiff`).
	pub const MIME_TIFF: &'static str = "image/tiff";

	// Document MIME types

	/// MIME type for PDF documents (`application/pdf`).
	pub const MIME_PDF: &'static str = "application/pdf";
	/// MIME type for legacy Microsoft Word documents (`application/msword`).
	pub const MIME_DOC: &'static str = "application/msword";
	/// MIME type for Microsoft Word OOXML documents.
	pub const MIME_DOCX: &'static str =
		"application/vnd.openxmlformats-officedocument.wordprocessingml.document";
	/// MIME type for legacy Microsoft Excel spreadsheets (`application/vnd.ms-excel`).
	pub const MIME_XLS: &'static str = "application/vnd.ms-excel";
	/// MIME type for Microsoft Excel OOXML spreadsheets.
	pub const MIME_XLSX: &'static str =
		"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
	/// MIME type for legacy Microsoft PowerPoint presentations (`application/vnd.ms-powerpoint`).
	pub const MIME_PPT: &'static str = "application/vnd.ms-powerpoint";
	/// MIME type for Microsoft PowerPoint OOXML presentations.
	pub const MIME_PPTX: &'static str =
		"application/vnd.openxmlformats-officedocument.presentationml.presentation";

	// Text MIME types

	/// MIME type for plain text files (`text/plain`).
	pub const MIME_TEXT: &'static str = "text/plain";
	/// MIME type for HTML documents (`text/html`).
	pub const MIME_HTML: &'static str = "text/html";
	/// MIME type for CSS stylesheets (`text/css`).
	pub const MIME_CSS: &'static str = "text/css";
	/// MIME type for JavaScript files (`text/javascript`).
	pub const MIME_JS: &'static str = "text/javascript";
	/// MIME type for JSON data (`application/json`).
	pub const MIME_JSON: &'static str = "application/json";
	/// MIME type for XML documents (`application/xml`).
	pub const MIME_XML: &'static str = "application/xml";

	// Archive MIME types

	/// MIME type for ZIP archives (`application/zip`).
	pub const MIME_ZIP: &'static str = "application/zip";
	/// MIME type for RAR archives (`application/x-rar-compressed`).
	pub const MIME_RAR: &'static str = "application/x-rar-compressed";
	/// MIME type for 7-Zip archives (`application/x-7z-compressed`).
	pub const MIME_7Z: &'static str = "application/x-7z-compressed";
	/// MIME type for TAR archives (`application/x-tar`).
	pub const MIME_TAR: &'static str = "application/x-tar";
	/// MIME type for GZIP compressed files (`application/gzip`).
	pub const MIME_GZIP: &'static str = "application/gzip";
}

impl FileTypeValidator {
	/// Creates a new validator with no restrictions
	///
	/// This validator will accept any file type.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FileTypeValidator;
	///
	/// let validator = FileTypeValidator::new();
	/// assert!(validator.validate_filename("any.file").is_ok());
	/// assert!(validator.validate_mime_type("any/type").is_ok());
	/// ```
	pub fn new() -> Self {
		Self {
			allowed_extensions: None,
			allowed_mime_types: None,
		}
	}

	/// Creates a validator that only allows specific file extensions
	///
	/// Extensions should be provided without the leading dot (e.g., "jpg", not ".jpg").
	/// The validation is case-insensitive.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FileTypeValidator;
	///
	/// let validator = FileTypeValidator::with_extensions(vec![
	///     "jpg".to_string(),
	///     "png".to_string(),
	/// ]);
	///
	/// assert!(validator.validate_filename("photo.jpg").is_ok());
	/// assert!(validator.validate_filename("photo.JPG").is_ok()); // Case-insensitive
	/// assert!(validator.validate_filename("document.pdf").is_err());
	/// ```
	pub fn with_extensions(extensions: Vec<String>) -> Self {
		Self {
			allowed_extensions: Some(extensions),
			allowed_mime_types: None,
		}
	}

	/// Creates a validator that only allows specific MIME types
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FileTypeValidator;
	///
	/// let validator = FileTypeValidator::with_mime_types(vec![
	///     "image/jpeg".to_string(),
	///     "image/png".to_string(),
	/// ]);
	///
	/// assert!(validator.validate_mime_type("image/jpeg").is_ok());
	/// assert!(validator.validate_mime_type("application/pdf").is_err());
	/// ```
	pub fn with_mime_types(mime_types: Vec<String>) -> Self {
		Self {
			allowed_extensions: None,
			allowed_mime_types: Some(mime_types),
		}
	}

	/// Creates a validator that only allows image files
	///
	/// Supports common image formats: JPEG, PNG, GIF, WebP, SVG, BMP, TIFF
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FileTypeValidator;
	///
	/// let validator = FileTypeValidator::images_only();
	/// assert!(validator.validate_filename("photo.jpg").is_ok());
	/// assert!(validator.validate_filename("image.png").is_ok());
	/// assert!(validator.validate_mime_type("image/jpeg").is_ok());
	/// assert!(validator.validate_filename("document.pdf").is_err());
	/// ```
	pub fn images_only() -> Self {
		Self {
			allowed_extensions: Some(vec![
				"jpg".to_string(),
				"jpeg".to_string(),
				"png".to_string(),
				"gif".to_string(),
				"webp".to_string(),
				"svg".to_string(),
				"bmp".to_string(),
				"tiff".to_string(),
				"tif".to_string(),
			]),
			allowed_mime_types: Some(vec![
				Self::MIME_JPEG.to_string(),
				Self::MIME_PNG.to_string(),
				Self::MIME_GIF.to_string(),
				Self::MIME_WEBP.to_string(),
				Self::MIME_SVG.to_string(),
				Self::MIME_BMP.to_string(),
				Self::MIME_TIFF.to_string(),
			]),
		}
	}

	/// Creates a validator that only allows document files
	///
	/// Supports: PDF, Microsoft Office formats (DOC, DOCX, XLS, XLSX, PPT, PPTX),
	/// and plain text files
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FileTypeValidator;
	///
	/// let validator = FileTypeValidator::documents_only();
	/// assert!(validator.validate_filename("report.pdf").is_ok());
	/// assert!(validator.validate_filename("data.xlsx").is_ok());
	/// assert!(validator.validate_mime_type("application/pdf").is_ok());
	/// assert!(validator.validate_filename("photo.jpg").is_err());
	/// ```
	pub fn documents_only() -> Self {
		Self {
			allowed_extensions: Some(vec![
				"pdf".to_string(),
				"doc".to_string(),
				"docx".to_string(),
				"xls".to_string(),
				"xlsx".to_string(),
				"ppt".to_string(),
				"pptx".to_string(),
				"txt".to_string(),
			]),
			allowed_mime_types: Some(vec![
				Self::MIME_PDF.to_string(),
				Self::MIME_DOC.to_string(),
				Self::MIME_DOCX.to_string(),
				Self::MIME_XLS.to_string(),
				Self::MIME_XLSX.to_string(),
				Self::MIME_PPT.to_string(),
				Self::MIME_PPTX.to_string(),
				Self::MIME_TEXT.to_string(),
			]),
		}
	}

	/// Validates a filename against allowed extensions
	///
	/// Returns Ok(()) if the file extension is allowed or if no extension restrictions exist.
	/// Returns Err(ValidationError) if the extension is not in the allowed list.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FileTypeValidator;
	///
	/// let validator = FileTypeValidator::with_extensions(vec!["jpg".to_string()]);
	/// assert!(validator.validate_filename("photo.jpg").is_ok());
	/// assert!(validator.validate_filename("photo.png").is_err());
	/// ```
	pub fn validate_filename(&self, filename: &str) -> ValidationResult<()> {
		if let Some(ref allowed) = self.allowed_extensions {
			// Extract file extension
			// rsplit returns the part after the last '.', or the entire string if no '.' exists
			if !filename.contains('.') {
				return Err(ValidationError::InvalidFileExtension {
					extension: "(none)".to_string(),
					allowed_extensions: allowed.join(", "),
				});
			}

			let extension = filename.rsplit('.').next().unwrap_or("").to_lowercase();

			if extension.is_empty() {
				return Err(ValidationError::InvalidFileExtension {
					extension: "(none)".to_string(),
					allowed_extensions: allowed.join(", "),
				});
			}

			// Check if extension is in allowed list (case-insensitive)
			let is_allowed = allowed.iter().any(|ext| ext.to_lowercase() == extension);

			if !is_allowed {
				return Err(ValidationError::InvalidFileExtension {
					extension: extension.to_string(),
					allowed_extensions: allowed.join(", "),
				});
			}
		}

		Ok(())
	}

	/// Validates a MIME type against allowed types
	///
	/// Returns Ok(()) if the MIME type is allowed or if no MIME type restrictions exist.
	/// Returns Err(ValidationError) if the MIME type is not in the allowed list.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FileTypeValidator;
	///
	/// let validator = FileTypeValidator::with_mime_types(vec!["image/jpeg".to_string()]);
	/// assert!(validator.validate_mime_type("image/jpeg").is_ok());
	/// assert!(validator.validate_mime_type("image/png").is_err());
	/// ```
	pub fn validate_mime_type(&self, mime_type: &str) -> ValidationResult<()> {
		// Basic MIME type format validation (type/subtype)
		if !mime_type.contains('/') {
			return Err(ValidationError::InvalidMimeType {
				mime_type: mime_type.to_string(),
				allowed_mime_types: self
					.allowed_mime_types
					.as_ref()
					.map(|v| v.join(", "))
					.unwrap_or_else(|| "(any)".to_string()),
			});
		}

		if let Some(ref allowed) = self.allowed_mime_types {
			let mime_lower = mime_type.to_lowercase();

			// Check if MIME type is in allowed list (case-insensitive)
			let is_allowed = allowed.iter().any(|mime| mime.to_lowercase() == mime_lower);

			if !is_allowed {
				return Err(ValidationError::InvalidMimeType {
					mime_type: mime_type.to_string(),
					allowed_mime_types: allowed.join(", "),
				});
			}
		}

		Ok(())
	}
}

impl Default for FileTypeValidator {
	fn default() -> Self {
		Self::new()
	}
}

/// File size validator with min/max/range constraints
///
/// Validates file sizes in bytes with flexible configuration options.
///
/// # Examples
///
/// ```
/// use reinhardt_core::validators::{FileSizeValidator, Validator};
///
/// // Minimum size
/// let validator = FileSizeValidator::min(FileSizeValidator::from_kb(100)); // 100 KB minimum
/// assert!(validator.validate(&(1024 * 1024)).is_ok()); // 1 MB
/// assert!(validator.validate(&(1024 * 50)).is_err()); // 50 KB
///
/// // Maximum size
/// let validator = FileSizeValidator::max(FileSizeValidator::from_mb(10)); // 10 MB maximum
/// assert!(validator.validate(&(1024 * 1024)).is_ok()); // 1 MB
/// assert!(validator.validate(&(1024 * 1024 * 20)).is_err()); // 20 MB
///
/// // Range
/// let validator = FileSizeValidator::range(
///     FileSizeValidator::from_kb(100),
///     FileSizeValidator::from_mb(10)
/// );
/// assert!(validator.validate(&(1024 * 1024)).is_ok()); // 1 MB
/// ```
#[derive(Debug, Clone)]
pub struct FileSizeValidator {
	min_bytes: Option<u64>,
	max_bytes: Option<u64>,
}

impl FileSizeValidator {
	/// Create a validator with minimum size constraint
	pub fn min(min_bytes: u64) -> Self {
		Self {
			min_bytes: Some(min_bytes),
			max_bytes: None,
		}
	}

	/// Create a validator with maximum size constraint
	pub fn max(max_bytes: u64) -> Self {
		Self {
			min_bytes: None,
			max_bytes: Some(max_bytes),
		}
	}

	/// Create a validator with both min and max constraints
	pub fn range(min_bytes: u64, max_bytes: u64) -> Self {
		Self {
			min_bytes: Some(min_bytes),
			max_bytes: Some(max_bytes),
		}
	}

	/// Helper: Convert kilobytes to bytes
	pub fn from_kb(kb: u64) -> u64 {
		kb * 1024
	}

	/// Helper: Convert megabytes to bytes
	pub fn from_mb(mb: u64) -> u64 {
		mb * 1024 * 1024
	}

	/// Helper: Convert gigabytes to bytes
	pub fn from_gb(gb: u64) -> u64 {
		gb * 1024 * 1024 * 1024
	}
}

impl Validator<u64> for FileSizeValidator {
	fn validate(&self, value: &u64) -> ValidationResult<()> {
		if let Some(min) = self.min_bytes
			&& *value < min
		{
			return Err(ValidationError::FileSizeTooSmall {
				size_bytes: *value,
				min_bytes: min,
			});
		}

		if let Some(max) = self.max_bytes
			&& *value > max
		{
			return Err(ValidationError::FileSizeTooLarge {
				size_bytes: *value,
				max_bytes: max,
			});
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_accepts_all() {
		let validator = FileTypeValidator::new();
		assert!(validator.validate_filename("any.file").is_ok());
		assert!(validator.validate_mime_type("any/type").is_ok());
	}

	#[test]
	fn test_with_extensions() {
		let validator =
			FileTypeValidator::with_extensions(vec!["jpg".to_string(), "png".to_string()]);

		assert!(validator.validate_filename("photo.jpg").is_ok());
		assert!(validator.validate_filename("image.png").is_ok());
		assert!(validator.validate_filename("document.pdf").is_err());
	}

	#[test]
	fn test_with_extensions_case_insensitive() {
		let validator = FileTypeValidator::with_extensions(vec!["jpg".to_string()]);

		assert!(validator.validate_filename("photo.jpg").is_ok());
		assert!(validator.validate_filename("photo.JPG").is_ok());
		assert!(validator.validate_filename("photo.Jpg").is_ok());
	}

	#[test]
	fn test_with_extensions_no_extension() {
		let validator = FileTypeValidator::with_extensions(vec!["jpg".to_string()]);

		match validator.validate_filename("noextension") {
			Err(ValidationError::InvalidFileExtension { extension, .. }) => {
				assert_eq!(extension, "(none)");
			}
			_ => panic!("Expected InvalidFileExtension error"),
		}
	}

	#[test]
	fn test_with_mime_types() {
		let validator = FileTypeValidator::with_mime_types(vec![
			"image/jpeg".to_string(),
			"image/png".to_string(),
		]);

		assert!(validator.validate_mime_type("image/jpeg").is_ok());
		assert!(validator.validate_mime_type("image/png").is_ok());
		assert!(validator.validate_mime_type("application/pdf").is_err());
	}

	#[test]
	fn test_with_mime_types_case_insensitive() {
		let validator = FileTypeValidator::with_mime_types(vec!["image/jpeg".to_string()]);

		assert!(validator.validate_mime_type("image/jpeg").is_ok());
		assert!(validator.validate_mime_type("IMAGE/JPEG").is_ok());
		assert!(validator.validate_mime_type("Image/Jpeg").is_ok());
	}

	#[test]
	fn test_mime_type_format_validation() {
		let validator = FileTypeValidator::new();
		assert!(validator.validate_mime_type("image/jpeg").is_ok());

		let validator = FileTypeValidator::with_mime_types(vec!["image/jpeg".to_string()]);
		assert!(validator.validate_mime_type("invalid").is_err());
	}

	#[test]
	fn test_images_only_extensions() {
		let validator = FileTypeValidator::images_only();

		assert!(validator.validate_filename("photo.jpg").is_ok());
		assert!(validator.validate_filename("photo.jpeg").is_ok());
		assert!(validator.validate_filename("image.png").is_ok());
		assert!(validator.validate_filename("animation.gif").is_ok());
		assert!(validator.validate_filename("picture.webp").is_ok());
		assert!(validator.validate_filename("vector.svg").is_ok());
		assert!(validator.validate_filename("bitmap.bmp").is_ok());
		assert!(validator.validate_filename("scan.tiff").is_ok());
		assert!(validator.validate_filename("scan.tif").is_ok());

		assert!(validator.validate_filename("document.pdf").is_err());
		assert!(validator.validate_filename("data.xlsx").is_err());
	}

	#[test]
	fn test_images_only_mime_types() {
		let validator = FileTypeValidator::images_only();

		assert!(validator.validate_mime_type("image/jpeg").is_ok());
		assert!(validator.validate_mime_type("image/png").is_ok());
		assert!(validator.validate_mime_type("image/gif").is_ok());
		assert!(validator.validate_mime_type("image/webp").is_ok());
		assert!(validator.validate_mime_type("image/svg+xml").is_ok());
		assert!(validator.validate_mime_type("image/bmp").is_ok());
		assert!(validator.validate_mime_type("image/tiff").is_ok());

		assert!(validator.validate_mime_type("application/pdf").is_err());
		assert!(validator.validate_mime_type("text/plain").is_err());
	}

	#[test]
	fn test_documents_only_extensions() {
		let validator = FileTypeValidator::documents_only();

		assert!(validator.validate_filename("report.pdf").is_ok());
		assert!(validator.validate_filename("letter.doc").is_ok());
		assert!(validator.validate_filename("document.docx").is_ok());
		assert!(validator.validate_filename("spreadsheet.xls").is_ok());
		assert!(validator.validate_filename("data.xlsx").is_ok());
		assert!(validator.validate_filename("presentation.ppt").is_ok());
		assert!(validator.validate_filename("slides.pptx").is_ok());
		assert!(validator.validate_filename("readme.txt").is_ok());

		assert!(validator.validate_filename("photo.jpg").is_err());
		assert!(validator.validate_filename("archive.zip").is_err());
	}

	#[test]
	fn test_documents_only_mime_types() {
		let validator = FileTypeValidator::documents_only();

		assert!(validator.validate_mime_type("application/pdf").is_ok());
		assert!(validator.validate_mime_type("application/msword").is_ok());
		assert!(
			validator
				.validate_mime_type(
					"application/vnd.openxmlformats-officedocument.wordprocessingml.document"
				)
				.is_ok()
		);
		assert!(
			validator
				.validate_mime_type("application/vnd.ms-excel")
				.is_ok()
		);
		assert!(
			validator
				.validate_mime_type(
					"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
				)
				.is_ok()
		);
		assert!(
			validator
				.validate_mime_type("application/vnd.ms-powerpoint")
				.is_ok()
		);
		assert!(
			validator
				.validate_mime_type(
					"application/vnd.openxmlformats-officedocument.presentationml.presentation"
				)
				.is_ok()
		);
		assert!(validator.validate_mime_type("text/plain").is_ok());

		assert!(validator.validate_mime_type("image/jpeg").is_err());
		assert!(validator.validate_mime_type("application/zip").is_err());
	}

	#[test]
	fn test_error_messages() {
		let validator = FileTypeValidator::with_extensions(vec!["jpg".to_string()]);

		match validator.validate_filename("document.pdf") {
			Err(ValidationError::InvalidFileExtension {
				extension,
				allowed_extensions,
			}) => {
				assert_eq!(extension, "pdf");
				assert_eq!(allowed_extensions, "jpg");
			}
			_ => panic!("Expected InvalidFileExtension error"),
		}
	}

	#[test]
	fn test_mime_type_error_messages() {
		let validator = FileTypeValidator::with_mime_types(vec!["image/jpeg".to_string()]);

		match validator.validate_mime_type("application/pdf") {
			Err(ValidationError::InvalidMimeType {
				mime_type,
				allowed_mime_types,
			}) => {
				assert_eq!(mime_type, "application/pdf");
				assert_eq!(allowed_mime_types, "image/jpeg");
			}
			_ => panic!("Expected InvalidMimeType error"),
		}
	}

	#[test]
	fn test_default_implementation() {
		let validator = FileTypeValidator::default();
		assert!(validator.validate_filename("any.file").is_ok());
		assert!(validator.validate_mime_type("any/type").is_ok());
	}

	#[test]
	fn test_multiple_dots_in_filename() {
		let validator = FileTypeValidator::with_extensions(vec!["gz".to_string()]);
		assert!(validator.validate_filename("archive.tar.gz").is_ok());
		assert!(validator.validate_filename("file.backup.txt").is_err());
	}

	#[test]
	fn test_mime_type_with_parameters() {
		let validator = FileTypeValidator::new();
		// MIME types can have parameters like "text/html; charset=utf-8"
		// This validator only checks the basic format
		assert!(
			validator
				.validate_mime_type("text/html; charset=utf-8")
				.is_ok()
		);
	}

	#[test]
	fn test_common_mime_type_constants() {
		// Verify that constants are defined correctly
		assert_eq!(FileTypeValidator::MIME_JPEG, "image/jpeg");
		assert_eq!(FileTypeValidator::MIME_PNG, "image/png");
		assert_eq!(FileTypeValidator::MIME_PDF, "application/pdf");
		assert_eq!(FileTypeValidator::MIME_JSON, "application/json");
	}

	#[test]
	fn test_filename_with_path() {
		let validator = FileTypeValidator::with_extensions(vec!["jpg".to_string()]);
		// Should work with full paths
		assert!(validator.validate_filename("/path/to/photo.jpg").is_ok());
		assert!(validator.validate_filename("C:\\Users\\photo.jpg").is_ok());
		assert!(validator.validate_filename("../relative/photo.jpg").is_ok());
	}

	#[test]
	fn test_empty_allowed_lists() {
		let validator = FileTypeValidator {
			allowed_extensions: Some(vec![]),
			allowed_mime_types: Some(vec![]),
		};

		// Empty whitelist should reject everything
		assert!(validator.validate_filename("file.txt").is_err());
		assert!(validator.validate_mime_type("text/plain").is_err());
	}

	// FileSizeValidator tests
	#[test]
	fn test_file_size_min_validator() {
		let validator = FileSizeValidator::min(1024); // 1 KB minimum
		assert!(validator.validate(&2048).is_ok()); // 2 KB
		assert!(validator.validate(&1024).is_ok()); // Exactly 1 KB
		assert!(validator.validate(&512).is_err()); // 512 bytes
	}

	#[test]
	fn test_file_size_max_validator() {
		let validator = FileSizeValidator::max(1024 * 1024); // 1 MB maximum
		assert!(validator.validate(&(1024 * 512)).is_ok()); // 512 KB
		assert!(validator.validate(&(1024 * 1024)).is_ok()); // Exactly 1 MB
		assert!(validator.validate(&(1024 * 1024 * 2)).is_err()); // 2 MB
	}

	#[test]
	fn test_file_size_range_validator() {
		let validator = FileSizeValidator::range(1024, 1024 * 1024); // 1 KB to 1 MB
		assert!(validator.validate(&(1024 * 512)).is_ok()); // 512 KB
		assert!(validator.validate(&1024).is_ok()); // Exactly 1 KB (min)
		assert!(validator.validate(&(1024 * 1024)).is_ok()); // Exactly 1 MB (max)
		assert!(validator.validate(&512).is_err()); // Too small
		assert!(validator.validate(&(1024 * 1024 * 2)).is_err()); // Too large
	}

	#[test]
	fn test_file_size_helper_kb() {
		assert_eq!(FileSizeValidator::from_kb(1), 1024);
		assert_eq!(FileSizeValidator::from_kb(100), 102400);
	}

	#[test]
	fn test_file_size_helper_mb() {
		assert_eq!(FileSizeValidator::from_mb(1), 1024 * 1024);
		assert_eq!(FileSizeValidator::from_mb(10), 10 * 1024 * 1024);
	}

	#[test]
	fn test_file_size_helper_gb() {
		assert_eq!(FileSizeValidator::from_gb(1), 1024 * 1024 * 1024);
		assert_eq!(FileSizeValidator::from_gb(2), 2 * 1024 * 1024 * 1024);
	}

	#[test]
	fn test_file_size_zero_bytes() {
		let validator = FileSizeValidator::min(1);
		assert!(validator.validate(&0).is_err());
	}

	#[test]
	fn test_file_size_u64_max() {
		let validator = FileSizeValidator::max(u64::MAX);
		assert!(validator.validate(&u64::MAX).is_ok());
		assert!(validator.validate(&(u64::MAX - 1)).is_ok());
	}

	#[test]
	fn test_file_size_error_messages() {
		let validator = FileSizeValidator::min(1024);
		match validator.validate(&512) {
			Err(ValidationError::FileSizeTooSmall {
				size_bytes,
				min_bytes,
			}) => {
				assert_eq!(size_bytes, 512);
				assert_eq!(min_bytes, 1024);
			}
			_ => panic!("Expected FileSizeTooSmall error"),
		}

		let validator = FileSizeValidator::max(1024);
		match validator.validate(&2048) {
			Err(ValidationError::FileSizeTooLarge {
				size_bytes,
				max_bytes,
			}) => {
				assert_eq!(size_bytes, 2048);
				assert_eq!(max_bytes, 1024);
			}
			_ => panic!("Expected FileSizeTooLarge error"),
		}
	}

	#[test]
	fn test_file_size_boundary_values() {
		let validator = FileSizeValidator::range(100, 200);
		// Boundary tests
		assert!(validator.validate(&99).is_err()); // Just below min
		assert!(validator.validate(&100).is_ok()); // Exactly min
		assert!(validator.validate(&101).is_ok()); // Just above min
		assert!(validator.validate(&199).is_ok()); // Just below max
		assert!(validator.validate(&200).is_ok()); // Exactly max
		assert!(validator.validate(&201).is_err()); // Just above max
	}
}
