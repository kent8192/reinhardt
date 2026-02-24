//! URL path validation logic for compile-time checks.

use std::collections::HashSet;

/// Errors that can occur during path validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PathValidationError {
	/// Path does not start with '/'
	MustStartWithSlash,
	/// Unmatched opening brace '{'
	UnmatchedOpenBrace(usize),
	/// Unmatched closing brace '}'
	UnmatchedCloseBrace(usize),
	/// Parameter name is empty
	EmptyParameterName(usize),
	/// Parameter name is not valid snake_case
	InvalidParameterName { name: String, position: usize },
	/// Double slash '//' found
	DoubleSlash(usize),
	/// Invalid character in path
	InvalidCharacter { ch: char, position: usize },
	/// Nested parameters (e.g., {{inner}})
	NestedParameters(usize),
	/// Consecutive parameters without separator (e.g., {id}{name})
	ConsecutiveParameters(usize),
	/// Wildcard not at end of path
	WildcardNotAtEnd(usize),
	/// Path traversal sequence '..' detected
	PathTraversal(usize),
	/// Duplicate parameter name detected
	DuplicateParameterName { name: String, position: usize },
}

/// Validates URL path syntax
pub(crate) fn validate_path_syntax(path: &str) -> Result<(), PathValidationError> {
	// Check if path starts with '/'
	if !path.starts_with('/') {
		return Err(PathValidationError::MustStartWithSlash);
	}

	// Reject path traversal sequences to prevent directory traversal attacks
	if let Some((i, _)) = path.match_indices("..").next() {
		return Err(PathValidationError::PathTraversal(i));
	}

	// Track parameter brace depth and position
	let mut brace_depth = 0;
	let mut param_start: Option<usize> = None;
	let mut prev_char: Option<char> = None;
	let mut seen_params: HashSet<String> = HashSet::new();

	for (i, ch) in path.char_indices() {
		// Check for double slashes
		if ch == '/' && prev_char == Some('/') {
			return Err(PathValidationError::DoubleSlash(i));
		}

		// Check for consecutive parameters without separator (e.g., {id}{name})
		if ch == '{' && prev_char == Some('}') {
			return Err(PathValidationError::ConsecutiveParameters(i));
		}

		match ch {
			'{' => {
				if brace_depth > 0 {
					return Err(PathValidationError::NestedParameters(i));
				}
				brace_depth += 1;
				param_start = Some(i);
			}
			'}' => {
				if brace_depth == 0 {
					return Err(PathValidationError::UnmatchedCloseBrace(i));
				}
				brace_depth -= 1;

				// Extract and validate parameter name
				if let Some(start) = param_start {
					let param_name = &path[start + 1..i];
					if param_name.is_empty() {
						return Err(PathValidationError::EmptyParameterName(start));
					}
					validate_parameter_name(param_name, start)?;

					// Check for duplicate parameter names
					if !seen_params.insert(param_name.to_string()) {
						return Err(PathValidationError::DuplicateParameterName {
							name: param_name.to_string(),
							position: start,
						});
					}
				}
				param_start = None;
			}
			c if brace_depth == 0 => {
				// Outside of parameter: validate path character
				validate_path_character(c, i)?;
			}
			_ => {
				// Inside parameter: will be validated when closing brace is found
			}
		}

		prev_char = Some(ch);
	}

	// Check for unclosed braces
	if brace_depth > 0
		&& let Some(start) = param_start
	{
		return Err(PathValidationError::UnmatchedOpenBrace(start));
	}

	// Validate wildcard position: '*' must only appear in the last segment
	validate_wildcard_position(path)?;

	Ok(())
}

/// Validates that wildcard '*' only appears in the last path segment
fn validate_wildcard_position(path: &str) -> Result<(), PathValidationError> {
	// Find all wildcard positions outside of parameter braces
	let mut brace_depth = 0;
	for (i, ch) in path.char_indices() {
		match ch {
			'{' => brace_depth += 1,
			'}' => brace_depth -= 1,
			'*' if brace_depth == 0 => {
				// Check that everything after '*' is empty or only '/'
				let rest = &path[i + 1..];
				if !rest.is_empty() && rest != "/" {
					return Err(PathValidationError::WildcardNotAtEnd(i));
				}
			}
			_ => {}
		}
	}
	Ok(())
}

/// Validates a single path character (outside of parameters)
fn validate_path_character(ch: char, position: usize) -> Result<(), PathValidationError> {
	match ch {
		'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '/' | '.' | '*' => Ok(()),
		_ => Err(PathValidationError::InvalidCharacter { ch, position }),
	}
}

/// Validates parameter name is valid snake_case identifier
fn validate_parameter_name(name: &str, position: usize) -> Result<(), PathValidationError> {
	if name.is_empty() {
		return Err(PathValidationError::EmptyParameterName(position));
	}

	let mut chars = name.chars();

	// First character must be lowercase letter or underscore
	if let Some(first) = chars.next()
		&& !matches!(first, 'a'..='z' | '_')
	{
		return Err(PathValidationError::InvalidParameterName {
			name: name.to_string(),
			position,
		});
	}

	// Remaining characters must be lowercase letter, digit, or underscore
	for ch in chars {
		if !matches!(ch, 'a'..='z' | '0'..='9' | '_') {
			return Err(PathValidationError::InvalidParameterName {
				name: name.to_string(),
				position,
			});
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_valid_simple_path() {
		// Arrange & Act & Assert
		assert!(validate_path_syntax("/users/").is_ok());
		assert!(validate_path_syntax("/items/").is_ok());
		assert!(validate_path_syntax("/api/v1/users/").is_ok());
	}

	#[rstest]
	fn test_valid_path_with_parameters() {
		// Arrange & Act & Assert
		assert!(validate_path_syntax("/users/{id}/").is_ok());
		assert!(validate_path_syntax("/users/{user_id}/").is_ok());
		assert!(validate_path_syntax("/users/{user_id}/posts/{post_id}/").is_ok());
		assert!(validate_path_syntax("/items/{item_id}/details/").is_ok());
	}

	#[rstest]
	fn test_must_start_with_slash() {
		// Arrange & Act
		let result = validate_path_syntax("users/");

		// Assert
		assert_eq!(result, Err(PathValidationError::MustStartWithSlash));
	}

	#[rstest]
	fn test_unmatched_open_brace() {
		// Arrange & Act
		let result = validate_path_syntax("/users/{id/");

		// Assert
		assert!(matches!(
			result,
			Err(PathValidationError::UnmatchedOpenBrace(_))
		));
	}

	#[rstest]
	fn test_unmatched_close_brace() {
		// Arrange & Act
		let result = validate_path_syntax("/users/id}/");

		// Assert
		assert!(matches!(
			result,
			Err(PathValidationError::UnmatchedCloseBrace(_))
		));
	}

	#[rstest]
	fn test_empty_parameter_name() {
		// Arrange & Act
		let result = validate_path_syntax("/users/{}/");

		// Assert
		assert!(matches!(
			result,
			Err(PathValidationError::EmptyParameterName(_))
		));
	}

	#[rstest]
	fn test_invalid_parameter_name_uppercase() {
		// Arrange & Act
		let result = validate_path_syntax("/users/{userId}/");

		// Assert
		assert!(matches!(
			result,
			Err(PathValidationError::InvalidParameterName { .. })
		));
	}

	#[rstest]
	fn test_invalid_parameter_name_hyphen() {
		// Arrange & Act
		let result = validate_path_syntax("/users/{user-id}/");

		// Assert
		assert!(matches!(
			result,
			Err(PathValidationError::InvalidParameterName { .. })
		));
	}

	#[rstest]
	fn test_double_slash() {
		// Arrange & Act
		let result = validate_path_syntax("/users//posts/");

		// Assert
		assert!(matches!(result, Err(PathValidationError::DoubleSlash(_))));
	}

	#[rstest]
	fn test_nested_parameters() {
		// Arrange & Act
		let result = validate_path_syntax("/users/{{id}}/");

		// Assert
		assert!(matches!(
			result,
			Err(PathValidationError::NestedParameters(_))
		));
	}

	#[rstest]
	fn test_valid_parameter_with_underscore() {
		// Arrange & Act & Assert
		assert!(validate_path_syntax("/users/{_id}/").is_ok());
		assert!(validate_path_syntax("/users/{_}/").is_ok());
		assert!(validate_path_syntax("/users/{user_id_123}/").is_ok());
	}

	#[rstest]
	fn test_valid_path_with_hyphens() {
		// Arrange & Act & Assert
		assert!(validate_path_syntax("/user-profiles/").is_ok());
		assert!(validate_path_syntax("/api-v1/user-data/").is_ok());
	}

	#[test]
	fn test_valid_path_with_single_dot() {
		// Single dots are valid in paths (e.g., file extensions, version numbers)
		assert!(validate_path_syntax("/files/document.pdf").is_ok());
		assert!(validate_path_syntax("/api/v1.0/users/").is_ok());
	}

	#[test]
	fn test_consecutive_parameters_without_separator() {
		// Consecutive parameters without any separator should be rejected
		let result = validate_path_syntax("/users/{id}{name}/");
		assert_eq!(result, Err(PathValidationError::ConsecutiveParameters(11)));

		let result = validate_path_syntax("/{a}{b}{c}/");
		assert_eq!(result, Err(PathValidationError::ConsecutiveParameters(4)));
	}

	#[test]
	fn test_consecutive_parameters_with_separator() {
		// Parameters separated by '/' are valid
		assert!(validate_path_syntax("/{year}/{month}/").is_ok());
		assert!(validate_path_syntax("/{a}/{b}/{c}/").is_ok());
	}

	#[test]
	fn test_wildcard_at_end() {
		// Wildcard at end of path is valid
		assert!(validate_path_syntax("/static/*").is_ok());
		assert!(validate_path_syntax("/files/*/").is_ok());
		assert!(validate_path_syntax("/users/{id}/*").is_ok());
	}

	#[test]
	fn test_wildcard_not_at_end() {
		// Wildcard not at end of path should be rejected
		let result = validate_path_syntax("/files/*/download");
		assert!(matches!(
			result,
			Err(PathValidationError::WildcardNotAtEnd(_))
		));

		let result = validate_path_syntax("/*/items/{item_id}/");
		assert!(matches!(
			result,
			Err(PathValidationError::WildcardNotAtEnd(_))
		));
	}

	#[test]
	fn test_path_traversal_rejected() {
		let result = validate_path_syntax("/users/../etc/passwd");
		assert!(matches!(result, Err(PathValidationError::PathTraversal(_))));

		let result = validate_path_syntax("/../../secret");
		assert!(matches!(result, Err(PathValidationError::PathTraversal(_))));

		let result = validate_path_syntax("/files/..hidden");
		assert!(matches!(result, Err(PathValidationError::PathTraversal(_))));
	}

	#[test]
	fn test_duplicate_parameter_name_rejected() {
		let result = validate_path_syntax("/users/{id}/posts/{id}/");
		assert!(matches!(
			result,
			Err(PathValidationError::DuplicateParameterName { .. })
		));

		let result = validate_path_syntax("/{name}/{name}/");
		assert!(matches!(
			result,
			Err(PathValidationError::DuplicateParameterName { .. })
		));
	}

	#[test]
	fn test_distinct_parameter_names_allowed() {
		assert!(validate_path_syntax("/users/{user_id}/posts/{post_id}/").is_ok());
		assert!(validate_path_syntax("/{a}/{b}/{c}/").is_ok());
	}
}
