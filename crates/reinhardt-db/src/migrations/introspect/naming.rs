//! Naming convention utilities for code generation.
//!
//! Provides functions for converting between different naming conventions
//! (snake_case, PascalCase) and handling Rust reserved keywords.

use std::collections::HashSet;
use std::sync::LazyLock;

/// Naming convention for generated identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NamingConvention {
	/// Convert to PascalCase (e.g., "user_profiles" -> "UserProfiles")
	#[default]
	PascalCase,
	/// Convert to snake_case (e.g., "UserProfiles" -> "user_profiles")
	SnakeCase,
	/// Preserve original naming
	Preserve,
}

/// Set of Rust reserved keywords that need escaping with `r#` prefix.
static RUST_KEYWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
	[
		// Strict keywords
		"as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
		"extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
		"mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
		"true", "type", "unsafe", "use", "where", "while",
		// Reserved keywords (may be used in future)
		"abstract", "become", "box", "do", "final", "macro", "override", "priv", "try", "typeof",
		"unsized", "virtual", "yield", // Weak keywords (context-sensitive)
		"union", "dyn",
	]
	.into_iter()
	.collect()
});

/// Escape a Rust keyword with `r#` prefix if necessary.
///
/// # Examples
///
/// ```rust
/// use reinhardt_db::migrations::introspect::escape_rust_keyword;
///
/// assert_eq!(escape_rust_keyword("type"), "r#type");
/// assert_eq!(escape_rust_keyword("users"), "users");
/// ```
pub fn escape_rust_keyword(name: &str) -> String {
	if RUST_KEYWORDS.contains(name) {
		format!("r#{}", name)
	} else {
		name.to_string()
	}
}

/// Check if a string is a Rust keyword.
// Allow dead_code: keyword check used during SQL-to-Rust identifier mapping in introspection
#[allow(dead_code)]
pub(super) fn is_rust_keyword(name: &str) -> bool {
	RUST_KEYWORDS.contains(name)
}

/// Sanitize an identifier to be a valid Rust identifier.
///
/// - Escapes Rust keywords
/// - Prefixes numeric-starting identifiers with underscore
/// - Replaces invalid characters
///
/// # Examples
///
/// ```rust
/// use reinhardt_db::migrations::introspect::sanitize_identifier;
///
/// assert_eq!(sanitize_identifier("type"), "r#type");
/// assert_eq!(sanitize_identifier("1column"), "_1column");
/// assert_eq!(sanitize_identifier("my-field"), "my_field");
/// ```
pub fn sanitize_identifier(name: &str) -> String {
	if name.is_empty() {
		return "_".to_string();
	}

	let mut result = String::with_capacity(name.len() + 2);

	// Handle numeric prefix
	let first_char = name.chars().next().unwrap();
	if first_char.is_ascii_digit() {
		result.push('_');
	}

	// Replace invalid characters
	for ch in name.chars() {
		if ch.is_ascii_alphanumeric() || ch == '_' {
			result.push(ch);
		} else if ch == '-' || ch == ' ' {
			result.push('_');
		}
		// Skip other invalid characters
	}

	// Escape keywords
	if RUST_KEYWORDS.contains(result.as_str()) {
		format!("r#{}", result)
	} else {
		result
	}
}

/// Convert a string to PascalCase.
///
/// Handles multiple separators: `_`, `.`, `-`, space
///
/// # Examples
///
/// ```rust
/// use reinhardt_db::migrations::introspect::to_pascal_case;
///
/// assert_eq!(to_pascal_case("user_profiles"), "UserProfiles");
/// assert_eq!(to_pascal_case("users"), "Users");
/// assert_eq!(to_pascal_case("USER_PROFILES"), "UserProfiles");
/// assert_eq!(to_pascal_case("userProfiles"), "UserProfiles");
/// assert_eq!(to_pascal_case("public.users"), "PublicUsers");
/// ```
pub fn to_pascal_case(s: &str) -> String {
	if s.is_empty() {
		return String::new();
	}

	let mut result = String::with_capacity(s.len());
	let mut capitalize_next = true;
	let mut prev_was_upper = false;

	for ch in s.chars() {
		if ch == '_' || ch == '-' || ch == ' ' || ch == '.' {
			capitalize_next = true;
			prev_was_upper = false;
		} else if ch.is_ascii_uppercase() {
			if prev_was_upper {
				// Handle consecutive uppercase (e.g., "URL" -> keep as is in sequence)
				result.push(ch.to_ascii_lowercase());
			} else {
				result.push(ch);
			}
			capitalize_next = false;
			prev_was_upper = true;
		} else if capitalize_next {
			result.push(ch.to_ascii_uppercase());
			capitalize_next = false;
			prev_was_upper = false;
		} else {
			result.push(ch.to_ascii_lowercase());
			prev_was_upper = false;
		}
	}

	result
}

/// Convert a string to snake_case.
///
/// # Examples
///
/// ```rust
/// use reinhardt_db::migrations::introspect::to_snake_case;
///
/// assert_eq!(to_snake_case("UserProfiles"), "user_profiles");
/// assert_eq!(to_snake_case("Users"), "users");
/// assert_eq!(to_snake_case("user_profiles"), "user_profiles");
/// assert_eq!(to_snake_case("HTTPRequest"), "http_request");
/// ```
pub fn to_snake_case(s: &str) -> String {
	if s.is_empty() {
		return String::new();
	}

	let mut result = String::with_capacity(s.len() + 4);
	let mut prev_was_upper = false;
	let mut prev_was_separator = true; // Treat start as separator to avoid leading underscore

	for (i, ch) in s.chars().enumerate() {
		if ch == '_' || ch == '-' || ch == ' ' {
			if !prev_was_separator && !result.is_empty() {
				result.push('_');
			}
			prev_was_separator = true;
			prev_was_upper = false;
		} else if ch.is_ascii_uppercase() {
			// Add underscore before uppercase if:
			// - Not at start
			// - Previous char was not a separator
			// - Previous char was not uppercase (handles "HTTPRequest" -> "http_request")
			if i > 0 && !prev_was_separator && !prev_was_upper {
				result.push('_');
			}
			result.push(ch.to_ascii_lowercase());
			prev_was_upper = true;
			prev_was_separator = false;
		} else {
			// If previous was uppercase and current is lowercase, we might need to insert underscore
			// For "HTTPRequest": H-T-T-P-R-e-q-u-e-s-t
			// When we hit 'e' after 'R', we need to back up and insert underscore before 'R'
			if prev_was_upper && !result.is_empty() {
				let len = result.len();
				if len > 1 {
					// Check if we need to insert underscore before last uppercase
					let last = result.pop().unwrap();
					if !result.ends_with('_') && !result.is_empty() {
						result.push('_');
					}
					result.push(last);
				}
			}
			result.push(ch.to_ascii_lowercase());
			prev_was_upper = false;
			prev_was_separator = false;
		}
	}

	result
}

/// Convert a table name to a struct name based on naming convention.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::introspect::naming::{table_to_struct_name, NamingConvention};
///
/// assert_eq!(table_to_struct_name("users", NamingConvention::PascalCase), "Users");
/// assert_eq!(table_to_struct_name("user_profiles", NamingConvention::PascalCase), "UserProfiles");
/// ```
pub(super) fn table_to_struct_name(table_name: &str, convention: NamingConvention) -> String {
	match convention {
		NamingConvention::PascalCase => to_pascal_case(table_name),
		NamingConvention::SnakeCase => to_snake_case(table_name),
		NamingConvention::Preserve => table_name.to_string(),
	}
}

/// Convert a column name to a field name based on naming convention.
///
/// Also handles keyword escaping.
pub(super) fn column_to_field_name(column_name: &str, convention: NamingConvention) -> String {
	let name = match convention {
		NamingConvention::PascalCase => to_pascal_case(column_name),
		NamingConvention::SnakeCase => to_snake_case(column_name),
		NamingConvention::Preserve => column_name.to_string(),
	};

	sanitize_identifier(&name)
}

/// Check if a string is a valid Rust identifier.
// Allow dead_code: identifier validation used during SQL column name to Rust field name conversion
#[allow(dead_code)]
pub(super) fn is_valid_rust_identifier(s: &str) -> bool {
	if s.is_empty() {
		return false;
	}

	let mut chars = s.chars();

	// First character must be letter or underscore
	match chars.next() {
		Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
		_ => return false,
	}

	// Rest must be alphanumeric or underscore
	chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_escape_rust_keyword() {
		assert_eq!(escape_rust_keyword("type"), "r#type");
		assert_eq!(escape_rust_keyword("struct"), "r#struct");
		assert_eq!(escape_rust_keyword("impl"), "r#impl");
		assert_eq!(escape_rust_keyword("users"), "users");
		assert_eq!(escape_rust_keyword("id"), "id");
	}

	#[test]
	fn test_to_pascal_case() {
		assert_eq!(to_pascal_case("user_profiles"), "UserProfiles");
		assert_eq!(to_pascal_case("users"), "Users");
		assert_eq!(to_pascal_case("USER_PROFILES"), "UserProfiles");
		assert_eq!(to_pascal_case("my_table_name"), "MyTableName");
		assert_eq!(to_pascal_case(""), "");
	}

	#[test]
	fn test_to_snake_case() {
		assert_eq!(to_snake_case("UserProfiles"), "user_profiles");
		assert_eq!(to_snake_case("Users"), "users");
		assert_eq!(to_snake_case("user_profiles"), "user_profiles");
		assert_eq!(to_snake_case("MyTableName"), "my_table_name");
		assert_eq!(to_snake_case(""), "");
	}

	#[test]
	fn test_sanitize_identifier() {
		assert_eq!(sanitize_identifier("type"), "r#type");
		assert_eq!(sanitize_identifier("1column"), "_1column");
		assert_eq!(sanitize_identifier("my-field"), "my_field");
		assert_eq!(sanitize_identifier("valid_name"), "valid_name");
		assert_eq!(sanitize_identifier(""), "_");
	}

	#[test]
	fn test_is_valid_rust_identifier() {
		assert!(is_valid_rust_identifier("valid_name"));
		assert!(is_valid_rust_identifier("_private"));
		assert!(is_valid_rust_identifier("name123"));
		assert!(!is_valid_rust_identifier(""));
		assert!(!is_valid_rust_identifier("123start"));
		assert!(!is_valid_rust_identifier("has-dash"));
	}
}
