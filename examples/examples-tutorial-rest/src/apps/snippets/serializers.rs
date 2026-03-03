use serde::{Deserialize, Serialize};
use validator::Validate;

/// Serializer for creating/updating snippets
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SnippetSerializer {
	#[validate(length(
		min = 1,
		max = 100,
		message = "Title must be between 1 and 100 characters"
	))]
	pub title: String,

	#[validate(length(min = 1, message = "Code cannot be empty"))]
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
		// Valid snippet
		let valid = SnippetSerializer {
			title: "Valid".to_string(),
			code: "fn main() {}".to_string(),
			language: "rust".to_string(),
		};
		assert!(valid.validate().is_ok());

		// Invalid: empty title
		let invalid_title = SnippetSerializer {
			title: "".to_string(),
			code: "fn main() {}".to_string(),
			language: "rust".to_string(),
		};
		assert!(invalid_title.validate().is_err());

		// Invalid: empty code
		let invalid_code = SnippetSerializer {
			title: "Valid".to_string(),
			code: "".to_string(),
			language: "rust".to_string(),
		};
		assert!(invalid_code.validate().is_err());

		// Invalid: title too long
		let long_title = "x".repeat(101);
		let invalid_long = SnippetSerializer {
			title: long_title,
			code: "fn main() {}".to_string(),
			language: "rust".to_string(),
		};
		assert!(invalid_long.validate().is_err());
	}
}
