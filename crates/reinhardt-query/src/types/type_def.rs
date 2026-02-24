//! Type definition types for DDL operations.
//!
//! This module provides types for custom type-related DDL operations:
//!
//! - [`TypeKind`]: Kind of custom type (ENUM, COMPOSITE, DOMAIN, RANGE)
//! - [`TypeDef`]: Type definition for CREATE TYPE
//! - [`TypeOperation`]: Operations for ALTER TYPE
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_query::types::type_def::{TypeKind, TypeDef};
//!
//! // CREATE TYPE mood AS ENUM ('happy', 'sad', 'neutral')
//! let type_def = TypeDef::new("mood")
//!     .kind(TypeKind::Enum {
//!         values: vec!["happy".to_string(), "sad".to_string(), "neutral".to_string()],
//!     });
//!
//! // CREATE TYPE address AS (street text, city text, zip integer)
//! let type_def = TypeDef::new("address")
//!     .kind(TypeKind::Composite {
//!         attributes: vec![
//!             ("street".to_string(), "text".to_string()),
//!             ("city".to_string(), "text".to_string()),
//!             ("zip".to_string(), "integer".to_string()),
//!         ],
//!     });
//! ```

use crate::types::{DynIden, IntoIden};

/// Kind of custom type.
///
/// This enum represents the different kinds of custom types that can be created
/// in PostgreSQL and CockroachDB.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::type_def::TypeKind;
///
/// // ENUM type
/// let enum_type = TypeKind::Enum {
///     values: vec!["red".to_string(), "green".to_string(), "blue".to_string()],
/// };
///
/// // COMPOSITE type
/// let composite_type = TypeKind::Composite {
///     attributes: vec![
///         ("x".to_string(), "integer".to_string()),
///         ("y".to_string(), "integer".to_string()),
///     ],
/// };
///
/// // DOMAIN type
/// let domain_type = TypeKind::Domain {
///     base_type: "text".to_string(),
///     constraint: Some("CHECK (VALUE ~ '^[A-Z]')".to_string()),
///     default: Some("'UNKNOWN'".to_string()),
///     not_null: true,
/// };
///
/// // RANGE type
/// let range_type = TypeKind::Range {
///     subtype: "integer".to_string(),
///     subtype_diff: None,
///     canonical: None,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
	/// ENUM type with list of values.
	Enum {
		/// List of enum values.
		values: Vec<String>,
	},
	/// COMPOSITE type with list of attributes (name, type).
	Composite {
		/// List of attributes as (name, type) pairs.
		attributes: Vec<(String, String)>,
	},
	/// DOMAIN type with base type and constraints.
	Domain {
		/// Base type for the domain.
		base_type: String,
		/// Optional constraint expression.
		constraint: Option<String>,
		/// Optional default value.
		default: Option<String>,
		/// Whether the domain is NOT NULL.
		not_null: bool,
	},
	/// RANGE type with subtype.
	Range {
		/// Subtype for the range.
		subtype: String,
		/// Optional subtype diff function.
		subtype_diff: Option<String>,
		/// Optional canonical function.
		canonical: Option<String>,
	},
}

/// Type definition for CREATE TYPE.
///
/// This struct represents a type definition, including its name and kind.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::type_def::{TypeDef, TypeKind};
///
/// // CREATE TYPE mood AS ENUM ('happy', 'sad')
/// let type_def = TypeDef::new("mood")
///     .kind(TypeKind::Enum {
///         values: vec!["happy".to_string(), "sad".to_string()],
///     });
/// ```
#[derive(Debug, Clone)]
pub struct TypeDef {
	// Fields are accessed by query builders (CreateTypeStatement, etc.)
	#[allow(dead_code)]
	pub(crate) name: Option<DynIden>,
	#[allow(dead_code)]
	pub(crate) kind: Option<TypeKind>,
}

/// Operations for ALTER TYPE.
///
/// This enum represents the different operations that can be performed
/// on a custom type using ALTER TYPE.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::type_def::TypeOperation;
/// use reinhardt_query::types::{Alias, IntoIden};
///
/// // RENAME TO new_name
/// let op = TypeOperation::RenameTo(Alias::new("new_mood").into_iden());
///
/// // ADD VALUE 'excited' BEFORE 'happy'
/// let op = TypeOperation::AddValue("excited".to_string(), Some("happy".to_string()));
///
/// // RENAME VALUE 'happy' TO 'joyful'
/// let op = TypeOperation::RenameValue("happy".to_string(), "joyful".to_string());
/// ```
#[derive(Debug, Clone)]
pub enum TypeOperation {
	/// RENAME TO new_name.
	RenameTo(DynIden),
	/// OWNER TO owner.
	OwnerTo(DynIden),
	/// SET SCHEMA schema.
	SetSchema(DynIden),
	/// ADD VALUE 'value' [BEFORE/AFTER 'existing_value'] (ENUM-specific).
	AddValue(String, Option<String>),
	/// RENAME VALUE 'old' TO 'new' (ENUM-specific).
	RenameValue(String, String),
	/// ADD CONSTRAINT name CHECK (constraint) (DOMAIN-specific).
	AddConstraint(String, String),
	/// DROP CONSTRAINT name [IF EXISTS] (DOMAIN-specific).
	DropConstraint(String, bool),
	/// SET DEFAULT value (DOMAIN-specific).
	SetDefault(String),
	/// DROP DEFAULT (DOMAIN-specific).
	DropDefault,
	/// SET NOT NULL (DOMAIN-specific).
	SetNotNull,
	/// DROP NOT NULL (DOMAIN-specific).
	DropNotNull,
}

impl TypeDef {
	/// Create a new type definition with the given name.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::type_def::TypeDef;
	///
	/// let type_def = TypeDef::new("my_type");
	/// ```
	pub fn new<T: IntoIden>(name: T) -> Self {
		Self {
			name: Some(name.into_iden()),
			kind: None,
		}
	}

	/// Set the type kind.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::type_def::{TypeDef, TypeKind};
	///
	/// let type_def = TypeDef::new("mood")
	///     .kind(TypeKind::Enum {
	///         values: vec!["happy".to_string(), "sad".to_string()],
	///     });
	/// ```
	pub fn kind(mut self, kind: TypeKind) -> Self {
		self.kind = Some(kind);
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::Alias;
	use rstest::rstest;

	// TypeKind tests
	#[rstest]
	fn test_type_kind_enum() {
		let kind = TypeKind::Enum {
			values: vec![
				"happy".to_string(),
				"sad".to_string(),
				"neutral".to_string(),
			],
		};

		if let TypeKind::Enum { values } = &kind {
			assert_eq!(values.len(), 3);
			assert_eq!(values[0], "happy");
			assert_eq!(values[1], "sad");
			assert_eq!(values[2], "neutral");
		} else {
			panic!("Expected Enum variant");
		}
	}

	#[rstest]
	fn test_type_kind_composite() {
		let kind = TypeKind::Composite {
			attributes: vec![
				("street".to_string(), "text".to_string()),
				("city".to_string(), "text".to_string()),
				("zip".to_string(), "integer".to_string()),
			],
		};

		if let TypeKind::Composite { attributes } = &kind {
			assert_eq!(attributes.len(), 3);
			assert_eq!(attributes[0].0, "street");
			assert_eq!(attributes[0].1, "text");
			assert_eq!(attributes[1].0, "city");
			assert_eq!(attributes[1].1, "text");
			assert_eq!(attributes[2].0, "zip");
			assert_eq!(attributes[2].1, "integer");
		} else {
			panic!("Expected Composite variant");
		}
	}

	#[rstest]
	fn test_type_kind_domain_full() {
		let kind = TypeKind::Domain {
			base_type: "text".to_string(),
			constraint: Some("CHECK (VALUE ~ '^[A-Z]')".to_string()),
			default: Some("'UNKNOWN'".to_string()),
			not_null: true,
		};

		if let TypeKind::Domain {
			base_type,
			constraint,
			default,
			not_null,
		} = &kind
		{
			assert_eq!(base_type, "text");
			assert_eq!(constraint.as_ref().unwrap(), "CHECK (VALUE ~ '^[A-Z]')");
			assert_eq!(default.as_ref().unwrap(), "'UNKNOWN'");
			assert!(not_null);
		} else {
			panic!("Expected Domain variant");
		}
	}

	#[rstest]
	fn test_type_kind_domain_minimal() {
		let kind = TypeKind::Domain {
			base_type: "integer".to_string(),
			constraint: None,
			default: None,
			not_null: false,
		};

		if let TypeKind::Domain {
			base_type,
			constraint,
			default,
			not_null,
		} = &kind
		{
			assert_eq!(base_type, "integer");
			assert!(constraint.is_none());
			assert!(default.is_none());
			assert!(!not_null);
		} else {
			panic!("Expected Domain variant");
		}
	}

	#[rstest]
	fn test_type_kind_range_full() {
		let kind = TypeKind::Range {
			subtype: "integer".to_string(),
			subtype_diff: Some("int4range_subdiff".to_string()),
			canonical: Some("int4range_canonical".to_string()),
		};

		if let TypeKind::Range {
			subtype,
			subtype_diff,
			canonical,
		} = &kind
		{
			assert_eq!(subtype, "integer");
			assert_eq!(subtype_diff.as_ref().unwrap(), "int4range_subdiff");
			assert_eq!(canonical.as_ref().unwrap(), "int4range_canonical");
		} else {
			panic!("Expected Range variant");
		}
	}

	#[rstest]
	fn test_type_kind_range_minimal() {
		let kind = TypeKind::Range {
			subtype: "timestamp".to_string(),
			subtype_diff: None,
			canonical: None,
		};

		if let TypeKind::Range {
			subtype,
			subtype_diff,
			canonical,
		} = &kind
		{
			assert_eq!(subtype, "timestamp");
			assert!(subtype_diff.is_none());
			assert!(canonical.is_none());
		} else {
			panic!("Expected Range variant");
		}
	}

	// TypeDef tests
	#[rstest]
	fn test_type_def_new() {
		let type_def = TypeDef::new("my_type");

		assert_eq!(type_def.name.as_ref().unwrap().to_string(), "my_type");
		assert!(type_def.kind.is_none());
	}

	#[rstest]
	fn test_type_def_with_enum() {
		let type_def = TypeDef::new("mood").kind(TypeKind::Enum {
			values: vec!["happy".to_string(), "sad".to_string()],
		});

		assert_eq!(type_def.name.as_ref().unwrap().to_string(), "mood");
		assert!(type_def.kind.is_some());

		if let Some(TypeKind::Enum { values }) = &type_def.kind {
			assert_eq!(values.len(), 2);
			assert_eq!(values[0], "happy");
			assert_eq!(values[1], "sad");
		} else {
			panic!("Expected Enum variant");
		}
	}

	#[rstest]
	fn test_type_def_with_composite() {
		let type_def = TypeDef::new("address").kind(TypeKind::Composite {
			attributes: vec![
				("street".to_string(), "text".to_string()),
				("city".to_string(), "text".to_string()),
			],
		});

		assert_eq!(type_def.name.as_ref().unwrap().to_string(), "address");

		if let Some(TypeKind::Composite { attributes }) = &type_def.kind {
			assert_eq!(attributes.len(), 2);
			assert_eq!(attributes[0].0, "street");
			assert_eq!(attributes[1].0, "city");
		} else {
			panic!("Expected Composite variant");
		}
	}

	#[rstest]
	fn test_type_def_with_alias() {
		let type_def = TypeDef::new(Alias::new("custom_type"));
		assert_eq!(type_def.name.as_ref().unwrap().to_string(), "custom_type");
	}

	// TypeOperation tests
	#[rstest]
	fn test_type_operation_rename_to() {
		let op = TypeOperation::RenameTo(Alias::new("new_name").into_iden());

		if let TypeOperation::RenameTo(name) = &op {
			assert_eq!(name.to_string(), "new_name");
		} else {
			panic!("Expected RenameTo variant");
		}
	}

	#[rstest]
	fn test_type_operation_owner_to() {
		let op = TypeOperation::OwnerTo(Alias::new("new_owner").into_iden());

		if let TypeOperation::OwnerTo(owner) = &op {
			assert_eq!(owner.to_string(), "new_owner");
		} else {
			panic!("Expected OwnerTo variant");
		}
	}

	#[rstest]
	fn test_type_operation_set_schema() {
		let op = TypeOperation::SetSchema(Alias::new("new_schema").into_iden());

		if let TypeOperation::SetSchema(schema) = &op {
			assert_eq!(schema.to_string(), "new_schema");
		} else {
			panic!("Expected SetSchema variant");
		}
	}

	#[rstest]
	fn test_type_operation_add_value_without_position() {
		let op = TypeOperation::AddValue("excited".to_string(), None);

		if let TypeOperation::AddValue(value, position) = &op {
			assert_eq!(value, "excited");
			assert!(position.is_none());
		} else {
			panic!("Expected AddValue variant");
		}
	}

	#[rstest]
	fn test_type_operation_add_value_with_position() {
		let op = TypeOperation::AddValue("excited".to_string(), Some("happy".to_string()));

		if let TypeOperation::AddValue(value, position) = &op {
			assert_eq!(value, "excited");
			assert_eq!(position.as_ref().unwrap(), "happy");
		} else {
			panic!("Expected AddValue variant");
		}
	}

	#[rstest]
	fn test_type_operation_rename_value() {
		let op = TypeOperation::RenameValue("happy".to_string(), "joyful".to_string());

		if let TypeOperation::RenameValue(old_name, new_name) = &op {
			assert_eq!(old_name, "happy");
			assert_eq!(new_name, "joyful");
		} else {
			panic!("Expected RenameValue variant");
		}
	}

	#[rstest]
	fn test_type_operation_add_constraint() {
		let op =
			TypeOperation::AddConstraint("positive".to_string(), "CHECK (VALUE > 0)".to_string());

		if let TypeOperation::AddConstraint(name, check) = &op {
			assert_eq!(name, "positive");
			assert_eq!(check, "CHECK (VALUE > 0)");
		} else {
			panic!("Expected AddConstraint variant");
		}
	}

	#[rstest]
	fn test_type_operation_drop_constraint_if_exists() {
		let op = TypeOperation::DropConstraint("my_constraint".to_string(), true);

		if let TypeOperation::DropConstraint(name, if_exists) = &op {
			assert_eq!(name, "my_constraint");
			assert!(if_exists);
		} else {
			panic!("Expected DropConstraint variant");
		}
	}

	#[rstest]
	fn test_type_operation_drop_constraint_without_if_exists() {
		let op = TypeOperation::DropConstraint("my_constraint".to_string(), false);

		if let TypeOperation::DropConstraint(name, if_exists) = &op {
			assert_eq!(name, "my_constraint");
			assert!(!if_exists);
		} else {
			panic!("Expected DropConstraint variant");
		}
	}

	#[rstest]
	fn test_type_operation_set_default() {
		let op = TypeOperation::SetDefault("'UNKNOWN'".to_string());

		if let TypeOperation::SetDefault(value) = &op {
			assert_eq!(value, "'UNKNOWN'");
		} else {
			panic!("Expected SetDefault variant");
		}
	}

	#[rstest]
	fn test_type_operation_drop_default() {
		let op = TypeOperation::DropDefault;
		assert!(matches!(op, TypeOperation::DropDefault));
	}

	#[rstest]
	fn test_type_operation_set_not_null() {
		let op = TypeOperation::SetNotNull;
		assert!(matches!(op, TypeOperation::SetNotNull));
	}

	#[rstest]
	fn test_type_operation_drop_not_null() {
		let op = TypeOperation::DropNotNull;
		assert!(matches!(op, TypeOperation::DropNotNull));
	}
}
