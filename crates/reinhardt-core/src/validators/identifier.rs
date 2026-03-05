//! Database identifier validation types.
//!
//! Provides type-safe wrappers for table names, field names, and constraint names
//! with compile-time and runtime validation.

use super::reserved::is_sql_reserved_word;

/// Maximum length for database identifiers (PostgreSQL standard)
const MAX_IDENTIFIER_LENGTH: usize = 63;

/// Validation errors for database identifiers
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentifierValidationError {
	#[error("Identifier cannot be empty")]
	Empty,

	#[error("Identifier too long: {0} characters (max {MAX_IDENTIFIER_LENGTH})")]
	TooLong(usize),

	#[error("Identifier must be snake_case: '{0}'")]
	NotSnakeCase(String),

	#[error("Identifier contains invalid character: '{0}'")]
	InvalidCharacter(char),

	#[error("Identifier is a SQL reserved word: '{0}'")]
	ReservedWord(String),

	#[error("Identifier must start with lowercase letter or underscore: '{0}'")]
	InvalidFirstCharacter(String),
}

/// Type-safe table name with validation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableName(&'static str);

impl TableName {
	/// Creates a new table name with compile-time validation.
	///
	/// # Panics
	///
	/// Panics at compile time if the name is invalid.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::TableName;
	///
	/// const USER_TABLE: TableName = TableName::new_const("user");
	/// const PROFILE_TABLE: TableName = TableName::new_const("user_profile");
	/// ```
	pub const fn new_const(name: &'static str) -> Self {
		assert!(!name.is_empty(), "Table name cannot be empty");
		assert!(name.len() <= MAX_IDENTIFIER_LENGTH, "Table name too long");
		assert!(
			Self::is_valid_snake_case(name),
			"Table name must be snake_case"
		);
		// Note: Reserved word check is only performed at runtime
		// due to const fn limitations
		Self(name)
	}

	/// Creates a new table name with runtime validation.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::TableName;
	///
	/// let table = TableName::new("user").unwrap();
	/// let profile = TableName::new("user_profile").unwrap();
	///
	/// assert!(TableName::new("User").is_err());
	/// assert!(TableName::new("select").is_err());
	/// ```
	pub fn new(name: &'static str) -> Result<Self, IdentifierValidationError> {
		if name.is_empty() {
			return Err(IdentifierValidationError::Empty);
		}

		if name.len() > MAX_IDENTIFIER_LENGTH {
			return Err(IdentifierValidationError::TooLong(name.len()));
		}

		if !Self::is_valid_identifier(name)? {
			return Err(IdentifierValidationError::NotSnakeCase(name.to_string()));
		}

		if is_sql_reserved_word(name) {
			return Err(IdentifierValidationError::ReservedWord(name.to_string()));
		}

		Ok(Self(name))
	}

	/// Returns the table name as a string slice.
	pub const fn as_str(&self) -> &'static str {
		self.0
	}

	/// Validates snake_case format at compile time.
	const fn is_valid_snake_case(s: &str) -> bool {
		let bytes = s.as_bytes();
		if bytes.is_empty() {
			return false;
		}

		// First character must be lowercase or underscore
		let first = bytes[0];
		if !matches!(first, b'a'..=b'z' | b'_') {
			return false;
		}

		// Remaining characters must be lowercase, digit, or underscore
		let mut i = 1;
		while i < bytes.len() {
			let b = bytes[i];
			if !matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_') {
				return false;
			}
			i += 1;
		}

		true
	}

	/// Validates identifier format at runtime.
	fn is_valid_identifier(s: &str) -> Result<bool, IdentifierValidationError> {
		let mut chars = s.chars();

		// First character
		if let Some(first) = chars.next() {
			if !matches!(first, 'a'..='z' | '_') {
				return Err(IdentifierValidationError::InvalidFirstCharacter(
					s.to_string(),
				));
			}
		} else {
			return Ok(false);
		}

		// Remaining characters
		for ch in chars {
			if !matches!(ch, 'a'..='z' | '0'..='9' | '_') {
				return Err(IdentifierValidationError::InvalidCharacter(ch));
			}
		}

		Ok(true)
	}
}

impl AsRef<str> for TableName {
	fn as_ref(&self) -> &str {
		self.0
	}
}

impl std::fmt::Display for TableName {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Type-safe field name with validation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldName(&'static str);

impl FieldName {
	/// Creates a new field name with compile-time validation.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FieldName;
	///
	/// const ID_FIELD: FieldName = FieldName::new_const("id");
	/// const USERNAME_FIELD: FieldName = FieldName::new_const("username");
	/// const EMAIL_FIELD: FieldName = FieldName::new_const("email_address");
	/// ```
	pub const fn new_const(name: &'static str) -> Self {
		assert!(!name.is_empty(), "Field name cannot be empty");
		assert!(name.len() <= MAX_IDENTIFIER_LENGTH, "Field name too long");
		assert!(
			Self::is_valid_snake_case(name),
			"Field name must be snake_case"
		);
		Self(name)
	}

	/// Creates a new field name with runtime validation.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::FieldName;
	///
	/// let field = FieldName::new("username").unwrap();
	/// let email = FieldName::new("email_address").unwrap();
	///
	/// assert!(FieldName::new("userName").is_err());
	/// ```
	pub fn new(name: &'static str) -> Result<Self, IdentifierValidationError> {
		if name.is_empty() {
			return Err(IdentifierValidationError::Empty);
		}

		if name.len() > MAX_IDENTIFIER_LENGTH {
			return Err(IdentifierValidationError::TooLong(name.len()));
		}

		if !Self::is_valid_identifier(name)? {
			return Err(IdentifierValidationError::NotSnakeCase(name.to_string()));
		}

		Ok(Self(name))
	}

	/// Returns the field name as a string slice.
	pub const fn as_str(&self) -> &'static str {
		self.0
	}

	const fn is_valid_snake_case(s: &str) -> bool {
		let bytes = s.as_bytes();
		if bytes.is_empty() {
			return false;
		}

		let first = bytes[0];
		if !matches!(first, b'a'..=b'z' | b'_') {
			return false;
		}

		let mut i = 1;
		while i < bytes.len() {
			let b = bytes[i];
			if !matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_') {
				return false;
			}
			i += 1;
		}

		true
	}

	fn is_valid_identifier(s: &str) -> Result<bool, IdentifierValidationError> {
		let mut chars = s.chars();

		if let Some(first) = chars.next() {
			if !matches!(first, 'a'..='z' | '_') {
				return Err(IdentifierValidationError::InvalidFirstCharacter(
					s.to_string(),
				));
			}
		} else {
			return Ok(false);
		}

		for ch in chars {
			if !matches!(ch, 'a'..='z' | '0'..='9' | '_') {
				return Err(IdentifierValidationError::InvalidCharacter(ch));
			}
		}

		Ok(true)
	}
}

impl AsRef<str> for FieldName {
	fn as_ref(&self) -> &str {
		self.0
	}
}

impl std::fmt::Display for FieldName {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Type-safe constraint name with validation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstraintName(&'static str);

impl ConstraintName {
	/// Creates a new constraint name with compile-time validation.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::ConstraintName;
	///
	/// const PK: ConstraintName = ConstraintName::new_const("pk_user");
	/// const FK: ConstraintName = ConstraintName::new_const("fk_user_profile");
	/// const UQ: ConstraintName = ConstraintName::new_const("uq_username");
	/// ```
	pub const fn new_const(name: &'static str) -> Self {
		assert!(!name.is_empty(), "Constraint name cannot be empty");
		assert!(
			name.len() <= MAX_IDENTIFIER_LENGTH,
			"Constraint name too long"
		);
		assert!(
			Self::is_valid_snake_case(name),
			"Constraint name must be snake_case"
		);
		Self(name)
	}

	/// Creates a new constraint name with runtime validation.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::validators::ConstraintName;
	///
	/// let pk = ConstraintName::new("pk_user").unwrap();
	/// let fk = ConstraintName::new("fk_user_profile").unwrap();
	///
	/// assert!(ConstraintName::new("PK_User").is_err());
	/// ```
	pub fn new(name: &'static str) -> Result<Self, IdentifierValidationError> {
		if name.is_empty() {
			return Err(IdentifierValidationError::Empty);
		}

		if name.len() > MAX_IDENTIFIER_LENGTH {
			return Err(IdentifierValidationError::TooLong(name.len()));
		}

		if !Self::is_valid_identifier(name)? {
			return Err(IdentifierValidationError::NotSnakeCase(name.to_string()));
		}

		Ok(Self(name))
	}

	/// Returns the constraint name as a string slice.
	pub const fn as_str(&self) -> &'static str {
		self.0
	}

	const fn is_valid_snake_case(s: &str) -> bool {
		let bytes = s.as_bytes();
		if bytes.is_empty() {
			return false;
		}

		let first = bytes[0];
		if !matches!(first, b'a'..=b'z' | b'_') {
			return false;
		}

		let mut i = 1;
		while i < bytes.len() {
			let b = bytes[i];
			if !matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_') {
				return false;
			}
			i += 1;
		}

		true
	}

	fn is_valid_identifier(s: &str) -> Result<bool, IdentifierValidationError> {
		let mut chars = s.chars();

		if let Some(first) = chars.next() {
			if !matches!(first, 'a'..='z' | '_') {
				return Err(IdentifierValidationError::InvalidFirstCharacter(
					s.to_string(),
				));
			}
		} else {
			return Ok(false);
		}

		for ch in chars {
			if !matches!(ch, 'a'..='z' | '0'..='9' | '_') {
				return Err(IdentifierValidationError::InvalidCharacter(ch));
			}
		}

		Ok(true)
	}
}

impl AsRef<str> for ConstraintName {
	fn as_ref(&self) -> &str {
		self.0
	}
}

impl std::fmt::Display for ConstraintName {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// TableName tests
	#[test]
	fn test_table_name_valid() {
		assert!(TableName::new("user").is_ok());
		assert!(TableName::new("user_profile").is_ok());
		assert!(TableName::new("_internal").is_ok());
		assert!(TableName::new("table_123").is_ok());
	}

	#[test]
	fn test_table_name_invalid_uppercase() {
		assert!(matches!(
			TableName::new("User"),
			Err(IdentifierValidationError::InvalidFirstCharacter(_))
		));
		assert!(matches!(
			TableName::new("user_Profile"),
			Err(IdentifierValidationError::InvalidCharacter(_))
		));
	}

	#[test]
	fn test_table_name_reserved_word() {
		assert!(matches!(
			TableName::new("select"),
			Err(IdentifierValidationError::ReservedWord(_))
		));
		assert!(matches!(
			TableName::new("table"),
			Err(IdentifierValidationError::ReservedWord(_))
		));
	}

	#[test]
	fn test_table_name_empty() {
		assert!(matches!(
			TableName::new(""),
			Err(IdentifierValidationError::Empty)
		));
	}

	// FieldName tests
	#[test]
	fn test_field_name_valid() {
		assert!(FieldName::new("id").is_ok());
		assert!(FieldName::new("username").is_ok());
		assert!(FieldName::new("email_address").is_ok());
		assert!(FieldName::new("_private").is_ok());
	}

	#[test]
	fn test_field_name_invalid() {
		assert!(matches!(
			FieldName::new("userName"),
			Err(IdentifierValidationError::InvalidCharacter(_))
		));
		assert!(matches!(
			FieldName::new("User"),
			Err(IdentifierValidationError::InvalidFirstCharacter(_))
		));
	}

	// ConstraintName tests
	#[test]
	fn test_constraint_name_valid() {
		assert!(ConstraintName::new("pk_user").is_ok());
		assert!(ConstraintName::new("fk_user_profile").is_ok());
		assert!(ConstraintName::new("uq_username").is_ok());
	}

	#[test]
	fn test_constraint_name_invalid() {
		assert!(matches!(
			ConstraintName::new("PK_User"),
			Err(IdentifierValidationError::InvalidFirstCharacter(_))
		));
	}

	// Const tests
	#[test]
	fn test_const_creation() {
		const TABLE: TableName = TableName::new_const("user");
		const FIELD: FieldName = FieldName::new_const("username");
		const CONSTRAINT: ConstraintName = ConstraintName::new_const("pk_user");

		assert_eq!(TABLE.as_str(), "user");
		assert_eq!(FIELD.as_str(), "username");
		assert_eq!(CONSTRAINT.as_str(), "pk_user");
	}
}
