//! Type-safe field representation with transformation support

use super::comparison::{ComparisonOperator, FieldComparison, FieldRef};
use super::lookup::{Lookup, LookupType};
use super::traits::{Comparable, Date, DateTime, NumericType};
use crate::orm::Model;
use std::marker::PhantomData;

/// Represents a field with its type information
///
/// The type parameter `M` is the model type, and `T` is the field's type.
/// This allows us to enforce type safety at compile time.
///
/// # Breaking Change
///
/// The type of `path` has been changed from `Vec<&'static str>` to `Vec<String>`.
/// Table alias support has been added.
#[derive(Debug, Clone)]
pub struct Field<M, T> {
	pub(crate) path: Vec<String>,
	pub(crate) table_alias: Option<String>,
	pub(crate) _phantom: PhantomData<(M, T)>,
}

impl<M: Model, T> Field<M, T> {
	/// Create a new field with the given path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::query_fields::Field;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i32>,
	///     name: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i32;
	///     type Fields = UserFields;
	///     fn table_name() -> &'static str {
	///         "users"
	///     }
	///     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         self.id
	///     }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = Some(value);
	///     }
	/// }
	///
	/// let field: Field<User, String> = Field::new(vec!["name"]);
	/// assert_eq!(field.path(), &["name".to_string()]);
	/// ```
	pub fn new<S: Into<String>>(path: Vec<S>) -> Self {
		Self {
			path: path.into_iter().map(|s| s.into()).collect(),
			table_alias: None,
			_phantom: PhantomData,
		}
	}

	/// Get the field path
	pub fn path(&self) -> &[String] {
		&self.path
	}

	/// Set table alias
	///
	/// Used when referencing the same table multiple times (e.g., self-JOINs).
	///
	/// # Examples
	///
	/// ```ignore
	/// let u1_id = Field::<User, i32>::new(vec!["id".to_string()]).with_alias("u1");
	/// let u2_id = Field::<User, i32>::new(vec!["id".to_string()]).with_alias("u2");
	/// ```
	pub fn with_alias(mut self, alias: &str) -> Self {
		self.table_alias = Some(alias.to_string());
		self
	}

	/// Convert Field to FieldRef for comparison operations
	pub(crate) fn to_field_ref(&self) -> FieldRef {
		FieldRef::Field {
			table_alias: self.table_alias.clone(),
			field_path: self.path.clone(),
		}
	}

	// =============================================================================
	// Inter-field comparison methods (for JOIN conditions)
	// =============================================================================

	/// Inter-field comparison: <
	///
	/// # Examples
	///
	/// ```ignore
	/// // u1.id < u2.id
	/// let comparison = u1_id.field_lt(u2_id);
	/// ```
	pub fn field_lt<M2: Model>(self, other: Field<M2, T>) -> FieldComparison {
		FieldComparison::new(
			self.to_field_ref(),
			other.to_field_ref(),
			ComparisonOperator::Lt,
		)
	}

	/// Inter-field comparison: >
	pub fn field_gt<M2: Model>(self, other: Field<M2, T>) -> FieldComparison {
		FieldComparison::new(
			self.to_field_ref(),
			other.to_field_ref(),
			ComparisonOperator::Gt,
		)
	}

	/// Inter-field comparison: <=
	pub fn field_lte<M2: Model>(self, other: Field<M2, T>) -> FieldComparison {
		FieldComparison::new(
			self.to_field_ref(),
			other.to_field_ref(),
			ComparisonOperator::Lte,
		)
	}

	/// Inter-field comparison: >=
	pub fn field_gte<M2: Model>(self, other: Field<M2, T>) -> FieldComparison {
		FieldComparison::new(
			self.to_field_ref(),
			other.to_field_ref(),
			ComparisonOperator::Gte,
		)
	}

	/// Inter-field comparison: ==
	pub fn field_eq<M2: Model>(self, other: Field<M2, T>) -> FieldComparison {
		FieldComparison::new(
			self.to_field_ref(),
			other.to_field_ref(),
			ComparisonOperator::Eq,
		)
	}

	/// Inter-field comparison: !=
	pub fn field_ne<M2: Model>(self, other: Field<M2, T>) -> FieldComparison {
		FieldComparison::new(
			self.to_field_ref(),
			other.to_field_ref(),
			ComparisonOperator::Ne,
		)
	}
}

// =============================================================================
// String-specific methods
// =============================================================================

impl<M: Model> Field<M, String> {
	/// Convert field to lowercase: LOWER(field)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::query_fields::Field;
	/// use reinhardt_db::orm::Model;
	/// use reinhardt_core::validators::TableName;
	///
	/// #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// struct User {
	///     id: i64,
	///     email: String,
	/// }
	///
	/// const USER_TABLE: TableName = TableName::new_const("users");
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn table_name() -> &'static str { USER_TABLE.as_str() }
	///     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// }
	///
	/// // Lower case transformation for case-insensitive comparisons
	/// let email_field = Field::<User, String>::new(vec!["email"]).lower();
	/// assert_eq!(email_field.path(), &["email", "lower"]);
	/// ```
	pub fn lower(mut self) -> Self {
		self.path.push("lower".to_string());
		self
	}
	/// Convert field to uppercase: UPPER(field)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::query_fields::Field;
	/// use reinhardt_db::orm::Model;
	/// use reinhardt_core::validators::TableName;
	///
	/// #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// struct Product {
	///     id: i64,
	///     code: String,
	/// }
	///
	/// const PRODUCT_TABLE: TableName = TableName::new_const("products");
	///
	/// #[derive(Clone)]
	/// struct ProductFields;
	/// impl reinhardt_db::orm::FieldSelector for ProductFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for Product {
	///     type PrimaryKey = i64;
	///     type Fields = ProductFields;
	///     fn table_name() -> &'static str { PRODUCT_TABLE.as_str() }
	///     fn new_fields() -> Self::Fields { ProductFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// }
	///
	/// // Upper case transformation for normalization
	/// let code_field = Field::<Product, String>::new(vec!["code"]).upper();
	/// assert_eq!(code_field.path(), &["code", "upper"]);
	/// ```
	pub fn upper(mut self) -> Self {
		self.path.push("upper".to_string());
		self
	}
	/// Trim whitespace: TRIM(field)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::query_fields::Field;
	/// use reinhardt_db::orm::Model;
	/// use reinhardt_core::validators::TableName;
	///
	/// #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// struct Comment {
	///     id: i64,
	///     text: String,
	/// }
	///
	/// const COMMENT_TABLE: TableName = TableName::new_const("comments");
	///
	/// #[derive(Clone)]
	/// struct CommentFields;
	/// impl reinhardt_db::orm::FieldSelector for CommentFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for Comment {
	///     type PrimaryKey = i64;
	///     type Fields = CommentFields;
	///     fn table_name() -> &'static str { COMMENT_TABLE.as_str() }
	///     fn new_fields() -> Self::Fields { CommentFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// }
	///
	/// // Trim whitespace from both ends
	/// let text_field = Field::<Comment, String>::new(vec!["text"]).trim();
	/// assert_eq!(text_field.path(), &["text", "trim"]);
	/// ```
	pub fn trim(mut self) -> Self {
		self.path.push("trim".to_string());
		self
	}
	/// Get string length: LENGTH(field)
	/// Note: Return type changes to usize
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::query_fields::Field;
	/// use reinhardt_db::orm::Model;
	/// use reinhardt_core::validators::TableName;
	///
	/// #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// struct Post {
	///     id: i64,
	///     title: String,
	/// }
	///
	/// const POST_TABLE: TableName = TableName::new_const("posts");
	///
	/// #[derive(Clone)]
	/// struct PostFields;
	/// impl reinhardt_db::orm::FieldSelector for PostFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for Post {
	///     type PrimaryKey = i64;
	///     type Fields = PostFields;
	///     fn table_name() -> &'static str { POST_TABLE.as_str() }
	///     fn new_fields() -> Self::Fields { PostFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// }
	///
	/// // Get length of string field (returns Field<M, usize>)
	/// let title_length = Field::<Post, String>::new(vec!["title"]).length();
	/// assert_eq!(title_length.path(), &["title", "length"]);
	/// ```
	pub fn length(mut self) -> Field<M, usize> {
		self.path.push("length".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}

	// Lookup methods specific to strings
	/// Check if field contains a substring: LIKE '%value%'
	///
	pub fn contains(self, value: &str) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Contains, value.into())
	}
	/// Case-insensitive contains: ILIKE '%value%'
	///
	pub fn icontains(self, value: &str) -> Lookup<M> {
		Lookup::new(self.path, LookupType::IContains, value.into())
	}
	/// Check if field starts with a substring: LIKE 'value%'
	///
	pub fn startswith(self, value: &str) -> Lookup<M> {
		Lookup::new(self.path, LookupType::StartsWith, value.into())
	}
	/// Case-insensitive startswith: ILIKE 'value%'
	///
	pub fn istartswith(self, value: &str) -> Lookup<M> {
		Lookup::new(self.path, LookupType::IStartsWith, value.into())
	}
	/// Check if field ends with a substring: LIKE '%value'
	///
	pub fn endswith(self, value: &str) -> Lookup<M> {
		Lookup::new(self.path, LookupType::EndsWith, value.into())
	}
	/// Case-insensitive endswith: ILIKE '%value'
	///
	pub fn iendswith(self, value: &str) -> Lookup<M> {
		Lookup::new(self.path, LookupType::IEndsWith, value.into())
	}
	/// Regular expression match: ~
	///
	pub fn regex(self, pattern: &str) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Regex, pattern.into())
	}
	/// Case-insensitive regex: ~*
	///
	pub fn iregex(self, pattern: &str) -> Lookup<M> {
		Lookup::new(self.path, LookupType::IRegex, pattern.into())
	}
}

// =============================================================================
// DateTime-specific methods
// =============================================================================

impl<M: Model> Field<M, DateTime> {
	/// Extract year: EXTRACT(YEAR FROM field)
	///
	pub fn year(mut self) -> Field<M, i32> {
		self.path.push("year".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Extract month: EXTRACT(MONTH FROM field)
	///
	pub fn month(mut self) -> Field<M, i32> {
		self.path.push("month".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Extract day: EXTRACT(DAY FROM field)
	///
	pub fn day(mut self) -> Field<M, i32> {
		self.path.push("day".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Extract week: EXTRACT(WEEK FROM field)
	///
	pub fn week(mut self) -> Field<M, i32> {
		self.path.push("week".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Extract day of week: EXTRACT(DOW FROM field)
	///
	pub fn weekday(mut self) -> Field<M, i32> {
		self.path.push("weekday".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Extract quarter: EXTRACT(QUARTER FROM field)
	///
	pub fn quarter(mut self) -> Field<M, i32> {
		self.path.push("quarter".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Extract hour: EXTRACT(HOUR FROM field)
	///
	pub fn hour(mut self) -> Field<M, i32> {
		self.path.push("hour".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Extract minute: EXTRACT(MINUTE FROM field)
	///
	pub fn minute(mut self) -> Field<M, i32> {
		self.path.push("minute".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Extract second: EXTRACT(SECOND FROM field)
	///
	pub fn second(mut self) -> Field<M, i32> {
		self.path.push("second".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
	/// Convert to date: DATE(field)
	///
	pub fn date(mut self) -> Field<M, Date> {
		self.path.push("date".to_string());
		Field {
			path: self.path,
			table_alias: self.table_alias,
			_phantom: PhantomData,
		}
	}
}

// =============================================================================
// Numeric-specific methods
// =============================================================================

impl<M: Model, T: NumericType> Field<M, T> {
	/// Absolute value: ABS(field)
	///
	pub fn abs(mut self) -> Self {
		self.path.push("abs".to_string());
		self
	}
	/// Ceiling: CEIL(field)
	///
	pub fn ceil(mut self) -> Self {
		self.path.push("ceil".to_string());
		self
	}
	/// Floor: FLOOR(field)
	///
	pub fn floor(mut self) -> Self {
		self.path.push("floor".to_string());
		self
	}
	/// Round: ROUND(field)
	///
	pub fn round(mut self) -> Self {
		self.path.push("round".to_string());
		self
	}
}

// =============================================================================
// Comparison methods (available for all Comparable types)
// =============================================================================

impl<M: Model, T: Comparable> Field<M, T> {
	/// Exact equality: =
	///
	pub fn eq(self, value: T) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Exact, value.into())
	}
	/// Case-insensitive equality (for strings): ILIKE
	///
	pub fn iexact(self, value: T) -> Lookup<M> {
		Lookup::new(self.path, LookupType::IExact, value.into())
	}
	/// Not equal: !=
	///
	pub fn ne(self, value: T) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Ne, value.into())
	}
	/// Greater than: >
	///
	pub fn gt(self, value: T) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Gt, value.into())
	}
	/// Greater than or equal: >=
	///
	pub fn gte(self, value: T) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Gte, value.into())
	}
	/// Less than: <
	///
	pub fn lt(self, value: T) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Lt, value.into())
	}
	/// Less than or equal: <=
	///
	pub fn lte(self, value: T) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Lte, value.into())
	}
	/// Range check: BETWEEN
	///
	pub fn in_range(self, start: T, end: T) -> Lookup<M> {
		Lookup::new(self.path, LookupType::Range, (start, end).into())
	}
}

// =============================================================================
// Option<T> specific methods
// =============================================================================

impl<M: Model, T> Field<M, Option<T>> {
	/// Check if NULL: IS NULL
	///
	pub fn is_null(self) -> Lookup<M> {
		Lookup::new(self.path, LookupType::IsNull, ().into())
	}
	/// Check if NOT NULL: IS NOT NULL
	///
	pub fn is_not_null(self) -> Lookup<M> {
		Lookup::new(self.path, LookupType::IsNotNull, ().into())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::Model;
	use reinhardt_core::validators::TableName;

	#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	struct TestUser {
		id: i64,
		email: String,
		age: i32,
		created_at: DateTime,
	}

	const TEST_USER_TABLE: TableName = TableName::new_const("test_user");

	#[derive(Debug, Clone)]
	struct TestUserFields;

	impl crate::orm::model::FieldSelector for TestUserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestUser {
		type PrimaryKey = i64;
		type Fields = TestUserFields;

		fn table_name() -> &'static str {
			TEST_USER_TABLE.as_str()
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.id)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}
	}

	// These tests verify that the type system works correctly
	#[test]
	fn test_string_methods_compile() {
		let _lookup = Field::<TestUser, String>::new(vec!["email"])
			.lower()
			.contains("test");

		let _lookup = Field::<TestUser, String>::new(vec!["email"])
			.upper()
			.startswith("TEST");
	}

	#[test]
	fn test_numeric_methods_compile() {
		let _lookup = Field::<TestUser, i32>::new(vec!["age"]).abs().gte(18);
	}

	#[test]
	fn test_datetime_methods_compile() {
		let _lookup = Field::<TestUser, DateTime>::new(vec!["created_at"])
			.year()
			.eq(2025);
	}

	#[test]
	fn test_comparison_methods_compile() {
		let _lookup =
			Field::<TestUser, String>::new(vec!["email"]).eq("test@example.com".to_string());

		let _lookup = Field::<TestUser, i32>::new(vec!["age"]).gte(18);
	}
}
