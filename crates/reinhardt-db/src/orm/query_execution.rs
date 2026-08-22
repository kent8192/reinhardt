//! # Query Compilation and Execution
//!
//! Compile and execute queries against the database.
//!
//! This module is inspired by SQLAlchemy's query execution patterns
//! Copyright 2005-2025 SQLAlchemy authors and contributors
//! Licensed under MIT License. See THIRD-PARTY-NOTICES for details.

use super::engine::Engine;
use super::types::DatabaseDialect;
use crate::orm::Model;
use crate::orm::expressions::{Q, QOperator};
use crate::orm::query::{parse_column_reference, parse_membership_string};
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Result};
use reinhardt_query::prelude::{
	Alias, ColumnRef, Condition, DeleteStatement, Expr, ExprTrait, InsertStatement, Order, Query,
	SelectStatement, SimpleExpr, UpdateStatement,
};
use serde::de::DeserializeOwned;
use std::marker::PhantomData;

/// Query compiler - converts query structures to SQL
#[derive(Debug, Clone)]
pub struct QueryCompiler {
	dialect: DatabaseDialect,
}

impl QueryCompiler {
	/// Create a new query compiler
	pub fn new(dialect: DatabaseDialect) -> Self {
		Self { dialect }
	}

	/// Convert Q expression to reinhardt-query Condition
	fn q_to_condition(q: &Q) -> Condition {
		match q {
			Q::Condition {
				field,
				operator,
				value,
			} => {
				// Raw SQL is not accepted by the safe query compiler.
				if field.is_empty() && operator.is_empty() {
					return Self::false_condition();
				}

				let expr =
					Self::build_condition_expr(field.as_str(), operator.as_str(), value.as_str());
				Condition::all().add(expr)
			}
			Q::Combined {
				operator,
				conditions,
			} => match operator {
				QOperator::And => {
					let mut cond = Condition::all();
					for q in conditions {
						let sub_cond = Self::q_to_condition(q);
						cond = cond.add(sub_cond);
					}
					cond
				}
				QOperator::Or => {
					let mut cond = Condition::any();
					for q in conditions {
						let sub_cond = Self::q_to_condition(q);
						cond = cond.add(sub_cond);
					}
					cond
				}
				QOperator::Not => {
					if let Some(first) = conditions.first() {
						Self::q_to_condition(first).not()
					} else {
						Condition::all()
					}
				}
			},
		}
	}

	/// Build condition expression from field, operator and value
	fn build_condition_expr(field: &str, operator: &str, value: &str) -> SimpleExpr {
		let column = Expr::col(parse_column_reference(field));
		let scalar = || Self::parse_value(value);

		match operator.to_ascii_uppercase().as_str() {
			"=" if value.eq_ignore_ascii_case("NULL") => column.is_null(),
			"!=" | "<>" if value.eq_ignore_ascii_case("NULL") => column.is_not_null(),
			"=" => column.eq(scalar()),
			"!=" | "<>" => column.ne(scalar()),
			">" => column.gt(scalar()),
			">=" => column.gte(scalar()),
			"<" => column.lt(scalar()),
			"<=" => column.lte(scalar()),
			"IN" => column.is_in(parse_membership_string(value)),
			"NOT IN" => column.is_not_in(parse_membership_string(value)),
			"LIKE" => column.like(value.to_owned()),
			"IS NULL" => column.is_null(),
			"IS NOT NULL" => column.is_not_null(),
			_ => Expr::cust("FALSE").into_simple_expr(),
		}
	}

	fn parse_value(value: impl AsRef<str>) -> reinhardt_query::value::Value {
		let value = value.as_ref().trim();
		let unquoted = value
			.strip_prefix('\'')
			.and_then(|value| value.strip_suffix('\''))
			.unwrap_or(value);

		if unquoted.eq_ignore_ascii_case("TRUE") {
			true.into()
		} else if unquoted.eq_ignore_ascii_case("FALSE") {
			false.into()
		} else if let Ok(value) = unquoted.parse::<i64>() {
			value.into()
		} else if let Ok(value) = unquoted.parse::<f64>() {
			value.into()
		} else {
			unquoted.replace("''", "'").into()
		}
	}

	fn false_condition() -> Condition {
		Condition::all().add(Expr::cust("FALSE").into_simple_expr())
	}

	/// Compile a SELECT query
	///
	pub fn compile_select<T: Model>(
		&self,
		table: &str,
		columns: &[&str],
		where_clause: Option<&Q>,
		order_by: &[&str],
		limit: Option<usize>,
		offset: Option<usize>,
	) -> SelectStatement {
		let mut stmt = Query::select();
		stmt.from(Alias::new(table));

		// Add columns
		if columns.is_empty() {
			stmt.column(ColumnRef::Asterisk);
		} else {
			for col in columns {
				stmt.column(Alias::new(*col));
			}
		}

		// Add WHERE clause
		if let Some(q) = where_clause {
			let cond = Self::q_to_condition(q);
			stmt.cond_where(cond);
		}

		// Add ORDER BY
		for col in order_by {
			stmt.order_by(Alias::new(*col), Order::Asc);
		}

		// Add LIMIT
		if let Some(lim) = limit {
			stmt.limit(lim as u64);
		}

		// Add OFFSET
		if let Some(off) = offset {
			stmt.offset(off as u64);
		}

		stmt.to_owned()
	}
	/// Compile an INSERT query
	///
	pub fn compile_insert<T: Model>(
		&self,
		table: &str,
		columns: &[&str],
		values: &[&str],
	) -> InsertStatement {
		let mut stmt = Query::insert();
		stmt.into_table(Alias::new(table));

		// Add columns
		let col_refs: Vec<_> = columns.iter().map(|c| Alias::new(*c)).collect();
		stmt.columns(col_refs);

		// Add values as parameterized values
		// Values are passed as strings, wrapped in Value::String for parameterized binding
		let vals: Vec<_> = values
			.iter()
			.map(|v| reinhardt_query::value::Value::String(Some(Box::new(v.to_string()))))
			.collect();
		stmt.values(vals).expect("Failed to add values");

		stmt.to_owned()
	}
	/// Compile an UPDATE query
	///
	pub fn compile_update<T: Model>(
		&self,
		table: &str,
		updates: &[(&str, &str)],
		where_clause: Option<&Q>,
	) -> UpdateStatement {
		let mut stmt = Query::update();
		stmt.table(Alias::new(table));

		// Add SET clauses
		for (col, val) in updates {
			// Values are passed as strings, wrapped in Value::String for parameterized binding
			stmt.value(Alias::new(*col), Expr::val(val.to_string()));
		}

		// Add WHERE clause
		if let Some(q) = where_clause {
			let cond = Self::q_to_condition(q);
			stmt.cond_where(cond);
		}

		stmt.to_owned()
	}
	/// Compile a DELETE query
	///
	pub fn compile_delete<T: Model>(
		&self,
		table: &str,
		where_clause: Option<&Q>,
	) -> DeleteStatement {
		let mut stmt = Query::delete();
		stmt.from_table(Alias::new(table));

		// Add WHERE clause
		if let Some(q) = where_clause {
			let cond = Self::q_to_condition(q);
			stmt.cond_where(cond);
		}

		stmt.to_owned()
	}
	/// Get the current dialect
	///
	pub fn dialect(&self) -> DatabaseDialect {
		self.dialect
	}
}

/// Executable query - compiled query ready to execute
pub struct ExecutableQuery<T: Model> {
	sql: String,
	engine: Option<Engine>,
	_phantom: PhantomData<T>,
}

impl<T: Model> ExecutableQuery<T> {
	/// Create a new executable query
	pub fn new(sql: impl Into<String>) -> Self {
		Self {
			sql: sql.into(),
			engine: None,
			_phantom: PhantomData,
		}
	}
	/// Bind an engine to this query
	pub fn with_engine(mut self, engine: Engine) -> Self {
		self.engine = Some(engine);
		self
	}
	/// Get the SQL string
	///
	pub fn sql(&self) -> &str {
		&self.sql
	}
	/// Execute the query and return affected rows
	///
	pub async fn execute(&self) -> Result<u64> {
		match &self.engine {
			Some(engine) => engine.execute(&self.sql).await,
			None => Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"No engine bound to query",
			)
			.into()),
		}
	}
	/// Execute the query and fetch all results
	///
	pub async fn fetch_all(&self) -> Result<Vec<sqlx::any::AnyRow>>
	where
		T: DeserializeOwned,
	{
		match &self.engine {
			Some(engine) => engine.fetch_all(&self.sql).await,
			None => Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"No engine bound to query",
			)
			.into()),
		}
	}
	/// Execute the query and fetch one result
	///
	pub async fn fetch_one(&self) -> Result<sqlx::any::AnyRow>
	where
		T: DeserializeOwned,
	{
		match &self.engine {
			Some(engine) => engine.fetch_one(&self.sql).await,
			None => Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"No engine bound to query",
			)
			.into()),
		}
	}
	/// Execute the query and fetch optional result
	///
	pub async fn fetch_optional(&self) -> Result<Option<sqlx::any::AnyRow>>
	where
		T: DeserializeOwned,
	{
		match &self.engine {
			Some(engine) => engine.fetch_optional(&self.sql).await,
			None => Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"No engine bound to query",
			)
			.into()),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::Manager;
	use reinhardt_core::validators::TableName;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestModel {
		id: Option<i64>,
		name: String,
	}

	const TEST_MODEL_TABLE: TableName = TableName::new_const("test_model");

	#[derive(Debug, Clone)]
	struct TestModelFields;

	impl crate::orm::model::FieldSelector for TestModelFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestModel {
		type PrimaryKey = i64;
		type Fields = TestModelFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			TEST_MODEL_TABLE.as_str()
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
			TestModelFields
		}
	}

	#[test]
	fn test_compile_select() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let stmt = compiler.compile_select::<TestModel>(
			"test_models",
			&["id", "name"],
			None,
			&[],
			None,
			None,
		);

		let sql = stmt.to_string(SqliteQueryBuilder);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("id"));
		assert!(sql.contains("name"));
		assert!(sql.contains("test_models"));
	}

	#[test]
	fn test_compile_select_with_where() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let q = Q::new("age", ">=", "18");
		let stmt =
			compiler.compile_select::<TestModel>("test_models", &[], Some(&q), &[], None, None);

		let (sql, values) = stmt.build(SqliteQueryBuilder);
		assert_eq!(sql, r#"SELECT * FROM "test_models" WHERE "age" >= ?"#);
		assert_eq!(
			values.0,
			vec![reinhardt_query::value::Value::BigInt(Some(18))]
		);
	}

	#[test]
	fn test_compile_select_binds_condition_value() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let payload = "Alice' OR 1=1 --";
		let q = Q::new("name", "=", payload);

		let stmt =
			compiler.compile_select::<TestModel>("test_models", &[], Some(&q), &[], None, None);
		let (sql, values) = stmt.build(SqliteQueryBuilder);

		assert_eq!(sql, r#"SELECT * FROM "test_models" WHERE "name" = ?"#);
		assert_eq!(
			values.0,
			vec![reinhardt_query::value::Value::String(Some(Box::new(
				payload.to_owned()
			)))]
		);
	}

	#[test]
	fn test_compile_select_rejects_invalid_operator() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let q = Q::new("name", "= ? OR 1=1 --", "Alice");

		let stmt =
			compiler.compile_select::<TestModel>("test_models", &[], Some(&q), &[], None, None);
		let (sql, values) = stmt.build(SqliteQueryBuilder);

		assert_eq!(sql, r#"SELECT * FROM "test_models" WHERE FALSE"#);
		assert_eq!(values.0, Vec::new());
	}

	#[test]
	fn test_compile_select_quotes_qualified_condition_field() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let q = Q::new("test_models.name", "=", "Alice");

		let stmt =
			compiler.compile_select::<TestModel>("test_models", &[], Some(&q), &[], None, None);
		let (sql, values) = stmt.build(SqliteQueryBuilder);

		assert_eq!(
			sql,
			r#"SELECT * FROM "test_models" WHERE "test_models"."name" = ?"#
		);
		assert_eq!(
			values.0,
			vec![reinhardt_query::value::Value::String(Some(Box::new(
				"Alice".to_owned()
			)))]
		);
	}

	#[test]
	fn test_compile_select_binds_negated_condition_value() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let payload = "Alice' OR 1=1 --";
		let q = Q::new("name", "=", payload).not();

		let stmt =
			compiler.compile_select::<TestModel>("test_models", &[], Some(&q), &[], None, None);
		let (sql, values) = stmt.build(SqliteQueryBuilder);

		assert_eq!(sql, r#"SELECT * FROM "test_models" WHERE NOT "name" = ?"#);
		assert_eq!(
			values.0,
			vec![reinhardt_query::value::Value::String(Some(Box::new(
				payload.to_owned()
			)))]
		);
	}

	#[test]
	fn test_compile_select_with_limit_offset() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let stmt = compiler.compile_select::<TestModel>(
			"test_models",
			&[],
			None,
			&["id"],
			Some(10),
			Some(20),
		);

		let sql = stmt.to_string(SqliteQueryBuilder);
		assert!(sql.contains("LIMIT"));
		assert!(sql.contains("OFFSET"));
		assert!(sql.contains("ORDER BY"));
	}

	#[test]
	fn test_compile_insert() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let stmt =
			compiler.compile_insert::<TestModel>("test_models", &["id", "name"], &["1", "'Alice'"]);

		let sql = stmt.to_string(SqliteQueryBuilder);
		assert!(sql.contains("INSERT"));
		assert!(sql.contains("test_models"));
		assert!(sql.contains("id"));
		assert!(sql.contains("name"));
	}

	#[test]
	fn test_compile_update() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let q = Q::new("id", "=", "1");
		let stmt = compiler.compile_update::<TestModel>(
			"test_models",
			&[("name", "'Bob'"), ("age", "25")],
			Some(&q),
		);

		let sql = stmt.to_string(SqliteQueryBuilder);
		assert!(sql.contains("UPDATE"));
		assert!(sql.contains("test_models"));
		assert!(sql.contains("SET"));
		assert!(sql.contains("WHERE"));
	}

	#[test]
	fn test_compile_delete() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let compiler = QueryCompiler::new(DatabaseDialect::SQLite);
		let q = Q::new("active", "=", "0");
		let stmt = compiler.compile_delete::<TestModel>("test_models", Some(&q));

		let sql = stmt.to_string(SqliteQueryBuilder);
		assert!(sql.contains("DELETE"));
		assert!(sql.contains("test_models"));
		assert!(sql.contains("WHERE"));
	}

	#[test]
	fn test_executable_query() {
		let query = ExecutableQuery::<TestModel>::new("SELECT * FROM test_models");
		assert_eq!(query.sql(), "SELECT * FROM test_models");
	}
}
