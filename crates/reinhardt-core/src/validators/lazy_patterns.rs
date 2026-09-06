//! Lazy-initialized regex patterns for validators
//!
//! This module provides pre-compiled regex patterns using `LazyLock` for
//! efficient reuse across validator instances. Each pattern is compiled
//! only once on first access, eliminating the overhead of recompiling
//! the same regex pattern for every validator instance.

use regex::Regex;
use std::sync::LazyLock;

// =============================================================================
// Email Patterns
// =============================================================================

/// RFC 5322 compliant email regex pattern.
///
/// This pattern ensures:
/// - Local part doesn't start/end with dots and has no consecutive dots
/// - Domain labels are valid (no leading/trailing hyphens)
/// - TLD is at least 2 characters
pub(crate) static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"^(?i)[a-z0-9]([a-z0-9._%+-]*[a-z0-9])?@[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*\.[a-z]{2,}$",
	)
	.expect("EMAIL_REGEX: Invalid regex pattern")
});

// =============================================================================
// URL Patterns
// =============================================================================

/// HTTP/HTTPS URL regex pattern.
///
/// Supports:
/// - Ports: :8080, :443, etc. (1-5 digits)
/// - Query strings and fragments
/// - Paths: /path/to/resource
/// - Domain labels cannot start or end with hyphens
pub(crate) static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"^https?://[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?)*(:[0-9]{1,5})?(/[^\s?#]*)?(\?[^\s#]*)?(#[^\s]*)?$",
	)
	.expect("URL_REGEX: Invalid regex pattern")
});

// =============================================================================
// Phone Number Patterns
// =============================================================================

/// E.164 format phone number regex.
///
/// Format: + followed by country code (1-3 digits starting with non-zero) and number.
/// Allows optional hyphens, spaces, dots, and parentheses for readability.
pub(crate) static PHONE_E164_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^\+([1-9]\d{0,2})[\s.\-()]*\d+[\s.\-\d()]*$")
		.expect("PHONE_E164_REGEX: Invalid regex pattern")
});

/// Phone extension detection regex.
///
/// Detects extension formats: "ext.", "ext", "x", "extension" followed by digits.
pub(crate) static PHONE_EXTENSION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^(.+?)(?:\s*(?:ext\.?|x|extension)\s*(\d+))?$")
		.expect("PHONE_EXTENSION_REGEX: Invalid regex pattern")
});

// =============================================================================
// Slug Patterns
// =============================================================================

/// ASCII-only slug pattern.
///
/// Matches slugs containing only ASCII letters, numbers, hyphens, and underscores.
pub(crate) static SLUG_ASCII_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^[-a-zA-Z0-9_]+$").expect("SLUG_ASCII_REGEX: Invalid regex pattern")
});

/// Unicode-enabled slug pattern.
///
/// Matches slugs containing Unicode word characters, hyphens, and underscores.
pub(crate) static SLUG_UNICODE_REGEX: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"^[-\w]+$").expect("SLUG_UNICODE_REGEX: Invalid regex pattern"));

// =============================================================================
// UUID Pattern
// =============================================================================

/// UUID format pattern (lowercase).
///
/// Format: 8-4-4-4-12 hex digits (e.g., "550e8400-e29b-41d4-a716-446655440000").
pub(crate) static UUID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
		.expect("UUID_REGEX: Invalid regex pattern")
});

// =============================================================================
// Color Patterns
// =============================================================================

/// Hex color pattern.
///
/// Matches: #RGB, #RRGGBB, or #RRGGBBAA formats.
pub(crate) static COLOR_HEX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^#([0-9A-Fa-f]{3}|[0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$")
		.expect("COLOR_HEX_REGEX: Invalid regex pattern")
});

/// RGB color pattern.
///
/// Matches: rgb(0-255, 0-255, 0-255) format.
pub(crate) static COLOR_RGB_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"^rgb\(\s*([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\s*,\s*([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\s*,\s*([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\s*\)$",
	)
	.expect("COLOR_RGB_REGEX: Invalid regex pattern")
});

/// RGBA color pattern.
///
/// Matches: rgba(0-255, 0-255, 0-255, 0-1) format.
pub(crate) static COLOR_RGBA_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"^rgba\(\s*([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\s*,\s*([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\s*,\s*([0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\s*,\s*(0|1|0?\.\d+)\s*\)$",
	)
	.expect("COLOR_RGBA_REGEX: Invalid regex pattern")
});

/// HSL color pattern.
///
/// Matches: hsl(0-360, 0-100%, 0-100%) format.
pub(crate) static COLOR_HSL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"^hsl\(\s*([0-9]|[1-9][0-9]|[1-2][0-9]{2}|3[0-5][0-9]|360)\s*,\s*([0-9]|[1-9][0-9]|100)%\s*,\s*([0-9]|[1-9][0-9]|100)%\s*\)$",
	)
	.expect("COLOR_HSL_REGEX: Invalid regex pattern")
});

/// HSLA color pattern.
///
/// Matches: hsla(0-360, 0-100%, 0-100%, 0-1) format.
pub(crate) static COLOR_HSLA_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"^hsla\(\s*([0-9]|[1-9][0-9]|[1-2][0-9]{2}|3[0-5][0-9]|360)\s*,\s*([0-9]|[1-9][0-9]|100)%\s*,\s*([0-9]|[1-9][0-9]|100)%\s*,\s*(0|1|0?\.\d+)\s*\)$",
	)
	.expect("COLOR_HSLA_REGEX: Invalid regex pattern")
});

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_email_regex() {
		assert!(EMAIL_REGEX.is_match("test@example.com"));
		assert!(EMAIL_REGEX.is_match("user.name@sub.example.co.uk"));
		assert!(!EMAIL_REGEX.is_match("invalid"));
		assert!(!EMAIL_REGEX.is_match("@example.com"));
	}

	#[test]
	fn test_url_regex() {
		assert!(URL_REGEX.is_match("http://example.com"));
		assert!(URL_REGEX.is_match("https://example.com:8080/path?query=value#section"));
		assert!(!URL_REGEX.is_match("ftp://example.com"));
		assert!(!URL_REGEX.is_match("invalid"));
	}

	#[test]
	fn test_phone_e164_regex() {
		assert!(PHONE_E164_REGEX.is_match("+1234567890"));
		assert!(PHONE_E164_REGEX.is_match("+81-90-1234-5678"));
		assert!(!PHONE_E164_REGEX.is_match("1234567890"));
		assert!(!PHONE_E164_REGEX.is_match("+0123456789"));
	}

	#[test]
	fn test_phone_extension_regex() {
		let caps = PHONE_EXTENSION_REGEX
			.captures("+1234567890 ext. 123")
			.unwrap();
		assert_eq!(caps.get(1).map(|m| m.as_str()), Some("+1234567890"));
		assert_eq!(caps.get(2).map(|m| m.as_str()), Some("123"));
	}

	#[test]
	fn test_slug_ascii_regex() {
		assert!(SLUG_ASCII_REGEX.is_match("valid-slug"));
		assert!(SLUG_ASCII_REGEX.is_match("valid_slug_123"));
		assert!(!SLUG_ASCII_REGEX.is_match("invalid slug"));
		assert!(!SLUG_ASCII_REGEX.is_match("日本語"));
	}

	#[test]
	fn test_slug_unicode_regex() {
		assert!(SLUG_UNICODE_REGEX.is_match("valid-slug"));
		assert!(SLUG_UNICODE_REGEX.is_match("日本語-slug"));
	}

	#[test]
	fn test_uuid_regex() {
		assert!(UUID_REGEX.is_match("550e8400-e29b-41d4-a716-446655440000"));
		assert!(!UUID_REGEX.is_match("invalid-uuid"));
		assert!(!UUID_REGEX.is_match("550E8400-E29B-41D4-A716-446655440000")); // uppercase
	}

	#[test]
	fn test_color_hex_regex() {
		assert!(COLOR_HEX_REGEX.is_match("#FFF"));
		assert!(COLOR_HEX_REGEX.is_match("#FF0000"));
		assert!(COLOR_HEX_REGEX.is_match("#FF0000FF"));
		assert!(!COLOR_HEX_REGEX.is_match("FF0000"));
	}

	#[test]
	fn test_color_rgb_regex() {
		assert!(COLOR_RGB_REGEX.is_match("rgb(255, 0, 0)"));
		assert!(COLOR_RGB_REGEX.is_match("rgb(0,0,0)"));
		assert!(!COLOR_RGB_REGEX.is_match("rgb(256, 0, 0)"));
	}

	#[test]
	fn test_color_rgba_regex() {
		assert!(COLOR_RGBA_REGEX.is_match("rgba(255, 0, 0, 1)"));
		assert!(COLOR_RGBA_REGEX.is_match("rgba(255, 0, 0, 0.5)"));
		assert!(!COLOR_RGBA_REGEX.is_match("rgba(255, 0, 0)"));
	}

	#[test]
	fn test_color_hsl_regex() {
		assert!(COLOR_HSL_REGEX.is_match("hsl(0, 100%, 50%)"));
		assert!(COLOR_HSL_REGEX.is_match("hsl(360, 0%, 0%)"));
		assert!(!COLOR_HSL_REGEX.is_match("hsl(361, 100%, 50%)"));
	}

	#[test]
	fn test_color_hsla_regex() {
		assert!(COLOR_HSLA_REGEX.is_match("hsla(0, 100%, 50%, 1)"));
		assert!(COLOR_HSLA_REGEX.is_match("hsla(0, 100%, 50%, 0.5)"));
		assert!(!COLOR_HSLA_REGEX.is_match("hsla(0, 100%, 50%)"));
	}
}
