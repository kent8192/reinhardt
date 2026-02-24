//! SELECT statement builder
//!
//! This module provides the `SelectStatement` type for building SQL SELECT queries.

use crate::{
	expr::{Condition, ConditionHolder, IntoCondition, SimpleExpr},
	types::{
		ColumnRef, DynIden, IntoColumnRef, IntoIden, IntoTableRef, JoinExpr, JoinType, Order,
		OrderExpr, TableRef, WindowStatement,
	},
	value::{IntoValue, Value, Values},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// SELECT statement builder
///
/// This struct provides a fluent API for constructing SELECT queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let query = Query::select()
///     .column(Expr::col("id"))
///     .column(Expr::col("name"))
///     .from("users")
///     .and_where(Expr::col("active").eq(true))
///     .order_by("name", Order::Asc)
///     .limit(10);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SelectStatement {
	pub(crate) ctes: Vec<CommonTableExpr>,
	pub(crate) distinct: Option<SelectDistinct>,
	pub(crate) selects: Vec<SelectExpr>,
	pub(crate) from: Vec<TableRef>,
	pub(crate) join: Vec<JoinExpr>,
	pub(crate) r#where: ConditionHolder,
	pub(crate) groups: Vec<SimpleExpr>,
	pub(crate) having: ConditionHolder,
	pub(crate) unions: Vec<(UnionType, SelectStatement)>,
	pub(crate) orders: Vec<OrderExpr>,
	pub(crate) limit: Option<Value>,
	pub(crate) offset: Option<Value>,
	pub(crate) lock: Option<LockClause>,
	pub(crate) windows: Vec<(DynIden, WindowStatement)>,
}

/// Common Table Expression (CTE) for WITH clause
///
/// This represents a single CTE in a WITH clause.
#[derive(Debug, Clone)]
pub struct CommonTableExpr {
	/// CTE name (alias)
	pub(crate) name: DynIden,
	/// CTE query
	pub(crate) query: Box<SelectStatement>,
	/// Whether this is a RECURSIVE CTE
	pub(crate) recursive: bool,
}

/// List of distinct keywords that can be used in select statement
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SelectDistinct {
	/// SELECT ALL
	All,
	/// SELECT DISTINCT
	Distinct,
	/// SELECT DISTINCTROW (MySQL)
	DistinctRow,
	/// SELECT DISTINCT ON (PostgreSQL)
	DistinctOn(Vec<ColumnRef>),
}

/// Select expression used in select statement.
#[derive(Debug, Clone)]
pub struct SelectExpr {
	/// The expression to select.
	pub expr: SimpleExpr,
	/// Optional alias for the expression (AS clause).
	pub alias: Option<DynIden>,
}

/// List of lock types that can be used in select statement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LockType {
	/// FOR UPDATE
	Update,
	/// FOR NO KEY UPDATE (PostgreSQL)
	NoKeyUpdate,
	/// FOR SHARE
	Share,
	/// FOR KEY SHARE (PostgreSQL)
	KeyShare,
}

/// List of lock behavior can be used in select statement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LockBehavior {
	/// NOWAIT
	Nowait,
	/// SKIP LOCKED
	SkipLocked,
}

/// Lock clause for SELECT ... FOR UPDATE/SHARE
// NOTE: FOR UPDATE/SHARE は現在未実装のため、フィールドが未使用となっている
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LockClause {
	pub(crate) r#type: LockType,
	pub(crate) tables: Vec<TableRef>,
	pub(crate) behavior: Option<LockBehavior>,
}

/// List of union types that can be used in union clause
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnionType {
	/// INTERSECT
	Intersect,
	/// UNION
	Distinct,
	/// EXCEPT
	Except,
	/// UNION ALL
	All,
}

impl<T> From<T> for SelectExpr
where
	T: Into<SimpleExpr>,
{
	fn from(expr: T) -> Self {
		SelectExpr {
			expr: expr.into(),
			alias: None,
		}
	}
}

impl SelectStatement {
	/// Create a new SELECT statement
	pub fn new() -> Self {
		Self::default()
	}

	/// Take the ownership of data in the current [`SelectStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			ctes: std::mem::take(&mut self.ctes),
			distinct: self.distinct.take(),
			selects: std::mem::take(&mut self.selects),
			from: std::mem::take(&mut self.from),
			join: std::mem::take(&mut self.join),
			r#where: std::mem::replace(&mut self.r#where, ConditionHolder::new()),
			groups: std::mem::take(&mut self.groups),
			having: std::mem::replace(&mut self.having, ConditionHolder::new()),
			unions: std::mem::take(&mut self.unions),
			orders: std::mem::take(&mut self.orders),
			limit: self.limit.take(),
			offset: self.offset.take(),
			lock: self.lock.take(),
			windows: std::mem::take(&mut self.windows),
		}
	}

	// Column selection methods

	/// Add a column to the SELECT clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .column("id")
	///     .column("name")
	///     .from("users");
	/// ```
	pub fn column<C>(&mut self, col: C) -> &mut Self
	where
		C: IntoColumnRef,
	{
		self.selects.push(SelectExpr {
			expr: SimpleExpr::Column(col.into_column_ref()),
			alias: None,
		});
		self
	}

	/// Add multiple columns to the SELECT clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .columns(["id", "name", "email"])
	///     .from("users");
	/// ```
	pub fn columns<I, C>(&mut self, cols: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoColumnRef,
	{
		for col in cols {
			self.column(col);
		}
		self
	}

	/// Add an expression to the SELECT clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .expr(Expr::col("price").mul(Expr::col("quantity")))
	///     .from("orders");
	/// ```
	pub fn expr<E>(&mut self, expr: E) -> &mut Self
	where
		E: Into<SimpleExpr>,
	{
		self.selects.push(SelectExpr {
			expr: expr.into(),
			alias: None,
		});
		self
	}

	/// Add an expression with an alias to the SELECT clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .expr_as(Expr::col("price").mul(Expr::col("quantity")), "total")
	///     .from("orders");
	/// ```
	pub fn expr_as<E, A>(&mut self, expr: E, alias: A) -> &mut Self
	where
		E: Into<SimpleExpr>,
		A: IntoIden,
	{
		self.selects.push(SelectExpr {
			expr: expr.into(),
			alias: Some(alias.into_iden()),
		});
		self
	}

	// FROM clause methods

	/// Add a table to the FROM clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .column("id")
	///     .from("users");
	/// ```
	pub fn from<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.from.push(tbl.into_table_ref());
		self
	}

	/// Add a table with alias to the FROM clause
	///
	/// Equivalent to `FROM table AS alias`.
	pub fn from_as<T, A>(&mut self, tbl: T, alias: A) -> &mut Self
	where
		T: IntoIden,
		A: IntoIden,
	{
		self.from
			.push(TableRef::TableAlias(tbl.into_iden(), alias.into_iden()));
		self
	}

	/// Add a subquery to the FROM clause
	///
	/// Equivalent to `FROM (SELECT ...) AS alias`.
	pub fn from_subquery(&mut self, query: SelectStatement, alias: impl IntoIden) -> &mut Self {
		self.from
			.push(TableRef::SubQuery(Box::new(query), alias.into_iden()));
		self
	}

	/// Clear all column selections
	pub fn clear_selects(&mut self) -> &mut Self {
		self.selects.clear();
		self
	}

	// JOIN clause methods

	/// Add a JOIN clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .from("users")
	///     .join(
	///         JoinType::InnerJoin,
	///         "orders",
	///         Expr::col(("users", "id")).equals(("orders", "user_id"))
	///     );
	/// ```
	pub fn join<T, C>(&mut self, join: JoinType, tbl: T, condition: C) -> &mut Self
	where
		T: IntoTableRef,
		C: IntoCondition,
	{
		self.join.push(JoinExpr {
			join,
			table: tbl.into_table_ref(),
			on: Some(crate::types::JoinOn::Condition(condition.into_condition())),
		});
		self
	}

	/// Add a LEFT JOIN clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .from("users")
	///     .left_join(
	///         "orders",
	///         Expr::col(("users", "id")).equals(("orders", "user_id"))
	///     );
	/// ```
	pub fn left_join<T, C>(&mut self, tbl: T, condition: C) -> &mut Self
	where
		T: IntoTableRef,
		C: IntoCondition,
	{
		self.join(JoinType::LeftJoin, tbl, condition)
	}

	/// Add a RIGHT JOIN clause
	pub fn right_join<T, C>(&mut self, tbl: T, condition: C) -> &mut Self
	where
		T: IntoTableRef,
		C: IntoCondition,
	{
		self.join(JoinType::RightJoin, tbl, condition)
	}

	/// Add a FULL OUTER JOIN clause
	pub fn full_outer_join<T, C>(&mut self, tbl: T, condition: C) -> &mut Self
	where
		T: IntoTableRef,
		C: IntoCondition,
	{
		self.join(JoinType::FullOuterJoin, tbl, condition)
	}

	/// Add an INNER JOIN clause
	pub fn inner_join<T, C>(&mut self, tbl: T, condition: C) -> &mut Self
	where
		T: IntoTableRef,
		C: IntoCondition,
	{
		self.join(JoinType::InnerJoin, tbl, condition)
	}

	/// Add a CROSS JOIN clause
	pub fn cross_join<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.join.push(JoinExpr {
			join: JoinType::CrossJoin,
			table: tbl.into_table_ref(),
			on: None,
		});
		self
	}

	// WHERE clause methods

	/// Add a condition to the WHERE clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .from("users")
	///     .and_where(Expr::col("active").eq(true));
	/// ```
	pub fn and_where<C>(&mut self, condition: C) -> &mut Self
	where
		C: IntoCondition,
	{
		self.r#where.add_and(condition);
		self
	}

	/// Add a condition to the WHERE clause using Condition
	pub fn cond_where(&mut self, condition: Condition) -> &mut Self {
		self.r#where.add_and(condition);
		self
	}

	// GROUP BY clause methods

	/// Add a GROUP BY clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .column("category")
	///     .expr_as(Expr::count("*"), "count")
	///     .from("products")
	///     .group_by("category");
	/// ```
	pub fn group_by<C>(&mut self, col: C) -> &mut Self
	where
		C: IntoColumnRef,
	{
		self.groups.push(SimpleExpr::Column(col.into_column_ref()));
		self
	}

	/// Add a column to the GROUP BY clause (alias for `group_by`)
	pub fn group_by_col<C>(&mut self, col: C) -> &mut Self
	where
		C: IntoColumnRef,
	{
		self.group_by(col)
	}

	/// Add multiple GROUP BY columns
	pub fn group_by_columns<I, C>(&mut self, cols: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoColumnRef,
	{
		for col in cols {
			self.group_by(col);
		}
		self
	}

	// HAVING clause methods

	/// Add a condition to the HAVING clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .column("category")
	///     .expr_as(Expr::count("*"), "count")
	///     .from("products")
	///     .group_by("category")
	///     .and_having(Expr::count("*").gt(5));
	/// ```
	pub fn and_having<C>(&mut self, condition: C) -> &mut Self
	where
		C: IntoCondition,
	{
		self.having.add_and(condition);
		self
	}

	/// Add a condition to the HAVING clause using Condition
	pub fn cond_having(&mut self, condition: Condition) -> &mut Self {
		self.having.add_and(condition);
		self
	}

	// ORDER BY clause methods

	/// Add an ORDER BY clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .from("users")
	///     .order_by("name", Order::Asc)
	///     .order_by("created_at", Order::Desc);
	/// ```
	pub fn order_by<C>(&mut self, col: C, order: Order) -> &mut Self
	where
		C: IntoColumnRef,
	{
		use crate::types::OrderExprKind;
		self.orders.push(OrderExpr {
			expr: OrderExprKind::Expr(Box::new(SimpleExpr::Column(col.into_column_ref()))),
			order,
			nulls: None,
		});
		self
	}

	/// Add an ORDER BY clause with expression
	pub fn order_by_expr<E>(&mut self, expr: E, order: Order) -> &mut Self
	where
		E: Into<SimpleExpr>,
	{
		use crate::types::OrderExprKind;
		self.orders.push(OrderExpr {
			expr: OrderExprKind::Expr(Box::new(expr.into())),
			order,
			nulls: None,
		});
		self
	}

	// LIMIT and OFFSET methods

	/// Set the LIMIT clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .from("users")
	///     .limit(10);
	/// ```
	pub fn limit<V>(&mut self, limit: V) -> &mut Self
	where
		V: IntoValue,
	{
		self.limit = Some(limit.into_value());
		self
	}

	/// Set the OFFSET clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .from("users")
	///     .limit(10)
	///     .offset(20);
	/// ```
	pub fn offset<V>(&mut self, offset: V) -> &mut Self
	where
		V: IntoValue,
	{
		self.offset = Some(offset.into_value());
		self
	}

	// DISTINCT methods

	/// Set DISTINCT
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .distinct()
	///     .column("category")
	///     .from("products");
	/// ```
	pub fn distinct(&mut self) -> &mut Self {
		self.distinct = Some(SelectDistinct::Distinct);
		self
	}

	/// Set DISTINCT ON (PostgreSQL only)
	pub fn distinct_on<I, C>(&mut self, cols: I) -> &mut Self
	where
		I: IntoIterator<Item = C>,
		C: IntoColumnRef,
	{
		let cols: Vec<ColumnRef> = cols.into_iter().map(|c| c.into_column_ref()).collect();
		self.distinct = Some(SelectDistinct::DistinctOn(cols));
		self
	}

	// UNION methods

	/// Add a UNION clause
	pub fn union(&mut self, query: SelectStatement) -> &mut Self {
		self.unions.push((UnionType::Distinct, query));
		self
	}

	/// Add a UNION ALL clause
	pub fn union_all(&mut self, query: SelectStatement) -> &mut Self {
		self.unions.push((UnionType::All, query));
		self
	}

	/// Add an INTERSECT clause
	pub fn intersect(&mut self, query: SelectStatement) -> &mut Self {
		self.unions.push((UnionType::Intersect, query));
		self
	}

	/// Add an EXCEPT clause
	pub fn except(&mut self, query: SelectStatement) -> &mut Self {
		self.unions.push((UnionType::Except, query));
		self
	}

	// WITH (CTE) methods

	/// Add a Common Table Expression (CTE) to the WITH clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let cte = Query::select()
	///     .column("id")
	///     .column("name")
	///     .from("users")
	///     .and_where(Expr::col("active").eq(true));
	///
	/// let query = Query::select()
	///     .with_cte("active_users", cte)
	///     .column("*")
	///     .from("active_users");
	/// ```
	pub fn with_cte<N>(&mut self, name: N, query: SelectStatement) -> &mut Self
	where
		N: IntoIden,
	{
		self.ctes.push(CommonTableExpr {
			name: name.into_iden(),
			query: Box::new(query),
			recursive: false,
		});
		self
	}

	/// Add a RECURSIVE Common Table Expression (CTE) to the WITH clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Recursive CTE for hierarchical data
	/// let cte = Query::select()
	///     .column("id")
	///     .column("parent_id")
	///     .column("name")
	///     .from("categories")
	///     .and_where(Expr::col("parent_id").is_null())
	///     .union_all(
	///         Query::select()
	///             .column(Expr::col(("c", "id")))
	///             .column(Expr::col(("c", "parent_id")))
	///             .column(Expr::col(("c", "name")))
	///             .from_as("categories", "c")
	///             .join(
	///                 JoinType::InnerJoin,
	///                 "category_tree",
	///                 Expr::col(("c", "parent_id")).eq(Expr::col(("category_tree", "id")))
	///             )
	///     );
	///
	/// let query = Query::select()
	///     .with_recursive_cte("category_tree", cte)
	///     .column("*")
	///     .from("category_tree");
	/// ```
	pub fn with_recursive_cte<N>(&mut self, name: N, query: SelectStatement) -> &mut Self
	where
		N: IntoIden,
	{
		self.ctes.push(CommonTableExpr {
			name: name.into_iden(),
			query: Box::new(query),
			recursive: true,
		});
		self
	}

	// WINDOW methods

	/// Add a named window specification to the WINDOW clause
	///
	/// Named windows can be referenced by window functions using `OVER window_name`.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![Expr::col("department_id").into_simple_expr()],
	///     order_by: vec![OrderExpr {
	///         expr: Expr::col("salary").into_simple_expr(),
	///         order: Order::Desc,
	///         nulls: None,
	///     }],
	///     frame: None,
	/// };
	///
	/// let query = Query::select()
	///     .column("name")
	///     .expr_as(Expr::row_number().over_named("w"), "rank")
	///     .from("employees")
	///     .window_as("w", window);
	/// ```
	pub fn window_as<T>(&mut self, name: T, window: WindowStatement) -> &mut Self
	where
		T: IntoIden,
	{
		self.windows.push((name.into_iden(), window));
		self
	}

	// LOCK methods

	/// Set FOR UPDATE lock
	pub fn lock(&mut self, lock_type: LockType) -> &mut Self {
		self.lock = Some(LockClause {
			r#type: lock_type,
			tables: Vec::new(),
			behavior: None,
		});
		self
	}

	/// Set FOR UPDATE lock
	pub fn lock_exclusive(&mut self) -> &mut Self {
		self.lock(LockType::Update)
	}

	/// Set FOR SHARE lock
	pub fn lock_shared(&mut self) -> &mut Self {
		self.lock(LockType::Share)
	}

	// Utility methods

	/// Apply a function conditionally
	pub fn apply_if<T, F>(&mut self, val: Option<T>, func: F) -> &mut Self
	where
		F: FnOnce(&mut Self, T),
	{
		if let Some(val) = val {
			func(self, val);
		}
		self
	}

	/// Conditional execution
	pub fn conditions<T, F>(&mut self, b: bool, if_true: T, if_false: F) -> &mut Self
	where
		T: FnOnce(&mut Self),
		F: FnOnce(&mut Self),
	{
		if b {
			if_true(self)
		} else {
			if_false(self)
		}
		self
	}
}

impl QueryStatementBuilder for SelectStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, Values) {
		use crate::backend::{
			MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder,
		};
		use std::any::Any;

		let any_builder = query_builder as &dyn Any;

		if let Some(pg) = any_builder.downcast_ref::<PostgresQueryBuilder>() {
			return pg.build_select(self);
		}

		if let Some(mysql) = any_builder.downcast_ref::<MySqlQueryBuilder>() {
			return mysql.build_select(self);
		}

		if let Some(sqlite) = any_builder.downcast_ref::<SqliteQueryBuilder>() {
			return sqlite.build_select(self);
		}

		panic!(
			"Unsupported query builder type. Use PostgresQueryBuilder, MySqlQueryBuilder, or SqliteQueryBuilder."
		);
	}
}

impl QueryStatementWriter for SelectStatement {}
