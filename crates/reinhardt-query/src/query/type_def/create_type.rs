//! CREATE TYPE statement builder
//!
//! This module provides the `CreateTypeStatement` type for building SQL CREATE TYPE queries.

use crate::{
	backend::QueryBuilder,
	types::{IntoIden, type_def::TypeKind},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE TYPE statement builder
///
/// This struct provides a fluent API for constructing CREATE TYPE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
/// use reinhardt_query::types::type_def::TypeKind;
///
/// // CREATE TYPE mood AS ENUM ('happy', 'sad', 'neutral')
/// let query = Query::create_type()
///     .name("mood")
///     .as_enum(vec!["happy".to_string(), "sad".to_string(), "neutral".to_string()]);
///
/// // CREATE TYPE address AS (street text, city text, zip integer)
/// let query = Query::create_type()
///     .name("address")
///     .as_composite(vec![
///         ("street".to_string(), "text".to_string()),
///         ("city".to_string(), "text".to_string()),
///         ("zip".to_string(), "integer".to_string()),
///     ]);
/// ```
#[derive(Debug, Clone)]
pub struct CreateTypeStatement {
	pub(crate) name: Option<crate::types::DynIden>,
	pub(crate) kind: Option<TypeKind>,
}

impl CreateTypeStatement {
	/// Create a new CREATE TYPE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type();
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			kind: None,
		}
	}

	/// Take the ownership of data in the current [`CreateTypeStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			name: self.name.clone(),
			kind: self.kind.clone(),
		};
		// Reset self to empty state
		self.name = None;
		self.kind = None;
		taken
	}

	/// Set the type name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("my_type");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Create an ENUM type with the given values
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("mood")
	///     .as_enum(vec!["happy".to_string(), "sad".to_string()]);
	/// ```
	pub fn as_enum(&mut self, values: Vec<String>) -> &mut Self {
		self.kind = Some(TypeKind::Enum { values });
		self
	}

	/// Create a COMPOSITE type with the given attributes
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("address")
	///     .as_composite(vec![
	///         ("street".to_string(), "text".to_string()),
	///         ("city".to_string(), "text".to_string()),
	///     ]);
	/// ```
	pub fn as_composite(&mut self, attributes: Vec<(String, String)>) -> &mut Self {
		self.kind = Some(TypeKind::Composite { attributes });
		self
	}

	/// Create a DOMAIN type with the given base type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("positive_int")
	///     .as_domain("integer".to_string())
	///     .constraint("positive_check".to_string(), "CHECK (VALUE > 0)".to_string());
	/// ```
	pub fn as_domain(&mut self, base_type: String) -> &mut Self {
		self.kind = Some(TypeKind::Domain {
			base_type,
			constraint: None,
			default: None,
			not_null: false,
		});
		self
	}

	/// Add a constraint to a DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("positive_int")
	///     .as_domain("integer".to_string())
	///     .constraint("positive_check".to_string(), "CHECK (VALUE > 0)".to_string());
	/// ```
	pub fn constraint(&mut self, _name: String, check: String) -> &mut Self {
		if let Some(TypeKind::Domain { constraint, .. }) = &mut self.kind {
			*constraint = Some(check);
		}
		self
	}

	/// Set the default value for a DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("my_domain")
	///     .as_domain("integer".to_string())
	///     .default_value("0".to_string());
	/// ```
	pub fn default_value(&mut self, default: String) -> &mut Self {
		if let Some(TypeKind::Domain {
			default: default_field,
			..
		}) = &mut self.kind
		{
			*default_field = Some(default);
		}
		self
	}

	/// Set NOT NULL for a DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("my_domain")
	///     .as_domain("integer".to_string())
	///     .not_null();
	/// ```
	pub fn not_null(&mut self) -> &mut Self {
		if let Some(TypeKind::Domain { not_null, .. }) = &mut self.kind {
			*not_null = true;
		}
		self
	}

	/// Create a RANGE type with the given subtype
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("int_range")
	///     .as_range("integer".to_string());
	/// ```
	pub fn as_range(&mut self, subtype: String) -> &mut Self {
		self.kind = Some(TypeKind::Range {
			subtype,
			subtype_diff: None,
			canonical: None,
		});
		self
	}

	/// Set the subtype diff function for a RANGE type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("int_range")
	///     .as_range("integer".to_string())
	///     .subtype_diff("int4range_subdiff".to_string());
	/// ```
	pub fn subtype_diff(&mut self, subtype_diff: String) -> &mut Self {
		if let Some(TypeKind::Range {
			subtype_diff: diff_field,
			..
		}) = &mut self.kind
		{
			*diff_field = Some(subtype_diff);
		}
		self
	}

	/// Set the canonical function for a RANGE type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_type()
	///     .name("int_range")
	///     .as_range("integer".to_string())
	///     .canonical("int4range_canonical".to_string());
	/// ```
	pub fn canonical(&mut self, canonical: String) -> &mut Self {
		if let Some(TypeKind::Range {
			canonical: canonical_field,
			..
		}) = &mut self.kind
		{
			*canonical_field = Some(canonical);
		}
		self
	}
}

impl Default for CreateTypeStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateTypeStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_create_type(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateTypeStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::Alias;
	use rstest::rstest;

	#[rstest]
	fn test_create_type_new() {
		let stmt = CreateTypeStatement::new();
		assert!(stmt.name.is_none());
		assert!(stmt.kind.is_none());
	}

	#[rstest]
	fn test_create_type_name() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("my_type");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_type");
	}

	#[rstest]
	fn test_create_type_enum() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("mood").as_enum(vec![
			"happy".to_string(),
			"sad".to_string(),
			"neutral".to_string(),
		]);

		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "mood");
		if let Some(TypeKind::Enum { values }) = &stmt.kind {
			assert_eq!(values.len(), 3);
			assert_eq!(values[0], "happy");
			assert_eq!(values[1], "sad");
			assert_eq!(values[2], "neutral");
		} else {
			panic!("Expected Enum type kind");
		}
	}

	#[rstest]
	fn test_create_type_composite() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("address").as_composite(vec![
			("street".to_string(), "text".to_string()),
			("city".to_string(), "text".to_string()),
			("zip".to_string(), "integer".to_string()),
		]);

		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "address");
		if let Some(TypeKind::Composite { attributes }) = &stmt.kind {
			assert_eq!(attributes.len(), 3);
			assert_eq!(attributes[0].0, "street");
			assert_eq!(attributes[0].1, "text");
			assert_eq!(attributes[1].0, "city");
			assert_eq!(attributes[1].1, "text");
			assert_eq!(attributes[2].0, "zip");
			assert_eq!(attributes[2].1, "integer");
		} else {
			panic!("Expected Composite type kind");
		}
	}

	#[rstest]
	fn test_create_type_domain_minimal() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("my_domain").as_domain("integer".to_string());

		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_domain");
		if let Some(TypeKind::Domain {
			base_type,
			constraint,
			default,
			not_null,
		}) = &stmt.kind
		{
			assert_eq!(base_type, "integer");
			assert!(constraint.is_none());
			assert!(default.is_none());
			assert!(!not_null);
		} else {
			panic!("Expected Domain type kind");
		}
	}

	#[rstest]
	fn test_create_type_domain_with_constraint() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("positive_int")
			.as_domain("integer".to_string())
			.constraint(
				"positive_check".to_string(),
				"CHECK (VALUE > 0)".to_string(),
			);

		if let Some(TypeKind::Domain { constraint, .. }) = &stmt.kind {
			assert_eq!(constraint.as_ref().unwrap(), "CHECK (VALUE > 0)");
		} else {
			panic!("Expected Domain type kind");
		}
	}

	#[rstest]
	fn test_create_type_domain_with_default() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("my_domain")
			.as_domain("integer".to_string())
			.default_value("0".to_string());

		if let Some(TypeKind::Domain { default, .. }) = &stmt.kind {
			assert_eq!(default.as_ref().unwrap(), "0");
		} else {
			panic!("Expected Domain type kind");
		}
	}

	#[rstest]
	fn test_create_type_domain_not_null() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("my_domain")
			.as_domain("integer".to_string())
			.not_null();

		if let Some(TypeKind::Domain { not_null, .. }) = &stmt.kind {
			assert!(not_null);
		} else {
			panic!("Expected Domain type kind");
		}
	}

	#[rstest]
	fn test_create_type_domain_full() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("positive_int")
			.as_domain("integer".to_string())
			.constraint(
				"positive_check".to_string(),
				"CHECK (VALUE > 0)".to_string(),
			)
			.default_value("1".to_string())
			.not_null();

		if let Some(TypeKind::Domain {
			base_type,
			constraint,
			default,
			not_null,
		}) = &stmt.kind
		{
			assert_eq!(base_type, "integer");
			assert_eq!(constraint.as_ref().unwrap(), "CHECK (VALUE > 0)");
			assert_eq!(default.as_ref().unwrap(), "1");
			assert!(not_null);
		} else {
			panic!("Expected Domain type kind");
		}
	}

	#[rstest]
	fn test_create_type_range_minimal() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("int_range").as_range("integer".to_string());

		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "int_range");
		if let Some(TypeKind::Range {
			subtype,
			subtype_diff,
			canonical,
		}) = &stmt.kind
		{
			assert_eq!(subtype, "integer");
			assert!(subtype_diff.is_none());
			assert!(canonical.is_none());
		} else {
			panic!("Expected Range type kind");
		}
	}

	#[rstest]
	fn test_create_type_range_with_subtype_diff() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("int_range")
			.as_range("integer".to_string())
			.subtype_diff("int4range_subdiff".to_string());

		if let Some(TypeKind::Range { subtype_diff, .. }) = &stmt.kind {
			assert_eq!(subtype_diff.as_ref().unwrap(), "int4range_subdiff");
		} else {
			panic!("Expected Range type kind");
		}
	}

	#[rstest]
	fn test_create_type_range_with_canonical() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("int_range")
			.as_range("integer".to_string())
			.canonical("int4range_canonical".to_string());

		if let Some(TypeKind::Range { canonical, .. }) = &stmt.kind {
			assert_eq!(canonical.as_ref().unwrap(), "int4range_canonical");
		} else {
			panic!("Expected Range type kind");
		}
	}

	#[rstest]
	fn test_create_type_range_full() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("int_range")
			.as_range("integer".to_string())
			.subtype_diff("int4range_subdiff".to_string())
			.canonical("int4range_canonical".to_string());

		if let Some(TypeKind::Range {
			subtype,
			subtype_diff,
			canonical,
		}) = &stmt.kind
		{
			assert_eq!(subtype, "integer");
			assert_eq!(subtype_diff.as_ref().unwrap(), "int4range_subdiff");
			assert_eq!(canonical.as_ref().unwrap(), "int4range_canonical");
		} else {
			panic!("Expected Range type kind");
		}
	}

	#[rstest]
	fn test_create_type_take() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name("my_type")
			.as_enum(vec!["happy".to_string(), "sad".to_string()]);

		let taken = stmt.take();
		assert_eq!(taken.name.as_ref().unwrap().to_string(), "my_type");
		assert!(taken.kind.is_some());

		// Original should be reset
		assert!(stmt.name.is_none());
		assert!(stmt.kind.is_none());
	}

	#[rstest]
	fn test_create_type_with_alias() {
		let mut stmt = CreateTypeStatement::new();
		stmt.name(Alias::new("custom_type"));
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "custom_type");
	}
}
