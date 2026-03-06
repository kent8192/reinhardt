use thiserror::Error;

/// Errors that can occur in the taggit system
#[derive(Debug, Error)]
pub enum TaggitError {
	/// Tag name is invalid
	#[error("Tag name is invalid: {0}")]
	InvalidTagName(String),

	/// Tag name exceeds maximum length
	#[error("Tag name too long: maximum {max} characters, got {len}")]
	TagNameTooLong { max: usize, len: usize },

	/// Tag name contains invalid characters
	#[error("Tag name contains invalid characters: {0}")]
	InvalidCharacters(String),

	/// Tag not found
	#[error("Tag not found: {0}")]
	TagNotFound(String),

	/// Tagged item not found
	#[error("Tagged item not found: {content_type}#{object_id} with tag '{tag_name}'")]
	TaggedItemNotFound {
		content_type: String,
		object_id: i64,
		tag_name: String,
	},

	/// Object not found
	#[error("Object not found: {content_type}#{object_id}")]
	ObjectNotFound {
		content_type: String,
		object_id: i64,
	},

	/// Duplicate tag assignment
	#[error("Duplicate tag assignment: {content_type}#{object_id} already has tag '{tag_name}'")]
	DuplicateTag {
		content_type: String,
		object_id: i64,
		tag_name: String,
	},

	/// Database error from reinhardt-db
	#[error("Database error: {0}")]
	DatabaseError(String),

	/// Transaction error
	#[error("Transaction error: {0}")]
	TransactionError(String),

	/// Configuration error
	#[error("Configuration error: {0}")]
	ConfigError(String),
}

/// Result type for taggit operations
pub type Result<T> = std::result::Result<T, TaggitError>;
