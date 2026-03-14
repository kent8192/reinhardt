use reinhardt::Validate;
use serde::{Deserialize, Serialize};

/// Serializer for creating/updating snippets
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SnippetSerializer {
	#[validate(length(
		min = 1,
		max = 100,
		message = "Title must be between 1 and 100 characters"
	))]
	pub title: String,

	#[validate(length(
		min = 1,
		max = 10000,
		message = "Code must be between 1 and 10000 characters"
	))]
	pub code: String,

	#[validate(length(
		min = 1,
		max = 50,
		message = "Language must be between 1 and 50 characters"
	))]
	pub language: String,
}

/// Response serializer for snippets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetResponse {
	pub id: i64,
	pub title: String,
	pub code: String,
	pub language: String,
	pub highlighted: String,
}

impl SnippetResponse {
	pub fn from_model(snippet: &super::models::Snippet) -> Self {
		Self {
			id: snippet.id,
			title: snippet.title.clone(),
			code: snippet.code.clone(),
			language: snippet.language.clone(),
			highlighted: snippet.highlighted(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_snippet_serializer_validation() {
		// Arrange
		let valid = SnippetSerializer {
			title: "Valid".to_string(),
			code: "fn main() {}".to_string(),
			language: "rust".to_string(),
		};
		let invalid_title = SnippetSerializer {
			title: "".to_string(),
			code: "fn main() {}".to_string(),
			language: "rust".to_string(),
		};
		let invalid_code = SnippetSerializer {
			title: "Valid".to_string(),
			code: "".to_string(),
			language: "rust".to_string(),
		};
		let invalid_long = SnippetSerializer {
			title: "x".repeat(101),
			code: "fn main() {}".to_string(),
			language: "rust".to_string(),
		};

		// Act & Assert
		assert!(valid.validate().is_ok());
		assert!(invalid_title.validate().is_err());
		assert!(invalid_code.validate().is_err());
		assert!(invalid_long.validate().is_err());
	}

	#[rstest]
	fn test_snippet_serializer_code_max_length_boundary() {
		// Arrange
		let at_limit = SnippetSerializer {
			title: "Valid".to_string(),
			code: "x".repeat(10000),
			language: "rust".to_string(),
		};

		// Act
		let result = at_limit.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_snippet_serializer_code_exceeds_max_length() {
		// Arrange
		let over_limit = SnippetSerializer {
			title: "Valid".to_string(),
			code: "x".repeat(10001),
			language: "rust".to_string(),
		};

		// Act
		let result = over_limit.validate();

		// Assert
		assert!(result.is_err());
	}
}
