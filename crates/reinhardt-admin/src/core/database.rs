//! Database integration for admin operations
//!
//! This module provides database access layer for admin CRUD operations,
//! integrating with reinhardt-orm's QuerySet API.

use crate::types::{AdminError, AdminResult};
use async_trait::async_trait;
use reinhardt_db::orm::execution::convert_values;
use reinhardt_db::orm::{
	DatabaseConnection, Filter, FilterCondition, FilterOperator, FilterValue, Model,
};
use reinhardt_di::{DiResult, Injectable, InjectionContext};
use reinhardt_query::prelude::{
	Alias, CaseStatement, ColumnRef, Condition, Expr, ExprTrait, IntoValue, Order,
	PostgresQueryBuilder, Query, QueryStatementBuilder, SimpleExpr, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Dummy record type for admin panel CRUD operations
///
/// This type exists solely to satisfy the `<M: Model>` generic constraint
/// in `AdminDatabase` methods. The admin panel operates on dynamic data
/// (serde_json::Value), not statically-typed models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminRecord {
	pub id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AdminRecordFields {
	pub id: reinhardt_db::orm::query_fields::Field<AdminRecord, Option<i64>>,
}

impl Default for AdminRecordFields {
	fn default() -> Self {
		Self::new()
	}
}

impl AdminRecordFields {
	pub fn new() -> Self {
		Self {
			id: reinhardt_db::orm::query_fields::Field::new(vec!["id".to_string()]),
		}
	}
}

impl reinhardt_db::orm::FieldSelector for AdminRecordFields {
	fn with_alias(mut self, alias: &str) -> Self {
		self.id = self.id.with_alias(alias);
		self
	}
}

impl Model for AdminRecord {
	type PrimaryKey = i64;
	type Fields = AdminRecordFields;

	fn table_name() -> &'static str {
		"admin_records"
	}

	fn new_fields() -> Self::Fields {
		AdminRecordFields::new()
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, pk: Self::PrimaryKey) {
		self.id = Some(pk);
	}
}

/// Convert FilterValue to Value
#[doc(hidden)]
pub fn filter_value_to_sea_value(v: &FilterValue) -> Value {
	match v {
		FilterValue::String(s) => s.clone().into(),
		FilterValue::Integer(i) | FilterValue::Int(i) => (*i).into(),
		FilterValue::Float(f) => (*f).into(),
		FilterValue::Boolean(b) | FilterValue::Bool(b) => (*b).into(),
		FilterValue::Null => Value::Int(None),
		FilterValue::Array(_) => Value::String(None),
		FilterValue::FieldRef(f) => {
			// FieldRef generates column reference, not scalar value.
			// For Value context, return field name as string.
			// Proper handling is in build_single_filter_expr().
			Value::String(Some(Box::new(f.field.clone())))
		}
		FilterValue::Expression(expr) => {
			// Expression generates SQL expression, not scalar value.
			// For Value context, return SQL string representation.
			// Proper handling is in build_single_filter_expr().
			Value::String(Some(Box::new(expr.to_sql())))
		}
		FilterValue::OuterRef(outer) => {
			// OuterRef generates outer query reference, not scalar value.
			// For Value context, return field name as string.
			// Proper handling is in build_single_filter_expr().
			Value::String(Some(Box::new(outer.field.clone())))
		}
	}
}

/// Convert an annotation `AnnotationValue` to a safe SeaQuery `SimpleExpr`.
///
/// Uses type-safe SeaQuery API for field references and literal values
/// instead of raw SQL string interpolation, preventing SQL injection.
fn annotation_value_to_safe_expr(
	val: &reinhardt_db::orm::annotation::AnnotationValue,
) -> SimpleExpr {
	use reinhardt_db::orm::annotation::AnnotationValue;

	match val {
		AnnotationValue::Value(v) => {
			use reinhardt_db::orm::annotation::Value as AnnotValue;
			match v {
				AnnotValue::String(s) => Expr::val(s.as_str()).into(),
				AnnotValue::Int(i) => Expr::val(*i).into(),
				AnnotValue::Float(f) => Expr::val(*f).into(),
				AnnotValue::Bool(b) => Expr::val(*b).into(),
				AnnotValue::Null => Expr::val(Option::<String>::None).into(),
			}
		}
		AnnotationValue::Field(f) => Expr::col(Alias::new(&f.field)).into(),
		AnnotationValue::Expression(e) => annotation_expr_to_safe_expr(e),
		AnnotationValue::Aggregate(a) => aggregate_to_safe_expr(a),
		// Subquery and PostgreSQL-specific aggregation types produce SQL
		// from internally constructed ORM queries, not from user HTTP input.
		// Their SQL output is safe because it's built through type-safe ORM APIs.
		AnnotationValue::Subquery(_)
		| AnnotationValue::ArrayAgg(_)
		| AnnotationValue::StringAgg(_)
		| AnnotationValue::JsonbAgg(_)
		| AnnotationValue::JsonbBuildObject(_)
		| AnnotationValue::TsRank(_) => Expr::cust(val.to_sql()).into(),
	}
}

/// Convert an `Aggregate` to a safe SeaQuery `SimpleExpr`.
///
/// Uses parameterized function templates with quoted column identifiers
/// instead of raw SQL string interpolation, preventing SQL injection
/// through field name manipulation.
fn aggregate_to_safe_expr(agg: &reinhardt_db::orm::aggregation::Aggregate) -> SimpleExpr {
	use reinhardt_db::orm::aggregation::AggregateFunc;

	let func_name = match agg.func {
		AggregateFunc::Count | AggregateFunc::CountDistinct => "COUNT",
		AggregateFunc::Sum => "SUM",
		AggregateFunc::Avg => "AVG",
		AggregateFunc::Max => "MAX",
		AggregateFunc::Min => "MIN",
	};

	if let Some(field) = &agg.field {
		let col_expr: SimpleExpr = Expr::col(Alias::new(field)).into();
		let is_distinct = agg.distinct || matches!(agg.func, AggregateFunc::CountDistinct);
		if is_distinct {
			Expr::cust_with_values(format!("{func_name}(DISTINCT ?)"), [col_expr]).into()
		} else {
			Expr::cust_with_values(format!("{func_name}(?)"), [col_expr]).into()
		}
	} else {
		// COUNT(*) case - static SQL template, no user input
		Expr::cust(format!("{func_name}(*)")).into()
	}
}

/// Convert an annotation `Expression` to a safe SeaQuery `SimpleExpr`.
///
/// Recursively converts all expression types using type-safe SeaQuery API
/// for field references and values, preventing SQL injection through
/// value manipulation in expression trees.
fn annotation_expr_to_safe_expr(expr: &reinhardt_db::orm::annotation::Expression) -> SimpleExpr {
	use reinhardt_db::orm::annotation::Expression as AnnotExpr;

	match expr {
		AnnotExpr::Add(left, right) => {
			let left_expr = annotation_value_to_safe_expr(left);
			let right_expr = annotation_value_to_safe_expr(right);
			Expr::cust_with_values("(? + ?)", [left_expr, right_expr]).into()
		}
		AnnotExpr::Subtract(left, right) => {
			let left_expr = annotation_value_to_safe_expr(left);
			let right_expr = annotation_value_to_safe_expr(right);
			Expr::cust_with_values("(? - ?)", [left_expr, right_expr]).into()
		}
		AnnotExpr::Multiply(left, right) => {
			let left_expr = annotation_value_to_safe_expr(left);
			let right_expr = annotation_value_to_safe_expr(right);
			Expr::cust_with_values("(? * ?)", [left_expr, right_expr]).into()
		}
		AnnotExpr::Divide(left, right) => {
			let left_expr = annotation_value_to_safe_expr(left);
			let right_expr = annotation_value_to_safe_expr(right);
			Expr::cust_with_values("(? / ?)", [left_expr, right_expr]).into()
		}
		AnnotExpr::Case { whens, default } => {
			let mut case = CaseStatement::new();
			for when in whens {
				// Q conditions are constructed internally by the ORM's query builder,
				// not from user HTTP input. The THEN values are safely converted
				// through annotation_value_to_safe_expr.
				let cond_expr: SimpleExpr = Expr::cust(when.condition.to_sql()).into();
				let then_expr = annotation_value_to_safe_expr(&when.then);
				case = case.when(cond_expr, then_expr);
			}
			if let Some(default_val) = default {
				case = case.else_result(annotation_value_to_safe_expr(default_val));
			}
			SimpleExpr::from(case)
		}
		AnnotExpr::Coalesce(values) => {
			let exprs: Vec<SimpleExpr> = values.iter().map(annotation_value_to_safe_expr).collect();
			if exprs.is_empty() {
				Expr::val(Option::<String>::None).into()
			} else {
				let placeholders = vec!["?"; exprs.len()].join(", ");
				Expr::cust_with_values(format!("COALESCE({placeholders})"), exprs).into()
			}
		}
	}
}

/// Escape SQL LIKE wildcard characters in user input
fn escape_like_pattern(input: &str) -> String {
	input
		.replace('\\', "\\\\")
		.replace('%', "\\%")
		.replace('_', "\\_")
}

/// Build a SimpleExpr from a single Filter
#[doc(hidden)]
pub fn build_single_filter_expr(filter: &Filter) -> Option<SimpleExpr> {
	let col = Expr::col(Alias::new(&filter.field));

	let expr = match (&filter.operator, &filter.value) {
		// Null handling (must come before generic patterns)
		(FilterOperator::Eq, FilterValue::Null) => col.is_null(),
		(FilterOperator::Ne, FilterValue::Null) => col.is_not_null(),

		// FieldRef: Column-to-column comparisons
		(FilterOperator::Eq, FilterValue::FieldRef(f)) => col.eq(Expr::col(Alias::new(&f.field))),
		(FilterOperator::Ne, FilterValue::FieldRef(f)) => col.ne(Expr::col(Alias::new(&f.field))),
		(FilterOperator::Gt, FilterValue::FieldRef(f)) => col.gt(Expr::col(Alias::new(&f.field))),
		(FilterOperator::Gte, FilterValue::FieldRef(f)) => col.gte(Expr::col(Alias::new(&f.field))),
		(FilterOperator::Lt, FilterValue::FieldRef(f)) => col.lt(Expr::col(Alias::new(&f.field))),
		(FilterOperator::Lte, FilterValue::FieldRef(f)) => col.lte(Expr::col(Alias::new(&f.field))),

		// OuterRef: Correlated subquery references (use type-safe column API)
		(FilterOperator::Eq, FilterValue::OuterRef(outer)) => {
			col.eq(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Ne, FilterValue::OuterRef(outer)) => {
			col.ne(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Gt, FilterValue::OuterRef(outer)) => {
			col.gt(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Gte, FilterValue::OuterRef(outer)) => {
			col.gte(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Lt, FilterValue::OuterRef(outer)) => {
			col.lt(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Lte, FilterValue::OuterRef(outer)) => {
			col.lte(Expr::col(Alias::new(&outer.field)))
		}

		// Expression: Arithmetic expressions (validate field names before building SQL)
		(FilterOperator::Eq, FilterValue::Expression(expr)) => {
			col.eq(annotation_expr_to_safe_expr(expr))
		}
		(FilterOperator::Ne, FilterValue::Expression(expr)) => {
			col.ne(annotation_expr_to_safe_expr(expr))
		}
		(FilterOperator::Gt, FilterValue::Expression(expr)) => {
			col.gt(annotation_expr_to_safe_expr(expr))
		}
		(FilterOperator::Gte, FilterValue::Expression(expr)) => {
			col.gte(annotation_expr_to_safe_expr(expr))
		}
		(FilterOperator::Lt, FilterValue::Expression(expr)) => {
			col.lt(annotation_expr_to_safe_expr(expr))
		}
		(FilterOperator::Lte, FilterValue::Expression(expr)) => {
			col.lte(annotation_expr_to_safe_expr(expr))
		}

		// Generic scalar value patterns
		(FilterOperator::Eq, v) => col.eq(filter_value_to_sea_value(v)),
		(FilterOperator::Ne, v) => col.ne(filter_value_to_sea_value(v)),
		(FilterOperator::Gt, v) => col.gt(filter_value_to_sea_value(v)),
		(FilterOperator::Gte, v) => col.gte(filter_value_to_sea_value(v)),
		(FilterOperator::Lt, v) => col.lt(filter_value_to_sea_value(v)),
		(FilterOperator::Lte, v) => col.lte(filter_value_to_sea_value(v)),

		// String-specific operators
		(FilterOperator::Contains, FilterValue::String(s)) => {
			col.like(format!("%{}%", escape_like_pattern(s)))
		}
		(FilterOperator::StartsWith, FilterValue::String(s)) => {
			col.like(format!("{}%", escape_like_pattern(s)))
		}
		(FilterOperator::EndsWith, FilterValue::String(s)) => {
			col.like(format!("%{}", escape_like_pattern(s)))
		}
		(FilterOperator::In, FilterValue::String(s)) => {
			let values: Vec<Value> = s.split(',').map(|v| v.trim().into_value()).collect();
			col.is_in(values)
		}
		(FilterOperator::NotIn, FilterValue::String(s)) => {
			let values: Vec<Value> = s.split(',').map(|v| v.trim().into_value()).collect();
			col.is_not_in(values)
		}

		// Skip unsupported combinations
		_ => return None,
	};

	Some(expr)
}

/// Build Condition from filters (AND logic only)
#[doc(hidden)]
pub fn build_filter_condition(filters: &[Filter]) -> Option<Condition> {
	if filters.is_empty() {
		return None;
	}

	let mut condition = Condition::all();

	for filter in filters {
		if let Some(expr) = build_single_filter_expr(filter) {
			condition = condition.add(expr);
		}
	}

	Some(condition)
}

/// Maximum recursion depth for filter conditions to prevent stack overflow
#[doc(hidden)]
pub const MAX_FILTER_DEPTH: usize = 100;

/// Build Condition from FilterCondition (supports AND/OR logic)
///
/// This function recursively processes FilterCondition to build complex
/// query conditions with nested AND/OR logic.
///
/// # Stack Overflow Protection
///
/// To prevent stack overflow with deeply nested filter conditions, this function
/// limits recursion depth to `MAX_FILTER_DEPTH` (100 levels). If the depth limit
/// is exceeded, the function returns `None`.
#[doc(hidden)]
pub fn build_composite_filter_condition(filter_condition: &FilterCondition) -> Option<Condition> {
	build_composite_filter_condition_with_depth(filter_condition, 0)
}

/// Internal helper for building composite filter conditions with depth tracking
#[doc(hidden)]
pub fn build_composite_filter_condition_with_depth(
	filter_condition: &FilterCondition,
	depth: usize,
) -> Option<Condition> {
	// Prevent stack overflow by limiting recursion depth
	if depth >= MAX_FILTER_DEPTH {
		return None;
	}

	match filter_condition {
		FilterCondition::Single(filter) => {
			build_single_filter_expr(filter).map(|expr| Condition::all().add(expr))
		}
		FilterCondition::And(conditions) => {
			if conditions.is_empty() {
				return None;
			}
			let mut and_condition = Condition::all();
			for cond in conditions {
				if let Some(sub_cond) = build_composite_filter_condition_with_depth(cond, depth + 1)
				{
					and_condition = and_condition.add(sub_cond);
				}
			}
			Some(and_condition)
		}
		FilterCondition::Or(conditions) => {
			if conditions.is_empty() {
				return None;
			}
			let mut or_condition = Condition::any();
			for cond in conditions {
				if let Some(sub_cond) = build_composite_filter_condition_with_depth(cond, depth + 1)
				{
					or_condition = or_condition.add(sub_cond);
				}
			}
			Some(or_condition)
		}
		FilterCondition::Not(inner) => {
			build_composite_filter_condition_with_depth(inner, depth + 1)
				.map(|inner_cond| inner_cond.not())
		}
	}
}

/// Admin database interface
///
/// Provides CRUD operations for admin panel, leveraging reinhardt-orm.
///
/// # Examples
///
/// ```
/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
/// use reinhardt_db::orm::DatabaseConnection;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
/// let db = AdminDatabase::new(conn);
///
/// // List items with filters
/// let items = db.list::<AdminRecord>("admin_records", vec![], 0, 50).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct AdminDatabase {
	connection: Arc<DatabaseConnection>,
}

impl AdminDatabase {
	/// Create a new admin database interface
	///
	/// This method accepts a DatabaseConnection directly without requiring `Arc` wrapping.
	/// The `Arc` wrapping is handled internally for you.
	pub fn new(connection: DatabaseConnection) -> Self {
		Self {
			connection: Arc::new(connection),
		}
	}

	/// Create a new admin database interface from an Arc-wrapped connection
	///
	/// This is provided for cases where you already have an `Arc<DatabaseConnection>`.
	/// In most cases, you should use `new()` instead.
	pub fn from_arc(connection: Arc<DatabaseConnection>) -> Self {
		Self { connection }
	}

	/// Get a reference to the underlying database connection
	pub fn connection(&self) -> &DatabaseConnection {
		&self.connection
	}

	/// Get a cloned Arc of the connection (for cases where you need ownership)
	///
	/// In most cases, you should use `connection()` instead to get a reference.
	pub fn connection_arc(&self) -> Arc<DatabaseConnection> {
		Arc::clone(&self.connection)
	}

	/// List items with filters, ordering, and pagination
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::orm::{DatabaseConnection, Filter, FilterOperator, FilterValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let db = AdminDatabase::new(conn);
	///
	/// let filters = vec![
	///     Filter::new("is_active".to_string(), FilterOperator::Eq, FilterValue::Boolean(true))
	/// ];
	///
	/// let items = db.list::<AdminRecord>("admin_records", filters, 0, 50).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn list<M: Model>(
		&self,
		table_name: &str,
		filters: Vec<Filter>,
		offset: u64,
		limit: u64,
	) -> AdminResult<Vec<HashMap<String, serde_json::Value>>> {
		// SELECT * is intentional: admin panel operates on dynamic schemas where
		// the column set is not known at compile time. Each ModelAdmin defines
		// list_display fields, and column filtering is applied at the application
		// layer after fetching all columns.
		let mut query = Query::select()
			.from(Alias::new(table_name))
			.column(ColumnRef::Asterisk)
			.to_owned();

		// Apply filters using build_filter_condition helper
		if let Some(condition) = build_filter_condition(&filters) {
			query.cond_where(condition);
		}

		// Apply pagination
		query.limit(limit).offset(offset);

		// Execute query
		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let rows = self
			.connection
			.query(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Convert QueryRow to HashMap
		Ok(rows
			.into_iter()
			.filter_map(|row| {
				// row.data is already a serde_json::Value, typically an Object
				if let serde_json::Value::Object(map) = row.data {
					Some(
						map.into_iter()
							.collect::<HashMap<String, serde_json::Value>>(),
					)
				} else {
					None
				}
			})
			.collect())
	}

	/// List items with composite filter conditions (supports AND/OR logic)
	///
	/// This method supports complex filter conditions using FilterCondition,
	/// which allows building nested AND/OR queries.
	///
	/// # Arguments
	///
	/// * `table_name` - The name of the table to query
	/// * `filter_condition` - Optional composite filter condition (AND/OR logic)
	/// * `additional_filters` - Additional simple filters to AND with the condition
	/// * `sort_by` - Optional sort field (prefix with "-" for descending, e.g., "created_at" or "-created_at")
	/// * `offset` - Number of items to skip for pagination
	/// * `limit` - Maximum number of items to return
	pub async fn list_with_condition<M: Model>(
		&self,
		table_name: &str,
		filter_condition: Option<&FilterCondition>,
		additional_filters: Vec<Filter>,
		sort_by: Option<&str>,
		offset: u64,
		limit: u64,
	) -> AdminResult<Vec<HashMap<String, serde_json::Value>>> {
		// SELECT * is intentional: admin panel operates on dynamic schemas where
		// the column set is not known at compile time. Each ModelAdmin defines
		// list_display fields, and column filtering is applied at the application
		// layer after fetching all columns.
		let mut query = Query::select()
			.from(Alias::new(table_name))
			.column(ColumnRef::Asterisk)
			.to_owned();

		// Build combined condition
		let mut combined = Condition::all();

		// Add composite filter condition (e.g., OR search across fields)
		if let Some(fc) = filter_condition
			&& let Some(cond) = build_composite_filter_condition(fc)
		{
			combined = combined.add(cond);
		}

		// Add simple filters (AND logic)
		if let Some(simple_cond) = build_filter_condition(&additional_filters) {
			combined = combined.add(simple_cond);
		}

		// Only add condition if we have actual filters
		if !additional_filters.is_empty() || filter_condition.is_some() {
			query.cond_where(combined);
		}

		// Apply sorting (if specified)
		if let Some(sort_str) = sort_by {
			let (field, is_desc) = if let Some(stripped) = sort_str.strip_prefix('-') {
				(stripped, true)
			} else {
				(sort_str, false)
			};

			let col = Alias::new(field);
			if is_desc {
				query.order_by(col, Order::Desc);
			} else {
				query.order_by(col, Order::Asc);
			}
		}

		// Apply pagination
		query.limit(limit).offset(offset);

		// Execute query
		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let rows = self
			.connection
			.query(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Convert QueryRow to HashMap
		Ok(rows
			.into_iter()
			.filter_map(|row| {
				if let serde_json::Value::Object(map) = row.data {
					Some(
						map.into_iter()
							.collect::<HashMap<String, serde_json::Value>>(),
					)
				} else {
					None
				}
			})
			.collect())
	}

	/// Count items with composite filter conditions (supports AND/OR logic)
	///
	/// # Arguments
	///
	/// * `table_name` - The name of the table to query
	/// * `filter_condition` - Optional composite filter condition (AND/OR logic)
	/// * `additional_filters` - Additional simple filters to AND with the condition
	pub async fn count_with_condition<M: Model>(
		&self,
		table_name: &str,
		filter_condition: Option<&FilterCondition>,
		additional_filters: Vec<Filter>,
	) -> AdminResult<u64> {
		let mut query = Query::select()
			.from(Alias::new(table_name))
			.expr(Expr::cust("COUNT(*) AS count"))
			.to_owned();

		// Build combined condition
		let mut combined = Condition::all();

		// Add composite filter condition
		if let Some(fc) = filter_condition
			&& let Some(cond) = build_composite_filter_condition(fc)
		{
			combined = combined.add(cond);
		}

		// Add simple filters
		if let Some(simple_cond) = build_filter_condition(&additional_filters) {
			combined = combined.add(simple_cond);
		}

		// Only add condition if we have actual filters
		if !additional_filters.is_empty() || filter_condition.is_some() {
			query.cond_where(combined);
		}

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_one(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Extract count from result, propagating errors for unexpected formats
		let count = extract_count_from_row(&row.data)?;

		Ok(count)
	}

	/// Get a single item by ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::orm::DatabaseConnection;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let db = AdminDatabase::new(conn);
	///
	/// let item = db.get::<AdminRecord>("admin_records", "id", "1").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get<M: Model>(
		&self,
		table_name: &str,
		pk_field: &str,
		id: &str,
	) -> AdminResult<Option<HashMap<String, serde_json::Value>>> {
		// Convert id to appropriate type for WHERE clause
		let pk_value: Value = if let Ok(num_id) = id.parse::<i64>() {
			Value::BigInt(Some(num_id))
		} else {
			Value::String(Some(Box::new(id.to_string())))
		};

		// SELECT * is intentional: admin detail view displays all fields from the
		// model. The admin panel operates on dynamic schemas where the column set
		// is determined by the ModelAdmin configuration at runtime.
		let query = Query::select()
			.from(Alias::new(table_name))
			.column(ColumnRef::Asterisk)
			.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value))
			.to_owned();

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_optional(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		Ok(row.and_then(|r| {
			// r.data is already a serde_json::Value, typically an Object
			if let serde_json::Value::Object(map) = r.data {
				Some(
					map.into_iter()
						.collect::<HashMap<String, serde_json::Value>>(),
				)
			} else {
				None
			}
		}))
	}

	/// Create a new item
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::orm::DatabaseConnection;
	/// use std::collections::HashMap;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let db = AdminDatabase::new(conn);
	///
	/// let mut data = HashMap::new();
	/// data.insert("name".to_string(), serde_json::json!("Alice"));
	/// data.insert("email".to_string(), serde_json::json!("alice@example.com"));
	///
	/// db.create::<AdminRecord>("admin_records", data).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn create<M: Model>(
		&self,
		table_name: &str,
		data: HashMap<String, serde_json::Value>,
	) -> AdminResult<u64> {
		let mut query = Query::insert()
			.into_table(Alias::new(table_name))
			.to_owned();

		// Sort keys for deterministic column ordering in generated SQL.
		// HashMap iteration order is non-deterministic, which causes
		// flaky tests and non-reproducible query plans.
		let mut sorted_keys: Vec<String> = data.keys().cloned().collect();
		sorted_keys.sort();

		// Build column and value lists in sorted order
		let mut columns = Vec::new();
		let mut values = Vec::new();

		for key in sorted_keys {
			let value = data.get(&key).cloned().unwrap_or(serde_json::Value::Null);
			columns.push(Alias::new(&key));

			let sea_value = match value {
				serde_json::Value::String(s) => Value::String(Some(Box::new(s))),
				serde_json::Value::Number(n) => {
					if let Some(i) = n.as_i64() {
						Value::BigInt(Some(i))
					} else if let Some(f) = n.as_f64() {
						Value::Double(Some(f))
					} else {
						Value::String(Some(Box::new(n.to_string())))
					}
				}
				serde_json::Value::Bool(b) => Value::Bool(Some(b)),
				serde_json::Value::Null => Value::Int(None),
				_ => Value::String(Some(Box::new(value.to_string()))),
			};
			values.push(sea_value);
		}

		// Pass values directly for reinhardt-query
		query.columns(columns).values(values).map_err(|e| {
			AdminError::DatabaseError(format!("column/value count mismatch: {}", e))
		})?;

		// Add RETURNING clause to get the inserted ID
		query.returning([Alias::new("id")]);

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_one(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Extract the ID from the returned row
		let id = if let Some(serde_json::Value::Number(n)) = row.data.get("id") {
			n.as_u64().unwrap_or(0)
		} else {
			0
		};

		Ok(id)
	}

	/// Update an existing item
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::orm::DatabaseConnection;
	/// use std::collections::HashMap;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let db = AdminDatabase::new(conn);
	///
	/// let mut data = HashMap::new();
	/// data.insert("name".to_string(), serde_json::json!("Alice Updated"));
	///
	/// db.update::<AdminRecord>("admin_records", "id", "1", data).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn update<M: Model>(
		&self,
		table_name: &str,
		pk_field: &str,
		id: &str,
		data: HashMap<String, serde_json::Value>,
	) -> AdminResult<u64> {
		let mut query = Query::update().table(Alias::new(table_name)).to_owned();

		// Sort keys for deterministic SET clause ordering in generated SQL
		let mut sorted_keys: Vec<String> = data.keys().cloned().collect();
		sorted_keys.sort();

		// Build SET clauses in sorted order
		for key in sorted_keys {
			let value = data.get(&key).cloned().unwrap_or(serde_json::Value::Null);
			let sea_value = match value {
				serde_json::Value::String(s) => Value::String(Some(Box::new(s))),
				serde_json::Value::Number(n) => {
					if let Some(i) = n.as_i64() {
						Value::BigInt(Some(i))
					} else if let Some(f) = n.as_f64() {
						Value::Double(Some(f))
					} else {
						Value::String(Some(Box::new(n.to_string())))
					}
				}
				serde_json::Value::Bool(b) => Value::Bool(Some(b)),
				serde_json::Value::Null => Value::Int(None),
				_ => Value::String(Some(Box::new(value.to_string()))),
			};
			query.value(Alias::new(&key), sea_value);
		}

		// Convert id to appropriate type for WHERE clause
		let pk_value: Value = if let Ok(num_id) = id.parse::<i64>() {
			Value::BigInt(Some(num_id))
		} else {
			Value::String(Some(Box::new(id.to_string())))
		};
		query.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value));

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let affected = self
			.connection
			.execute(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		Ok(affected)
	}

	/// Delete an item by ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::orm::DatabaseConnection;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let db = AdminDatabase::new(conn);
	///
	/// db.delete::<AdminRecord>("admin_records", "id", "1").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn delete<M: Model>(
		&self,
		table_name: &str,
		pk_field: &str,
		id: &str,
	) -> AdminResult<u64> {
		// Convert id to appropriate type for WHERE clause
		let pk_value: Value = if let Ok(num_id) = id.parse::<i64>() {
			Value::BigInt(Some(num_id))
		} else {
			Value::String(Some(Box::new(id.to_string())))
		};

		let query = Query::delete()
			.from_table(Alias::new(table_name))
			.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value))
			.to_owned();

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let affected = self
			.connection
			.execute(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		Ok(affected)
	}

	/// Delete multiple items by IDs (bulk delete)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::orm::DatabaseConnection;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let db = AdminDatabase::new(conn);
	///
	/// let ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
	/// db.bulk_delete::<AdminRecord>("admin_records", "id", ids).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn bulk_delete<M: Model>(
		&self,
		table_name: &str,
		pk_field: &str,
		ids: Vec<String>,
	) -> AdminResult<u64> {
		self.bulk_delete_by_table(table_name, pk_field, ids).await
	}

	/// Delete multiple items by IDs without requiring Model type parameter
	///
	/// This method provides a type-safe way to perform bulk deletions without
	/// requiring a Model type parameter. It's particularly useful for admin actions
	/// where the model type may not be known at compile time.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminDatabase;
	/// use reinhardt_db::orm::DatabaseConnection;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let db = AdminDatabase::new(conn);
	///
	/// let ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
	/// db.bulk_delete_by_table("users", "id", ids).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn bulk_delete_by_table(
		&self,
		table_name: &str,
		pk_field: &str,
		ids: Vec<String>,
	) -> AdminResult<u64> {
		if ids.is_empty() {
			return Ok(0);
		}

		// Convert each id to appropriate type for WHERE clause
		let pk_values: Vec<Value> = ids
			.iter()
			.map(|id| {
				if let Ok(num_id) = id.parse::<i64>() {
					Value::BigInt(Some(num_id))
				} else {
					Value::String(Some(Box::new(id.to_string())))
				}
			})
			.collect();

		let query = Query::delete()
			.from_table(Alias::new(table_name))
			.and_where(Expr::col(Alias::new(pk_field)).is_in(pk_values))
			.to_owned();

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let affected = self
			.connection
			.execute(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		Ok(affected)
	}

	/// Count total items with optional filters
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::orm::{DatabaseConnection, Filter, FilterOperator, FilterValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let conn = DatabaseConnection::connect("postgres://localhost/test").await?;
	/// let db = AdminDatabase::new(conn);
	///
	/// let filters = vec![
	///     Filter::new("is_active".to_string(), FilterOperator::Eq, FilterValue::Boolean(true))
	/// ];
	///
	/// let count = db.count::<AdminRecord>("admin_records", filters).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn count<M: Model>(
		&self,
		table_name: &str,
		filters: Vec<Filter>,
	) -> AdminResult<u64> {
		let mut query = Query::select()
			.from(Alias::new(table_name))
			.expr(Expr::cust("COUNT(*) AS count"))
			.to_owned();

		// Apply filters using build_filter_condition helper
		if let Some(condition) = build_filter_condition(&filters) {
			query.cond_where(condition);
		}

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_one(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Extract count from result, propagating errors for unexpected formats
		let count = extract_count_from_row(&row.data)?;

		Ok(count)
	}
}

/// Extract count value from a query result row
///
/// Attempts to extract an integer count from the query result in the following order:
/// 1. Look for a "count" key in the JSON object
/// 2. Take the first value from the JSON object
///
/// Returns an error if the data format is unexpected or the value cannot be
/// interpreted as an integer.
fn extract_count_from_row(data: &serde_json::Value) -> AdminResult<u64> {
	if let Some(count_value) = data.get("count") {
		return count_value.as_i64().map(|v| v as u64).ok_or_else(|| {
			AdminError::DatabaseError(format!(
				"COUNT query returned non-integer value: {}",
				count_value
			))
		});
	}

	if let Some(obj) = data.as_object()
		&& let Some(first_value) = obj.values().next()
	{
		return first_value.as_i64().map(|v| v as u64).ok_or_else(|| {
			AdminError::DatabaseError(format!(
				"COUNT query returned non-integer value: {}",
				first_value
			))
		});
	}

	Err(AdminError::DatabaseError(format!(
		"COUNT query returned unexpected data format: {}",
		data
	)))
}

/// Injectable trait implementation for AdminDatabase
///
/// This allows AdminDatabase to be injected via the DI container.
/// The implementation resolves `Arc<AdminDatabase>` from the container
/// and clones the inner value.
#[async_trait]
impl Injectable for AdminDatabase {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Resolve Arc<AdminDatabase> from the container and clone it
		ctx.resolve::<Self>().await.map(|arc| (*arc).clone())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_db::orm::annotation::Expression;
	use reinhardt_db::orm::expressions::{F, OuterRef};
	use rstest::rstest;

	// ==================== escape_like_pattern tests ====================

	#[rstest]
	fn test_escape_like_pattern_percent() {
		// Arrange
		let input = "100%";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "100\\%");
	}

	#[rstest]
	fn test_escape_like_pattern_underscore() {
		// Arrange
		let input = "user_name";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "user\\_name");
	}

	#[rstest]
	fn test_escape_like_pattern_backslash() {
		// Arrange
		let input = "path\\to";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "path\\\\to");
	}

	#[rstest]
	fn test_escape_like_pattern_combined() {
		// Arrange
		let input = "100%_done";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "100\\%\\_done");
	}

	#[rstest]
	fn test_escape_like_pattern_no_special_chars() {
		// Arrange
		let input = "normal text";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "normal text");
	}

	// ==================== escape_like_pattern regression tests (#632) ====================

	/// Regression tests for issue #632: LIKE wildcard injection via unescaped metacharacters.
	/// Verifies that percent, underscore, and backslash in user input are always escaped
	/// so they cannot be used as LIKE wildcards or escape prefix injections.
	#[rstest]
	#[case("%wildcard%", "\\%wildcard\\%")]
	#[case("under_score", "under\\_score")]
	#[case("back\\slash", "back\\\\slash")]
	#[case("%_%", "\\%\\_\\%")]
	fn test_escape_like_pattern_sanitizes_special_chars(
		#[case] input: &str,
		#[case] expected: &str,
	) {
		// Arrange: user-supplied string containing LIKE metacharacters
		// Act
		let escaped = escape_like_pattern(input);
		// Assert: output exactly matches fully-escaped form with no unescaped metacharacters
		assert_eq!(
			escaped, expected,
			"input={input:?} was not correctly escaped"
		);
	}

	// ==================== build_composite_filter_condition tests ====================

	#[test]
	fn test_build_composite_single_condition() {
		let filter = Filter::new(
			"name".to_string(),
			FilterOperator::Eq,
			FilterValue::String("Alice".to_string()),
		);
		let condition = FilterCondition::Single(filter);

		let result = build_composite_filter_condition(&condition);

		assert!(result.is_some());
		// The condition should produce valid SQL when used
		let cond = result.unwrap();
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond)
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("\"name\""));
		assert!(query.contains("'Alice'"));
	}

	#[test]
	fn test_build_composite_or_condition() {
		let filter1 = Filter::new(
			"name".to_string(),
			FilterOperator::Contains,
			FilterValue::String("Alice".to_string()),
		);
		let filter2 = Filter::new(
			"email".to_string(),
			FilterOperator::Contains,
			FilterValue::String("alice".to_string()),
		);

		let condition = FilterCondition::Or(vec![
			FilterCondition::Single(filter1),
			FilterCondition::Single(filter2),
		]);

		let result = build_composite_filter_condition(&condition);

		assert!(result.is_some());
		let cond = result.unwrap();
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond)
			.to_string(PostgresQueryBuilder);
		// OR condition should produce SQL with OR keyword
		assert!(query.contains("\"name\""));
		assert!(query.contains("\"email\""));
		assert!(query.contains("OR"));
	}

	#[test]
	fn test_build_composite_and_condition() {
		let filter1 = Filter::new(
			"is_active".to_string(),
			FilterOperator::Eq,
			FilterValue::Boolean(true),
		);
		let filter2 = Filter::new(
			"is_staff".to_string(),
			FilterOperator::Eq,
			FilterValue::Boolean(true),
		);

		let condition = FilterCondition::And(vec![
			FilterCondition::Single(filter1),
			FilterCondition::Single(filter2),
		]);

		let result = build_composite_filter_condition(&condition);

		assert!(result.is_some());
		let cond = result.unwrap();
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond)
			.to_string(PostgresQueryBuilder);
		// AND condition should produce SQL with AND keyword
		assert!(query.contains("\"is_active\""));
		assert!(query.contains("\"is_staff\""));
		assert!(query.contains("AND"));
	}

	#[test]
	fn test_build_composite_nested_condition() {
		// Build: (name LIKE '%Alice%' OR email LIKE '%alice%') AND is_active = true
		let filter_name = Filter::new(
			"name".to_string(),
			FilterOperator::Contains,
			FilterValue::String("Alice".to_string()),
		);
		let filter_email = Filter::new(
			"email".to_string(),
			FilterOperator::Contains,
			FilterValue::String("alice".to_string()),
		);
		let filter_active = Filter::new(
			"is_active".to_string(),
			FilterOperator::Eq,
			FilterValue::Boolean(true),
		);

		let or_condition = FilterCondition::Or(vec![
			FilterCondition::Single(filter_name),
			FilterCondition::Single(filter_email),
		]);

		let and_condition =
			FilterCondition::And(vec![or_condition, FilterCondition::Single(filter_active)]);

		let result = build_composite_filter_condition(&and_condition);

		assert!(result.is_some());
		let cond = result.unwrap();
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond)
			.to_string(PostgresQueryBuilder);
		// Nested condition should contain both OR and AND
		assert!(query.contains("\"name\""));
		assert!(query.contains("\"email\""));
		assert!(query.contains("\"is_active\""));
		assert!(query.contains("OR"));
		assert!(query.contains("AND"));
	}

	#[test]
	fn test_build_composite_empty_or() {
		let condition = FilterCondition::Or(vec![]);

		let result = build_composite_filter_condition(&condition);

		// Empty OR should return None
		assert!(result.is_none());
	}

	#[test]
	fn test_build_composite_empty_and() {
		let condition = FilterCondition::And(vec![]);

		let result = build_composite_filter_condition(&condition);

		// Empty AND should return None
		assert!(result.is_none());
	}

	// ==================== FieldRef/OuterRef/Expression filter tests ====================

	#[test]
	fn test_build_single_filter_expr_field_ref_eq() {
		let filter = Filter::new(
			"price".to_string(),
			FilterOperator::Eq,
			FilterValue::FieldRef(F::new("discount_price")),
		);
		let result = build_single_filter_expr(&filter);
		assert!(result.is_some());

		let query = Query::select()
			.from(Alias::new("products"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("\"price\""));
		assert!(query.contains("\"discount_price\""));
	}

	#[test]
	fn test_build_single_filter_expr_field_ref_gt() {
		let filter = Filter::new(
			"price".to_string(),
			FilterOperator::Gt,
			FilterValue::FieldRef(F::new("cost")),
		);
		let result = build_single_filter_expr(&filter);
		assert!(result.is_some());
	}

	#[test]
	fn test_build_single_filter_expr_field_ref_all_operators() {
		let operators = [
			FilterOperator::Eq,
			FilterOperator::Ne,
			FilterOperator::Gt,
			FilterOperator::Gte,
			FilterOperator::Lt,
			FilterOperator::Lte,
		];

		for op in operators {
			let filter = Filter::new(
				"field_a".to_string(),
				op.clone(),
				FilterValue::FieldRef(F::new("field_b")),
			);
			let result = build_single_filter_expr(&filter);
			assert!(
				result.is_some(),
				"FieldRef with {:?} should produce Some",
				op
			);
		}
	}

	#[test]
	fn test_build_single_filter_expr_outer_ref() {
		let filter = Filter::new(
			"author_id".to_string(),
			FilterOperator::Eq,
			FilterValue::OuterRef(OuterRef::new("authors.id")),
		);
		let result = build_single_filter_expr(&filter);
		assert!(result.is_some());

		let query = Query::select()
			.from(Alias::new("books"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("author_id"));
		assert!(query.contains("authors.id"));
	}

	#[test]
	fn test_build_single_filter_expr_outer_ref_all_operators() {
		let operators = [
			FilterOperator::Eq,
			FilterOperator::Ne,
			FilterOperator::Gt,
			FilterOperator::Gte,
			FilterOperator::Lt,
			FilterOperator::Lte,
		];

		for op in operators {
			let filter = Filter::new(
				"child_id".to_string(),
				op.clone(),
				FilterValue::OuterRef(OuterRef::new("parent.id")),
			);
			let result = build_single_filter_expr(&filter);
			assert!(
				result.is_some(),
				"OuterRef with {:?} should produce Some",
				op
			);
		}
	}

	#[test]
	fn test_build_single_filter_expr_expression() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value};

		// Test: price > (cost * 2)
		let expr = Expression::Multiply(
			Box::new(AnnotationValue::Field(F::new("cost"))),
			Box::new(AnnotationValue::Value(Value::Int(2))),
		);
		let filter = Filter::new(
			"price".to_string(),
			FilterOperator::Gt,
			FilterValue::Expression(expr),
		);
		let result = build_single_filter_expr(&filter);
		assert!(result.is_some());
	}

	#[test]
	fn test_build_single_filter_expr_expression_all_operators() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

		let operators = [
			FilterOperator::Eq,
			FilterOperator::Ne,
			FilterOperator::Gt,
			FilterOperator::Gte,
			FilterOperator::Lt,
			FilterOperator::Lte,
		];

		for op in operators {
			let expr = Expression::Add(
				Box::new(AnnotationValue::Field(F::new("base"))),
				Box::new(AnnotationValue::Value(OrmValue::Int(10))),
			);
			let filter = Filter::new(
				"total".to_string(),
				op.clone(),
				FilterValue::Expression(expr),
			);
			let result = build_single_filter_expr(&filter);
			assert!(
				result.is_some(),
				"Expression with {:?} should produce Some",
				op
			);
		}
	}

	#[test]
	fn test_filter_value_to_sea_value_field_ref_fallback() {
		let value = FilterValue::FieldRef(F::new("test_field"));
		let sea_value = filter_value_to_sea_value(&value);

		// Should return string representation, not panic
		match sea_value {
			Value::String(Some(s)) => assert_eq!(s.as_str(), "test_field"),
			_ => panic!("Expected String value"),
		}
	}

	#[test]
	fn test_filter_value_to_sea_value_outer_ref_fallback() {
		let value = FilterValue::OuterRef(OuterRef::new("outer.field"));
		let sea_value = filter_value_to_sea_value(&value);

		// Should return string representation, not panic
		match sea_value {
			Value::String(Some(s)) => assert_eq!(s.as_str(), "outer.field"),
			_ => panic!("Expected String value"),
		}
	}

	#[test]
	fn test_filter_value_to_sea_value_expression_fallback() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

		let expr = Expression::Add(
			Box::new(AnnotationValue::Field(F::new("a"))),
			Box::new(AnnotationValue::Value(OrmValue::Int(1))),
		);
		let value = FilterValue::Expression(expr);
		let sea_value = filter_value_to_sea_value(&value);

		// Should return SQL string representation, not panic
		match sea_value {
			Value::String(Some(s)) => {
				assert!(s.contains("a"), "SQL should contain field name 'a'");
				assert!(s.contains("1"), "SQL should contain value '1'");
			}
			_ => panic!("Expected String value"),
		}
	}

	// ==================== insert values mismatch tests (#1551) ====================

	#[rstest]
	fn test_insert_values_mismatch_returns_error_not_panic() {
		// Arrange
		// Simulate the scenario where columns and values count mismatch
		// by calling SeaQuery's values() with wrong number of values
		let mut query = Query::insert()
			.into_table(Alias::new("test_table"))
			.to_owned();

		let columns = vec![Alias::new("col1"), Alias::new("col2"), Alias::new("col3")];
		let values = vec![Value::String(Some(Box::new("val1".to_string())))]; // Only 1 value for 3 columns

		// Act
		let result = query.columns(columns).values(values);

		// Assert - should return Err, not panic
		assert!(result.is_err());
	}

	#[rstest]
	fn test_insert_values_matching_count_succeeds() {
		// Arrange
		let mut query = Query::insert()
			.into_table(Alias::new("test_table"))
			.to_owned();

		let columns = vec![Alias::new("col1"), Alias::new("col2")];
		let values = vec![
			Value::String(Some(Box::new("val1".to_string()))),
			Value::String(Some(Box::new("val2".to_string()))),
		];

		// Act
		let result = query.columns(columns).values(values);

		// Assert
		assert!(result.is_ok());
	}

	// ==================== SQL injection prevention tests ====================

	#[test]
	fn test_outer_ref_filter_uses_safe_column_api() {
		// Arrange: OuterRef with a field name that could be an injection attempt
		let filter = Filter::new(
			"author_id".to_string(),
			FilterOperator::Eq,
			FilterValue::OuterRef(OuterRef::new("users.id")),
		);

		// Act
		let result = build_single_filter_expr(&filter);

		// Assert: should produce a valid expression using quoted identifiers
		assert!(result.is_some());
		let expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("books"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(expr))
			.to_string(PostgresQueryBuilder);
		// The field names should be quoted by SeaQuery's Alias, not raw interpolation
		assert!(
			query.contains("\"author_id\""),
			"Column should be properly quoted: {}",
			query
		);
	}

	#[test]
	fn test_outer_ref_injection_attempt_is_safely_quoted() {
		// Arrange: attacker tries SQL injection through OuterRef field name
		let filter = Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::OuterRef(OuterRef::new("id; DROP TABLE users; --")),
		);

		// Act
		let result = build_single_filter_expr(&filter);

		// Assert: the injection string should be treated as a quoted identifier
		assert!(result.is_some());
		let expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("items"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(expr))
			.to_string(PostgresQueryBuilder);
		// SeaQuery's Alias wraps the name in double quotes, treating the entire
		// injection payload as a single identifier name (not executable SQL).
		// The right side of the equality uses Expr::col(Alias::new(...)) which
		// produces a quoted identifier instead of raw SQL interpolation.
		assert!(
			query.contains("\"id; DROP TABLE users; --\""),
			"Injection payload should be enclosed in double quotes as identifier: {}",
			query
		);
		// Verify the query is a valid single-statement SELECT (no semicolons
		// appear outside of the quoted identifier)
		let unquoted_parts: Vec<&str> = query.split('"').enumerate()
			.filter(|(i, _)| i % 2 == 0) // Even indices are outside quotes
			.map(|(_, s)| s)
			.collect();
		let unquoted_sql = unquoted_parts.join("");
		assert!(
			!unquoted_sql.contains(';'),
			"No semicolons should appear outside quoted identifiers: {}",
			query
		);
	}

	#[test]
	fn test_expression_filter_uses_safe_api() {
		use reinhardt_db::orm::annotation::AnnotationValue;

		// Arrange: arithmetic expression (price * quantity)
		let expr = Expression::Multiply(
			Box::new(AnnotationValue::Field(F::new("unit_price"))),
			Box::new(AnnotationValue::Field(F::new("quantity"))),
		);
		let filter = Filter::new(
			"total".to_string(),
			FilterOperator::Eq,
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter);

		// Assert
		assert!(result.is_some());
		let sea_expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("orders"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(sea_expr))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("\"total\""),
			"Left side should be quoted: {}",
			query
		);
	}

	#[test]
	fn test_expression_filter_with_literal_value() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

		// Arrange: field + literal value
		let expr = Expression::Add(
			Box::new(AnnotationValue::Field(F::new("price"))),
			Box::new(AnnotationValue::Value(OrmValue::Int(100))),
		);
		let filter = Filter::new(
			"adjusted_price".to_string(),
			FilterOperator::Gt,
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter);

		// Assert
		assert!(result.is_some());
	}

	#[test]
	fn test_outer_ref_all_operators_use_safe_api() {
		// Arrange & Act & Assert: verify all comparison operators work with OuterRef
		let operators = vec![
			FilterOperator::Eq,
			FilterOperator::Ne,
			FilterOperator::Gt,
			FilterOperator::Gte,
			FilterOperator::Lt,
			FilterOperator::Lte,
		];

		for op in operators {
			let filter = Filter::new(
				"field_a".to_string(),
				op.clone(),
				FilterValue::OuterRef(OuterRef::new("field_b")),
			);
			let result = build_single_filter_expr(&filter);
			assert!(
				result.is_some(),
				"OuterRef with {:?} should produce Some",
				op
			);
		}
	}

	// ==================== Case/Coalesce safe expression tests ====================

	#[test]
	fn test_coalesce_expression_uses_safe_parameterized_api() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

		// Arrange: COALESCE(field_a, 0)
		let expr = Expression::Coalesce(vec![
			AnnotationValue::Field(F::new("field_a")),
			AnnotationValue::Value(OrmValue::Int(0)),
		]);
		let filter = Filter::new(
			"result".to_string(),
			FilterOperator::Gt,
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter);

		// Assert
		assert!(result.is_some());
		let sea_expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("items"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(sea_expr))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("COALESCE"),
			"Should contain COALESCE function: {}",
			query
		);
		assert!(
			query.contains("\"result\""),
			"Left side should be quoted: {}",
			query
		);
	}

	#[test]
	fn test_case_expression_uses_safe_api() {
		use reinhardt_db::orm::annotation::{
			AnnotationValue, Value as OrmValue, When as AnnotWhen,
		};
		use reinhardt_db::orm::expressions::Q;

		// Arrange: CASE WHEN status = 'active' THEN 1 ELSE 0 END
		let expr = Expression::Case {
			whens: vec![AnnotWhen::new(
				Q::new("status", "=", "'active'"),
				AnnotationValue::Value(OrmValue::Int(1)),
			)],
			default: Some(Box::new(AnnotationValue::Value(OrmValue::Int(0)))),
		};
		let filter = Filter::new(
			"priority".to_string(),
			FilterOperator::Eq,
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter);

		// Assert
		assert!(result.is_some());
		let sea_expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("tasks"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(sea_expr))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("CASE"),
			"Should contain CASE keyword: {}",
			query
		);
		assert!(
			query.contains("WHEN"),
			"Should contain WHEN keyword: {}",
			query
		);
		assert!(
			query.contains("ELSE"),
			"Should contain ELSE keyword: {}",
			query
		);
	}

	#[test]
	fn test_empty_coalesce_returns_null() {
		// Arrange: COALESCE() with no values
		let expr = Expression::Coalesce(vec![]);

		// Act
		let result = annotation_expr_to_safe_expr(&expr);

		// Assert: should produce NULL expression without panicking
		let query = Query::select()
			.from(Alias::new("test"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("NULL"),
			"Empty COALESCE should produce NULL: {}",
			query
		);
	}

	// ==================== Aggregate safe expression tests ====================

	#[test]
	fn test_aggregate_count_uses_safe_api() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};

		// Arrange: COUNT(*)
		let agg = Aggregate {
			func: AggregateFunc::Count,
			field: None,
			alias: None,
			distinct: false,
		};

		// Act
		let result = aggregate_to_safe_expr(&agg);

		// Assert
		let query = Query::select()
			.from(Alias::new("items"))
			.expr(result)
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("COUNT(*)"),
			"Should contain COUNT(*): {}",
			query
		);
	}

	#[test]
	fn test_aggregate_sum_field_uses_quoted_identifier() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};

		// Arrange: SUM(price)
		let agg = Aggregate {
			func: AggregateFunc::Sum,
			field: Some("price".to_string()),
			alias: None,
			distinct: false,
		};

		// Act
		let result = aggregate_to_safe_expr(&agg);

		// Assert
		let query = Query::select()
			.from(Alias::new("orders"))
			.expr(result)
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("SUM("),
			"Should contain SUM function: {}",
			query
		);
		assert!(
			query.contains("\"price\""),
			"Field name should be quoted: {}",
			query
		);
	}

	#[test]
	fn test_aggregate_count_distinct_uses_distinct_keyword() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};

		// Arrange: COUNT(DISTINCT category)
		let agg = Aggregate {
			func: AggregateFunc::CountDistinct,
			field: Some("category".to_string()),
			alias: None,
			distinct: false, // AggregateFunc::CountDistinct implies DISTINCT
		};

		// Act
		let result = aggregate_to_safe_expr(&agg);

		// Assert
		let query = Query::select()
			.from(Alias::new("products"))
			.expr(result)
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("COUNT(DISTINCT"),
			"Should contain COUNT(DISTINCT: {}",
			query
		);
		assert!(
			query.contains("\"category\""),
			"Field name should be quoted: {}",
			query
		);
	}

	#[test]
	fn test_aggregate_injection_attempt_is_quoted() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};

		// Arrange: attacker tries injection via aggregate field name
		let agg = Aggregate {
			func: AggregateFunc::Sum,
			field: Some("price); DROP TABLE users; --".to_string()),
			alias: None,
			distinct: false,
		};

		// Act
		let result = aggregate_to_safe_expr(&agg);

		// Assert: injection payload should be treated as a quoted identifier
		let query = Query::select()
			.from(Alias::new("orders"))
			.expr(result)
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("\"price); DROP TABLE users; --\""),
			"Injection payload should be enclosed in double quotes: {}",
			query
		);
	}
}
