//! # SQLAlchemy-style Query Builder
//!
//! Implements SQLAlchemy's select(), where(), join() query construction pattern.
//!
//! This module is inspired by SQLAlchemy's sql/selectable.py
//! Copyright 2005-2025 SQLAlchemy authors and contributors
//! Licensed under MIT License. See THIRD-PARTY-NOTICES for details.

use super::set_operations::CombinedQuery;
use super::typed_join::TypedJoin;
use super::{Model, Q};
use crate::orm::query_fields::{Field, Lookup, QueryFieldCompiler};
use std::marker::PhantomData;

/// Column reference for SELECT clause
#[derive(Debug, Clone)]
pub struct Column {
	table: Option<String>,
	name: String,
}

impl Column {
	pub fn new(name: &str) -> Self {
		Self {
			table: None,
			name: name.to_string(),
		}
	}

	pub fn with_table(mut self, table: &str) -> Self {
		self.table = Some(table.to_string());
		self
	}

	pub fn to_sql(&self) -> String {
		match &self.table {
			Some(table) => format!("{}.{}", table, self.name),
			None => self.name.clone(),
		}
	}
}

/// Join type - mirrors SQLAlchemy's join types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
	Inner,
	Left,
	Right,
	Full,
}

/// SQLAlchemy-style SELECT query builder
/// Inspired by SQLAlchemy's Select class
#[derive(Debug)]
pub struct SelectQuery<T: Model> {
	columns: Vec<Column>,
	where_clauses: Vec<Q>,
	joins: Vec<(String, JoinType, String)>, // (table, join_type, condition)
	order_by: Vec<(String, bool)>,          // (column, ascending)
	group_by: Vec<String>,
	having: Option<Q>,
	limit: Option<usize>,
	offset: Option<usize>,
	distinct: bool,
	_phantom: PhantomData<T>,
}

impl<T: Model> SelectQuery<T> {
	/// Create a new SELECT query
	/// Corresponds to SQLAlchemy's select()
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::sqlalchemy_query::SelectQuery;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i64>,
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
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn table_name() -> &'static str { "users" }
	///     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// let query = SelectQuery::<User>::new();
	/// // Query is ready for configuration
	/// ```
	pub fn new() -> Self {
		Self {
			columns: Vec::new(),
			where_clauses: Vec::new(),
			joins: Vec::new(),
			order_by: Vec::new(),
			group_by: Vec::new(),
			having: None,
			limit: None,
			offset: None,
			distinct: false,
			_phantom: PhantomData,
		}
	}

	/// Add columns to SELECT
	/// Corresponds to SQLAlchemy's select(column1, column2, ...)
	pub fn columns(mut self, cols: Vec<Column>) -> Self {
		self.columns = cols;
		self
	}

	/// Add WHERE clause
	/// Corresponds to SQLAlchemy's .where()
	pub fn where_clause(mut self, condition: Q) -> Self {
		self.where_clauses.push(condition);
		self
	}

	/// Add multiple WHERE clauses (AND combined)
	pub fn where_all(mut self, conditions: Vec<Q>) -> Self {
		self.where_clauses.extend(conditions);
		self
	}

	/// Add JOIN
	/// Corresponds to SQLAlchemy's .join()
	pub fn join(mut self, table: &str, on_condition: &str) -> Self {
		self.joins
			.push((table.to_string(), JoinType::Inner, on_condition.to_string()));
		self
	}

	/// Add LEFT JOIN
	/// Corresponds to SQLAlchemy's .outerjoin()
	pub fn left_join(mut self, table: &str, on_condition: &str) -> Self {
		self.joins
			.push((table.to_string(), JoinType::Left, on_condition.to_string()));
		self
	}

	/// Type-safe JOIN using TypedJoin
	///
	/// This method provides compile-time type safety for JOIN operations.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::orm::{Model, query_fields::Field, typed_join::TypedJoin, sqlalchemy_query::select};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Post { id: Option<i64> }
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Comment { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # #[derive(Clone)]
	/// # struct CommentFields;
	/// # impl reinhardt_db::orm::FieldSelector for CommentFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	/// # impl Model for Comment {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = CommentFields;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "comments" }
	/// #     fn new_fields() -> Self::Fields { CommentFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	/// # impl User {
	/// #     fn id() -> Field<Self, i64> { Field::new(vec!["id"]) }
	/// # }
	/// # impl Post {
	/// #     fn user_id() -> Field<Self, i64> { Field::new(vec!["user_id"]) }
	/// # }
	/// # impl Comment {
	/// #     fn user_id() -> Field<Self, i64> { Field::new(vec!["user_id"]) }
	/// # }
	/// select::<User>()
	///     .join_on(TypedJoin::on(User::id(), Post::user_id()))
	///     .join_on(TypedJoin::left_on(User::id(), Comment::user_id()));
	/// ```
	///
	/// # Type Safety
	///
	/// The compiler enforces:
	/// - Both join fields must have the same type
	/// - Field names must exist on their respective models
	pub fn join_on<R: Model>(mut self, join: TypedJoin<T, R>) -> Self {
		let (table, join_type, condition) = join.to_sql();
		self.joins.push((table, join_type, condition));
		self
	}

	/// Add ORDER BY
	/// Corresponds to SQLAlchemy's .order_by()
	pub fn order_by(mut self, column: &str, ascending: bool) -> Self {
		self.order_by.push((column.to_string(), ascending));
		self
	}

	/// Type-safe ORDER BY using Field
	///
	/// This method provides compile-time type safety for ordering.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::orm::{Model, query_fields::Field, sqlalchemy_query::select};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	/// # impl User {
	/// #     fn email() -> Field<Self, String> { Field::new(vec!["email"]) }
	/// #     fn age() -> Field<Self, i32> { Field::new(vec!["age"]) }
	/// # }
	/// select::<User>()
	///     .order_by_field(User::email(), true)  // ASC
	///     .order_by_field(User::age(), false);   // DESC
	/// ```
	pub fn order_by_field<F>(mut self, field: Field<T, F>, ascending: bool) -> Self {
		let field_path = field.path().join(".");
		self.order_by.push((field_path, ascending));
		self
	}

	/// Add GROUP BY
	/// Corresponds to SQLAlchemy's .group_by()
	pub fn group_by(mut self, columns: Vec<&str>) -> Self {
		self.group_by = columns.iter().map(|c| c.to_string()).collect();
		self
	}

	/// Add HAVING clause
	/// Corresponds to SQLAlchemy's .having()
	pub fn having(mut self, condition: Q) -> Self {
		self.having = Some(condition);
		self
	}

	/// Set LIMIT
	/// Corresponds to SQLAlchemy's .limit()
	pub fn limit(mut self, limit: usize) -> Self {
		self.limit = Some(limit);
		self
	}

	/// Set OFFSET
	/// Corresponds to SQLAlchemy's .offset()
	pub fn offset(mut self, offset: usize) -> Self {
		self.offset = Some(offset);
		self
	}

	/// Set DISTINCT
	/// Corresponds to SQLAlchemy's .distinct()
	pub fn distinct(mut self) -> Self {
		self.distinct = true;
		self
	}

	/// Filter by column values (dict-style)
	/// Corresponds to SQLAlchemy's .filter_by()
	pub fn filter_by(mut self, filters: Vec<(&str, &str)>) -> Self {
		for (column, value) in filters {
			self.where_clauses.push(Q::new(column, "=", value));
		}
		self
	}

	/// Select specific entities/columns
	/// Corresponds to SQLAlchemy's .with_entities()
	pub fn with_entities(mut self, columns: Vec<Column>) -> Self {
		self.columns = columns;
		self
	}

	/// Count query
	/// Returns query that counts results
	pub fn count_query(mut self) -> Self {
		self.columns = vec![Column::new("COUNT(*)")];
		self
	}

	/// First result - corresponds to SQLAlchemy's .first()
	pub fn first(mut self) -> Self {
		self.limit = Some(1);
		self
	}

	/// All results - corresponds to SQLAlchemy's .all()
	pub fn all(self) -> Self {
		self
	}

	/// One result - corresponds to SQLAlchemy's .one()
	///
	/// Sets LIMIT 2 to detect multiple results. The execution layer should:
	/// - Error if 0 results are returned (NoResultFound)
	/// - Error if 2+ results are returned (MultipleResultsFound)
	/// - Return the single result if exactly 1 is found
	pub fn one(mut self) -> Self {
		self.limit = Some(2);
		self
	}

	/// Scalar result - corresponds to SQLAlchemy's .scalar()
	/// Returns single column of first result
	pub fn scalar(mut self) -> Self {
		self.limit = Some(1);
		self
	}

	/// Filter - corresponds to SQLAlchemy's .filter()
	pub fn filter(mut self, condition: Q) -> Self {
		self.where_clauses.push(condition);
		self
	}

	/// Type-safe filter using FieldLookup system
	///
	/// This method provides compile-time type safety for filter conditions.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::orm::{Model, query_fields::Field, sqlalchemy_query::select};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	/// # impl User {
	/// #     fn email() -> Field<Self, String> { Field::new(vec!["email"]) }
	/// #     fn age() -> Field<Self, i32> { Field::new(vec!["age"]) }
	/// # }
	/// select::<User>()
	///     .filter_lookup(User::email().lower().contains("example.com"))
	///     .filter_lookup(User::age().gte(18));
	/// ```
	///
	/// # Type Safety
	///
	/// The compiler enforces:
	/// - Field names must exist on the model
	/// - Operations must be valid for the field type
	/// - Values must match the field type
	pub fn filter_lookup(mut self, lookup: Lookup<T>) -> Self {
		// Compile the type-safe lookup into SQL
		let sql = QueryFieldCompiler::compile(&lookup);
		// Convert to Q and add to where clauses
		self.where_clauses.push(Q::from_sql(&sql));
		self
	}

	/// Slice - corresponds to SQLAlchemy's slicing `[start:end]`
	pub fn slice(mut self, start: usize, end: usize) -> Self {
		self.offset = Some(start);
		self.limit = Some(end - start);
		self
	}

	/// Union with another query
	///
	/// Combines two queries using UNION (removes duplicates).
	/// Returns a CombinedQuery that can be further chained.
	///
	/// # Examples
	///
	/// ```ignore
	/// let query1 = select::<User>().filter(User::is_active.eq(true));
	/// let query2 = select::<User>().filter(User::is_admin.eq(true));
	/// let combined = query1.union(query2);
	/// ```
	pub fn union(self, other: SelectQuery<T>) -> CombinedQuery {
		CombinedQuery::new(self.to_sql()).union(other.to_sql())
	}

	/// Union All with another query
	///
	/// Combines two queries using UNION ALL (keeps duplicates).
	/// Returns a CombinedQuery that can be further chained.
	///
	/// # Examples
	///
	/// ```ignore
	/// let query1 = select::<User>().filter(User::is_active.eq(true));
	/// let query2 = select::<User>().filter(User::is_admin.eq(true));
	/// let combined = query1.union_all(query2);
	/// ```
	pub fn union_all(self, other: SelectQuery<T>) -> CombinedQuery {
		CombinedQuery::new(self.to_sql()).union_all(other.to_sql())
	}

	/// Intersect with another query
	///
	/// Combines two queries using INTERSECT (returns common rows).
	/// Returns a CombinedQuery that can be further chained.
	///
	/// # Examples
	///
	/// ```ignore
	/// let query1 = select::<User>().filter(User::is_active.eq(true));
	/// let query2 = select::<User>().filter(User::department.eq("Engineering"));
	/// let combined = query1.intersect(query2);  // Active engineers
	/// ```
	pub fn intersect(self, other: SelectQuery<T>) -> CombinedQuery {
		CombinedQuery::new(self.to_sql()).intersect(other.to_sql())
	}

	/// Except (difference) with another query
	///
	/// Combines two queries using EXCEPT (returns rows in first query but not in second).
	/// Returns a CombinedQuery that can be further chained.
	///
	/// # Examples
	///
	/// ```ignore
	/// let query1 = select::<User>();  // All users
	/// let query2 = select::<User>().filter(User::is_deleted.eq(true));  // Deleted users
	/// let combined = query1.except(query2);  // Active users only
	/// ```
	pub fn except(self, other: SelectQuery<T>) -> CombinedQuery {
		CombinedQuery::new(self.to_sql()).except(other.to_sql())
	}

	/// Generate SQL string
	pub fn to_sql(&self) -> String {
		let mut sql = String::from("SELECT ");

		if self.distinct {
			sql.push_str("DISTINCT ");
		}

		// SELECT columns
		if self.columns.is_empty() {
			sql.push('*');
		} else {
			let cols: Vec<String> = self.columns.iter().map(|c| c.to_sql()).collect();
			sql.push_str(&cols.join(", "));
		}

		// FROM table
		sql.push_str(&format!(" FROM {}", T::table_name()));

		// JOINs
		for (table, join_type, condition) in &self.joins {
			let join_keyword = match join_type {
				JoinType::Inner => "INNER JOIN",
				JoinType::Left => "LEFT JOIN",
				JoinType::Right => "RIGHT JOIN",
				JoinType::Full => "FULL JOIN",
			};
			sql.push_str(&format!(" {} {} ON {}", join_keyword, table, condition));
		}

		// WHERE clauses
		if !self.where_clauses.is_empty() {
			sql.push_str(" WHERE ");
			let conditions: Vec<String> = self.where_clauses.iter().map(|q| q.to_sql()).collect();
			sql.push_str(&conditions.join(" AND "));
		}

		// GROUP BY
		if !self.group_by.is_empty() {
			sql.push_str(&format!(" GROUP BY {}", self.group_by.join(", ")));
		}

		// HAVING
		if let Some(having) = &self.having {
			sql.push_str(&format!(" HAVING {}", having.to_sql()));
		}

		// ORDER BY
		if !self.order_by.is_empty() {
			sql.push_str(" ORDER BY ");
			let orders: Vec<String> = self
				.order_by
				.iter()
				.map(|(col, asc)| {
					if *asc {
						col.clone()
					} else {
						format!("{} DESC", col)
					}
				})
				.collect();
			sql.push_str(&orders.join(", "));
		}

		// LIMIT and OFFSET
		if let Some(limit) = self.limit {
			sql.push_str(&format!(" LIMIT {}", limit));
		}
		if let Some(offset) = self.offset {
			sql.push_str(&format!(" OFFSET {}", offset));
		}

		sql
	}
}

impl<T: Model> Default for SelectQuery<T> {
	fn default() -> Self {
		Self::new()
	}
}

/// Helper function to create SELECT query
/// Mimics SQLAlchemy's select() function
pub fn select<T: Model>() -> SelectQuery<T> {
	SelectQuery::new()
}

/// Helper to create column reference
/// Mimics SQLAlchemy's column() function
pub fn column(name: &str) -> Column {
	Column::new(name)
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::validators::TableName;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct User {
		id: Option<i64>,
		name: String,
		email: String,
	}

	const USER_TABLE: TableName = TableName::new_const("users");

	#[derive(Debug, Clone)]
	struct UserFields;

	impl crate::orm::model::FieldSelector for UserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for User {
		type PrimaryKey = i64;
		type Fields = UserFields;

		fn table_name() -> &'static str {
			USER_TABLE.as_str()
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
			UserFields
		}
	}

	// Allow dead_code: test model struct for SQLAlchemy-style query tests
	#[allow(dead_code)]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct Post {
		id: Option<i64>,
		user_id: i64,
		title: String,
	}

	const POST_TABLE: TableName = TableName::new_const("test_post");

	#[derive(Debug, Clone)]
	struct PostFields;

	impl crate::orm::model::FieldSelector for PostFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for Post {
		type PrimaryKey = i64;
		type Fields = PostFields;

		fn table_name() -> &'static str {
			POST_TABLE.as_str()
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
			PostFields
		}
	}

	#[test]
	fn test_simple_select() {
		let query = select::<User>();
		let sql = query.to_sql();
		assert_eq!(sql, "SELECT * FROM users");
	}

	#[test]
	fn test_select_with_columns() {
		let query = select::<User>().columns(vec![column("id"), column("name")]);
		let sql = query.to_sql();
		assert_eq!(sql, "SELECT id, name FROM users");
	}

	#[test]
	fn test_select_with_where() {
		let query = select::<User>().where_clause(Q::new("email", "=", "test@example.com"));
		let sql = query.to_sql();
		assert_eq!(sql, "SELECT * FROM users WHERE email = 'test@example.com'");
	}

	#[test]
	fn test_select_with_join() {
		let query = select::<User>()
			.join("posts", "users.id = posts.user_id")
			.where_clause(Q::new("posts.published", "=", "true"));
		let sql = query.to_sql();
		assert!(sql.contains("INNER JOIN posts ON users.id = posts.user_id"));
		assert!(
			sql.contains("WHERE posts.published = true")
				|| sql.contains("WHERE posts.published = TRUE")
		);
	}

	#[test]
	fn test_select_with_left_join() {
		let query = select::<User>().left_join("posts", "users.id = posts.user_id");
		let sql = query.to_sql();
		assert!(sql.contains("LEFT JOIN posts ON users.id = posts.user_id"));
	}

	#[test]
	fn test_select_with_order_by() {
		let query = select::<User>()
			.order_by("name", true)
			.order_by("created_at", false);
		let sql = query.to_sql();
		assert!(sql.contains("ORDER BY name, created_at DESC"));
	}

	#[test]
	fn test_select_with_group_by() {
		let query = select::<User>()
			.columns(vec![column("status"), column("COUNT(*)")])
			.group_by(vec!["status"]);
		let sql = query.to_sql();
		assert!(sql.contains("GROUP BY status"));
	}

	#[test]
	fn test_select_with_having() {
		let query = select::<User>()
			.group_by(vec!["status"])
			.having(Q::new("COUNT(*)", ">", "5"));
		let sql = query.to_sql();
		assert!(sql.contains("HAVING COUNT(*) > 5"));
	}

	#[test]
	fn test_select_with_limit_offset() {
		let query = select::<User>().limit(10).offset(20);
		let sql = query.to_sql();
		assert!(sql.contains("LIMIT 10"));
		assert!(sql.contains("OFFSET 20"));
	}

	#[test]
	fn test_sqlalchemy_query_select_distinct() {
		let query = select::<User>().distinct().columns(vec![column("email")]);
		let sql = query.to_sql();
		assert!(sql.contains("SELECT DISTINCT email"));
	}

	#[test]
	fn test_complex_query() {
		let query = select::<User>()
			.columns(vec![column("users.id"), column("users.name")])
			.join("posts", "users.id = posts.user_id")
			.where_clause(Q::new("users.active", "=", "true"))
			.where_clause(Q::new("posts.published", "=", "true"))
			.group_by(vec!["users.id", "users.name"])
			.having(Q::new("COUNT(posts.id)", ">", "5"))
			.order_by("users.name", true)
			.limit(10);

		let sql = query.to_sql();
		assert!(sql.contains("SELECT users.id, users.name"));
		assert!(sql.contains("FROM users"));
		assert!(sql.contains("INNER JOIN posts"));
		assert!(
			sql.contains("WHERE users.active = true") || sql.contains("WHERE users.active = TRUE")
		);
		assert!(sql.contains("GROUP BY users.id, users.name"));
		assert!(sql.contains("HAVING COUNT(posts.id) > 5"));
		assert!(sql.contains("ORDER BY users.name"));
		assert!(sql.contains("LIMIT 10"));
	}
}
