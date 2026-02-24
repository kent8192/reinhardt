//! Query helper functions
//!
//! Common query patterns using reinhardt-query for ORM operations.

use reinhardt_query::prelude::{
	Alias, ColumnRef, DeleteStatement, Expr, ExprTrait, Func, InsertStatement, Query,
	SelectStatement, UpdateStatement,
};

use crate::orm::model::Model;

/// Build SELECT COUNT(*) query for a model
///
/// # Example
/// ```rust
/// # use reinhardt_db::orm::Model;
/// # use reinhardt_query::prelude::{QueryStatementBuilder, PostgresQueryBuilder, Query, Alias, ColumnRef, Expr, ExprTrait, Func, SelectStatement};
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct User { id: i64 }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// #     fn primary_key_field() -> &'static str { "id" }
/// # }
/// # fn build_count_query<M: Model>() -> SelectStatement {
/// #     Query::select()
/// #         .expr(Func::count(Expr::asterisk().into_simple_expr()))
/// #         .from(Alias::new(M::table_name()))
/// #         .to_owned()
/// # }
/// let stmt = build_count_query::<User>();
/// let sql = stmt.to_string(PostgresQueryBuilder);
/// assert!(sql.contains("COUNT"));
/// assert!(sql.contains("users"));
/// ```
pub fn build_count_query<M: Model>() -> SelectStatement {
	Query::select()
		.expr(Func::count(Expr::asterisk().into_simple_expr()))
		.from(Alias::new(M::table_name()))
		.to_owned()
}

/// Build SELECT * WHERE pk = ? LIMIT 1 query for a model
///
/// # Example
/// ```rust
/// # use reinhardt_db::orm::Model;
/// # use reinhardt_query::prelude::{QueryStatementBuilder, PostgresQueryBuilder, Query, Alias, ColumnRef, Expr, ExprTrait, SelectStatement};
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct User { id: i64 }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// #     fn primary_key_field() -> &'static str { "id" }
/// # }
/// # fn build_get_query<M: Model, V>(pk: V) -> SelectStatement
/// # where
/// #     V: Into<reinhardt_query::value::Value>,
/// # {
/// #     Query::select()
/// #         .from(Alias::new(M::table_name()))
/// #         .column(ColumnRef::Asterisk)
/// #         .and_where(Expr::col(Alias::new(M::primary_key_field())).eq(pk.into()))
/// #         .limit(1)
/// #         .to_owned()
/// # }
/// let stmt = build_get_query::<User, _>(1);
/// let (sql, _values) = stmt.build(PostgresQueryBuilder);
/// assert!(sql.contains("SELECT"));
/// assert!(sql.contains("users"));
/// assert!(sql.contains("LIMIT"));
/// ```
pub fn build_get_query<M: Model, V>(pk: V) -> SelectStatement
where
	V: Into<reinhardt_query::value::Value>,
{
	Query::select()
		.from(Alias::new(M::table_name()))
		.column(ColumnRef::Asterisk)
		.and_where(Expr::col(Alias::new(M::primary_key_field())).eq(pk.into()))
		.limit(1)
		.to_owned()
}

/// Build DELETE WHERE pk = ? query for a model
///
/// # Example
/// ```rust
/// # use reinhardt_db::orm::Model;
/// # use reinhardt_query::prelude::{QueryStatementBuilder, PostgresQueryBuilder, Query, Alias, Expr, ExprTrait, DeleteStatement};
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct User { id: i64 }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// #     fn primary_key_field() -> &'static str { "id" }
/// # }
/// # fn build_delete_query<M: Model, V>(pk: V) -> DeleteStatement
/// # where
/// #     V: Into<reinhardt_query::value::Value>,
/// # {
/// #     Query::delete()
/// #         .from_table(Alias::new(M::table_name()))
/// #         .and_where(Expr::col(Alias::new(M::primary_key_field())).eq(pk.into()))
/// #         .to_owned()
/// # }
/// let stmt = build_delete_query::<User, _>(1);
/// let (sql, values) = stmt.build(PostgresQueryBuilder);
/// assert!(sql.contains("DELETE"));
/// assert!(sql.contains("users"));
/// assert_eq!(values.0.len(), 1);
/// ```
pub fn build_delete_query<M: Model, V>(pk: V) -> DeleteStatement
where
	V: Into<reinhardt_query::value::Value>,
{
	Query::delete()
		.from_table(Alias::new(M::table_name()))
		.and_where(Expr::col(Alias::new(M::primary_key_field())).eq(pk.into()))
		.to_owned()
}

/// Build INSERT INTO table (columns...) VALUES (values...) query
///
/// # Example
/// ```rust
/// # use reinhardt_db::orm::Model;
/// # use reinhardt_query::prelude::{QueryStatementBuilder, PostgresQueryBuilder, Query, Alias, Expr, ExprTrait, InsertStatement};
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct User { id: i64 }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// #     fn primary_key_field() -> &'static str { "id" }
/// # }
/// # fn build_insert_query<M: Model>(
/// #     columns: Vec<&str>,
/// #     values: Vec<reinhardt_query::value::Value>,
/// # ) -> InsertStatement {
/// #     let mut stmt = Query::insert()
/// #         .into_table(Alias::new(M::table_name()))
/// #         .to_owned();
/// #     let col_refs: Vec<_> = columns.iter().map(|c| Alias::new(*c)).collect();
/// #     stmt.columns(col_refs);
/// #     if !values.is_empty() {
/// #         let exprs: Vec<_> = values.into_iter().map(Expr::val).collect();
/// #         stmt.values_panic(exprs);
/// #     }
/// #     stmt
/// # }
/// let stmt = build_insert_query::<User>(
///     vec!["name", "email"],
///     vec!["Alice".into(), "alice@example.com".into()]
/// );
/// let (sql, values) = stmt.build(PostgresQueryBuilder);
/// assert!(sql.contains("INSERT"));
/// assert!(sql.contains("users"));
/// assert_eq!(values.0.len(), 2);
/// ```
pub fn build_insert_query<M: Model>(
	columns: Vec<&str>,
	values: Vec<reinhardt_query::value::Value>,
) -> InsertStatement {
	let mut stmt = Query::insert()
		.into_table(Alias::new(M::table_name()))
		.to_owned();

	// Set columns
	let col_refs: Vec<_> = columns.iter().map(|c| Alias::new(*c)).collect();
	stmt.columns(col_refs);

	// Set values - values_panic expects an array/slice, not a Vec
	if !values.is_empty() {
		// Convert Vec<Value> to expressions for values_panic
		let exprs: Vec<_> = values.into_iter().map(Expr::val).collect();
		stmt.values_panic(exprs);
	}

	stmt
}

/// Build UPDATE table SET columns = values WHERE pk = ? query
///
/// # Example
/// ```rust
/// # use reinhardt_db::orm::Model;
/// # use reinhardt_query::prelude::{QueryStatementBuilder, PostgresQueryBuilder, Query, Alias, Expr, ExprTrait, UpdateStatement};
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct User { id: i64 }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// #     fn primary_key_field() -> &'static str { "id" }
/// # }
/// # fn build_update_query<M: Model, V>(
/// #     updates: Vec<(&str, reinhardt_query::value::Value)>,
/// #     pk: V,
/// # ) -> UpdateStatement
/// # where
/// #     V: Into<reinhardt_query::value::Value>,
/// # {
/// #     let mut stmt = Query::update()
/// #         .table(Alias::new(M::table_name()))
/// #         .to_owned();
/// #     for (col, val) in updates {
/// #         stmt.value(Alias::new(col), val);
/// #     }
/// #     stmt.and_where(Expr::col(Alias::new(M::primary_key_field())).eq(pk.into()))
/// #         .to_owned()
/// # }
/// let stmt = build_update_query::<User, _>(
///     vec![("name", "Bob".into()), ("email", "bob@example.com".into())],
///     1
/// );
/// let (sql, values) = stmt.build(PostgresQueryBuilder);
/// assert!(sql.contains("UPDATE"));
/// assert!(sql.contains("users"));
/// assert_eq!(values.0.len(), 3); // 2 updates + 1 where condition
/// ```
pub fn build_update_query<M: Model, V>(
	updates: Vec<(&str, reinhardt_query::value::Value)>,
	pk: V,
) -> UpdateStatement
where
	V: Into<reinhardt_query::value::Value>,
{
	let mut stmt = Query::update()
		.table(Alias::new(M::table_name()))
		.to_owned();

	for (col, val) in updates {
		stmt.value(Alias::new(col), val);
	}

	stmt.and_where(Expr::col(Alias::new(M::primary_key_field())).eq(pk.into()))
		.to_owned()
}

/// Build SELECT EXISTS(...) query
///
/// # Example
/// ```rust
/// # use reinhardt_query::prelude::{QueryStatementBuilder, PostgresQueryBuilder, Query, Alias, Expr, ExprTrait, SelectStatement};
/// # fn build_exists_query(inner: SelectStatement) -> SelectStatement {
/// #     Query::select().expr(Expr::exists(inner)).to_owned()
/// # }
/// let inner = Query::select()
///     .from(Alias::new("users"))
///     .and_where(Expr::col("id").eq(1))
///     .to_owned();
/// let stmt = build_exists_query(inner);
/// let sql = stmt.to_string(PostgresQueryBuilder);
/// assert!(sql.contains("EXISTS"));
/// assert!(sql.contains("SELECT"));
/// ```
pub fn build_exists_query(inner: SelectStatement) -> SelectStatement {
	Query::select().expr(Expr::exists(inner)).to_owned()
}

/// Build SELECT * FROM table WHERE column IN (values...) query
///
/// # Example
/// ```rust
/// # use reinhardt_db::orm::Model;
/// # use reinhardt_query::prelude::{QueryStatementBuilder, PostgresQueryBuilder, Query, Alias, ColumnRef, Expr, ExprTrait, SelectStatement};
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct User { id: i64 }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// #     fn primary_key_field() -> &'static str { "id" }
/// # }
/// # fn build_in_query<M: Model>(column: &str, values: Vec<reinhardt_query::value::Value>) -> SelectStatement {
/// #     Query::select()
/// #         .from(Alias::new(M::table_name()))
/// #         .column(ColumnRef::Asterisk)
/// #         .and_where(Expr::col(Alias::new(column)).is_in(values))
/// #         .to_owned()
/// # }
/// let stmt = build_in_query::<User>("id", vec![1.into(), 2.into(), 3.into()]);
/// let (sql, values) = stmt.build(PostgresQueryBuilder);
/// assert!(sql.contains("SELECT"));
/// assert!(sql.contains("users"));
/// assert!(sql.contains("IN"));
/// assert_eq!(values.0.len(), 3);
/// ```
pub fn build_in_query<M: Model>(
	column: &str,
	values: Vec<reinhardt_query::value::Value>,
) -> SelectStatement {
	Query::select()
		.from(Alias::new(M::table_name()))
		.column(ColumnRef::Asterisk)
		.and_where(Expr::col(Alias::new(column)).is_in(values))
		.to_owned()
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_query::prelude::{
		MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder,
	};
	use serde::{Deserialize, Serialize};

	// Mock Model for testing
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestModel {
		id: i64,
	}

	#[derive(Clone)]
	struct TestModelFields;
	impl crate::orm::model::FieldSelector for TestModelFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestModel {
		type PrimaryKey = i64;
		type Fields = TestModelFields;

		fn table_name() -> &'static str {
			"test_table"
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			TestModelFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.id)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}
	}

	#[test]
	fn test_build_count_query() {
		let stmt = build_count_query::<TestModel>();
		let (sql, _) = PostgresQueryBuilder::new().build_select(&stmt);
		assert!(sql.contains("COUNT"));
		assert!(sql.contains("test_table"));
	}

	#[test]
	fn test_build_get_query() {
		let stmt = build_get_query::<TestModel, _>(123);
		let (sql, values) = PostgresQueryBuilder::new().build_select(&stmt);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("test_table"));
		assert!(sql.contains("id"));
		assert!(sql.contains("LIMIT"));
		// reinhardt-query binds LIMIT value as a parameter, so we have 2 values: pk + limit
		assert_eq!(values.0.len(), 2);
	}

	#[test]
	fn test_build_delete_query() {
		let stmt = build_delete_query::<TestModel, _>(456);
		let (sql, values) = PostgresQueryBuilder::new().build_delete(&stmt);
		assert!(sql.contains("DELETE"));
		assert!(sql.contains("test_table"));
		assert!(sql.contains("id"));
		assert_eq!(values.0.len(), 1);
	}

	#[test]
	fn test_build_insert_query() {
		let stmt = build_insert_query::<TestModel>(
			vec!["name", "email"],
			vec!["Alice".into(), "alice@example.com".into()],
		);
		let (sql, values) = PostgresQueryBuilder::new().build_insert(&stmt);
		assert!(sql.contains("INSERT"));
		assert!(sql.contains("test_table"));
		assert!(sql.contains("name"));
		assert!(sql.contains("email"));
		assert_eq!(values.0.len(), 2);
	}

	#[test]
	fn test_build_update_query() {
		let stmt = build_update_query::<TestModel, _>(
			vec![("name", "Bob".into()), ("age", 30.into())],
			789,
		);
		let (sql, values) = PostgresQueryBuilder::new().build_update(&stmt);
		assert!(sql.contains("UPDATE"));
		assert!(sql.contains("test_table"));
		assert!(sql.contains("name"));
		assert!(sql.contains("age"));
		assert!(sql.contains("id"));
		assert_eq!(values.0.len(), 3); // 2 updates + 1 where condition
	}

	#[test]
	fn test_build_in_query() {
		let stmt = build_in_query::<TestModel>("status", vec!["active".into(), "pending".into()]);
		let (sql, values) = PostgresQueryBuilder::new().build_select(&stmt);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("test_table"));
		assert!(sql.contains("IN"));
		assert_eq!(values.0.len(), 2);
	}

	#[test]
	fn test_different_backends() {
		let stmt = build_count_query::<TestModel>();

		// PostgreSQL uses double quotes
		let (pg_sql, _) = PostgresQueryBuilder::new().build_select(&stmt);
		assert!(pg_sql.contains("\"test_table\""));

		// MySQL uses backticks
		let (mysql_sql, _) = MySqlQueryBuilder::new().build_select(&stmt);
		assert!(mysql_sql.contains("`test_table`"));

		// SQLite uses double quotes
		let (sqlite_sql, _) = SqliteQueryBuilder::new().build_select(&stmt);
		assert!(sqlite_sql.contains("\"test_table\""));
	}
}
