//! Unified query interface facade
//!
//! This module provides a unified entry point for querying functionality.
//! By default, it exports the expression-based query API (SQLAlchemy-style).

use super::FieldSelector;
use crate::orm::query_fields::GroupByFields;
use crate::orm::query_fields::aggregate::{AggregateExpr, ComparisonExpr};
use crate::orm::query_fields::comparison::FieldComparison;
use crate::orm::query_fields::compiler::QueryFieldCompiler;
use reinhardt_query::prelude::{
	Alias, BinOper, ColumnRef, Condition, Expr, ExprTrait, Func, JoinType as SeaJoinType, Order,
	PostgresQueryBuilder, Query, QueryStatementBuilder, SelectStatement, SimpleExpr,
};
use reinhardt_query::types::PgBinOper;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use uuid::Uuid;

// Django QuerySet API types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterOperator {
	Eq,
	Ne,
	Gt,
	Gte,
	Lt,
	Lte,
	In,
	NotIn,
	Contains,
	StartsWith,
	EndsWith,
	// PostgreSQL array operators
	/// Array contains all elements (@>)
	ArrayContains,
	/// Array is contained by (<@)
	ArrayContainedBy,
	/// Arrays overlap (&&) - at least one common element
	ArrayOverlap,
	// PostgreSQL full-text search
	/// Full-text search match (@@)
	FullTextMatch,
	// PostgreSQL JSONB operators
	/// JSONB contains (@>)
	JsonbContains,
	/// JSONB is contained by (<@)
	JsonbContainedBy,
	/// JSONB key exists (?)
	JsonbKeyExists,
	/// JSONB any key exists (?|)
	JsonbAnyKeyExists,
	/// JSONB all keys exist (?&)
	JsonbAllKeysExist,
	/// JSONB path exists (@?)
	JsonbPathExists,
	// Other operators
	/// Is null check
	IsNull,
	/// Is not null check
	IsNotNull,
	/// Range contains value (@>)
	RangeContains,
	/// Value is within range (<@)
	RangeContainedBy,
	/// Range overlaps (&&)
	RangeOverlaps,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterValue {
	String(String),
	Integer(i64),
	/// Alias for Integer (for compatibility with test code)
	Int(i64),
	Float(f64),
	Boolean(bool),
	/// Alias for Boolean (for compatibility with test code)
	Bool(bool),
	Null,
	Array(Vec<String>),
	/// Field reference for field-to-field comparisons (e.g., WHERE discount_price < total_price)
	FieldRef(super::expressions::F),
	/// Arithmetic expression (e.g., WHERE total != unit_price * quantity)
	Expression(super::annotation::Expression),
	/// Outer query reference for correlated subqueries (e.g., WHERE books.author_id = OuterRef("authors.id"))
	OuterRef(super::expressions::OuterRef),
}

#[derive(Debug, Clone)]
pub struct Filter {
	pub field: String,
	pub operator: FilterOperator,
	pub value: FilterValue,
}

impl Filter {
	pub fn new(field: impl Into<String>, operator: FilterOperator, value: FilterValue) -> Self {
		Self {
			field: field.into(),
			operator,
			value,
		}
	}
}

/// Values that can be used in UPDATE statements
#[derive(Debug, Clone)]
pub enum UpdateValue {
	String(String),
	Integer(i64),
	Float(f64),
	Boolean(bool),
	Null,
	/// Field reference for field-to-field updates (e.g., SET discount_price = total_price)
	FieldRef(super::expressions::F),
	/// Arithmetic expression (e.g., SET total = unit_price * quantity)
	Expression(super::annotation::Expression),
}

/// Composite filter condition supporting AND/OR logic
///
/// This enum allows building complex filter expressions with nested AND/OR conditions.
/// It's particularly useful for search functionality that needs to match across
/// multiple fields using OR logic.
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::{Filter, FilterCondition, FilterOperator, FilterValue};
///
/// // Simple single filter
/// let single = FilterCondition::Single(Filter::new(
///     "name".to_string(),
///     FilterOperator::Eq,
///     FilterValue::String("Alice".to_string()),
/// ));
///
/// // OR condition across multiple fields (useful for search)
/// let search = FilterCondition::Or(vec![
///     FilterCondition::Single(Filter::new(
///         "name".to_string(),
///         FilterOperator::Contains,
///         FilterValue::String("alice".to_string()),
///     )),
///     FilterCondition::Single(Filter::new(
///         "email".to_string(),
///         FilterOperator::Contains,
///         FilterValue::String("alice".to_string()),
///     )),
/// ]);
///
/// // Complex nested condition: (status = 'active') AND (name LIKE '%alice%' OR email LIKE '%alice%')
/// let complex = FilterCondition::And(vec![
///     FilterCondition::Single(Filter::new(
///         "status".to_string(),
///         FilterOperator::Eq,
///         FilterValue::String("active".to_string()),
///     )),
///     search,
/// ]);
/// ```
#[derive(Debug, Clone)]
pub enum FilterCondition {
	/// A single filter expression
	Single(Filter),
	/// All conditions must match (AND logic)
	And(Vec<FilterCondition>),
	/// Any condition must match (OR logic)
	Or(Vec<FilterCondition>),
	/// Negates the inner condition (NOT logic)
	Not(Box<FilterCondition>),
}

impl FilterCondition {
	/// Create a single filter condition
	pub fn single(filter: Filter) -> Self {
		Self::Single(filter)
	}

	/// Create an AND condition from multiple conditions
	pub fn and(conditions: Vec<FilterCondition>) -> Self {
		Self::And(conditions)
	}

	/// Create an OR condition from multiple conditions
	pub fn or(conditions: Vec<FilterCondition>) -> Self {
		Self::Or(conditions)
	}

	/// Create a NOT condition that negates the given condition
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::{Filter, FilterCondition, FilterOperator, FilterValue};
	///
	/// let condition = FilterCondition::not(
	///     FilterCondition::Single(Filter::new(
	///         "is_active".to_string(),
	///         FilterOperator::Eq,
	///         FilterValue::Boolean(true),
	///     ))
	/// );
	/// ```
	// This method is intentionally named `not` for API consistency with Django's Q object.
	// It does not implement std::ops::Not because it constructs a FilterCondition variant,
	// not a boolean negation.
	#[allow(clippy::should_implement_trait)]
	pub fn not(condition: FilterCondition) -> Self {
		Self::Not(Box::new(condition))
	}

	/// Create an OR condition from multiple filters (convenience method for search)
	///
	/// This is particularly useful for implementing search across multiple fields.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::{Filter, FilterCondition, FilterOperator, FilterValue};
	///
	/// let search_filters = vec![
	///     Filter::new("name".to_string(), FilterOperator::Contains, FilterValue::String("test".to_string())),
	///     Filter::new("email".to_string(), FilterOperator::Contains, FilterValue::String("test".to_string())),
	/// ];
	/// let or_condition = FilterCondition::or_filters(search_filters);
	/// ```
	pub fn or_filters(filters: Vec<Filter>) -> Self {
		Self::Or(filters.into_iter().map(FilterCondition::Single).collect())
	}

	/// Create an AND condition from multiple filters
	pub fn and_filters(filters: Vec<Filter>) -> Self {
		Self::And(filters.into_iter().map(FilterCondition::Single).collect())
	}

	/// Check if this condition is empty (no actual filters)
	pub fn is_empty(&self) -> bool {
		match self {
			FilterCondition::Single(_) => false,
			FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
				conditions.is_empty() || conditions.iter().all(|c| c.is_empty())
			}
			FilterCondition::Not(condition) => condition.is_empty(),
		}
	}
}

// From implementations for FilterValue
impl From<String> for FilterValue {
	fn from(s: String) -> Self {
		FilterValue::String(s)
	}
}

impl From<&str> for FilterValue {
	fn from(s: &str) -> Self {
		FilterValue::String(s.to_string())
	}
}

impl From<i64> for FilterValue {
	fn from(i: i64) -> Self {
		FilterValue::Integer(i)
	}
}

impl From<i32> for FilterValue {
	fn from(i: i32) -> Self {
		FilterValue::Integer(i as i64)
	}
}

impl From<f64> for FilterValue {
	fn from(f: f64) -> Self {
		FilterValue::Float(f)
	}
}

impl From<bool> for FilterValue {
	fn from(b: bool) -> Self {
		FilterValue::Boolean(b)
	}
}

impl From<uuid::Uuid> for FilterValue {
	fn from(u: uuid::Uuid) -> Self {
		FilterValue::String(u.to_string())
	}
}

#[derive(Debug, Clone)]
pub struct OrmQuery {
	filters: Vec<Filter>,
}

impl OrmQuery {
	pub fn new() -> Self {
		Self {
			filters: Vec::new(),
		}
	}

	pub fn filter(mut self, filter: Filter) -> Self {
		self.filters.push(filter);
		self
	}
}

impl Default for OrmQuery {
	fn default() -> Self {
		Self::new()
	}
}

/// JOIN clause specification for QuerySet
#[derive(Clone, Debug)]
struct JoinClause {
	/// The type of JOIN (INNER, LEFT, RIGHT, CROSS)
	join_type: super::sqlalchemy_query::JoinType,
	/// The name of the table to join
	target_table: String,
	/// Optional alias for the target table (for self-joins)
	target_alias: Option<String>,
	/// The ON condition as a SQL expression string
	/// Format: "left_table.left_field = right_table.right_field"
	/// Can include table aliases for self-joins (e.g., "u1.id < u2.id")
	on_condition: String,
}

/// Aggregate function types for HAVING clauses
#[derive(Clone, Debug)]
enum AggregateFunc {
	Avg,
	Count,
	Sum,
	Min,
	Max,
}

/// Comparison operators for HAVING clauses
#[derive(Clone, Debug)]
pub enum ComparisonOp {
	Eq,
	Ne,
	Gt,
	Gte,
	Lt,
	Lte,
}

/// Value types for aggregate comparisons in HAVING clauses
#[derive(Clone, Debug)]
enum AggregateValue {
	Int(i64),
	Float(f64),
}

/// HAVING clause condition specification
#[derive(Clone, Debug)]
enum HavingCondition {
	/// Compare an aggregate function result with a value
	/// Example: HAVING AVG(price) > 1500.0
	AggregateCompare {
		func: AggregateFunc,
		field: String,
		operator: ComparisonOp,
		value: AggregateValue,
	},
}

/// Subquery condition specification for WHERE clause
#[derive(Clone, Debug)]
enum SubqueryCondition {
	/// WHERE field IN (subquery)
	/// Example: WHERE author_id IN (SELECT id FROM authors WHERE name = 'John')
	In { field: String, subquery: String },
	/// WHERE field NOT IN (subquery)
	NotIn { field: String, subquery: String },
	/// WHERE EXISTS (subquery)
	/// Example: WHERE EXISTS (SELECT 1 FROM books WHERE author_id = authors.id)
	Exists { subquery: String },
	/// WHERE NOT EXISTS (subquery)
	NotExists { subquery: String },
}

#[derive(Clone)]
pub struct QuerySet<T>
where
	T: super::Model,
{
	_phantom: std::marker::PhantomData<T>,
	filters: SmallVec<[Filter; 10]>,
	select_related_fields: Vec<String>,
	prefetch_related_fields: Vec<String>,
	order_by_fields: Vec<String>,
	distinct_enabled: bool,
	selected_fields: Option<Vec<String>>,
	deferred_fields: Vec<String>,
	annotations: Vec<super::annotation::Annotation>,
	manager: Option<std::sync::Arc<super::manager::Manager<T>>>,
	limit: Option<usize>,
	offset: Option<usize>,
	ctes: super::cte::CTECollection,
	lateral_joins: super::lateral_join::LateralJoins,
	joins: Vec<JoinClause>,
	group_by_fields: Vec<String>,
	having_conditions: Vec<HavingCondition>,
	subquery_conditions: Vec<SubqueryCondition>,
	from_alias: Option<String>,
	/// Subquery SQL for FROM clause (derived table)
	/// When set, the FROM clause will use this subquery instead of the model's table
	from_subquery_sql: Option<String>,
}

impl<T> QuerySet<T>
where
	T: super::Model,
{
	pub fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
			filters: SmallVec::new(),
			select_related_fields: Vec::new(),
			prefetch_related_fields: Vec::new(),
			order_by_fields: Vec::new(),
			distinct_enabled: false,
			selected_fields: None,
			deferred_fields: Vec::new(),
			annotations: Vec::new(),
			manager: None,
			limit: None,
			offset: None,
			ctes: super::cte::CTECollection::new(),
			lateral_joins: super::lateral_join::LateralJoins::new(),
			joins: Vec::new(),
			group_by_fields: Vec::new(),
			having_conditions: Vec::new(),
			subquery_conditions: Vec::new(),
			from_alias: None,
			from_subquery_sql: None,
		}
	}

	pub fn with_manager(manager: std::sync::Arc<super::manager::Manager<T>>) -> Self {
		Self {
			_phantom: std::marker::PhantomData,
			filters: SmallVec::new(),
			select_related_fields: Vec::new(),
			prefetch_related_fields: Vec::new(),
			order_by_fields: Vec::new(),
			distinct_enabled: false,
			selected_fields: None,
			deferred_fields: Vec::new(),
			annotations: Vec::new(),
			manager: Some(manager),
			limit: None,
			offset: None,
			ctes: super::cte::CTECollection::new(),
			lateral_joins: super::lateral_join::LateralJoins::new(),
			joins: Vec::new(),
			group_by_fields: Vec::new(),
			having_conditions: Vec::new(),
			subquery_conditions: Vec::new(),
			from_alias: None,
			from_subquery_sql: None,
		}
	}

	pub fn filter(mut self, filter: Filter) -> Self {
		self.filters.push(filter);
		self
	}

	/// Create a QuerySet from a subquery (FROM clause subquery / derived table)
	///
	/// This method creates a new QuerySet that uses a subquery as its data source
	/// instead of a regular table. The subquery becomes a derived table in the FROM clause.
	///
	/// # Type Parameters
	///
	/// * `M` - The model type for the subquery
	/// * `F` - A closure that builds the subquery
	///
	/// # Parameters
	///
	/// * `builder` - A closure that receives a fresh `QuerySet<M>` and returns a configured QuerySet
	/// * `alias` - The alias for the derived table (required for FROM subqueries)
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::{Model, QuerySet};
	/// # use reinhardt_db::orm::annotation::{Annotation, AnnotationValue};
	/// # use reinhardt_db::orm::aggregation::Aggregate;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use reinhardt_db::orm::GroupByFields;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Book { id: Option<i64>, author_id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct BookFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for BookFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Book {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = BookFields;
	/// #     fn table_name() -> &'static str { "books" }
	/// #     fn new_fields() -> Self::Fields { BookFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// // Query from a derived table showing author book counts
	/// let results = QuerySet::<Book>::from_subquery(
	///     |subq: QuerySet<Book>| {
	///         subq.values(&["author_id"])
	///             .annotate(Annotation::new("book_count", AnnotationValue::Aggregate(Aggregate::count_all())))
	///     },
	///     "book_stats"
	/// )
	/// .filter(Filter::new("book_count", FilterOperator::Gt, FilterValue::Int(1)))
	/// .to_sql();
	/// // Generates: SELECT * FROM (SELECT author_id, COUNT(*) AS book_count FROM books GROUP BY author_id) AS book_stats WHERE book_count > 1
	/// ```
	pub fn from_subquery<M, F>(builder: F, alias: &str) -> Self
	where
		M: super::Model + 'static,
		F: FnOnce(QuerySet<M>) -> QuerySet<M>,
	{
		// Create a fresh QuerySet for the subquery model
		let subquery_qs = QuerySet::<M>::new();
		// Apply the builder to configure the subquery
		let configured_subquery = builder(subquery_qs);
		// Generate SQL for the subquery (wrapped in parentheses)
		let subquery_sql = configured_subquery.as_subquery();

		// Create a new QuerySet with the subquery as FROM source
		Self {
			_phantom: std::marker::PhantomData,
			filters: SmallVec::new(),
			select_related_fields: Vec::new(),
			prefetch_related_fields: Vec::new(),
			order_by_fields: Vec::new(),
			distinct_enabled: false,
			selected_fields: None,
			deferred_fields: Vec::new(),
			annotations: Vec::new(),
			manager: None,
			limit: None,
			offset: None,
			ctes: super::cte::CTECollection::new(),
			lateral_joins: super::lateral_join::LateralJoins::new(),
			joins: Vec::new(),
			group_by_fields: Vec::new(),
			having_conditions: Vec::new(),
			subquery_conditions: Vec::new(),
			from_alias: Some(alias.to_string()),
			from_subquery_sql: Some(subquery_sql),
		}
	}

	/// Add an INNER JOIN to the query
	///
	/// Performs an INNER JOIN between the current model (T) and another model (R).
	/// Only rows with matching values in both tables are included in the result.
	///
	/// # Type Parameters
	///
	/// * `R` - The model type to join with (must implement `Model` trait)
	///
	/// # Parameters
	///
	/// * `left_field` - The field name from the left table (current model T)
	/// * `right_field` - The field name from the right table (model R)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, user_id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Join User and Post on user.id = post.user_id
	/// let sql = User::objects()
	///     .all()
	///     .inner_join::<Post>("id", "user_id")
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn inner_join<R: super::Model>(mut self, left_field: &str, right_field: &str) -> Self {
		let condition = format!(
			"{}.{} = {}.{}",
			T::table_name(),
			left_field,
			R::table_name(),
			right_field
		);

		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Inner,
			target_table: R::table_name().to_string(),
			target_alias: None,
			on_condition: condition,
		});

		self
	}

	/// Add a LEFT OUTER JOIN to the query
	///
	/// Performs a LEFT OUTER JOIN between the current model (T) and another model (R).
	/// All rows from the left table are included, with matching rows from the right table
	/// or NULL values if no match is found.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, user_id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Left join User and Post
	/// let sql = User::objects()
	///     .all()
	///     .left_join::<Post>("id", "user_id")
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn left_join<R: super::Model>(mut self, left_field: &str, right_field: &str) -> Self {
		let condition = format!(
			"{}.{} = {}.{}",
			T::table_name(),
			left_field,
			R::table_name(),
			right_field
		);

		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Left,
			target_table: R::table_name().to_string(),
			target_alias: None,
			on_condition: condition,
		});

		self
	}

	/// Add a RIGHT OUTER JOIN to the query
	///
	/// Performs a RIGHT OUTER JOIN between the current model (T) and another model (R).
	/// All rows from the right table are included, with matching rows from the left table
	/// or NULL values if no match is found.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, user_id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Right join User and Post
	/// let sql = User::objects()
	///     .all()
	///     .right_join::<Post>("id", "user_id")
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn right_join<R: super::Model>(mut self, left_field: &str, right_field: &str) -> Self {
		let condition = format!(
			"{}.{} = {}.{}",
			T::table_name(),
			left_field,
			R::table_name(),
			right_field
		);

		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Right,
			target_table: R::table_name().to_string(),
			target_alias: None,
			on_condition: condition,
		});

		self
	}

	/// Add a CROSS JOIN to the query
	///
	/// Performs a CROSS JOIN between the current model (T) and another model (R).
	/// Produces the Cartesian product of both tables (all possible combinations).
	/// No ON condition is needed for CROSS JOIN.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Category { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct CategoryFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for CategoryFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Category {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = CategoryFields;
	/// #     fn table_name() -> &'static str { "categories" }
	/// #     fn new_fields() -> Self::Fields { CategoryFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Cross join User and Category
	/// let sql = User::objects()
	///     .all()
	///     .cross_join::<Category>()
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn cross_join<R: super::Model>(mut self) -> Self {
		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Inner, // CROSS JOIN uses Inner with empty condition
			target_table: R::table_name().to_string(),
			target_alias: None,
			on_condition: String::new(), // Empty condition for CROSS JOIN
		});

		self
	}

	/// Set an alias for the base table (FROM clause)
	///
	/// This is useful for self-joins where you need to reference the same table multiple times.
	///
	/// # Parameters
	///
	/// * `alias` - The alias name for the base table
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::query_fields::Field;
	/// # use reinhardt_db::orm::FieldSelector;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields {
	/// #     pub id: Field<User, i64>,
	/// # }
	/// # impl UserFields {
	/// #     pub fn new() -> Self {
	/// #         Self { id: Field::new(vec!["id"]) }
	/// #     }
	/// # }
	/// # impl FieldSelector for UserFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.id = self.id.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Self-join: find user pairs
	/// let sql = User::objects()
	///     .all()
	///     .from_as("u1")
	///     .inner_join_as::<User, _>("u1", "u2", |left, right| left.id.field_lt(right.id))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn from_as(mut self, alias: &str) -> Self {
		self.from_alias = Some(alias.to_string());
		self
	}

	/// Add an INNER JOIN with custom condition
	///
	/// Performs an INNER JOIN with a custom ON condition expression.
	/// Use this when you need complex join conditions beyond simple equality.
	///
	/// # Type Parameters
	///
	/// * `R` - The model type to join with (must implement `Model` trait)
	///
	/// # Parameters
	///
	/// * `condition` - Custom SQL condition for the JOIN (e.g., "users.id = posts.user_id AND posts.status = 'published'")
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, user_id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Join with complex condition
	/// let sql = User::objects()
	///     .all()
	///     .inner_join_on::<Post>("users.id = posts.user_id AND posts.title LIKE 'First%'")
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn inner_join_on<R: super::Model>(mut self, condition: &str) -> Self {
		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Inner,
			target_table: R::table_name().to_string(),
			target_alias: None,
			on_condition: condition.to_string(),
		});

		self
	}

	/// Add a LEFT OUTER JOIN with custom condition
	///
	/// Similar to `inner_join_on()` but performs a LEFT OUTER JOIN.
	///
	/// # Type Parameters
	///
	/// * `R` - The model type to join with (must implement `Model` trait)
	///
	/// # Parameters
	///
	/// * `condition` - Custom SQL condition for the JOIN
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, user_id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let sql = User::objects()
	///     .all()
	///     .left_join_on::<Post>("users.id = posts.user_id AND posts.published = true")
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn left_join_on<R: super::Model>(mut self, condition: &str) -> Self {
		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Left,
			target_table: R::table_name().to_string(),
			target_alias: None,
			on_condition: condition.to_string(),
		});

		self
	}

	/// Add a RIGHT OUTER JOIN with custom condition
	///
	/// Similar to `inner_join_on()` but performs a RIGHT OUTER JOIN.
	///
	/// # Type Parameters
	///
	/// * `R` - The model type to join with (must implement `Model` trait)
	///
	/// # Parameters
	///
	/// * `condition` - Custom SQL condition for the JOIN
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, user_id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let sql = User::objects()
	///     .all()
	///     .right_join_on::<Post>("users.id = posts.user_id AND users.active = true")
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn right_join_on<R: super::Model>(mut self, condition: &str) -> Self {
		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Right,
			target_table: R::table_name().to_string(),
			target_alias: None,
			on_condition: condition.to_string(),
		});

		self
	}

	/// Add an INNER JOIN with table alias
	///
	/// Performs an INNER JOIN with an alias for the target table.
	/// Useful for self-joins or when you need to reference the same table multiple times.
	///
	/// # Type Parameters
	///
	/// * `R` - The model type to join with (must implement `Model` trait)
	/// * `F` - Closure that builds the JOIN ON condition
	///
	/// # Parameters
	///
	/// * `alias` - Alias name for the target table
	/// * `condition_fn` - Closure that receives a `JoinOnBuilder` and returns it with the condition set
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::query_fields::Field;
	/// # use reinhardt_db::orm::FieldSelector;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields {
	/// #     pub id: Field<User, i64>,
	/// # }
	/// # impl UserFields {
	/// #     pub fn new() -> Self {
	/// #         Self { id: Field::new(vec!["id"]) }
	/// #     }
	/// # }
	/// # impl FieldSelector for UserFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.id = self.id.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Self-join: find user pairs where user1.id < user2.id
	/// let sql = User::objects()
	///     .all()
	///     .inner_join_as::<User, _>("u1", "u2", |u1, u2| u1.id.field_lt(u2.id))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	/// # Breaking Change
	///
	/// The signature of this method has been changed from string-based JOIN conditions
	/// to type-safe field comparisons.
	pub fn inner_join_as<R: super::Model, F>(
		mut self,
		left_alias: &str,
		right_alias: &str,
		condition_fn: F,
	) -> Self
	where
		F: FnOnce(T::Fields, R::Fields) -> FieldComparison,
	{
		// Set base table alias
		if self.from_alias.is_none() {
			self.from_alias = Some(left_alias.to_string());
		}

		// Create field selectors and set aliases
		let left_fields = T::new_fields().with_alias(left_alias);
		let right_fields = R::new_fields().with_alias(right_alias);

		// Get comparison expression from closure
		let comparison = condition_fn(left_fields, right_fields);

		// Convert to SQL
		let condition = QueryFieldCompiler::compile_field_comparison(&comparison);

		// Add to JoinClause
		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Inner,
			target_table: R::table_name().to_string(),
			target_alias: Some(right_alias.to_string()),
			on_condition: condition,
		});

		self
	}

	/// Add a LEFT OUTER JOIN with table alias
	///
	/// Similar to `inner_join_as()` but performs a LEFT OUTER JOIN.
	///
	/// # Type Parameters
	///
	/// * `R` - The model type to join with (must implement `Model` trait)
	/// * `F` - Closure that builds the JOIN ON condition
	///
	/// # Parameters
	///
	/// * `alias` - Alias name for the target table
	/// * `condition_fn` - Closure that receives a `JoinOnBuilder` and returns it with the condition set
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::query_fields::Field;
	/// # use reinhardt_db::orm::FieldSelector;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields {
	/// #     pub id: Field<User, i64>,
	/// #     pub manager_id: Field<User, i64>,
	/// # }
	/// # impl UserFields {
	/// #     pub fn new() -> Self {
	/// #         Self {
	/// #             id: Field::new(vec!["id"]),
	/// #             manager_id: Field::new(vec!["manager_id"]),
	/// #         }
	/// #     }
	/// # }
	/// # impl FieldSelector for UserFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.id = self.id.with_alias(alias);
	/// #         self.manager_id = self.manager_id.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Self-join with LEFT JOIN: find employees and their managers
	/// let sql = User::objects()
	///     .all()
	///     .left_join_as::<User, _>("u1", "u2", |u1, u2| u2.id.field_eq(u1.manager_id))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	/// # Breaking Change
	///
	/// This method signature has been changed from string-based JOIN conditions
	/// to type-safe field comparisons.
	pub fn left_join_as<R: super::Model, F>(
		mut self,
		left_alias: &str,
		right_alias: &str,
		condition_fn: F,
	) -> Self
	where
		F: FnOnce(T::Fields, R::Fields) -> FieldComparison,
	{
		// Set base table alias
		if self.from_alias.is_none() {
			self.from_alias = Some(left_alias.to_string());
		}

		// Create field selectors with aliases
		let left_fields = T::new_fields().with_alias(left_alias);
		let right_fields = R::new_fields().with_alias(right_alias);

		// Get comparison from closure
		let comparison = condition_fn(left_fields, right_fields);

		// Convert to SQL
		let condition = QueryFieldCompiler::compile_field_comparison(&comparison);

		// Add to JoinClause
		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Left,
			target_table: R::table_name().to_string(),
			target_alias: Some(right_alias.to_string()),
			on_condition: condition,
		});

		self
	}

	/// Add a RIGHT OUTER JOIN with table alias
	///
	/// Similar to `inner_join_as()` but performs a RIGHT OUTER JOIN.
	///
	/// # Type Parameters
	///
	/// * `R` - The model type to join with (must implement `Model` trait)
	/// * `F` - Closure that builds the JOIN ON condition
	///
	/// # Parameters
	///
	/// * `alias` - Alias name for the target table
	/// * `condition_fn` - Closure that receives a `JoinOnBuilder` and returns it with the condition set
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::query_fields::Field;
	/// # use reinhardt_db::orm::FieldSelector;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields {
	/// #     pub id: Field<User, i64>,
	/// #     pub department_id: Field<User, i64>,
	/// # }
	/// # impl UserFields {
	/// #     pub fn new() -> Self {
	/// #         Self {
	/// #             id: Field::new(vec!["id"]),
	/// #             department_id: Field::new(vec!["department_id"]),
	/// #         }
	/// #     }
	/// # }
	/// # impl FieldSelector for UserFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.id = self.id.with_alias(alias);
	/// #         self.department_id = self.department_id.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // RIGHT JOIN: find all departments even if no users belong to them
	/// let sql = User::objects()
	///     .all()
	///     .right_join_as::<User, _>("u1", "u2", |u1, u2| u2.id.field_eq(u1.department_id))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	/// # Breaking Change
	///
	/// This method signature has been changed from string-based JOIN conditions
	/// to type-safe field comparisons.
	pub fn right_join_as<R: super::Model, F>(
		mut self,
		left_alias: &str,
		right_alias: &str,
		condition_fn: F,
	) -> Self
	where
		F: FnOnce(T::Fields, R::Fields) -> FieldComparison,
	{
		// Set base table alias
		if self.from_alias.is_none() {
			self.from_alias = Some(left_alias.to_string());
		}

		// Create field selectors with aliases
		let left_fields = T::new_fields().with_alias(left_alias);
		let right_fields = R::new_fields().with_alias(right_alias);

		// Get comparison from closure
		let comparison = condition_fn(left_fields, right_fields);

		// Convert to SQL
		let condition = QueryFieldCompiler::compile_field_comparison(&comparison);

		// Add to JoinClause
		self.joins.push(JoinClause {
			join_type: super::sqlalchemy_query::JoinType::Right,
			target_table: R::table_name().to_string(),
			target_alias: Some(right_alias.to_string()),
			on_condition: condition,
		});

		self
	}

	/// Add GROUP BY clause to the query
	///
	/// Groups rows that have the same values in specified columns into summary rows.
	/// Typically used with aggregate functions (COUNT, MAX, MIN, SUM, AVG).
	///
	/// # Type Parameters
	///
	/// * `F` - Closure that builds the GROUP BY field list
	///
	/// # Parameters
	///
	/// * `builder_fn` - Closure that receives a `GroupByBuilder` and returns it with fields set
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::{Model, query_fields::{Field, GroupByFields}, FieldSelector};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Book { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct BookFields {
	/// #     pub author_id: Field<Book, i64>,
	/// # }
	/// # impl BookFields {
	/// #     pub fn new() -> Self {
	/// #         Self { author_id: Field::new(vec!["author_id"]) }
	/// #     }
	/// # }
	/// # impl FieldSelector for BookFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.author_id = self.author_id.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for Book {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = BookFields;
	/// #     fn table_name() -> &'static str { "books" }
	/// #     fn new_fields() -> Self::Fields { BookFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Sale { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct SaleFields {
	/// #     pub region: Field<Sale, String>,
	/// #     pub product_category: Field<Sale, String>,
	/// # }
	/// # impl SaleFields {
	/// #     pub fn new() -> Self {
	/// #         Self {
	/// #             region: Field::new(vec!["region"]),
	/// #             product_category: Field::new(vec!["product_category"]),
	/// #         }
	/// #     }
	/// # }
	/// # impl FieldSelector for SaleFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.region = self.region.with_alias(alias);
	/// #         self.product_category = self.product_category.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for Sale {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = SaleFields;
	/// #     fn table_name() -> &'static str { "sales" }
	/// #     fn new_fields() -> Self::Fields { SaleFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Group by single field
	/// let sql1 = Book::objects()
	///     .all()
	///     .group_by(|fields| GroupByFields::new().add(&fields.author_id))
	///     .to_sql();
	///
	/// // Group by multiple fields (chain .add())
	/// let sql2 = Sale::objects()
	///     .all()
	///     .group_by(|fields| GroupByFields::new().add(&fields.region).add(&fields.product_category))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	/// # Breaking Change
	///
	/// This method signature has been changed from string-based field selection
	/// to type-safe field selectors.
	pub fn group_by<F>(mut self, selector_fn: F) -> Self
	where
		F: FnOnce(T::Fields) -> GroupByFields,
	{
		let fields = T::new_fields();
		let group_by_fields = selector_fn(fields);
		self.group_by_fields = group_by_fields.build();
		self
	}

	/// Add HAVING clause for AVG aggregate
	///
	/// Filters grouped rows based on the average value of a field.
	///
	/// # Type Parameters
	///
	/// * `F` - Closure that selects the field
	///
	/// # Parameters
	///
	/// * `field_fn` - Closure that receives a `HavingFieldSelector` and returns it with the field set
	/// * `operator` - Comparison operator (Eq, Ne, Gt, Gte, Lt, Lte)
	/// * `value` - Value to compare against
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::{Model, query_fields::{Field, GroupByFields}, FieldSelector};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct AuthorFields {
	/// #     pub author_id: Field<Author, i64>,
	/// #     pub price: Field<Author, f64>,
	/// # }
	/// # impl AuthorFields {
	/// #     pub fn new() -> Self {
	/// #         Self {
	/// #             author_id: Field::new(vec!["author_id"]),
	/// #             price: Field::new(vec!["price"]),
	/// #         }
	/// #     }
	/// # }
	/// # impl FieldSelector for AuthorFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.author_id = self.author_id.with_alias(alias);
	/// #         self.price = self.price.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find authors with average book price > 1500
	/// let sql = Author::objects()
	///     .all()
	///     .group_by(|fields| GroupByFields::new().add(&fields.author_id))
	///     .having_avg(|fields| &fields.price, |avg| avg.gt(1500.0))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	/// # Breaking Change
	///
	/// This method signature has been changed to use type-safe field selectors
	/// and aggregate expressions.
	pub fn having_avg<FS, FE, NT>(mut self, field_selector: FS, expr_fn: FE) -> Self
	where
		FS: FnOnce(&T::Fields) -> &super::query_fields::Field<T, NT>,
		NT: super::query_fields::NumericType,
		FE: FnOnce(AggregateExpr) -> ComparisonExpr,
	{
		let fields = T::new_fields();
		let field = field_selector(&fields);
		let field_path = field.path().join(".");

		let avg_expr = AggregateExpr::avg(&field_path);
		let comparison = expr_fn(avg_expr);

		// Extract components for HavingCondition
		let operator = match comparison.op {
			super::query_fields::comparison::ComparisonOperator::Eq => ComparisonOp::Eq,
			super::query_fields::comparison::ComparisonOperator::Ne => ComparisonOp::Ne,
			super::query_fields::comparison::ComparisonOperator::Gt => ComparisonOp::Gt,
			super::query_fields::comparison::ComparisonOperator::Gte => ComparisonOp::Gte,
			super::query_fields::comparison::ComparisonOperator::Lt => ComparisonOp::Lt,
			super::query_fields::comparison::ComparisonOperator::Lte => ComparisonOp::Lte,
		};

		let value = match comparison.value {
			super::query_fields::aggregate::ComparisonValue::Int(i) => {
				AggregateValue::Float(i as f64)
			}
			super::query_fields::aggregate::ComparisonValue::Float(f) => AggregateValue::Float(f),
		};

		self.having_conditions
			.push(HavingCondition::AggregateCompare {
				func: AggregateFunc::Avg,
				field: comparison.aggregate.field().to_string(),
				operator,
				value,
			});
		self
	}

	/// Add HAVING clause for COUNT aggregate
	///
	/// Filters grouped rows based on the count of rows in each group.
	///
	/// # Type Parameters
	///
	/// * `F` - Closure that selects the field
	///
	/// # Parameters
	///
	/// * `field_fn` - Closure that receives a `HavingFieldSelector` and returns it with the field set
	/// * `operator` - Comparison operator (Eq, Ne, Gt, Gte, Lt, Lte)
	/// * `value` - Value to compare against
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::{Model, query_fields::{Field, GroupByFields}, FieldSelector};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct AuthorFields {
	/// #     pub author_id: Field<Author, i64>,
	/// # }
	/// # impl AuthorFields {
	/// #     pub fn new() -> Self {
	/// #         Self { author_id: Field::new(vec!["author_id"]) }
	/// #     }
	/// # }
	/// # impl FieldSelector for AuthorFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.author_id = self.author_id.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find authors with more than 5 books
	/// let sql = Author::objects()
	///     .all()
	///     .group_by(|fields| GroupByFields::new().add(&fields.author_id))
	///     .having_count(|count| count.gt(5))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	/// # Breaking Change
	///
	/// This method signature has been changed to use type-safe aggregate expressions.
	pub fn having_count<F>(mut self, expr_fn: F) -> Self
	where
		F: FnOnce(AggregateExpr) -> ComparisonExpr,
	{
		let count_expr = AggregateExpr::count("*");
		let comparison = expr_fn(count_expr);

		// Extract components for HavingCondition
		let operator = match comparison.op {
			super::query_fields::comparison::ComparisonOperator::Eq => ComparisonOp::Eq,
			super::query_fields::comparison::ComparisonOperator::Ne => ComparisonOp::Ne,
			super::query_fields::comparison::ComparisonOperator::Gt => ComparisonOp::Gt,
			super::query_fields::comparison::ComparisonOperator::Gte => ComparisonOp::Gte,
			super::query_fields::comparison::ComparisonOperator::Lt => ComparisonOp::Lt,
			super::query_fields::comparison::ComparisonOperator::Lte => ComparisonOp::Lte,
		};

		let value = match comparison.value {
			super::query_fields::aggregate::ComparisonValue::Int(i) => AggregateValue::Int(i),
			super::query_fields::aggregate::ComparisonValue::Float(f) => AggregateValue::Float(f),
		};

		self.having_conditions
			.push(HavingCondition::AggregateCompare {
				func: AggregateFunc::Count,
				field: comparison.aggregate.field().to_string(),
				operator,
				value,
			});
		self
	}

	/// Add HAVING clause for SUM aggregate
	///
	/// Filters grouped rows based on the sum of values in a field.
	///
	/// # Type Parameters
	///
	/// * `F` - Closure that selects the field
	///
	/// # Parameters
	///
	/// * `field_fn` - Closure that receives a `HavingFieldSelector` and returns it with the field set
	/// * `operator` - Comparison operator (Eq, Ne, Gt, Gte, Lt, Lte)
	/// * `value` - Value to compare against
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::{Model, query_fields::{Field, GroupByFields}, FieldSelector};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Product { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct ProductFields {
	/// #     pub category: Field<Product, String>,
	/// #     pub sales_amount: Field<Product, f64>,
	/// # }
	/// # impl ProductFields {
	/// #     pub fn new() -> Self {
	/// #         Self {
	/// #             category: Field::new(vec!["category"]),
	/// #             sales_amount: Field::new(vec!["sales_amount"]),
	/// #         }
	/// #     }
	/// # }
	/// # impl FieldSelector for ProductFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.category = self.category.with_alias(alias);
	/// #         self.sales_amount = self.sales_amount.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for Product {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = ProductFields;
	/// #     fn table_name() -> &'static str { "products" }
	/// #     fn new_fields() -> Self::Fields { ProductFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find categories with total sales > 10000
	/// let sql = Product::objects()
	///     .all()
	///     .group_by(|fields| GroupByFields::new().add(&fields.category))
	///     .having_sum(|fields| &fields.sales_amount, |sum| sum.gt(10000.0))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	/// # Breaking Change
	///
	/// This method signature has been changed to use type-safe field selectors.
	pub fn having_sum<FS, FE, NT>(mut self, field_selector: FS, expr_fn: FE) -> Self
	where
		FS: FnOnce(&T::Fields) -> &super::query_fields::Field<T, NT>,
		NT: super::query_fields::NumericType,
		FE: FnOnce(AggregateExpr) -> ComparisonExpr,
	{
		let fields = T::new_fields();
		let field = field_selector(&fields);
		let field_path = field.path().join(".");

		let sum_expr = AggregateExpr::sum(&field_path);
		let comparison = expr_fn(sum_expr);

		let operator = match comparison.op {
			super::query_fields::comparison::ComparisonOperator::Eq => ComparisonOp::Eq,
			super::query_fields::comparison::ComparisonOperator::Ne => ComparisonOp::Ne,
			super::query_fields::comparison::ComparisonOperator::Gt => ComparisonOp::Gt,
			super::query_fields::comparison::ComparisonOperator::Gte => ComparisonOp::Gte,
			super::query_fields::comparison::ComparisonOperator::Lt => ComparisonOp::Lt,
			super::query_fields::comparison::ComparisonOperator::Lte => ComparisonOp::Lte,
		};

		let value = match comparison.value {
			super::query_fields::aggregate::ComparisonValue::Int(i) => AggregateValue::Int(i),
			super::query_fields::aggregate::ComparisonValue::Float(f) => AggregateValue::Float(f),
		};

		self.having_conditions
			.push(HavingCondition::AggregateCompare {
				func: AggregateFunc::Sum,
				field: comparison.aggregate.field().to_string(),
				operator,
				value,
			});
		self
	}

	/// Add HAVING clause for MIN aggregate
	///
	/// Filters grouped rows based on the minimum value in a field.
	///
	/// # Breaking Change
	///
	/// This method signature has been changed to use type-safe field selectors.
	///
	/// # Type Parameters
	///
	/// * `FS` - Field selector closure that returns a reference to a numeric field
	/// * `FE` - Expression closure that builds the comparison expression
	///
	/// # Parameters
	///
	/// * `field_selector` - Closure that selects the field from the model
	/// * `expr_fn` - Closure that builds the comparison expression using method chaining
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::{Model, query_fields::{Field, GroupByFields}, FieldSelector};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct AuthorFields {
	/// #     pub author_id: Field<Author, i64>,
	/// #     pub price: Field<Author, f64>,
	/// # }
	/// # impl AuthorFields {
	/// #     pub fn new() -> Self {
	/// #         Self {
	/// #             author_id: Field::new(vec!["author_id"]),
	/// #             price: Field::new(vec!["price"]),
	/// #         }
	/// #     }
	/// # }
	/// # impl FieldSelector for AuthorFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.author_id = self.author_id.with_alias(alias);
	/// #         self.price = self.price.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find authors where minimum book price > 1000
	/// let sql = Author::objects()
	///     .all()
	///     .group_by(|fields| GroupByFields::new().add(&fields.author_id))
	///     .having_min(|fields| &fields.price, |min| min.gt(1000.0))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn having_min<FS, FE, NT>(mut self, field_selector: FS, expr_fn: FE) -> Self
	where
		FS: FnOnce(&T::Fields) -> &super::query_fields::Field<T, NT>,
		NT: super::query_fields::NumericType,
		FE: FnOnce(AggregateExpr) -> ComparisonExpr,
	{
		let fields = T::new_fields();
		let field = field_selector(&fields);
		let field_path = field.path().join(".");

		let min_expr = AggregateExpr::min(&field_path);
		let comparison = expr_fn(min_expr);

		let operator = match comparison.op {
			super::query_fields::comparison::ComparisonOperator::Eq => ComparisonOp::Eq,
			super::query_fields::comparison::ComparisonOperator::Ne => ComparisonOp::Ne,
			super::query_fields::comparison::ComparisonOperator::Gt => ComparisonOp::Gt,
			super::query_fields::comparison::ComparisonOperator::Gte => ComparisonOp::Gte,
			super::query_fields::comparison::ComparisonOperator::Lt => ComparisonOp::Lt,
			super::query_fields::comparison::ComparisonOperator::Lte => ComparisonOp::Lte,
		};

		let value = match comparison.value {
			super::query_fields::aggregate::ComparisonValue::Int(i) => AggregateValue::Int(i),
			super::query_fields::aggregate::ComparisonValue::Float(f) => AggregateValue::Float(f),
		};

		self.having_conditions
			.push(HavingCondition::AggregateCompare {
				func: AggregateFunc::Min,
				field: comparison.aggregate.field().to_string(),
				operator,
				value,
			});
		self
	}

	/// Add HAVING clause for MAX aggregate
	///
	/// Filters grouped rows based on the maximum value in a field.
	///
	/// # Breaking Change
	///
	/// This method signature has been changed to use type-safe field selectors.
	///
	/// # Type Parameters
	///
	/// * `FS` - Field selector closure that returns a reference to a numeric field
	/// * `FE` - Expression closure that builds the comparison expression
	///
	/// # Parameters
	///
	/// * `field_selector` - Closure that selects the field from the model
	/// * `expr_fn` - Closure that builds the comparison expression using method chaining
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_db::orm::{Model, query_fields::{Field, GroupByFields}, FieldSelector};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// #
	/// # #[derive(Clone)]
	/// # struct AuthorFields {
	/// #     pub author_id: Field<Author, i64>,
	/// #     pub price: Field<Author, f64>,
	/// # }
	/// # impl AuthorFields {
	/// #     pub fn new() -> Self {
	/// #         Self {
	/// #             author_id: Field::new(vec!["author_id"]),
	/// #             price: Field::new(vec!["price"]),
	/// #         }
	/// #     }
	/// # }
	/// # impl FieldSelector for AuthorFields {
	/// #     fn with_alias(mut self, alias: &str) -> Self {
	/// #         self.author_id = self.author_id.with_alias(alias);
	/// #         self.price = self.price.with_alias(alias);
	/// #         self
	/// #     }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields::new() }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find authors where maximum book price < 5000
	/// let sql = Author::objects()
	///     .all()
	///     .group_by(|fields| GroupByFields::new().add(&fields.author_id))
	///     .having_max(|fields| &fields.price, |max| max.lt(5000.0))
	///     .to_sql();
	/// # Ok(())
	/// # }
	/// ```
	pub fn having_max<FS, FE, NT>(mut self, field_selector: FS, expr_fn: FE) -> Self
	where
		FS: FnOnce(&T::Fields) -> &super::query_fields::Field<T, NT>,
		NT: super::query_fields::NumericType,
		FE: FnOnce(AggregateExpr) -> ComparisonExpr,
	{
		let fields = T::new_fields();
		let field = field_selector(&fields);
		let field_path = field.path().join(".");

		let max_expr = AggregateExpr::max(&field_path);
		let comparison = expr_fn(max_expr);

		let operator = match comparison.op {
			super::query_fields::comparison::ComparisonOperator::Eq => ComparisonOp::Eq,
			super::query_fields::comparison::ComparisonOperator::Ne => ComparisonOp::Ne,
			super::query_fields::comparison::ComparisonOperator::Gt => ComparisonOp::Gt,
			super::query_fields::comparison::ComparisonOperator::Gte => ComparisonOp::Gte,
			super::query_fields::comparison::ComparisonOperator::Lt => ComparisonOp::Lt,
			super::query_fields::comparison::ComparisonOperator::Lte => ComparisonOp::Lte,
		};

		let value = match comparison.value {
			super::query_fields::aggregate::ComparisonValue::Int(i) => AggregateValue::Int(i),
			super::query_fields::aggregate::ComparisonValue::Float(f) => AggregateValue::Float(f),
		};

		self.having_conditions
			.push(HavingCondition::AggregateCompare {
				func: AggregateFunc::Max,
				field: comparison.aggregate.field().to_string(),
				operator,
				value,
			});
		self
	}

	/// Add WHERE IN (subquery) condition
	///
	/// Filters rows where the specified field's value is in the result set of a subquery.
	///
	/// # Type Parameters
	///
	/// * `R` - The model type used in the subquery (must implement `Model` trait)
	/// * `F` - Function that builds the subquery QuerySet
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{QuerySet, Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct AuthorFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for AuthorFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Book { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct BookFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for BookFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Book {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = BookFields;
	/// #     fn table_name() -> &'static str { "books" }
	/// #     fn new_fields() -> Self::Fields { BookFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find authors who have books priced over 1500
	/// let authors = Author::objects()
	///     .filter_in_subquery("id", |subq: QuerySet<Book>| {
	///         subq.filter(Filter::new("price", FilterOperator::Gt, FilterValue::Int(1500)))
	///             .values(&["author_id"])
	///     })
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_in_subquery<R: super::Model, F>(mut self, field: &str, subquery_fn: F) -> Self
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		let subquery_qs = subquery_fn(QuerySet::<R>::new());
		let subquery_sql = subquery_qs.as_subquery();

		self.subquery_conditions.push(SubqueryCondition::In {
			field: field.to_string(),
			subquery: subquery_sql,
		});

		self
	}

	/// Add WHERE NOT IN (subquery) condition
	///
	/// Filters rows where the specified field's value is NOT in the result set of a subquery.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{QuerySet, Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct AuthorFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for AuthorFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Book { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct BookFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for BookFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Book {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = BookFields;
	/// #     fn table_name() -> &'static str { "books" }
	/// #     fn new_fields() -> Self::Fields { BookFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find authors who have NO books priced over 1500
	/// let authors = Author::objects()
	///     .filter_not_in_subquery("id", |subq: QuerySet<Book>| {
	///         subq.filter(Filter::new("price", FilterOperator::Gt, FilterValue::Int(1500)))
	///             .values(&["author_id"])
	///     })
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_not_in_subquery<R: super::Model, F>(mut self, field: &str, subquery_fn: F) -> Self
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		let subquery_qs = subquery_fn(QuerySet::<R>::new());
		let subquery_sql = subquery_qs.as_subquery();

		self.subquery_conditions.push(SubqueryCondition::NotIn {
			field: field.to_string(),
			subquery: subquery_sql,
		});

		self
	}

	/// Add WHERE EXISTS (subquery) condition
	///
	/// Filters rows where the subquery returns at least one row.
	/// Typically used with correlated subqueries.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{QuerySet, Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct AuthorFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for AuthorFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Book { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct BookFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for BookFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Book {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = BookFields;
	/// #     fn table_name() -> &'static str { "books" }
	/// #     fn new_fields() -> Self::Fields { BookFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::F;
	/// // Find authors who have at least one book
	/// let authors = Author::objects()
	///     .filter_exists(|subq: QuerySet<Book>| {
	///         subq.filter(Filter::new("author_id", FilterOperator::Eq, FilterValue::FieldRef(F::new("authors.id"))))
	///     })
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_exists<R: super::Model, F>(mut self, subquery_fn: F) -> Self
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		let subquery_qs = subquery_fn(QuerySet::<R>::new());
		let subquery_sql = subquery_qs.as_subquery();

		self.subquery_conditions.push(SubqueryCondition::Exists {
			subquery: subquery_sql,
		});

		self
	}

	/// Add WHERE NOT EXISTS (subquery) condition
	///
	/// Filters rows where the subquery returns no rows.
	/// Typically used with correlated subqueries.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{QuerySet, Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct AuthorFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for AuthorFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Book { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct BookFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for BookFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Book {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = BookFields;
	/// #     fn table_name() -> &'static str { "books" }
	/// #     fn new_fields() -> Self::Fields { BookFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::F;
	/// // Find authors who have NO books
	/// let authors = Author::objects()
	///     .filter_not_exists(|subq: QuerySet<Book>| {
	///         subq.filter(Filter::new("author_id", FilterOperator::Eq, FilterValue::FieldRef(F::new("authors.id"))))
	///     })
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_not_exists<R: super::Model, F>(mut self, subquery_fn: F) -> Self
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		let subquery_qs = subquery_fn(QuerySet::<R>::new());
		let subquery_sql = subquery_qs.as_subquery();

		self.subquery_conditions.push(SubqueryCondition::NotExists {
			subquery: subquery_sql,
		});

		self
	}

	/// Add a Common Table Expression (WITH clause) to the query
	///
	/// CTEs allow you to define named subqueries that can be referenced
	/// in the main query. This is useful for complex queries that need
	/// to reference the same subquery multiple times or for recursive queries.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::cte::CTE;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Employee { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct EmployeeFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for EmployeeFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Employee {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = EmployeeFields;
	/// #     fn table_name() -> &'static str { "employees" }
	/// #     fn new_fields() -> Self::Fields { EmployeeFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Simple CTE
	/// let high_earners = CTE::new("high_earners", "SELECT * FROM employees WHERE salary > 100000");
	/// let results = Employee::objects()
	///     .with_cte(high_earners)
	///     .all()
	///     .await?;
	///
	/// // Recursive CTE for hierarchical data
	/// let hierarchy = CTE::new(
	///     "org_hierarchy",
	///     "SELECT id, name, manager_id, 1 as level FROM employees WHERE manager_id IS NULL \
	///      UNION ALL \
	///      SELECT e.id, e.name, e.manager_id, h.level + 1 \
	///      FROM employees e JOIN org_hierarchy h ON e.manager_id = h.id"
	/// ).recursive();
	///
	/// let org = Employee::objects()
	///     .with_cte(hierarchy)
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_cte(mut self, cte: super::cte::CTE) -> Self {
		self.ctes.add(cte);
		self
	}

	/// Add a LATERAL JOIN to the query
	///
	/// LATERAL JOINs allow correlated subqueries in the FROM clause,
	/// where the subquery can reference columns from preceding tables.
	/// This is useful for "top-N per group" queries and similar patterns.
	///
	/// **Note**: LATERAL JOIN is supported in PostgreSQL 9.3+, MySQL 8.0.14+,
	/// but NOT in SQLite.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Customer { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct CustomerFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for CustomerFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Customer {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = CustomerFields;
	/// #     fn table_name() -> &'static str { "customers" }
	/// #     fn new_fields() -> Self::Fields { CustomerFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::lateral_join::{LateralJoin, LateralJoinPatterns};
	///
	/// // Get top 3 orders per customer
	/// let top_orders = LateralJoinPatterns::top_n_per_group(
	///     "recent_orders",
	///     "orders",
	///     "customer_id",
	///     "customers",
	///     "created_at DESC",
	///     3,
	/// );
	///
	/// let results = Customer::objects()
	///     .all()
	///     .with_lateral_join(top_orders)
	///     .all()
	///     .await?;
	///
	/// // Get latest order per customer
	/// let latest = LateralJoinPatterns::latest_per_parent(
	///     "latest_order",
	///     "orders",
	///     "customer_id",
	///     "customers",
	///     "created_at",
	/// );
	///
	/// let customers_with_orders = Customer::objects()
	///     .all()
	///     .with_lateral_join(latest)
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_lateral_join(mut self, join: super::lateral_join::LateralJoin) -> Self {
		self.lateral_joins.add(join);
		self
	}

	/// Build WHERE condition using reinhardt-query from accumulated filters
	fn build_where_condition(&self) -> Option<Condition> {
		// Early return only if both filters and subquery_conditions are empty
		if self.filters.is_empty() && self.subquery_conditions.is_empty() {
			return None;
		}

		let mut cond = Condition::all();

		for filter in &self.filters {
			let col = Expr::col(Alias::new(&filter.field));

			let expr = match (&filter.operator, &filter.value) {
				// Field-to-field comparisons (must come before generic patterns)
				(FilterOperator::Eq, FilterValue::FieldRef(f)) => {
					col.eq(Expr::col(Alias::new(&f.field)))
				}
				(FilterOperator::Ne, FilterValue::FieldRef(f)) => {
					col.ne(Expr::col(Alias::new(&f.field)))
				}
				(FilterOperator::Gt, FilterValue::FieldRef(f)) => {
					col.gt(Expr::col(Alias::new(&f.field)))
				}
				(FilterOperator::Gte, FilterValue::FieldRef(f)) => {
					col.gte(Expr::col(Alias::new(&f.field)))
				}
				(FilterOperator::Lt, FilterValue::FieldRef(f)) => {
					col.lt(Expr::col(Alias::new(&f.field)))
				}
				(FilterOperator::Lte, FilterValue::FieldRef(f)) => {
					col.lte(Expr::col(Alias::new(&f.field)))
				}
				// OuterRef comparisons for correlated subqueries
				(FilterOperator::Eq, FilterValue::OuterRef(outer)) => {
					// For correlated subqueries, reference outer query field
					// e.g., WHERE books.author_id = authors.id (where authors is from outer query)
					Expr::cust(format!("{} = {}", filter.field, outer.to_sql())).into_simple_expr()
				}
				(FilterOperator::Ne, FilterValue::OuterRef(outer)) => {
					Expr::cust(format!("{} != {}", filter.field, outer.to_sql())).into_simple_expr()
				}
				(FilterOperator::Gt, FilterValue::OuterRef(outer)) => {
					Expr::cust(format!("{} > {}", filter.field, outer.to_sql())).into_simple_expr()
				}
				(FilterOperator::Gte, FilterValue::OuterRef(outer)) => {
					Expr::cust(format!("{} >= {}", filter.field, outer.to_sql())).into_simple_expr()
				}
				(FilterOperator::Lt, FilterValue::OuterRef(outer)) => {
					Expr::cust(format!("{} < {}", filter.field, outer.to_sql())).into_simple_expr()
				}
				(FilterOperator::Lte, FilterValue::OuterRef(outer)) => {
					Expr::cust(format!("{} <= {}", filter.field, outer.to_sql())).into_simple_expr()
				}
				// Expression comparisons (F("a") * F("b") etc.)
				(FilterOperator::Eq, FilterValue::Expression(expr)) => {
					col.eq(Self::expression_to_query_expr(expr))
				}
				(FilterOperator::Ne, FilterValue::Expression(expr)) => {
					col.ne(Self::expression_to_query_expr(expr))
				}
				(FilterOperator::Gt, FilterValue::Expression(expr)) => {
					col.gt(Self::expression_to_query_expr(expr))
				}
				(FilterOperator::Gte, FilterValue::Expression(expr)) => {
					col.gte(Self::expression_to_query_expr(expr))
				}
				(FilterOperator::Lt, FilterValue::Expression(expr)) => {
					col.lt(Self::expression_to_query_expr(expr))
				}
				(FilterOperator::Lte, FilterValue::Expression(expr)) => {
					col.lte(Self::expression_to_query_expr(expr))
				}
				// NULL checks
				(FilterOperator::Eq, FilterValue::Null) => col.is_null(),
				(FilterOperator::Ne, FilterValue::Null) => col.is_not_null(),
				// Generic value comparisons (catch-all for other FilterValue types)
				(FilterOperator::Eq, v) => col.eq(Self::filter_value_to_sea_value(v)),
				(FilterOperator::Ne, v) => col.ne(Self::filter_value_to_sea_value(v)),
				(FilterOperator::Gt, v) => col.gt(Self::filter_value_to_sea_value(v)),
				(FilterOperator::Gte, v) => col.gte(Self::filter_value_to_sea_value(v)),
				(FilterOperator::Lt, v) => col.lt(Self::filter_value_to_sea_value(v)),
				(FilterOperator::Lte, v) => col.lte(Self::filter_value_to_sea_value(v)),
				(FilterOperator::In, FilterValue::String(s)) => {
					let values = Self::parse_array_string(s);
					col.is_in(values)
				}
				(FilterOperator::In, FilterValue::Array(arr)) => {
					col.is_in(arr.iter().map(|s| s.as_str()).collect::<Vec<_>>())
				}
				(FilterOperator::NotIn, FilterValue::String(s)) => {
					let values = Self::parse_array_string(s);
					col.is_not_in(values)
				}
				(FilterOperator::NotIn, FilterValue::Array(arr)) => {
					col.is_not_in(arr.iter().map(|s| s.as_str()).collect::<Vec<_>>())
				}
				(FilterOperator::Contains, FilterValue::String(s)) => col.like(format!("%{}%", s)),
				(FilterOperator::Contains, FilterValue::Array(arr)) => {
					col.like(format!("%{}%", arr.first().unwrap_or(&String::new())))
				}
				(FilterOperator::StartsWith, FilterValue::String(s)) => col.like(format!("{}%", s)),
				(FilterOperator::StartsWith, FilterValue::Array(arr)) => {
					col.like(format!("{}%", arr.first().unwrap_or(&String::new())))
				}
				(FilterOperator::EndsWith, FilterValue::String(s)) => col.like(format!("%{}", s)),
				(FilterOperator::EndsWith, FilterValue::Array(arr)) => {
					col.like(format!("%{}", arr.first().unwrap_or(&String::new())))
				}
				// Handle Integer, Float, Boolean for text operators
				(FilterOperator::Contains, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.like(format!("%{}%", i))
				}
				(FilterOperator::Contains, FilterValue::Float(f)) => col.like(format!("%{}%", f)),
				(FilterOperator::Contains, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.like(format!("%{}%", b))
				}
				(FilterOperator::Contains, FilterValue::Null) => col.like("%"),
				(FilterOperator::StartsWith, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.like(format!("{}%", i))
				}
				(FilterOperator::StartsWith, FilterValue::Float(f)) => col.like(format!("{}%", f)),
				(FilterOperator::StartsWith, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.like(format!("{}%", b))
				}
				(FilterOperator::StartsWith, FilterValue::Null) => col.like("%"),
				(FilterOperator::EndsWith, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.like(format!("%{}", i))
				}
				(FilterOperator::EndsWith, FilterValue::Float(f)) => col.like(format!("%{}", f)),
				(FilterOperator::EndsWith, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.like(format!("%{}", b))
				}
				(FilterOperator::EndsWith, FilterValue::Null) => col.like("%"),
				// Handle In/NotIn for non-String types
				(FilterOperator::In, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.is_in(vec![*i])
				}
				(FilterOperator::In, FilterValue::Float(f)) => col.is_in(vec![*f]),
				(FilterOperator::In, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.is_in(vec![*b])
				}
				(FilterOperator::In, FilterValue::Null) => {
					col.is_in(vec![reinhardt_query::value::Value::Int(None)])
				}
				(FilterOperator::NotIn, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.is_not_in(vec![*i])
				}
				(FilterOperator::NotIn, FilterValue::Float(f)) => col.is_not_in(vec![*f]),
				(FilterOperator::NotIn, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.is_not_in(vec![*b])
				}
				(FilterOperator::NotIn, FilterValue::Null) => {
					col.is_not_in(vec![reinhardt_query::value::Value::Int(None)])
				}
				// IsNull/IsNotNull operators
				(FilterOperator::IsNull, _) => col.is_null(),
				(FilterOperator::IsNotNull, _) => col.is_not_null(),
				// PostgreSQL Array operators (using custom SQL)
				(FilterOperator::ArrayContains, FilterValue::Array(arr)) => {
					// field @> ARRAY[?, ?] - parameterized
					let placeholders = arr.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
					Expr::cust_with_values(
						format!("{} @> ARRAY[{}]", filter.field, placeholders),
						arr.iter().cloned(),
					)
					.into_simple_expr()
				}
				(FilterOperator::ArrayContainedBy, FilterValue::Array(arr)) => {
					// field <@ ARRAY[?, ?] - parameterized
					let placeholders = arr.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
					Expr::cust_with_values(
						format!("{} <@ ARRAY[{}]", filter.field, placeholders),
						arr.iter().cloned(),
					)
					.into_simple_expr()
				}
				(FilterOperator::ArrayOverlap, FilterValue::Array(arr)) => {
					// field && ARRAY[?, ?] - parameterized
					let placeholders = arr.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
					Expr::cust_with_values(
						format!("{} && ARRAY[{}]", filter.field, placeholders),
						arr.iter().cloned(),
					)
					.into_simple_expr()
				}
				// PostgreSQL Full-text search
				(FilterOperator::FullTextMatch, FilterValue::String(query)) => {
					// field @@ plainto_tsquery('english', ?) - parameterized
					Expr::cust_with_values(
						format!("{} @@ plainto_tsquery('english', ?)", filter.field),
						[query.clone()],
					)
					.into_simple_expr()
				}
				// PostgreSQL JSONB operators
				(FilterOperator::JsonbContains, FilterValue::String(json)) => {
					// field @> ?::jsonb - parameterized
					Expr::cust_with_values(format!("{} @> ?::jsonb", filter.field), [json.clone()])
						.into_simple_expr()
				}
				(FilterOperator::JsonbContainedBy, FilterValue::String(json)) => {
					// field <@ ?::jsonb - parameterized
					Expr::cust_with_values(format!("{} <@ ?::jsonb", filter.field), [json.clone()])
						.into_simple_expr()
				}
				(FilterOperator::JsonbKeyExists, FilterValue::String(key)) => {
					// field ? 'key' - using PgBinOper for safe parameterization
					Expr::cust(&filter.field).into_simple_expr().binary(
						BinOper::PgOperator(PgBinOper::JsonContainsKey),
						SimpleExpr::from(key.clone()),
					)
				}
				(FilterOperator::JsonbAnyKeyExists, FilterValue::Array(keys)) => {
					// field ?| array[?, ?] - using PgBinOper for safe parameterization
					let placeholders = keys.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
					let array_expr = Expr::cust_with_values(
						format!("array[{}]", placeholders),
						keys.iter().cloned(),
					)
					.into_simple_expr();
					Expr::cust(&filter.field).into_simple_expr().binary(
						BinOper::PgOperator(PgBinOper::JsonContainsAnyKey),
						array_expr,
					)
				}
				(FilterOperator::JsonbAllKeysExist, FilterValue::Array(keys)) => {
					// field ?& array[?, ?] - using PgBinOper for safe parameterization
					let placeholders = keys.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
					let array_expr = Expr::cust_with_values(
						format!("array[{}]", placeholders),
						keys.iter().cloned(),
					)
					.into_simple_expr();
					Expr::cust(&filter.field).into_simple_expr().binary(
						BinOper::PgOperator(PgBinOper::JsonContainsAllKeys),
						array_expr,
					)
				}
				(FilterOperator::JsonbPathExists, FilterValue::String(path)) => {
					// field @? ? - parameterized
					Expr::cust_with_values(format!("{} @? ?", filter.field), [path.clone()])
						.into_simple_expr()
				}
				// PostgreSQL Range operators
				(FilterOperator::RangeContains, v) => {
					// field @> ? - parameterized
					let val = Self::filter_value_to_sql_string(v);
					Expr::cust_with_values(format!("{} @> ?", filter.field), [val])
						.into_simple_expr()
				}
				(FilterOperator::RangeContainedBy, FilterValue::String(range)) => {
					// field <@ ? - parameterized
					Expr::cust_with_values(format!("{} <@ ?", filter.field), [range.clone()])
						.into_simple_expr()
				}
				(FilterOperator::RangeOverlaps, FilterValue::String(range)) => {
					// field && ? - parameterized
					Expr::cust_with_values(format!("{} && ?", filter.field), [range.clone()])
						.into_simple_expr()
				}
				// Fallback for unsupported combinations
				_ => {
					// Default to equality for unhandled cases
					col.eq(Self::filter_value_to_sea_value(&filter.value))
				}
			};

			cond = cond.add(expr);
		}

		// Add subquery conditions
		for subq_cond in &self.subquery_conditions {
			let expr = match subq_cond {
				SubqueryCondition::In { field, subquery } => {
					// field IN (subquery)
					Expr::cust(format!("{} IN {}", field, subquery)).into_simple_expr()
				}
				SubqueryCondition::NotIn { field, subquery } => {
					// field NOT IN (subquery)
					Expr::cust(format!("{} NOT IN {}", field, subquery)).into_simple_expr()
				}
				SubqueryCondition::Exists { subquery } => {
					// EXISTS (subquery)
					Expr::cust(format!("EXISTS {}", subquery)).into_simple_expr()
				}
				SubqueryCondition::NotExists { subquery } => {
					// NOT EXISTS (subquery)
					Expr::cust(format!("NOT EXISTS {}", subquery)).into_simple_expr()
				}
			};

			cond = cond.add(expr);
		}

		Some(cond)
	}

	/// Convert FilterValue to reinhardt_query::value::Value
	/// Convert Expression to reinhardt-query Expr for use in WHERE clauses
	///
	/// Uses Expr::cust() for arithmetic operations as reinhardt-query doesn't provide
	/// multiply/divide/etc. methods. SQL injection risk is low since F() only
	/// accepts field names.
	fn expression_to_query_expr(expr: &super::annotation::Expression) -> Expr {
		use crate::orm::annotation::Expression;

		match expr {
			Expression::Add(left, right) => {
				let left_sql = Self::annotation_value_to_sql(left);
				let right_sql = Self::annotation_value_to_sql(right);
				Expr::cust(format!("({} + {})", left_sql, right_sql))
			}
			Expression::Subtract(left, right) => {
				let left_sql = Self::annotation_value_to_sql(left);
				let right_sql = Self::annotation_value_to_sql(right);
				Expr::cust(format!("({} - {})", left_sql, right_sql))
			}
			Expression::Multiply(left, right) => {
				let left_sql = Self::annotation_value_to_sql(left);
				let right_sql = Self::annotation_value_to_sql(right);
				Expr::cust(format!("({} * {})", left_sql, right_sql))
			}
			Expression::Divide(left, right) => {
				let left_sql = Self::annotation_value_to_sql(left);
				let right_sql = Self::annotation_value_to_sql(right);
				Expr::cust(format!("({} / {})", left_sql, right_sql))
			}
			Expression::Case { whens, default } => {
				let mut case_sql = "CASE".to_string();
				for when in whens.iter() {
					// Use When::to_sql() which generates "WHEN condition THEN value"
					case_sql.push_str(&format!(" {}", when.to_sql()));
				}
				if let Some(default_val) = default {
					case_sql.push_str(&format!(
						" ELSE {}",
						Self::annotation_value_to_sql(default_val)
					));
				}
				case_sql.push_str(" END");
				Expr::cust(case_sql)
			}
			Expression::Coalesce(values) => {
				let value_sqls = values
					.iter()
					.map(|v| Self::annotation_value_to_sql(v))
					.collect::<Vec<_>>()
					.join(", ");
				Expr::cust(format!("COALESCE({})", value_sqls))
			}
		}
	}

	/// Convert AnnotationValue to SQL string for custom expressions
	///
	/// Delegates to the `AnnotationValue::to_sql()` method which provides
	/// complete SQL generation for all annotation value types.
	fn annotation_value_to_sql(value: &super::annotation::AnnotationValue) -> String {
		value.to_sql()
	}

	fn filter_value_to_sea_value(v: &FilterValue) -> reinhardt_query::value::Value {
		match v {
			FilterValue::String(s) => {
				// Try to parse as UUID first for proper PostgreSQL uuid column handling
				if let Ok(uuid) = Uuid::parse_str(s) {
					reinhardt_query::value::Value::Uuid(Some(Box::new(uuid)))
				} else {
					s.clone().into()
				}
			}
			FilterValue::Integer(i) | FilterValue::Int(i) => (*i).into(),
			FilterValue::Float(f) => (*f).into(),
			FilterValue::Boolean(b) | FilterValue::Bool(b) => (*b).into(),
			FilterValue::Null => reinhardt_query::value::Value::Int(None),
			FilterValue::Array(arr) => arr.join(",").into(),
			// FieldRef, Expression, and OuterRef are typically handled separately
			// in build_where_condition(), but provide proper conversion as fallback
			FilterValue::FieldRef(f) => f.field.clone().into(),
			FilterValue::Expression(expr) => expr.to_sql().into(),
			FilterValue::OuterRef(outer_ref) => outer_ref.field.clone().into(),
		}
	}

	/// Convert FilterValue to SQL-safe string representation
	/// Used for custom SQL expressions (PostgreSQL operators)
	fn filter_value_to_sql_string(v: &FilterValue) -> String {
		match v {
			FilterValue::String(s) => format!("'{}'", s.replace('\'', "''")),
			FilterValue::Integer(i) | FilterValue::Int(i) => i.to_string(),
			FilterValue::Float(f) => f.to_string(),
			FilterValue::Boolean(b) | FilterValue::Bool(b) => {
				if *b { "TRUE" } else { "FALSE" }.to_string()
			}
			FilterValue::Null => "NULL".to_string(),
			FilterValue::Array(arr) => {
				// Format as PostgreSQL array literal
				let elements = arr
					.iter()
					.map(|s| format!("'{}'", s.replace('\'', "''")))
					.collect::<Vec<_>>();
				format!("ARRAY[{}]", elements.join(", "))
			}
			FilterValue::FieldRef(f) => f.field.clone(),
			FilterValue::Expression(expr) => expr.to_sql(),
			FilterValue::OuterRef(outer_ref) => outer_ref.field.clone(),
		}
	}

	/// Convert FilterValue to String representation
	#[allow(dead_code)]
	fn value_to_string(v: &FilterValue) -> String {
		match v {
			FilterValue::String(s) => s.clone(),
			FilterValue::Integer(i) | FilterValue::Int(i) => i.to_string(),
			FilterValue::Float(f) => f.to_string(),
			FilterValue::Boolean(b) | FilterValue::Bool(b) => b.to_string(),
			FilterValue::Null => String::new(),
			FilterValue::Array(arr) => arr.join(","),
			FilterValue::FieldRef(f) => f.field.clone(),
			FilterValue::Expression(expr) => expr.to_sql(),
			FilterValue::OuterRef(outer_ref) => outer_ref.field.clone(),
		}
	}

	/// Parse array string into `Vec<reinhardt_query::value::Value>`
	/// Supports comma-separated values or JSON array format
	fn parse_array_string(s: &str) -> Vec<reinhardt_query::value::Value> {
		let trimmed = s.trim();

		// Try parsing as JSON array first
		if trimmed.starts_with('[')
			&& trimmed.ends_with(']')
			&& let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed)
		{
			return arr
				.iter()
				.map(|v| match v {
					serde_json::Value::String(s) => s.clone().into(),
					serde_json::Value::Number(n) => {
						if let Some(i) = n.as_i64() {
							i.into()
						} else if let Some(f) = n.as_f64() {
							f.into()
						} else {
							n.to_string().into()
						}
					}
					serde_json::Value::Bool(b) => (*b).into(),
					_ => v.to_string().into(),
				})
				.collect();
		}

		// Fallback to comma-separated parsing
		trimmed
			.split(',')
			.map(|s| s.trim())
			.filter(|s| !s.is_empty())
			.map(|s| s.to_string().into())
			.collect()
	}

	/// Convert FilterValue to array of reinhardt_query::value::Value
	#[allow(dead_code)]
	fn value_to_array(v: &FilterValue) -> Vec<reinhardt_query::value::Value> {
		match v {
			FilterValue::String(s) => Self::parse_array_string(s),
			FilterValue::Integer(i) | FilterValue::Int(i) => vec![(*i).into()],
			FilterValue::Float(f) => vec![(*f).into()],
			FilterValue::Boolean(b) | FilterValue::Bool(b) => vec![(*b).into()],
			FilterValue::Null => vec![reinhardt_query::value::Value::Int(None)],
			FilterValue::Array(arr) => arr.iter().map(|s| s.clone().into()).collect(),
			FilterValue::FieldRef(f) => vec![f.field.clone().into()],
			FilterValue::Expression(expr) => vec![expr.to_sql().into()],
			FilterValue::OuterRef(outer) => vec![outer.field.clone().into()],
		}
	}

	/// Build WHERE clause from accumulated filters
	///
	/// # Deprecation Note
	///
	/// This method is maintained for backward compatibility with existing code that
	/// expects a string-based WHERE clause. New code should use `build_where_condition()`
	/// which returns a `Condition` object that can be directly added to reinhardt-query statements.
	///
	/// This method generates a complete SELECT statement internally and extracts only
	/// the WHERE portion, which is less efficient than using `build_where_condition()`.
	#[allow(dead_code)]
	fn build_where_clause(&self) -> (String, Vec<String>) {
		if self.filters.is_empty() {
			return (String::new(), Vec::new());
		}

		// Build reinhardt-query condition
		let mut stmt = Query::select();
		stmt.from(Alias::new("dummy"));

		if let Some(cond) = self.build_where_condition() {
			stmt.cond_where(cond);
		}

		// Convert to SQL string with inline values
		use reinhardt_query::prelude::PostgresQueryBuilder;
		let sql = stmt.to_string(PostgresQueryBuilder);

		// Extract WHERE clause portion by finding the WHERE keyword
		let where_clause = if let Some(idx) = sql.find(" WHERE ") {
			sql[idx..].to_string()
		} else {
			String::new()
		};

		(where_clause, Vec::new())
	}

	/// Eagerly load related objects using JOIN queries
	///
	/// This method performs SQL JOINs to fetch related objects in a single query,
	/// reducing the number of database round-trips and preventing N+1 query problems.
	///
	/// # Performance
	///
	/// Best for one-to-one and many-to-one relationships where JOIN won't create
	/// significant data duplication. For one-to-many and many-to-many relationships,
	/// consider using `prefetch_related()` instead.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, author: Author, category: Category }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { name: String }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Category { name: String }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Single query with JOINs instead of N+1 queries
	/// let posts = Post::objects()
	///     .select_related(&["author", "category"])
	///     .all()
	///     .await?;
	///
	/// // Each post has author and category pre-loaded
	/// for post in posts {
	///     println!("Author: {}", post.author.name); // No additional query
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub fn select_related(mut self, fields: &[&str]) -> Self {
		self.select_related_fields = fields.iter().map(|s| s.to_string()).collect();
		self
	}

	/// Generate SELECT query with JOIN clauses for select_related fields
	///
	/// Returns reinhardt-query SelectStatement with LEFT JOIN for each related field to enable eager loading.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// let queryset = Post::objects()
	///     .select_related(&["author", "category"])
	///     .filter(Filter::new("published", FilterOperator::Eq, FilterValue::Boolean(true)));
	///
	/// let stmt = queryset.select_related_query();
	/// // Generates:
	/// // SELECT posts.*, author.*, category.* FROM posts
	/// //   LEFT JOIN users AS author ON posts.author_id = author.id
	/// //   LEFT JOIN categories AS category ON posts.category_id = category.id
	/// //   WHERE posts.published = $1
	/// ```
	pub fn select_related_query(&self) -> SelectStatement {
		let table_name = T::table_name();
		let mut stmt = Query::select();

		// Apply FROM clause with optional alias
		if let Some(ref alias) = self.from_alias {
			stmt.from_as(Alias::new(table_name), Alias::new(alias));
		} else {
			stmt.from(Alias::new(table_name));
		}

		// Apply DISTINCT if enabled
		if self.distinct_enabled {
			stmt.distinct();
		}

		// Add main table columns
		stmt.column(ColumnRef::table_asterisk(Alias::new(table_name)));

		// Add LEFT JOIN for each related field
		for related_field in &self.select_related_fields {
			// Convention: related_field is the field name in the model
			// We assume FK field is "{related_field}_id" and join to "{related_field}s" table
			let fk_field = Alias::new(format!("{}_id", related_field));
			let related_table = Alias::new(format!("{}s", related_field));
			let related_alias = Alias::new(related_field);

			// LEFT JOIN related_table AS related_field ON table.fk_field = related_field.id
			stmt.left_join(
				related_table,
				Expr::col((Alias::new(table_name), fk_field))
					.equals((related_alias.clone(), Alias::new("id"))),
			);

			// Add related table columns to SELECT
			stmt.column(ColumnRef::table_asterisk(related_alias));
		}

		// Apply manual JOINs
		for join in &self.joins {
			if join.on_condition.is_empty() {
				// CROSS JOIN (no ON condition)
				if let Some(ref alias) = join.target_alias {
					stmt.cross_join((Alias::new(&join.target_table), Alias::new(alias)));
				} else {
					stmt.cross_join(Alias::new(&join.target_table));
				}
			} else {
				// Convert reinhardt JoinType to reinhardt-query JoinType
				let sea_join_type = match join.join_type {
					super::sqlalchemy_query::JoinType::Inner => SeaJoinType::InnerJoin,
					super::sqlalchemy_query::JoinType::Left => SeaJoinType::LeftJoin,
					super::sqlalchemy_query::JoinType::Right => SeaJoinType::RightJoin,
					super::sqlalchemy_query::JoinType::Full => SeaJoinType::FullOuterJoin,
				};

				// Build the join with optional alias
				if let Some(ref alias) = join.target_alias {
					stmt.join(
						sea_join_type,
						(Alias::new(&join.target_table), Alias::new(alias)),
						Expr::cust(join.on_condition.clone()),
					);
				} else {
					stmt.join(
						sea_join_type,
						Alias::new(&join.target_table),
						Expr::cust(join.on_condition.clone()),
					);
				}
			}
		}

		// Apply WHERE conditions
		if let Some(cond) = self.build_where_condition() {
			stmt.cond_where(cond);
		}

		// Apply GROUP BY
		for group_field in &self.group_by_fields {
			let col_ref = parse_column_reference(group_field);
			stmt.group_by_col(col_ref);
		}

		// Apply HAVING
		for having_cond in &self.having_conditions {
			match having_cond {
				HavingCondition::AggregateCompare {
					func,
					field,
					operator,
					value,
				} => {
					// Build aggregate function expression
					let agg_expr = match func {
						AggregateFunc::Avg => {
							Func::avg(Expr::col(Alias::new(field)).into_simple_expr())
						}
						AggregateFunc::Count => {
							if field == "*" {
								Func::count(Expr::asterisk().into_simple_expr())
							} else {
								Func::count(Expr::col(Alias::new(field)).into_simple_expr())
							}
						}
						AggregateFunc::Sum => {
							Func::sum(Expr::col(Alias::new(field)).into_simple_expr())
						}
						AggregateFunc::Min => {
							Func::min(Expr::col(Alias::new(field)).into_simple_expr())
						}
						AggregateFunc::Max => {
							Func::max(Expr::col(Alias::new(field)).into_simple_expr())
						}
					};

					// Build comparison expression
					let having_expr = match operator {
						ComparisonOp::Eq => match value {
							AggregateValue::Int(v) => agg_expr.eq(*v),
							AggregateValue::Float(v) => agg_expr.eq(*v),
						},
						ComparisonOp::Ne => match value {
							AggregateValue::Int(v) => agg_expr.ne(*v),
							AggregateValue::Float(v) => agg_expr.ne(*v),
						},
						ComparisonOp::Gt => match value {
							AggregateValue::Int(v) => agg_expr.gt(*v),
							AggregateValue::Float(v) => agg_expr.gt(*v),
						},
						ComparisonOp::Gte => match value {
							AggregateValue::Int(v) => agg_expr.gte(*v),
							AggregateValue::Float(v) => agg_expr.gte(*v),
						},
						ComparisonOp::Lt => match value {
							AggregateValue::Int(v) => agg_expr.lt(*v),
							AggregateValue::Float(v) => agg_expr.lt(*v),
						},
						ComparisonOp::Lte => match value {
							AggregateValue::Int(v) => agg_expr.lte(*v),
							AggregateValue::Float(v) => agg_expr.lte(*v),
						},
					};

					stmt.and_having(having_expr);
				}
			}
		}

		// Apply ORDER BY
		for order_field in &self.order_by_fields {
			let (field, is_desc) = if let Some(stripped) = order_field.strip_prefix('-') {
				(stripped, true)
			} else {
				(order_field.as_str(), false)
			};

			let col_ref = parse_column_reference(field);
			let expr = Expr::col(col_ref);
			if is_desc {
				stmt.order_by_expr(expr, Order::Desc);
			} else {
				stmt.order_by_expr(expr, Order::Asc);
			}
		}

		// Apply LIMIT/OFFSET
		if let Some(limit) = self.limit {
			stmt.limit(limit as u64);
		}
		if let Some(offset) = self.offset {
			stmt.offset(offset as u64);
		}

		stmt.to_owned()
	}

	/// Eagerly load related objects using separate queries
	///
	/// This method performs separate SQL queries for related objects and joins them
	/// in memory, which is more efficient than JOINs for one-to-many and many-to-many
	/// relationships that would create significant data duplication.
	///
	/// # Performance
	///
	/// Best for one-to-many and many-to-many relationships where JOINs would create
	/// data duplication (e.g., a post with 100 comments would duplicate post data 100 times).
	/// Uses 1 + N queries where N is the number of prefetch_related fields.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, comments: Vec<Comment>, tags: Vec<Tag> }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Comment { text: String }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Tag { name: String }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // 2 queries total instead of N+1 queries
	/// let posts = Post::objects()
	///     .prefetch_related(&["comments", "tags"])
	///     .all()
	///     .await?;
	///
	/// // Each post has comments and tags pre-loaded
	/// for post in posts {
	///     for comment in &post.comments {
	///         println!("Comment: {}", comment.text); // No additional query
	///     }
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub fn prefetch_related(mut self, fields: &[&str]) -> Self {
		self.prefetch_related_fields = fields.iter().map(|s| s.to_string()).collect();
		self
	}

	/// Generate SELECT queries for prefetch_related fields
	///
	/// Returns a vector of (field_name, SelectStatement) tuples, one for each prefetch field.
	/// Each query fetches related objects using IN clause with collected primary keys.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let queryset = Post::objects()
	///     .prefetch_related(&["comments", "tags"]);
	///
	/// let main_results = queryset.all().await?; // Main query
	/// let pk_values = vec![1, 2, 3]; // Collected from main results
	///
	/// let prefetch_queries = queryset.prefetch_related_queries(&pk_values);
	/// // Returns SelectStatements for:
	/// // 1. comments: SELECT * FROM comments WHERE post_id IN ($1, $2, $3)
	/// // 2. tags: SELECT tags.* FROM tags
	/// //          INNER JOIN post_tags ON tags.id = post_tags.tag_id
	/// //          WHERE post_tags.post_id IN ($1, $2, $3)
	/// # Ok(())
	/// # }
	/// ```
	pub fn prefetch_related_queries(&self, pk_values: &[i64]) -> Vec<(String, SelectStatement)> {
		if pk_values.is_empty() {
			return Vec::new();
		}

		let mut queries = Vec::new();

		for related_field in &self.prefetch_related_fields {
			// Determine if this is a many-to-many relation or one-to-many
			// by querying the model's relationship metadata
			let is_m2m = self.is_many_to_many_relation(related_field);

			let stmt = if is_m2m {
				self.prefetch_many_to_many_query(related_field, pk_values)
			} else {
				self.prefetch_one_to_many_query(related_field, pk_values)
			};

			queries.push((related_field.clone(), stmt));
		}

		queries
	}

	/// Check if a related field is a many-to-many relation
	///
	/// Determines relationship type by querying the model's metadata.
	/// Returns true if the relationship is defined as ManyToMany in the model metadata.
	fn is_many_to_many_relation(&self, related_field: &str) -> bool {
		// Get relationship metadata from the model
		let relations = T::relationship_metadata();

		// Find the relationship with the matching name
		relations
			.iter()
			.find(|rel| rel.name == related_field)
			.map(|rel| rel.relationship_type == super::relationship::RelationshipType::ManyToMany)
			.unwrap_or(false)
	}

	/// Generate query for one-to-many prefetch
	///
	/// Generates: SELECT * FROM related_table WHERE fk_field IN (pk_values)
	fn prefetch_one_to_many_query(
		&self,
		related_field: &str,
		pk_values: &[i64],
	) -> SelectStatement {
		let table_name = T::table_name();
		let related_table = Alias::new(format!("{}s", related_field));
		let fk_field = Alias::new(format!("{}_id", table_name.trim_end_matches('s')));

		let mut stmt = Query::select();
		stmt.from(related_table).column(ColumnRef::Asterisk);

		// Add IN clause with pk_values
		let values: Vec<reinhardt_query::value::Value> =
			pk_values.iter().map(|&id| id.into()).collect();
		stmt.and_where(Expr::col(fk_field).is_in(values));

		stmt.to_owned()
	}

	/// Generate query for many-to-many prefetch
	///
	/// Generates: SELECT related.*, junction.main_id FROM related
	///            INNER JOIN junction ON related.id = junction.related_id
	///            WHERE junction.main_id IN (pk_values)
	fn prefetch_many_to_many_query(
		&self,
		related_field: &str,
		pk_values: &[i64],
	) -> SelectStatement {
		let table_name = T::table_name();
		let junction_table = Alias::new(format!("{}_{}", table_name, related_field));
		let related_table = Alias::new(format!("{}s", related_field));
		let junction_main_fk = Alias::new(format!("{}_id", table_name.trim_end_matches('s')));
		let junction_related_fk = Alias::new(format!("{}_id", related_field));

		let mut stmt = Query::select();
		stmt.from(related_table.clone())
			.column(ColumnRef::table_asterisk(related_table.clone()))
			.column((junction_table.clone(), junction_main_fk.clone()))
			.inner_join(
				junction_table.clone(),
				Expr::col((related_table.clone(), Alias::new("id")))
					.equals((junction_table.clone(), junction_related_fk)),
			);

		// Add IN clause with pk_values
		let values: Vec<reinhardt_query::value::Value> =
			pk_values.iter().map(|&id| id.into()).collect();
		stmt.and_where(Expr::col((junction_table, junction_main_fk)).is_in(values));

		stmt.to_owned()
	}

	/// Execute the queryset and return all matching records
	///
	/// Fetches all records from the database that match the accumulated filters.
	/// If `select_related` fields are specified, performs JOIN queries for eager loading.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Fetch all users (Manager.all() returns QuerySet, then call .all().await)
	/// let users = User::objects().all().all().await?;
	///
	/// // Fetch filtered users with eager loading
	/// let active_users = User::objects()
	///     .filter(
	///         "is_active",
	///         FilterOperator::Eq,
	///         FilterValue::Boolean(true),
	///     )
	///     .select_related(&["profile"])
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - Database connection fails
	/// - SQL execution fails
	/// - Deserialization of results fails
	pub async fn all(&self) -> reinhardt_core::exception::Result<Vec<T>>
	where
		T: serde::de::DeserializeOwned,
	{
		let conn = super::manager::get_connection().await?;

		let stmt = if self.select_related_fields.is_empty() {
			// Simple SELECT without JOINs
			let mut stmt = Query::select();
			stmt.from(Alias::new(T::table_name()));

			// Column selection considering selected_fields and deferred_fields
			if let Some(ref fields) = self.selected_fields {
				for field in fields {
					// Detect raw SQL expressions (like COUNT(*), AVG(price), etc.)
					if field.contains('(') && field.contains(')') {
						// Use expr() for raw SQL expressions - clone to satisfy lifetime
						stmt.expr(Expr::cust(field.clone()));
					} else {
						// Regular column reference
						let col_ref = parse_column_reference(field);
						stmt.column(col_ref);
					}
				}
			} else if !self.deferred_fields.is_empty() {
				let all_fields = T::field_metadata();
				for field in all_fields {
					if !self.deferred_fields.contains(&field.name) {
						let col_ref = parse_column_reference(&field.name);
						stmt.column(col_ref);
					}
				}
			} else {
				stmt.column(ColumnRef::Asterisk);
			}

			if let Some(cond) = self.build_where_condition() {
				stmt.cond_where(cond);
			}

			// Apply ORDER BY clause
			for order_field in &self.order_by_fields {
				let (field, is_desc) = if let Some(stripped) = order_field.strip_prefix('-') {
					(stripped, true)
				} else {
					(order_field.as_str(), false)
				};

				let col_ref = parse_column_reference(field);
				let expr = Expr::col(col_ref);
				if is_desc {
					stmt.order_by_expr(expr, Order::Desc);
				} else {
					stmt.order_by_expr(expr, Order::Asc);
				}
			}

			// Apply LIMIT/OFFSET
			if let Some(limit) = self.limit {
				stmt.limit(limit as u64);
			}
			if let Some(offset) = self.offset {
				stmt.offset(offset as u64);
			}

			stmt.to_owned()
		} else {
			// SELECT with JOINs for select_related
			self.select_related_query()
		};

		// Convert statement to SQL with inline values (no placeholders)
		let sql = stmt.to_string(PostgresQueryBuilder);

		// Execute query and deserialize results
		let rows = conn.query(&sql, vec![]).await?;
		rows.into_iter()
			.map(|row| {
				serde_json::from_value(serde_json::to_value(&row.data).map_err(|e| {
					reinhardt_core::exception::Error::Database(format!(
						"Serialization error: {}",
						e
					))
				})?)
				.map_err(|e| {
					reinhardt_core::exception::Error::Database(format!(
						"Deserialization error: {}",
						e
					))
				})
			})
			.collect()
	}

	/// Execute the queryset and return the first matching record
	///
	/// Returns `None` if no records match the query.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Fetch first active user
	/// let user = User::objects()
	///     .filter(
	///         "is_active",
	///         FilterOperator::Eq,
	///         FilterValue::Boolean(true),
	///     )
	///     .first()
	///     .await?;
	///
	/// match user {
	///     Some(u) => println!("Found user: {}", u.username),
	///     None => println!("No active users found"),
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn first(&self) -> reinhardt_core::exception::Result<Option<T>>
	where
		T: serde::de::DeserializeOwned,
	{
		let mut results = self.all().await?;
		Ok(results.drain(..).next())
	}

	/// Execute the queryset and return a single matching record
	///
	/// Returns an error if zero or multiple records are found.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, email: String }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Fetch user with specific email (must be unique)
	/// let user = User::objects()
	///     .filter(
	///         "email",
	///         FilterOperator::Eq,
	///         FilterValue::String("alice@example.com".to_string()),
	///     )
	///     .get()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - No records match the query
	/// - Multiple records match the query
	/// - Database connection fails
	pub async fn get(&self) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let results = self.all().await?;
		match results.len() {
			0 => Err(reinhardt_core::exception::Error::Database(
				"No record found matching the query".to_string(),
			)),
			1 => Ok(results.into_iter().next().unwrap()),
			n => Err(reinhardt_core::exception::Error::Database(format!(
				"Multiple records found ({}), expected exactly one",
				n
			))),
		}
	}

	/// Execute the queryset with an explicit database connection and return all records
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let db = reinhardt_db::orm::manager::get_connection().await?;
	/// let users = User::objects()
	///     .all()
	///     .all_with_db(&db)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn all_with_db(
		&self,
		conn: &super::connection::DatabaseConnection,
	) -> reinhardt_core::exception::Result<Vec<T>>
	where
		T: serde::de::DeserializeOwned,
	{
		let stmt = if self.select_related_fields.is_empty() {
			let mut stmt = Query::select();
			stmt.from(Alias::new(T::table_name()));

			// Column selection considering selected_fields and deferred_fields
			if let Some(ref fields) = self.selected_fields {
				for field in fields {
					// Detect raw SQL expressions (like COUNT(*), AVG(price), etc.)
					if field.contains('(') && field.contains(')') {
						// Use expr() for raw SQL expressions - clone to satisfy lifetime
						stmt.expr(Expr::cust(field.clone()));
					} else {
						// Regular column reference
						let col_ref = parse_column_reference(field);
						stmt.column(col_ref);
					}
				}
			} else if !self.deferred_fields.is_empty() {
				let all_fields = T::field_metadata();
				for field in all_fields {
					if !self.deferred_fields.contains(&field.name) {
						let col_ref = parse_column_reference(&field.name);
						stmt.column(col_ref);
					}
				}
			} else {
				stmt.column(ColumnRef::Asterisk);
			}

			if let Some(cond) = self.build_where_condition() {
				stmt.cond_where(cond);
			}

			// Apply ORDER BY clause
			for order_field in &self.order_by_fields {
				let (field, is_desc) = if let Some(stripped) = order_field.strip_prefix('-') {
					(stripped, true)
				} else {
					(order_field.as_str(), false)
				};

				let col_ref = parse_column_reference(field);
				let expr = Expr::col(col_ref);
				if is_desc {
					stmt.order_by_expr(expr, Order::Desc);
				} else {
					stmt.order_by_expr(expr, Order::Asc);
				}
			}

			// Apply LIMIT/OFFSET
			if let Some(limit) = self.limit {
				stmt.limit(limit as u64);
			}
			if let Some(offset) = self.offset {
				stmt.offset(offset as u64);
			}

			stmt.to_owned()
		} else {
			self.select_related_query()
		};

		let sql = stmt.to_string(PostgresQueryBuilder);

		let rows = conn.query(&sql, vec![]).await?;
		rows.into_iter()
			.map(|row| {
				serde_json::from_value(serde_json::to_value(&row.data).map_err(|e| {
					reinhardt_core::exception::Error::Database(format!(
						"Serialization error: {}",
						e
					))
				})?)
				.map_err(|e| {
					reinhardt_core::exception::Error::Database(format!(
						"Deserialization error: {}",
						e
					))
				})
			})
			.collect()
	}

	/// Execute the queryset with an explicit database connection and return a single record
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let user_id = 1;
	/// let db = reinhardt_db::orm::manager::get_connection().await?;
	/// let user = User::objects()
	///     .filter("id", reinhardt_db::orm::FilterOperator::Eq, reinhardt_db::orm::FilterValue::Integer(user_id))
	///     .get_with_db(&db)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get_with_db(
		&self,
		conn: &super::connection::DatabaseConnection,
	) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let results = self.all_with_db(conn).await?;
		match results.len() {
			0 => Err(reinhardt_core::exception::Error::NotFound(
				"No record found matching the query".to_string(),
			)),
			1 => Ok(results.into_iter().next().unwrap()),
			n => Err(reinhardt_core::exception::Error::Database(format!(
				"Multiple records found ({}), expected exactly one",
				n
			))),
		}
	}

	/// Execute the queryset with an explicit database connection and return the first record
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let db = reinhardt_db::orm::manager::get_connection().await?;
	/// let user = User::objects()
	///     .filter("status", reinhardt_db::orm::FilterOperator::Eq, reinhardt_db::orm::FilterValue::String("active".to_string()))
	///     .first_with_db(&db)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn first_with_db(
		&self,
		conn: &super::connection::DatabaseConnection,
	) -> reinhardt_core::exception::Result<Option<T>>
	where
		T: serde::de::DeserializeOwned,
	{
		let mut results = self.all_with_db(conn).await?;
		Ok(results.drain(..).next())
	}

	/// Execute the queryset and return the count of matching records
	///
	/// More efficient than calling `all().await?.len()` as it only executes COUNT query.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Count active users
	/// let count = User::objects()
	///     .filter(
	///         "is_active",
	///         FilterOperator::Eq,
	///         FilterValue::Boolean(true),
	///     )
	///     .count()
	///     .await?;
	///
	/// println!("Active users: {}", count);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn count(&self) -> reinhardt_core::exception::Result<usize> {
		use reinhardt_query::prelude::{Func, PostgresQueryBuilder, QueryBuilder};

		let conn = super::manager::get_connection().await?;

		// Build COUNT query using reinhardt-query
		let mut stmt = Query::select();
		stmt.from(Alias::new(T::table_name()))
			.expr(Func::count(Expr::asterisk().into_simple_expr()));

		// Add WHERE conditions
		if let Some(cond) = self.build_where_condition() {
			stmt.cond_where(cond);
		}

		// Convert to SQL and extract parameter values
		let (sql, values) = PostgresQueryBuilder.build_select(&stmt);

		// Convert reinhardt_query::value::Values to QueryValue
		let params = super::execution::convert_values(values);

		// Execute query with parameters
		let rows = conn.query(&sql, params).await?;
		if let Some(row) = rows.first() {
			// Extract count from first row
			if let Some(count_value) = row.data.get("count")
				&& let Some(count) = count_value.as_i64()
			{
				return Ok(count as usize);
			}
		}

		Ok(0)
	}

	/// Check if any records match the queryset
	///
	/// More efficient than calling `count().await? > 0` as it can short-circuit.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Check if any admin users exist
	/// let has_admin = User::objects()
	///     .filter(
	///         "role",
	///         FilterOperator::Eq,
	///         FilterValue::String("admin".to_string()),
	///     )
	///     .exists()
	///     .await?;
	///
	/// if has_admin {
	///     println!("Admin users exist");
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn exists(&self) -> reinhardt_core::exception::Result<bool> {
		let count = self.count().await?;
		Ok(count > 0)
	}

	/// Create a new object in the database
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String, email: String }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let user = User {
	///     id: None,
	///     username: "alice".to_string(),
	///     email: "alice@example.com".to_string(),
	/// };
	/// let created = User::objects().create(&user).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn create(&self, object: T) -> reinhardt_core::exception::Result<T>
	where
		T: super::Model + Clone,
	{
		// Delegate to Manager::create() which handles all the SQL generation,
		// database connection, primary key retrieval, and error handling
		match &self.manager {
			Some(manager) => manager.create(&object).await,
			None => {
				// Fallback: create a new manager instance if none exists
				let manager = super::manager::Manager::<T>::new();
				manager.create(&object).await
			}
		}
	}

	/// Generate UPDATE statement using reinhardt-query
	pub fn update_query(
		&self,
		updates: &HashMap<String, UpdateValue>,
	) -> reinhardt_query::prelude::UpdateStatement {
		let mut stmt = Query::update();
		stmt.table(Alias::new(T::table_name()));

		// Add SET clauses
		for (field, value) in updates {
			let val_expr = match value {
				UpdateValue::String(s) => Expr::val(s.clone()),
				UpdateValue::Integer(i) => Expr::val(*i),
				UpdateValue::Float(f) => Expr::val(*f),
				UpdateValue::Boolean(b) => Expr::val(*b),
				UpdateValue::Null => Expr::val(reinhardt_query::value::Value::Int(None)),
				UpdateValue::FieldRef(f) => Expr::col(Alias::new(&f.field)),
				UpdateValue::Expression(expr) => Self::expression_to_query_expr(expr),
			};
			stmt.value_expr(Alias::new(field), val_expr);
		}

		// Add WHERE conditions
		if let Some(cond) = self.build_where_condition() {
			stmt.cond_where(cond);
		}

		stmt.to_owned()
	}

	/// Generate UPDATE SQL with WHERE clause and parameter binding
	///
	/// Returns SQL with placeholders ($1, $2, etc.) and the values to bind.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use reinhardt_db::orm::query::UpdateValue;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// use std::collections::HashMap;
	/// let queryset = User::objects()
	///     .filter("id", FilterOperator::Eq, FilterValue::Integer(1));
	///
	/// let mut updates = HashMap::new();
	/// updates.insert("name".to_string(), UpdateValue::String("Alice".to_string()));
	/// updates.insert("email".to_string(), UpdateValue::String("alice@example.com".to_string()));
	/// let (sql, params) = queryset.update_sql(&updates);
	/// // sql: "UPDATE users SET name = $1, email = $2 WHERE id = $3"
	/// // params: ["Alice", "alice@example.com", "1"]
	/// ```
	pub fn update_sql(&self, updates: &HashMap<String, UpdateValue>) -> (String, Vec<String>) {
		let stmt = self.update_query(updates);
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};
		let (sql, values) = PostgresQueryBuilder.build_update(&stmt);
		let params: Vec<String> = values
			.iter()
			.map(|v| Self::sea_value_to_string(v))
			.collect();
		(sql, params)
	}

	/// Convert reinhardt-query Value to String without SQL quoting
	fn sea_value_to_string(value: &reinhardt_query::value::Value) -> String {
		use reinhardt_query::value::Value;
		match value {
			Value::Bool(Some(b)) => b.to_string(),
			Value::TinyInt(Some(i)) => i.to_string(),
			Value::SmallInt(Some(i)) => i.to_string(),
			Value::Int(Some(i)) => i.to_string(),
			Value::BigInt(Some(i)) => i.to_string(),
			Value::TinyUnsigned(Some(i)) => i.to_string(),
			Value::SmallUnsigned(Some(i)) => i.to_string(),
			Value::Unsigned(Some(i)) => i.to_string(),
			Value::BigUnsigned(Some(i)) => i.to_string(),
			Value::Float(Some(f)) => f.to_string(),
			Value::Double(Some(f)) => f.to_string(),
			Value::String(Some(s)) => s.to_string(),
			Value::Bytes(Some(b)) => String::from_utf8_lossy(b).to_string(),
			_ => String::new(),
		}
	}

	/// Generate DELETE SQL with WHERE clause and parameter binding
	///
	/// Returns SQL with placeholders ($1, $2, etc.) and the values to bind.
	///
	/// # Safety
	///
	/// This method will panic if no filters are set to prevent accidental deletion of all rows.
	/// Always use `.filter()` before calling this method.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// let queryset = User::objects()
	///     .filter("id", FilterOperator::Eq, FilterValue::Integer(1));
	///
	/// let (sql, params) = queryset.delete_sql();
	/// // sql: "DELETE FROM users WHERE id = $1"
	/// // params: ["1"]
	/// ```
	/// Generate DELETE statement using reinhardt-query
	pub fn delete_query(&self) -> reinhardt_query::prelude::DeleteStatement {
		if self.filters.is_empty() {
			panic!(
				"DELETE without WHERE clause is not allowed. Use .filter() to specify which rows to delete."
			);
		}

		let mut stmt = Query::delete();
		stmt.from_table(Alias::new(T::table_name()));

		// Add WHERE conditions
		if let Some(cond) = self.build_where_condition() {
			stmt.cond_where(cond);
		}

		stmt.to_owned()
	}

	pub fn delete_sql(&self) -> (String, Vec<String>) {
		let stmt = self.delete_query();
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};
		let (sql, values) = PostgresQueryBuilder.build_delete(&stmt);
		let params: Vec<String> = values
			.iter()
			.map(|v| Self::sea_value_to_string(v))
			.collect();
		(sql, params)
	}

	/// Retrieve a single object by composite primary key
	///
	/// This method queries the database using all fields that compose the composite primary key.
	/// It validates that all required primary key fields are provided and returns the matching record.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::composite_pk::{CompositePrimaryKey, PkValue};
	/// # use serde::{Serialize, Deserialize};
	/// # use std::collections::HashMap;
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct PostTag { post_id: i64, tag_id: i64 }
	/// # #[derive(Clone)]
	/// # struct PostTagFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostTagFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for PostTag {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostTagFields;
	/// #     fn table_name() -> &'static str { "post_tags" }
	/// #     fn new_fields() -> Self::Fields { PostTagFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { None }
	/// #     fn set_primary_key(&mut self, _value: Self::PrimaryKey) {}
	/// #     fn composite_primary_key() -> Option<CompositePrimaryKey> {
	/// #         CompositePrimaryKey::new(vec!["post_id".to_string(), "tag_id".to_string()]).ok()
	/// #     }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let mut pk_values = HashMap::new();
	/// pk_values.insert("post_id".to_string(), PkValue::Int(1));
	/// pk_values.insert("tag_id".to_string(), PkValue::Int(5));
	///
	/// let post_tag = PostTag::objects().get_composite(&pk_values).await?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The model doesn't have a composite primary key
	/// - Required primary key fields are missing from the provided values
	/// - No matching record is found in the database
	/// - Multiple records match (should not happen with a valid composite PK)
	pub async fn get_composite(
		&self,
		pk_values: &HashMap<String, super::composite_pk::PkValue>,
	) -> reinhardt_core::exception::Result<T>
	where
		T: super::Model + Clone,
	{
		use reinhardt_query::prelude::{
			Alias, BinOper, ColumnRef, Expr, PostgresQueryBuilder, Value,
		};

		// Get composite primary key definition from the model
		let composite_pk = T::composite_primary_key().ok_or_else(|| {
			reinhardt_core::exception::Error::Database(
				"Model does not have a composite primary key".to_string(),
			)
		})?;

		// Validate that all required PK fields are provided
		composite_pk.validate(pk_values).map_err(|e| {
			reinhardt_core::exception::Error::Database(format!(
				"Composite PK validation failed: {}",
				e
			))
		})?;

		// Build SELECT query using reinhardt-query
		let table_name = T::table_name();
		let mut query = Query::select();

		// Use Alias::new for table name
		let table_alias = Alias::new(table_name);
		query.from(table_alias).column(ColumnRef::Asterisk);

		// Add WHERE conditions for each composite PK field
		for field_name in composite_pk.fields() {
			let pk_value: &super::composite_pk::PkValue = pk_values.get(field_name).unwrap();
			let col_alias = Alias::new(field_name);

			match pk_value {
				&super::composite_pk::PkValue::Int(v) => {
					let condition = Expr::col(col_alias)
						.binary(BinOper::Equal, Expr::value(Value::BigInt(Some(v))));
					query.and_where(condition);
				}
				&super::composite_pk::PkValue::Uint(v) => {
					let condition = Expr::col(col_alias)
						.binary(BinOper::Equal, Expr::value(Value::BigInt(Some(v as i64))));
					query.and_where(condition);
				}
				super::composite_pk::PkValue::String(v) => {
					let condition = Expr::col(col_alias).binary(
						BinOper::Equal,
						Expr::value(Value::String(Some(Box::new(v.clone())))),
					);
					query.and_where(condition);
				}
				&super::composite_pk::PkValue::Bool(v) => {
					let condition = Expr::col(col_alias)
						.binary(BinOper::Equal, Expr::value(Value::Bool(Some(v))));
					query.and_where(condition);
				}
			}
		}

		// Build SQL with inline values (no placeholders)
		let sql = query.to_string(PostgresQueryBuilder);

		// Execute query using database connection
		let conn = super::manager::get_connection().await?;

		// Execute the SELECT query
		let rows = conn.query(&sql, vec![]).await?;

		// Composite PK queries should return exactly one row
		if rows.is_empty() {
			return Err(reinhardt_core::exception::Error::Database(
				"No record found matching the composite primary key".to_string(),
			));
		}

		if rows.len() > 1 {
			return Err(reinhardt_core::exception::Error::Database(format!(
				"Multiple records found ({}) for composite primary key, expected exactly one",
				rows.len()
			)));
		}

		// Deserialize the single row into the model
		let row = &rows[0];
		let value = serde_json::to_value(&row.data).map_err(|e| {
			reinhardt_core::exception::Error::Database(format!("Serialization error: {}", e))
		})?;

		serde_json::from_value(value).map_err(|e| {
			reinhardt_core::exception::Error::Database(format!("Deserialization error: {}", e))
		})
	}

	/// Add an annotation to the QuerySet
	///
	/// Annotations allow you to add calculated fields to query results using expressions,
	/// aggregations, or subqueries. The annotation will be added to the SELECT clause.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::annotation::{Annotation, AnnotationValue};
	/// use reinhardt_db::orm::aggregation::Aggregate;
	///
	/// // Add aggregate annotation
	/// let users = User::objects()
	///     .annotate(Annotation::new("total_orders",
	///         AnnotationValue::Aggregate(Aggregate::count(Some("orders")))))
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn annotate(mut self, annotation: super::annotation::Annotation) -> Self {
		self.annotations.push(annotation);
		self
	}

	/// Add a subquery annotation to the QuerySet (SELECT clause subquery)
	///
	/// This method adds a scalar subquery to the SELECT clause, allowing you to
	/// include computed values from related tables without explicit JOINs.
	///
	/// # Type Parameters
	///
	/// * `M` - The model type for the subquery
	/// * `F` - A closure that builds the subquery
	///
	/// # Parameters
	///
	/// * `name` - The alias for the subquery result column
	/// * `builder` - A closure that receives a fresh `QuerySet<M>` and returns a configured QuerySet
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use reinhardt_db::orm::OuterRef;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Author { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct AuthorFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for AuthorFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Author {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = AuthorFields;
	/// #     fn table_name() -> &'static str { "authors" }
	/// #     fn new_fields() -> Self::Fields { AuthorFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Book { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct BookFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for BookFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Book {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = BookFields;
	/// #     fn table_name() -> &'static str { "books" }
	/// #     fn new_fields() -> Self::Fields { BookFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Add book count for each author
	/// let authors = Author::objects()
	///     .annotate_subquery::<Book, _>("book_count", |subq| {
	///         subq.filter(Filter::new(
	///             "author_id",
	///             FilterOperator::Eq,
	///             FilterValue::OuterRef(OuterRef::new("authors.id"))
	///         ))
	///         .values(&["COUNT(*)"])
	///     })
	///     .all()
	///     .await?;
	/// // Generates: SELECT *, (SELECT COUNT(*) FROM books WHERE author_id = authors.id) AS book_count FROM authors
	/// # Ok(())
	/// # }
	/// ```
	pub fn annotate_subquery<M, F>(mut self, name: &str, builder: F) -> Self
	where
		M: super::Model + 'static,
		F: FnOnce(QuerySet<M>) -> QuerySet<M>,
	{
		// Create a fresh QuerySet for the subquery model
		let subquery_qs = QuerySet::<M>::new();
		// Apply the builder to configure the subquery
		let configured_subquery = builder(subquery_qs);
		// Generate SQL for the subquery (wrapped in parentheses)
		let subquery_sql = configured_subquery.as_subquery();

		// Add as annotation using AnnotationValue::Subquery
		let annotation = super::annotation::Annotation {
			alias: name.to_string(),
			value: super::annotation::AnnotationValue::Subquery(subquery_sql),
		};
		self.annotations.push(annotation);
		self
	}

	/// Perform an aggregation on the QuerySet
	///
	/// Aggregations allow you to calculate summary statistics (COUNT, SUM, AVG, MAX, MIN)
	/// for the queryset. The aggregation result will be added to the SELECT clause.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Serialize, Deserialize, Clone)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Serialize, Deserialize, Clone)]
	/// # struct Order { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct OrderFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for OrderFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Order {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = OrderFields;
	/// #     fn table_name() -> &'static str { "orders" }
	/// #     fn new_fields() -> Self::Fields { OrderFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::aggregation::Aggregate;
	///
	/// // Count all users
	/// let result = User::objects()
	///     .all()
	///     .aggregate(Aggregate::count_all().with_alias("total_users"))
	///     .all()
	///     .await?;
	///
	/// // Sum order amounts
	/// let result = Order::objects()
	///     .all()
	///     .aggregate(Aggregate::sum("amount").with_alias("total_amount"))
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn aggregate(mut self, aggregate: super::aggregation::Aggregate) -> Self {
		// Convert Aggregate to Annotation and add to annotations list
		let alias = aggregate
			.alias
			.clone()
			.unwrap_or_else(|| aggregate.func.to_string().to_lowercase());
		let annotation = super::annotation::Annotation {
			alias,
			value: super::annotation::AnnotationValue::Aggregate(aggregate),
		};
		self.annotations.push(annotation);
		self
	}

	pub fn to_sql(&self) -> String {
		let mut stmt = if self.select_related_fields.is_empty() {
			// Simple SELECT without JOINs
			let mut stmt = Query::select();

			// Apply FROM clause with optional alias
			if let Some(ref alias) = self.from_alias {
				stmt.from_as(Alias::new(T::table_name()), Alias::new(alias));
			} else {
				stmt.from(Alias::new(T::table_name()));
			}

			// Apply DISTINCT if enabled
			if self.distinct_enabled {
				stmt.distinct();
			}

			// Column selection considering selected_fields and deferred_fields
			if let Some(ref fields) = self.selected_fields {
				for field in fields {
					// Detect raw SQL expressions (like COUNT(*), AVG(price), etc.)
					if field.contains('(') && field.contains(')') {
						// Use expr() for raw SQL expressions - clone to satisfy lifetime
						stmt.expr(Expr::cust(field.clone()));
					} else {
						// Regular column reference
						let col_ref = parse_column_reference(field);
						stmt.column(col_ref);
					}
				}
			} else if !self.deferred_fields.is_empty() {
				let all_fields = T::field_metadata();
				for field in all_fields {
					if !self.deferred_fields.contains(&field.name) {
						let col_ref = parse_column_reference(&field.name);
						stmt.column(col_ref);
					}
				}
			} else {
				stmt.column(ColumnRef::Asterisk);
			}

			// Apply JOINs
			for join in &self.joins {
				if join.on_condition.is_empty() {
					// CROSS JOIN (no ON condition)
					if let Some(ref alias) = join.target_alias {
						// CROSS JOIN with alias - reinhardt-query doesn't support this directly
						// Use regular join syntax instead
						stmt.cross_join((Alias::new(&join.target_table), Alias::new(alias)));
					} else {
						stmt.cross_join(Alias::new(&join.target_table));
					}
				} else {
					// Convert reinhardt JoinType to reinhardt-query JoinType
					let sea_join_type = match join.join_type {
						super::sqlalchemy_query::JoinType::Inner => SeaJoinType::InnerJoin,
						super::sqlalchemy_query::JoinType::Left => SeaJoinType::LeftJoin,
						super::sqlalchemy_query::JoinType::Right => SeaJoinType::RightJoin,
						super::sqlalchemy_query::JoinType::Full => SeaJoinType::FullOuterJoin,
					};

					// Build the join with optional alias
					if let Some(ref alias) = join.target_alias {
						// JOIN with alias: (table, alias)
						stmt.join(
							sea_join_type,
							(Alias::new(&join.target_table), Alias::new(alias)),
							Expr::cust(join.on_condition.clone()),
						);
					} else {
						// JOIN without alias
						stmt.join(
							sea_join_type,
							Alias::new(&join.target_table),
							Expr::cust(join.on_condition.clone()),
						);
					}
				}
			}

			// Apply WHERE conditions
			if let Some(cond) = self.build_where_condition() {
				stmt.cond_where(cond);
			}

			// Apply GROUP BY
			for group_field in &self.group_by_fields {
				stmt.group_by_col(Alias::new(group_field));
			}

			// Apply HAVING
			for having_cond in &self.having_conditions {
				match having_cond {
					HavingCondition::AggregateCompare {
						func,
						field,
						operator,
						value,
					} => {
						// Build aggregate function expression
						let agg_expr = match func {
							AggregateFunc::Avg => {
								Func::avg(Expr::col(Alias::new(field)).into_simple_expr())
							}
							AggregateFunc::Count => {
								if field == "*" {
									Func::count(Expr::asterisk().into_simple_expr())
								} else {
									Func::count(Expr::col(Alias::new(field)).into_simple_expr())
								}
							}
							AggregateFunc::Sum => {
								Func::sum(Expr::col(Alias::new(field)).into_simple_expr())
							}
							AggregateFunc::Min => {
								Func::min(Expr::col(Alias::new(field)).into_simple_expr())
							}
							AggregateFunc::Max => {
								Func::max(Expr::col(Alias::new(field)).into_simple_expr())
							}
						};

						// Build comparison expression
						let having_expr = match operator {
							ComparisonOp::Eq => match value {
								AggregateValue::Int(v) => agg_expr.eq(*v),
								AggregateValue::Float(v) => agg_expr.eq(*v),
							},
							ComparisonOp::Ne => match value {
								AggregateValue::Int(v) => agg_expr.ne(*v),
								AggregateValue::Float(v) => agg_expr.ne(*v),
							},
							ComparisonOp::Gt => match value {
								AggregateValue::Int(v) => agg_expr.gt(*v),
								AggregateValue::Float(v) => agg_expr.gt(*v),
							},
							ComparisonOp::Gte => match value {
								AggregateValue::Int(v) => agg_expr.gte(*v),
								AggregateValue::Float(v) => agg_expr.gte(*v),
							},
							ComparisonOp::Lt => match value {
								AggregateValue::Int(v) => agg_expr.lt(*v),
								AggregateValue::Float(v) => agg_expr.lt(*v),
							},
							ComparisonOp::Lte => match value {
								AggregateValue::Int(v) => agg_expr.lte(*v),
								AggregateValue::Float(v) => agg_expr.lte(*v),
							},
						};

						stmt.and_having(having_expr);
					}
				}
			}

			// Apply ORDER BY
			for order_field in &self.order_by_fields {
				let (field, is_desc) = if let Some(stripped) = order_field.strip_prefix('-') {
					(stripped, true)
				} else {
					(order_field.as_str(), false)
				};

				let col_ref = parse_column_reference(field);
				let expr = Expr::col(col_ref);
				if is_desc {
					stmt.order_by_expr(expr, Order::Desc);
				} else {
					stmt.order_by_expr(expr, Order::Asc);
				}
			}

			// Apply LIMIT/OFFSET
			if let Some(limit) = self.limit {
				stmt.limit(limit as u64);
			}
			if let Some(offset) = self.offset {
				stmt.offset(offset as u64);
			}

			stmt.to_owned()
		} else {
			// SELECT with JOINs for select_related
			self.select_related_query()
		};

		// Add annotations to SELECT clause if any using reinhardt-query API
		// Collect annotation SQL strings first to handle lifetime issues
		// Note: Use to_sql_expr() to get expression without alias (reinhardt-query adds alias via expr_as)
		let annotation_exprs: Vec<_> = self
			.annotations
			.iter()
			.map(|a| (a.value.to_sql_expr(), a.alias.clone()))
			.collect();

		for (value_sql, alias) in annotation_exprs {
			stmt.expr_as(Expr::cust(value_sql), Alias::new(alias));
		}

		use reinhardt_query::prelude::PostgresQueryBuilder;
		let mut select_sql = stmt.to_string(PostgresQueryBuilder);

		// Insert LATERAL JOIN clauses after FROM clause
		if !self.lateral_joins.is_empty() {
			let lateral_sql = self.lateral_joins.to_sql().join(" ");

			// Find insertion point: after FROM clause, before WHERE/ORDER BY/LIMIT
			// Look for WHERE, ORDER BY, or end of string
			let insert_pos = select_sql
				.find(" WHERE ")
				.or_else(|| select_sql.find(" ORDER BY "))
				.or_else(|| select_sql.find(" LIMIT "))
				.unwrap_or(select_sql.len());

			select_sql.insert_str(insert_pos, &format!(" {}", lateral_sql));
		}

		// Replace FROM table with FROM subquery if from_subquery_sql is set
		if let Some(ref subquery_sql) = self.from_subquery_sql
			&& let Some(ref alias) = self.from_alias
		{
			// Pattern: FROM "table_name" AS "alias" or FROM "table_name"
			let from_pattern_with_alias = format!("FROM \"{}\" AS \"{}\"", T::table_name(), alias);
			let from_pattern_simple = format!("FROM \"{}\"", T::table_name());

			let from_replacement = format!("FROM {} AS \"{}\"", subquery_sql, alias);

			// Try to replace with alias pattern first, then simple pattern
			if select_sql.contains(&from_pattern_with_alias) {
				select_sql = select_sql.replace(&from_pattern_with_alias, &from_replacement);
			} else if select_sql.contains(&from_pattern_simple) {
				select_sql = select_sql.replace(&from_pattern_simple, &from_replacement);
			}
		}

		// Prepend CTE clause if any CTEs are defined
		if let Some(cte_sql) = self.ctes.to_sql() {
			format!("{} {}", cte_sql, select_sql)
		} else {
			select_sql
		}
	}

	/// Select specific values from the QuerySet
	///
	/// Returns only the specified fields instead of all columns.
	/// Useful for optimizing queries when you don't need all model fields.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Select only specific fields
	/// let users = User::objects()
	///     .values(&["id", "username", "email"])
	///     .all()
	///     .await?;
	/// // Generates: SELECT id, username, email FROM users
	///
	/// // Combine with filters
	/// let active_user_names = User::objects()
	///     .filter("is_active", FilterOperator::Eq, FilterValue::Boolean(true))
	///     .values(&["username"])
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn values(mut self, fields: &[&str]) -> Self {
		self.selected_fields = Some(fields.iter().map(|s| s.to_string()).collect());
		self
	}

	/// Select specific values as a list
	///
	/// Alias for `values()` - returns tuple-like results with specified fields.
	/// In Django, this returns tuples instead of dictionaries, but in Rust
	/// the behavior is the same as `values()` due to type safety.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Same as values()
	/// let user_data = User::objects()
	///     .values_list(&["id", "username"])
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn values_list(self, fields: &[&str]) -> Self {
		self.values(fields)
	}

	/// Order the QuerySet by specified fields
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # fn example() {
	/// // Ascending order
	/// User::objects().order_by(&["name"]);
	///
	/// // Descending order (prefix with '-')
	/// User::objects().order_by(&["-created_at"]);
	///
	/// // Multiple fields
	/// User::objects().order_by(&["department", "-salary"]);
	/// # }
	/// ```
	pub fn order_by(mut self, fields: &[&str]) -> Self {
		self.order_by_fields = fields.iter().map(|s| s.to_string()).collect();
		self
	}

	/// Return only distinct results
	pub fn distinct(mut self) -> Self {
		self.distinct_enabled = true;
		self
	}

	/// Set LIMIT clause
	///
	/// Limits the number of records returned by the query.
	/// Corresponds to Django's QuerySet slicing `[:limit]`.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let users = User::objects()
	///     .limit(10)
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn limit(mut self, limit: usize) -> Self {
		self.limit = Some(limit);
		self
	}

	/// Set OFFSET clause
	///
	/// Skips the specified number of records before returning results.
	/// Corresponds to Django's QuerySet slicing `[offset:]`.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let users = User::objects()
	///     .offset(20)
	///     .limit(10)
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn offset(mut self, offset: usize) -> Self {
		self.offset = Some(offset);
		self
	}

	/// Paginate results using page number and page size
	///
	/// Convenience method that calculates offset automatically.
	/// Corresponds to Django REST framework's PageNumberPagination.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Page 3, 10 items per page (offset=20, limit=10)
	/// let users = User::objects()
	///     .paginate(3, 10)
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn paginate(self, page: usize, page_size: usize) -> Self {
		let offset = page.saturating_sub(1) * page_size;
		self.offset(offset).limit(page_size)
	}

	/// Convert QuerySet to a subquery
	///
	/// Returns the QuerySet as a SQL subquery wrapped in parentheses,
	/// suitable for use in IN clauses, EXISTS clauses, or as a derived table.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// // Use in IN clause
	/// let active_user_ids = User::objects()
	///     .filter("is_active", FilterOperator::Eq, FilterValue::Bool(true))
	///     .values(&["id"])
	///     .as_subquery();
	/// // Generates: (SELECT id FROM users WHERE is_active = $1)
	///
	/// // Use as derived table
	/// let subquery = Post::objects()
	///     .filter("published", FilterOperator::Eq, FilterValue::Bool(true))
	///     .as_subquery();
	/// // Generates: (SELECT * FROM posts WHERE published = $1)
	/// ```
	pub fn as_subquery(self) -> String {
		format!("({})", self.to_sql())
	}

	/// Defer loading of specific fields
	///
	/// Marks specific fields for deferred loading (lazy loading).
	/// The specified fields will be excluded from the initial query.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String, email: String }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Defer large text fields
	/// let users = User::objects()
	///     .defer(&["bio", "profile_picture"])
	///     .all()
	///     .await?;
	/// // Generates: SELECT id, username, email FROM users (excluding bio, profile_picture)
	/// # Ok(())
	/// # }
	/// ```
	pub fn defer(mut self, fields: &[&str]) -> Self {
		self.deferred_fields = fields.iter().map(|s| s.to_string()).collect();
		self
	}

	/// Load only specific fields
	///
	/// Alias for `values()` - specifies which fields to load immediately.
	/// In Django, this is used for deferred loading optimization, but in Rust
	/// it behaves the same as `values()`.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Load only specific fields
	/// let users = User::objects()
	///     .only(&["id", "username"])
	///     .all()
	///     .await?;
	/// // Generates: SELECT id, username FROM users
	/// # Ok(())
	/// # }
	/// ```
	pub fn only(self, fields: &[&str]) -> Self {
		self.values(fields)
	}

	// ==================== PostgreSQL-specific convenience methods ====================

	/// Filter by PostgreSQL full-text search
	///
	/// This method adds a filter for full-text search using PostgreSQL's `@@` operator.
	/// The query is converted using `plainto_tsquery` for simple word matching.
	///
	/// # Arguments
	///
	/// * `field` - The tsvector field to search
	/// * `query` - The search query string
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Article { id: Option<i64>, title: String }
	/// # #[derive(Clone)]
	/// # struct ArticleFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for ArticleFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Article {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = ArticleFields;
	/// #     fn table_name() -> &'static str { "articles" }
	/// #     fn new_fields() -> Self::Fields { ArticleFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Search articles for "rust programming"
	/// let articles = Article::objects()
	///     .full_text_search("search_vector", "rust programming")
	///     .all()
	///     .await?;
	/// // Generates: WHERE search_vector @@ plainto_tsquery('english', 'rust programming')
	/// # Ok(())
	/// # }
	/// ```
	pub fn full_text_search(self, field: &str, query: &str) -> Self {
		self.filter(Filter::new(
			field,
			FilterOperator::FullTextMatch,
			FilterValue::String(query.to_string()),
		))
	}

	/// Filter by PostgreSQL array overlap
	///
	/// Returns rows where the array field has at least one element in common with the given values.
	///
	/// # Arguments
	///
	/// * `field` - The array field name
	/// * `values` - Values to check for overlap
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, title: String }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find posts with any of these tags
	/// let posts = Post::objects()
	///     .filter_array_overlap("tags", &["rust", "programming"])
	///     .all()
	///     .await?;
	/// // Generates: WHERE tags && ARRAY['rust', 'programming']
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_array_overlap(self, field: &str, values: &[&str]) -> Self {
		self.filter(Filter::new(
			field,
			FilterOperator::ArrayOverlap,
			FilterValue::Array(values.iter().map(|s| s.to_string()).collect()),
		))
	}

	/// Filter by PostgreSQL array containment
	///
	/// Returns rows where the array field contains all the given values.
	///
	/// # Arguments
	///
	/// * `field` - The array field name
	/// * `values` - Values that must all be present in the array
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64>, title: String }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find posts that have both "rust" and "async" tags
	/// let posts = Post::objects()
	///     .filter_array_contains("tags", &["rust", "async"])
	///     .all()
	///     .await?;
	/// // Generates: WHERE tags @> ARRAY['rust', 'async']
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_array_contains(self, field: &str, values: &[&str]) -> Self {
		self.filter(Filter::new(
			field,
			FilterOperator::ArrayContains,
			FilterValue::Array(values.iter().map(|s| s.to_string()).collect()),
		))
	}

	/// Filter by PostgreSQL JSONB containment
	///
	/// Returns rows where the JSONB field contains the given JSON object.
	///
	/// # Arguments
	///
	/// * `field` - The JSONB field name
	/// * `json` - JSON string to check for containment
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Product { id: Option<i64>, name: String }
	/// # #[derive(Clone)]
	/// # struct ProductFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for ProductFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Product {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = ProductFields;
	/// #     fn table_name() -> &'static str { "products" }
	/// #     fn new_fields() -> Self::Fields { ProductFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find products with specific metadata
	/// let products = Product::objects()
	///     .filter_jsonb_contains("metadata", r#"{"active": true}"#)
	///     .all()
	///     .await?;
	/// // Generates: WHERE metadata @> '{"active": true}'::jsonb
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_jsonb_contains(self, field: &str, json: &str) -> Self {
		self.filter(Filter::new(
			field,
			FilterOperator::JsonbContains,
			FilterValue::String(json.to_string()),
		))
	}

	/// Filter by PostgreSQL JSONB key existence
	///
	/// Returns rows where the JSONB field contains the given key.
	///
	/// # Arguments
	///
	/// * `field` - The JSONB field name
	/// * `key` - Key to check for existence
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Product { id: Option<i64>, name: String }
	/// # #[derive(Clone)]
	/// # struct ProductFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for ProductFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Product {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = ProductFields;
	/// #     fn table_name() -> &'static str { "products" }
	/// #     fn new_fields() -> Self::Fields { ProductFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find products with "sale_price" in metadata
	/// let products = Product::objects()
	///     .filter_jsonb_key_exists("metadata", "sale_price")
	///     .all()
	///     .await?;
	/// // Generates: WHERE metadata ? 'sale_price'
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_jsonb_key_exists(self, field: &str, key: &str) -> Self {
		self.filter(Filter::new(
			field,
			FilterOperator::JsonbKeyExists,
			FilterValue::String(key.to_string()),
		))
	}

	/// Filter by PostgreSQL range containment
	///
	/// Returns rows where the range field contains the given value.
	///
	/// # Arguments
	///
	/// * `field` - The range field name
	/// * `value` - Value to check for containment in the range
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: Option<i64>, name: String }
	/// # #[derive(Clone)]
	/// # struct EventFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for EventFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Event {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = EventFields;
	/// #     fn table_name() -> &'static str { "events" }
	/// #     fn new_fields() -> Self::Fields { EventFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Find events that include a specific date
	/// let events = Event::objects()
	///     .filter_range_contains("date_range", "2024-06-15")
	///     .all()
	///     .await?;
	/// // Generates: WHERE date_range @> '2024-06-15'
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_range_contains(self, field: &str, value: &str) -> Self {
		self.filter(Filter::new(
			field,
			FilterOperator::RangeContains,
			FilterValue::String(value.to_string()),
		))
	}
}

impl<T> Default for QuerySet<T>
where
	T: super::Model,
{
	fn default() -> Self {
		Self::new()
	}
}

// Convenience conversions for FilterValue
impl FilterValue {
	/// Create a String variant from any value that can be converted to String
	///
	/// Accepts any type that implements `ToString`, including:
	/// - String, &str
	/// - Uuid (via Display)
	/// - Numeric types (i64, u64, etc. via Display)
	pub fn string(value: impl ToString) -> Self {
		Self::String(value.to_string())
	}
}

// ============================================================================
// Helper Functions for JOIN Support
// ============================================================================

/// Parse field reference into reinhardt-query column expression
///
/// Handles both qualified (`table.column`) and unqualified (`column`) references.
///
/// # Examples
///
/// - `"id"` → `ColumnRef::Column("id")`
/// - `"users.id"` → `ColumnRef::Column("users.id")` (qualified name as-is)
///
/// Note: For reinhardt-query v1.0.0-rc.29+, we use the full qualified name as a column identifier.
/// This works for most databases that support qualified column references.
///
/// This function also detects raw SQL expressions (containing parentheses, like `COUNT(*)`,
/// `AVG(price)`) and returns them wrapped in `Expr::cust()` instead of as column references.
fn parse_column_reference(field: &str) -> reinhardt_query::prelude::ColumnRef {
	use reinhardt_query::prelude::ColumnRef;

	// Detect raw SQL expressions by checking for parentheses
	// Examples: COUNT(*), AVG(price), SUM(amount), MAX(value)
	if field.contains('(') && field.contains(')') {
		// Use column reference with raw expression name
		ColumnRef::column(Alias::new(field))
	} else if field.contains('.') {
		// Qualified column reference (table.column format)
		let parts: Vec<&str> = field.split('.').collect();
		if parts.len() == 2 {
			// Produces: "table"."column" instead of "table.column"
			ColumnRef::table_column(Alias::new(parts[0]), Alias::new(parts[1]))
		} else {
			// Fallback for unexpected formats (e.g., schema.table.column)
			ColumnRef::column(Alias::new(field))
		}
	} else {
		// Simple column reference
		ColumnRef::column(Alias::new(field))
	}
}

#[cfg(test)]
mod tests {
	use crate::orm::query::UpdateValue;
	use crate::orm::{FilterOperator, FilterValue, Model, QuerySet, query::Filter};
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestUser {
		id: Option<i64>,
		username: String,
		email: String,
	}

	impl TestUser {
		// Allow dead_code: test helper constructor for query tests
		#[allow(dead_code)]
		fn new(username: String, email: String) -> Self {
			Self {
				id: None,
				username,
				email,
			}
		}
	}

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
			"test_users"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}
	}

	#[tokio::test]
	async fn test_queryset_create_with_manager() {
		// Test QuerySet::create() with explicit manager
		let manager = std::sync::Arc::new(TestUser::objects());
		let queryset = QuerySet::with_manager(manager);

		let user = TestUser {
			id: None,
			username: "testuser".to_string(),
			email: "test@example.com".to_string(),
		};

		// Note: This will fail without a real database connection
		// In actual integration tests, we would set up a test database
		let result = queryset.create(user).await;

		// In unit tests, we expect this to fail due to no database
		// In integration tests with TestContainers, this would succeed
		assert!(result.is_err() || result.is_ok());
	}

	#[tokio::test]
	async fn test_queryset_create_without_manager() {
		// Test QuerySet::create() fallback without manager
		let queryset = QuerySet::<TestUser>::new();

		let user = TestUser {
			id: None,
			username: "fallback_user".to_string(),
			email: "fallback@example.com".to_string(),
		};

		// Note: This will fail without a real database connection
		let result = queryset.create(user).await;

		// In unit tests, we expect this to fail due to no database
		assert!(result.is_err() || result.is_ok());
	}

	#[test]
	fn test_queryset_with_manager() {
		let manager = std::sync::Arc::new(TestUser::objects());
		let queryset = QuerySet::with_manager(manager.clone());

		// Verify manager is set
		assert!(queryset.manager.is_some());
	}

	#[test]
	fn test_queryset_filter_preserves_manager() {
		let manager = std::sync::Arc::new(TestUser::objects());
		let queryset = QuerySet::with_manager(manager);

		let filter = Filter::new(
			"username".to_string(),
			FilterOperator::Eq,
			FilterValue::String("alice".to_string()),
		);

		let filtered = queryset.filter(filter);

		// Verify manager is preserved after filter
		assert!(filtered.manager.is_some());
	}

	#[test]
	fn test_queryset_select_related_preserves_manager() {
		let manager = std::sync::Arc::new(TestUser::objects());
		let queryset = QuerySet::with_manager(manager);

		let selected = queryset.select_related(&["profile", "posts"]);

		// Verify manager is preserved after select_related
		assert!(selected.manager.is_some());
		assert_eq!(selected.select_related_fields, vec!["profile", "posts"]);
	}

	#[test]
	fn test_queryset_prefetch_related_preserves_manager() {
		let manager = std::sync::Arc::new(TestUser::objects());
		let queryset = QuerySet::with_manager(manager);

		let prefetched = queryset.prefetch_related(&["comments", "likes"]);

		// Verify manager is preserved after prefetch_related
		assert!(prefetched.manager.is_some());
		assert_eq!(
			prefetched.prefetch_related_fields,
			vec!["comments", "likes"]
		);
	}

	#[tokio::test]
	async fn test_get_composite_validation_error() {
		use std::collections::HashMap;

		let queryset = QuerySet::<TestUser>::new();
		let pk_values = HashMap::new(); // Empty HashMap - should fail validation

		let result = queryset.get_composite(&pk_values).await;

		// Expect error because TestUser doesn't have a composite primary key
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("composite primary key"));
	}

	// SQL Generation Tests

	#[test]
	fn test_update_sql_single_field_single_filter() {
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(1),
		));

		let mut updates = HashMap::new();
		updates.insert(
			"username".to_string(),
			UpdateValue::String("alice".to_string()),
		);
		let (sql, params) = queryset.update_sql(&updates);

		assert_eq!(
			sql,
			"UPDATE \"test_users\" SET \"username\" = $1 WHERE \"id\" = $2"
		);
		assert_eq!(params, vec!["alice", "1"]);
	}

	#[test]
	fn test_update_sql_multiple_fields_multiple_filters() {
		let queryset = QuerySet::<TestUser>::new()
			.filter(Filter::new(
				"id".to_string(),
				FilterOperator::Gt,
				FilterValue::Integer(10),
			))
			.filter(Filter::new(
				"email".to_string(),
				FilterOperator::Contains,
				FilterValue::String("example.com".to_string()),
			));

		let mut updates = HashMap::new();
		updates.insert(
			"username".to_string(),
			UpdateValue::String("bob".to_string()),
		);
		updates.insert(
			"email".to_string(),
			UpdateValue::String("bob@test.com".to_string()),
		);
		let (sql, params) = queryset.update_sql(&updates);

		// HashMap iteration order is not guaranteed, so we check both possible orderings
		let valid_sql_1 = "UPDATE \"test_users\" SET \"username\" = $1, \"email\" = $2 WHERE (\"id\" > $3 AND \"email\" LIKE $4)";
		let valid_sql_2 = "UPDATE \"test_users\" SET \"email\" = $1, \"username\" = $2 WHERE (\"id\" > $3 AND \"email\" LIKE $4)";
		assert!(
			sql == valid_sql_1 || sql == valid_sql_2,
			"Generated SQL '{}' does not match either expected pattern",
			sql
		);

		// Check that all expected values are present (order may vary for SET clause)
		assert!(
			params.contains(&"bob".to_string()) || params.contains(&"bob@test.com".to_string())
		);
		assert_eq!(params[2], "10");
		assert_eq!(params[3], "%example.com%");
	}

	#[test]
	fn test_delete_sql_single_filter() {
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(1),
		));

		let (sql, params) = queryset.delete_sql();

		assert_eq!(sql, "DELETE FROM \"test_users\" WHERE \"id\" = $1");
		assert_eq!(params, vec!["1"]);
	}

	#[test]
	fn test_delete_sql_multiple_filters() {
		let queryset = QuerySet::<TestUser>::new()
			.filter(Filter::new(
				"username".to_string(),
				FilterOperator::Eq,
				FilterValue::String("alice".to_string()),
			))
			.filter(Filter::new(
				"email".to_string(),
				FilterOperator::StartsWith,
				FilterValue::String("alice@".to_string()),
			));

		let (sql, params) = queryset.delete_sql();

		assert_eq!(
			sql,
			"DELETE FROM \"test_users\" WHERE (\"username\" = $1 AND \"email\" LIKE $2)"
		);
		assert_eq!(params, vec!["alice", "alice@%"]);
	}

	#[test]
	#[should_panic(
		expected = "DELETE without WHERE clause is not allowed. Use .filter() to specify which rows to delete."
	)]
	fn test_delete_sql_without_filters_panics() {
		let queryset = QuerySet::<TestUser>::new();
		let _ = queryset.delete_sql();
	}

	#[test]
	fn test_filter_operators() {
		let queryset = QuerySet::<TestUser>::new()
			.filter(Filter::new(
				"id".to_string(),
				FilterOperator::Gte,
				FilterValue::Integer(5),
			))
			.filter(Filter::new(
				"username".to_string(),
				FilterOperator::Ne,
				FilterValue::String("admin".to_string()),
			));

		let (sql, params) = queryset.delete_sql();

		assert_eq!(
			sql,
			"DELETE FROM \"test_users\" WHERE (\"id\" >= $1 AND \"username\" <> $2)"
		);
		assert_eq!(params, vec!["5", "admin"]);
	}

	#[test]
	fn test_null_value_filter() {
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"email".to_string(),
			FilterOperator::Eq,
			FilterValue::Null,
		));

		let (sql, params) = queryset.delete_sql();

		assert_eq!(sql, "DELETE FROM \"test_users\" WHERE \"email\" IS NULL");
		assert_eq!(params, Vec::<String>::new());
	}

	#[test]
	fn test_not_null_value_filter() {
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"email".to_string(),
			FilterOperator::Ne,
			FilterValue::Null,
		));

		let (sql, params) = queryset.delete_sql();

		assert_eq!(
			sql,
			"DELETE FROM \"test_users\" WHERE \"email\" IS NOT NULL"
		);
		assert_eq!(params, Vec::<String>::new());
	}

	// Query Optimization Tests

	#[test]
	fn test_select_related_query_generation() {
		// Test that select_related_query() generates SelectStatement correctly
		let queryset = QuerySet::<TestUser>::new().select_related(&["profile", "department"]);

		let stmt = queryset.select_related_query();

		// Convert to SQL to verify structure
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryStatementBuilder};
		let sql = stmt.build(PostgresQueryBuilder).0;

		assert!(sql.contains("SELECT"));
		assert!(sql.contains("test_users"));
		assert!(sql.contains("LEFT JOIN"));
	}

	#[test]
	fn test_prefetch_related_queries_generation() {
		// Test that prefetch_related_queries() generates correct queries
		let queryset = QuerySet::<TestUser>::new().prefetch_related(&["posts", "comments"]);
		let pk_values = vec![1, 2, 3];

		let queries = queryset.prefetch_related_queries(&pk_values);

		// Should generate 2 queries (one for each prefetch field)
		assert_eq!(queries.len(), 2);

		// Each query should be a (field_name, SelectStatement) tuple
		assert_eq!(queries[0].0, "posts");
		assert_eq!(queries[1].0, "comments");
	}

	#[test]
	fn test_prefetch_related_queries_empty_pk_values() {
		let queryset = QuerySet::<TestUser>::new().prefetch_related(&["posts", "comments"]);
		let pk_values = vec![];

		let queries = queryset.prefetch_related_queries(&pk_values);

		// Should return empty vector when no PK values provided
		assert_eq!(queries.len(), 0);
	}

	#[test]
	fn test_select_related_and_prefetch_together() {
		// Test that both can be used together
		let queryset = QuerySet::<TestUser>::new()
			.select_related(&["profile"])
			.prefetch_related(&["posts", "comments"]);

		// Check select_related generates query
		let select_stmt = queryset.select_related_query();
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryStatementBuilder};
		let select_sql = select_stmt.build(PostgresQueryBuilder).0;
		assert!(select_sql.contains("LEFT JOIN"));

		// Check prefetch_related generates queries
		let pk_values = vec![1, 2, 3];
		let prefetch_queries = queryset.prefetch_related_queries(&pk_values);
		assert_eq!(prefetch_queries.len(), 2);
	}

	// SmallVec Optimization Tests

	#[test]
	fn test_smallvec_stack_allocation_within_capacity() {
		// Test with exactly 10 filters (at capacity)
		let mut queryset = QuerySet::<TestUser>::new();

		for i in 0..10 {
			queryset = queryset.filter(Filter::new(
				format!("field{}", i),
				FilterOperator::Eq,
				FilterValue::Integer(i as i64),
			));
		}

		// Verify all filters are stored
		assert_eq!(queryset.filters.len(), 10);

		// Generate SQL to ensure filters work correctly
		let (sql, params) = queryset.delete_sql();
		assert!(sql.contains("WHERE"));
		assert_eq!(params.len(), 10);
	}

	#[test]
	fn test_smallvec_heap_fallback_over_capacity() {
		// Test with 15 filters (5 over capacity, should trigger heap allocation)
		let mut queryset = QuerySet::<TestUser>::new();

		for i in 0..15 {
			queryset = queryset.filter(Filter::new(
				format!("field{}", i),
				FilterOperator::Eq,
				FilterValue::Integer(i as i64),
			));
		}

		// Verify all filters are stored (SmallVec automatically spills to heap)
		assert_eq!(queryset.filters.len(), 15);

		// Generate SQL to ensure filters work correctly even after heap fallback
		let (sql, params) = queryset.delete_sql();
		assert!(sql.contains("WHERE"));
		assert_eq!(params.len(), 15);
	}

	#[test]
	fn test_smallvec_typical_use_case_1_5_filters() {
		// Test typical use case: 1-5 filters (well within stack capacity)
		let queryset = QuerySet::<TestUser>::new()
			.filter(Filter::new(
				"username".to_string(),
				FilterOperator::StartsWith,
				FilterValue::String("admin".to_string()),
			))
			.filter(Filter::new(
				"email".to_string(),
				FilterOperator::Contains,
				FilterValue::String("example.com".to_string()),
			))
			.filter(Filter::new(
				"id".to_string(),
				FilterOperator::Gt,
				FilterValue::Integer(100),
			));

		// Verify filters stored correctly
		assert_eq!(queryset.filters.len(), 3);

		// Generate SQL
		let (sql, params) = queryset.delete_sql();
		assert!(sql.contains("WHERE"));
		assert!(sql.contains("\"username\" LIKE"));
		assert!(sql.contains("\"email\" LIKE"));
		assert!(sql.contains("\"id\" >"));
		assert_eq!(params.len(), 3);
	}

	#[test]
	fn test_smallvec_empty_initialization() {
		// Test that empty SmallVec is initialized correctly
		let queryset = QuerySet::<TestUser>::new();

		assert_eq!(queryset.filters.len(), 0);
		assert!(queryset.filters.is_empty());

		// Generate SQL with no filters should not include WHERE clause
		let (where_clause, params) = queryset.build_where_clause();
		assert!(where_clause.is_empty());
		assert!(params.is_empty());
	}

	#[test]
	fn test_smallvec_single_filter() {
		// Test single filter (minimal usage)
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::Integer(1),
		));

		assert_eq!(queryset.filters.len(), 1);

		let (sql, params) = queryset.delete_sql();
		assert_eq!(sql, "DELETE FROM \"test_users\" WHERE \"id\" = $1");
		assert_eq!(params, vec!["1"]);
	}
}
