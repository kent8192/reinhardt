//! Error types for deeplink configuration and handling.

use thiserror::Error;

/// Maximum allowed length for a URL scheme name.
const MAX_SCHEME_LENGTH: usize = 64;

/// Maximum allowed length for a bundle ID.
const MAX_BUNDLE_ID_LENGTH: usize = 155;

/// Dangerous URL schemes that must be rejected to prevent XSS and other attacks.
const DANGEROUS_SCHEMES: &[&str] = &["javascript", "data", "vbscript", "file"];

/// Errors that can occur during deeplink configuration and handling.
#[derive(Debug, Error)]
pub enum DeeplinkError {
	/// Invalid iOS app ID format.
	///
	/// App IDs must follow the format `TEAM_ID.bundle_identifier`.
	#[error("invalid iOS app ID format: {0}. Expected format: TEAM_ID.bundle_identifier")]
	InvalidAppId(String),

	/// Invalid Android package name format.
	///
	/// Package names must follow Java package naming conventions.
	#[error(
		"invalid Android package name: {0}. Expected Java package format (e.g., com.example.app)"
	)]
	InvalidPackageName(String),

	/// Invalid Android SHA256 fingerprint format.
	///
	/// Fingerprints must be 32 colon-separated hex bytes (e.g., `FA:C6:17:...`).
	#[error("invalid Android fingerprint format: {0}. Expected 32 colon-separated hex bytes")]
	InvalidFingerprint(String),

	/// Invalid custom URL scheme name.
	///
	/// Scheme names must comply with RFC 3986: start with a letter, followed by
	/// letters, digits, `+`, `-`, or `.`. Dangerous schemes are rejected.
	#[error("invalid URL scheme name: {0}")]
	InvalidSchemeName(String),

	/// Invalid bundle ID format.
	///
	/// Bundle IDs must follow reverse-domain notation with at least 2 segments.
	#[error("invalid bundle ID format: {0}")]
	InvalidBundleId(String),

	/// No paths specified for iOS Universal Links.
	#[error("no paths specified for iOS Universal Links")]
	NoPathsSpecified,

	/// Package name is required for Android App Links.
	#[error("package name required for Android App Links")]
	MissingPackageName,

	/// At least one SHA256 fingerprint is required for Android.
	#[error("at least one SHA256 fingerprint required for Android")]
	MissingFingerprint,

	/// iOS configuration is required but not provided.
	#[error("iOS configuration required but not provided")]
	MissingIosConfig,

	/// Android configuration is required but not provided.
	#[error("Android configuration required but not provided")]
	MissingAndroidConfig,

	/// JSON serialization failed.
	#[error("serialization failed: {0}")]
	Serialization(#[from] serde_json::Error),
}

/// Validates an iOS app ID format.
///
/// Valid format: `TEAM_ID.bundle_identifier` where:
/// - TEAM_ID is typically 10 alphanumeric characters
/// - bundle_identifier follows reverse domain notation (validated by [`validate_bundle_id`])
///
/// # Errors
///
/// Returns `DeeplinkError::InvalidAppId` if the format is invalid.
pub fn validate_app_id(app_id: &str) -> Result<(), DeeplinkError> {
	// Split into team ID and bundle ID (requires at least one dot)
	let Some((team_id, bundle_id)) = app_id.split_once('.') else {
		return Err(DeeplinkError::InvalidAppId(app_id.to_string()));
	};

	// Team ID should be alphanumeric (typically 10 characters, but we allow flexibility)
	if team_id.is_empty() || !team_id.chars().all(|c| c.is_ascii_alphanumeric()) {
		return Err(DeeplinkError::InvalidAppId(app_id.to_string()));
	}

	// Validate the bundle ID portion using strict reverse-domain notation
	validate_bundle_id(bundle_id).map_err(|_| DeeplinkError::InvalidAppId(app_id.to_string()))?;

	Ok(())
}

/// Validates a bundle ID follows reverse-domain notation.
///
/// Valid bundle IDs must:
/// - Have at least 2 segments separated by dots (e.g., `com.example`)
/// - Each segment must start with a letter or underscore
/// - Each segment may contain only ASCII alphanumeric characters, hyphens, or underscores
/// - Total length must not exceed `MAX_BUNDLE_ID_LENGTH` (155) characters
///
/// # Errors
///
/// Returns `DeeplinkError::InvalidBundleId` if the format is invalid.
pub fn validate_bundle_id(bundle_id: &str) -> Result<(), DeeplinkError> {
	if bundle_id.is_empty() || bundle_id.len() > MAX_BUNDLE_ID_LENGTH {
		return Err(DeeplinkError::InvalidBundleId(bundle_id.to_string()));
	}

	let segments: Vec<&str> = bundle_id.split('.').collect();

	// Must have at least 2 segments (reverse-domain notation)
	if segments.len() < 2 {
		return Err(DeeplinkError::InvalidBundleId(bundle_id.to_string()));
	}

	for segment in &segments {
		if !is_valid_bundle_segment(segment) {
			return Err(DeeplinkError::InvalidBundleId(bundle_id.to_string()));
		}
	}

	Ok(())
}

/// Checks if a single bundle ID segment is valid.
///
/// A valid segment:
/// - Is not empty
/// - Starts with a letter or underscore
/// - Contains only ASCII alphanumeric characters, hyphens, or underscores
fn is_valid_bundle_segment(segment: &str) -> bool {
	if segment.is_empty() {
		return false;
	}

	let first = segment.as_bytes()[0];
	if !first.is_ascii_alphabetic() && first != b'_' {
		return false;
	}

	segment
		.bytes()
		.all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Validates a custom URL scheme name per RFC 3986.
///
/// Valid scheme names must:
/// - Start with an ASCII letter
/// - Contain only ASCII letters, digits, `+`, `-`, or `.`
/// - Not be a dangerous scheme (`javascript`, `data`, `vbscript`, `file`)
/// - Not exceed `MAX_SCHEME_LENGTH` (64) characters
///
/// # Errors
///
/// Returns `DeeplinkError::InvalidSchemeName` if the scheme is invalid.
pub fn validate_scheme_name(scheme: &str) -> Result<(), DeeplinkError> {
	if scheme.is_empty() || scheme.len() > MAX_SCHEME_LENGTH {
		return Err(DeeplinkError::InvalidSchemeName(scheme.to_string()));
	}

	// RFC 3986: scheme must start with a letter
	if !scheme.as_bytes()[0].is_ascii_alphabetic() {
		return Err(DeeplinkError::InvalidSchemeName(scheme.to_string()));
	}

	// RFC 3986: followed by letters, digits, +, -, or .
	if !scheme
		.bytes()
		.all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
	{
		return Err(DeeplinkError::InvalidSchemeName(scheme.to_string()));
	}

	// Reject dangerous schemes (case-insensitive comparison)
	let lower = scheme.to_ascii_lowercase();
	if DANGEROUS_SCHEMES.contains(&lower.as_str()) {
		return Err(DeeplinkError::InvalidSchemeName(scheme.to_string()));
	}

	Ok(())
}

/// Validates an Android package name format.
///
/// Android package names must follow Java package naming conventions:
/// - Must contain at least one dot separator
/// - Each segment must start with a letter
/// - Only letters, digits, and underscores are allowed in each segment
/// - Must not be empty
///
/// # Errors
///
/// Returns `DeeplinkError::InvalidPackageName` if the format is invalid.
pub fn validate_package_name(name: &str) -> Result<(), DeeplinkError> {
	if name.is_empty() {
		return Err(DeeplinkError::InvalidPackageName(name.to_string()));
	}

	// Must contain at least one dot
	if !name.contains('.') {
		return Err(DeeplinkError::InvalidPackageName(name.to_string()));
	}

	let segments: Vec<&str> = name.split('.').collect();
	for segment in &segments {
		// Each segment must not be empty
		if segment.is_empty() {
			return Err(DeeplinkError::InvalidPackageName(name.to_string()));
		}

		// Each segment must start with a letter
		let first_char = segment
			.chars()
			.next()
			.expect("segment is non-empty after the emptiness check above");
		if !first_char.is_ascii_alphabetic() {
			return Err(DeeplinkError::InvalidPackageName(name.to_string()));
		}

		// Each segment can only contain letters, digits, and underscores
		if !segment
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '_')
		{
			return Err(DeeplinkError::InvalidPackageName(name.to_string()));
		}
	}

	Ok(())
}

/// Validates an Android SHA256 fingerprint format.
///
/// Valid format: 32 colon-separated hex bytes (e.g., `FA:C6:17:45:...`).
///
/// # Errors
///
/// Returns `DeeplinkError::InvalidFingerprint` if the format is invalid.
pub fn validate_fingerprint(fingerprint: &str) -> Result<(), DeeplinkError> {
	let parts: Vec<&str> = fingerprint.split(':').collect();

	// Must have exactly 32 bytes
	if parts.len() != 32 {
		return Err(DeeplinkError::InvalidFingerprint(fingerprint.to_string()));
	}

	// Each part must be exactly 2 hex characters
	for part in parts {
		if part.len() != 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
			return Err(DeeplinkError::InvalidFingerprint(fingerprint.to_string()));
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	// -- validate_app_id --

	#[rstest]
	#[case("TEAM123456.com.example.app", true)]
	#[case("ABC123XYZ0.com.example.myapp", true)]
	#[case("TEAM.com.example", true)]
	#[case("TEAM.com.example-app", true)]
	#[case("TEAM._private.app", true)]
	#[case("invalid", false)] // no dot
	#[case("", false)] // empty
	#[case(".com.example", false)] // empty team ID
	#[case("TEAM.", false)] // empty bundle ID
	#[case("TEAM.bundle", false)] // single-segment bundle ID (not reverse-domain)
	#[case("TEAM.com.", false)] // trailing dot creates empty segment
	#[case("TEAM..com", false)] // empty segment between dots
	#[case("TEAM.123.app", false)] // segment starting with digit
	#[case("TEAM.com.app!x", false)] // invalid character in segment
	fn test_validate_app_id(#[case] app_id: &str, #[case] expected_valid: bool) {
		// Arrange
		// (inputs provided by #[case])

		// Act
		let result = validate_app_id(app_id);

		// Assert
		assert_eq!(result.is_ok(), expected_valid, "app_id: {}", app_id);
	}

	// -- validate_bundle_id --

	#[rstest]
	#[case("com.example", true)]
	#[case("com.example.app", true)]
	#[case("io.github.user", true)]
	#[case("com.my-app.test", true)]
	#[case("com._private.app", true)]
	#[case("org.example.my_app", true)]
	#[case("", false)] // empty
	#[case("single", false)] // single segment
	#[case(".com.example", false)] // leading dot creates empty segment
	#[case("com.example.", false)] // trailing dot creates empty segment
	#[case("com..example", false)] // empty segment
	#[case("123.example", false)] // segment starting with digit
	#[case("com.123app", false)] // segment starting with digit
	#[case("com.app!x", false)] // invalid character
	#[case("com.app x", false)] // space in segment
	fn test_validate_bundle_id(#[case] bundle_id: &str, #[case] expected_valid: bool) {
		// Arrange
		// (inputs provided by #[case])

		// Act
		let result = validate_bundle_id(bundle_id);

		// Assert
		assert_eq!(result.is_ok(), expected_valid, "bundle_id: {}", bundle_id);
	}

	#[rstest]
	fn test_validate_bundle_id_exceeds_max_length() {
		// Arrange
		let long_bundle_id = format!("com.{}", "a".repeat(MAX_BUNDLE_ID_LENGTH));

		// Act
		let result = validate_bundle_id(&long_bundle_id);

		// Assert
		assert!(
			result.is_err(),
			"bundle ID exceeding max length should be rejected"
		);
	}

	// -- validate_scheme_name --

	#[rstest]
	#[case("myapp", true)]
	#[case("my-app", true)]
	#[case("my.app", true)]
	#[case("my+app", true)]
	#[case("a123", true)]
	#[case("x", true)]
	#[case("MyApp", true)] // uppercase allowed per RFC 3986
	#[case("", false)] // empty
	#[case("1app", false)] // starts with digit
	#[case("-app", false)] // starts with hyphen
	#[case(".app", false)] // starts with dot
	#[case("my app", false)] // space
	#[case("my_app", false)] // underscore not allowed in scheme
	#[case("my@app", false)] // special character
	#[case("javascript", false)] // dangerous scheme
	#[case("JavaScript", false)] // dangerous scheme (case-insensitive)
	#[case("data", false)] // dangerous scheme
	#[case("DATA", false)] // dangerous scheme (case-insensitive)
	#[case("vbscript", false)] // dangerous scheme
	#[case("file", false)] // dangerous scheme
	#[case("FILE", false)] // dangerous scheme (case-insensitive)
	fn test_validate_scheme_name(#[case] scheme: &str, #[case] expected_valid: bool) {
		// Arrange
		// (inputs provided by #[case])

		// Act
		let result = validate_scheme_name(scheme);

		// Assert
		assert_eq!(result.is_ok(), expected_valid, "scheme: {}", scheme);
	}

	#[rstest]
	fn test_validate_scheme_name_exceeds_max_length() {
		// Arrange
		let long_scheme = format!("a{}", "b".repeat(MAX_SCHEME_LENGTH));

		// Act
		let result = validate_scheme_name(&long_scheme);

		// Assert
		assert!(
			result.is_err(),
			"scheme exceeding max length should be rejected"
		);
	}

	// -- validate_fingerprint --

	#[rstest]
	#[case("com.example.app", true)]
	#[case("com.example.myapp", true)]
	#[case("org.company.product", true)]
	#[case("com.example.app_v2", true)]
	#[case("", false)] // empty
	#[case("nopackage", false)] // no dot
	#[case(".com.example", false)] // starts with dot (empty first segment)
	#[case("com.example.", false)] // ends with dot (empty last segment)
	#[case("123.invalid.name", false)] // segment starts with digit
	#[case("com.123.app", false)] // segment starts with digit
	#[case("com.exam ple.app", false)] // contains space
	#[case("com.exam-ple.app", false)] // contains hyphen
	fn test_validate_package_name(#[case] name: &str, #[case] expected_valid: bool) {
		let result = validate_package_name(name);
		assert_eq!(result.is_ok(), expected_valid, "package_name: {}", name);
	}

	#[rstest]
	#[case(
		"FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C",
		true
	)]
	#[case(
		"00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00",
		true
	)]
	#[case("invalid", false)]
	#[case("", false)]
	#[case("FA:C6:17", false)]
	#[case(
		"FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:XX",
		false
	)]
	fn test_validate_fingerprint(#[case] fingerprint: &str, #[case] expected_valid: bool) {
		// Arrange
		// (inputs provided by #[case])

		// Act
		let result = validate_fingerprint(fingerprint);

		// Assert
		assert_eq!(
			result.is_ok(),
			expected_valid,
			"fingerprint: {}",
			fingerprint
		);
	}
}
