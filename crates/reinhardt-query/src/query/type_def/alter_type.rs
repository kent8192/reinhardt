//! ALTER TYPE statement builder
//!
//! This module provides the `AlterTypeStatement` type for building SQL ALTER TYPE queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden, type_def::TypeOperation},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ALTER TYPE statement builder
///
/// This struct provides a fluent API for constructing ALTER TYPE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // ALTER TYPE mood RENAME TO feeling
/// let query = Query::alter_type()
///     .name("mood")
///     .rename_to("feeling");
///
/// // ALTER TYPE mood ADD VALUE 'excited' BEFORE 'happy'
/// let query = Query::alter_type()
///     .name("mood")
///     .add_value("excited", Some("happy"));
///
/// // ALTER TYPE mood RENAME VALUE 'happy' TO 'joyful'
/// let query = Query::alter_type()
///     .name("mood")
///     .rename_value("happy", "joyful");
/// ```
#[derive(Debug, Clone)]
pub struct AlterTypeStatement {
	pub(crate) name: DynIden,
	pub(crate) operations: Vec<TypeOperation>,
}

impl AlterTypeStatement {
	/// Create a new ALTER TYPE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type();
	/// ```
	pub fn new() -> Self {
		Self {
			name: "".into_iden(),
			operations: Vec::new(),
		}
	}

	/// Take the ownership of data in the current [`AlterTypeStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			name: self.name.clone(),
			operations: self.operations.clone(),
		};
		// Reset self to empty state
		self.name = "".into_iden();
		self.operations.clear();
		taken
	}

	/// Set the type name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("my_type");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = name.into_iden();
		self
	}

	/// Add RENAME TO operation
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("old_name")
	///     .rename_to("new_name");
	/// ```
	pub fn rename_to<N>(&mut self, new_name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.operations
			.push(TypeOperation::RenameTo(new_name.into_iden()));
		self
	}

	/// Add OWNER TO operation
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("my_type")
	///     .owner_to("new_owner");
	/// ```
	pub fn owner_to<O>(&mut self, owner: O) -> &mut Self
	where
		O: IntoIden,
	{
		self.operations
			.push(TypeOperation::OwnerTo(owner.into_iden()));
		self
	}

	/// Add SET SCHEMA operation
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("my_type")
	///     .set_schema("new_schema");
	/// ```
	pub fn set_schema<S>(&mut self, schema: S) -> &mut Self
	where
		S: IntoIden,
	{
		self.operations
			.push(TypeOperation::SetSchema(schema.into_iden()));
		self
	}

	/// Add VALUE to ENUM type (optionally before/after existing value)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// // ADD VALUE 'excited'
	/// let query = Query::alter_type()
	///     .name("mood")
	///     .add_value("excited", None);
	///
	/// // ADD VALUE 'excited' BEFORE 'happy'
	/// let query = Query::alter_type()
	///     .name("mood")
	///     .add_value("excited", Some("happy"));
	/// ```
	pub fn add_value(&mut self, value: &str, before_after: Option<&str>) -> &mut Self {
		self.operations.push(TypeOperation::AddValue(
			value.to_string(),
			before_after.map(|s| s.to_string()),
		));
		self
	}

	/// Rename an ENUM value
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("mood")
	///     .rename_value("happy", "joyful");
	/// ```
	pub fn rename_value(&mut self, old_value: &str, new_value: &str) -> &mut Self {
		self.operations.push(TypeOperation::RenameValue(
			old_value.to_string(),
			new_value.to_string(),
		));
		self
	}

	/// Add constraint to DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("my_domain")
	///     .add_constraint("positive_check", "CHECK (VALUE > 0)");
	/// ```
	pub fn add_constraint(&mut self, name: &str, check: &str) -> &mut Self {
		self.operations.push(TypeOperation::AddConstraint(
			name.to_string(),
			check.to_string(),
		));
		self
	}

	/// Drop constraint from DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP CONSTRAINT my_constraint
	/// let query = Query::alter_type()
	///     .name("my_domain")
	///     .drop_constraint("my_constraint", false);
	///
	/// // DROP CONSTRAINT IF EXISTS my_constraint
	/// let query = Query::alter_type()
	///     .name("my_domain")
	///     .drop_constraint("my_constraint", true);
	/// ```
	pub fn drop_constraint(&mut self, name: &str, if_exists: bool) -> &mut Self {
		self.operations
			.push(TypeOperation::DropConstraint(name.to_string(), if_exists));
		self
	}

	/// Set default value for DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("my_domain")
	///     .set_default("0");
	/// ```
	pub fn set_default(&mut self, value: &str) -> &mut Self {
		self.operations
			.push(TypeOperation::SetDefault(value.to_string()));
		self
	}

	/// Drop default value from DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("my_domain")
	///     .drop_default();
	/// ```
	pub fn drop_default(&mut self) -> &mut Self {
		self.operations.push(TypeOperation::DropDefault);
		self
	}

	/// Set NOT NULL for DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("my_domain")
	///     .set_not_null();
	/// ```
	pub fn set_not_null(&mut self) -> &mut Self {
		self.operations.push(TypeOperation::SetNotNull);
		self
	}

	/// Drop NOT NULL from DOMAIN type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_type()
	///     .name("my_domain")
	///     .drop_not_null();
	/// ```
	pub fn drop_not_null(&mut self) -> &mut Self {
		self.operations.push(TypeOperation::DropNotNull);
		self
	}
}

impl Default for AlterTypeStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterTypeStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_alter_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_alter_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_alter_type(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_alter_type(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AlterTypeStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::Alias;
	use rstest::rstest;

	#[rstest]
	fn test_alter_type_new() {
		let stmt = AlterTypeStatement::new();
		assert!(stmt.name.to_string().is_empty());
		assert!(stmt.operations.is_empty());
	}

	#[rstest]
	fn test_alter_type_with_name() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_type");
		assert_eq!(stmt.name.to_string(), "my_type");
	}

	#[rstest]
	fn test_alter_type_rename_to() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("old_name").rename_to("new_name");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::RenameTo(name) => {
				assert_eq!(name.to_string(), "new_name");
			}
			_ => panic!("Expected RenameTo operation"),
		}
	}

	#[rstest]
	fn test_alter_type_owner_to() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_type").owner_to("new_owner");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::OwnerTo(owner) => {
				assert_eq!(owner.to_string(), "new_owner");
			}
			_ => panic!("Expected OwnerTo operation"),
		}
	}

	#[rstest]
	fn test_alter_type_set_schema() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_type").set_schema("new_schema");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::SetSchema(schema) => {
				assert_eq!(schema.to_string(), "new_schema");
			}
			_ => panic!("Expected SetSchema operation"),
		}
	}

	#[rstest]
	fn test_alter_type_add_value_without_position() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("mood").add_value("excited", None);
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::AddValue(value, position) => {
				assert_eq!(value, "excited");
				assert!(position.is_none());
			}
			_ => panic!("Expected AddValue operation"),
		}
	}

	#[rstest]
	fn test_alter_type_add_value_with_position() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("mood").add_value("excited", Some("happy"));
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::AddValue(value, position) => {
				assert_eq!(value, "excited");
				assert_eq!(position.as_ref().unwrap(), "happy");
			}
			_ => panic!("Expected AddValue operation"),
		}
	}

	#[rstest]
	fn test_alter_type_rename_value() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("mood").rename_value("happy", "joyful");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::RenameValue(old_val, new_val) => {
				assert_eq!(old_val, "happy");
				assert_eq!(new_val, "joyful");
			}
			_ => panic!("Expected RenameValue operation"),
		}
	}

	#[rstest]
	fn test_alter_type_add_constraint() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_domain")
			.add_constraint("positive_check", "CHECK (VALUE > 0)");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::AddConstraint(name, check) => {
				assert_eq!(name, "positive_check");
				assert_eq!(check, "CHECK (VALUE > 0)");
			}
			_ => panic!("Expected AddConstraint operation"),
		}
	}

	#[rstest]
	fn test_alter_type_drop_constraint_without_if_exists() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_domain")
			.drop_constraint("my_constraint", false);
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::DropConstraint(name, if_exists) => {
				assert_eq!(name, "my_constraint");
				assert!(!if_exists);
			}
			_ => panic!("Expected DropConstraint operation"),
		}
	}

	#[rstest]
	fn test_alter_type_drop_constraint_with_if_exists() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_domain")
			.drop_constraint("my_constraint", true);
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::DropConstraint(name, if_exists) => {
				assert_eq!(name, "my_constraint");
				assert!(if_exists);
			}
			_ => panic!("Expected DropConstraint operation"),
		}
	}

	#[rstest]
	fn test_alter_type_set_default() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_domain").set_default("0");
		assert_eq!(stmt.operations.len(), 1);
		match &stmt.operations[0] {
			TypeOperation::SetDefault(value) => {
				assert_eq!(value, "0");
			}
			_ => panic!("Expected SetDefault operation"),
		}
	}

	#[rstest]
	fn test_alter_type_drop_default() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_domain").drop_default();
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(stmt.operations[0], TypeOperation::DropDefault));
	}

	#[rstest]
	fn test_alter_type_set_not_null() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_domain").set_not_null();
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(stmt.operations[0], TypeOperation::SetNotNull));
	}

	#[rstest]
	fn test_alter_type_drop_not_null() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_domain").drop_not_null();
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(stmt.operations[0], TypeOperation::DropNotNull));
	}

	#[rstest]
	fn test_alter_type_multiple_operations() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_domain")
			.set_default("0")
			.set_not_null()
			.add_constraint("positive", "CHECK (VALUE > 0)");
		assert_eq!(stmt.operations.len(), 3);
	}

	#[rstest]
	fn test_alter_type_take() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name("my_type").rename_to("new_name");
		let taken = stmt.take();
		assert!(stmt.name.to_string().is_empty());
		assert!(stmt.operations.is_empty());
		assert_eq!(taken.name.to_string(), "my_type");
		assert_eq!(taken.operations.len(), 1);
	}

	#[rstest]
	fn test_alter_type_with_alias() {
		let mut stmt = AlterTypeStatement::new();
		stmt.name(Alias::new("custom_type"));
		assert_eq!(stmt.name.to_string(), "custom_type");
	}
}
