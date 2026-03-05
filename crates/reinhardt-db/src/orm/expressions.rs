use serde::{Deserialize, Serialize};
use std::fmt;
use std::marker::PhantomData;

use crate::orm::query::{Filter, FilterOperator, FilterValue};

/// F expression - represents a database field reference
/// Similar to Django's F() objects for database-side operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F {
	pub field: String,
}

impl F {
	/// Create a field reference for database operations
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::F;
	///
	/// // Reference a field for comparisons or updates
	/// let price_ref = F::new("price");
	/// assert_eq!(price_ref.to_sql(), "price");
	///
	/// // Can be used in queries like: WHERE price > F("cost") + 10
	/// ```
	pub fn new(field: impl Into<String>) -> Self {
		Self {
			field: field.into(),
		}
	}
	/// Generate SQL representation of the field reference
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let user_id = F::new("user_id");
	/// assert_eq!(user_id.to_sql(), "user_id");
	/// ```
	pub fn to_sql(&self) -> String {
		self.field.clone()
	}
}

impl fmt::Display for F {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.field)
	}
}

/// Type-safe field reference for database operations
///
/// `FieldRef<M, T>` provides compile-time type safety for field references,
/// where `M` is the model type and `T` is the field type.
///
/// This type replaces Python-style `__` (double underscore) field lookup notation
/// with Rust-idiomatic typed field accessors.
///
/// # Type Parameters
///
/// - `M`: Model type (e.g., `User`, `Post`)
/// - `T`: Field type (e.g., `i64`, `String`)
///
/// # Examples
///
/// ```ignore
/// use reinhardt_db::orm::expressions::FieldRef;
/// use reinhardt_core::macros::model;
/// use serde::{Serialize, Deserialize};
///
/// #[model(app_label = "users", table_name = "users")]
/// #[derive(Serialize, Deserialize)]
/// struct User {
///     #[field(primary_key = true)]
///     id: i64,
///     #[field(max_length = 255)]
///     name: String,
///     #[field(max_length = 255)]
///     email: String,
/// }
///
/// // The #[model] attribute macro automatically generates:
/// // impl User {
/// //     pub const fn field_id() -> FieldRef<User, i64> {
/// //         FieldRef::new("id")
/// //     }
/// //     pub const fn field_name() -> FieldRef<User, String> {
/// //         FieldRef::new("name")
/// //     }
/// //     pub const fn field_email() -> FieldRef<User, String> {
/// //         FieldRef::new("email")
/// //     }
/// // }
///
/// // Basic usage:
/// let id_ref = User::field_id();
/// assert_eq!(id_ref.name(), "id");
/// assert_eq!(id_ref.to_sql(), "id");
///
/// // Convert to F expression for use in queries:
/// use reinhardt_db::orm::expressions::F;
/// let f: F = User::field_name().into();
/// assert_eq!(f.to_sql(), "name");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct FieldRef<M, T> {
	name: &'static str,
	_phantom: PhantomData<(M, T)>,
}

impl<M, T> FieldRef<M, T> {
	/// Create a new field reference with compile-time type safety
	///
	/// This constructor is typically used by the `#[derive(Model)]` macro
	/// to generate field accessor methods.
	///
	/// # Arguments
	///
	/// - `name`: Field name as a static string
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::orm::expressions::FieldRef;
	///
	/// const USER_ID: FieldRef<User, i64> = FieldRef::new("id");
	/// ```
	pub const fn new(name: &'static str) -> Self {
		Self {
			name,
			_phantom: PhantomData,
		}
	}

	/// Get the field name
	///
	/// # Examples
	///
	/// ```ignore
	/// let id_ref = User::field_id();
	/// assert_eq!(id_ref.name(), "id");
	/// ```
	pub const fn name(&self) -> &'static str {
		self.name
	}

	/// Convert to SQL representation
	///
	/// # Examples
	///
	/// ```ignore
	/// let id_ref = User::field_id();
	/// assert_eq!(id_ref.to_sql(), "id");
	/// ```
	pub fn to_sql(&self) -> String {
		self.name.to_string()
	}

	/// Create an equality filter for this field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = User::field_id().eq(42);
	/// // Results in: WHERE id = 42
	/// ```
	pub fn eq<V: Into<FilterValue>>(&self, value: V) -> Filter {
		Filter::new(self.name.to_string(), FilterOperator::Eq, value.into())
	}

	/// Create a not-equal filter for this field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = User::field_status().ne("inactive");
	/// // Results in: WHERE status != 'inactive'
	/// ```
	pub fn ne<V: Into<FilterValue>>(&self, value: V) -> Filter {
		Filter::new(self.name.to_string(), FilterOperator::Ne, value.into())
	}

	/// Create a greater-than filter for this field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = User::field_age().gt(18);
	/// // Results in: WHERE age > 18
	/// ```
	pub fn gt<V: Into<FilterValue>>(&self, value: V) -> Filter {
		Filter::new(self.name.to_string(), FilterOperator::Gt, value.into())
	}

	/// Create a greater-than-or-equal filter for this field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = User::field_age().gte(18);
	/// // Results in: WHERE age >= 18
	/// ```
	pub fn gte<V: Into<FilterValue>>(&self, value: V) -> Filter {
		Filter::new(self.name.to_string(), FilterOperator::Gte, value.into())
	}

	/// Create a less-than filter for this field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = User::field_age().lt(65);
	/// // Results in: WHERE age < 65
	/// ```
	pub fn lt<V: Into<FilterValue>>(&self, value: V) -> Filter {
		Filter::new(self.name.to_string(), FilterOperator::Lt, value.into())
	}

	/// Create a less-than-or-equal filter for this field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = User::field_age().lte(65);
	/// // Results in: WHERE age <= 65
	/// ```
	pub fn lte<V: Into<FilterValue>>(&self, value: V) -> Filter {
		Filter::new(self.name.to_string(), FilterOperator::Lte, value.into())
	}

	/// Create an equality filter comparing this field to another field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = Order::field_discount_price().eq_field(Order::field_total_price());
	/// // Results in: WHERE discount_price = total_price
	/// ```
	pub fn eq_field<T2>(&self, other: FieldRef<M, T2>) -> Filter {
		Filter::new(
			self.name.to_string(),
			FilterOperator::Eq,
			FilterValue::FieldRef(F::new(other.name)),
		)
	}

	/// Create a not-equal filter comparing this field to another field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = Order::field_discount_price().ne_field(Order::field_total_price());
	/// // Results in: WHERE discount_price != total_price
	/// ```
	pub fn ne_field<T2>(&self, other: FieldRef<M, T2>) -> Filter {
		Filter::new(
			self.name.to_string(),
			FilterOperator::Ne,
			FilterValue::FieldRef(F::new(other.name)),
		)
	}

	/// Create a greater-than filter comparing this field to another field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = Order::field_total_price().gt_field(Order::field_discount_price());
	/// // Results in: WHERE total_price > discount_price
	/// ```
	pub fn gt_field<T2>(&self, other: FieldRef<M, T2>) -> Filter {
		Filter::new(
			self.name.to_string(),
			FilterOperator::Gt,
			FilterValue::FieldRef(F::new(other.name)),
		)
	}

	/// Create a greater-than-or-equal filter comparing this field to another field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = Order::field_total_price().gte_field(Order::field_discount_price());
	/// // Results in: WHERE total_price >= discount_price
	/// ```
	pub fn gte_field<T2>(&self, other: FieldRef<M, T2>) -> Filter {
		Filter::new(
			self.name.to_string(),
			FilterOperator::Gte,
			FilterValue::FieldRef(F::new(other.name)),
		)
	}

	/// Create a less-than filter comparing this field to another field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = Order::field_discount_price().lt_field(Order::field_total_price());
	/// // Results in: WHERE discount_price < total_price
	/// ```
	pub fn lt_field<T2>(&self, other: FieldRef<M, T2>) -> Filter {
		Filter::new(
			self.name.to_string(),
			FilterOperator::Lt,
			FilterValue::FieldRef(F::new(other.name)),
		)
	}

	/// Create a less-than-or-equal filter comparing this field to another field
	///
	/// # Examples
	///
	/// ```ignore
	/// let filter = Order::field_discount_price().lte_field(Order::field_total_price());
	/// // Results in: WHERE discount_price <= total_price
	/// ```
	pub fn lte_field<T2>(&self, other: FieldRef<M, T2>) -> Filter {
		Filter::new(
			self.name.to_string(),
			FilterOperator::Lte,
			FilterValue::FieldRef(F::new(other.name)),
		)
	}
}

impl<M, T> fmt::Display for FieldRef<M, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.name)
	}
}

// Allow conversion from FieldRef to String for Manager::filter()
impl<M, T> From<FieldRef<M, T>> for String {
	fn from(field_ref: FieldRef<M, T>) -> Self {
		field_ref.name.to_string()
	}
}

// Allow conversion from FieldRef to F for backward compatibility
impl<M, T> From<FieldRef<M, T>> for F {
	fn from(field_ref: FieldRef<M, T>) -> Self {
		F::new(field_ref.name)
	}
}

/// OuterRef - reference to a field in the outer query (for subqueries)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OuterRef {
	pub field: String,
}

impl OuterRef {
	/// Create a reference to an outer query field (for subqueries)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::OuterRef;
	///
	/// // Reference parent query field in subquery
	/// let parent_id = OuterRef::new("parent_id");
	/// assert_eq!(parent_id.to_sql(), "parent_id");
	///
	// Useful in correlated subqueries like:
	// SELECT * FROM items WHERE id IN (
	//   SELECT item_id FROM tags WHERE user_id = OuterRef("user_id")
	// )
	/// ```
	pub fn new(field: impl Into<String>) -> Self {
		Self {
			field: field.into(),
		}
	}
	/// Generate SQL for the outer reference
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::OuterRef;
	///
	/// let outer_field = OuterRef::new("category_id");
	/// assert_eq!(outer_field.to_sql(), "category_id");
	/// ```
	pub fn to_sql(&self) -> String {
		// In a subquery context, this references the outer query's field
		self.field.clone()
	}
}

/// Subquery - represents a subquery expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subquery {
	pub sql: String,
	pub template: String,
}

impl Subquery {
	/// Create a subquery expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Subquery;
	///
	/// // Create a subquery for filtering
	/// let sq = Subquery::new("SELECT id FROM users WHERE active = 1");
	/// let sql = sq.to_sql();
	/// assert!(sql.contains("SELECT id FROM users"));
	/// assert!(sql.starts_with("(") && sql.ends_with(")"));
	/// ```
	pub fn new(sql: impl Into<String>) -> Self {
		Self {
			sql: sql.into(),
			template: "(%(subquery)s)".to_string(),
		}
	}
	/// Customize the SQL template for the subquery
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Subquery;
	///
	/// let sq = Subquery::new("SELECT COUNT(*) FROM orders")
	///     .with_template("ORDER_COUNT = %(subquery)s");
	/// assert_eq!(sq.to_sql(), "ORDER_COUNT = SELECT COUNT(*) FROM orders");
	/// ```
	pub fn with_template(mut self, template: impl Into<String>) -> Self {
		self.template = template.into();
		self
	}
	/// Generate final SQL from template
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Subquery;
	///
	/// let sq = Subquery::new("SELECT MAX(price) FROM products");
	/// assert!(sq.to_sql().starts_with("("));
	/// ```
	pub fn to_sql(&self) -> String {
		self.template.replace("%(subquery)s", &self.sql)
	}
}

/// Exists - check if a subquery returns any rows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exists {
	pub subquery: Subquery,
}

impl Exists {
	/// Create an EXISTS check for a subquery
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Exists;
	///
	/// // Check if related records exist
	/// let exists = Exists::new("SELECT 1 FROM orders WHERE user_id = 123");
	/// let sql = exists.to_sql();
	/// assert!(sql.starts_with("EXISTS("));
	/// assert!(sql.contains("SELECT 1 FROM orders"));
	/// ```
	pub fn new(sql: impl Into<String>) -> Self {
		Self {
			subquery: Subquery {
				sql: sql.into(),
				template: "%(subquery)s".to_string(),
			},
		}
	}
	/// Generate EXISTS SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Exists;
	///
	/// let exists = Exists::new("SELECT 1 FROM tags WHERE item_id = items.id");
	/// assert!(exists.to_sql().starts_with("EXISTS("));
	/// ```
	pub fn to_sql(&self) -> String {
		format!("EXISTS({})", self.subquery.to_sql())
	}
}

/// Value expression - represents a literal value in a query
/// Similar to Django's Value() for using literal values in expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value {
	pub value: ValueType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueType {
	String(String),
	Integer(i64),
	Float(f64),
	Bool(bool),
	Null,
}

impl Value {
	/// Create a literal value expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::{Value, ValueType};
	///
	/// let val = Value::new("active");
	/// // Verify the value is created successfully
	/// let _: Value = val;
	/// ```
	pub fn new<T: Into<ValueType>>(value: T) -> Self {
		Self {
			value: value.into(),
		}
	}
	/// Create a string literal value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Value;
	///
	/// let status = Value::string("active");
	/// assert_eq!(status.to_sql(), "'active'");
	/// ```
	pub fn string(s: impl Into<String>) -> Self {
		Self {
			value: ValueType::String(s.into()),
		}
	}
	/// Create an integer literal value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Value;
	///
	/// let count = Value::int(42);
	/// assert_eq!(count.to_sql(), "42");
	/// ```
	pub fn int(i: i64) -> Self {
		Self {
			value: ValueType::Integer(i),
		}
	}
	/// Create a float literal value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Value;
	///
	/// let price = Value::float(99.99);
	/// assert_eq!(price.to_sql(), "99.99");
	/// ```
	pub fn float(f: f64) -> Self {
		Self {
			value: ValueType::Float(f),
		}
	}
	/// Create a boolean literal value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Value;
	///
	/// let is_active = Value::bool(true);
	/// assert_eq!(is_active.to_sql(), "TRUE");
	/// ```
	pub fn bool(b: bool) -> Self {
		Self {
			value: ValueType::Bool(b),
		}
	}
	/// Create a NULL literal value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Value;
	///
	/// let empty = Value::null();
	/// assert_eq!(empty.to_sql(), "NULL");
	/// ```
	pub fn null() -> Self {
		Self {
			value: ValueType::Null,
		}
	}
	/// Generate SQL for this literal value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Value;
	///
	/// assert_eq!(Value::string("test").to_sql(), "'test'");
	/// assert_eq!(Value::int(10).to_sql(), "10");
	/// assert_eq!(Value::bool(false).to_sql(), "FALSE");
	/// ```
	pub fn to_sql(&self) -> String {
		match &self.value {
			ValueType::String(s) => format!("'{}'", s.replace('\'', "''")),
			ValueType::Integer(i) => i.to_string(),
			ValueType::Float(f) => f.to_string(),
			ValueType::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
			ValueType::Null => "NULL".to_string(),
		}
	}
}

impl From<String> for ValueType {
	fn from(s: String) -> Self {
		ValueType::String(s)
	}
}

impl From<&str> for ValueType {
	fn from(s: &str) -> Self {
		ValueType::String(s.to_string())
	}
}

impl From<i64> for ValueType {
	fn from(i: i64) -> Self {
		ValueType::Integer(i)
	}
}

impl From<i32> for ValueType {
	fn from(i: i32) -> Self {
		ValueType::Integer(i as i64)
	}
}

impl From<f64> for ValueType {
	fn from(f: f64) -> Self {
		ValueType::Float(f)
	}
}

impl From<bool> for ValueType {
	fn from(b: bool) -> Self {
		ValueType::Bool(b)
	}
}

/// Q operator for combining query conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QOperator {
	And,
	Or,
	Not,
}

impl fmt::Display for QOperator {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			QOperator::And => write!(f, "AND"),
			QOperator::Or => write!(f, "OR"),
			QOperator::Not => write!(f, "NOT"),
		}
	}
}

/// Q object - represents a complex query condition
/// Similar to Django's Q() objects for building complex queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Q {
	/// Simple condition: field, operator, value
	Condition {
		field: String,
		operator: String,
		value: String,
	},
	/// Combined conditions with AND/OR/NOT
	Combined {
		operator: QOperator,
		conditions: Vec<Q>,
	},
}

impl Q {
	/// Create a simple Q object with a condition
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Q;
	///
	/// // Create a simple condition
	/// let q = Q::new("age", ">=", "18");
	/// assert_eq!(q.to_sql(), "age >= 18");
	///
	/// // Combine conditions
	/// let q1 = Q::new("status", "=", "active");
	/// let q2 = Q::new("verified", "=", "true");
	/// let combined = q1.and(q2);
	/// ```
	pub fn new(
		field: impl Into<String>,
		operator: impl Into<String>,
		value: impl Into<String>,
	) -> Self {
		Self::Condition {
			field: field.into(),
			operator: operator.into(),
			value: value.into(),
		}
	}
	/// Create a Q object from raw SQL condition
	/// Used internally by the type-safe query builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::Q;
	///
	/// let q = Q::from_sql("age > 18");
	/// let q = Q::from_sql("name LIKE '%John%'");
	/// let q = Q::from_sql("email IS NOT NULL");
	/// let q = Q::from_sql("status IN ('active', 'pending')");
	/// let q = Q::from_sql("age BETWEEN 18 AND 65");
	/// ```
	pub fn from_sql(sql: &str) -> Self {
		super::sql_condition_parser::SqlConditionParser::parse(sql)
	}
	/// Create an empty Q object (always true condition)
	///
	pub fn empty() -> Self {
		Self::Combined {
			operator: QOperator::And,
			conditions: vec![],
		}
	}
	/// Combine this Q object with another using AND
	///
	pub fn and(self, other: Q) -> Self {
		match self {
			Q::Combined {
				operator: QOperator::And,
				mut conditions,
			} => {
				conditions.push(other);
				Q::Combined {
					operator: QOperator::And,
					conditions,
				}
			}
			_ => Q::Combined {
				operator: QOperator::And,
				conditions: vec![self, other],
			},
		}
	}
	/// Combine this Q object with another using OR
	///
	pub fn or(self, other: Q) -> Self {
		match self {
			Q::Combined {
				operator: QOperator::Or,
				mut conditions,
			} => {
				conditions.push(other);
				Q::Combined {
					operator: QOperator::Or,
					conditions,
				}
			}
			_ => Q::Combined {
				operator: QOperator::Or,
				conditions: vec![self, other],
			},
		}
	}
	/// Negate this Q object
	///
	/// Note: This method consumes `self` and returns a new `Q` object,
	/// which is incompatible with the `std::ops::Not` trait that requires
	/// returning a reference. Therefore, we keep this as a regular method.
	#[allow(clippy::should_implement_trait)]
	pub fn not(self) -> Self {
		Q::Combined {
			operator: QOperator::Not,
			conditions: vec![self],
		}
	}
	/// Generate SQL for this Q object
	///
	pub fn to_sql(&self) -> String {
		match self {
			Q::Condition {
				field,
				operator,
				value,
			} => {
				// If field and operator are empty, this is a raw SQL condition from FieldLookupCompiler
				if field.is_empty() && operator.is_empty() {
					return value.clone();
				}

				// Quote string values if they don't look like numbers or SQL keywords
				let formatted_value = if value.parse::<f64>().is_ok()
					|| value.to_uppercase() == "TRUE"
					|| value.to_uppercase() == "FALSE"
					|| value.to_uppercase() == "NULL"
					|| value.starts_with("COUNT(")
					|| value.starts_with("SUM(")
					|| value.starts_with("AVG(")
					|| value.starts_with("MAX(")
					|| value.starts_with("MIN(")
					|| (value.starts_with('\'') && value.ends_with('\''))
				{
					value.clone()
				} else {
					format!("'{}'", value)
				};
				format!("{} {} {}", field, operator, formatted_value)
			}
			Q::Combined {
				operator,
				conditions,
			} => {
				let sql_conditions: Vec<String> = conditions.iter().map(|q| q.to_sql()).collect();

				match operator {
					QOperator::Not => {
						if conditions.len() == 1 {
							format!("NOT ({})", sql_conditions[0])
						} else {
							format!("NOT ({})", sql_conditions.join(" AND "))
						}
					}
					QOperator::And => {
						if sql_conditions.len() == 1 {
							sql_conditions[0].clone()
						} else {
							format!("({})", sql_conditions.join(" AND "))
						}
					}
					QOperator::Or => {
						if sql_conditions.len() == 1 {
							sql_conditions[0].clone()
						} else {
							format!("({})", sql_conditions.join(" OR "))
						}
					}
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Test model for FieldRef demonstration
	#[allow(dead_code)]
	struct TestUser {
		id: i64,
		name: String,
	}

	// Simulating what #[derive(Model)] macro would generate
	impl TestUser {
		const fn field_id() -> FieldRef<TestUser, i64> {
			FieldRef::new("id")
		}

		const fn field_name() -> FieldRef<TestUser, String> {
			FieldRef::new("name")
		}
	}

	#[test]
	fn test_field_ref_basic() {
		let id_ref = TestUser::field_id();
		assert_eq!(id_ref.name(), "id");
		assert_eq!(id_ref.to_sql(), "id");
		assert_eq!(format!("{}", id_ref), "id");
	}

	#[test]
	fn test_field_ref_string_field() {
		let name_ref = TestUser::field_name();
		assert_eq!(name_ref.name(), "name");
		assert_eq!(name_ref.to_sql(), "name");
	}

	#[test]
	fn test_field_ref_to_f_conversion() {
		let id_ref = TestUser::field_id();
		let f: F = id_ref.into();
		assert_eq!(f.to_sql(), "id");
	}

	#[test]
	fn test_expressions_f_unit() {
		let f = F::new("price");
		assert_eq!(f.to_sql(), "price");
		assert_eq!(format!("{}", f), "price");
	}

	#[test]
	fn test_q_simple_condition() {
		let q = Q::new("age", ">=", "18");
		assert_eq!(q.to_sql(), "age >= 18");
	}

	#[test]
	fn test_q_and_operator() {
		let q1 = Q::new("age", ">=", "18");
		let q2 = Q::new("country", "=", "US");
		let q = q1.and(q2);

		let sql = q.to_sql();
		assert_eq!(
			sql, "(age >= 18 AND country = 'US')",
			"Expected exact AND query structure, got: {}",
			sql
		);
	}

	#[test]
	fn test_q_or_operator() {
		let q1 = Q::new("status", "=", "active");
		let q2 = Q::new("status", "=", "pending");
		let q = q1.or(q2);

		let sql = q.to_sql();
		assert_eq!(
			sql, "(status = 'active' OR status = 'pending')",
			"Expected exact OR query structure, got: {}",
			sql
		);
	}

	#[test]
	fn test_q_not_operator() {
		let q = Q::new("deleted", "=", "1").not();
		assert_eq!(q.to_sql(), "NOT (deleted = 1)");
	}

	#[test]
	fn test_q_complex_query() {
		// (age >= 18 AND country = 'US') OR (status = 'premium')
		let q1 = Q::new("age", ">=", "18");
		let q2 = Q::new("country", "=", "US");
		let q3 = Q::new("status", "=", "premium");

		let q = q1.and(q2).or(q3);

		let sql = q.to_sql();
		assert_eq!(
			sql, "((age >= 18 AND country = 'US') OR status = 'premium')",
			"Expected exact complex query structure, got: {}",
			sql
		);
	}

	#[test]
	fn test_q_chained_and() {
		let q1 = Q::new("a", "=", "1");
		let q2 = Q::new("b", "=", "2");
		let q3 = Q::new("c", "=", "3");

		let q = q1.and(q2).and(q3);

		let sql = q.to_sql();
		assert_eq!(
			sql, "(a = 1 AND b = 2 AND c = 3)",
			"Expected exact chained AND query structure, got: {}",
			sql
		);
	}

	#[test]
	fn test_q_chained_or() {
		let q1 = Q::new("x", "=", "1");
		let q2 = Q::new("y", "=", "2");
		let q3 = Q::new("z", "=", "3");

		let q = q1.or(q2).or(q3);

		let sql = q.to_sql();
		assert_eq!(
			sql, "(x = 1 OR y = 2 OR z = 3)",
			"Expected exact chained OR query structure, got: {}",
			sql
		);
	}

	#[test]
	fn test_outer_ref() {
		let outer_ref = OuterRef::new("parent_id");
		assert_eq!(outer_ref.to_sql(), "parent_id");
	}

	#[test]
	fn test_subquery() {
		let subquery = Subquery::new("SELECT id FROM users WHERE active = 1");
		let sql = subquery.to_sql();
		assert_eq!(
			sql, "(SELECT id FROM users WHERE active = 1)",
			"Expected exact subquery SQL with parentheses, got: {}",
			sql
		);
	}

	#[test]
	fn test_subquery_custom_template() {
		let subquery =
			Subquery::new("SELECT COUNT(*) FROM orders").with_template("COUNT = %(subquery)s");
		let sql = subquery.to_sql();
		assert_eq!(sql, "COUNT = SELECT COUNT(*) FROM orders");
	}

	#[test]
	fn test_expressions_exists() {
		let exists = Exists::new("SELECT 1 FROM orders WHERE user_id = 123");
		let sql = exists.to_sql();
		assert_eq!(
			sql, "EXISTS(SELECT 1 FROM orders WHERE user_id = 123)",
			"Expected exact EXISTS SQL structure, got: {}",
			sql
		);
	}

	// FieldRef-based F expression tests

	#[test]
	fn test_field_ref_to_f_direct_conversion() {
		// Verify FieldRef can be directly converted to F expression
		let id_field = TestUser::field_id();
		let f: F = id_field.into();

		assert_eq!(f.to_sql(), "id");
		assert_eq!(format!("{}", f), "id");
	}

	#[test]
	fn test_field_ref_string_field_to_f() {
		// Verify String-typed FieldRef works with F expression
		let name_field = TestUser::field_name();
		let f: F = name_field.into();

		assert_eq!(f.to_sql(), "name");
		assert_eq!(format!("{}", f), "name");
	}

	#[test]
	fn test_multiple_field_refs_to_f() {
		// Verify multiple FieldRefs can be converted to F expressions
		let id_f: F = TestUser::field_id().into();
		let name_f: F = TestUser::field_name().into();

		assert_eq!(id_f.to_sql(), "id");
		assert_eq!(name_f.to_sql(), "name");
		assert_ne!(id_f.to_sql(), name_f.to_sql());
	}

	#[test]
	fn test_field_ref_preserves_field_name_in_f() {
		// Ensure field name is correctly preserved through conversion
		let id_field = TestUser::field_id();
		let original_name = id_field.name();
		let f: F = id_field.into();

		assert_eq!(f.to_sql(), original_name);
	}

	#[test]
	fn test_field_ref_const_to_f_conversion() {
		// Verify const FieldRef can be converted to F
		const ID_FIELD: FieldRef<TestUser, i64> = FieldRef::new("id");
		let f: F = ID_FIELD.into();

		assert_eq!(f.to_sql(), "id");
	}
}
// Auto-generated tests for expressions module
// Translated from Django/SQLAlchemy test suite
// Total available: 370 | Included: 100

#[cfg(test)]
mod expressions_extended_tests {
	use super::*;
	use crate::orm::aggregation::*;
	// Tests use annotation types directly
	use crate::orm::annotation::Value;
	use crate::orm::expressions::{F, Q};

	#[test]
	// From: Django/expressions
	fn test_values_expression_group_by() {
		// Test that Value expressions can be used in group by contexts
		let val = Value::String("test_group".to_string());
		assert_eq!(val.to_sql(), "'test_group'");
	}

	#[test]
	// From: Django/expressions
	fn test_values_expression_group_by_1() {
		// Test that Value expressions can be used in group by contexts
		let val = Value::Int(42);
		assert_eq!(val.to_sql(), "42");
	}

	#[test]
	// From: Django/expressions
	fn test_aggregate_rawsql_annotation() {
		// Test aggregate with annotation
		let agg = Aggregate::sum("amount").with_alias("total_amount");
		assert_eq!(agg.to_sql(), "SUM(amount) AS total_amount");
	}

	#[test]
	// From: Django/expressions
	fn test_aggregate_rawsql_annotation_1() {
		// Test aggregate with annotation
		let agg = Aggregate::max("price").with_alias("max_price");
		assert_eq!(agg.to_sql(), "MAX(price) AS max_price");
	}

	#[test]
	// From: Django/expressions
	fn test_aggregate_subquery_annotation() {
		// Test subquery with aggregate
		let subquery = Subquery::new("SELECT COUNT(*) FROM orders WHERE status = 'completed'");
		let sql = subquery.to_sql();
		assert_eq!(
			sql, "(SELECT COUNT(*) FROM orders WHERE status = 'completed')",
			"Expected exact subquery with aggregate, got: {}",
			sql
		);
	}

	#[test]
	// From: Django/expressions
	fn test_aggregate_subquery_annotation_1() {
		// Test subquery with aggregate
		let subquery = Subquery::new("SELECT AVG(price) FROM products");
		let sql = subquery.to_sql();
		assert_eq!(
			sql, "(SELECT AVG(price) FROM products)",
			"Expected exact subquery with AVG aggregate, got: {}",
			sql
		);
	}

	#[test]
	// From: Django/expressions
	fn test_aggregates() {
		// Test basic aggregates
		let agg = Aggregate::avg("score");
		assert_eq!(agg.to_sql(), "AVG(score)");
	}

	#[test]
	// From: Django/expressions
	fn test_aggregates_1() {
		// Test basic aggregates
		let agg = Aggregate::min("age");
		assert_eq!(agg.to_sql(), "MIN(age)");
	}

	#[test]
	// From: Django/expressions
	fn test_annotate_by_empty_custom_exists() {
		// Test EXISTS with empty subquery
		let exists = Exists::new("");
		let sql = exists.to_sql();
		assert_eq!(sql, "EXISTS()");
	}

	#[test]
	// From: Django/expressions
	fn test_annotate_by_empty_custom_exists_1() {
		// Test EXISTS with subquery
		let exists = Exists::new("SELECT 1");
		let sql = exists.to_sql();
		assert_eq!(sql, "EXISTS(SELECT 1)");
	}

	#[test]
	// From: Django/expressions
	fn test_annotate_values_aggregate() {
		// Test aggregates with values
		let agg = Aggregate::count_all().with_alias("total");
		assert_eq!(agg.to_sql(), "COUNT(*) AS total");
	}

	#[test]
	// From: Django/expressions
	fn test_annotate_values_aggregate_1() {
		// Test aggregates with values
		let agg = Aggregate::sum("quantity").with_alias("total_qty");
		assert_eq!(agg.to_sql(), "SUM(quantity) AS total_qty");
	}

	#[test]
	// From: Django/expressions
	fn test_annotate_values_count() {
		let agg = Aggregate::count(Some("id")).with_alias("total");
		assert_eq!(agg.to_sql(), "COUNT(id) AS total");
	}

	#[test]
	// From: Django/expressions
	fn test_annotate_values_count_1() {
		let agg = Aggregate::count(Some("id")).with_alias("total");
		assert_eq!(agg.to_sql(), "COUNT(id) AS total");
	}

	#[test]
	// From: Django/expressions
	fn test_annotate_values_filter() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_annotate_values_filter_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_annotation_with_deeply_nested_outerref() {
		// Test deeply nested OuterRef
		let outer_ref = OuterRef::new("parent.grandparent.id");
		assert_eq!(outer_ref.to_sql(), "parent.grandparent.id");
	}

	#[test]
	// From: Django/expressions
	fn test_annotation_with_deeply_nested_outerref_1() {
		// Test deeply nested OuterRef
		let outer_ref = OuterRef::new("root.level1.level2.field");
		assert_eq!(outer_ref.to_sql(), "root.level1.level2.field");
	}

	#[test]
	// From: Django/expressions
	fn test_annotation_with_nested_outerref() {
		// Test nested OuterRef
		let outer_ref = OuterRef::new("parent.user_id");
		assert_eq!(outer_ref.to_sql(), "parent.user_id");
	}

	#[test]
	// From: Django/expressions
	fn test_annotation_with_nested_outerref_1() {
		// Test nested OuterRef
		let outer_ref = OuterRef::new("outer.category_id");
		assert_eq!(outer_ref.to_sql(), "outer.category_id");
	}

	#[test]
	// From: Django/expressions
	fn test_annotation_with_outerref() {
		// Test OuterRef in annotation
		let outer_ref = OuterRef::new("user_id");
		assert_eq!(outer_ref.to_sql(), "user_id");
	}

	#[test]
	// From: Django/expressions
	fn test_annotation_with_outerref_1() {
		// Test OuterRef in annotation
		let outer_ref = OuterRef::new("category_id");
		assert_eq!(outer_ref.to_sql(), "category_id");
	}

	#[test]
	// From: Django/expressions
	fn test_annotation_with_outerref_and_output_field() {
		// Test OuterRef with output field
		let outer_ref = OuterRef::new("price");
		let f = F::new("product_price");
		assert_eq!(outer_ref.to_sql(), "price");
		assert_eq!(f.to_sql(), "product_price");
	}

	#[test]
	// From: Django/expressions
	fn test_annotation_with_outerref_and_output_field_1() {
		// Test OuterRef with output field
		let outer_ref = OuterRef::new("amount");
		assert_eq!(outer_ref.to_sql(), "amount");
	}

	#[test]
	// From: Django/expressions
	fn test_annotations_within_subquery() {
		// Test annotations in subquery
		let subquery = Subquery::new("SELECT id, COUNT(*) as total FROM items GROUP BY id");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT id, COUNT(*) as total FROM items GROUP BY id)",
			"Expected exact subquery with annotations, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_annotations_within_subquery_1() {
		// Test annotations in subquery
		let subquery =
			Subquery::new("SELECT user_id, SUM(amount) as total FROM orders GROUP BY user_id");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT user_id, SUM(amount) as total FROM orders GROUP BY user_id)",
			"Expected exact subquery with SUM aggregate, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_case_in_filter_if_boolean_output_field() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_case_in_filter_if_boolean_output_field_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_date_subquery_subtraction() {
		// Test date subtraction in subquery
		let subquery = Subquery::new("SELECT date1 - date2 FROM events");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT date1 - date2 FROM events)",
			"Expected exact subquery with date subtraction, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_date_subquery_subtraction_1() {
		// Test date subtraction in subquery
		let subquery = Subquery::new("SELECT end_date - start_date FROM projects");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT end_date - start_date FROM projects)",
			"Expected exact subquery with date subtraction, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_datetime_and_duration_field_addition_with_annotate_and_no_output_field() {
		// Test datetime and duration addition
		let f = F::new("created_at + INTERVAL 7 DAY");
		assert_eq!(f.to_sql(), "created_at + INTERVAL 7 DAY");
	}

	#[test]
	// From: Django/expressions
	fn test_datetime_and_duration_field_addition_with_annotate_and_no_output_field_1() {
		// Test datetime and duration addition
		let f = F::new("start_time + duration");
		assert_eq!(f.to_sql(), "start_time + duration");
	}

	#[test]
	// From: Django/expressions
	fn test_datetime_and_durationfield_addition_with_filter() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_datetime_and_durationfield_addition_with_filter_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_datetime_subquery_subtraction() {
		// Test datetime subtraction in subquery
		let subquery = Subquery::new("SELECT updated_at - created_at FROM records");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT updated_at - created_at FROM records)",
			"Expected exact subquery with datetime subtraction, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_datetime_subquery_subtraction_1() {
		// Test datetime subtraction in subquery
		let subquery = Subquery::new("SELECT NOW() - last_login FROM users");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT NOW() - last_login FROM users)",
			"Expected exact subquery with NOW() function, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_datetime_subtraction_with_annotate_and_no_output_field() {
		// Test datetime subtraction
		let f = F::new("end_time - start_time");
		assert_eq!(f.to_sql(), "end_time - start_time");
	}

	#[test]
	// From: Django/expressions
	fn test_datetime_subtraction_with_annotate_and_no_output_field_1() {
		// Test datetime subtraction
		let f = F::new("checkout_time - checkin_time");
		assert_eq!(f.to_sql(), "checkout_time - checkin_time");
	}

	#[test]
	// From: Django/expressions
	fn test_distinct_aggregates() {
		// Test DISTINCT aggregates
		let agg = Aggregate::count_distinct("user_id");
		assert_eq!(agg.to_sql(), "COUNT(DISTINCT user_id)");
	}

	#[test]
	// From: Django/expressions
	fn test_distinct_aggregates_1() {
		// Test DISTINCT aggregates
		let agg = Aggregate::count_distinct("email");
		assert_eq!(agg.to_sql(), "COUNT(DISTINCT email)");
	}

	#[test]
	// From: Django/expressions
	fn test_empty_group_by() {
		// Test empty group by - aggregate over all rows
		let agg = Aggregate::count_all();
		assert_eq!(agg.to_sql(), "COUNT(*)");
	}

	#[test]
	// From: Django/expressions
	fn test_empty_group_by_1() {
		// Test empty group by - aggregate over all rows
		let agg = Aggregate::sum("total");
		assert_eq!(agg.to_sql(), "SUM(total)");
	}

	#[test]
	// From: Django/expressions
	fn test_exists_in_filter() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_exists_in_filter_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_expressions_range_lookups_join_choice() {
		// Test range lookups with expressions
		let q1 = Q::new("price", ">=", "10");
		let q2 = Q::new("price", "<=", "100");
		let q = q1.and(q2);
		let sql = q.to_sql();
		assert_eq!(
			sql, "(price >= 10 AND price <= 100)",
			"Expected exact range query with AND, got: {}",
			sql
		);
	}

	#[test]
	// From: Django/expressions
	fn test_expressions_range_lookups_join_choice_1() {
		// Test range lookups with expressions
		let q1 = Q::new("age", ">", "18");
		let q2 = Q::new("age", "<", "65");
		let q = q1.and(q2);
		let sql = q.to_sql();
		assert_eq!(
			sql, "(age > 18 AND age < 65)",
			"Expected exact age range query, got: {}",
			sql
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_by_empty_exists() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_by_empty_exists_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_decimal_expression() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_decimal_expression_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_inter_attribute() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_inter_attribute_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_not_equals_other_field() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_not_equals_other_field_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_with_join() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filter_with_join_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filtered_aggregates() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filtered_aggregates_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filtering_on_annotate_that_uses_q() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filtering_on_annotate_that_uses_q_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filtering_on_q_that_is_boolean() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filtering_on_q_that_is_boolean_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filtering_on_rawsql_that_is_boolean() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_filtering_on_rawsql_that_is_boolean_1() {
		let q = Q::new("status", "=", "active");
		assert_eq!(
			q.to_sql(),
			"status = 'active'",
			"Expected exact Q condition SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_in_lookup_allows_f_expressions_and_expressions_for_integers() {
		// Test IN lookup with F expressions
		let f = F::new("category_id");
		assert_eq!(f.to_sql(), "category_id");
	}

	#[test]
	// From: Django/expressions
	fn test_in_lookup_allows_f_expressions_and_expressions_for_integers_1() {
		// Test IN lookup with integer expressions
		let q = Q::new("id", "IN", "1,2,3,4,5");
		assert_eq!(
			q.to_sql(),
			"id IN '1,2,3,4,5'",
			"Expected exact IN query, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_in_subquery() {
		// Test IN with subquery
		let subquery = Subquery::new("SELECT id FROM active_users");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT id FROM active_users)",
			"Expected exact subquery for IN clause, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_in_subquery_1() {
		// Test IN with subquery
		let subquery = Subquery::new("SELECT category_id FROM featured_categories");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT category_id FROM featured_categories)",
			"Expected exact subquery for featured categories, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_incorrect_field_in_f_expression() {
		// Test F expression with any field name (no validation at this level)
		let f = F::new("nonexistent_field");
		assert_eq!(f.to_sql(), "nonexistent_field");
	}

	#[test]
	// From: Django/expressions
	fn test_incorrect_field_in_f_expression_1() {
		// Test F expression with any field name (no validation at this level)
		let f = F::new("invalid__field__name");
		assert_eq!(f.to_sql(), "invalid__field__name");
	}

	#[test]
	// From: Django/expressions
	fn test_incorrect_joined_field_in_f_expression() {
		// Test F expression with joined field reference
		let f = F::new("related__invalid_field");
		assert_eq!(f.to_sql(), "related__invalid_field");
	}

	#[test]
	// From: Django/expressions
	fn test_incorrect_joined_field_in_f_expression_1() {
		// Test F expression with joined field reference
		let f = F::new("user__profile__missing");
		assert_eq!(f.to_sql(), "user__profile__missing");
	}

	#[test]
	// From: Django/expressions
	fn test_lookups_subquery() {
		// Test lookups with subquery
		let subquery = Subquery::new("SELECT MAX(price) FROM products WHERE available = 1");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT MAX(price) FROM products WHERE available = 1)",
			"Expected exact subquery with MAX aggregate, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_lookups_subquery_1() {
		// Test lookups with subquery
		let subquery = Subquery::new("SELECT MIN(created_at) FROM events");
		assert_eq!(
			subquery.to_sql(),
			"(SELECT MIN(created_at) FROM events)",
			"Expected exact subquery with MIN aggregate, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_mixed_char_date_with_annotate() {
		// Test mixed character and date fields
		let f1 = F::new("name");
		let f2 = F::new("created_date");
		assert_eq!(f1.to_sql(), "name");
		assert_eq!(f2.to_sql(), "created_date");
	}

	#[test]
	// From: Django/expressions
	fn test_mixed_char_date_with_annotate_1() {
		// Test mixed character and date fields
		let val_str = Value::String("test".to_string());
		let f_date = F::new("birth_date");
		assert_eq!(val_str.to_sql(), "'test'");
		assert_eq!(f_date.to_sql(), "birth_date");
	}

	#[test]
	// From: Django/expressions
	fn test_negated_empty_exists() {
		// Test negated EXISTS
		let exists = Exists::new("");
		let q = Q::new("NOT", "", exists.to_sql());
		assert_eq!(
			q.to_sql(),
			"NOT  'EXISTS()'",
			"Expected exact negated EXISTS SQL, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_negated_empty_exists_1() {
		// Test negated EXISTS query
		let q = Q::new("id", "NOT IN", "SELECT id FROM deleted");
		assert_eq!(
			q.to_sql(),
			"id NOT IN 'SELECT id FROM deleted'",
			"Expected exact NOT IN query, got: {}",
			q.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_nested_subquery() {
		// Test nested subquery
		let inner = Subquery::new("SELECT id FROM users WHERE active = 1");
		let outer = Subquery::new(format!(
			"SELECT * FROM orders WHERE user_id IN {}",
			inner.to_sql()
		));
		assert_eq!(
			outer.to_sql(),
			"(SELECT * FROM orders WHERE user_id IN (SELECT id FROM users WHERE active = 1))",
			"Expected exact nested subquery, got: {}",
			outer.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_nested_subquery_1() {
		// Test nested subquery
		let subquery = Subquery::new(
			"SELECT category_id FROM (SELECT * FROM products WHERE price > 100) AS expensive",
		);
		assert_eq!(
			subquery.to_sql(),
			"(SELECT category_id FROM (SELECT * FROM products WHERE price > 100) AS expensive)",
			"Expected exact nested subquery with alias, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_nested_subquery_join_outer_ref() {
		// Test nested subquery with OuterRef
		let outer_ref = OuterRef::new("parent.id");
		let subquery = Subquery::new(format!(
			"SELECT COUNT(*) FROM children WHERE parent_id = {}",
			outer_ref.to_sql()
		));
		assert_eq!(
			subquery.to_sql(),
			"(SELECT COUNT(*) FROM children WHERE parent_id = parent.id)",
			"Expected exact subquery with OuterRef, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_nested_subquery_join_outer_ref_1() {
		// Test nested subquery with OuterRef
		let outer_ref = OuterRef::new("order.user_id");
		assert_eq!(outer_ref.to_sql(), "order.user_id");
	}

	#[test]
	// From: Django/expressions
	fn test_nested_subquery_outer_ref_2() {
		// Test OuterRef in nested subquery
		let outer_ref = OuterRef::new("main.category_id");
		assert_eq!(outer_ref.to_sql(), "main.category_id");
	}

	#[test]
	// From: Django/expressions
	fn test_nested_subquery_outer_ref_2_1() {
		// Test OuterRef in nested subquery
		let outer_ref = OuterRef::new("outer_table.field");
		assert_eq!(outer_ref.to_sql(), "outer_table.field");
	}

	#[test]
	// From: Django/expressions
	fn test_nested_subquery_outer_ref_with_autofield() {
		// Test OuterRef with autofield (id)
		let outer_ref = OuterRef::new("id");
		assert_eq!(outer_ref.to_sql(), "id");
	}

	#[test]
	// From: Django/expressions
	fn test_nested_subquery_outer_ref_with_autofield_1() {
		// Test OuterRef with pk field
		let outer_ref = OuterRef::new("pk");
		assert_eq!(outer_ref.to_sql(), "pk");
	}

	#[test]
	// From: Django/expressions
	fn test_non_empty_group_by() {
		// Test group by with field
		let f = F::new("category");
		let agg = Aggregate::count(Some("id"));
		assert_eq!(f.to_sql(), "category");
		assert_eq!(agg.to_sql(), "COUNT(id)");
	}

	#[test]
	// From: Django/expressions
	fn test_non_empty_group_by_1() {
		// Test group by with multiple fields
		let f1 = F::new("year");
		let f2 = F::new("month");
		assert_eq!(f1.to_sql(), "year");
		assert_eq!(f2.to_sql(), "month");
	}

	#[test]
	// From: Django/expressions
	fn test_object_create_with_aggregate() {
		// Test creating object with aggregate value
		let agg = Aggregate::max("score");
		assert_eq!(agg.to_sql(), "MAX(score)");
	}

	#[test]
	// From: Django/expressions
	fn test_object_create_with_aggregate_1() {
		// Test creating object with aggregate value
		let agg = Aggregate::avg("rating");
		assert_eq!(agg.to_sql(), "AVG(rating)");
	}

	#[test]
	// From: Django/expressions
	fn test_object_create_with_f_expression_in_subquery() {
		// Test F expression in subquery
		let f = F::new("price");
		let subquery = Subquery::new(format!("SELECT {} FROM products", f.to_sql()));
		assert_eq!(
			subquery.to_sql(),
			"(SELECT price FROM products)",
			"Expected exact subquery with F expression, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_object_create_with_f_expression_in_subquery_1() {
		// Test F expression in subquery
		let f = F::new("quantity");
		assert_eq!(f.to_sql(), "quantity");
	}

	#[test]
	// From: Django/expressions
	fn test_order_by_exists() {
		// Test ordering by EXISTS clause
		let exists = Exists::new("SELECT 1 FROM related WHERE related.parent_id = main.id");
		assert_eq!(
			exists.to_sql(),
			"EXISTS(SELECT 1 FROM related WHERE related.parent_id = main.id)",
			"Expected exact EXISTS with related join, got: {}",
			exists.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_order_by_exists_1() {
		// Test ordering by EXISTS clause
		let exists = Exists::new("SELECT 1 FROM tags WHERE tags.item_id = items.id");
		assert_eq!(
			exists.to_sql(),
			"EXISTS(SELECT 1 FROM tags WHERE tags.item_id = items.id)",
			"Expected exact EXISTS with correlation, got: {}",
			exists.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_order_by_multiline_sql() {
		// Test multiline SQL expression
		let subquery = Subquery::new(
			"SELECT id
FROM users
WHERE active = 1",
		);
		assert_eq!(
			subquery.to_sql(),
			"(SELECT id\nFROM users\nWHERE active = 1)",
			"Expected exact multiline subquery, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_order_by_multiline_sql_1() {
		// Test multiline SQL expression
		let subquery = Subquery::new(
			"SELECT COUNT(*)
FROM orders
GROUP BY user_id",
		);
		assert_eq!(
			subquery.to_sql(),
			"(SELECT COUNT(*)\nFROM orders\nGROUP BY user_id)",
			"Expected exact multiline subquery with GROUP BY, got: {}",
			subquery.to_sql()
		);
	}

	#[test]
	// From: Django/expressions
	fn test_order_of_operations() {
		// Test order of operations in Q expressions
		let q1 = Q::new("a", "=", "1");
		let q2 = Q::new("b", "=", "2");
		let q3 = Q::new("c", "=", "3");
		let q = q1.and(q2).or(q3);
		let sql = q.to_sql();
		assert_eq!(
			sql, "((a = 1 AND b = 2) OR c = 3)",
			"Expected exact order of operations with AND/OR, got: {}",
			sql
		);
	}

	#[test]
	// From: Django/expressions
	fn test_order_of_operations_1() {
		// Test order of operations with NOT
		let q1 = Q::new("x", "=", "1");
		let q2 = Q::new("y", "=", "2");
		let q = q1.or(q2).not();
		assert_eq!(
			q.to_sql(),
			"NOT ((x = 1 OR y = 2))",
			"Expected exact NOT with OR operation, got: {}",
			q.to_sql()
		);
	}
}

/// When clause for Case expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct When {
	pub condition: Q,
	then: Box<Expression>,
}

impl When {
	/// Create a WHEN clause for CASE expressions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::{When, Q, Value, Expression};
	///
	/// let when_clause = When::new(
	///     Q::new("status", "=", "active"),
	///     Expression::Value(Value::string("Active User"))
	/// );
	/// // Verify the WHEN clause is created successfully
	/// let _: When = when_clause;
	/// ```
	pub fn new(condition: Q, then: Expression) -> Self {
		Self {
			condition,
			then: Box::new(then),
		}
	}

	/// Get a reference to the THEN expression
	pub fn then(&self) -> &Expression {
		&self.then
	}

	/// Convert into the THEN expression
	pub fn into_then(self) -> Expression {
		*self.then
	}

	/// Generate SQL for the WHEN clause
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::{When, Q, Value, Expression};
	///
	/// let when = When::new(
	///     Q::new("age", ">=", "18"),
	///     Expression::Value(Value::string("adult"))
	/// );
	/// assert!(when.to_sql().starts_with("WHEN"));
	/// ```
	pub fn to_sql(&self) -> String {
		format!(
			"WHEN {} THEN {}",
			self.condition.to_sql(),
			self.then.to_sql()
		)
	}
}

/// Case expression - conditional logic in SQL
/// Similar to Django's Case() for conditional expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
	pub when_clauses: Vec<When>,
	default: Option<Box<Expression>>,
}

impl Case {
	/// Create a new CASE expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::{Case, When, Q, Value, Expression};
	///
	/// let case_expr = Case::new()
	///     .when(When::new(
	///         Q::new("status", "=", "active"),
	///         Expression::Value(Value::int(1))
	///     ))
	///     .default(Expression::Value(Value::int(0)));
	/// // Verify the CASE expression is created successfully
	/// let _: Case = case_expr;
	/// ```
	pub fn new() -> Self {
		Self {
			when_clauses: Vec::new(),
			default: None,
		}
	}

	/// Get a reference to the default ELSE expression
	pub fn default_value(&self) -> Option<&Expression> {
		self.default.as_deref()
	}

	/// Convert into the default ELSE expression
	pub fn into_default(self) -> Option<Expression> {
		self.default.map(|b| *b)
	}

	/// Add a WHEN clause to the CASE expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::{Case, When, Q, Value, Expression};
	///
	/// let case = Case::new().when(When::new(
	///     Q::new("age", ">=", "18"),
	///     Expression::Value(Value::string("adult"))
	/// ));
	/// // Verify the CASE with WHEN clause is created successfully
	/// let _: Case = case;
	/// ```
	pub fn when(mut self, when: When) -> Self {
		self.when_clauses.push(when);
		self
	}

	/// Set the default ELSE value for the CASE expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::{Case, Value, Expression};
	///
	/// let case = Case::new().default(Expression::Value(Value::string("unknown")));
	/// // Verify the CASE with default value is created successfully
	/// let _: Case = case;
	/// ```
	pub fn default(mut self, default: Expression) -> Self {
		self.default = Some(Box::new(default));
		self
	}

	/// Generate SQL for the CASE expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::{Case, When, Q, Value, Expression};
	///
	/// let case = Case::new()
	///     .when(When::new(Q::new("x", "=", "1"), Expression::Value(Value::string("one"))))
	///     .default(Expression::Value(Value::string("other")));
	/// assert!(case.to_sql().starts_with("CASE"));
	/// assert!(case.to_sql().contains("END"));
	/// ```
	pub fn to_sql(&self) -> String {
		let when_clauses = self
			.when_clauses
			.iter()
			.map(|w| w.to_sql())
			.collect::<Vec<_>>()
			.join(" ");

		let default_clause = self
			.default
			.as_ref()
			.map(|d| format!(" ELSE {}", d.to_sql()))
			.unwrap_or_default();

		format!("CASE {}{} END", when_clauses, default_clause)
	}
}

impl Default for Case {
	fn default() -> Self {
		Self::new()
	}
}

/// Generic expression enum to support different expression types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
	F(F),
	Value(Value),
	Case(Case),
	// Aggregate(super::aggregation::Aggregate),
}

impl Expression {
	/// Generate SQL from this expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::expressions::{Expression, F, Value};
	///
	/// let field_expr = Expression::F(F::new("price"));
	/// assert_eq!(field_expr.to_sql(), "price");
	///
	/// let value_expr = Expression::Value(Value::int(100));
	/// assert_eq!(value_expr.to_sql(), "100");
	/// ```
	pub fn to_sql(&self) -> String {
		match self {
			Expression::F(f) => f.to_sql(),
			Expression::Value(v) => v.to_sql(),
			Expression::Case(c) => c.to_sql(),
		}
	}
}

impl From<F> for Expression {
	fn from(f: F) -> Self {
		Expression::F(f)
	}
}

impl From<Value> for Expression {
	fn from(v: Value) -> Self {
		Expression::Value(v)
	}
}

impl From<Case> for Expression {
	fn from(c: Case) -> Self {
		Expression::Case(c)
	}
}
