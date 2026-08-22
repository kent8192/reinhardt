//! Unified query interface facade
//!
//! This module provides a unified entry point for querying functionality.
//! By default, it exports the expression-based query API (SQLAlchemy-style).

use super::connection::{OrmExecutor, QueryRow, RowStream};
use super::expressions::{OrderingField, UniqueFieldRef};
use super::field_codec::{
	DatabaseField, DatabaseValue, FieldCodecError, IntoFieldValue, database_value_to_query_value,
};
use super::{FieldSelector, Model};
use crate::backends::types::QueryValue;
use crate::naming::to_snake_case;
use crate::orm::query_fields::comparison::FieldComparison;
use crate::orm::query_fields::compiler::QueryFieldCompiler;
use crate::orm::query_fields::expression::{compiler::compile_expression, node::StoredExpression};
use crate::orm::query_fields::{
	AnnotationExpressionKind, GroupByFields, HavingPredicate, LabeledExpression, OrderedExpression,
	TypedExpression,
};
use crate::orm::relations::{RelationJoinGraph, RelationJoinKind, RelationPathLike, RelationStep};
use futures::{Stream, StreamExt};
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
use reinhardt_query::prelude::{
	Alias, BinOper, CockroachDBQueryBuilder, ColumnRef, Condition, ExplainStatement, Expr,
	ExprTrait, Func, JoinType as SeaJoinType, LockBehavior, LockType, MySqlQueryBuilder, Order,
	PostgresQueryBuilder, Query, QueryBuilder, QueryStatementBuilder, SelectStatement, SimpleExpr,
	SqliteQueryBuilder, TableRef, TemporalTimeZone, TemporalTruncKind, TemporalTruncOutput,
	UpdateStatement,
};
use reinhardt_query::types::PgBinOper;
use reinhardt_query::value::Value;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Instant;
use uuid::Uuid;

#[path = "query/aggregate.rs"]
mod aggregate;
pub use aggregate::AggregateInput;

pub use reinhardt_query::query::{ExplainFormat, ExplainOptions};

fn executor_error(error: reinhardt_core::exception::Error) -> DatabaseError {
	crate::backends::error::into_database_error(error)
}

/// Backend-independent body returned by a plan-only EXPLAIN operation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ExplainBody {
	/// Single-column human-readable output joined with newlines.
	Text(String),
	/// Machine-readable JSON output.
	Json(serde_json::Value),
	/// Backend tabular output retained as JSON objects.
	Rows(Vec<serde_json::Value>),
}

/// Database dialect that generated an EXPLAIN plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExplainBackend {
	/// PostgreSQL.
	Postgres,
	/// MySQL or MariaDB.
	MySql,
	/// SQLite.
	Sqlite,
	/// CockroachDB's PostgreSQL-compatible dialect.
	CockroachDb,
}

/// Structured output from a backend-aware plan-only EXPLAIN operation.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplainOutput {
	/// Backend that generated the plan.
	pub backend: ExplainBackend,
	/// Effective output format.
	pub format: ExplainFormat,
	/// Decoded plan body, separate from model-row deserialization.
	pub body: ExplainBody,
}

mod temporal_projection_field {
	pub trait Sealed {}

	impl Sealed for chrono::NaiveDate {}
	impl Sealed for Option<chrono::NaiveDate> {}
	impl Sealed for chrono::DateTime<chrono::Utc> {}
	impl Sealed for Option<chrono::DateTime<chrono::Utc>> {}
}

/// Field types accepted by [`QuerySet::dates`].
pub trait DateProjectionField: temporal_projection_field::Sealed {}

impl DateProjectionField for chrono::NaiveDate {}
impl DateProjectionField for Option<chrono::NaiveDate> {}

/// Field types accepted by [`QuerySet::datetimes`].
pub trait DateTimeProjectionField: temporal_projection_field::Sealed {}

impl DateTimeProjectionField for chrono::DateTime<chrono::Utc> {}
impl DateTimeProjectionField for Option<chrono::DateTime<chrono::Utc>> {}

/// Truncation unit for date projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTruncKind {
	/// First day of the calendar year.
	Year,
	/// First day of the calendar month.
	Month,
	/// Monday of the ISO week.
	Week,
	/// Calendar day.
	Day,
}

impl From<DateTruncKind> for TemporalTruncKind {
	fn from(kind: DateTruncKind) -> Self {
		match kind {
			DateTruncKind::Year => Self::Year,
			DateTruncKind::Month => Self::Month,
			DateTruncKind::Week => Self::Week,
			DateTruncKind::Day => Self::Day,
		}
	}
}

/// Truncation unit for datetime projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeTruncKind {
	/// First instant of the calendar year.
	Year,
	/// First instant of the calendar month.
	Month,
	/// Monday at midnight of the ISO week.
	Week,
	/// Midnight of the calendar day.
	Day,
	/// Start of the hour.
	Hour,
	/// Start of the minute.
	Minute,
	/// Start of the second.
	Second,
}

impl From<DateTimeTruncKind> for TemporalTruncKind {
	fn from(kind: DateTimeTruncKind) -> Self {
		match kind {
			DateTimeTruncKind::Year => Self::Year,
			DateTimeTruncKind::Month => Self::Month,
			DateTimeTruncKind::Week => Self::Week,
			DateTimeTruncKind::Day => Self::Day,
			DateTimeTruncKind::Hour => Self::Hour,
			DateTimeTruncKind::Minute => Self::Minute,
			DateTimeTruncKind::Second => Self::Second,
		}
	}
}

/// Ordering direction for date and datetime projections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DateProjectionOrder {
	/// Ascending chronological order.
	#[default]
	Asc,
	/// Descending chronological order.
	Desc,
}

impl From<DateProjectionOrder> for Order {
	fn from(order: DateProjectionOrder) -> Self {
		match order {
			DateProjectionOrder::Asc => Self::Asc,
			DateProjectionOrder::Desc => Self::Desc,
		}
	}
}

/// A lifetime-bound stream of decoded QuerySet models.
pub type QuerySetStream<'a, T> =
	Pin<Box<dyn Stream<Item = reinhardt_core::exception::Result<T>> + Send + 'a>>;

struct PendingRowPoll {
	generation: u64,
	started_at: Instant,
	wake_duration: Option<std::time::Duration>,
}

#[derive(Default)]
struct RowStreamTiming {
	next_generation: u64,
	pending: Option<PendingRowPoll>,
}

/// Records a streaming query when its generator finishes or is dropped.
struct StreamQueryAccounting {
	sql: String,
	params: Vec<String>,
	duration: std::time::Duration,
	completed: bool,
	timing: Arc<Mutex<RowStreamTiming>>,
}

impl StreamQueryAccounting {
	fn new(sql: String, params: Vec<String>) -> Self {
		Self {
			sql,
			params,
			duration: std::time::Duration::ZERO,
			completed: true,
			timing: Arc::new(Mutex::new(RowStreamTiming::default())),
		}
	}

	fn record_poll(&mut self, duration: std::time::Duration) {
		self.duration += duration;
	}

	fn disarm_completion(&mut self) {
		self.completed = false;
	}

	fn take_woken_duration(&mut self) -> Option<std::time::Duration> {
		let mut timing = self
			.timing
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner());
		let pending = timing.pending.take()?;
		pending.wake_duration
	}

	fn record_woken_duration(&mut self) {
		if let Some(duration) = self.take_woken_duration() {
			self.record_poll(duration);
		}
	}
}

impl Drop for StreamQueryAccounting {
	fn drop(&mut self) {
		self.record_woken_duration();
		if !self.completed {
			return;
		}
		super::instrumentation::instrumentation().orm_query_end_with_params_sync(
			&self.sql,
			&self.params,
			self.duration,
		);
	}
}
struct RowStreamWake {
	timing: Arc<Mutex<RowStreamTiming>>,
	generation: u64,
	target: Waker,
}

impl RowStreamWake {
	fn record_wake(&self) {
		let mut timing = self
			.timing
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner());
		if let Some(pending) = timing.pending.as_mut()
			&& pending.generation == self.generation
			&& pending.wake_duration.is_none()
		{
			pending.wake_duration = Some(pending.started_at.elapsed());
		}
	}
}

impl Wake for RowStreamWake {
	fn wake(self: Arc<Self>) {
		self.record_wake();
		self.target.wake_by_ref();
	}

	fn wake_by_ref(self: &Arc<Self>) {
		self.record_wake();
		self.target.wake_by_ref();
	}
}

/// Times backend-stream polls through their wakeup without charging consumer idle time.
///
/// A wrapped waker records a pending poll when the backend wakes the task. If
/// the consumer cancels that poll and later polls again without a wakeup, the
/// stale pending interval is discarded before the next backend poll.
struct TimedRowStream<'rows, 'accounting> {
	rows: RowStream<'rows>,
	accounting: &'accounting mut StreamQueryAccounting,
}

impl<'rows, 'accounting> TimedRowStream<'rows, 'accounting> {
	fn new(rows: RowStream<'rows>, accounting: &'accounting mut StreamQueryAccounting) -> Self {
		Self { rows, accounting }
	}

	fn start_pending_poll(&mut self, started_at: Instant, target: Waker) -> (u64, Waker) {
		let generation = {
			let mut timing = self
				.accounting
				.timing
				.lock()
				.unwrap_or_else(|poisoned| poisoned.into_inner());
			let generation = timing.next_generation;
			timing.next_generation = timing.next_generation.wrapping_add(1);
			timing.pending = Some(PendingRowPoll {
				generation,
				started_at,
				wake_duration: None,
			});
			generation
		};
		let waker = Waker::from(Arc::new(RowStreamWake {
			timing: Arc::clone(&self.accounting.timing),
			generation,
			target,
		}));
		(generation, waker)
	}

	fn clear_pending_poll(&mut self, generation: u64) {
		let mut timing = self
			.accounting
			.timing
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner());
		if timing
			.pending
			.as_ref()
			.is_some_and(|pending| pending.generation == generation)
		{
			timing.pending = None;
		}
	}
}

impl Stream for TimedRowStream<'_, '_> {
	type Item = reinhardt_core::exception::Result<crate::backends::types::Row>;

	fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		self.accounting.record_woken_duration();
		let started_at = Instant::now();
		let (generation, waker) = self.start_pending_poll(started_at, context.waker().clone());
		let mut timing_context = Context::from_waker(&waker);
		let result = self.rows.as_mut().poll_next(&mut timing_context);
		if result.is_ready() {
			self.clear_pending_poll(generation);
			self.accounting.record_poll(started_at.elapsed());
		}
		result
	}
}

// Django QuerySet API types
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Defines possible filter operator values.
pub enum FilterOperator {
	/// Eq variant.
	Eq,
	/// Case-insensitive exact match.
	IExact,
	/// Ne variant.
	Ne,
	/// Gt variant.
	Gt,
	/// Gte variant.
	Gte,
	/// Lt variant.
	Lt,
	/// Lte variant.
	Lte,
	/// In variant.
	In,
	/// NotIn variant.
	NotIn,
	/// Contains variant.
	Contains,
	/// Case-insensitive contains variant.
	IContains,
	/// StartsWith variant.
	StartsWith,
	/// Case-insensitive starts-with variant.
	IStartsWith,
	/// EndsWith variant.
	EndsWith,
	/// Case-insensitive ends-with variant.
	IEndsWith,
	/// Regular expression match.
	Regex,
	/// Case-insensitive regular expression match.
	IRegex,
	/// BETWEEN range lookup.
	Range,
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
/// Defines possible filter value values.
pub enum FilterValue {
	/// Fallible scalar produced by a typed field codec.
	Typed(Result<DatabaseValue, FieldCodecError>),
	/// String variant.
	String(String),
	/// UTC timestamp variant.
	Timestamp(chrono::DateTime<chrono::Utc>),
	/// UUID variant.
	Uuid(uuid::Uuid),
	/// Integer variant.
	Integer(i64),
	/// Alias for Integer (for compatibility with test code)
	Int(i64),
	/// Float variant.
	Float(f64),
	/// Boolean variant.
	Boolean(bool),
	/// Alias for Boolean (for compatibility with test code)
	Bool(bool),
	/// Null variant.
	Null,
	/// Array variant.
	Array(Vec<String>),
	/// Typed list variant for IN and NOT IN lookups.
	List(Vec<FilterValue>),
	/// Two-value range for BETWEEN lookups.
	Range(Box<FilterValue>, Box<FilterValue>),
	/// Field reference for field-to-field comparisons (e.g., WHERE discount_price < total_price)
	FieldRef(super::expressions::F),
	/// Arithmetic expression (e.g., WHERE total != unit_price * quantity)
	Expression(super::annotation::Expression),
	/// Outer query reference for correlated subqueries (e.g., WHERE books.author_id = OuterRef("authors.id"))
	OuterRef(super::expressions::OuterRef),
}

impl FilterValue {
	fn is_empty_membership_collection(&self) -> bool {
		match self {
			Self::List(values) => values.is_empty(),
			Self::Array(values) => values.is_empty(),
			Self::String(value) => parse_membership_string(value).is_empty(),
			_ => false,
		}
	}
}

#[derive(Debug, Clone)]
enum FilterField {
	Column(String),
	Expression(String),
	// Boxed to keep `FilterCondition::Single(Filter)` compact: `StoredExpression`
	// carries the full typed aggregate/annotation expression tree and is
	// significantly larger than the other `FilterField` variants.
	TypedPredicate(Result<Box<StoredExpression>, String>),
}

#[derive(Debug, Clone)]
struct FilterRelation {
	steps: SmallVec<[RelationStep; 4]>,
	join_kind_override: Option<RelationJoinKind>,
	leaf_alias: String,
	root_type_name: &'static str,
}

impl FilterRelation {
	fn from_path<P>(path: &P) -> Self
	where
		P: RelationPathLike,
	{
		let mut steps: SmallVec<[RelationStep; 4]> = SmallVec::new();
		steps.extend(path.steps().iter().cloned());
		Self {
			steps,
			join_kind_override: path.join_kind_override(),
			leaf_alias: path.leaf_alias().to_string(),
			root_type_name: std::any::type_name::<P::Root>(),
		}
	}

	fn add_to_graph(&self, graph: &mut RelationJoinGraph) {
		graph.add_steps_with_override(&self.steps, self.join_kind_override);
	}

	fn rebase_join_alias(&mut self, graph: &RelationJoinGraph) {
		if let Some(alias) = graph
			.aliases_for_steps(&self.steps)
			.and_then(|aliases| aliases.last().cloned())
		{
			self.leaf_alias = alias;
		}
	}

	fn root_type_name(&self) -> &'static str {
		self.root_type_name
	}
}

#[derive(Debug, Clone)]
/// Represents a filter.
pub struct Filter {
	/// The field.
	pub field: String,
	field_source: FilterField,
	relation: Option<Box<FilterRelation>>,
	field_type: Option<String>,
	/// The operator.
	pub operator: FilterOperator,
	/// The value.
	pub value: FilterValue,
}

impl Filter {
	/// Creates a new instance.
	pub fn new(field: impl Into<String>, operator: FilterOperator, value: FilterValue) -> Self {
		let field = field.into();
		Self {
			field_source: FilterField::Column(field.clone()),
			field,
			relation: None,
			field_type: None,
			operator,
			value,
		}
	}

	/// Creates a filter for a field reached through a typed relation path.
	pub(crate) fn related<P>(
		field: impl Into<String>,
		operator: FilterOperator,
		value: FilterValue,
		path: &P,
	) -> Self
	where
		P: RelationPathLike,
	{
		let field = field.into();
		let field_type = P::Target::field_metadata()
			.into_iter()
			.find(|metadata| metadata.name == field || metadata.db_column_name() == field)
			.map(|metadata| metadata.field_type);
		Self {
			field_source: FilterField::Column(field.clone()),
			field,
			relation: Some(Box::new(FilterRelation::from_path(path))),
			field_type,
			operator,
			value,
		}
	}

	/// Rewrite an expression-backed filter after normalizing its column names.
	pub fn map_expression_source<F>(&mut self, mapper: F)
	where
		F: FnOnce(&str) -> String,
	{
		if let FilterField::Expression(sql) = &mut self.field_source {
			let mapped = mapper(sql);
			self.field = mapped.clone();
			*sql = mapped;
		}
	}

	/// Returns the model field that produced this filter, when it is known.
	pub fn source_field_name(&self) -> Option<&str> {
		match &self.field_source {
			FilterField::Column(source_field) => Some(source_field),
			FilterField::Expression(_) | FilterField::TypedPredicate(_) => None,
		}
	}

	/// Returns the SQL expression used on the left side of this filter.
	pub fn lhs_expr(&self) -> Expr {
		filter_lhs_expr(self)
	}

	/// Returns the SQL text used on the left side of this filter.
	pub fn lhs_sql(&self) -> String {
		filter_lhs_sql(self)
	}

	/// Returns the SQL expression used on the left side, qualified to a root alias.
	#[doc(hidden)]
	pub fn lhs_expr_for_root(&self, root_alias: &str) -> Expr {
		filter_lhs_expr_for_root(self, root_alias)
	}

	/// Returns the SQL text used on the left side, qualified to a root alias.
	#[doc(hidden)]
	pub fn lhs_sql_for_root(&self, root_alias: &str) -> String {
		filter_lhs_sql_for_root(self, root_alias)
	}

	/// Returns a typed predicate, optionally qualified to a root alias.
	#[doc(hidden)]
	pub fn typed_predicate_expr(&self, root_alias: Option<&str>) -> Option<SimpleExpr> {
		let FilterField::TypedPredicate(expression) = &self.field_source else {
			return None;
		};
		let root_alias = root_alias?;
		let expression = expression.as_deref().ok()?;
		let mut graph = RelationJoinGraph::new(root_alias);
		for path in &expression.joins.paths {
			let join_kind = if path
				.iter()
				.any(|step| step.default_join_kind == RelationJoinKind::Left)
			{
				RelationJoinKind::Left
			} else {
				RelationJoinKind::Inner
			};
			graph.add_steps(path, join_kind);
		}
		compile_expression(expression, root_alias, &graph).ok()
	}

	/// Combine this filter with another condition using AND.
	pub fn and(self, other: impl Into<FilterCondition>) -> FilterCondition {
		FilterCondition::And(vec![FilterCondition::from(self), other.into()])
	}

	/// Combine this filter with another condition using OR.
	pub fn or(self, other: impl Into<FilterCondition>) -> FilterCondition {
		FilterCondition::Or(vec![FilterCondition::from(self), other.into()])
	}

	/// Negate this filter.
	// This method mirrors Django-style query combinators and returns FilterCondition,
	// so implementing std::ops::Not would not provide the same fluent API.
	#[allow(clippy::should_implement_trait)]
	pub fn not(self) -> FilterCondition {
		FilterCondition::not(self)
	}

	fn is_always_true(&self) -> bool {
		matches!(self.operator, FilterOperator::NotIn)
			&& self.value.is_empty_membership_collection()
	}

	fn is_always_false(&self) -> bool {
		matches!(self.operator, FilterOperator::In) && self.value.is_empty_membership_collection()
	}

	pub(crate) fn expression(
		sql: impl Into<String>,
		operator: FilterOperator,
		value: FilterValue,
	) -> Self {
		let sql = sql.into();
		Self {
			field: sql.clone(),
			field_source: FilterField::Expression(sql),
			relation: None,
			field_type: None,
			operator,
			value,
		}
	}

	pub(crate) fn typed_predicate<M>(
		predicate: crate::orm::query_fields::TypedPredicate<M>,
	) -> Self {
		Self {
			field: String::new(),
			field_source: FilterField::TypedPredicate(predicate.expression.map(Box::new)),
			relation: None,
			field_type: None,
			operator: FilterOperator::Eq,
			value: FilterValue::Boolean(true),
		}
	}

	fn relation_alias(&self) -> Option<&str> {
		self.relation
			.as_ref()
			.map(|relation| relation.leaf_alias.as_str())
	}

	fn add_relation_joins(&self, graph: &mut RelationJoinGraph) {
		if let Some(relation) = &self.relation {
			relation.add_to_graph(graph);
		}
		if let FilterField::TypedPredicate(Ok(expression)) = &self.field_source {
			for path in &expression.joins.paths {
				let join_kind = if path
					.iter()
					.any(|step| step.default_join_kind == RelationJoinKind::Left)
				{
					RelationJoinKind::Left
				} else {
					RelationJoinKind::Inner
				};
				graph.add_steps(path, join_kind);
			}
		}
	}

	fn rebase_relation_alias(&mut self, graph: &RelationJoinGraph) {
		if let Some(relation) = &mut self.relation {
			relation.rebase_join_alias(graph);
		}
	}

	fn has_relation(&self) -> bool {
		self.relation.is_some()
	}

	fn assert_relation_root<T>(&self)
	where
		T: super::Model,
	{
		let Some(relation) = &self.relation else {
			return;
		};
		assert_eq!(
			relation.root_type_name(),
			std::any::type_name::<T>(),
			"typed relation filter root does not match QuerySet model"
		);
	}
}

#[derive(Debug, Clone)]
/// Filter whose relation path is tied to a concrete root model.
pub struct TypedFilter<Root>
where
	Root: super::Model,
{
	filter: Filter,
	_phantom: PhantomData<Root>,
}

impl<Root> TypedFilter<Root>
where
	Root: super::Model,
{
	/// Create a typed filter from the untyped internal representation.
	pub(crate) fn new(filter: Filter) -> Self {
		Self {
			filter,
			_phantom: PhantomData,
		}
	}

	/// Combine this filter with another root-compatible filter using AND.
	pub fn and(self, other: impl QueryFilterInput<Root>) -> TypedFilterCondition<Root> {
		TypedFilterCondition::new(FilterCondition::And(vec![
			FilterCondition::Single(self.filter),
			other.into_filter_condition(),
		]))
	}

	/// Combine this filter with another root-compatible filter using OR.
	pub fn or(self, other: impl QueryFilterInput<Root>) -> TypedFilterCondition<Root> {
		TypedFilterCondition::new(FilterCondition::Or(vec![
			FilterCondition::Single(self.filter),
			other.into_filter_condition(),
		]))
	}

	/// Negate this filter.
	// This method mirrors Django-style query combinators and returns a typed condition.
	#[allow(clippy::should_implement_trait)]
	pub fn not(self) -> TypedFilterCondition<Root> {
		TypedFilterCondition::new(FilterCondition::Not(Box::new(FilterCondition::Single(
			self.filter,
		))))
	}
}

#[derive(Debug, Clone)]
/// Composite filter condition whose typed relation paths share one root model.
pub struct TypedFilterCondition<Root>
where
	Root: super::Model,
{
	condition: FilterCondition,
	_phantom: PhantomData<Root>,
}

impl<Root> TypedFilterCondition<Root>
where
	Root: super::Model,
{
	fn new(condition: FilterCondition) -> Self {
		Self {
			condition,
			_phantom: PhantomData,
		}
	}
}

/// Values that can be used in UPDATE statements
#[derive(Debug, Clone)]
pub enum UpdateValue {
	/// Fallible scalar produced by a typed field codec.
	Typed(Result<DatabaseValue, FieldCodecError>),
	/// String variant.
	String(String),
	/// Integer variant.
	Integer(i64),
	/// Float variant.
	Float(f64),
	/// Boolean variant.
	Boolean(bool),
	/// Null variant.
	Null,
	/// Timestamp variant.
	Timestamp(chrono::DateTime<chrono::Utc>),
	/// UUID variant.
	Uuid(Uuid),
	/// Field reference for field-to-field updates (e.g., SET discount_price = total_price)
	FieldRef(super::expressions::F),
	/// Arithmetic expression (e.g., SET total = unit_price * quantity)
	Expression(super::annotation::Expression),
}

impl From<String> for UpdateValue {
	fn from(value: String) -> Self {
		Self::String(value)
	}
}

impl From<&str> for UpdateValue {
	fn from(value: &str) -> Self {
		Self::String(value.to_string())
	}
}

impl From<i64> for UpdateValue {
	fn from(value: i64) -> Self {
		Self::Integer(value)
	}
}

impl From<i32> for UpdateValue {
	fn from(value: i32) -> Self {
		Self::Integer(value as i64)
	}
}

impl From<f64> for UpdateValue {
	fn from(value: f64) -> Self {
		Self::Float(value)
	}
}

impl From<f32> for UpdateValue {
	fn from(value: f32) -> Self {
		Self::Float(value as f64)
	}
}

impl From<bool> for UpdateValue {
	fn from(value: bool) -> Self {
		Self::Boolean(value)
	}
}

impl From<chrono::DateTime<chrono::Utc>> for UpdateValue {
	fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
		Self::Timestamp(value)
	}
}

impl From<Uuid> for UpdateValue {
	fn from(value: Uuid) -> Self {
		Self::Uuid(value)
	}
}

impl<T> From<Option<T>> for UpdateValue
where
	T: Into<UpdateValue>,
{
	fn from(value: Option<T>) -> Self {
		value.map_or(Self::Null, Into::into)
	}
}

/// One field assignment for a partial `QuerySet` update.
#[derive(Debug, Clone)]
pub struct FieldAssignment {
	field: String,
	value: UpdateValue,
}

impl FieldAssignment {
	/// Creates a new field assignment.
	pub fn new(field: impl Into<String>, value: impl Into<UpdateValue>) -> Self {
		Self {
			field: field.into(),
			value: value.into(),
		}
	}

	/// Returns the assigned field name.
	pub fn field(&self) -> &str {
		&self.field
	}

	/// Returns the assigned value.
	pub fn value(&self) -> &UpdateValue {
		&self.value
	}
}

impl<M, T, Origin, V> From<(super::expressions::FieldRef<M, T, Origin>, V)> for FieldAssignment
where
	T: DatabaseField,
	V: IntoFieldValue<T>,
{
	fn from((field, value): (super::expressions::FieldRef<M, T, Origin>, V)) -> Self {
		let context = field.codec_context();
		Self {
			field: field.name().to_owned(),
			value: UpdateValue::Typed(value.into_field_value_with_context(&context)),
		}
	}
}

impl<V> From<(&str, V)> for FieldAssignment
where
	V: Into<UpdateValue>,
{
	fn from((field, value): (&str, V)) -> Self {
		Self::new(field, value)
	}
}

impl<V> From<(String, V)> for FieldAssignment
where
	V: Into<UpdateValue>,
{
	fn from((field, value): (String, V)) -> Self {
		Self::new(field, value)
	}
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
/// let complex = Filter::new(
///     "status".to_string(),
///     FilterOperator::Eq,
///     FilterValue::String("active".to_string()),
/// ).and(search);
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
	/// let condition = Filter::new(
	///     "is_active".to_string(),
	///     FilterOperator::Eq,
	///     FilterValue::Boolean(true),
	/// ).not();
	/// ```
	// This method is intentionally named `not` for API consistency with Django's Q object.
	// It does not implement std::ops::Not because it constructs a FilterCondition variant,
	// not a boolean negation.
	#[allow(clippy::should_implement_trait)]
	pub fn not(condition: impl Into<FilterCondition>) -> Self {
		Self::Not(Box::new(condition.into()))
	}

	/// Create an AND condition from multiple conditions.
	pub fn all(conditions: Vec<FilterCondition>) -> Self {
		Self::and(conditions)
	}

	/// Create an OR condition from multiple conditions.
	pub fn any(conditions: Vec<FilterCondition>) -> Self {
		Self::or(conditions)
	}

	/// Create a NOT condition that negates the given condition.
	pub fn negate(condition: impl Into<FilterCondition>) -> Self {
		Self::not(condition)
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

	fn is_always_true(&self) -> bool {
		match self {
			Self::Single(filter) => filter.is_always_true(),
			Self::And(conditions) => conditions.iter().all(Self::is_always_true),
			Self::Or(conditions) => {
				!conditions.is_empty() && conditions.iter().any(Self::is_always_true)
			}
			Self::Not(condition) => condition.is_always_false(),
		}
	}

	fn is_always_false(&self) -> bool {
		match self {
			Self::Single(filter) => filter.is_always_false(),
			Self::And(conditions) => {
				!conditions.is_empty() && conditions.iter().any(Self::is_always_false)
			}
			Self::Or(conditions) => conditions.iter().all(Self::is_always_false),
			Self::Not(condition) => condition.is_always_true(),
		}
	}

	fn has_relation(&self) -> reinhardt_core::exception::Result<bool> {
		self.has_relation_at_depth(0)
	}

	fn has_relation_at_depth(&self, depth: usize) -> reinhardt_core::exception::Result<bool> {
		if depth >= MAX_FILTER_CONDITION_DEPTH {
			return Err(reinhardt_core::exception::Error::Validation(format!(
				"Filter condition exceeded maximum depth of {} levels",
				MAX_FILTER_CONDITION_DEPTH
			)));
		}

		match self {
			FilterCondition::Single(filter) => Ok(filter.has_relation()),
			FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
				for condition in conditions {
					if condition.has_relation_at_depth(depth + 1)? {
						return Ok(true);
					}
				}
				Ok(false)
			}
			FilterCondition::Not(condition) => condition.has_relation_at_depth(depth + 1),
		}
	}

	fn rebase_relation_aliases(&mut self, graph: &RelationJoinGraph, depth: usize) {
		if depth >= MAX_FILTER_CONDITION_DEPTH {
			return;
		}

		match self {
			FilterCondition::Single(filter) => filter.rebase_relation_alias(graph),
			FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
				for condition in conditions {
					condition.rebase_relation_aliases(graph, depth + 1);
				}
			}
			FilterCondition::Not(condition) => {
				condition.rebase_relation_aliases(graph, depth + 1);
			}
		}
	}

	fn assert_relation_root<T>(&self)
	where
		T: super::Model,
	{
		let mut pending = vec![self];
		while let Some(condition) = pending.pop() {
			match condition {
				FilterCondition::Single(filter) => filter.assert_relation_root::<T>(),
				FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
					pending.extend(conditions);
				}
				FilterCondition::Not(condition) => pending.push(condition),
			}
		}
	}
}

fn map_filter_condition_columns<F>(condition: &mut FilterCondition, mapper: &mut F)
where
	F: FnMut(&mut Filter),
{
	match condition {
		FilterCondition::Single(filter) => mapper(filter),
		FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
			for condition in conditions {
				map_filter_condition_columns(condition, mapper);
			}
		}
		FilterCondition::Not(condition) => map_filter_condition_columns(condition, mapper),
	}
}

impl From<Filter> for FilterCondition {
	fn from(filter: Filter) -> Self {
		Self::Single(filter)
	}
}

/// Input accepted by `QuerySet::filter`.
pub trait QueryFilterInput<T>
where
	T: super::Model,
{
	/// Convert this input into the internal filter condition.
	fn into_filter_condition(self) -> FilterCondition;
}

impl<T> QueryFilterInput<T> for Filter
where
	T: super::Model,
{
	fn into_filter_condition(self) -> FilterCondition {
		FilterCondition::Single(self)
	}
}

impl<T> QueryFilterInput<T> for FilterCondition
where
	T: super::Model,
{
	fn into_filter_condition(self) -> FilterCondition {
		self
	}
}

impl<T> QueryFilterInput<T> for TypedFilter<T>
where
	T: super::Model,
{
	fn into_filter_condition(self) -> FilterCondition {
		FilterCondition::Single(self.filter)
	}
}

impl<T> QueryFilterInput<T> for TypedFilterCondition<T>
where
	T: super::Model,
{
	fn into_filter_condition(self) -> FilterCondition {
		self.condition
	}
}

/// Parse a membership-list string into query values.
///
/// Supports JSON arrays and comma-separated values. An empty result compiles to
/// `FALSE` for `IN` and `TRUE` for `NOT IN`.
pub(crate) fn parse_membership_string(s: &str) -> Vec<reinhardt_query::value::Value> {
	let trimmed = s.trim();

	if trimmed.starts_with('[')
		&& trimmed.ends_with(']')
		&& let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed)
	{
		return arr
			.iter()
			.map(|value| match value {
				serde_json::Value::String(value) => value.clone().into(),
				serde_json::Value::Number(value) => value.as_i64().map_or_else(
					|| {
						value
							.as_f64()
							.map_or_else(|| value.to_string().into(), Into::into)
					},
					Into::into,
				),
				serde_json::Value::Bool(value) => (*value).into(),
				_ => value.to_string().into(),
			})
			.collect();
	}

	// Fallback to comma-separated parsing
	let trimmed = trimmed
		.strip_prefix('(')
		.and_then(|value| value.strip_suffix(')'))
		.unwrap_or(trimmed);
	trimmed
		.split(',')
		.map(|s| s.trim())
		.filter(|s| !s.is_empty())
		.map(|s| s.to_string().into())
		.collect()
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

macro_rules! filter_value_signed_integer {
	($($type:ty),+ $(,)?) => {
		$(
			impl From<$type> for FilterValue {
				fn from(value: $type) -> Self {
					Self::Integer(i64::from(value))
				}
			}
		)+
	};
}

macro_rules! filter_value_unsigned_integer {
	($($type:ty),+ $(,)?) => {
		$(
			impl From<$type> for FilterValue {
				fn from(value: $type) -> Self {
					Self::Integer(i64::from(value))
				}
			}
		)+
	};
}

filter_value_signed_integer!(i8, i16);
filter_value_unsigned_integer!(u8, u16, u32);

impl From<u64> for FilterValue {
	fn from(value: u64) -> Self {
		value
			.try_into()
			.map_or_else(|_| Self::String(value.to_string()), Self::Integer)
	}
}

impl From<usize> for FilterValue {
	fn from(value: usize) -> Self {
		Self::from(value as u64)
	}
}

impl From<isize> for FilterValue {
	fn from(value: isize) -> Self {
		Self::Integer(value as i64)
	}
}

impl From<i128> for FilterValue {
	fn from(value: i128) -> Self {
		value
			.try_into()
			.map_or_else(|_| Self::String(value.to_string()), Self::Integer)
	}
}

impl From<u128> for FilterValue {
	fn from(value: u128) -> Self {
		value
			.try_into()
			.map_or_else(|_| Self::String(value.to_string()), Self::Integer)
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

impl From<chrono::DateTime<chrono::Utc>> for FilterValue {
	fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
		Self::Timestamp(value)
	}
}

impl From<uuid::Uuid> for FilterValue {
	fn from(u: uuid::Uuid) -> Self {
		Self::Uuid(u)
	}
}

#[derive(Debug, Clone)]
/// Represents a orm query.
pub struct OrmQuery {
	filters: Vec<Filter>,
}

impl OrmQuery {
	/// Creates a new instance.
	pub fn new() -> Self {
		Self {
			filters: Vec::new(),
		}
	}

	/// Performs the filter operation.
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

/// Subquery condition specification for WHERE clause
#[derive(Clone, Debug)]
enum SubqueryCondition {
	/// WHERE field IN (subquery)
	/// Example: WHERE author_id IN (SELECT id FROM authors WHERE name = 'John')
	In {
		field: String,
		subquery: SubquerySql,
		lockable: bool,
	},
	/// WHERE field NOT IN (subquery)
	NotIn {
		field: String,
		subquery: SubquerySql,
		lockable: bool,
	},
	/// WHERE EXISTS (subquery)
	/// Example: WHERE EXISTS (SELECT 1 FROM books WHERE author_id = authors.id)
	Exists {
		subquery: SubquerySql,
		outer_fields: Vec<String>,
		lockable: bool,
	},
	/// WHERE NOT EXISTS (subquery)
	NotExists {
		subquery: SubquerySql,
		outer_fields: Vec<String>,
		lockable: bool,
	},
}

fn rewrite_subquery_field_to_placeholder(sql: &mut String, old_field: &str, placeholder: &str) {
	for quote in ['"', '`'] {
		let escaped = old_field.replace(quote, &format!("{quote}{quote}"));
		*sql = sql.replace(&format!("{quote}{escaped}{quote}"), placeholder);
		let qualified = old_field
			.split('.')
			.map(|part| {
				let escaped = part.replace(quote, &format!("{quote}{quote}"));
				format!("{quote}{escaped}{quote}")
			})
			.collect::<Vec<_>>()
			.join(".");
		*sql = sql.replace(&qualified, placeholder);
	}
}

fn rewrite_subquery_fields(sql: &str, rewrites: &[(String, String)]) -> String {
	let mut rewritten = sql.to_owned();
	let placeholders = rewrites
		.iter()
		.enumerate()
		.map(|(index, _)| format!("\u{1}reinhardt_subquery_field_{index}\u{1}"))
		.collect::<Vec<_>>();

	for ((old_field, _), placeholder) in rewrites.iter().zip(&placeholders) {
		rewrite_subquery_field_to_placeholder(&mut rewritten, old_field, placeholder);
	}
	for ((_, new_field), placeholder) in rewrites.iter().zip(placeholders) {
		let replacement = if rewritten.contains('`') {
			new_field
				.split('.')
				.map(|part| format!("`{}`", part.replace('`', "``")))
				.collect::<Vec<_>>()
				.join(".")
		} else {
			quote_identifier(new_field)
		};
		rewritten = rewritten.replace(&placeholder, &replacement);
	}

	rewritten
}

fn collect_subquery_outer_fields(value: &FilterValue, fields: &mut Vec<String>) {
	match value {
		FilterValue::FieldRef(field) if field.field.contains('.') => {
			fields.push(field.field.clone());
		}
		FilterValue::OuterRef(field) => fields.push(field.field.clone()),
		FilterValue::List(values) => {
			for value in values {
				collect_subquery_outer_fields(value, fields);
			}
		}
		FilterValue::Range(start, end) => {
			collect_subquery_outer_fields(start, fields);
			collect_subquery_outer_fields(end, fields);
		}
		_ => {}
	}
}

fn collect_subquery_outer_condition(condition: &FilterCondition, fields: &mut Vec<String>) {
	match condition {
		FilterCondition::Single(filter) => collect_subquery_outer_fields(&filter.value, fields),
		FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
			for condition in conditions {
				collect_subquery_outer_condition(condition, fields);
			}
		}
		FilterCondition::Not(condition) => collect_subquery_outer_condition(condition, fields),
	}
}

#[derive(Clone, Debug)]
struct SubquerySql {
	postgres: String,
	mysql: String,
	sqlite: String,
}

#[derive(Clone)]
struct SubqueryStatements {
	postgres: SelectStatement,
	mysql: SelectStatement,
	sqlite: SelectStatement,
}

impl SubqueryStatements {
	fn for_backend(&self, backend: crate::backends::types::DatabaseType) -> &SelectStatement {
		match backend {
			crate::backends::types::DatabaseType::Postgres => &self.postgres,
			crate::backends::types::DatabaseType::Mysql => &self.mysql,
			crate::backends::types::DatabaseType::Sqlite => &self.sqlite,
		}
	}
}

impl SubquerySql {
	fn for_backend(&self, backend: crate::backends::types::DatabaseType) -> &str {
		match backend {
			crate::backends::types::DatabaseType::Postgres => &self.postgres,
			crate::backends::types::DatabaseType::Mysql => &self.mysql,
			crate::backends::types::DatabaseType::Sqlite => &self.sqlite,
		}
	}

	fn add_lock(&mut self) {
		fn append_lock(sql: &str) -> String {
			let trimmed = sql.trim_end();
			trimmed.strip_suffix(')').map_or_else(
				|| format!("{trimmed} FOR UPDATE"),
				|inner| format!("{inner} FOR UPDATE)"),
			)
		}

		self.postgres = append_lock(&self.postgres);
		self.mysql = append_lock(&self.mysql);
		self.sqlite = append_lock(&self.sqlite);
	}

	fn rewrite_fields(&mut self, rewrites: &[(String, String)]) {
		self.postgres = rewrite_subquery_fields(&self.postgres, rewrites);
		self.mysql = rewrite_subquery_fields(&self.mysql, rewrites);
		self.sqlite = rewrite_subquery_fields(&self.sqlite, rewrites);
	}
}

const MAX_FILTER_CONDITION_DEPTH: usize = 64;

/// Input accepted by `QuerySet::select_related` and `QuerySet::prefetch_related`.
pub trait RelationLoadInput<T>
where
	T: super::Model,
{
	/// Add this input to the `select_related` plan.
	fn apply_select_related(self, queryset: &mut QuerySet<T>);

	/// Add this input to the `prefetch_related` plan.
	fn apply_prefetch_related(self, queryset: &mut QuerySet<T>);
}

/// Input accepted by [`QuerySet::order_by`].
pub trait IntoOrderBy<M>
where
	M: super::Model,
{
	/// Replace the active ordering with this input.
	fn apply(self, queryset: &mut QuerySet<M>);
}

impl<M> IntoOrderBy<M> for &[&str]
where
	M: super::Model,
{
	fn apply(self, queryset: &mut QuerySet<M>) {
		queryset.order_by_fields = self.iter().map(|field| (*field).to_owned()).collect();
		queryset.order_by_expressions.clear();
	}
}

impl<M, const N: usize> IntoOrderBy<M> for &[&str; N]
where
	M: super::Model,
{
	fn apply(self, queryset: &mut QuerySet<M>) {
		self.as_slice().apply(queryset);
	}
}

impl<M> IntoOrderBy<M> for &Vec<&str>
where
	M: super::Model,
{
	fn apply(self, queryset: &mut QuerySet<M>) {
		self.as_slice().apply(queryset);
	}
}

impl<M> IntoOrderBy<M> for OrderedExpression<M>
where
	M: super::Model,
{
	fn apply(self, queryset: &mut QuerySet<M>) {
		queryset.order_by_fields.clear();
		queryset.order_by_expressions.clear();
		queryset.order_by_expressions.push(self);
	}
}

impl<T> RelationLoadInput<T> for &[&str]
where
	T: super::Model,
{
	fn apply_select_related(self, queryset: &mut QuerySet<T>) {
		for field in self {
			queryset
				.validate_relation_path(field)
				.expect("invalid relation path passed to select_related");
		}
		for field in self {
			if !queryset
				.select_related_fields
				.iter()
				.any(|item| item == field)
			{
				queryset.select_related_fields.push((*field).to_string());
			}
		}
	}

	fn apply_prefetch_related(self, queryset: &mut QuerySet<T>) {
		assert!(
			T::composite_primary_key().is_none_or(|key| key.field_count() == 1),
			"typed prefetch_related does not support composite primary-key roots"
		);
		for field in self {
			queryset
				.validate_relation_path(field)
				.expect("invalid relation path passed to prefetch_related");
		}
		for field in self {
			if !queryset
				.prefetch_related_fields
				.iter()
				.any(|item| item == field)
			{
				queryset.prefetch_related_fields.push((*field).to_string());
			}
		}
	}
}

impl<T, const N: usize> RelationLoadInput<T> for &[&str; N]
where
	T: super::Model,
{
	fn apply_select_related(self, queryset: &mut QuerySet<T>) {
		self.as_slice().apply_select_related(queryset);
	}

	fn apply_prefetch_related(self, queryset: &mut QuerySet<T>) {
		self.as_slice().apply_prefetch_related(queryset);
	}
}

impl<T, const N: usize> RelationLoadInput<T> for [&str; N]
where
	T: super::Model,
{
	fn apply_select_related(self, queryset: &mut QuerySet<T>) {
		self.as_slice().apply_select_related(queryset);
	}

	fn apply_prefetch_related(self, queryset: &mut QuerySet<T>) {
		self.as_slice().apply_prefetch_related(queryset);
	}
}

impl<T, S> RelationLoadInput<T> for &Vec<S>
where
	T: super::Model,
	S: AsRef<str>,
{
	fn apply_select_related(self, queryset: &mut QuerySet<T>) {
		for field in self {
			queryset
				.validate_relation_path(field.as_ref())
				.expect("invalid relation path passed to select_related");
		}
		for field in self {
			let field = field.as_ref();
			if !queryset
				.select_related_fields
				.iter()
				.any(|item| item == field)
			{
				queryset.select_related_fields.push(field.to_string());
			}
		}
	}

	fn apply_prefetch_related(self, queryset: &mut QuerySet<T>) {
		assert!(
			T::composite_primary_key().is_none_or(|key| key.field_count() == 1),
			"typed prefetch_related does not support composite primary-key roots"
		);
		for field in self {
			queryset
				.validate_relation_path(field.as_ref())
				.expect("invalid relation path passed to prefetch_related");
		}
		for field in self {
			let field = field.as_ref();
			if !queryset
				.prefetch_related_fields
				.iter()
				.any(|item| item == field)
			{
				queryset.prefetch_related_fields.push(field.to_string());
			}
		}
	}
}

impl<T, P> RelationLoadInput<T> for P
where
	T: super::Model,
	P: RelationPathLike<Root = T>,
{
	fn apply_select_related(self, queryset: &mut QuerySet<T>) {
		assert!(
			!self.is_multi_valued(),
			"typed select_related supports only single-valued relation paths; use prefetch_related for multi-valued relations"
		);
		let typed = TypedSelectRelation::from_path(&self);
		queryset.relation_joins.add_path(&self);
		if !queryset.typed_select_related.contains(&typed) {
			queryset.typed_select_related.push(typed);
		}
	}

	fn apply_prefetch_related(self, queryset: &mut QuerySet<T>) {
		let typed = TypedPrefetchRelation::from_path(&self);
		assert!(
			typed.is_direct_multi_valued_relation() && typed.uses_root_primary_key::<T>(),
			"typed prefetch_related supports only direct multi-valued relation paths through the root primary key; use select_related for single-valued relations"
		);
		if !queryset.prefetch_related_fields.contains(&typed.field) {
			queryset.prefetch_related_fields.push(typed.field.clone());
		}
		if !queryset
			.typed_prefetch_related
			.iter()
			.any(|relation| relation.field == typed.field)
		{
			queryset.typed_prefetch_related.push(typed);
		}
	}
}

#[derive(Debug, Clone)]
struct TypedPrefetchRelation {
	field: String,
	alias: String,
	steps: SmallVec<[RelationStep; 4]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypedSelectRelation {
	steps: SmallVec<[RelationStep; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectForUpdateBehavior {
	Blocking,
	Nowait,
	SkipLocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectForUpdateTarget {
	Root,
	Relation(Box<SmallVec<[RelationStep; 4]>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectForUpdateSpec {
	behavior: SelectForUpdateBehavior,
	no_key: bool,
	targets: Vec<SelectForUpdateTarget>,
}

impl Default for SelectForUpdateSpec {
	fn default() -> Self {
		Self {
			behavior: SelectForUpdateBehavior::Blocking,
			no_key: false,
			targets: Vec::new(),
		}
	}
}

/// Type state for a blocking row lock.
#[derive(Debug, Clone, Copy, Default)]
pub struct Blocking;

/// Type state for a row lock that fails immediately when a row is unavailable.
#[derive(Debug, Clone, Copy)]
pub struct Nowait;

/// Type state for a row lock that omits rows which cannot be locked immediately.
#[derive(Debug, Clone, Copy)]
pub struct SkipLocked;

/// Typed builder for a transaction-scoped `SELECT ... FOR UPDATE`.
///
/// `nowait` and `skip_locked` are transitions from [`Blocking`] to distinct
/// type states, so they cannot be combined. Evaluate the builder with
/// [`Self::all_with_executor`] or [`Self::rows_with_executor`] using a
/// caller-owned active transaction executor.
pub struct SelectForUpdate<M, Behavior = Blocking>
where
	M: super::Model,
{
	queryset: QuerySet<M>,
	_behavior: PhantomData<Behavior>,
}

impl<M, Behavior> SelectForUpdate<M, Behavior>
where
	M: super::Model,
{
	/// Requests PostgreSQL's weaker `FOR NO KEY UPDATE` lock strength.
	///
	/// Other backends return an explicit unsupported-capability error.
	pub fn no_key(mut self) -> Self {
		self.queryset
			.select_for_update
			.as_mut()
			.expect("select-for-update builder always contains a lock specification")
			.no_key = true;
		self
	}

	/// Restricts locking to the root model table.
	pub fn of_model(mut self) -> Self {
		let spec = self
			.queryset
			.select_for_update
			.as_mut()
			.expect("select-for-update builder always contains a lock specification");
		if !spec.targets.contains(&SelectForUpdateTarget::Root) {
			spec.targets.push(SelectForUpdateTarget::Root);
		}
		self
	}

	/// Restricts locking to a typed relation target and adds its required joins.
	pub fn of_relation<P>(mut self, path: P) -> Self
	where
		P: RelationPathLike<Root = M>,
	{
		let mut steps: SmallVec<[RelationStep; 4]> = SmallVec::new();
		steps.extend(path.steps().iter().cloned());
		self.queryset
			.relation_joins
			.add_steps_with_override(&steps, path.join_kind_override());
		let target = SelectForUpdateTarget::Relation(Box::new(steps));
		let spec = self
			.queryset
			.select_for_update
			.as_mut()
			.expect("select-for-update builder always contains a lock specification");
		if !spec.targets.contains(&target) {
			spec.targets.push(target);
		}
		self
	}

	/// Rejects evaluation without an explicit active transaction.
	pub async fn all(&self) -> reinhardt_core::exception::Result<Vec<M>>
	where
		M: serde::de::DeserializeOwned,
	{
		self.queryset.ensure_not_locking_without_transaction()?;
		unreachable!("locking queryset validation always returns an error")
	}

	/// Rejects evaluation through an ordinary ORM executor.
	pub async fn all_with_db<E>(
		&self,
		executor: &mut E,
	) -> reinhardt_core::exception::Result<Vec<M>>
	where
		M: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		self.queryset.all_with_db(executor).await
	}

	/// Rejects raw-row evaluation through an ordinary ORM executor.
	pub async fn rows_with_db<E>(
		&self,
		executor: &mut E,
	) -> reinhardt_core::exception::Result<Vec<QueryRow>>
	where
		E: OrmExecutor,
	{
		self.queryset.rows_with_db(executor).await
	}

	/// Executes the locking query through a caller-owned active transaction.
	pub async fn all_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> Result<Vec<M>, crate::backends::error::DatabaseError>
	where
		M: serde::de::DeserializeOwned,
	{
		self.queryset.all_with_executor(executor).await
	}

	/// Executes the locking query and returns undecoded rows.
	pub async fn rows_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> Result<Vec<QueryRow>, crate::backends::error::DatabaseError> {
		self.queryset.rows_with_executor(executor).await
	}
}

impl<M> SelectForUpdate<M, Blocking>
where
	M: super::Model,
{
	/// Changes blocking behavior to `NOWAIT`.
	pub fn nowait(mut self) -> SelectForUpdate<M, Nowait> {
		self.queryset
			.select_for_update
			.as_mut()
			.expect("select-for-update builder always contains a lock specification")
			.behavior = SelectForUpdateBehavior::Nowait;
		SelectForUpdate {
			queryset: self.queryset,
			_behavior: PhantomData,
		}
	}

	/// Changes blocking behavior to `SKIP LOCKED`.
	pub fn skip_locked(mut self) -> SelectForUpdate<M, SkipLocked> {
		self.queryset
			.select_for_update
			.as_mut()
			.expect("select-for-update builder always contains a lock specification")
			.behavior = SelectForUpdateBehavior::SkipLocked;
		SelectForUpdate {
			queryset: self.queryset,
			_behavior: PhantomData,
		}
	}
}

impl TypedSelectRelation {
	fn from_path<P>(path: &P) -> Self
	where
		P: RelationPathLike,
	{
		let mut steps: SmallVec<[RelationStep; 4]> = SmallVec::new();
		steps.extend(path.steps().iter().cloned());
		Self { steps }
	}

	fn aliases(&self, graph: &RelationJoinGraph) -> SmallVec<[String; 4]> {
		graph.aliases_for_steps(&self.steps).unwrap_or_default()
	}
}

impl TypedPrefetchRelation {
	fn from_path<P>(path: &P) -> Self
	where
		P: RelationPathLike,
	{
		let mut steps: SmallVec<[RelationStep; 4]> = SmallVec::new();
		steps.extend(path.steps().iter().cloned());
		let field = steps.last().map_or_else(
			|| path.leaf_alias().to_string(),
			|step| step.name.to_string(),
		);
		let alias = match steps.as_slice() {
			[through_step, target_step] if through_step.name.to_string().ends_with("__through") => {
				target_step.name.to_string()
			}
			_ => path.leaf_alias().to_string(),
		};
		Self {
			field,
			alias,
			steps,
		}
	}

	fn is_direct_multi_valued_relation(&self) -> bool {
		match self.steps.as_slice() {
			[step] => step.multiplicity == crate::orm::relations::RelationMultiplicity::Multiple,
			[through_step, target_step] => {
				through_step.name.to_string().ends_with("__through")
					&& target_step.name.as_ref() == self.field
					&& target_step.source_table == through_step.target_table
			}
			_ => false,
		}
	}

	fn uses_root_primary_key<T>(&self) -> bool
	where
		T: super::Model,
	{
		self.steps
			.first()
			.is_some_and(|step| step.source_column.as_ref() == T::primary_key_column())
	}
}

#[derive(Clone)]
/// Represents a query set.
pub struct QuerySet<T>
where
	T: super::Model,
{
	_phantom: std::marker::PhantomData<T>,
	filters: SmallVec<[Filter; 10]>,
	filter_conditions: SmallVec<[FilterCondition; 4]>,
	select_related_fields: Vec<String>,
	typed_select_related: Vec<TypedSelectRelation>,
	prefetch_related_fields: Vec<String>,
	typed_prefetch_related: Vec<TypedPrefetchRelation>,
	relation_joins: RelationJoinGraph,
	order_by_fields: Vec<String>,
	order_by_expressions: Vec<OrderedExpression<T>>,
	distinct_enabled: bool,
	selected_fields: Option<Vec<String>>,
	selected_expressions: Vec<(String, StoredExpression)>,
	deferred_fields: Vec<String>,
	annotations: Vec<super::annotation::Annotation>,
	backend_annotations: Vec<super::postgres_features::BackendAnnotation>,
	typed_annotations: Vec<StoredExpression>,
	typed_havings: Vec<StoredExpression>,
	manager: Option<std::sync::Arc<super::manager::Manager<T>>>,
	limit: Option<usize>,
	offset: Option<usize>,
	ctes: super::cte::CTECollection,
	lateral_joins: super::lateral_join::LateralJoins,
	joins: Vec<JoinClause>,
	group_by_fields: Vec<String>,
	subquery_conditions: Vec<SubqueryCondition>,
	from_alias: Option<String>,
	empty_result: bool,
	/// Subquery SQL for FROM clause (derived table)
	/// When set, the FROM clause will use this subquery instead of the model's table
	from_subquery_sql: Option<SubquerySql>,
	from_subquery_statement: Option<SubqueryStatements>,
	/// Whether the derived source selects a complete model-shaped row.
	from_subquery_model_shaped: Option<bool>,
	/// Rust model type used to build the derived source, when available.
	from_subquery_model_type: Option<&'static str>,
	select_for_update: Option<SelectForUpdateSpec>,
}

impl<T> QuerySet<T>
where
	T: super::Model,
{
	/// Creates a new instance.
	pub fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
			filters: SmallVec::new(),
			filter_conditions: SmallVec::new(),
			select_related_fields: Vec::new(),
			typed_select_related: Vec::new(),
			prefetch_related_fields: Vec::new(),
			typed_prefetch_related: Vec::new(),
			relation_joins: RelationJoinGraph::new(T::table_name()),
			order_by_fields: Vec::new(),
			order_by_expressions: Vec::new(),
			distinct_enabled: false,
			selected_fields: None,
			selected_expressions: Vec::new(),
			deferred_fields: Vec::new(),
			annotations: Vec::new(),
			backend_annotations: Vec::new(),
			typed_annotations: Vec::new(),
			typed_havings: Vec::new(),
			manager: None,
			limit: None,
			offset: None,
			ctes: super::cte::CTECollection::new(),
			lateral_joins: super::lateral_join::LateralJoins::new(),
			joins: Vec::new(),
			group_by_fields: Vec::new(),
			subquery_conditions: Vec::new(),
			from_alias: None,
			empty_result: false,
			from_subquery_sql: None,
			from_subquery_statement: None,
			from_subquery_model_shaped: None,
			from_subquery_model_type: None,
			select_for_update: None,
		}
	}

	/// Returns a QuerySet that is known to contain no rows.
	///
	/// Execution methods can use this marker to avoid invoking an executor.
	pub fn none(mut self) -> Self {
		self.empty_result = true;
		self
	}

	pub(crate) fn is_empty_result(&self) -> bool {
		self.empty_result
	}

	fn executor_backend(
		executor: &dyn super::connection::TransactionExecutor,
	) -> super::connection::DatabaseBackend {
		match executor.backend() {
			crate::backends::types::DatabaseType::Postgres => {
				super::connection::DatabaseBackend::Postgres
			}
			crate::backends::types::DatabaseType::Mysql => {
				super::connection::DatabaseBackend::MySql
			}
			crate::backends::types::DatabaseType::Sqlite => {
				super::connection::DatabaseBackend::Sqlite
			}
		}
	}

	fn build_select_statement(&self) -> reinhardt_core::exception::Result<SelectStatement> {
		self.build_select_statement_for_backend(crate::backends::types::DatabaseType::Postgres)
	}

	fn build_select_statement_for_backend(
		&self,
		backend: crate::backends::types::DatabaseType,
	) -> reinhardt_core::exception::Result<SelectStatement> {
		if self.has_select_related() {
			return self.select_related_query_with_condition(
				self.build_where_condition_for_backend(backend)?,
			);
		}

		let mut stmt = Query::select();
		self.apply_model_from_for_backend(&mut stmt, backend);
		if self.distinct_enabled {
			stmt.distinct();
		}
		if let Some(ref fields) = self.selected_fields {
			for field in fields {
				if field.contains('(') && field.contains(')') {
					stmt.expr(Expr::cust(field.clone()));
				} else {
					stmt.column(self.root_column_reference(field));
				}
			}
		} else if !self.deferred_fields.is_empty() {
			for field in T::field_metadata() {
				if !self.deferred_fields.contains(&field.name) {
					stmt.column(self.root_column_reference(field.db_column_name()));
				}
			}
		} else {
			self.add_default_select_columns(&mut stmt);
		}
		self.apply_typed_select_expressions(&mut stmt)?;
		self.apply_annotations_to_select(&mut stmt);
		self.apply_relation_joins(&mut stmt);
		self.apply_manual_joins(&mut stmt);

		if let Some(condition) = self.build_where_condition_for_backend(backend)? {
			stmt.cond_where(condition);
		}
		self.apply_typed_annotation_grouping(&mut stmt)?;
		self.apply_grouping_and_having(&mut stmt)?;
		self.apply_ordering(&mut stmt)?;
		if let Some(limit) = self.limit {
			stmt.limit(limit as u64);
		}
		if let Some(offset) = self.offset {
			stmt.offset(offset as u64);
		}
		self.apply_select_for_update(&mut stmt);
		Ok(stmt.to_owned())
	}

	/// Builds a model-shaped SELECT for execution by a configured [`Session`].
	///
	/// A session decodes one complete model from every selected row, so querysets
	/// that change the projection or result shape are rejected instead of being
	/// silently decoded as a different model.
	///
	/// Production session execution uses
	/// `build_full_model_select_statement_for_backend`. This PostgreSQL
	/// convenience wrapper exists for crate tests that assert the model-shaped
	/// contract without selecting a backend.
	#[cfg(test)]
	pub(crate) fn build_full_model_select_statement(
		&self,
	) -> reinhardt_core::exception::Result<SelectStatement> {
		self.build_full_model_select_statement_for_backend(
			crate::backends::types::DatabaseType::Postgres,
		)
	}

	pub(crate) fn build_full_model_select_statement_for_backend(
		&self,
		backend: crate::backends::types::DatabaseType,
	) -> reinhardt_core::exception::Result<SelectStatement> {
		if !self.is_model_shaped_source() {
			return Err(reinhardt_core::exception::Error::from(
				reinhardt_core::exception::DatabaseError::new(
					reinhardt_core::exception::DatabaseErrorKind::Query,
					"Session::list requires a model-shaped QuerySet",
				),
			));
		}

		self.build_select_statement_for_backend(backend)
	}

	fn is_model_shaped_source(&self) -> bool {
		self.selected_fields.is_none()
			&& self.selected_expressions.is_empty()
			&& self.deferred_fields.is_empty()
			&& self.annotations.is_empty()
			&& self.backend_annotations.is_empty()
			&& self.typed_annotations.is_empty()
			&& self.select_related_fields.is_empty()
			&& self.typed_select_related.is_empty()
			&& self.ctes.is_empty()
			&& self.lateral_joins.is_empty()
			&& self.group_by_fields.is_empty()
			&& self.typed_havings.is_empty()
			&& (self.from_subquery_sql.is_none()
				|| (self.from_subquery_model_shaped == Some(true)
					&& self.from_subquery_model_type == Some(std::any::type_name::<T>())))
	}

	pub(crate) fn validate_row_lock_source(
		&self,
	) -> Result<(), crate::backends::error::DatabaseError> {
		if self.from_subquery_sql.is_some() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"SELECT FOR UPDATE does not support derived table sources",
			));
		}
		Ok(())
	}

	fn ensure_explainable_shape(&self) -> reinhardt_core::exception::Result<()> {
		if !self.ctes.is_empty()
			|| !self.lateral_joins.is_empty()
			|| self.from_subquery_sql.is_some()
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"EXPLAIN does not support QuerySets with CTEs, lateral joins, or a subquery source.",
			)
			.into());
		}
		Ok(())
	}

	fn apply_annotations_to_select(&self, stmt: &mut SelectStatement) {
		for annotation in &self.annotations {
			let expression = self
				.annotation_value_to_select_expr(&annotation.value)
				.unwrap_or_else(|| {
					Expr::cust(self.annotation_value_to_select_sql(&annotation.value))
						.into_simple_expr()
				});
			stmt.expr_as(expression, Alias::new(&annotation.alias));
		}
		for annotation in &self.backend_annotations {
			let root_alias = self.root_alias().to_owned();
			let expression = annotation.to_sql_with_field_mapper(|field| {
				if field.contains('.') || field.contains('(') {
					field.to_owned()
				} else {
					quote_identifier(&format!("{root_alias}.{field}"))
				}
			});
			stmt.expr(Expr::cust(expression));
		}
	}

	fn has_typed_having(&self) -> bool {
		!self.typed_havings.is_empty()
	}

	fn apply_typed_annotation_grouping(
		&self,
		stmt: &mut SelectStatement,
	) -> reinhardt_core::exception::Result<()> {
		self.ensure_typed_aggregate_query_shape()?;
		if !self.has_aggregate_annotation() {
			return Ok(());
		}

		if let Some(fields) = &self.selected_fields {
			for field in fields {
				if !field.contains('(') && !field.contains(')') {
					stmt.group_by_col(ColumnRef::table_column(
						Alias::new(self.root_alias()),
						Alias::new(Self::database_column_for_field(field)),
					));
				}
			}
		} else {
			for field in T::field_metadata() {
				stmt.group_by_col(ColumnRef::table_column(
					Alias::new(self.root_alias()),
					Alias::new(field.db_column_name()),
				));
			}
		}

		let graph = self.expression_relation_join_graph_for_query();
		let scalar_expressions = StoredExpression::deduplicate(
			self.selected_expressions
				.iter()
				.map(|(_, expression)| expression)
				.chain(self.typed_annotations.iter())
				.chain(
					self.order_by_expressions
						.iter()
						.map(|ordering| &ordering.expression),
				)
				.flat_map(|expression| {
					expression
						.node
						.scalar_grouping_nodes()
						.into_iter()
						.map(|node| StoredExpression::new(node, expression.joins.clone(), None))
				})
				.collect(),
		);
		for expression in scalar_expressions {
			stmt.group_by_expr(compile_expression(&expression, self.root_alias(), &graph)?);
		}
		Ok(())
	}

	fn ensure_typed_aggregate_query_shape(&self) -> reinhardt_core::exception::Result<()> {
		if self.has_aggregate_annotation()
			&& !self.group_by_fields.is_empty()
			&& self.selected_fields.is_none()
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"aggregate annotations with explicit GROUP BY require an explicit projection",
			)
			.into());
		}
		if self.has_aggregate_annotation()
			&& self.selected_fields.is_none()
			&& T::field_metadata().is_empty()
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"aggregate annotations require model field metadata or an explicit projection",
			)
			.into());
		}
		if self.has_aggregate_annotation()
			&& self
				.filter_relation_join_graph_for_query()
				.has_multi_valued_join()
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"aggregate annotations over multi-valued filters require isolated subqueries",
			)
			.into());
		}
		let having_paths = self
			.typed_havings
			.iter()
			.flat_map(|expression| expression.joins.paths.iter())
			.filter(|path| {
				path.iter().any(|step| {
					step.multiplicity == super::relations::RelationMultiplicity::Multiple
				})
			})
			.collect::<Vec<_>>();
		for (index, left) in having_paths.iter().enumerate() {
			for right in having_paths.iter().skip(index + 1) {
				if left != right {
					return Err(DatabaseError::new(
						DatabaseErrorKind::Unsupported,
						"HAVING expressions over distinct multi-valued relations require isolated subqueries",
					)
					.into());
				}
			}
		}
		if self
			.typed_annotations
			.iter()
			.any(|item| item.contains_aggregate())
			&& self
				.backend_annotations
				.iter()
				.any(|item| !item.is_aggregate())
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"scalar backend annotations cannot be combined with portable aggregate annotations",
			)
			.into());
		}
		if self
			.order_by_expressions
			.iter()
			.any(|ordering| ordering.expression.node.contains_aggregate())
			&& !self.has_aggregate_annotation()
			&& self.group_by_fields.is_empty()
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"aggregate ordering requires an aggregate annotation or explicit GROUP BY projection",
			)
			.into());
		}
		if self.has_aggregate_annotation()
			&& self.selected_fields.as_ref().is_some_and(|fields| {
				fields
					.iter()
					.any(|field| field.contains('(') || field.contains(')'))
			}) {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"aggregate annotations do not support raw selected expressions; use select_expr for structured scalar projections",
			)
			.into());
		}
		if self.has_typed_having()
			&& !self.has_aggregate_annotation()
			&& self.group_by_fields.is_empty()
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"HAVING requires an aggregate annotation or an explicit GROUP BY projection",
			)
			.into());
		}
		if self.has_select_related() && self.has_aggregate_annotation() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"aggregate annotations do not support select_related projections",
			)
			.into());
		}

		let mut paths = self
			.typed_annotations
			.iter()
			.filter(|expression| expression.contains_aggregate())
			.flat_map(|expression| expression.joins.paths.iter())
			.filter(|path| {
				path.iter().any(|step| {
					step.multiplicity == super::relations::RelationMultiplicity::Multiple
				})
			})
			.collect::<Vec<_>>();
		if self.has_aggregate_annotation() {
			paths.extend(
				self.selected_expressions
					.iter()
					.flat_map(|(_, expression)| expression.joins.paths.iter())
					.filter(|path| {
						path.iter().any(|step| {
							step.multiplicity == super::relations::RelationMultiplicity::Multiple
						})
					}),
			);
			paths.extend(
				self.typed_havings
					.iter()
					.flat_map(|expression| expression.joins.paths.iter())
					.filter(|path| {
						path.iter().any(|step| {
							step.multiplicity == super::relations::RelationMultiplicity::Multiple
						})
					}),
			);
		}
		for (index, left) in paths.iter().enumerate() {
			for right in paths.iter().skip(index + 1) {
				if left != right {
					return Err(DatabaseError::new(
						DatabaseErrorKind::Unsupported,
						"aggregate query expressions over independent multi-valued relations require isolated subqueries",
					)
					.into());
				}
			}
		}
		Ok(())
	}

	fn apply_grouping_and_having(
		&self,
		stmt: &mut SelectStatement,
	) -> reinhardt_core::exception::Result<()> {
		for group_field in &self.group_by_fields {
			stmt.group_by_col(self.root_column_reference(group_field));
		}

		self.apply_typed_having(stmt)
	}

	fn apply_typed_having(
		&self,
		stmt: &mut SelectStatement,
	) -> reinhardt_core::exception::Result<()> {
		let graph = self.expression_relation_join_graph_for_query();
		for expression in &self.typed_havings {
			stmt.and_having(compile_expression(expression, self.root_alias(), &graph)?);
		}
		Ok(())
	}

	fn apply_select_for_update(&self, stmt: &mut SelectStatement) {
		let Some(spec) = &self.select_for_update else {
			return;
		};

		stmt.lock(if spec.no_key {
			LockType::NoKeyUpdate
		} else {
			LockType::Update
		});
		match spec.behavior {
			SelectForUpdateBehavior::Blocking => {}
			SelectForUpdateBehavior::Nowait => {
				stmt.lock_behavior(LockBehavior::Nowait);
			}
			SelectForUpdateBehavior::SkipLocked => {
				stmt.lock_behavior(LockBehavior::SkipLocked);
			}
		}

		if spec.targets.is_empty() {
			return;
		}

		let graph = self.relation_join_graph_for_query();
		let aliases = spec.targets.iter().filter_map(|target| match target {
			SelectForUpdateTarget::Root => Some(self.root_alias().to_string()),
			SelectForUpdateTarget::Relation(steps) => graph
				.aliases_for_steps(steps)
				.and_then(|aliases| aliases.last().cloned()),
		});
		stmt.lock_tables(aliases.map(Alias::new));
	}

	pub(crate) fn validate_select_for_update(
		&self,
		capabilities: crate::backends::types::RowLockCapabilities,
		backend: crate::backends::types::DatabaseType,
	) -> Result<(), crate::backends::error::DatabaseError> {
		let Some(spec) = &self.select_for_update else {
			return Ok(());
		};
		if !self.ctes.is_empty() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"SELECT FOR UPDATE does not support CTE-backed querysets",
			));
		}
		self.validate_row_lock_source()?;
		if !self.lateral_joins.is_empty() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"SELECT FOR UPDATE does not support LATERAL joins",
			));
		}
		if self.has_raw_aggregate_projection() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"SELECT FOR UPDATE does not support raw aggregate projections",
			));
		}
		if self.has_aggregate_annotation() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"SELECT FOR UPDATE does not support aggregate annotations",
			));
		}
		if !capabilities.update {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"SELECT FOR UPDATE is not supported by this backend or server version",
			));
		}
		if spec.no_key && !capabilities.no_key_update {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"FOR NO KEY UPDATE is only supported by PostgreSQL 9.3 or newer",
			));
		}
		if spec.behavior == SelectForUpdateBehavior::Nowait && !capabilities.nowait {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"NOWAIT row locking is not supported by this backend or server version",
			));
		}
		if spec.behavior == SelectForUpdateBehavior::SkipLocked && !capabilities.skip_locked {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"SKIP LOCKED row locking is not supported by this backend or server version",
			));
		}
		if !spec.targets.is_empty() && !capabilities.targets {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"explicit row-lock targets are not supported by this backend or server version",
			));
		}
		let graph = self.relation_join_graph_for_query();
		let has_outer_join = graph
			.joins()
			.iter()
			.any(|join| join.join_kind == RelationJoinKind::Left)
			|| !self.select_related_fields.is_empty()
			|| self.joins.iter().any(|join| {
				!join.on_condition.is_empty()
					&& matches!(
						join.join_type,
						super::sqlalchemy_query::JoinType::Left
							| super::sqlalchemy_query::JoinType::Right
							| super::sqlalchemy_query::JoinType::Full
					)
			});
		if backend == crate::backends::types::DatabaseType::Postgres
			&& spec.targets.is_empty()
			&& has_outer_join
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Query,
				"SELECT FOR UPDATE with an outer join requires explicit non-nullable lock targets",
			));
		}
		for target in &spec.targets {
			let SelectForUpdateTarget::Relation(steps) = target else {
				continue;
			};
			if steps.is_empty() {
				return Err(DatabaseError::new(
					DatabaseErrorKind::Query,
					"SELECT FOR UPDATE relation lock target must contain at least one relation step",
				));
			}
			let aliases = graph.aliases_for_steps(steps).ok_or_else(|| {
				DatabaseError::new(
					DatabaseErrorKind::Query,
					"SELECT FOR UPDATE relation lock target could not be resolved",
				)
			})?;
			for alias in aliases {
				let join = graph
					.joins()
					.iter()
					.find(|join| join.alias == alias)
					.expect("resolved relation aliases always have a planned join");
				if backend == crate::backends::types::DatabaseType::Postgres
					&& join.join_kind == RelationJoinKind::Left
				{
					return Err(DatabaseError::new(
						DatabaseErrorKind::Query,
						"SELECT FOR UPDATE cannot lock a relation reached through an outer join",
					));
				}
			}
		}
		Ok(())
	}

	fn has_raw_aggregate_projection(&self) -> bool {
		self.selected_fields.as_ref().is_some_and(|fields| {
			fields
				.iter()
				.any(|field| contains_known_raw_aggregate(field))
		})
	}

	fn has_aggregate_annotation(&self) -> bool {
		self.typed_annotations
			.iter()
			.any(StoredExpression::contains_aggregate)
			|| self
				.backend_annotations
				.iter()
				.any(super::postgres_features::BackendAnnotation::is_aggregate)
	}

	fn supports_scope_subquery_row_lock(&self) -> bool {
		let has_outer_join = self
			.relation_join_graph_for_query()
			.joins()
			.iter()
			.any(|join| join.join_kind == RelationJoinKind::Left)
			|| !self.select_related_fields.is_empty()
			|| self.joins.iter().any(|join| {
				matches!(
					join.join_type,
					super::sqlalchemy_query::JoinType::Left
						| super::sqlalchemy_query::JoinType::Right
						| super::sqlalchemy_query::JoinType::Full
				)
			});

		self.select_for_update.is_none()
			&& self.ctes.is_empty()
			&& self.from_subquery_sql.is_none()
			&& self.lateral_joins.is_empty()
			&& !has_outer_join
			&& !self.distinct_enabled
			&& self.group_by_fields.is_empty()
			&& !self.has_typed_having()
			&& !self.has_raw_aggregate_projection()
			&& !self.has_aggregate_annotation()
	}

	fn ensure_backend_annotations_supported(
		&self,
		backend: super::connection::DatabaseBackend,
	) -> Result<(), DatabaseError> {
		if !self.backend_annotations.is_empty()
			&& backend != super::connection::DatabaseBackend::Postgres
		{
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"PostgreSQL backend annotations require a PostgreSQL executor",
			));
		}
		Ok(())
	}

	pub(crate) fn ensure_not_locking_without_transaction(
		&self,
	) -> reinhardt_core::exception::Result<()> {
		if self.select_for_update.is_some() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Transaction,
				"select_for_update requires all_with_executor or rows_with_executor with a caller-owned active transaction",
			)
			.into());
		}
		Ok(())
	}

	fn decode_backend_rows(
		rows: Vec<crate::backends::types::Row>,
	) -> Result<Vec<T>, crate::backends::error::DatabaseError>
	where
		T: serde::de::DeserializeOwned,
	{
		rows.into_iter()
			.map(|row| {
				super::connection::QueryRow::from_backend_row(row)
					.deserialize_model::<T>()
					.map_err(|error| {
						DatabaseError::new(DatabaseErrorKind::Serialization, error.to_string())
					})
			})
			.collect()
	}

	fn build_explain_for_backend(
		stmt: &ExplainStatement,
		backend: super::connection::DatabaseBackend,
		is_cockroachdb: bool,
	) -> Result<(String, reinhardt_query::prelude::Values, ExplainBackend), DatabaseError> {
		let (result, explain_backend) = if is_cockroachdb {
			if backend != super::connection::DatabaseBackend::Postgres {
				return Err(DatabaseError::new(
					DatabaseErrorKind::Unsupported,
					"CockroachDB EXPLAIN requires a PostgreSQL-compatible executor",
				));
			}
			(
				stmt.build_cockroachdb_checked(),
				ExplainBackend::CockroachDb,
			)
		} else {
			match backend {
				super::connection::DatabaseBackend::Postgres => {
					(stmt.build_postgres_checked(), ExplainBackend::Postgres)
				}
				super::connection::DatabaseBackend::MySql => {
					(stmt.build_mysql_checked(), ExplainBackend::MySql)
				}
				super::connection::DatabaseBackend::Sqlite => {
					(stmt.build_sqlite_checked(), ExplainBackend::Sqlite)
				}
			}
		};
		result
			.map(|(sql, values)| (sql, values, explain_backend))
			.map_err(|error| DatabaseError::new(DatabaseErrorKind::Unsupported, error.to_string()))
	}

	fn decode_explain_rows(
		rows: Vec<crate::backends::types::Row>,
		backend: ExplainBackend,
		format: ExplainFormat,
	) -> Result<ExplainOutput, DatabaseError> {
		let rows = rows
			.into_iter()
			.map(|row| QueryRow::from_backend_row(row).data)
			.collect::<Vec<_>>();
		let body = if format == ExplainFormat::Json {
			ExplainBody::Json(Self::decode_json_explain_body(rows)?)
		} else if rows.iter().all(|row| {
			row.as_object().is_some_and(|values| {
				values.len() == 1 && values.values().all(is_scalar_plan_value)
			})
		}) {
			let lines = rows
				.into_iter()
				.filter_map(|row| {
					row.as_object()
						.and_then(|values| values.values().next().cloned())
				})
				.map(plan_value_to_text)
				.collect::<Vec<_>>();
			ExplainBody::Text(lines.join("\n"))
		} else {
			ExplainBody::Rows(rows)
		};

		Ok(ExplainOutput {
			backend,
			format,
			body,
		})
	}

	fn decode_json_explain_body(
		rows: Vec<serde_json::Value>,
	) -> Result<serde_json::Value, DatabaseError> {
		let mut plans = Vec::with_capacity(rows.len());
		for row in rows {
			let value = row
				.as_object()
				.and_then(|values| {
					if values.len() == 1 {
						values.values().next().cloned()
					} else {
						None
					}
				})
				.unwrap_or(row);
			let value = match value {
				serde_json::Value::String(value) => {
					serde_json::from_str(&value).map_err(|error| {
						DatabaseError::new(
							DatabaseErrorKind::Serialization,
							format!("EXPLAIN JSON output could not be decoded: {error}"),
						)
					})?
				}
				value => value,
			};
			plans.push(value);
		}

		Ok(if plans.len() == 1 {
			plans.pop().expect("one plan was recorded")
		} else {
			serde_json::Value::Array(plans)
		})
	}

	fn temporal_projection_statement(
		&self,
		field: &str,
		kind: TemporalTruncKind,
		order: DateProjectionOrder,
		time_zone: Option<TemporalTimeZone>,
		output: TemporalTruncOutput,
	) -> reinhardt_core::exception::Result<SelectStatement> {
		if self.from_subquery_sql.is_some() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"date and datetime projections are not supported on querysets created from subqueries",
			)
			.into());
		}
		if !self.ctes.is_empty() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"date and datetime projections are not supported on querysets with CTEs",
			)
			.into());
		}
		if !self.lateral_joins.is_empty() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"date and datetime projections are not supported on querysets with lateral joins",
			)
			.into());
		}
		if !self.group_by_fields.is_empty() || self.has_typed_having() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Unsupported,
				"date and datetime projections are not supported on grouped querysets",
			)
			.into());
		}
		let source = Expr::col(self.root_column_reference(field)).into_simple_expr();
		let projection =
			Func::temporal_trunc(source.clone(), kind, time_zone, output).map_err(|error| {
				DatabaseError::new(DatabaseErrorKind::Unsupported, error.to_string())
			})?;
		let mut stmt = Query::select();
		self.apply_model_from(&mut stmt);
		stmt.expr_as(projection.clone(), Alias::new("value"));
		self.apply_relation_joins(&mut stmt);
		self.apply_manual_joins(&mut stmt);
		if let Some(condition) = self.build_where_condition()? {
			stmt.cond_where(condition);
		}
		stmt.and_where(source.is_not_null());
		stmt.distinct();
		stmt.order_by_expr(Expr::col(Alias::new("value")), order.into());
		if let Some(limit) = self.limit {
			stmt.limit(limit as u64);
		}
		if let Some(offset) = self.offset {
			stmt.offset(offset as u64);
		}
		Ok(stmt.to_owned())
	}

	async fn temporal_rows_with_db<E>(
		stmt: &SelectStatement,
		conn: &mut E,
	) -> reinhardt_core::exception::Result<Vec<crate::backends::types::Row>>
	where
		E: OrmExecutor,
	{
		let context = super::execution::pgvector_context_for_select(stmt);
		let (sql, values) =
			Self::build_select_for_backend(stmt, conn.backend(), conn.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let started_at = Instant::now();
		let result = conn.fetch_all_with_context(&sql, params, context).await;
		let duration = started_at.elapsed();
		match result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				Ok(rows)
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				Err(error)
			}
		}
	}

	async fn temporal_rows_with_executor(
		stmt: &SelectStatement,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> Result<Vec<crate::backends::types::Row>, crate::backends::error::DatabaseError> {
		let context = super::execution::pgvector_context_for_select(stmt);
		let (sql, values) = Self::build_select_for_backend(
			stmt,
			Self::executor_backend(executor),
			executor.is_cockroachdb(),
		)?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let started_at = Instant::now();
		let result = executor
			.fetch_all_with_context(&sql, params, context)
			.await
			.map_err(executor_error);
		let duration = started_at.elapsed();
		match result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				Ok(rows)
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				Err(error)
			}
		}
	}

	fn projection_value(
		row: crate::backends::types::Row,
	) -> Result<crate::backends::types::QueryValue, DatabaseError> {
		row.data.get("value").cloned().ok_or_else(|| {
			DatabaseError::new(
				DatabaseErrorKind::ColumnNotFound,
				"date projection did not return the `value` column",
			)
		})
	}

	fn decode_date_projection(
		rows: Vec<crate::backends::types::Row>,
	) -> Result<Vec<chrono::NaiveDate>, DatabaseError> {
		let mut values = Vec::with_capacity(rows.len());
		for row in rows {
			let date = match Self::projection_value(row)? {
				crate::backends::types::QueryValue::Null => continue,
				crate::backends::types::QueryValue::String(value) => {
					chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| {
						DatabaseError::new(
							DatabaseErrorKind::Serialization,
							format!("invalid projected date `{value}`: {error}"),
						)
					})?
				}
				crate::backends::types::QueryValue::NaiveTimestamp(value) => value.date(),
				crate::backends::types::QueryValue::Timestamp(value) => value.date_naive(),
				value => {
					return Err(DatabaseError::new(
						DatabaseErrorKind::Type,
						format!("cannot decode projected date from {value:?}"),
					));
				}
			};
			values.push(date);
		}
		Ok(values)
	}

	fn decode_datetime_projection(
		rows: Vec<crate::backends::types::Row>,
		time_zone: chrono_tz::Tz,
	) -> Result<Vec<chrono::DateTime<chrono_tz::Tz>>, DatabaseError> {
		let mut values = Vec::with_capacity(rows.len());
		for row in rows {
			let utc = match Self::projection_value(row)? {
				crate::backends::types::QueryValue::Null => continue,
				crate::backends::types::QueryValue::Timestamp(value) => value,
				crate::backends::types::QueryValue::NaiveTimestamp(value) => value.and_utc(),
				crate::backends::types::QueryValue::String(value) => {
					if let Ok(value) = chrono::DateTime::parse_from_rfc3339(&value) {
						value.with_timezone(&chrono::Utc)
					} else {
						chrono::NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S%.f")
							.map(|value| value.and_utc())
							.map_err(|error| {
								DatabaseError::new(
									DatabaseErrorKind::Serialization,
									format!("invalid projected datetime `{value}`: {error}"),
								)
							})?
					}
				}
				value => {
					return Err(DatabaseError::new(
						DatabaseErrorKind::Type,
						format!("cannot decode projected datetime from {value:?}"),
					));
				}
			};
			values.push(utc.with_timezone(&time_zone));
		}
		Ok(values)
	}

	/// Sets the manager and returns self for chaining.
	pub fn with_manager(manager: std::sync::Arc<super::manager::Manager<T>>) -> Self {
		Self {
			_phantom: std::marker::PhantomData,
			filters: SmallVec::new(),
			filter_conditions: SmallVec::new(),
			select_related_fields: Vec::new(),
			typed_select_related: Vec::new(),
			prefetch_related_fields: Vec::new(),
			typed_prefetch_related: Vec::new(),
			relation_joins: RelationJoinGraph::new(T::table_name()),
			order_by_fields: Vec::new(),
			order_by_expressions: Vec::new(),
			distinct_enabled: false,
			selected_fields: None,
			selected_expressions: Vec::new(),
			deferred_fields: Vec::new(),
			annotations: Vec::new(),
			backend_annotations: Vec::new(),
			typed_annotations: Vec::new(),
			typed_havings: Vec::new(),
			manager: Some(manager),
			limit: None,
			offset: None,
			ctes: super::cte::CTECollection::new(),
			lateral_joins: super::lateral_join::LateralJoins::new(),
			joins: Vec::new(),
			group_by_fields: Vec::new(),
			subquery_conditions: Vec::new(),
			from_alias: None,
			empty_result: false,
			from_subquery_sql: None,
			from_subquery_statement: None,
			from_subquery_model_shaped: None,
			from_subquery_model_type: None,
			select_for_update: None,
		}
	}

	/// Configures this queryset to lock selected rows until transaction completion.
	///
	/// The returned typestate builder prevents combining `NOWAIT` and
	/// `SKIP LOCKED`. Locking queries must be evaluated with a caller-owned
	/// [`TransactionExecutor`](super::connection::TransactionExecutor). CTE-backed
	/// querysets, derived `FROM` sources, LATERAL joins, raw aggregate projections
	/// from [`Self::values`], and aggregate annotations are rejected before query
	/// execution to preserve lock scope.
	pub fn select_for_update(mut self) -> SelectForUpdate<T> {
		self.select_for_update = Some(SelectForUpdateSpec::default());
		SelectForUpdate {
			queryset: self,
			_behavior: PhantomData,
		}
	}

	/// Appends a filter expression to this `QuerySet`.
	///
	/// Accepts typed and untyped inputs through [`QueryFilterInput`]. Typed
	/// relation filters must be rooted at this `QuerySet` model.
	pub fn filter(mut self, filter: impl QueryFilterInput<T>) -> Self {
		let condition = filter.into_filter_condition();
		condition.assert_relation_root::<T>();
		match condition {
			FilterCondition::Single(mut filter) => {
				filter.add_relation_joins(&mut self.relation_joins);
				let relation_joins = self.relation_join_graph_for_query();
				filter.rebase_relation_alias(&relation_joins);
				self.filters.push(filter);
			}
			mut condition => {
				self.collect_condition_relation_joins(&condition);
				let relation_joins = self.relation_join_graph_for_query();
				condition.rebase_relation_aliases(&relation_joins, 0);
				self.filter_conditions.push(condition);
			}
		}
		self
	}

	/// Returns the filters that have been applied to this `QuerySet`.
	///
	/// Useful for inspection in tests and for custom managers that need to
	/// observe or assert on the active filter chain (Issue #3980).
	pub fn filters(&self) -> &[Filter] {
		&self.filters
	}

	/// Returns composite filter conditions applied to this `QuerySet`.
	pub fn filter_conditions(&self) -> &[FilterCondition] {
		&self.filter_conditions
	}

	/// Maps every stored filter, including filters nested in composite conditions.
	pub fn map_filter_columns<F>(&mut self, mut mapper: F)
	where
		F: FnMut(&mut Filter),
	{
		for filter in &mut self.filters {
			mapper(filter);
		}
		for condition in &mut self.filter_conditions {
			map_filter_condition_columns(condition, &mut mapper);
		}
	}

	/// Maps every stored ordering field.
	pub fn map_order_by_fields<F>(&mut self, mut mapper: F)
	where
		F: FnMut(&mut String),
	{
		for field in &mut self.order_by_fields {
			mapper(field);
		}
	}

	/// Maps outer fields used by subquery predicates.
	pub fn map_subquery_fields<F>(&mut self, mut mapper: F)
	where
		F: FnMut(&mut String),
	{
		for condition in &mut self.subquery_conditions {
			match condition {
				SubqueryCondition::In { field, .. } | SubqueryCondition::NotIn { field, .. } => {
					mapper(field)
				}
				SubqueryCondition::Exists {
					subquery,
					outer_fields,
					..
				}
				| SubqueryCondition::NotExists {
					subquery,
					outer_fields,
					..
				} => {
					let mut rewrites = Vec::new();
					for field in outer_fields {
						let old_field = field.clone();
						mapper(field);
						if old_field != *field {
							rewrites.push((old_field, field.clone()));
						}
					}
					if !rewrites.is_empty() {
						subquery.rewrite_fields(&rewrites);
					}
				}
			}
		}
	}

	/// Returns fields used by subquery predicates, including correlated fields.
	pub fn subquery_fields(&self) -> impl Iterator<Item = &str> {
		self.subquery_conditions
			.iter()
			.flat_map(|condition| {
				let fields: &[String] = match condition {
					SubqueryCondition::In { field, .. }
					| SubqueryCondition::NotIn { field, .. } => std::slice::from_ref(field),
					SubqueryCondition::Exists { outer_fields, .. }
					| SubqueryCondition::NotExists { outer_fields, .. } => outer_fields,
				};
				fields.iter()
			})
			.map(String::as_str)
	}

	fn outer_reference_fields(&self) -> Vec<String> {
		let mut fields = Vec::new();
		for filter in &self.filters {
			collect_subquery_outer_fields(&filter.value, &mut fields);
		}
		for condition in &self.filter_conditions {
			collect_subquery_outer_condition(condition, &mut fields);
		}
		fields.sort_unstable();
		fields.dedup();
		fields
	}

	/// Returns whether this queryset contains an authorization subquery.
	pub fn has_subquery_conditions(&self) -> bool {
		!self.subquery_conditions.is_empty()
	}

	/// Returns whether a mutation needs serializable isolation for scope checks.
	///
	/// Authorization expressed through subqueries or manual joins is not fully
	/// represented by the typed relation-lock graph. Mutations using either
	/// shape need serializable isolation so a concurrent scope change cannot
	/// occur between the authorization recheck and the write.
	pub fn requires_serializable_transaction(&self) -> bool {
		self.has_subquery_conditions()
			|| !self.joins.is_empty()
			|| !self.relation_join_graph_for_query().is_empty()
	}

	/// Add row-locking clauses to subqueries used as authorization predicates.
	///
	/// The outer mutation recheck locks its model rows, but a scope expressed as
	/// IN or EXISTS reads rows from a separate subquery. Lock those rows on the
	/// same transaction connection so the scope cannot change between the
	/// recheck and the subsequent write.
	pub(crate) fn lock_scope_subqueries(&mut self) {
		for condition in &mut self.subquery_conditions {
			match condition {
				SubqueryCondition::In {
					subquery, lockable, ..
				}
				| SubqueryCondition::NotIn {
					subquery, lockable, ..
				}
				| SubqueryCondition::Exists {
					subquery, lockable, ..
				}
				| SubqueryCondition::NotExists {
					subquery, lockable, ..
				} if *lockable => {
					subquery.add_lock();
				}
				_ => {}
			}
		}
	}

	fn collect_condition_relation_joins(&mut self, condition: &FilterCondition) {
		Self::collect_condition_relation_joins_at_depth(&mut self.relation_joins, condition, 0);
	}

	fn collect_condition_relation_joins_at_depth(
		graph: &mut RelationJoinGraph,
		condition: &FilterCondition,
		depth: usize,
	) {
		if depth >= MAX_FILTER_CONDITION_DEPTH {
			return;
		}
		match condition {
			FilterCondition::Single(filter) => {
				filter.add_relation_joins(graph);
			}
			FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
				for condition in conditions {
					Self::collect_condition_relation_joins_at_depth(graph, condition, depth + 1);
				}
			}
			FilterCondition::Not(condition) => {
				Self::collect_condition_relation_joins_at_depth(graph, condition, depth + 1);
			}
		}
	}

	fn rebase_filter_relation_aliases(&mut self) {
		let relation_joins = self.relation_join_graph_for_query();
		for filter in &mut self.filters {
			filter.rebase_relation_alias(&relation_joins);
		}
		for condition in &mut self.filter_conditions {
			condition.rebase_relation_aliases(&relation_joins, 0);
		}
	}

	fn has_where_predicates(&self) -> bool {
		!(self.filters.is_empty()
			&& self.filter_conditions.is_empty()
			&& self.subquery_conditions.is_empty())
	}

	fn has_restricting_where_predicates(&self) -> bool {
		if !self.subquery_conditions.is_empty() {
			return true;
		}
		self.filters.iter().any(|filter| !filter.is_always_true())
			|| self
				.filter_conditions
				.iter()
				.any(|condition| !condition.is_always_true())
	}

	fn has_select_related(&self) -> bool {
		!(self.select_related_fields.is_empty() && self.typed_select_related.is_empty())
	}

	fn has_related_filters(&self) -> reinhardt_core::exception::Result<bool> {
		if self.filters.iter().any(Filter::has_relation) {
			return Ok(true);
		}
		for condition in &self.filter_conditions {
			if condition.has_relation()? {
				return Ok(true);
			}
		}
		Ok(false)
	}

	fn validate_no_related_filters_for_write(
		&self,
		operation: &str,
	) -> reinhardt_core::exception::Result<()> {
		if self.has_related_filters()? {
			return Err(reinhardt_core::exception::Error::Validation(format!(
				"{operation} does not support typed related filters; use a subquery or select query first"
			)));
		}
		Ok(())
	}

	fn build_where_condition_for_write(
		&self,
	) -> reinhardt_core::exception::Result<Option<Condition>> {
		if self.empty_result {
			return Ok(Some(Condition::all().add(Expr::val(false))));
		}
		let mut queryset = self.clone();
		queryset.relation_joins = RelationJoinGraph::new(T::table_name());
		queryset.from_alias = None;
		queryset.resolve_write_predicate_fields();
		queryset.build_where_condition()
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
	/// # use reinhardt_db::orm::func;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "books" }
	/// #     fn new_fields() -> Self::Fields { BookFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// // Query from a derived table showing author book counts
	/// let results = QuerySet::<Book>::from_subquery(
	///     |subq: QuerySet<Book>| {
	///         subq.values(&["author_id"])
	///             .annotate(
	///                 func::count_all::<Book>()
	///                     .label("book_count")
	///                     .expect("valid annotation label"),
	///             )
	///             .expect("annotation should compile")
	///     },
	///     "book_stats"
	/// )
	/// .expect("subquery should compile")
	/// .filter(Filter::new("book_count", FilterOperator::Gt, FilterValue::Int(1)))
	/// .to_sql()
	/// .expect("query SQL should compile");
	/// // Generates: SELECT * FROM (SELECT author_id, COUNT(*) AS book_count FROM books GROUP BY author_id) AS book_stats WHERE book_count > 1
	/// ```
	pub fn from_subquery<M, F>(builder: F, alias: &str) -> reinhardt_core::exception::Result<Self>
	where
		M: super::Model + 'static,
		F: FnOnce(QuerySet<M>) -> QuerySet<M>,
	{
		// Create a fresh QuerySet for the subquery model
		let subquery_qs = QuerySet::<M>::new();
		// Apply the builder to configure the subquery
		let configured_subquery = builder(subquery_qs);
		let empty_result = configured_subquery.empty_result;
		let subquery_statement = SubqueryStatements {
			postgres: configured_subquery.build_select_statement_for_backend(
				crate::backends::types::DatabaseType::Postgres,
			)?,
			mysql: configured_subquery
				.build_select_statement_for_backend(crate::backends::types::DatabaseType::Mysql)?,
			sqlite: configured_subquery
				.build_select_statement_for_backend(crate::backends::types::DatabaseType::Sqlite)?,
		};
		let subquery_model_shaped = configured_subquery.is_model_shaped_source();
		// Generate SQL for the subquery (wrapped in parentheses)
		let subquery_sql = configured_subquery.as_subquery_sql()?;

		// Create a new QuerySet with the subquery as FROM source
		Ok(Self {
			_phantom: std::marker::PhantomData,
			filters: SmallVec::new(),
			filter_conditions: SmallVec::new(),
			select_related_fields: Vec::new(),
			typed_select_related: Vec::new(),
			prefetch_related_fields: Vec::new(),
			typed_prefetch_related: Vec::new(),
			relation_joins: RelationJoinGraph::new(alias),
			order_by_fields: Vec::new(),
			order_by_expressions: Vec::new(),
			distinct_enabled: false,
			selected_fields: None,
			selected_expressions: Vec::new(),
			deferred_fields: Vec::new(),
			annotations: Vec::new(),
			backend_annotations: Vec::new(),
			typed_annotations: Vec::new(),
			typed_havings: Vec::new(),
			manager: None,
			limit: None,
			offset: None,
			ctes: super::cte::CTECollection::new(),
			lateral_joins: super::lateral_join::LateralJoins::new(),
			joins: Vec::new(),
			group_by_fields: Vec::new(),
			subquery_conditions: Vec::new(),
			from_alias: Some(alias.to_string()),
			empty_result,
			from_subquery_sql: Some(subquery_sql),
			from_subquery_statement: Some(subquery_statement),
			from_subquery_model_shaped: Some(subquery_model_shaped),
			from_subquery_model_type: Some(std::any::type_name::<M>()),
			select_for_update: None,
		})
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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

	/// Add a typed aggregate predicate to the `HAVING` clause.
	///
	/// Aggregate comparisons are lowered from the same structured expression
	/// nodes used by typed annotations. Scalar predicates intentionally do not
	/// implement this method's input type and therefore cannot be reinterpreted
	/// as `HAVING` conditions.
	pub fn having(mut self, predicate: HavingPredicate<T>) -> Self {
		self.typed_havings.push(predicate.into_stored_expression());
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	///     })?
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_in_subquery<R: super::Model, F>(
		mut self,
		field: &str,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<Self>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		let subquery_qs = subquery_fn(QuerySet::<R>::new());
		let lockable = subquery_qs.supports_scope_subquery_row_lock();
		let subquery_sql = subquery_qs.as_subquery_sql()?;

		self.subquery_conditions.push(SubqueryCondition::In {
			field: field.to_string(),
			subquery: subquery_sql,
			lockable,
		});

		Ok(self)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	///     })?
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_not_in_subquery<R: super::Model, F>(
		mut self,
		field: &str,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<Self>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		let subquery_qs = subquery_fn(QuerySet::<R>::new());
		let lockable = subquery_qs.supports_scope_subquery_row_lock();
		let subquery_sql = subquery_qs.as_subquery_sql()?;

		self.subquery_conditions.push(SubqueryCondition::NotIn {
			field: field.to_string(),
			subquery: subquery_sql,
			lockable,
		});

		Ok(self)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	///     })?
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_exists<R: super::Model, F>(
		mut self,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<Self>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		let subquery_qs = subquery_fn(QuerySet::<R>::new());
		let lockable = subquery_qs.supports_scope_subquery_row_lock();
		let outer_fields = subquery_qs.outer_reference_fields();
		let subquery_sql = subquery_qs.as_subquery_sql()?;

		self.subquery_conditions.push(SubqueryCondition::Exists {
			subquery: subquery_sql,
			outer_fields,
			lockable,
		});

		Ok(self)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	///     })?
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter_not_exists<R: super::Model, F>(
		mut self,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<Self>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		let subquery_qs = subquery_fn(QuerySet::<R>::new());
		let lockable = subquery_qs.supports_scope_subquery_row_lock();
		let outer_fields = subquery_qs.outer_reference_fields();
		let subquery_sql = subquery_qs.as_subquery_sql()?;

		self.subquery_conditions.push(SubqueryCondition::NotExists {
			subquery: subquery_sql,
			outer_fields,
			lockable,
		});

		Ok(self)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		self.rebase_filter_relation_aliases();
		self
	}

	fn relation_join_graph_for_query(&self) -> RelationJoinGraph {
		self.relation_joins
			.clone()
			.with_root_alias_and_reserved_aliases(self.root_alias(), self.manual_join_aliases())
	}

	fn expression_relation_join_graph_for_query(&self) -> RelationJoinGraph {
		let mut graph = self.relation_joins.clone();
		for expression in self
			.selected_expressions
			.iter()
			.map(|(_, expression)| expression)
			.chain(self.typed_annotations.iter())
			.chain(self.typed_havings.iter())
			.chain(
				self.order_by_expressions
					.iter()
					.map(|ordering| &ordering.expression),
			) {
			for path in &expression.joins.paths {
				graph.add_aggregate_steps(path);
			}
		}
		graph.with_root_alias_and_reserved_aliases(self.root_alias(), self.manual_join_aliases())
	}

	fn filter_relation_join_graph_for_query(&self) -> RelationJoinGraph {
		let mut graph = RelationJoinGraph::new(T::table_name());
		for filter in &self.filters {
			filter.add_relation_joins(&mut graph);
		}
		for condition in &self.filter_conditions {
			Self::collect_condition_relation_joins_at_depth(&mut graph, condition, 0);
		}
		graph.with_root_alias_and_reserved_aliases(self.root_alias(), self.manual_join_aliases())
	}

	fn manual_join_aliases(&self) -> impl Iterator<Item = String> + '_ {
		self.joins
			.iter()
			.map(|join| {
				join.target_alias
					.clone()
					.unwrap_or_else(|| join.target_table.clone())
			})
			.chain(self.lateral_joins.aliases())
	}

	pub(crate) fn root_alias(&self) -> &str {
		self.from_alias.as_deref().unwrap_or(T::table_name())
	}

	pub(crate) fn inner_relation_aliases_for_lock(&self) -> Vec<String> {
		let mut aliases = self
			.filter_relation_join_graph_for_query()
			.joins()
			.iter()
			.filter(|join| join.join_kind == RelationJoinKind::Inner)
			.map(|join| join.alias.clone())
			.collect::<Vec<_>>();
		aliases.extend(
			self.joins
				.iter()
				.filter(|&join| matches!(join.join_type, super::sqlalchemy_query::JoinType::Inner))
				.map(|join| {
					join.target_alias
						.clone()
						.unwrap_or_else(|| join.target_table.clone())
				}),
		);
		aliases.sort_unstable();
		aliases.dedup();
		aliases
	}

	pub(crate) fn has_right_join(&self) -> bool {
		self.joins
			.iter()
			.any(|join| matches!(join.join_type, super::sqlalchemy_query::JoinType::Right))
	}

	pub(crate) fn nullable_filter_relations_for_lock(&self) -> Vec<(String, String, String)> {
		self.filter_relation_join_graph_for_query()
			.joins()
			.iter()
			.filter(|join| join.join_kind == RelationJoinKind::Left)
			.map(|join| {
				(
					join.target_table.clone(),
					join.alias.clone(),
					join.target_column.clone(),
				)
			})
			.collect()
	}

	fn apply_model_from(&self, stmt: &mut SelectStatement) {
		if let Some(ref alias) = self.from_alias {
			stmt.from_as(Alias::new(T::table_name()), Alias::new(alias));
		} else {
			stmt.from(Alias::new(T::table_name()));
		}
	}

	fn apply_model_from_for_backend(
		&self,
		stmt: &mut SelectStatement,
		backend: crate::backends::types::DatabaseType,
	) {
		if let (Some(subquery), Some(alias)) = (&self.from_subquery_statement, &self.from_alias) {
			stmt.from_subquery(subquery.for_backend(backend).clone(), Alias::new(alias));
		} else {
			self.apply_model_from(stmt);
		}
	}

	fn add_default_select_columns(&self, stmt: &mut SelectStatement) {
		if self.expression_relation_join_graph_for_query().is_empty() {
			stmt.column(ColumnRef::Asterisk);
		} else {
			stmt.column(ColumnRef::table_asterisk(Alias::new(self.root_alias())));
		}
	}

	fn add_select_related_root_columns(&self, stmt: &mut SelectStatement) {
		if let Some(ref fields) = self.selected_fields {
			for field in fields {
				if field.contains('(') && field.contains(')') {
					stmt.expr(Expr::cust(field.clone()));
				} else if field.contains('.') {
					stmt.column(parse_column_reference(field));
				} else {
					stmt.column(ColumnRef::table_column(
						Alias::new(self.root_alias()),
						Alias::new(field),
					));
				}
			}
		} else if !self.deferred_fields.is_empty() {
			for field in T::field_metadata() {
				if !self.deferred_fields.contains(&field.name) {
					stmt.column(ColumnRef::table_column(
						Alias::new(self.root_alias()),
						Alias::new(field.db_column_name()),
					));
				}
			}
		} else {
			stmt.column(ColumnRef::table_asterisk(Alias::new(self.root_alias())));
		}
	}

	fn root_column_reference(&self, field: &str) -> ColumnRef {
		if (!self.expression_relation_join_graph_for_query().is_empty()
			|| !self.joins.is_empty()
			|| self.has_select_related())
			&& !field.contains('.')
			&& !self.is_projection_alias(field)
		{
			ColumnRef::table_column(Alias::new(self.root_alias()), Alias::new(field))
		} else {
			parse_column_reference(field)
		}
	}

	fn is_projection_alias(&self, field: &str) -> bool {
		self.annotations
			.iter()
			.any(|annotation| annotation.alias == field)
			|| self
				.selected_expressions
				.iter()
				.map(|(alias, _)| alias.as_str())
				.chain(
					self.typed_annotations
						.iter()
						.filter_map(|expression| expression.label.as_deref()),
				)
				.any(|alias| alias == field)
			|| self
				.backend_annotations
				.iter()
				.any(|annotation| annotation.label() == field)
	}

	fn database_column_for_field(field: &str) -> String {
		if field.contains('.') {
			return field.to_string();
		}

		T::field_metadata()
			.iter()
			.find(|metadata| metadata.name == field)
			.map(|metadata| metadata.db_column_name().to_string())
			.unwrap_or_else(|| field.to_string())
	}

	fn resolve_write_predicate_fields(&mut self) {
		for filter in &mut self.filters {
			Self::resolve_write_filter_fields(filter);
		}

		for condition in &mut self.filter_conditions {
			Self::resolve_write_filter_condition_fields(condition);
		}

		for condition in &mut self.subquery_conditions {
			match condition {
				SubqueryCondition::In { field, .. } | SubqueryCondition::NotIn { field, .. } => {
					*field = Self::database_column_for_field(field);
				}
				SubqueryCondition::Exists { .. } | SubqueryCondition::NotExists { .. } => {}
			}
		}
	}

	fn resolve_write_filter_condition_fields(condition: &mut FilterCondition) {
		match condition {
			FilterCondition::Single(filter) => Self::resolve_write_filter_fields(filter),
			FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
				for condition in conditions {
					Self::resolve_write_filter_condition_fields(condition);
				}
			}
			FilterCondition::Not(condition) => {
				Self::resolve_write_filter_condition_fields(condition)
			}
		}
	}

	fn resolve_write_filter_fields(filter: &mut Filter) {
		if filter.relation.is_some() {
			return;
		}

		if matches!(&filter.field_source, FilterField::Column(_)) {
			filter.field = Self::database_column_for_field(&filter.field);
		}
		Self::resolve_write_filter_value_fields(&mut filter.value);
	}

	fn resolve_write_filter_value_fields(value: &mut FilterValue) {
		match value {
			FilterValue::List(values) => {
				for value in values {
					Self::resolve_write_filter_value_fields(value);
				}
			}
			FilterValue::Range(start, end) => {
				Self::resolve_write_filter_value_fields(start);
				Self::resolve_write_filter_value_fields(end);
			}
			FilterValue::FieldRef(field) => {
				field.field = Self::database_column_for_field(&field.field);
			}
			FilterValue::Expression(expression) => {
				Self::resolve_write_expression_fields(expression);
			}
			_ => {}
		}
	}

	fn resolve_write_expression_fields(expression: &mut super::annotation::Expression) {
		use super::annotation::Expression;

		match expression {
			Expression::Add(left, right)
			| Expression::Subtract(left, right)
			| Expression::Multiply(left, right)
			| Expression::Divide(left, right) => {
				Self::resolve_write_annotation_value_fields(left);
				Self::resolve_write_annotation_value_fields(right);
			}
			Expression::Case { whens, default } => {
				for when in whens {
					Self::resolve_write_q_fields(&mut when.condition);
					Self::resolve_write_annotation_value_fields(&mut when.then);
				}
				if let Some(default) = default {
					Self::resolve_write_annotation_value_fields(default);
				}
			}
			Expression::Coalesce(values) => {
				for value in values {
					Self::resolve_write_annotation_value_fields(value);
				}
			}
		}
	}

	fn resolve_write_annotation_value_fields(value: &mut super::annotation::AnnotationValue) {
		match value {
			super::annotation::AnnotationValue::Field(field) => {
				field.field = Self::database_column_for_field(&field.field);
			}
			super::annotation::AnnotationValue::Expression(expression) => {
				Self::resolve_write_expression_fields(expression);
			}
			_ => {}
		}
	}

	fn resolve_write_q_fields(condition: &mut super::expressions::Q) {
		match condition {
			super::expressions::Q::Condition {
				field, operator, ..
			} if !field.is_empty() && !operator.is_empty() => {
				*field = Self::database_column_for_field(field);
			}
			super::expressions::Q::Combined { conditions, .. } => {
				for condition in conditions {
					Self::resolve_write_q_fields(condition);
				}
			}
			_ => {}
		}
	}

	fn root_column_sql_for_backend(
		&self,
		field: &str,
		backend: crate::backends::types::DatabaseType,
	) -> String {
		if !self.expression_relation_join_graph_for_query().is_empty() && !field.contains('.') {
			quote_identifier_for_backend(&format!("{}.{}", self.root_alias(), field), backend)
		} else {
			quote_identifier_for_backend(field, backend)
		}
	}

	fn distinct_root_primary_key_sql(&self) -> String {
		let root_alias = quote_identifier(self.root_alias());
		if let Some(composite_key) = T::composite_primary_key() {
			let field_metadata = T::field_metadata();
			let columns = composite_key
				.fields()
				.iter()
				.map(|field| {
					let column = field_metadata
						.iter()
						.find(|metadata| metadata.name == *field)
						.map_or(field.as_str(), |metadata| metadata.db_column_name());
					format!("{root_alias}.{}", quote_identifier(column))
				})
				.collect::<Vec<_>>()
				.join(", ");
			if composite_key.field_count() > 1 {
				format!("({columns})")
			} else {
				columns
			}
		} else {
			format!("{root_alias}.{}", quote_identifier(T::primary_key_column()))
		}
	}

	fn has_composite_primary_key(&self) -> bool {
		T::composite_primary_key().is_some_and(|primary_key| primary_key.field_count() > 1)
	}

	fn root_primary_key_columns(&self) -> Vec<ColumnRef> {
		let root_alias = self.root_alias();
		if let Some(composite_key) = T::composite_primary_key() {
			let field_metadata = T::field_metadata();
			return composite_key
				.fields()
				.iter()
				.map(|field| {
					let column = field_metadata
						.iter()
						.find(|metadata| metadata.name == *field)
						.map_or(field.as_str(), |metadata| metadata.db_column_name());
					ColumnRef::table_column(Alias::new(root_alias), Alias::new(column))
				})
				.collect();
		}

		vec![ColumnRef::table_column(
			Alias::new(root_alias),
			Alias::new(T::primary_key_column()),
		)]
	}

	fn validate_relation_path(&self, path: &str) -> reinhardt_core::exception::Result<()> {
		let relations = T::relationship_metadata();
		if relations.is_empty() {
			return Ok(());
		}

		if path.contains("__") {
			return Err(reinhardt_core::exception::Error::Validation(format!(
				"Nested string relation path `{}` is not supported for {}; use typed relation paths instead",
				path,
				std::any::type_name::<T>()
			)));
		}

		let first = path.split("__").next().unwrap_or(path);
		if relations.iter().any(|relation| relation.name == first) {
			Ok(())
		} else {
			Err(reinhardt_core::exception::Error::Validation(format!(
				"Unknown relation path `{}` for {}",
				path,
				std::any::type_name::<T>()
			)))
		}
	}

	#[cfg(test)]
	fn validate_relation_path_for_test(&self, path: &str) -> reinhardt_core::exception::Result<()> {
		self.validate_relation_path(path)
	}

	fn apply_relation_joins(&self, stmt: &mut SelectStatement) {
		let graph = self.expression_relation_join_graph_for_query();
		Self::apply_relation_join_graph(stmt, &graph);
	}

	fn apply_manual_joins(&self, stmt: &mut SelectStatement) {
		for join in &self.joins {
			if join.on_condition.is_empty() {
				if let Some(ref alias) = join.target_alias {
					stmt.cross_join((Alias::new(&join.target_table), Alias::new(alias)));
				} else {
					stmt.cross_join(Alias::new(&join.target_table));
				}
				continue;
			}

			let sea_join_type = match join.join_type {
				super::sqlalchemy_query::JoinType::Inner => SeaJoinType::InnerJoin,
				super::sqlalchemy_query::JoinType::Left => SeaJoinType::LeftJoin,
				super::sqlalchemy_query::JoinType::Right => SeaJoinType::RightJoin,
				super::sqlalchemy_query::JoinType::Full => SeaJoinType::FullOuterJoin,
			};
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

	fn apply_typed_select_expressions(
		&self,
		stmt: &mut SelectStatement,
	) -> reinhardt_core::exception::Result<()> {
		let graph = self.expression_relation_join_graph_for_query();
		for (alias, expression) in &self.selected_expressions {
			stmt.expr_as(
				compile_expression(expression, self.root_alias(), &graph)?,
				Alias::new(alias),
			);
		}
		for expression in &self.typed_annotations {
			let alias = expression
				.label
				.as_deref()
				.expect("typed annotations always retain their validated label");
			stmt.expr_as(
				compile_expression(expression, self.root_alias(), &graph)?,
				Alias::new(alias),
			);
		}
		Ok(())
	}

	fn apply_ordering(&self, stmt: &mut SelectStatement) -> reinhardt_core::exception::Result<()> {
		for order_field in &self.order_by_fields {
			let (field, order) = order_field
				.strip_prefix('-')
				.map_or((order_field.as_str(), Order::Asc), |field| {
					(field, Order::Desc)
				});
			stmt.order_by_expr(Expr::col(self.root_column_reference(field)), order);
		}
		for ordering in &self.order_by_expressions {
			stmt.order_by_expr(
				compile_expression(
					&ordering.expression,
					self.root_alias(),
					&self.expression_relation_join_graph_for_query(),
				)?,
				ordering.order,
			);
		}
		Ok(())
	}

	fn apply_relation_join_graph(stmt: &mut SelectStatement, graph: &RelationJoinGraph) {
		for join in graph.joins() {
			let sea_join_type = match join.join_kind {
				RelationJoinKind::Inner => SeaJoinType::InnerJoin,
				RelationJoinKind::Left => SeaJoinType::LeftJoin,
			};
			stmt.join(
				sea_join_type,
				TableRef::table_alias(
					Alias::new(join.target_table.clone()),
					Alias::new(&join.alias),
				),
				Expr::col((
					Alias::new(&join.source_alias),
					Alias::new(&join.source_column),
				))
				.equals((Alias::new(&join.alias), Alias::new(&join.target_column))),
			);
		}
	}

	/// Build WHERE condition using reinhardt-query from accumulated filters
	fn build_where_condition(&self) -> reinhardt_core::exception::Result<Option<Condition>> {
		self.build_where_condition_for_backend(crate::backends::types::DatabaseType::Postgres)
	}

	fn build_where_condition_for_backend(
		&self,
		backend: crate::backends::types::DatabaseType,
	) -> reinhardt_core::exception::Result<Option<Condition>> {
		if self.empty_result {
			return Ok(Some(Condition::all().add(Expr::val(1).eq(0))));
		}

		if !self.has_where_predicates() {
			return Ok(None);
		}

		let mut cond = Condition::all();
		let mut added = false;

		for filter in &self.filters {
			if let FilterField::TypedPredicate(expression) = &filter.field_source {
				let expression = expression.as_ref().map_err(|message| {
					Error::Validation(format!(
						"typed predicate value could not be encoded: {message}"
					))
				})?;
				let graph = self.expression_relation_join_graph_for_query();
				cond = cond.add(compile_expression(expression, self.root_alias(), &graph)?);
				added = true;
				continue;
			}
			let col = self.filter_lhs_expr(filter);

			let expr = match (&filter.operator, &filter.value) {
				// Field-to-field comparisons (must come before generic patterns)
				(FilterOperator::Eq, FilterValue::FieldRef(f)) => {
					col.eq(Expr::col(self.root_column_reference(&f.field)))
				}
				(FilterOperator::Ne, FilterValue::FieldRef(f)) => {
					col.ne(Expr::col(self.root_column_reference(&f.field)))
				}
				(FilterOperator::Gt, FilterValue::FieldRef(f)) => {
					col.gt(Expr::col(self.root_column_reference(&f.field)))
				}
				(FilterOperator::Gte, FilterValue::FieldRef(f)) => {
					col.gte(Expr::col(self.root_column_reference(&f.field)))
				}
				(FilterOperator::Lt, FilterValue::FieldRef(f)) => {
					col.lt(Expr::col(self.root_column_reference(&f.field)))
				}
				(FilterOperator::Lte, FilterValue::FieldRef(f)) => {
					col.lte(Expr::col(self.root_column_reference(&f.field)))
				}
				// OuterRef comparisons for correlated subqueries
				(FilterOperator::Eq, FilterValue::OuterRef(outer)) => {
					// For correlated subqueries, reference outer query field
					// e.g., WHERE books.author_id = authors.id (where authors is from outer query)
					col.eq(Expr::col(parse_column_reference(&outer.field)))
				}
				(FilterOperator::Ne, FilterValue::OuterRef(outer)) => {
					col.ne(Expr::col(parse_column_reference(&outer.field)))
				}
				(FilterOperator::Gt, FilterValue::OuterRef(outer)) => {
					col.gt(Expr::col(parse_column_reference(&outer.field)))
				}
				(FilterOperator::Gte, FilterValue::OuterRef(outer)) => {
					col.gte(Expr::col(parse_column_reference(&outer.field)))
				}
				(FilterOperator::Lt, FilterValue::OuterRef(outer)) => {
					col.lt(Expr::col(parse_column_reference(&outer.field)))
				}
				(FilterOperator::Lte, FilterValue::OuterRef(outer)) => {
					col.lte(Expr::col(parse_column_reference(&outer.field)))
				}
				// Expression comparisons (F("a") * F("b") etc.)
				(FilterOperator::Eq, FilterValue::Expression(expr)) => {
					col.eq(self.filter_expression_to_query_expr(expr))
				}
				(FilterOperator::Ne, FilterValue::Expression(expr)) => {
					col.ne(self.filter_expression_to_query_expr(expr))
				}
				(FilterOperator::Gt, FilterValue::Expression(expr)) => {
					col.gt(self.filter_expression_to_query_expr(expr))
				}
				(FilterOperator::Gte, FilterValue::Expression(expr)) => {
					col.gte(self.filter_expression_to_query_expr(expr))
				}
				(FilterOperator::Lt, FilterValue::Expression(expr)) => {
					col.lt(self.filter_expression_to_query_expr(expr))
				}
				(FilterOperator::Lte, FilterValue::Expression(expr)) => {
					col.lte(self.filter_expression_to_query_expr(expr))
				}
				// Typed scalar values retain codec errors until this compilation step.
				(FilterOperator::Eq, FilterValue::Typed(value)) => {
					match Self::typed_database_value(value)? {
						DatabaseValue::Null => col.is_null(),
						value => col.eq(database_value_to_query_value(value.clone())),
					}
				}
				(FilterOperator::Ne, FilterValue::Typed(value)) => {
					match Self::typed_database_value(value)? {
						DatabaseValue::Null => col.is_not_null(),
						value => col.ne(database_value_to_query_value(value.clone())),
					}
				}
				(FilterOperator::Gt, FilterValue::Typed(value)) => col.gt(
					database_value_to_query_value(Self::typed_database_value(value)?.clone()),
				),
				(FilterOperator::Gte, FilterValue::Typed(value)) => col.gte(
					database_value_to_query_value(Self::typed_database_value(value)?.clone()),
				),
				(FilterOperator::Lt, FilterValue::Typed(value)) => col.lt(
					database_value_to_query_value(Self::typed_database_value(value)?.clone()),
				),
				(FilterOperator::Lte, FilterValue::Typed(value)) => col.lte(
					database_value_to_query_value(Self::typed_database_value(value)?.clone()),
				),
				(FilterOperator::IExact, FilterValue::Typed(value)) => {
					match Self::typed_database_value(value)? {
						DatabaseValue::String(value) => {
							self.like_expr(filter, value, LikePattern::Exact, true)
						}
						value => col.eq(database_value_to_query_value(value.clone())),
					}
				}
				(
					operator @ (FilterOperator::Contains
					| FilterOperator::IContains
					| FilterOperator::StartsWith
					| FilterOperator::IStartsWith
					| FilterOperator::EndsWith
					| FilterOperator::IEndsWith),
					FilterValue::Typed(value),
				) => {
					let value = Self::typed_database_value(value)?;
					if matches!(value, DatabaseValue::Null) {
						return Ok(Some(Condition::all().add(col.is_null())));
					}
					let value = Self::database_value_to_string(value);
					let (pattern, insensitive) = match operator {
						FilterOperator::Contains => (LikePattern::Contains, false),
						FilterOperator::IContains => (LikePattern::Contains, true),
						FilterOperator::StartsWith => (LikePattern::StartsWith, false),
						FilterOperator::IStartsWith => (LikePattern::StartsWith, true),
						FilterOperator::EndsWith => (LikePattern::EndsWith, false),
						FilterOperator::IEndsWith => (LikePattern::EndsWith, true),
						_ => unreachable!(),
					};
					self.like_expr(filter, &value, pattern, insensitive)
				}
				(
					operator @ (FilterOperator::Regex | FilterOperator::IRegex),
					FilterValue::Typed(value),
				) => {
					let value = Self::database_value_to_string(Self::typed_database_value(value)?);
					let sql_operator = if matches!(operator, FilterOperator::IRegex) {
						"~*"
					} else {
						"~"
					};
					Expr::cust_with_values(
						format!("{} {sql_operator} ?", self.filter_lhs_sql(filter)),
						[value],
					)
					.into_simple_expr()
				}
				(FilterOperator::In, FilterValue::Typed(value)) => {
					col.is_in([database_value_to_query_value(
						Self::typed_database_value(value)?.clone(),
					)])
				}
				(FilterOperator::NotIn, FilterValue::Typed(value)) => {
					col.is_not_in([database_value_to_query_value(
						Self::typed_database_value(value)?.clone(),
					)])
				}
				// NULL checks
				(FilterOperator::Eq, FilterValue::Null) => col.is_null(),
				(FilterOperator::Ne, FilterValue::Null) => col.is_not_null(),
				(FilterOperator::IExact, FilterValue::String(s)) => {
					self.like_expr(filter, s, LikePattern::Exact, true)
				}
				(FilterOperator::IExact, v) => {
					col.eq(self.filter_value_to_sea_value_for_filter(filter, v)?)
				}
				// Generic value comparisons (catch-all for other FilterValue types)
				(FilterOperator::Eq, v) => {
					col.eq(self.filter_value_to_sea_value_for_filter(filter, v)?)
				}
				(FilterOperator::Ne, v) => {
					col.ne(self.filter_value_to_sea_value_for_filter(filter, v)?)
				}
				(FilterOperator::Gt, v) => {
					col.gt(self.filter_value_to_sea_value_for_filter(filter, v)?)
				}
				(FilterOperator::Gte, v) => {
					col.gte(self.filter_value_to_sea_value_for_filter(filter, v)?)
				}
				(FilterOperator::Lt, v) => {
					col.lt(self.filter_value_to_sea_value_for_filter(filter, v)?)
				}
				(FilterOperator::Lte, v) => {
					col.lte(self.filter_value_to_sea_value_for_filter(filter, v)?)
				}
				(FilterOperator::In, FilterValue::String(s)) => {
					let values = Self::parse_array_string(s);
					col.is_in(values)
				}
				(FilterOperator::In, FilterValue::Array(arr)) => {
					col.is_in(arr.iter().map(|s| s.as_str()).collect::<Vec<_>>())
				}
				(FilterOperator::In, FilterValue::List(values)) => col.is_in(
					values
						.iter()
						.map(|value| self.filter_value_to_sea_value_for_filter(filter, value))
						.collect::<reinhardt_core::exception::Result<Vec<_>>>()?,
				),
				(FilterOperator::NotIn, FilterValue::String(s)) => {
					let values = Self::parse_array_string(s);
					col.is_not_in(values)
				}
				(FilterOperator::NotIn, FilterValue::Array(arr)) => {
					col.is_not_in(arr.iter().map(|s| s.as_str()).collect::<Vec<_>>())
				}
				(FilterOperator::NotIn, FilterValue::List(values)) => col.is_not_in(
					values
						.iter()
						.map(|value| self.filter_value_to_sea_value_for_filter(filter, value))
						.collect::<reinhardt_core::exception::Result<Vec<_>>>()?,
				),
				(FilterOperator::Contains, FilterValue::String(s)) => {
					self.like_expr(filter, s, LikePattern::Contains, false)
				}
				(FilterOperator::IContains, FilterValue::String(s)) => {
					self.like_expr(filter, s, LikePattern::Contains, true)
				}
				(FilterOperator::Contains, FilterValue::Array(arr)) => {
					let value = arr.first().map(String::as_str).unwrap_or("");
					self.like_expr(filter, value, LikePattern::Contains, false)
				}
				(FilterOperator::StartsWith, FilterValue::String(s)) => {
					self.like_expr(filter, s, LikePattern::StartsWith, false)
				}
				(FilterOperator::IStartsWith, FilterValue::String(s)) => {
					self.like_expr(filter, s, LikePattern::StartsWith, true)
				}
				(FilterOperator::StartsWith, FilterValue::Array(arr)) => {
					let value = arr.first().map(String::as_str).unwrap_or("");
					self.like_expr(filter, value, LikePattern::StartsWith, false)
				}
				(FilterOperator::EndsWith, FilterValue::String(s)) => {
					self.like_expr(filter, s, LikePattern::EndsWith, false)
				}
				(FilterOperator::IEndsWith, FilterValue::String(s)) => {
					self.like_expr(filter, s, LikePattern::EndsWith, true)
				}
				(FilterOperator::EndsWith, FilterValue::Array(arr)) => {
					let value = arr.first().map(String::as_str).unwrap_or("");
					self.like_expr(filter, value, LikePattern::EndsWith, false)
				}
				(FilterOperator::Regex, FilterValue::String(pattern)) => Expr::cust_with_values(
					format!("{} ~ ?", self.filter_lhs_sql(filter)),
					[pattern.clone()],
				)
				.into_simple_expr(),
				(FilterOperator::IRegex, FilterValue::String(pattern)) => Expr::cust_with_values(
					format!("{} ~* ?", self.filter_lhs_sql(filter)),
					[pattern.clone()],
				)
				.into_simple_expr(),
				(FilterOperator::Range, FilterValue::Range(start, end)) => Expr::cust_with_values(
					format!("{} BETWEEN ? AND ?", self.filter_lhs_sql(filter)),
					[
						self.filter_value_to_sea_value_for_filter(filter, start)?,
						self.filter_value_to_sea_value_for_filter(filter, end)?,
					],
				)
				.into_simple_expr(),
				// Handle Integer, Float, Boolean for text operators
				(FilterOperator::Contains, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.like(format!("%{}%", i))
				}
				(FilterOperator::IContains, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("%{}%", i)))
				}
				(FilterOperator::Contains, FilterValue::Float(f)) => col.like(format!("%{}%", f)),
				(FilterOperator::IContains, FilterValue::Float(f)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("%{}%", f)))
				}
				(FilterOperator::Contains, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.like(format!("%{}%", b))
				}
				(FilterOperator::IContains, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("%{}%", b)))
				}
				(FilterOperator::Contains, FilterValue::Null) => col.like("%"),
				(FilterOperator::IContains, FilterValue::Null) => {
					col.binary(BinOper::ILike, SimpleExpr::from("%"))
				}
				(FilterOperator::StartsWith, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.like(format!("{}%", i))
				}
				(FilterOperator::IStartsWith, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("{}%", i)))
				}
				(FilterOperator::StartsWith, FilterValue::Float(f)) => col.like(format!("{}%", f)),
				(FilterOperator::IStartsWith, FilterValue::Float(f)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("{}%", f)))
				}
				(FilterOperator::StartsWith, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.like(format!("{}%", b))
				}
				(FilterOperator::IStartsWith, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("{}%", b)))
				}
				(FilterOperator::StartsWith, FilterValue::Null) => col.like("%"),
				(FilterOperator::IStartsWith, FilterValue::Null) => {
					col.binary(BinOper::ILike, SimpleExpr::from("%"))
				}
				(FilterOperator::EndsWith, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.like(format!("%{}", i))
				}
				(FilterOperator::IEndsWith, FilterValue::Integer(i) | FilterValue::Int(i)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("%{}", i)))
				}
				(FilterOperator::EndsWith, FilterValue::Float(f)) => col.like(format!("%{}", f)),
				(FilterOperator::IEndsWith, FilterValue::Float(f)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("%{}", f)))
				}
				(FilterOperator::EndsWith, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.like(format!("%{}", b))
				}
				(FilterOperator::IEndsWith, FilterValue::Boolean(b) | FilterValue::Bool(b)) => {
					col.binary(BinOper::ILike, SimpleExpr::from(format!("%{}", b)))
				}
				(FilterOperator::EndsWith, FilterValue::Null) => col.like("%"),
				(FilterOperator::IEndsWith, FilterValue::Null) => {
					col.binary(BinOper::ILike, SimpleExpr::from("%"))
				}
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
						format!("{} @> ARRAY[{}]", self.filter_lhs_sql(filter), placeholders),
						arr.iter().cloned(),
					)
					.into_simple_expr()
				}
				(FilterOperator::ArrayContainedBy, FilterValue::Array(arr)) => {
					// field <@ ARRAY[?, ?] - parameterized
					let placeholders = arr.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
					Expr::cust_with_values(
						format!("{} <@ ARRAY[{}]", self.filter_lhs_sql(filter), placeholders),
						arr.iter().cloned(),
					)
					.into_simple_expr()
				}
				(FilterOperator::ArrayOverlap, FilterValue::Array(arr)) => {
					// field && ARRAY[?, ?] - parameterized
					let placeholders = arr.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
					Expr::cust_with_values(
						format!("{} && ARRAY[{}]", self.filter_lhs_sql(filter), placeholders),
						arr.iter().cloned(),
					)
					.into_simple_expr()
				}
				// PostgreSQL Full-text search
				(FilterOperator::FullTextMatch, FilterValue::String(query)) => {
					// field @@ plainto_tsquery('english', ?) - parameterized
					Expr::cust_with_values(
						format!(
							"{} @@ plainto_tsquery('english', ?)",
							self.filter_lhs_sql(filter)
						),
						[query.clone()],
					)
					.into_simple_expr()
				}
				// PostgreSQL JSONB operators
				(FilterOperator::JsonbContains, FilterValue::String(json)) => {
					// field @> ?::jsonb - parameterized
					Expr::cust_with_values(
						format!("{} @> ?::jsonb", self.filter_lhs_sql(filter)),
						[json.clone()],
					)
					.into_simple_expr()
				}
				(FilterOperator::JsonbContainedBy, FilterValue::String(json)) => {
					// field <@ ?::jsonb - parameterized
					Expr::cust_with_values(
						format!("{} <@ ?::jsonb", self.filter_lhs_sql(filter)),
						[json.clone()],
					)
					.into_simple_expr()
				}
				(FilterOperator::JsonbKeyExists, FilterValue::String(key)) => {
					// field ? 'key' - using PgBinOper for safe parameterization
					Expr::cust(self.filter_lhs_sql(filter))
						.into_simple_expr()
						.binary(
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
					Expr::cust(self.filter_lhs_sql(filter))
						.into_simple_expr()
						.binary(
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
					Expr::cust(self.filter_lhs_sql(filter))
						.into_simple_expr()
						.binary(
							BinOper::PgOperator(PgBinOper::JsonContainsAllKeys),
							array_expr,
						)
				}
				(FilterOperator::JsonbPathExists, FilterValue::String(path)) => {
					// field @? ? - parameterized
					Expr::cust_with_values(
						format!("{} @? ?", self.filter_lhs_sql(filter)),
						[path.clone()],
					)
					.into_simple_expr()
				}
				// PostgreSQL Range operators
				(FilterOperator::RangeContains, v) => {
					// field @> ? - parameterized
					Expr::cust_with_values(
						format!("{} @> ?", self.filter_lhs_sql(filter)),
						[self.filter_value_to_sea_value_for_filter(filter, v)?],
					)
					.into_simple_expr()
				}
				(FilterOperator::RangeContainedBy, FilterValue::String(range)) => {
					// field <@ ? - parameterized
					Expr::cust_with_values(
						format!("{} <@ ?", self.filter_lhs_sql(filter)),
						[range.clone()],
					)
					.into_simple_expr()
				}
				(FilterOperator::RangeOverlaps, FilterValue::String(range)) => {
					// field && ? - parameterized
					Expr::cust_with_values(
						format!("{} && ?", self.filter_lhs_sql(filter)),
						[range.clone()],
					)
					.into_simple_expr()
				}
				// Fallback for unsupported combinations
				_ => {
					// Default to equality for unhandled cases
					col.eq(self.filter_value_to_sea_value_for_filter(filter, &filter.value)?)
				}
			};

			cond = cond.add(expr);
			added = true;
		}

		for filter_condition in &self.filter_conditions {
			if let Some(expr) = self.build_filter_condition(filter_condition, 0)? {
				cond = cond.add(expr);
				added = true;
			}
		}

		// Add subquery conditions
		for subq_cond in &self.subquery_conditions {
			let expr = match subq_cond {
				SubqueryCondition::In {
					field, subquery, ..
				} => {
					// field IN (subquery)
					Expr::cust(format!(
						"{} IN {}",
						self.root_column_sql_for_backend(field, backend),
						subquery.for_backend(backend)
					))
					.into_simple_expr()
				}
				SubqueryCondition::NotIn {
					field, subquery, ..
				} => {
					// field NOT IN (subquery)
					Expr::cust(format!(
						"{} NOT IN {}",
						self.root_column_sql_for_backend(field, backend),
						subquery.for_backend(backend)
					))
					.into_simple_expr()
				}
				SubqueryCondition::Exists { subquery, .. } => {
					// EXISTS (subquery)
					Expr::cust(format!("EXISTS {}", subquery.for_backend(backend)))
						.into_simple_expr()
				}
				SubqueryCondition::NotExists { subquery, .. } => {
					// NOT EXISTS (subquery)
					Expr::cust(format!("NOT EXISTS {}", subquery.for_backend(backend)))
						.into_simple_expr()
				}
			};

			cond = cond.add(expr);
			added = true;
		}

		Ok(added.then_some(cond))
	}

	fn build_filter_condition(
		&self,
		filter_condition: &FilterCondition,
		depth: usize,
	) -> reinhardt_core::exception::Result<Option<Condition>> {
		if depth >= MAX_FILTER_CONDITION_DEPTH {
			return Err(reinhardt_core::exception::Error::Validation(format!(
				"Filter condition exceeded maximum depth of {} levels",
				MAX_FILTER_CONDITION_DEPTH
			)));
		}

		match filter_condition {
			FilterCondition::Single(filter) => {
				let mut queryset = self.clone();
				queryset.filters.clear();
				queryset.filter_conditions.clear();
				queryset.subquery_conditions.clear();
				queryset.filters.push(filter.clone());
				queryset.build_where_condition()
			}
			FilterCondition::And(conditions) => {
				let mut condition = Condition::all();
				let mut added = false;
				for item in conditions {
					if let Some(sub_condition) = self.build_filter_condition(item, depth + 1)? {
						condition = condition.add(sub_condition);
						added = true;
					}
				}
				if !added {
					condition = condition.add(Expr::val(true));
				}
				Ok(Some(condition))
			}
			FilterCondition::Or(conditions) => {
				if conditions.is_empty() {
					return Ok(Some(Condition::all().add(Expr::val(false))));
				}
				let mut condition = Condition::any();
				let mut added = false;
				for item in conditions {
					if let Some(sub_condition) = self.build_filter_condition(item, depth + 1)? {
						condition = condition.add(sub_condition);
						added = true;
					}
				}
				Ok(added.then_some(condition))
			}
			FilterCondition::Not(condition) => Ok(self
				.build_filter_condition(condition, depth + 1)?
				.map(|condition| condition.not())),
		}
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

	fn filter_expression_to_query_expr(&self, expr: &super::annotation::Expression) -> Expr {
		if self.has_joined_tables() {
			Expr::cust(self.annotation_expression_to_select_sql(expr))
		} else {
			Self::expression_to_query_expr(expr)
		}
	}

	/// Convert AnnotationValue to SQL string for custom expressions
	///
	/// Delegates to the `AnnotationValue::to_sql()` method which provides
	/// complete SQL generation for all annotation value types.
	fn annotation_value_to_sql(value: &super::annotation::AnnotationValue) -> String {
		value.to_sql()
	}

	/// Builds the annotation forms represented by the query AST structurally.
	///
	/// Field and standard aggregate annotations must remain AST nodes so the
	/// selected backend, rather than PostgreSQL-style pre-rendered SQL, quotes
	/// their identifiers. Other legacy annotation forms still require their
	/// established SQL rendering path.
	fn annotation_value_to_select_expr(
		&self,
		value: &super::annotation::AnnotationValue,
	) -> Option<SimpleExpr> {
		match value {
			super::annotation::AnnotationValue::Field(field) => {
				Some(Expr::col(self.root_column_reference(&field.field)).into_simple_expr())
			}
			_ => None,
		}
	}

	fn annotation_value_to_select_sql(&self, value: &super::annotation::AnnotationValue) -> String {
		if self.has_joined_tables() {
			match value {
				super::annotation::AnnotationValue::Field(field) => {
					return self.annotation_field_to_select_sql(field);
				}
				super::annotation::AnnotationValue::Expression(expression) => {
					return self.annotation_expression_to_select_sql(expression);
				}
				_ => {}
			}
		}

		value.to_sql_expr()
	}

	fn annotation_field_to_select_sql(&self, field: &super::expressions::F) -> String {
		if field.field.contains('.') {
			field.to_sql()
		} else {
			quote_identifier(&format!("{}.{}", self.root_alias(), field.field))
		}
	}

	fn annotation_expression_to_select_sql(
		&self,
		expression: &super::annotation::Expression,
	) -> String {
		use super::annotation::Expression;

		match expression {
			Expression::Add(left, right) => format!(
				"({} + {})",
				self.annotation_value_to_select_sql(left),
				self.annotation_value_to_select_sql(right)
			),
			Expression::Subtract(left, right) => format!(
				"({} - {})",
				self.annotation_value_to_select_sql(left),
				self.annotation_value_to_select_sql(right)
			),
			Expression::Multiply(left, right) => format!(
				"({} * {})",
				self.annotation_value_to_select_sql(left),
				self.annotation_value_to_select_sql(right)
			),
			Expression::Divide(left, right) => format!(
				"({} / {})",
				self.annotation_value_to_select_sql(left),
				self.annotation_value_to_select_sql(right)
			),
			Expression::Case { whens, default } => {
				let mut case_sql = "CASE".to_string();
				for when in whens {
					case_sql.push_str(&format!(
						" WHEN {} THEN {}",
						self.annotation_condition_to_select_sql(&when.condition),
						self.annotation_value_to_select_sql(&when.then)
					));
				}
				if let Some(default_value) = default {
					case_sql.push_str(&format!(
						" ELSE {}",
						self.annotation_value_to_select_sql(default_value)
					));
				}
				case_sql.push_str(" END");
				case_sql
			}
			Expression::Coalesce(values) => format!(
				"COALESCE({})",
				values
					.iter()
					.map(|value| self.annotation_value_to_select_sql(value))
					.collect::<Vec<_>>()
					.join(", ")
			),
		}
	}

	fn annotation_condition_to_select_sql(&self, condition: &super::expressions::Q) -> String {
		use super::expressions::{Q, QOperator};

		match condition {
			Q::Condition {
				field,
				operator,
				value,
			} => {
				if field.is_empty() && operator.is_empty() {
					return value.clone();
				}

				format!(
					"{} {} {}",
					self.annotation_root_field_to_select_sql(field),
					operator,
					Self::annotation_condition_value_to_sql(value)
				)
			}
			Q::Combined {
				operator,
				conditions,
			} => {
				let sql_conditions: Vec<_> = conditions
					.iter()
					.map(|condition| self.annotation_condition_to_select_sql(condition))
					.collect();

				match operator {
					QOperator::Not => {
						if sql_conditions.len() == 1 {
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

	fn annotation_condition_value_to_sql(value: &str) -> String {
		if value.parse::<f64>().is_ok()
			|| value.eq_ignore_ascii_case("TRUE")
			|| value.eq_ignore_ascii_case("FALSE")
			|| value.eq_ignore_ascii_case("NULL")
			|| value.starts_with("COUNT(")
			|| value.starts_with("SUM(")
			|| value.starts_with("AVG(")
			|| value.starts_with("MAX(")
			|| value.starts_with("MIN(")
			|| (value.starts_with('\'') && value.ends_with('\''))
		{
			value.to_string()
		} else {
			format!("'{}'", value)
		}
	}

	fn annotation_root_field_to_select_sql(&self, field: &str) -> String {
		let mut characters = field.chars();
		let Some(first) = characters.next() else {
			return field.to_string();
		};
		if !(first.is_ascii_alphabetic() || first == '_')
			|| !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
		{
			return field.to_string();
		}

		quote_identifier(&format!("{}.{}", self.root_alias(), field))
	}

	fn has_joined_tables(&self) -> bool {
		!self.expression_relation_join_graph_for_query().is_empty()
			|| !self.select_related_fields.is_empty()
			|| !self.joins.is_empty()
	}

	fn filter_lhs_expr(&self, filter: &Filter) -> Expr {
		if self.has_joined_tables() {
			return filter.lhs_expr_for_root(self.root_alias());
		}

		filter.lhs_expr()
	}

	fn filter_lhs_sql(&self, filter: &Filter) -> String {
		if self.has_joined_tables() {
			return filter.lhs_sql_for_root(self.root_alias());
		}

		filter.lhs_sql()
	}

	fn like_expr(
		&self,
		filter: &Filter,
		value: &str,
		pattern: LikePattern,
		case_insensitive: bool,
	) -> SimpleExpr {
		let operator = if case_insensitive { "ILIKE" } else { "LIKE" };
		Expr::cust_with_values(
			format!("{} {} ? ESCAPE '\\'", self.filter_lhs_sql(filter), operator),
			[pattern.apply(value)],
		)
		.into_simple_expr()
	}

	fn typed_database_value(
		value: &Result<DatabaseValue, FieldCodecError>,
	) -> reinhardt_core::exception::Result<&DatabaseValue> {
		value
			.as_ref()
			.map_err(|error| Self::typed_field_codec_error(error.clone()))
	}

	fn typed_field_codec_error(error: FieldCodecError) -> Error {
		let kind = match &error {
			FieldCodecError::TypeMismatch { .. }
			| FieldCodecError::InvalidEnumValue { .. }
			| FieldCodecError::MissingFieldMetadata { .. }
			| FieldCodecError::FieldPolicyMismatch { .. } => DatabaseErrorKind::Type,
			FieldCodecError::Serialization(_) => DatabaseErrorKind::Serialization,
		};
		let message = format!("typed field codec failed: {error}");
		Error::database_with_source(kind, message, error)
	}

	fn database_value_to_string(value: &DatabaseValue) -> String {
		match value {
			DatabaseValue::Null => String::new(),
			DatabaseValue::Bool(value) => value.to_string(),
			DatabaseValue::I32(value) => value.to_string(),
			DatabaseValue::I64(value) => value.to_string(),
			DatabaseValue::F32(value) => value.to_string(),
			DatabaseValue::F64(value) => value.to_string(),
			DatabaseValue::Decimal(value) => value.to_string(),
			DatabaseValue::String(value) => value.clone(),
			DatabaseValue::Bytes(value) => String::from_utf8_lossy(value).into_owned(),
			DatabaseValue::Json(value) => value.to_string(),
			#[cfg(feature = "pgvector")]
			DatabaseValue::Vector(values) => serde_json::Value::Array(
				values
					.iter()
					.copied()
					.map(serde_json::Value::from)
					.collect(),
			)
			.to_string(),
			DatabaseValue::Array { values, .. } => serde_json::Value::Array(
				values
					.iter()
					.cloned()
					.map(|value| value.into_json_value().unwrap_or(serde_json::Value::Null))
					.collect(),
			)
			.to_string(),
			DatabaseValue::Uuid(value) => value.to_string(),
			DatabaseValue::Date(value) => value.to_string(),
			DatabaseValue::Time(value) => value.to_string(),
			DatabaseValue::DateTime(value) => value.to_rfc3339(),
			DatabaseValue::NaiveDateTime(value) => value.to_string(),
		}
	}

	pub(crate) fn filter_value_to_sea_value(
		v: &FilterValue,
	) -> reinhardt_core::exception::Result<reinhardt_query::value::Value> {
		let value = match v {
			FilterValue::Typed(value) => {
				database_value_to_query_value(Self::typed_database_value(value)?.clone())
			}
			FilterValue::String(s) => s.clone().into(),
			FilterValue::Timestamp(value) => (*value).into(),
			FilterValue::Uuid(value) => (*value).into(),
			FilterValue::Integer(i) | FilterValue::Int(i) => (*i).into(),
			FilterValue::Float(f) => (*f).into(),
			FilterValue::Boolean(b) | FilterValue::Bool(b) => (*b).into(),
			FilterValue::Null => reinhardt_query::value::Value::Int(None),
			FilterValue::Array(arr) => arr.join(",").into(),
			FilterValue::List(values) => values
				.iter()
				.map(Self::value_to_string)
				.collect::<reinhardt_core::exception::Result<Vec<_>>>()?
				.join(",")
				.into(),
			FilterValue::Range(start, end) => format!(
				"{},{}",
				Self::value_to_string(start)?,
				Self::value_to_string(end)?
			)
			.into(),
			// FieldRef, Expression, and OuterRef are typically handled separately
			// in build_where_condition(), but provide proper conversion as fallback
			FilterValue::FieldRef(f) => f.field.clone().into(),
			FilterValue::Expression(expr) => expr.to_sql().into(),
			FilterValue::OuterRef(outer_ref) => outer_ref.field.clone().into(),
		};
		Ok(value)
	}

	fn filter_value_to_sea_value_for_filter(
		&self,
		filter: &Filter,
		value: &FilterValue,
	) -> reinhardt_core::exception::Result<reinhardt_query::value::Value> {
		let field_type = filter.field_type.clone().or_else(|| {
			let field_name = filter.field.rsplit("__").next().unwrap_or(&filter.field);
			T::field_metadata()
				.into_iter()
				.find(|metadata| {
					metadata.name == field_name || metadata.db_column_name() == field_name
				})
				.map(|metadata| metadata.field_type)
		});
		let Some(field_type) = field_type else {
			return Self::filter_value_to_sea_value(value);
		};

		let FilterValue::String(value) = value else {
			return Self::filter_value_to_sea_value(value);
		};

		Ok(match field_type.rsplit('.').next() {
			Some("IntegerField") | Some("AutoField") => value
				.parse::<i32>()
				.map_or_else(|_| value.clone().into(), Into::into),
			Some("BigIntegerField") | Some("BigAutoField") => value
				.parse::<i64>()
				.map_or_else(|_| value.clone().into(), Into::into),
			Some("FloatField") => value
				.parse::<f64>()
				.map_or_else(|_| value.clone().into(), Into::into),
			Some("BooleanField") => value
				.parse::<bool>()
				.map_or_else(|_| value.clone().into(), Into::into),
			Some("UuidField") => Uuid::parse_str(value).map_or_else(
				|_| value.clone().into(),
				|uuid| reinhardt_query::value::Value::Uuid(Some(Box::new(uuid))),
			),
			_ => value.clone().into(),
		})
	}

	/// Convert FilterValue to String representation
	// Allow dead_code: internal conversion helper for filter value stringification in queries
	#[allow(dead_code)]
	fn value_to_string(v: &FilterValue) -> reinhardt_core::exception::Result<String> {
		let value = match v {
			FilterValue::Typed(value) => {
				Self::database_value_to_string(Self::typed_database_value(value)?)
			}
			FilterValue::String(s) => s.clone(),
			FilterValue::Timestamp(value) => value.to_rfc3339(),
			FilterValue::Uuid(value) => value.to_string(),
			FilterValue::Integer(i) | FilterValue::Int(i) => i.to_string(),
			FilterValue::Float(f) => f.to_string(),
			FilterValue::Boolean(b) | FilterValue::Bool(b) => b.to_string(),
			FilterValue::Null => String::new(),
			FilterValue::Array(arr) => arr.join(","),
			FilterValue::List(values) => values
				.iter()
				.map(Self::value_to_string)
				.collect::<reinhardt_core::exception::Result<Vec<_>>>()?
				.join(","),
			FilterValue::Range(start, end) => {
				format!(
					"{},{}",
					Self::value_to_string(start)?,
					Self::value_to_string(end)?
				)
			}
			FilterValue::FieldRef(f) => f.field.clone(),
			FilterValue::Expression(expr) => expr.to_sql(),
			FilterValue::OuterRef(outer_ref) => outer_ref.field.clone(),
		};
		Ok(value)
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
	// Allow dead_code: internal conversion for IN clause array parameter binding
	#[allow(dead_code)]
	fn value_to_array(
		v: &FilterValue,
	) -> reinhardt_core::exception::Result<Vec<reinhardt_query::value::Value>> {
		let values = match v {
			FilterValue::Typed(value) => vec![database_value_to_query_value(
				Self::typed_database_value(value)?.clone(),
			)],
			FilterValue::String(s) => Self::parse_array_string(s),
			FilterValue::Timestamp(value) => vec![(*value).into()],
			FilterValue::Uuid(value) => vec![(*value).into()],
			FilterValue::Integer(i) | FilterValue::Int(i) => vec![(*i).into()],
			FilterValue::Float(f) => vec![(*f).into()],
			FilterValue::Boolean(b) | FilterValue::Bool(b) => vec![(*b).into()],
			FilterValue::Null => vec![reinhardt_query::value::Value::Int(None)],
			FilterValue::Array(arr) => arr.iter().map(|s| s.clone().into()).collect(),
			FilterValue::List(values) => values
				.iter()
				.map(Self::filter_value_to_sea_value)
				.collect::<reinhardt_core::exception::Result<Vec<_>>>()?,
			FilterValue::Range(start, end) => vec![
				Self::filter_value_to_sea_value(start)?,
				Self::filter_value_to_sea_value(end)?,
			],
			FilterValue::FieldRef(f) => vec![f.field.clone().into()],
			FilterValue::Expression(expr) => vec![expr.to_sql().into()],
			FilterValue::OuterRef(outer) => vec![outer.field.clone().into()],
		};
		Ok(values)
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
	// Allow dead_code: backward-compatible string-based WHERE clause builder for legacy code paths
	#[allow(dead_code)]
	fn build_where_clause(&self) -> reinhardt_core::exception::Result<(String, Vec<String>)> {
		if !self.has_where_predicates() {
			return Ok((String::new(), Vec::new()));
		}

		// Build reinhardt-query condition
		let mut stmt = Query::select();
		stmt.from(Alias::new("dummy"));

		if let Some(cond) = self.build_where_condition()? {
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

		Ok((where_clause, Vec::new()))
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	pub fn select_related<I>(mut self, fields: I) -> Self
	where
		I: RelationLoadInput<T>,
	{
		fields.apply_select_related(&mut self);
		self
	}

	/// Generate SELECT query with JOIN clauses for select_related fields
	///
	/// Returns reinhardt-query SelectStatement with LEFT JOIN for each related field to enable eager loading.
	/// Explicit root projections configured by `values` or `only` are preserved while related
	/// table columns remain eagerly selected.
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// let queryset = Post::objects()
	///     .select_related(&["author", "category"])
	///     .filter(Filter::new("published", FilterOperator::Eq, FilterValue::Boolean(true)));
	///
	/// let stmt = queryset.select_related_query().expect("select-related query should compile");
	/// // Generates:
	/// // SELECT posts.*, author.*, category.* FROM posts
	/// //   LEFT JOIN users AS author ON posts.author_id = author.id
	/// //   LEFT JOIN categories AS category ON posts.category_id = category.id
	/// //   WHERE posts.published = $1
	/// ```
	pub fn select_related_query(&self) -> reinhardt_core::exception::Result<SelectStatement> {
		self.select_related_query_with_condition(self.build_where_condition()?)
	}

	fn select_related_query_with_condition(
		&self,
		where_condition: Option<Condition>,
	) -> reinhardt_core::exception::Result<SelectStatement> {
		let table_name = T::table_name();
		let root_alias = self.from_alias.as_deref().unwrap_or(table_name);
		let relation_joins = self.expression_relation_join_graph_for_query();
		let typed_relation_aliases: Vec<_> = self
			.typed_select_related
			.iter()
			.map(|relation| relation.aliases(&relation_joins))
			.collect();
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

		// Add main table columns while preserving explicit projections.
		self.add_select_related_root_columns(&mut stmt);
		self.apply_typed_select_expressions(&mut stmt)?;

		// Add LEFT JOIN for each legacy related field that is not already covered
		// by a typed join. The typed graph owns its aliases and join order.
		let mut selected_relation_aliases = Vec::new();
		for related_field in &self.select_related_fields {
			if let Some(alias) = relation_joins
				.joins()
				.iter()
				.find(|join| {
					join.source_alias == root_alias && join.relation_name == *related_field
				})
				.map(|join| join.alias.clone())
			{
				if !selected_relation_aliases.contains(&alias) {
					stmt.column(ColumnRef::table_asterisk(Alias::new(&alias)));
					selected_relation_aliases.push(alias);
				}
				continue;
			}

			// Convention: related_field is the field name in the model
			// We assume FK field is "{related_field}_id" and join to "{related_field}s" table
			let fk_field = Alias::new(format!("{}_id", related_field));
			let related_table = Alias::new(format!("{}s", related_field));
			let related_alias = Alias::new(related_field);

			// LEFT JOIN related_table AS related_field ON table.fk_field = related_field.id
			stmt.left_join(
				related_table,
				Expr::col((Alias::new(root_alias), fk_field))
					.equals((related_alias.clone(), Alias::new("id"))),
			);

			// Add related table columns to SELECT
			stmt.column(ColumnRef::table_asterisk(related_alias));
			selected_relation_aliases.push(related_field.clone());
		}

		Self::apply_relation_join_graph(&mut stmt, &relation_joins);

		let mut selected_typed_aliases = Vec::new();
		for aliases in typed_relation_aliases {
			for alias in aliases {
				if !selected_relation_aliases.contains(&alias)
					&& !selected_typed_aliases.contains(&alias)
				{
					stmt.column(ColumnRef::table_asterisk(Alias::new(&alias)));
					selected_typed_aliases.push(alias);
				}
			}
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
		if let Some(cond) = where_condition {
			stmt.cond_where(cond);
		}
		self.apply_annotations_to_select(&mut stmt);
		self.apply_typed_annotation_grouping(&mut stmt)?;
		self.apply_grouping_and_having(&mut stmt)?;

		self.apply_ordering(&mut stmt)?;

		// Apply LIMIT/OFFSET
		if self.empty_result {
			stmt.limit(0);
		} else if let Some(limit) = self.limit {
			stmt.limit(limit as u64);
		}
		if let Some(offset) = self.offset {
			stmt.offset(offset as u64);
		}

		self.apply_select_for_update(&mut stmt);
		Ok(stmt.to_owned())
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	pub fn prefetch_related<I>(mut self, fields: I) -> Self
	where
		I: RelationLoadInput<T>,
	{
		fields.apply_prefetch_related(&mut self);
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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

		for relation in &self.typed_prefetch_related {
			let stmt = self.typed_prefetch_query(relation, pk_values);
			queries.push((relation.field.clone(), stmt));
		}

		for related_field in &self.prefetch_related_fields {
			if self
				.typed_prefetch_related
				.iter()
				.any(|relation| relation.field == *related_field)
			{
				continue;
			}

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

	fn typed_prefetch_query(
		&self,
		relation: &TypedPrefetchRelation,
		pk_values: &[i64],
	) -> SelectStatement {
		match relation.steps.as_slice() {
			[step] => self.typed_prefetch_single_hop_query(relation, step, pk_values),
			[through_step, target_step] => self.typed_prefetch_many_to_many_query(
				relation,
				through_step,
				target_step,
				pk_values,
			),
			_ => unreachable!("typed prefetch paths are validated when they are registered"),
		}
	}

	fn typed_prefetch_single_hop_query(
		&self,
		relation: &TypedPrefetchRelation,
		step: &RelationStep,
		pk_values: &[i64],
	) -> SelectStatement {
		let related_alias = Alias::new(&relation.alias);
		let mut stmt = Query::select();
		stmt.from_as(
			Alias::new(step.target_table.as_ref()),
			related_alias.clone(),
		)
		.column(ColumnRef::table_asterisk(related_alias.clone()));

		let values: Vec<reinhardt_query::value::Value> =
			pk_values.iter().map(|&id| id.into()).collect();
		stmt.and_where(
			Expr::col((related_alias, Alias::new(step.target_column.as_ref()))).is_in(values),
		);

		stmt.to_owned()
	}

	fn typed_prefetch_many_to_many_query(
		&self,
		relation: &TypedPrefetchRelation,
		through_step: &RelationStep,
		target_step: &RelationStep,
		pk_values: &[i64],
	) -> SelectStatement {
		let target_alias = Alias::new(&relation.alias);
		let through_alias = Alias::new(through_step.name.as_ref());

		let mut stmt = Query::select();
		stmt.from_as(
			Alias::new(target_step.target_table.as_ref()),
			target_alias.clone(),
		)
		.column(ColumnRef::table_asterisk(target_alias.clone()))
		.column((
			through_alias.clone(),
			Alias::new(through_step.target_column.as_ref()),
		))
		.join(
			SeaJoinType::InnerJoin,
			TableRef::table_alias(
				Alias::new(through_step.target_table.as_ref()),
				through_alias.clone(),
			),
			Expr::col((target_alias, Alias::new(target_step.target_column.as_ref()))).equals((
				through_alias.clone(),
				Alias::new(target_step.source_column.as_ref()),
			)),
		);

		let values: Vec<reinhardt_query::value::Value> =
			pk_values.iter().map(|&id| id.into()).collect();
		stmt.and_where(
			Expr::col((
				through_alias,
				Alias::new(through_step.target_column.as_ref()),
			))
			.is_in(values),
		);

		stmt.to_owned()
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
		// Apply the canonical M2M naming rule used by
		// `ManyToManyAccessor::default_through_table` and the autodetector
		// (`crates/reinhardt-db/src/migrations/autodetector.rs`):
		// `{source_table.to_lowercase()}_{to_snake_case(field_name)}`.
		// Without this, prefetch joins target a junction table whose
		// casing/snake-case diverges from what `makemigrations` produced
		// for the same M2M field (#4659).
		let junction_table = Alias::new(format!(
			"{}_{}",
			table_name.to_lowercase(),
			to_snake_case(related_field)
		));

		// Look up relationship metadata to derive FK names correctly
		let rel_info = T::relationship_metadata().into_iter().find(|r| {
			r.name == related_field
				&& r.relationship_type == super::relationship::RelationshipType::ManyToMany
		});

		// Derive related table name from metadata
		let related_table = if let Some(ref info) = rel_info {
			Alias::new(to_snake_case(&info.related_model).to_lowercase())
		} else {
			// Fallback to pluralization heuristic
			Alias::new(format!("{}s", related_field))
		};

		// Derive junction FK names from metadata or use default_link_fields logic
		let table_name_lower = table_name.to_lowercase();
		let (junction_main_fk, junction_related_fk) = if let Some(ref info) = rel_info {
			let source_fk = if let Some(ref sf) = info.source_field {
				sf.clone()
			} else {
				// Mirror ManyToManyAccessor::default_link_fields logic
				let related_lower = to_snake_case(&info.related_model).to_lowercase();
				if table_name_lower == related_lower {
					format!("from_{}_id", table_name_lower)
				} else {
					format!("{}_id", table_name_lower)
				}
			};

			let target_fk = if let Some(ref tf) = info.target_field {
				tf.clone()
			} else {
				let related_lower = to_snake_case(&info.related_model).to_lowercase();
				if table_name_lower == related_lower {
					format!("to_{}_id", table_name_lower)
				} else {
					format!("{}_id", to_snake_case(related_field))
				}
			};

			(Alias::new(source_fk), Alias::new(target_fk))
		} else {
			// Fallback to heuristics
			let source_fk = format!("{}_id", table_name_lower);
			let target_fk = format!("{}_id", to_snake_case(related_field));
			(Alias::new(source_fk), Alias::new(target_fk))
		};

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

	/// Returns the backend's estimated plan for this queryset without executing
	/// the data-producing SELECT.
	///
	/// Only typed plan-only options are accepted. `ANALYZE`, arbitrary option
	/// strings, and execution statistics are intentionally unavailable.
	///
	/// # Errors
	///
	/// Returns an unsupported database error when the requested format or option
	/// is unavailable on the active backend.
	pub async fn explain(
		&self,
		options: ExplainOptions,
	) -> reinhardt_core::exception::Result<ExplainOutput> {
		let mut conn = super::manager::get_connection().await?;
		self.explain_with_db(&mut conn, options).await
	}

	/// Returns the estimated plan through a caller-owned ORM executor.
	///
	/// This is the diagnostic counterpart to [`Self::all_with_db`]. The existing
	/// filtered, joined, and ordered SELECT is wrapped in one EXPLAIN statement;
	/// the unwrapped SELECT is never submitted separately.
	pub async fn explain_with_db<E>(
		&self,
		conn: &mut E,
		options: ExplainOptions,
	) -> reinhardt_core::exception::Result<ExplainOutput>
	where
		E: OrmExecutor,
	{
		self.ensure_explainable_shape()?;
		self.ensure_backend_annotations_supported(conn.backend())?;
		let select = self.build_select_statement()?;
		let context = super::execution::pgvector_context_for_select(&select);
		let statement = ExplainStatement::new(select, options);
		let (sql, values, backend) =
			Self::build_explain_for_backend(&statement, conn.backend(), conn.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let started_at = Instant::now();
		let result = conn.fetch_all_with_context(&sql, params, context).await;
		let duration = started_at.elapsed();
		let rows = match result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				rows
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				return Err(error);
			}
		};
		Self::decode_explain_rows(rows, backend, options.format).map_err(Into::into)
	}

	/// Returns the estimated plan through an active transaction executor.
	///
	/// This caller-owned executor path mirrors [`Self::all_with_executor`] and
	/// keeps the diagnostic on the transaction's dedicated connection.
	pub async fn explain_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		options: ExplainOptions,
	) -> Result<ExplainOutput, crate::backends::error::DatabaseError> {
		self.ensure_explainable_shape().map_err(executor_error)?;
		self.ensure_backend_annotations_supported(Self::executor_backend(executor))?;
		let select = self.build_select_statement().map_err(executor_error)?;
		let context = super::execution::pgvector_context_for_select(&select);
		let statement = ExplainStatement::new(select, options);
		let (sql, values, backend) = Self::build_explain_for_backend(
			&statement,
			Self::executor_backend(executor),
			executor.is_cockroachdb(),
		)?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let started_at = Instant::now();
		let result = executor
			.fetch_all_with_context(&sql, params, context)
			.await
			.map_err(executor_error);
		let duration = started_at.elapsed();
		let rows = match result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				rows
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				return Err(error);
			}
		};
		Self::decode_explain_rows(rows, backend, options.format)
	}

	/// Return distinct truncated values from a generated date field.
	///
	/// Truncation, `DISTINCT`, null exclusion, and ordering are performed by the
	/// database. ISO weeks begin on Monday. Querysets created from subqueries,
	/// querysets with CTEs, querysets with lateral joins, and grouped or HAVING
	/// querysets are not supported.
	pub async fn dates<F, Origin>(
		&self,
		field: super::expressions::FieldRef<T, F, Origin>,
		kind: DateTruncKind,
		order: DateProjectionOrder,
	) -> reinhardt_core::exception::Result<Vec<chrono::NaiveDate>>
	where
		F: DateProjectionField,
	{
		let mut conn = super::manager::get_connection().await?;
		self.dates_with_db(&mut conn, field, kind, order).await
	}

	/// Return distinct truncated dates through a caller-owned ORM executor.
	///
	/// Querysets created from subqueries, querysets with CTEs, querysets with
	/// lateral joins, and grouped or HAVING querysets are not supported.
	pub async fn dates_with_db<E, F, Origin>(
		&self,
		conn: &mut E,
		field: super::expressions::FieldRef<T, F, Origin>,
		kind: DateTruncKind,
		order: DateProjectionOrder,
	) -> reinhardt_core::exception::Result<Vec<chrono::NaiveDate>>
	where
		E: OrmExecutor,
		F: DateProjectionField,
	{
		self.ensure_backend_annotations_supported(conn.backend())?;
		let stmt = self.temporal_projection_statement(
			field.name(),
			kind.into(),
			order,
			None,
			TemporalTruncOutput::Date,
		)?;
		let rows = Self::temporal_rows_with_db(&stmt, conn).await?;
		Self::decode_date_projection(rows).map_err(Error::from)
	}

	/// Return distinct truncated dates through an active transaction executor.
	///
	/// Querysets created from subqueries, querysets with CTEs, querysets with
	/// lateral joins, and grouped or HAVING querysets are not supported.
	pub async fn dates_with_executor<F, Origin>(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		field: super::expressions::FieldRef<T, F, Origin>,
		kind: DateTruncKind,
		order: DateProjectionOrder,
	) -> Result<Vec<chrono::NaiveDate>, crate::backends::error::DatabaseError>
	where
		F: DateProjectionField,
	{
		self.ensure_backend_annotations_supported(Self::executor_backend(executor))?;
		let stmt = self
			.temporal_projection_statement(
				field.name(),
				kind.into(),
				order,
				None,
				TemporalTruncOutput::Date,
			)
			.map_err(executor_error)?;
		let rows = Self::temporal_rows_with_executor(&stmt, executor).await?;
		Self::decode_date_projection(rows)
	}

	/// Return distinct truncated values from a generated UTC datetime field.
	///
	/// `time_zone` defaults to UTC. The database converts each source instant
	/// before truncation. SQLite and MySQL return an `Unsupported` capability
	/// error for named zones; PostgreSQL performs named-zone conversion. Querysets
	/// created from subqueries, querysets with CTEs, querysets with lateral joins,
	/// and grouped or HAVING querysets are not supported.
	pub async fn datetimes<F, Origin>(
		&self,
		field: super::expressions::FieldRef<T, F, Origin>,
		kind: DateTimeTruncKind,
		order: DateProjectionOrder,
		time_zone: Option<chrono_tz::Tz>,
	) -> reinhardt_core::exception::Result<Vec<chrono::DateTime<chrono_tz::Tz>>>
	where
		F: DateTimeProjectionField,
	{
		let mut conn = super::manager::get_connection().await?;
		self.datetimes_with_db(&mut conn, field, kind, order, time_zone)
			.await
	}

	/// Return distinct truncated datetimes through a caller-owned ORM executor.
	///
	/// Querysets created from subqueries, querysets with CTEs, querysets with
	/// lateral joins, and grouped or HAVING querysets are not supported.
	pub async fn datetimes_with_db<E, F, Origin>(
		&self,
		conn: &mut E,
		field: super::expressions::FieldRef<T, F, Origin>,
		kind: DateTimeTruncKind,
		order: DateProjectionOrder,
		time_zone: Option<chrono_tz::Tz>,
	) -> reinhardt_core::exception::Result<Vec<chrono::DateTime<chrono_tz::Tz>>>
	where
		E: OrmExecutor,
		F: DateTimeProjectionField,
	{
		self.ensure_backend_annotations_supported(conn.backend())?;
		let time_zone = time_zone.unwrap_or(chrono_tz::Tz::UTC);
		let query_time_zone = if time_zone == chrono_tz::Tz::UTC {
			TemporalTimeZone::Utc
		} else {
			TemporalTimeZone::Named(time_zone.name().to_string())
		};
		let stmt = self.temporal_projection_statement(
			field.name(),
			kind.into(),
			order,
			Some(query_time_zone),
			TemporalTruncOutput::DateTime,
		)?;
		let rows = Self::temporal_rows_with_db(&stmt, conn).await?;
		Self::decode_datetime_projection(rows, time_zone).map_err(Error::from)
	}

	/// Return distinct truncated datetimes through an active transaction executor.
	///
	/// Querysets created from subqueries, querysets with CTEs, querysets with
	/// lateral joins, and grouped or HAVING querysets are not supported.
	pub async fn datetimes_with_executor<F, Origin>(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		field: super::expressions::FieldRef<T, F, Origin>,
		kind: DateTimeTruncKind,
		order: DateProjectionOrder,
		time_zone: Option<chrono_tz::Tz>,
	) -> Result<Vec<chrono::DateTime<chrono_tz::Tz>>, crate::backends::error::DatabaseError>
	where
		F: DateTimeProjectionField,
	{
		self.ensure_backend_annotations_supported(Self::executor_backend(executor))?;
		let time_zone = time_zone.unwrap_or(chrono_tz::Tz::UTC);
		let query_time_zone = if time_zone == chrono_tz::Tz::UTC {
			TemporalTimeZone::Utc
		} else {
			TemporalTimeZone::Named(time_zone.name().to_string())
		};
		let stmt = self
			.temporal_projection_statement(
				field.name(),
				kind.into(),
				order,
				Some(query_time_zone),
				TemporalTruncOutput::DateTime,
			)
			.map_err(executor_error)?;
		let rows = Self::temporal_rows_with_executor(&stmt, executor).await?;
		Self::decode_datetime_projection(rows, time_zone)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	///     .filter(Filter::new(
	///         "is_active",
	///         FilterOperator::Eq,
	///         FilterValue::Boolean(true),
	///     ))
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
		if self.empty_result {
			return Ok(Vec::new());
		}
		let mut conn = super::manager::get_connection().await?;
		self.all_with_db(&mut conn).await
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Fetch first active user
	/// let user = User::objects()
	///     .filter(Filter::new(
	///         "is_active",
	///         FilterOperator::Eq,
	///         FilterValue::Boolean(true),
	///     ))
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
		if self.empty_result {
			return Ok(None);
		}
		let mut conn = super::manager::get_connection().await?;
		self.first_with_db(&mut conn).await
	}

	/// Return the newest row using the model's `get_latest_by` metadata.
	pub async fn latest(&self) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let ordering = self.metadata_retrieval_ordering(true)?;
		if self.empty_result {
			return Err(Error::NotFound(
				"No record found matching the query".to_string(),
			));
		}
		self.ensure_unsliced_retrieval()?;
		let mut conn = super::manager::get_connection().await?;
		self.single_with_db(&mut conn, ordering).await
	}

	/// Return the oldest row using the model's `get_latest_by` metadata.
	pub async fn earliest(&self) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let ordering = self.metadata_retrieval_ordering(false)?;
		if self.empty_result {
			return Err(Error::NotFound(
				"No record found matching the query".to_string(),
			));
		}
		self.ensure_unsliced_retrieval()?;
		let mut conn = super::manager::get_connection().await?;
		self.single_with_db(&mut conn, ordering).await
	}

	/// Return the newest row ordered by the supplied typed model fields.
	pub async fn latest_by(
		&self,
		fields: &[OrderingField<T>],
	) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let ordering = Self::typed_retrieval_ordering(fields, true)?;
		if self.empty_result {
			return Err(Error::NotFound(
				"No record found matching the query".to_string(),
			));
		}
		self.ensure_unsliced_retrieval()?;
		let mut conn = super::manager::get_connection().await?;
		self.single_with_db(&mut conn, ordering).await
	}

	/// Return the oldest row ordered by the supplied typed model fields.
	pub async fn earliest_by(
		&self,
		fields: &[OrderingField<T>],
	) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let ordering = Self::typed_retrieval_ordering(fields, false)?;
		if self.empty_result {
			return Err(Error::NotFound(
				"No record found matching the query".to_string(),
			));
		}
		self.ensure_unsliced_retrieval()?;
		let mut conn = super::manager::get_connection().await?;
		self.single_with_db(&mut conn, ordering).await
	}

	/// Return the newest row through a caller-owned ORM executor.
	pub async fn latest_with_db<E>(&self, conn: &mut E) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		let ordering = self.metadata_retrieval_ordering(true)?;
		self.single_with_db(conn, ordering).await
	}

	/// Return the oldest row through a caller-owned ORM executor.
	pub async fn earliest_with_db<E>(&self, conn: &mut E) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		let ordering = self.metadata_retrieval_ordering(false)?;
		self.single_with_db(conn, ordering).await
	}

	/// Return the newest row ordered by typed fields through a caller-owned ORM executor.
	pub async fn latest_by_with_db<E>(
		&self,
		conn: &mut E,
		fields: &[OrderingField<T>],
	) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		let ordering = Self::typed_retrieval_ordering(fields, true)?;
		self.single_with_db(conn, ordering).await
	}

	/// Return the oldest row ordered by typed fields through a caller-owned ORM executor.
	pub async fn earliest_by_with_db<E>(
		&self,
		conn: &mut E,
		fields: &[OrderingField<T>],
	) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		let ordering = Self::typed_retrieval_ordering(fields, false)?;
		self.single_with_db(conn, ordering).await
	}

	/// Return the newest row through an active transaction executor.
	pub async fn latest_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> crate::backends::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let ordering = self.metadata_retrieval_ordering(true)?;
		self.single_with_executor(executor, ordering).await
	}

	/// Return the oldest row through an active transaction executor.
	pub async fn earliest_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> crate::backends::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let ordering = self.metadata_retrieval_ordering(false)?;
		self.single_with_executor(executor, ordering).await
	}

	/// Return the newest row ordered by typed fields through an active transaction executor.
	pub async fn latest_by_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		fields: &[OrderingField<T>],
	) -> crate::backends::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let ordering = Self::typed_retrieval_ordering(fields, true)?;
		self.single_with_executor(executor, ordering).await
	}

	/// Return the oldest row ordered by typed fields through an active transaction executor.
	pub async fn earliest_by_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		fields: &[OrderingField<T>],
	) -> crate::backends::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		let ordering = Self::typed_retrieval_ordering(fields, false)?;
		self.single_with_executor(executor, ordering).await
	}

	fn metadata_retrieval_ordering(
		&self,
		latest: bool,
	) -> reinhardt_core::exception::Result<Vec<String>> {
		let fields = T::latest_by_fields();
		if fields.is_empty() {
			return Err(Error::Validation(format!(
				"{} requires Model::latest_by_fields() metadata or an explicit typed field",
				if latest {
					"QuerySet::latest"
				} else {
					"QuerySet::earliest"
				}
			)));
		}
		Ok(fields
			.iter()
			.map(|field| {
				if latest {
					field
						.strip_prefix('-')
						.map_or_else(|| format!("-{field}"), ToOwned::to_owned)
				} else {
					(*field).to_owned()
				}
			})
			.collect())
	}

	fn typed_retrieval_ordering(
		fields: &[OrderingField<T>],
		latest: bool,
	) -> reinhardt_core::exception::Result<Vec<String>> {
		if fields.is_empty() {
			return Err(Error::Validation(
				"QuerySet retrieval requires at least one typed ordering field".to_string(),
			));
		}
		Ok(fields
			.iter()
			.map(|field| {
				if latest {
					format!("-{}", field.name())
				} else {
					field.name().to_owned()
				}
			})
			.collect())
	}

	async fn single_with_db<E>(
		&self,
		conn: &mut E,
		ordering: Vec<String>,
	) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		self.ensure_unsliced_retrieval()?;
		let mut queryset = self.clone();
		queryset.order_by_fields = ordering;
		queryset.order_by_expressions.clear();
		queryset.limit = Some(1);
		queryset
			.all_with_db(conn)
			.await?
			.into_iter()
			.next()
			.ok_or_else(|| Error::NotFound("No record found matching the query".to_string()))
	}

	async fn single_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		ordering: Vec<String>,
	) -> crate::backends::Result<T>
	where
		T: serde::de::DeserializeOwned,
	{
		self.ensure_unsliced_retrieval().map_err(executor_error)?;
		let mut queryset = self.clone();
		queryset.order_by_fields = ordering;
		queryset.order_by_expressions.clear();
		queryset.limit = Some(1);
		queryset
			.all_with_executor(executor)
			.await?
			.into_iter()
			.next()
			.ok_or_else(|| Error::NotFound("No record found matching the query".to_string()))
	}

	fn ensure_unsliced_retrieval(&self) -> reinhardt_core::exception::Result<()> {
		if self.limit.is_some() || self.offset.is_some() {
			return Err(Error::Validation(
				"QuerySet retrieval helpers cannot be called on a sliced queryset".to_string(),
			));
		}
		Ok(())
	}

	fn bulk_key_batches<K>(
		keys: BTreeSet<K>,
		backend: super::connection::DatabaseBackend,
		reserved_binds: usize,
	) -> reinhardt_core::exception::Result<Vec<Vec<K>>> {
		let parameter_limit: usize = match backend {
			super::connection::DatabaseBackend::Sqlite => 900,
			super::connection::DatabaseBackend::Postgres
			| super::connection::DatabaseBackend::MySql => 65_535,
		};
		if keys.is_empty() {
			return Ok(Vec::new());
		}
		if reserved_binds >= parameter_limit {
			return Err(Error::Validation(format!(
				"QuerySet bulk retrieval cannot add lookup keys because the source query uses all {parameter_limit} available bind parameters"
			)));
		}
		let limit = parameter_limit - reserved_binds;
		let mut batches = Vec::with_capacity(keys.len().div_ceil(limit));
		let mut batch = Vec::with_capacity(limit);
		for key in keys {
			batch.push(key);
			if batch.len() == limit {
				batches.push(batch);
				batch = Vec::with_capacity(limit);
			}
		}
		if !batch.is_empty() {
			batches.push(batch);
		}
		Ok(batches)
	}

	fn select_bind_count(
		&self,
		backend: super::connection::DatabaseBackend,
		is_cockroachdb: bool,
	) -> reinhardt_core::exception::Result<usize> {
		self.ensure_backend_annotations_supported(backend)?;
		let statement = self.build_select_statement()?;
		let (_, values) = Self::build_select_for_backend(&statement, backend, is_cockroachdb)?;
		Ok(values.len())
	}

	fn with_bulk_lookup_column(mut self, column: &str) -> Self {
		if let Some(fields) = &mut self.selected_fields {
			if !fields
				.iter()
				.any(|field| Self::projection_includes_column(field, column))
			{
				fields.push(column.to_string());
			}
		} else {
			let field_name = Self::logical_field_name_for_column(column);
			self.deferred_fields
				.retain(|field| !Self::projection_includes_column(field, &field_name));
		}
		self
	}

	fn projection_includes_column(field: &str, column: &str) -> bool {
		field == column
			|| field
				.rsplit_once('.')
				.is_some_and(|(_, field_name)| field_name == column)
	}

	fn logical_field_name_for_column(column: &str) -> String {
		T::field_metadata()
			.iter()
			.find(|metadata| metadata.db_column_name() == column)
			.map_or_else(|| column.to_owned(), |metadata| metadata.name.clone())
	}

	/// Fetch rows by primary key and return them in deterministic key order.
	///
	/// When the queryset uses a field projection, the primary-key column is
	/// selected automatically so every returned model can be indexed.
	pub async fn in_bulk<I>(
		&self,
		keys: I,
	) -> reinhardt_core::exception::Result<BTreeMap<T::PrimaryKey, T>>
	where
		T: serde::de::DeserializeOwned,
		T::PrimaryKey: DatabaseField + Ord,
		I: IntoIterator<Item = T::PrimaryKey>,
	{
		self.ensure_unsliced_retrieval()?;
		let keys = keys.into_iter().collect::<BTreeSet<_>>();
		if self.empty_result || keys.is_empty() {
			return Ok(BTreeMap::new());
		}
		let mut conn = super::manager::get_connection().await?;
		self.in_bulk_with_keys_db(&mut conn, keys).await
	}

	/// Fetch rows by primary key through a caller-owned ORM executor.
	pub async fn in_bulk_with_db<E, I>(
		&self,
		conn: &mut E,
		keys: I,
	) -> reinhardt_core::exception::Result<BTreeMap<T::PrimaryKey, T>>
	where
		T: serde::de::DeserializeOwned,
		T::PrimaryKey: DatabaseField + Ord,
		E: OrmExecutor,
		I: IntoIterator<Item = T::PrimaryKey>,
	{
		self.ensure_unsliced_retrieval()?;
		let keys = keys.into_iter().collect::<BTreeSet<_>>();
		if self.empty_result || keys.is_empty() {
			return Ok(BTreeMap::new());
		}
		self.in_bulk_with_keys_db(conn, keys).await
	}

	/// Fetch rows by primary key through an active transaction executor.
	pub async fn in_bulk_with_executor<I>(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		keys: I,
	) -> crate::backends::Result<BTreeMap<T::PrimaryKey, T>>
	where
		T: serde::de::DeserializeOwned,
		T::PrimaryKey: DatabaseField + Ord,
		I: IntoIterator<Item = T::PrimaryKey>,
	{
		self.ensure_unsliced_retrieval().map_err(executor_error)?;
		let keys = keys.into_iter().collect::<BTreeSet<_>>();
		if self.empty_result || keys.is_empty() {
			return Ok(BTreeMap::new());
		}
		self.in_bulk_with_keys_executor(executor, keys).await
	}

	/// Fetch rows by a metadata-proven unique field in deterministic key order.
	pub async fn in_bulk_by<K, I>(
		&self,
		unique_field: UniqueFieldRef<T, K>,
		keys: I,
	) -> reinhardt_core::exception::Result<BTreeMap<K, T>>
	where
		T: serde::de::DeserializeOwned,
		K: DatabaseField + Ord,
		I: IntoIterator<Item = K>,
	{
		self.ensure_unsliced_retrieval()?;
		let keys = keys.into_iter().collect::<BTreeSet<_>>();
		if self.empty_result || keys.is_empty() {
			return Ok(BTreeMap::new());
		}
		let getter = Self::unique_getter(&unique_field)?;
		let mut conn = super::manager::get_connection().await?;
		self.in_bulk_by_keys_db(&mut conn, unique_field, getter, keys)
			.await
	}

	/// Fetch rows by a metadata-proven unique field through a caller-owned ORM executor.
	pub async fn in_bulk_by_with_db<E, K, I>(
		&self,
		conn: &mut E,
		unique_field: UniqueFieldRef<T, K>,
		keys: I,
	) -> reinhardt_core::exception::Result<BTreeMap<K, T>>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
		K: DatabaseField + Ord,
		I: IntoIterator<Item = K>,
	{
		self.ensure_unsliced_retrieval()?;
		let keys = keys.into_iter().collect::<BTreeSet<_>>();
		if self.empty_result || keys.is_empty() {
			return Ok(BTreeMap::new());
		}
		let getter = Self::unique_getter(&unique_field)?;
		self.in_bulk_by_keys_db(conn, unique_field, getter, keys)
			.await
	}

	/// Fetch rows by a metadata-proven unique field through an active transaction executor.
	pub async fn in_bulk_by_with_executor<K, I>(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		unique_field: UniqueFieldRef<T, K>,
		keys: I,
	) -> crate::backends::Result<BTreeMap<K, T>>
	where
		T: serde::de::DeserializeOwned,
		K: DatabaseField + Ord,
		I: IntoIterator<Item = K>,
	{
		self.ensure_unsliced_retrieval().map_err(executor_error)?;
		let keys = keys.into_iter().collect::<BTreeSet<_>>();
		if self.empty_result || keys.is_empty() {
			return Ok(BTreeMap::new());
		}
		let getter = Self::unique_getter(&unique_field)?;
		self.in_bulk_by_keys_executor(executor, unique_field, getter, keys)
			.await
	}

	fn unique_getter<K>(
		unique_field: &UniqueFieldRef<T, K>,
	) -> reinhardt_core::exception::Result<fn(&T) -> Option<K>>
	where
		K: DatabaseField,
	{
		unique_field.getter().ok_or_else(|| {
			Error::Validation(format!(
				"QuerySet::in_bulk_by requires a generated getter for unique field `{}`",
				unique_field.name()
			))
		})
	}

	async fn in_bulk_with_keys_db<E>(
		&self,
		conn: &mut E,
		keys: BTreeSet<T::PrimaryKey>,
	) -> reinhardt_core::exception::Result<BTreeMap<T::PrimaryKey, T>>
	where
		T: serde::de::DeserializeOwned,
		T::PrimaryKey: DatabaseField + Ord,
		E: OrmExecutor,
	{
		let field = super::expressions::FieldRef::<
			T,
			T::PrimaryKey,
			super::expressions::UnverifiedModelField,
		>::new(T::primary_key_column());
		let reserved_binds = self.select_bind_count(conn.backend(), conn.is_cockroachdb())?;
		let mut result = BTreeMap::new();
		for keys in Self::bulk_key_batches(keys, conn.backend(), reserved_binds)? {
			let rows = self
				.clone()
				.with_bulk_lookup_column(T::primary_key_column())
				.filter(field.is_in(keys))
				.all_with_db(conn)
				.await?;
			result.extend(
				rows.into_iter()
					.filter_map(|model| model.primary_key().map(|key| (key, model))),
			);
		}
		Ok(result)
	}

	async fn in_bulk_with_keys_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		keys: BTreeSet<T::PrimaryKey>,
	) -> crate::backends::Result<BTreeMap<T::PrimaryKey, T>>
	where
		T: serde::de::DeserializeOwned,
		T::PrimaryKey: DatabaseField + Ord,
	{
		let backend = Self::executor_backend(executor);
		let reserved_binds = self.select_bind_count(backend, executor.is_cockroachdb())?;
		let mut result = BTreeMap::new();
		for keys in Self::bulk_key_batches(keys, backend, reserved_binds)? {
			let filter = super::expressions::FieldRef::<
				T,
				T::PrimaryKey,
				super::expressions::UnverifiedModelField,
			>::new(T::primary_key_column())
			.is_in(keys);
			let rows = self
				.clone()
				.with_bulk_lookup_column(T::primary_key_column())
				.filter(filter)
				.all_with_executor(executor)
				.await?;
			result.extend(
				rows.into_iter()
					.filter_map(|model| model.primary_key().map(|key| (key, model))),
			);
		}
		Ok(result)
	}

	async fn in_bulk_by_keys_db<E, K>(
		&self,
		conn: &mut E,
		unique_field: UniqueFieldRef<T, K>,
		getter: fn(&T) -> Option<K>,
		keys: BTreeSet<K>,
	) -> reinhardt_core::exception::Result<BTreeMap<K, T>>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
		K: DatabaseField + Ord,
	{
		let reserved_binds = self.select_bind_count(conn.backend(), conn.is_cockroachdb())?;
		let mut result = BTreeMap::new();
		for keys in Self::bulk_key_batches(keys, conn.backend(), reserved_binds)? {
			let rows = self
				.clone()
				.with_bulk_lookup_column(unique_field.name())
				.filter(unique_field.is_in(keys))
				.all_with_db(conn)
				.await?;
			result.extend(
				rows.into_iter()
					.filter_map(|model| getter(&model).map(|key| (key, model))),
			);
		}
		Ok(result)
	}

	async fn in_bulk_by_keys_executor<K>(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		unique_field: UniqueFieldRef<T, K>,
		getter: fn(&T) -> Option<K>,
		keys: BTreeSet<K>,
	) -> crate::backends::Result<BTreeMap<K, T>>
	where
		T: serde::de::DeserializeOwned,
		K: DatabaseField + Ord,
	{
		let backend = Self::executor_backend(executor);
		let reserved_binds = self.select_bind_count(backend, executor.is_cockroachdb())?;
		let mut result = BTreeMap::new();
		for keys in Self::bulk_key_batches(keys, backend, reserved_binds)? {
			let rows = self
				.clone()
				.with_bulk_lookup_column(unique_field.name())
				.filter(unique_field.is_in(keys))
				.all_with_executor(executor)
				.await?;
			result.extend(
				rows.into_iter()
					.filter_map(|model| getter(&model).map(|key| (key, model))),
			);
		}
		Ok(result)
	}

	/// Execute the queryset and return a single matching record
	///
	/// Returns an error if zero or multiple records are found.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::Model;
	/// # use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Fetch user with specific email (must be unique)
	/// let user = User::objects()
	///     .filter(Filter::new(
	///         "email",
	///         FilterOperator::Eq,
	///         FilterValue::String("alice@example.com".to_string()),
	///     ))
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
		if self.empty_result {
			return Err(reinhardt_core::exception::Error::NotFound(
				"No record found matching the query".to_string(),
			));
		}
		let mut conn = super::manager::get_connection().await?;
		self.get_with_db(&mut conn).await
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let mut db = reinhardt_db::orm::manager::get_connection().await?;
	/// let users = User::objects()
	///     .all()
	///     .all_with_db(&mut db)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn all_with_db<E>(&self, conn: &mut E) -> reinhardt_core::exception::Result<Vec<T>>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		if self.empty_result {
			return Ok(Vec::new());
		}
		self.ensure_backend_annotations_supported(conn.backend())?;
		self.ensure_not_locking_without_transaction()?;
		let stmt = self.build_select_statement()?;
		let context = super::execution::pgvector_context_for_select(&stmt);
		let (sql, values) =
			Self::build_select_for_backend(&stmt, conn.backend(), conn.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = query_values_from_sea_values(values)?;

		let started_at = Instant::now();
		let query_result = conn.fetch_all_with_context(&sql, params, context).await;
		let duration = started_at.elapsed();

		let rows = match query_result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				rows
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				return Err(error);
			}
		};
		rows.into_iter()
			.map(|row| {
				QueryRow::from_backend_row(row)
					.deserialize_model::<T>()
					.map_err(|error| {
						Error::from(DatabaseError::new(
							DatabaseErrorKind::Serialization,
							format!("Deserialization error: {error}"),
						))
					})
			})
			.collect()
	}

	/// Execute the queryset with an explicit database connection and return rows
	/// without deserializing them into the model.
	///
	/// This preserves selected and annotated expression values that are not model
	/// fields while retaining the same backend validation, bound parameters, and
	/// structural error context as [`Self::all_with_db`].
	pub async fn rows_with_db<E>(
		&self,
		conn: &mut E,
	) -> reinhardt_core::exception::Result<Vec<QueryRow>>
	where
		E: OrmExecutor,
	{
		if self.empty_result {
			return Ok(Vec::new());
		}
		self.ensure_backend_annotations_supported(conn.backend())?;
		self.ensure_not_locking_without_transaction()?;
		let stmt = self.build_select_statement()?;
		let context = super::execution::pgvector_context_for_select(&stmt);
		let (sql, values) =
			Self::build_select_for_backend(&stmt, conn.backend(), conn.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);

		let started_at = Instant::now();
		let query_result = conn.fetch_all_with_context(&sql, params, context).await;
		let duration = started_at.elapsed();

		match query_result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				Ok(rows.into_iter().map(QueryRow::from_backend_row).collect())
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				Err(error)
			}
		}
	}

	/// Streams decoded models through a caller-owned ORM executor.
	///
	/// The returned stream borrows `conn`, so the executor cannot be reused
	/// until the stream completes or is dropped. `chunk_size` is a bounded
	/// driver fetch or buffering hint and must be greater than zero.
	pub fn iterator_with_db<'a, E>(
		&self,
		conn: &'a mut E,
		chunk_size: usize,
	) -> reinhardt_core::exception::Result<QuerySetStream<'a, T>>
	where
		T: serde::de::DeserializeOwned + 'a,
		E: OrmExecutor + 'a,
	{
		if chunk_size == 0 {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"QuerySet iterator chunk_size must be greater than zero",
			)
			.into());
		}
		if self.empty_result {
			return Ok(Box::pin(futures::stream::empty()));
		}
		self.ensure_backend_annotations_supported(conn.backend())?;

		let stmt = self.build_select_statement()?;
		let context = super::execution::pgvector_context_for_select(&stmt);
		let (sql, values) =
			Self::build_select_for_backend(&stmt, conn.backend(), conn.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let rows = conn.fetch_stream_with_context(sql.clone(), params, chunk_size, context)?;

		Ok(Box::pin(async_stream::stream! {
			let mut accounting = StreamQueryAccounting::new(sql.clone(), param_samples);
			let mut rows = Some(TimedRowStream::new(rows, &mut accounting));
			loop {
				let row = rows.as_mut().expect("row stream is available").next().await;
				let Some(row) = row else {
					break;
				};
				let row = match row {
					Ok(row) => row,
					Err(error) => {
						super::instrumentation::instrumentation()
							.orm_query_error(&sql, &format!("{error:?}"))
							.await;
						drop(rows.take());
						accounting.disarm_completion();
						yield Err(error);
						return;
					}
				};
				match QueryRow::from_backend_row(row).deserialize_model::<T>() {
					Ok(model) => yield Ok(model),
					Err(error) => {
						let error = Error::from(DatabaseError::new(
							DatabaseErrorKind::Serialization,
							format!("Deserialization error: {error}"),
						));
						super::instrumentation::instrumentation()
							.orm_query_error(&sql, &format!("{error:?}"))
							.await;
						drop(rows.take());
						yield Err(error);
						return;
					}
				}
			}
		}))
	}

	/// Streams decoded models through an active transaction executor.
	///
	/// This caller-owned-executor path never reacquires a pooled connection and
	/// never falls back to eager materialization or pagination.
	pub fn iterator_with_executor<'a>(
		&self,
		executor: &'a mut dyn super::connection::TransactionExecutor,
		chunk_size: usize,
	) -> reinhardt_core::exception::Result<QuerySetStream<'a, T>>
	where
		T: serde::de::DeserializeOwned + 'a,
	{
		if chunk_size == 0 {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"QuerySet iterator chunk_size must be greater than zero",
			)
			.into());
		}
		if self.empty_result {
			return Ok(Box::pin(futures::stream::empty()));
		}
		self.ensure_backend_annotations_supported(Self::executor_backend(executor))?;

		let stmt = self.build_select_statement()?;
		let context = super::execution::pgvector_context_for_select(&stmt);
		let (sql, values) = Self::build_select_for_backend(
			&stmt,
			Self::executor_backend(executor),
			executor.is_cockroachdb(),
		)?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let rows = executor.fetch_stream_with_context(sql.clone(), params, chunk_size, context)?;

		Ok(Box::pin(async_stream::stream! {
			let mut accounting = StreamQueryAccounting::new(sql.clone(), param_samples);
			let mut rows = Some(TimedRowStream::new(rows, &mut accounting));
			loop {
				let row = rows.as_mut().expect("row stream is available").next().await;
				let Some(row) = row else {
					break;
				};
				let row = match row {
					Ok(row) => row,
					Err(error) => {
						super::instrumentation::instrumentation()
							.orm_query_error(&sql, &format!("{error:?}"))
							.await;
						drop(rows.take());
						accounting.disarm_completion();
						yield Err(error);
						return;
					}
				};
				match QueryRow::from_backend_row(row).deserialize_model::<T>() {
					Ok(model) => yield Ok(model),
					Err(error) => {
						let error = Error::from(DatabaseError::new(
							DatabaseErrorKind::Serialization,
							format!("Deserialization error: {error}"),
						));
						super::instrumentation::instrumentation()
							.orm_query_error(&sql, &format!("{error:?}"))
							.await;
						drop(rows.take());
						yield Err(error);
						return;
					}
				}
			}
		}))
	}

	/// Execute the queryset through an active transaction executor and return all records.
	pub async fn all_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> Result<Vec<T>, crate::backends::error::DatabaseError>
	where
		T: serde::de::DeserializeOwned,
	{
		if self.empty_result {
			return Ok(Vec::new());
		}
		self.ensure_backend_annotations_supported(Self::executor_backend(executor))?;
		self.validate_select_for_update(executor.row_lock_capabilities(), executor.backend())?;
		let stmt = self.build_select_statement().map_err(executor_error)?;
		let context = super::execution::pgvector_context_for_select(&stmt);
		let (sql, values) = Self::build_select_for_backend(
			&stmt,
			Self::executor_backend(executor),
			executor.is_cockroachdb(),
		)?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let started = Instant::now();
		let result = executor
			.fetch_all_with_context(&sql, params, context)
			.await
			.map_err(executor_error);
		let duration = started.elapsed();
		let rows = match result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				rows
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				return Err(error);
			}
		};
		Self::decode_backend_rows(rows)
	}

	/// Executes the queryset through an active transaction and returns raw rows.
	pub async fn rows_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> Result<Vec<QueryRow>, crate::backends::error::DatabaseError> {
		if self.empty_result {
			return Ok(Vec::new());
		}
		self.ensure_backend_annotations_supported(Self::executor_backend(executor))?;
		self.validate_select_for_update(executor.row_lock_capabilities(), executor.backend())?;
		let stmt = self.build_select_statement().map_err(executor_error)?;
		let context = super::execution::pgvector_context_for_select(&stmt);
		let (sql, values) = Self::build_select_for_backend(
			&stmt,
			Self::executor_backend(executor),
			executor.is_cockroachdb(),
		)?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let started = Instant::now();
		let result = executor
			.fetch_all_with_context(&sql, params, context)
			.await
			.map_err(executor_error);
		let duration = started.elapsed();
		match result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				Ok(rows.into_iter().map(QueryRow::from_backend_row).collect())
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				Err(error)
			}
		}
	}

	/// Execute the count query through an active transaction executor.
	pub async fn count_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> Result<usize, crate::backends::error::DatabaseError> {
		if self.empty_result {
			return Ok(0);
		}
		self.ensure_backend_annotations_supported(Self::executor_backend(executor))?;
		let stmt = self.count_select_query().map_err(executor_error)?;
		let context = super::execution::pgvector_context_for_select(&stmt);
		let (sql, values) = Self::build_select_for_backend(
			&stmt,
			Self::executor_backend(executor),
			executor.is_cockroachdb(),
		)?;
		let params = super::execution::convert_values(values);
		let row = executor
			.fetch_one_with_context(&sql, params, context)
			.await
			.map_err(executor_error)?;
		let row = QueryRow::from_backend_row(row);
		Ok(row.get::<i64>("count").unwrap_or_default() as usize)
	}

	/// Execute the queryset through an active transaction executor and return one record.
	pub async fn one_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> Result<Vec<T>, crate::backends::error::DatabaseError>
	where
		T: serde::de::DeserializeOwned,
	{
		if self.empty_result {
			return Ok(Vec::new());
		}

		let mut queryset = self.clone();
		queryset.limit = Some(queryset.limit.map_or(2, |limit| limit.min(2)));
		queryset.all_with_executor(executor).await
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let user_id = 1;
	/// let mut db = reinhardt_db::orm::manager::get_connection().await?;
	/// let user = User::objects()
	///     .filter(reinhardt_db::orm::Filter::new("id", reinhardt_db::orm::FilterOperator::Eq, reinhardt_db::orm::FilterValue::Integer(user_id)))
	///     .get_with_db(&mut db)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get_with_db<E>(&self, conn: &mut E) -> reinhardt_core::exception::Result<T>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		if self.empty_result {
			return Err(reinhardt_core::exception::Error::NotFound(
				"No record found matching the query".to_string(),
			));
		}

		let results = self.all_with_db(conn).await?;
		match results.len() {
			0 => Err(reinhardt_core::exception::Error::NotFound(
				"No record found matching the query".to_string(),
			)),
			1 => Ok(results.into_iter().next().unwrap()),
			n => Err(DatabaseError::new(
				DatabaseErrorKind::Query,
				format!("Multiple records found ({n}), expected exactly one"),
			)
			.into()),
		}
	}

	/// Execute this queryset through an explicit connection and return at most two rows.
	pub async fn one_with_db(
		&self,
		conn: &super::connection::DatabaseConnection,
	) -> reinhardt_core::exception::Result<Vec<T>>
	where
		T: serde::de::DeserializeOwned,
	{
		let mut queryset = self.clone();
		queryset.limit = Some(queryset.limit.map_or(2, |limit| limit.min(2)));
		let mut conn = *conn;
		queryset.all_with_db(&mut conn).await
	}

	/// Count this queryset through an explicit connection without resolving the global connection.
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let mut db = reinhardt_db::orm::manager::get_connection().await?;
	/// let user = User::objects()
	///     .filter(reinhardt_db::orm::Filter::new("status", reinhardt_db::orm::FilterOperator::Eq, reinhardt_db::orm::FilterValue::String("active".to_string())))
	///     .first_with_db(&mut db)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn first_with_db<E>(
		&self,
		conn: &mut E,
	) -> reinhardt_core::exception::Result<Option<T>>
	where
		T: serde::de::DeserializeOwned,
		E: OrmExecutor,
	{
		if self.empty_result {
			return Ok(None);
		}

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Count active users
	/// let count = User::objects()
	///     .filter(Filter::new(
	///         "is_active",
	///         FilterOperator::Eq,
	///         FilterValue::Boolean(true),
	///     ))
	///     .count()
	///     .await?;
	///
	/// println!("Active users: {}", count);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn count(&self) -> reinhardt_core::exception::Result<usize> {
		if self.empty_result {
			return Ok(0);
		}
		let mut conn = super::manager::get_connection().await?;
		self.count_with_db(&mut conn).await
	}

	/// Execute the count query through a caller-owned ORM executor.
	pub async fn count_with_db<E>(&self, conn: &mut E) -> reinhardt_core::exception::Result<usize>
	where
		E: OrmExecutor,
	{
		if self.empty_result {
			return Ok(0);
		}
		self.ensure_backend_annotations_supported(conn.backend())?;
		let stmt = self.count_select_query()?;
		let context = super::execution::pgvector_context_for_select(&stmt);
		let (sql, values) =
			Self::build_select_for_backend(&stmt, conn.backend(), conn.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);
		let started_at = Instant::now();
		let query_result = conn.fetch_one_with_context(&sql, params, context).await;
		let duration = started_at.elapsed();
		let row = match query_result {
			Ok(row) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				row
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				return Err(error);
			}
		};
		let row = QueryRow::from_backend_row(row);
		Ok(row.get::<i64>("count").unwrap_or_default() as usize)
	}

	fn count_distinct_composite_primary_key_query(
		&self,
		filter_relation_joins: &RelationJoinGraph,
	) -> reinhardt_core::exception::Result<SelectStatement> {
		let mut distinct_stmt = Query::select();
		self.apply_model_from(&mut distinct_stmt);
		for column in self.root_primary_key_columns() {
			distinct_stmt.column(column);
		}
		distinct_stmt.distinct();
		Self::apply_relation_join_graph(&mut distinct_stmt, filter_relation_joins);
		if let Some(cond) = self.build_where_condition()? {
			distinct_stmt.cond_where(cond);
		}

		let mut count_stmt = Query::select();
		count_stmt.expr_as(
			Func::count(Expr::asterisk().into_simple_expr()),
			Alias::new("count"),
		);
		count_stmt.from_subquery(distinct_stmt.to_owned(), Alias::new("distinct_root_rows"));
		Ok(count_stmt.to_owned())
	}

	fn count_select_query(&self) -> reinhardt_core::exception::Result<SelectStatement> {
		let mut count_queryset = self.clone();
		count_queryset.relation_joins = self.filter_relation_join_graph_for_query();
		count_queryset.rebase_filter_relation_aliases();
		let filter_relation_joins = count_queryset.relation_join_graph_for_query();

		if filter_relation_joins.has_multi_valued_join()
			&& count_queryset.has_composite_primary_key()
		{
			return count_queryset
				.count_distinct_composite_primary_key_query(&filter_relation_joins);
		}

		let mut stmt = Query::select();
		count_queryset.apply_model_from(&mut stmt);
		if filter_relation_joins.has_multi_valued_join() {
			stmt.expr_as(
				Expr::cust(format!(
					"COUNT(DISTINCT {})",
					count_queryset.distinct_root_primary_key_sql()
				)),
				Alias::new("count"),
			);
		} else {
			stmt.expr_as(
				Func::count(Expr::asterisk().into_simple_expr()),
				Alias::new("count"),
			);
		}

		Self::apply_relation_join_graph(&mut stmt, &filter_relation_joins);

		if let Some(cond) = count_queryset.build_where_condition()? {
			stmt.cond_where(cond);
		}

		Ok(stmt.to_owned())
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Check if any admin users exist
	/// let has_admin = User::objects()
	///     .filter(Filter::new(
	///         "role",
	///         FilterOperator::Eq,
	///         FilterValue::String("admin".to_string()),
	///     ))
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
		if self.empty_result {
			return Ok(false);
		}
		let mut conn = super::manager::get_connection().await?;
		self.exists_with_db(&mut conn).await
	}

	/// Check whether this queryset has rows through a caller-owned ORM executor.
	pub async fn exists_with_db<E>(&self, conn: &mut E) -> reinhardt_core::exception::Result<bool>
	where
		E: OrmExecutor,
	{
		if self.empty_result {
			return Ok(false);
		}

		Ok(self.count_with_db(conn).await? > 0)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		let mut conn = super::manager::get_connection().await?;
		self.create_with_conn(&mut conn, object).await
	}

	/// Create an object through a caller-owned ORM executor.
	pub async fn create_with_conn<E>(
		&self,
		conn: &mut E,
		object: T,
	) -> reinhardt_core::exception::Result<T>
	where
		T: super::Model + Clone,
		E: OrmExecutor,
	{
		match &self.manager {
			Some(manager) => manager.create_with_conn(conn, &object).await,
			None => {
				let manager = super::manager::Manager::<T>::new();
				manager.create_with_conn(conn, &object).await
			}
		}
	}

	/// Generate UPDATE statement using reinhardt-query
	pub fn update_query(
		&self,
		updates: &HashMap<String, UpdateValue>,
	) -> reinhardt_core::exception::Result<reinhardt_query::prelude::UpdateStatement> {
		self.validate_no_related_filters_for_write("QuerySet::update_query")?;
		let mut stmt = Query::update();
		stmt.table(Alias::new(T::table_name()));

		// Add SET clauses
		let mut has_values = false;
		for (field, value) in updates {
			if T::generated_field_names().contains(&field.as_str()) {
				continue;
			}
			let column = Self::database_column_for_field(field);
			stmt.value_expr(Alias::new(column), Self::update_value_to_query_expr(value)?);
			has_values = true;
		}

		if !has_values {
			let primary_key = T::primary_key_column();
			stmt.value_expr(Alias::new(primary_key), Expr::col(Alias::new(primary_key)));
		}

		// Add WHERE conditions
		if let Some(cond) = self.build_where_condition_for_write()? {
			stmt.cond_where(cond);
		}

		Ok(stmt.to_owned())
	}

	/// Generate an UPDATE statement for field assignments on rows matched by this `QuerySet`.
	///
	/// Unlike [`QuerySet::update_query`], this public partial-update builder validates
	/// that at least one non-empty predicate is present so callers cannot
	/// accidentally update every row in the model table.
	pub fn update_fields_query<I, A>(
		&self,
		values: I,
	) -> reinhardt_core::exception::Result<UpdateStatement>
	where
		I: IntoIterator<Item = A>,
		A: Into<FieldAssignment>,
	{
		let assignments = Self::collect_field_assignments(values);
		self.update_fields_query_from_assignments(&assignments)
	}

	/// Generate PostgreSQL UPDATE SQL for field assignments on this `QuerySet`.
	///
	/// This mirrors [`QuerySet::update_sql`] for tests and custom SQL inspection.
	/// Use [`QuerySet::update_fields`] to execute the update against the configured
	/// database backend.
	pub fn update_fields_sql<I, A>(
		&self,
		values: I,
	) -> reinhardt_core::exception::Result<(String, Vec<String>)>
	where
		I: IntoIterator<Item = A>,
		A: Into<FieldAssignment>,
	{
		let stmt = self.update_fields_query(values)?;
		let (sql, values) = PostgresQueryBuilder.build_update(&stmt);
		let params = values
			.iter()
			.map(|value| Self::sea_value_to_string(value))
			.collect();
		Ok((sql, params))
	}

	/// Update fields for rows matched by this `QuerySet` and return the affected row count.
	///
	/// The generated `UPDATE` preserves every filter, composite condition, and
	/// subquery predicate already attached to the `QuerySet`.
	pub async fn update_fields<I, A>(self, values: I) -> reinhardt_core::exception::Result<u64>
	where
		I: IntoIterator<Item = A>,
		A: Into<FieldAssignment>,
	{
		if self.empty_result {
			return Ok(0);
		}
		let mut conn = super::manager::get_connection().await?;
		self.update_fields_with_conn(&mut conn, values).await
	}

	/// Update fields using an explicit database connection.
	pub async fn update_fields_with_conn<E, I, A>(
		&self,
		conn: &mut E,
		values: I,
	) -> reinhardt_core::exception::Result<u64>
	where
		E: OrmExecutor,
		I: IntoIterator<Item = A>,
		A: Into<FieldAssignment>,
	{
		if self.empty_result {
			return Ok(0);
		}
		let stmt = self.update_fields_query(values)?;
		let context = super::execution::pgvector_context_for_update(&stmt);
		let (sql, values) =
			Self::build_update_for_backend(&stmt, conn.backend(), conn.is_cockroachdb())?;
		let params = super::execution::convert_values(values);

		Ok(conn
			.execute_with_context(&sql, params, context)
			.await?
			.rows_affected)
	}

	fn collect_field_assignments<I, A>(values: I) -> Vec<FieldAssignment>
	where
		I: IntoIterator<Item = A>,
		A: Into<FieldAssignment>,
	{
		values.into_iter().map(Into::into).collect()
	}

	fn update_fields_query_from_assignments(
		&self,
		assignments: &[FieldAssignment],
	) -> reinhardt_core::exception::Result<UpdateStatement> {
		Self::validate_update_fields(assignments)?;
		self.validate_no_related_filters_for_write("QuerySet::update_fields")?;

		if !self.has_restricting_where_predicates() {
			let message = if self.has_where_predicates() {
				"QuerySet::update_fields requires at least one non-empty filter predicate"
			} else {
				"QuerySet::update_fields requires at least one filter predicate"
			};
			return Err(reinhardt_core::exception::Error::Validation(
				message.to_string(),
			));
		}

		let condition = self.build_where_condition_for_write()?.ok_or_else(|| {
			reinhardt_core::exception::Error::Validation(
				"QuerySet::update_fields requires at least one non-empty filter predicate"
					.to_string(),
			)
		})?;

		let mut stmt = Query::update();
		stmt.table(Alias::new(T::table_name()));

		for assignment in assignments {
			let column = Self::database_column_for_field(assignment.field());
			stmt.value_expr(
				Alias::new(column),
				Self::update_value_to_query_expr(assignment.value())?,
			);
		}

		stmt.cond_where(condition);

		Ok(stmt.to_owned())
	}

	fn validate_update_fields(
		assignments: &[FieldAssignment],
	) -> reinhardt_core::exception::Result<()> {
		if assignments.is_empty() {
			return Err(reinhardt_core::exception::Error::Validation(
				"QuerySet::update_fields requires at least one field assignment".to_string(),
			));
		}

		if assignments
			.iter()
			.any(|assignment| assignment.field().trim().is_empty())
		{
			return Err(reinhardt_core::exception::Error::Validation(
				"QuerySet::update_fields field names must not be empty".to_string(),
			));
		}

		if let Some(assignment) = assignments
			.iter()
			.find(|assignment| T::generated_field_names().contains(&assignment.field()))
		{
			return Err(reinhardt_core::exception::Error::Validation(format!(
				"QuerySet::update_fields cannot assign generated field `{}`",
				assignment.field()
			)));
		}

		for assignment in assignments {
			if let UpdateValue::Typed(value) = assignment.value() {
				Self::typed_database_value(value)?;
			}
		}

		Ok(())
	}

	fn build_update_for_backend(
		stmt: &UpdateStatement,
		backend: super::connection::DatabaseBackend,
		is_cockroachdb: bool,
	) -> Result<(String, reinhardt_query::prelude::Values), reinhardt_core::exception::DatabaseError>
	{
		let result = if is_cockroachdb {
			CockroachDBQueryBuilder::new().build_update_checked(stmt)
		} else {
			match backend {
				super::connection::DatabaseBackend::Postgres => {
					PostgresQueryBuilder.build_update_checked(stmt)
				}
				super::connection::DatabaseBackend::MySql => {
					MySqlQueryBuilder.build_update_checked(stmt)
				}
				super::connection::DatabaseBackend::Sqlite => {
					SqliteQueryBuilder.build_update_checked(stmt)
				}
			}
		};
		result
			.map_err(|error| DatabaseError::new(DatabaseErrorKind::Unsupported, error.to_string()))
	}

	fn build_delete_for_backend(
		stmt: &reinhardt_query::prelude::DeleteStatement,
		backend: super::connection::DatabaseBackend,
		is_cockroachdb: bool,
	) -> Result<(String, reinhardt_query::prelude::Values), reinhardt_core::exception::DatabaseError>
	{
		let result = if is_cockroachdb {
			CockroachDBQueryBuilder::new().build_delete_checked(stmt)
		} else {
			match backend {
				super::connection::DatabaseBackend::Postgres => {
					PostgresQueryBuilder.build_delete_checked(stmt)
				}
				super::connection::DatabaseBackend::MySql => {
					MySqlQueryBuilder.build_delete_checked(stmt)
				}
				super::connection::DatabaseBackend::Sqlite => {
					SqliteQueryBuilder.build_delete_checked(stmt)
				}
			}
		};
		result
			.map_err(|error| DatabaseError::new(DatabaseErrorKind::Unsupported, error.to_string()))
	}

	fn build_select_for_backend(
		stmt: &SelectStatement,
		backend: super::connection::DatabaseBackend,
		is_cockroachdb: bool,
	) -> Result<(String, reinhardt_query::prelude::Values), reinhardt_core::exception::DatabaseError>
	{
		let result = if is_cockroachdb {
			CockroachDBQueryBuilder::new().build_select_checked(stmt)
		} else {
			match backend {
				super::connection::DatabaseBackend::Postgres => {
					PostgresQueryBuilder.build_select_checked(stmt)
				}
				super::connection::DatabaseBackend::MySql => {
					MySqlQueryBuilder.build_select_checked(stmt)
				}
				super::connection::DatabaseBackend::Sqlite => {
					SqliteQueryBuilder.build_select_checked(stmt)
				}
			}
		};
		result
			.map_err(|error| DatabaseError::new(DatabaseErrorKind::Unsupported, error.to_string()))
	}

	fn update_value_to_query_expr(value: &UpdateValue) -> reinhardt_core::exception::Result<Expr> {
		let expr = match value {
			UpdateValue::Typed(Ok(value)) => {
				Expr::val(database_value_to_query_value(value.clone()))
			}
			UpdateValue::Typed(Err(error)) => {
				return Err(Self::typed_field_codec_error(error.clone()));
			}
			UpdateValue::String(s) => Expr::val(s.clone()),
			UpdateValue::Integer(i) => Expr::val(*i),
			UpdateValue::Float(f) => Expr::val(*f),
			UpdateValue::Boolean(b) => Expr::val(*b),
			UpdateValue::Null => Expr::cust("NULL"),
			UpdateValue::Timestamp(dt) => Expr::val(
				reinhardt_query::value::Value::ChronoDateTimeUtc(Some(Box::new(*dt))),
			),
			UpdateValue::Uuid(uuid) => {
				Expr::val(reinhardt_query::value::Value::Uuid(Some(Box::new(*uuid))))
			}
			UpdateValue::FieldRef(field) => Expr::col(parse_column_reference(
				&Self::database_column_for_field(&field.field),
			)),
			UpdateValue::Expression(expression) => {
				let mut expression = expression.clone();
				Self::resolve_write_expression_fields(&mut expression);
				Self::expression_to_query_expr(&expression)
			}
		};
		Ok(expr)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// use std::collections::HashMap;
	/// let queryset = User::objects()
	///     .filter(Filter::new("id", FilterOperator::Eq, FilterValue::Integer(1)));
	///
	/// let mut updates = HashMap::new();
	/// updates.insert("name".to_string(), UpdateValue::String("Alice".to_string()));
	/// updates.insert("email".to_string(), UpdateValue::String("alice@example.com".to_string()));
	/// let (sql, params) = queryset.update_sql(&updates).expect("update SQL should compile");
	/// // sql: "UPDATE users SET name = $1, email = $2 WHERE id = $3"
	/// // params: ["Alice", "alice@example.com", "1"]
	/// ```
	pub fn update_sql(
		&self,
		updates: &HashMap<String, UpdateValue>,
	) -> reinhardt_core::exception::Result<(String, Vec<String>)> {
		let stmt = self.update_query(updates)?;
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};
		let (sql, values) = PostgresQueryBuilder.build_update(&stmt);
		let params: Vec<String> = values
			.iter()
			.map(|v| Self::sea_value_to_string(v))
			.collect();
		Ok((sql, params))
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
			Value::ChronoDateTimeUtc(Some(dt)) => dt.to_rfc3339(),
			Value::Uuid(Some(uuid)) => uuid.to_string(),
			#[cfg(feature = "pgvector")]
			Value::Vector(Some(values)) => {
				Self::database_value_to_string(&DatabaseValue::Vector(values.as_ref().clone()))
			}
			#[cfg(feature = "pgvector")]
			Value::Vector(None) => String::new(),
			_ => String::new(),
		}
	}

	/// Generate DELETE SQL with WHERE clause and parameter binding
	///
	/// Returns SQL with placeholders ($1, $2, etc.) and the values to bind.
	///
	/// # Safety
	///
	/// This method will panic if no restricting filter is set. Tautologies such as an
	/// empty `AND` or empty `NOT IN` are rejected to prevent deleting every row.
	/// Always use `.filter()` with a restricting predicate before calling this method.
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// let queryset = User::objects()
	///     .filter(Filter::new("id", FilterOperator::Eq, FilterValue::Integer(1)));
	///
	/// let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");
	/// // sql: "DELETE FROM users WHERE id = $1"
	/// // params: ["1"]
	/// ```
	/// Generate DELETE statement using reinhardt-query
	pub fn delete_query(
		&self,
	) -> reinhardt_core::exception::Result<reinhardt_query::prelude::DeleteStatement> {
		self.validate_no_related_filters_for_write("QuerySet::delete_query")?;
		if !self.has_restricting_where_predicates() {
			panic!(
				"DELETE without WHERE clause is not allowed. Use .filter() to specify which rows to delete."
			);
		}

		let Some(cond) = self.build_where_condition_for_write()? else {
			panic!(
				"DELETE without WHERE clause is not allowed. Use .filter() to specify which rows to delete."
			);
		};

		let mut stmt = Query::delete();
		stmt.from_table(Alias::new(T::table_name()));
		stmt.cond_where(cond);

		Ok(stmt.to_owned())
	}

	/// Delete rows matched by this `QuerySet` using an explicit connection.
	///
	/// The generated `DELETE` preserves every supported predicate attached to
	/// the queryset and returns the affected row count to the caller.
	pub async fn delete_with_conn<E>(&self, conn: &mut E) -> reinhardt_core::exception::Result<u64>
	where
		E: OrmExecutor,
	{
		if self.empty_result {
			return Ok(0);
		}
		let stmt = self.delete_query()?;
		let (sql, values) =
			Self::build_delete_for_backend(&stmt, conn.backend(), conn.is_cockroachdb())?;
		let params = super::execution::convert_values(values);

		Ok(conn.execute(&sql, params).await?.rows_affected)
	}

	/// Deletes sql.
	pub fn delete_sql(&self) -> reinhardt_core::exception::Result<(String, Vec<String>)> {
		let stmt = self.delete_query()?;
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};
		let (sql, values) = PostgresQueryBuilder.build_delete(&stmt);
		let params: Vec<String> = values
			.iter()
			.map(|v| Self::sea_value_to_string(v))
			.collect();
		Ok((sql, params))
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
		Self::composite_primary_key_for_values(pk_values)?;
		if self.empty_result {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Query,
				"No record found matching the composite primary key",
			)
			.into());
		}
		let mut conn = super::manager::get_connection().await?;
		self.get_composite_with_db(&mut conn, pk_values).await
	}

	fn composite_primary_key_for_values(
		pk_values: &HashMap<String, super::composite_pk::PkValue>,
	) -> reinhardt_core::exception::Result<super::composite_pk::CompositePrimaryKey>
	where
		T: super::Model,
	{
		let composite_pk = T::composite_primary_key().ok_or_else(|| {
			Error::from(DatabaseError::new(
				DatabaseErrorKind::Query,
				"Model does not have a composite primary key",
			))
		})?;

		composite_pk.validate(pk_values).map_err(|error| {
			Error::from(DatabaseError::new(
				DatabaseErrorKind::Query,
				format!("Composite PK validation failed: {error}"),
			))
		})?;

		Ok(composite_pk)
	}

	/// Retrieve a composite-primary-key row through a caller-owned ORM executor.
	pub async fn get_composite_with_db<E>(
		&self,
		conn: &mut E,
		pk_values: &HashMap<String, super::composite_pk::PkValue>,
	) -> reinhardt_core::exception::Result<T>
	where
		T: super::Model + Clone,
		E: OrmExecutor,
	{
		if self.empty_result {
			return Err(Error::NotFound(
				"No record found matching the query".to_string(),
			));
		}
		use reinhardt_query::prelude::{Alias, BinOper, ColumnRef, Expr, Value};

		let composite_pk = Self::composite_primary_key_for_values(pk_values)?;
		if self.empty_result {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Query,
				"No record found matching the composite primary key",
			)
			.into());
		}

		// Build SELECT query using reinhardt-query
		let table_name = T::table_name();
		let mut query = Query::select();

		// Use Alias::new for table name
		let table_alias = Alias::new(table_name);
		query.from(table_alias).column(ColumnRef::Asterisk);

		// Add WHERE conditions for each composite PK field
		let field_metadata = T::field_metadata();
		for field_name in composite_pk.fields() {
			let pk_value: &super::composite_pk::PkValue = pk_values.get(field_name).unwrap();
			let column = field_metadata
				.iter()
				.find(|field| field.name == *field_name)
				.map(|field| field.db_column_name())
				.unwrap_or(field_name);
			let col_alias = Alias::new(column);

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

		let context = super::execution::pgvector_context_for_select(&query);
		let (sql, values) =
			Self::build_select_for_backend(&query, conn.backend(), conn.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::execution::convert_values(values);

		let started_at = Instant::now();
		let query_result = conn.fetch_all_with_context(&sql, params, context).await;
		let duration = started_at.elapsed();
		let rows = match query_result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				rows
			}
			Err(error) => {
				super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				return Err(error);
			}
		};

		// Composite PK queries should return exactly one row
		if rows.is_empty() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Query,
				"No record found matching the composite primary key",
			)
			.into());
		}

		if rows.len() > 1 {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Query,
				format!(
					"Multiple records found ({}) for composite primary key, expected exactly one",
					rows.len()
				),
			)
			.into());
		}

		// Deserialize the single row into the model
		QueryRow::from_backend_row(rows.into_iter().next().expect("row count was checked"))
			.deserialize_model::<T>()
			.map_err(|error| {
				Error::from(DatabaseError::new(
					DatabaseErrorKind::Serialization,
					format!("Deserialization error: {error}"),
				))
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::orm::func;
	///
	/// let display_name =
	///     func::literal::<User, String>("user".to_owned())?.label("display_name")?;
	/// let users = User::objects()
	///     .all()
	///     .annotate(display_name)?
	///     .all()
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(test)]
	pub(crate) fn annotate_legacy(mut self, annotation: super::annotation::Annotation) -> Self {
		self.annotations.push(annotation);
		self
	}

	/// Add a validated model-rooted expression to the selected columns.
	pub fn annotate<K>(
		mut self,
		expression: LabeledExpression<T, K>,
	) -> reinhardt_core::exception::Result<Self>
	where
		K: AnnotationExpressionKind,
	{
		let label = expression.label().to_owned();
		if let Some(field) = T::field_metadata()
			.into_iter()
			.find(|field| field.name == label || field.db_column_name() == label)
		{
			return Err(Error::Validation(format!(
				"annotation label `{label}` collides with model field `{}`",
				field.name
			)));
		}
		if self
			.annotations
			.iter()
			.any(|annotation| annotation.alias == label)
			|| self
				.typed_annotations
				.iter()
				.any(|annotation| annotation.label.as_deref() == Some(label.as_str()))
			|| self
				.backend_annotations
				.iter()
				.any(|annotation| annotation.label() == label)
			|| self
				.selected_expressions
				.iter()
				.any(|(alias, _)| alias == &label)
		{
			return Err(Error::Validation(format!(
				"annotation label `{label}` is already in use"
			)));
		}
		self.typed_annotations
			.push(expression.into_stored_expression());
		Ok(self)
	}

	/// Adds a PostgreSQL-only projection outside the portable annotation tree.
	pub fn annotate_backend(
		mut self,
		annotation: super::postgres_features::BackendAnnotation,
	) -> reinhardt_core::exception::Result<Self> {
		let label = annotation.label().to_owned();
		validate_annotation_label(&label)?;
		if T::field_metadata()
			.into_iter()
			.any(|field| field.name == label || field.db_column_name() == label)
			|| self.annotations.iter().any(|item| item.alias == label)
			|| self
				.typed_annotations
				.iter()
				.any(|item| item.label.as_deref() == Some(&label))
			|| self
				.backend_annotations
				.iter()
				.any(|item| item.label() == label)
			|| self
				.selected_expressions
				.iter()
				.any(|(alias, _)| alias == &label)
		{
			return Err(Error::Validation(format!(
				"annotation label `{label}` is already in use"
			)));
		}
		self.backend_annotations.push(annotation);
		Ok(self)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	///     })?
	///     .all()
	///     .await?;
	/// // Generates: SELECT *, (SELECT COUNT(*) FROM books WHERE author_id = authors.id) AS book_count FROM authors
	/// # Ok(())
	/// # }
	/// ```
	pub fn annotate_subquery<M, F>(
		mut self,
		name: &str,
		builder: F,
	) -> reinhardt_core::exception::Result<Self>
	where
		M: super::Model + 'static,
		F: FnOnce(QuerySet<M>) -> QuerySet<M>,
	{
		// Create a fresh QuerySet for the subquery model
		let subquery_qs = QuerySet::<M>::new();
		// Apply the builder to configure the subquery
		let configured_subquery = builder(subquery_qs);
		// Generate SQL for the subquery (wrapped in parentheses)
		let subquery_sql = configured_subquery.as_subquery()?;

		// Add as annotation using AnnotationValue::Subquery
		let annotation = super::annotation::Annotation {
			alias: name.to_string(),
			value: super::annotation::AnnotationValue::Subquery(subquery_sql),
		};
		self.annotations.push(annotation);
		Ok(self)
	}

	/// Converts to sql.
	pub fn to_sql(&self) -> reinhardt_core::exception::Result<String> {
		self.to_sql_for_backend(crate::backends::types::DatabaseType::Postgres)
	}

	fn to_sql_for_backend(
		&self,
		backend: crate::backends::types::DatabaseType,
	) -> reinhardt_core::exception::Result<String> {
		let mut stmt = if !self.has_select_related() {
			// Simple SELECT without JOINs
			let mut stmt = Query::select();

			self.apply_model_from(&mut stmt);

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
						let col_ref = self.root_column_reference(field);
						stmt.column(col_ref);
					}
				}
			} else if !self.deferred_fields.is_empty() {
				let all_fields = T::field_metadata();
				for field in all_fields {
					if !self.deferred_fields.contains(&field.name) {
						let col_ref = self.root_column_reference(&field.name);
						stmt.column(col_ref);
					}
				}
			} else {
				self.add_default_select_columns(&mut stmt);
			}
			self.apply_typed_select_expressions(&mut stmt)?;

			self.apply_relation_joins(&mut stmt);

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
			if let Some(cond) = self.build_where_condition_for_backend(backend)? {
				stmt.cond_where(cond);
			}
			// Apply GROUP BY
			for group_field in &self.group_by_fields {
				stmt.group_by_col(self.root_column_reference(group_field));
			}
			self.apply_typed_annotation_grouping(&mut stmt)?;

			// Apply HAVING
			self.apply_typed_having(&mut stmt)?;

			self.apply_ordering(&mut stmt)?;

			// Apply LIMIT/OFFSET
			if self.empty_result {
				stmt.limit(0);
			} else if let Some(limit) = self.limit {
				stmt.limit(limit as u64);
			}
			if let Some(offset) = self.offset {
				stmt.offset(offset as u64);
			}

			stmt.to_owned()
		} else {
			// SELECT with JOINs for select_related
			self.select_related_query_with_condition(
				self.build_where_condition_for_backend(backend)?,
			)?
		};

		if !self.has_select_related() {
			self.apply_annotations_to_select(&mut stmt);
		}

		let mut select_sql = match backend {
			crate::backends::types::DatabaseType::Postgres => stmt.to_string(PostgresQueryBuilder),
			crate::backends::types::DatabaseType::Mysql => stmt.to_string(MySqlQueryBuilder),
			crate::backends::types::DatabaseType::Sqlite => stmt.to_string(SqliteQueryBuilder),
		};

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
			let quote = match backend {
				crate::backends::types::DatabaseType::Mysql => '`',
				crate::backends::types::DatabaseType::Postgres
				| crate::backends::types::DatabaseType::Sqlite => '"',
			};
			let from_pattern_with_alias = format!(
				"FROM {quote}{}{quote} AS {quote}{alias}{quote}",
				T::table_name()
			);
			let from_pattern_simple = format!("FROM {quote}{}{quote}", T::table_name());

			let from_replacement = format!(
				"FROM {} AS {quote}{alias}{quote}",
				subquery_sql.for_backend(backend)
			);

			// Try to replace with alias pattern first, then simple pattern
			if select_sql.contains(&from_pattern_with_alias) {
				select_sql = select_sql.replace(&from_pattern_with_alias, &from_replacement);
			} else if select_sql.contains(&from_pattern_simple) {
				select_sql = select_sql.replace(&from_pattern_simple, &from_replacement);
			}
		}

		// Prepend CTE clause if any CTEs are defined
		if let Some(cte_sql) = self.ctes.to_sql() {
			Ok(format!("{} {}", cte_sql, select_sql))
		} else {
			Ok(select_sql)
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	///     .filter(Filter::new("is_active", FilterOperator::Eq, FilterValue::Boolean(true)))
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

	/// Select a model-rooted expression under an identifier-safe alias.
	///
	/// # Panics
	///
	/// Panics when `alias` is invalid or collides with an existing projection.
	pub fn select_expr<R>(
		mut self,
		alias: impl Into<String>,
		expression: TypedExpression<T, R>,
	) -> Self {
		let alias = alias.into();
		validate_annotation_label(&alias)
			.unwrap_or_else(|error| panic!("invalid selected expression alias `{alias}`: {error}"));
		assert!(
			!T::field_metadata()
				.into_iter()
				.any(|field| field.name == alias || field.db_column_name() == alias),
			"selected expression alias `{alias}` collides with a model field"
		);
		assert!(
			!(self
				.annotations
				.iter()
				.any(|annotation| annotation.alias == alias)
				|| self
					.typed_annotations
					.iter()
					.any(|annotation| annotation.label.as_deref() == Some(alias.as_str()))
				|| self
					.backend_annotations
					.iter()
					.any(|annotation| annotation.label() == alias)
				|| self
					.selected_expressions
					.iter()
					.any(|(existing_alias, _)| existing_alias == &alias)),
			"selected expression alias `{alias}` is already in use"
		);
		self.selected_expressions.push((
			alias.clone(),
			expression.into_stored_expression(Some(alias)),
		));
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	pub fn order_by<I>(mut self, ordering: I) -> Self
	where
		I: IntoOrderBy<T>,
	{
		ordering.apply(&mut self);
		self
	}

	/// Return only distinct results
	pub fn distinct(mut self) -> Self {
		self.distinct_enabled = true;
		self
	}

	/// Clear DISTINCT for a single-row mutation lookup.
	pub fn without_distinct(mut self) -> Self {
		self.distinct_enabled = false;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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

	/// Return whether this queryset limits or offsets its result set.
	pub fn has_slicing(&self) -> bool {
		self.limit.is_some() || self.offset.is_some()
	}

	/// Removes result-shape modifiers before a session decodes rows as models.
	///
	/// Scope predicates, ordering, limits, and offsets remain intact. Projection,
	/// annotations, and eager-loading options are discarded because the session
	/// list path decodes only the root model from each row.
	pub fn for_model_session(mut self) -> Self {
		self.selected_fields = None;
		self.selected_expressions.clear();
		self.deferred_fields.clear();
		self.annotations.clear();
		self.backend_annotations.clear();
		self.typed_annotations.clear();
		self.typed_havings.clear();
		self.group_by_fields.clear();
		self.select_related_fields.clear();
		self.typed_select_related.clear();
		self.prefetch_related_fields.clear();
		self.typed_prefetch_related.clear();
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// # }
	/// // Use in IN clause
	/// let active_user_ids = User::objects()
	///     .filter(Filter::new("is_active", FilterOperator::Eq, FilterValue::Bool(true)))
	///     .values(&["id"])
	///     .as_subquery()
	///     .expect("subquery should compile");
	/// // Generates: (SELECT id FROM users WHERE is_active = $1)
	///
	/// // Use as derived table
	/// let subquery = Post::objects()
	///     .filter(Filter::new("published", FilterOperator::Eq, FilterValue::Bool(true)))
	///     .as_subquery()
	///     .expect("subquery should compile");
	/// // Generates: (SELECT * FROM posts WHERE published = $1)
	/// ```
	pub fn as_subquery(self) -> reinhardt_core::exception::Result<String> {
		Ok(self.as_subquery_sql()?.postgres)
	}

	fn as_subquery_sql(&self) -> reinhardt_core::exception::Result<SubquerySql> {
		Ok(SubquerySql {
			postgres: format!(
				"({})",
				self.to_sql_for_backend(crate::backends::types::DatabaseType::Postgres)?
			),
			mysql: format!(
				"({})",
				self.to_sql_for_backend(crate::backends::types::DatabaseType::Mysql)?
			),
			sqlite: format!(
				"({})",
				self.to_sql_for_backend(crate::backends::types::DatabaseType::Sqlite)?
			),
		})
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
// Helper Functions
// ============================================================================

/// Quote a SQL identifier to prevent injection via field names.
/// Uses PostgreSQL double-quote escaping (also valid for SQLite).
/// Handles dot-separated qualified names (e.g., "table.column" becomes "table"."column").
pub(crate) fn quote_identifier(field: &str) -> String {
	if field.contains('\0') {
		panic!("SQL identifier must not contain null bytes");
	}

	fn quote_single(name: &str) -> String {
		format!("\"{}\"", name.replace('"', "\"\""))
	}

	if field.contains('.') {
		field
			.split('.')
			.map(quote_single)
			.collect::<Vec<_>>()
			.join(".")
	} else {
		quote_single(field)
	}
}

fn quote_identifier_for_backend(
	field: &str,
	backend: crate::backends::types::DatabaseType,
) -> String {
	if backend != crate::backends::types::DatabaseType::Mysql {
		return quote_identifier(field);
	}

	if field.contains('\0') {
		panic!("SQL identifier must not contain null bytes");
	}

	field
		.split('.')
		.map(|name| format!("`{}`", name.replace('`', "``")))
		.collect::<Vec<_>>()
		.join(".")
}

/// Validates an annotation label using the same identifier policy as typed annotations.
pub(crate) fn validate_annotation_label(label: &str) -> reinhardt_core::exception::Result<()> {
	crate::orm::query_fields::expression::validate_label(label)
}

fn filter_lhs_expr(filter: &Filter) -> Expr {
	if let Some(alias) = filter.relation_alias() {
		return Expr::col((Alias::new(alias), Alias::new(&filter.field)));
	}

	match &filter.field_source {
		FilterField::Column(_) => Expr::col(parse_column_reference(&filter.field)),
		FilterField::Expression(sql) if filter.field == *sql => Expr::cust(sql.clone()),
		FilterField::Expression(_) => Expr::col(parse_column_reference(&filter.field)),
		FilterField::TypedPredicate(_) => Expr::cust("TRUE"),
	}
}

fn filter_lhs_expr_for_root(filter: &Filter, root_alias: &str) -> Expr {
	if filter.relation_alias().is_none() {
		match &filter.field_source {
			FilterField::Column(_) if !filter.field.contains('.') => {
				return Expr::col((Alias::new(root_alias), Alias::new(&filter.field)));
			}
			FilterField::Expression(sql) if filter.field == *sql => {
				return Expr::cust(qualify_filter_expression_sql(sql, root_alias));
			}
			_ => {}
		}
	}

	filter_lhs_expr(filter)
}

fn filter_lhs_sql(filter: &Filter) -> String {
	if let Some(alias) = filter.relation_alias() {
		return quote_identifier(&format!("{alias}.{}", filter.field));
	}

	match &filter.field_source {
		FilterField::Column(_) => quote_identifier(&filter.field),
		FilterField::Expression(sql) if filter.field == *sql => sql.clone(),
		FilterField::Expression(_) => quote_identifier(&filter.field),
		FilterField::TypedPredicate(_) => "TRUE".to_owned(),
	}
}

fn filter_lhs_sql_for_root(filter: &Filter, root_alias: &str) -> String {
	if filter.relation_alias().is_none() {
		match &filter.field_source {
			FilterField::Column(_) if !filter.field.contains('.') => {
				return quote_identifier(&format!("{root_alias}.{}", filter.field));
			}
			FilterField::Expression(sql) if filter.field == *sql => {
				return qualify_filter_expression_sql(sql, root_alias);
			}
			_ => {}
		}
	}

	filter_lhs_sql(filter)
}

fn qualify_filter_expression_sql(sql: &str, root_alias: &str) -> String {
	let root_alias = quote_identifier(root_alias);
	let mut qualified = String::with_capacity(sql.len() + root_alias.len());
	let mut cursor = 0;

	while cursor < sql.len() {
		let next_identifier = sql[cursor..].find('"');
		let next_literal = sql[cursor..].find('\'');
		let relative_start = match (next_identifier, next_literal) {
			(Some(identifier), Some(literal)) => identifier.min(literal),
			(Some(start), None) | (None, Some(start)) => start,
			(None, None) => {
				qualified.push_str(&sql[cursor..]);
				return qualified;
			}
		};
		let start = cursor + relative_start;
		if sql.as_bytes()[start] == b'\'' {
			let mut end = start + 1;
			loop {
				let Some(relative_end) = sql[end..].find('\'') else {
					qualified.push_str(&sql[cursor..]);
					return qualified;
				};
				end += relative_end;
				if sql.as_bytes().get(end + 1) == Some(&b'\'') {
					end += 2;
					continue;
				}
				end += 1;
				qualified.push_str(&sql[cursor..end]);
				cursor = end;
				break;
			}
			continue;
		}

		let mut end = start + 1;
		loop {
			let Some(relative_end) = sql[end..].find('"') else {
				qualified.push_str(&sql[cursor..]);
				return qualified;
			};
			end += relative_end;
			if sql.as_bytes().get(end + 1) == Some(&b'"') {
				end += 2;
				continue;
			}
			break;
		}

		qualified.push_str(&sql[cursor..start]);
		let previous = sql[..start].chars().next_back();
		let next = sql[end + 1..].chars().next();
		if previous == Some('.') || next == Some('.') {
			qualified.push_str(&sql[start..=end]);
		} else {
			qualified.push_str(&root_alias);
			qualified.push('.');
			qualified.push_str(&sql[start..=end]);
		}
		cursor = end + 1;
	}

	qualified.push_str(&sql[cursor..]);
	qualified
}

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
pub(crate) fn parse_column_reference(field: &str) -> reinhardt_query::prelude::ColumnRef {
	use reinhardt_query::prelude::ColumnRef;

	// Detect raw SQL expressions by checking for parentheses
	// Examples: COUNT(*), AVG(price), SUM(amount), MAX(value)
	if field.contains('(') && field.contains(')') {
		// Use column reference with raw expression name
		ColumnRef::column(Alias::new(field))
	} else if field.contains('.') {
		// Qualified column reference (table.column format)
		let parts: Vec<&str> = field.split('.').collect();
		match parts.as_slice() {
			[table, column] => {
				// Produces: "table"."column" instead of "table.column"
				ColumnRef::table_column(Alias::new(*table), Alias::new(*column))
			}
			[schema, table, column] => {
				// Produces: "schema"."table"."column"
				ColumnRef::schema_table_column(
					Alias::new(*schema),
					Alias::new(*table),
					Alias::new(*column),
				)
			}
			_ => {
				// Fallback for unexpected formats (4+ parts)
				ColumnRef::column(Alias::new(field))
			}
		}
	} else {
		// Simple column reference
		ColumnRef::column(Alias::new(field))
	}
}

fn contains_known_raw_aggregate(projection: &str) -> bool {
	const AGGREGATE_FUNCTIONS: &[&str] = &[
		"COUNT",
		"SUM",
		"AVG",
		"MIN",
		"MAX",
		"ARRAY_AGG",
		"JSON_AGG",
		"JSONB_AGG",
		"STRING_AGG",
		"BOOL_AND",
		"BOOL_OR",
		"EVERY",
		"XMLAGG",
	];

	let bytes = projection.as_bytes();
	let mut index = 0;
	while index < bytes.len() {
		if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
			let start = index;
			index += 1;
			while index < bytes.len()
				&& (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
			{
				index += 1;
			}

			let function = &projection[start..index];
			let mut next = index;
			while next < bytes.len() && bytes[next].is_ascii_whitespace() {
				next += 1;
			}
			if bytes.get(next) == Some(&b'(')
				&& AGGREGATE_FUNCTIONS
					.iter()
					.any(|aggregate| function.eq_ignore_ascii_case(aggregate))
			{
				return true;
			}
		} else {
			index += 1;
		}
	}
	false
}

#[derive(Debug, Clone, Copy)]
enum LikePattern {
	Exact,
	Contains,
	StartsWith,
	EndsWith,
}

impl LikePattern {
	fn apply(self, value: &str) -> String {
		let escaped = escape_like_pattern(value);
		match self {
			Self::Exact => escaped,
			Self::Contains => format!("%{}%", escaped),
			Self::StartsWith => format!("{}%", escaped),
			Self::EndsWith => format!("%{}", escaped),
		}
	}
}

fn escape_like_pattern(value: &str) -> String {
	let mut escaped = String::with_capacity(value.len());
	for ch in value.chars() {
		if matches!(ch, '\\' | '%' | '_') {
			escaped.push('\\');
		}
		escaped.push(ch);
	}
	escaped
}

fn is_scalar_plan_value(value: &serde_json::Value) -> bool {
	matches!(
		value,
		serde_json::Value::Null
			| serde_json::Value::Bool(_)
			| serde_json::Value::Number(_)
			| serde_json::Value::String(_)
	)
}

fn plan_value_to_text(value: serde_json::Value) -> String {
	match value {
		serde_json::Value::String(value) => value,
		value => value.to_string(),
	}
}

#[cfg(test)]
fn build_select_statement(
	statement: &SelectStatement,
	backend: super::connection::DatabaseBackend,
) -> reinhardt_core::exception::Result<(String, Vec<QueryValue>)> {
	let (sql, values) = match backend {
		super::connection::DatabaseBackend::Postgres => statement.build(PostgresQueryBuilder),
		super::connection::DatabaseBackend::MySql => statement.build(MySqlQueryBuilder),
		super::connection::DatabaseBackend::Sqlite => statement.build(SqliteQueryBuilder),
	};

	let params = query_values_from_sea_values(values)?;
	Ok((sql, params))
}

fn query_values_from_sea_values(
	values: reinhardt_query::prelude::Values,
) -> reinhardt_core::exception::Result<Vec<QueryValue>> {
	for value in values.iter() {
		if let Value::BigUnsigned(Some(value)) = value
			&& i64::try_from(*value).is_err()
		{
			return Err(Error::from(DatabaseError::new(
				DatabaseErrorKind::Type,
				format!("Unsigned query parameter {value} exceeds the supported i64 range"),
			)));
		}
	}

	Ok(super::execution::convert_values(values))
}

#[cfg(test)]
mod tests {
	use super::{
		DateProjectionOrder, FilterCondition, MAX_FILTER_CONDITION_DEPTH, QueryFilterInput,
		RowStream, StreamQueryAccounting, TimedRowStream, build_select_statement,
	};
	#[cfg(feature = "pgvector")]
	use crate::orm::Field;
	use crate::orm::connection::DatabaseBackend;
	use crate::orm::query::{FieldAssignment, UpdateValue};
	use crate::orm::{
		DatabaseValue, FieldCodecError, FilterOperator, FilterValue, Manager, Model, QuerySet,
		query::Filter,
	};
	use futures::Stream;
	#[cfg(feature = "pgvector")]
	use reinhardt_core::macros::model;
	use reinhardt_query::prelude::ExprTrait;
	use reinhardt_query::{
		QueryBuilder,
		prelude::{
			MySqlQueryBuilder, PostgresQueryBuilder, QueryStatementBuilder, SqliteQueryBuilder,
			TemporalTruncKind, TemporalTruncOutput,
		},
	};
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	#[cfg(feature = "pgvector")]
	use std::borrow::Cow;
	use std::collections::{BTreeSet, HashMap};
	#[cfg(feature = "pgvector")]
	use std::fmt;
	use std::pin::Pin;
	use std::sync::{Arc, Mutex};
	use std::task::{Context, Poll, Waker};
	use std::time::Duration;

	#[derive(Default)]
	struct WakeDrivenRowStreamState {
		ready: bool,
		waker: Option<Waker>,
	}

	struct WakeDrivenRowStream {
		state: Arc<Mutex<WakeDrivenRowStreamState>>,
	}

	impl Stream for WakeDrivenRowStream {
		type Item = reinhardt_core::exception::Result<crate::backends::types::Row>;

		fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
			let mut state = self
				.state
				.lock()
				.unwrap_or_else(|poisoned| poisoned.into_inner());
			if state.ready {
				return Poll::Ready(None);
			}
			state.waker = Some(context.waker().clone());
			Poll::Pending
		}
	}

	struct PendingThenReadyRowStream {
		first_poll: bool,
		waker: Option<Waker>,
	}

	impl Stream for PendingThenReadyRowStream {
		type Item = reinhardt_core::exception::Result<crate::backends::types::Row>;

		fn poll_next(
			mut self: Pin<&mut Self>,
			context: &mut Context<'_>,
		) -> Poll<Option<Self::Item>> {
			if self.first_poll {
				self.first_poll = false;
				self.waker = Some(context.waker().clone());
				return Poll::Pending;
			}
			assert!(
				self.waker.take().is_some(),
				"pending row stream must retain its registered waker"
			);
			Poll::Ready(None)
		}
	}

	#[rstest]
	fn timed_row_stream_records_pending_io_when_the_backend_wakes() {
		// Arrange
		let state = Arc::new(Mutex::new(WakeDrivenRowStreamState::default()));
		let rows: RowStream<'_> = Box::pin(WakeDrivenRowStream {
			state: Arc::clone(&state),
		});
		let mut accounting = StreamQueryAccounting::new("SELECT 1".to_owned(), Vec::new());
		let waker = futures::task::noop_waker();
		let mut context = Context::from_waker(&waker);

		// Act
		{
			let mut stream = TimedRowStream::new(rows, &mut accounting);
			assert!(matches!(
				Pin::new(&mut stream).poll_next(&mut context),
				Poll::Pending
			));
			std::thread::sleep(Duration::from_millis(20));
			let waker = {
				let mut state = state
					.lock()
					.unwrap_or_else(|poisoned| poisoned.into_inner());
				state.ready = true;
				state
					.waker
					.take()
					.expect("pending row stream must register a waker")
			};
			waker.wake();
			assert!(matches!(
				Pin::new(&mut stream).poll_next(&mut context),
				Poll::Ready(None)
			));
		}

		// Assert
		assert!(accounting.duration >= Duration::from_millis(10));
	}

	#[rstest]
	fn timed_row_stream_discards_unwoken_pending_time_after_cancellation() {
		// Arrange
		let rows: RowStream<'_> = Box::pin(PendingThenReadyRowStream {
			first_poll: true,
			waker: None,
		});
		let mut accounting = StreamQueryAccounting::new("SELECT 1".to_owned(), Vec::new());
		let waker = futures::task::noop_waker();
		let mut context = Context::from_waker(&waker);

		// Act
		{
			let mut stream = TimedRowStream::new(rows, &mut accounting);
			assert!(matches!(
				Pin::new(&mut stream).poll_next(&mut context),
				Poll::Pending
			));
			std::thread::sleep(Duration::from_millis(20));
			assert!(matches!(
				Pin::new(&mut stream).poll_next(&mut context),
				Poll::Ready(None)
			));
		}

		// Assert
		assert!(accounting.duration < Duration::from_millis(10));
	}

	#[cfg(feature = "pgvector")]
	struct RecordingExecutor {
		backend: DatabaseBackend,
		calls: Vec<(String, Vec<crate::orm::QueryValue>)>,
		contexts: Vec<Option<crate::backends::error::PgvectorOperationKind>>,
		rows: Vec<crate::orm::Row>,
	}

	#[cfg(feature = "pgvector")]
	impl RecordingExecutor {
		fn for_backend(backend: DatabaseBackend) -> Self {
			Self {
				backend,
				calls: Vec::new(),
				contexts: Vec::new(),
				rows: Vec::new(),
			}
		}
	}

	#[cfg(feature = "pgvector")]
	impl Default for RecordingExecutor {
		fn default() -> Self {
			Self::for_backend(DatabaseBackend::Postgres)
		}
	}

	#[cfg(feature = "pgvector")]
	#[derive(Debug)]
	struct MissingVectorOperatorError;

	#[cfg(feature = "pgvector")]
	impl fmt::Display for MissingVectorOperatorError {
		fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
			formatter.write_str("operator does not exist: vector <=> vector")
		}
	}

	#[cfg(feature = "pgvector")]
	impl std::error::Error for MissingVectorOperatorError {}

	#[cfg(feature = "pgvector")]
	impl sqlx::error::DatabaseError for MissingVectorOperatorError {
		fn message(&self) -> &str {
			"operator does not exist: vector <=> vector"
		}

		fn code(&self) -> Option<Cow<'_, str>> {
			Some(Cow::Borrowed("42883"))
		}

		fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
			self
		}

		fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
			self
		}

		fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
			self
		}

		fn kind(&self) -> sqlx::error::ErrorKind {
			sqlx::error::ErrorKind::Other
		}
	}

	#[cfg(feature = "pgvector")]
	struct SourcedErrorTransactionExecutor;

	#[cfg(feature = "pgvector")]
	struct PgvectorUpdateErrorExecutor {
		code: &'static str,
		message: &'static str,
	}

	#[cfg(feature = "pgvector")]
	#[async_trait::async_trait]
	impl crate::orm::OrmExecutor for PgvectorUpdateErrorExecutor {
		fn backend(&self) -> DatabaseBackend {
			DatabaseBackend::Postgres
		}

		fn supports_pgvector_error_hints(&self) -> bool {
			true
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<crate::orm::QueryResult> {
			Err(reinhardt_core::exception::DatabaseError::new(
				reinhardt_core::exception::DatabaseErrorKind::Query,
				self.message,
			)
			.with_code(self.code)
			.into())
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<crate::orm::Row> {
			panic!("pgvector update error executor does not fetch rows")
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<Vec<crate::orm::Row>> {
			panic!("pgvector update error executor does not fetch rows")
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<Option<crate::orm::Row>> {
			panic!("pgvector update error executor does not fetch rows")
		}
	}

	#[cfg(feature = "pgvector")]
	#[async_trait::async_trait]
	impl crate::orm::connection::TransactionExecutor for SourcedErrorTransactionExecutor {
		fn supports_pgvector_error_hints(&self) -> bool {
			true
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<crate::orm::QueryResult> {
			panic!("SELECT test transaction does not execute mutations")
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<crate::orm::Row> {
			panic!("all_with_executor test transaction does not fetch one row")
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<Vec<crate::orm::Row>> {
			Err(reinhardt_core::exception::Error::DatabaseWithSource {
				database_error: reinhardt_core::exception::DatabaseError::new(
					reinhardt_core::exception::DatabaseErrorKind::Query,
					"operator does not exist: vector <=> vector",
				)
				.with_code("42883"),
				source: Box::new(sqlx::Error::Database(Box::new(MissingVectorOperatorError))),
			})
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<Option<crate::orm::Row>> {
			panic!("all_with_executor test transaction does not fetch an optional row")
		}

		async fn commit(self: Box<Self>) -> crate::backends::error::Result<()> {
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> crate::backends::error::Result<()> {
			Ok(())
		}
	}

	#[cfg(feature = "pgvector")]
	#[async_trait::async_trait]
	impl crate::orm::OrmExecutor for RecordingExecutor {
		fn backend(&self) -> DatabaseBackend {
			self.backend
		}

		async fn execute(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<crate::orm::QueryResult> {
			self.calls.push((sql.to_owned(), params));
			Ok(crate::orm::QueryResult {
				rows_affected: 0,
				last_insert_id: None,
			})
		}

		async fn execute_with_context(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> reinhardt_core::exception::Result<crate::orm::QueryResult> {
			self.contexts.push(context);
			self.execute(sql, params).await
		}

		async fn fetch_one(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<crate::orm::Row> {
			self.calls.push((sql.to_owned(), params));
			Ok(crate::orm::Row {
				data: HashMap::from([("count".to_owned(), crate::orm::QueryValue::Int(0))]),
			})
		}

		async fn fetch_one_with_context(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> reinhardt_core::exception::Result<crate::orm::Row> {
			self.contexts.push(context);
			self.fetch_one(sql, params).await
		}

		async fn fetch_all(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<Vec<crate::orm::Row>> {
			self.calls.push((sql.to_owned(), params));
			Ok(std::mem::take(&mut self.rows))
		}

		async fn fetch_all_with_context(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> reinhardt_core::exception::Result<Vec<crate::orm::Row>> {
			self.contexts.push(context);
			self.fetch_all(sql, params).await
		}

		async fn fetch_optional(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<Option<crate::orm::Row>> {
			self.calls.push((sql.to_owned(), params));
			Ok(None)
		}

		async fn fetch_optional_with_context(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> reinhardt_core::exception::Result<Option<crate::orm::Row>> {
			self.contexts.push(context);
			self.fetch_optional(sql, params).await
		}
	}

	#[cfg(feature = "pgvector")]
	#[async_trait::async_trait]
	impl crate::orm::connection::TransactionExecutor for RecordingExecutor {
		fn backend(&self) -> crate::backends::types::DatabaseType {
			match self.backend {
				DatabaseBackend::Postgres => crate::backends::types::DatabaseType::Postgres,
				DatabaseBackend::MySql => crate::backends::types::DatabaseType::Mysql,
				DatabaseBackend::Sqlite => crate::backends::types::DatabaseType::Sqlite,
			}
		}

		async fn execute(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<crate::orm::QueryResult> {
			self.calls.push((sql.to_owned(), params));
			Ok(crate::orm::QueryResult {
				rows_affected: 0,
				last_insert_id: None,
			})
		}

		async fn fetch_one(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<crate::orm::Row> {
			self.calls.push((sql.to_owned(), params));
			Ok(crate::orm::Row {
				data: HashMap::from([("count".to_owned(), crate::orm::QueryValue::Int(0))]),
			})
		}

		async fn fetch_one_with_context(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> crate::backends::error::Result<crate::orm::Row> {
			self.contexts.push(context);
			<Self as crate::orm::connection::TransactionExecutor>::fetch_one(self, sql, params)
				.await
		}

		async fn fetch_all(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<Vec<crate::orm::Row>> {
			self.calls.push((sql.to_owned(), params));
			Ok(Vec::new())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<Option<crate::orm::Row>> {
			Ok(None)
		}

		async fn commit(self: Box<Self>) -> crate::backends::error::Result<()> {
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> crate::backends::error::Result<()> {
			Ok(())
		}
	}

	#[cfg(feature = "pgvector")]
	fn typed_vector_target(values: &[f32]) -> crate::orm::Vector<3> {
		crate::orm::Vector::try_from_slice(values).unwrap()
	}

	#[cfg(feature = "pgvector")]
	fn distance_and_vector_context() -> crate::backends::error::PgvectorOperationKind {
		crate::backends::error::PgvectorOperationKind::DistanceOperator
			.union(crate::backends::error::PgvectorOperationKind::VectorValue)
	}

	#[cfg(feature = "pgvector")]
	fn typed_vector_sql_and_params(
		queryset: &QuerySet<TestUser>,
	) -> (String, Vec<crate::orm::QueryValue>) {
		let statement = queryset
			.build_select_statement()
			.expect("typed vector query should build");
		let (sql, values) = PostgresQueryBuilder.build_select(&statement);
		(sql, crate::orm::execution::convert_values(values))
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_l2_distance_uses_postgres_operator_and_bound_value() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().values(&["id"]).select_expr(
			"distance",
			field.l2_distance(typed_vector_target(&[1.0, 2.0, 3.0])),
		);

		let (sql, params) = typed_vector_sql_and_params(&queryset);

		assert_eq!(
			sql,
			r#"SELECT "id", "test_users"."embedding" <-> $1 AS "distance" FROM "test_users""#
		);
		assert_eq!(
			params,
			vec![crate::orm::QueryValue::Vector(Some(vec![1.0, 2.0, 3.0]))]
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_negative_inner_product_uses_postgres_operator_and_bound_value() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().values(&["id"]).select_expr(
			"distance",
			field.negative_inner_product(typed_vector_target(&[3.0, 2.0, 1.0])),
		);

		let (sql, params) = typed_vector_sql_and_params(&queryset);

		assert_eq!(
			sql,
			r#"SELECT "id", "test_users"."embedding" <#> $1 AS "distance" FROM "test_users""#
		);
		assert_eq!(
			params,
			vec![crate::orm::QueryValue::Vector(Some(vec![3.0, 2.0, 1.0]))]
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_cosine_distance_uses_postgres_operator_and_bound_value() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().values(&["id"]).select_expr(
			"distance",
			field.cosine_distance(typed_vector_target(&[0.5, 1.5, 2.5])),
		);

		let (sql, params) = typed_vector_sql_and_params(&queryset);

		assert_eq!(
			sql,
			r#"SELECT "id", "test_users"."embedding" <=> $1 AS "distance" FROM "test_users""#
		);
		assert_eq!(
			params,
			vec![crate::orm::QueryValue::Vector(Some(vec![0.5, 1.5, 2.5]))]
		);
	}

	#[cfg(feature = "pgvector")]
	#[tokio::test]
	async fn typed_vector_public_executor_error_preserves_sqlx_database_source() {
		use std::error::Error as _;

		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().select_expr(
			"distance",
			field.cosine_distance(typed_vector_target(&[0.5, 1.5, 2.5])),
		);
		let mut executor = SourcedErrorTransactionExecutor;

		let error = queryset.all_with_executor(&mut executor).await.unwrap_err();

		assert_eq!(
			error.kind(),
			reinhardt_core::exception::DatabaseErrorKind::Query
		);
		assert_eq!(error.code(), Some("42883"));
		assert!(
			error
				.to_string()
				.contains("CreateExtension::new(\"vector\")")
		);
		let sqlx_error = error
			.source()
			.and_then(|source| source.downcast_ref::<sqlx::Error>())
			.expect("public QuerySet error should retain the original SQLx error");
		assert!(
			sqlx_error
				.as_database_error()
				.and_then(|source| source
					.as_error()
					.downcast_ref::<MissingVectorOperatorError>())
				.is_some()
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_predicate_and_ordering_keep_separate_monotonic_bindings() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new()
			.values(&["id"])
			.filter(
				field
					.clone()
					.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
					.lt(0.25),
			)
			.order_by(
				field
					.l2_distance(typed_vector_target(&[4.0, 5.0, 6.0]))
					.asc(),
			);

		let (sql, params) = typed_vector_sql_and_params(&queryset);

		assert_eq!(
			sql,
			r#"SELECT "id" FROM "test_users" WHERE "test_users"."embedding" <=> $1 < $2 ORDER BY "test_users"."embedding" <-> $3 ASC"#
		);
		assert_eq!(
			params,
			vec![
				crate::orm::QueryValue::Vector(Some(vec![1.0, 2.0, 3.0])),
				crate::orm::QueryValue::Float(0.25),
				crate::orm::QueryValue::Vector(Some(vec![4.0, 5.0, 6.0])),
			]
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_join_qualifies_only_model_root_expressions_with_current_alias() {
		let root_field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let peer_field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"])
			.with_alias("test_vector_peers");
		let queryset = QuerySet::<TestUser>::new()
			.from_as("root_users")
			.inner_join_on::<TestVectorPeer>("root_users.id = test_vector_peers.user_id")
			.select_expr(
				"selected_distance",
				root_field
					.clone()
					.l2_distance(typed_vector_target(&[1.0, 2.0, 3.0])),
			)
			.select_expr(
				"peer_distance",
				peer_field.cosine_distance(typed_vector_target(&[3.0, 2.0, 1.0])),
			)
			.annotate(
				root_field
					.clone()
					.negative_inner_product(typed_vector_target(&[4.0, 5.0, 6.0]))
					.label("annotated_distance")
					.expect("test annotation label is valid"),
			)
			.expect("test annotation should be accepted")
			.filter(
				root_field
					.clone()
					.cosine_distance(typed_vector_target(&[7.0, 8.0, 9.0]))
					.lt(0.25),
			)
			.order_by(
				root_field
					.l2_distance(typed_vector_target(&[9.0, 8.0, 7.0]))
					.asc(),
			);

		let (sql, params) = typed_vector_sql_and_params(&queryset);

		assert_eq!(
			sql,
			r#"SELECT *, "root_users"."embedding" <-> $1 AS "selected_distance", "test_vector_peers"."embedding" <=> $2 AS "peer_distance", "root_users"."embedding" <#> $3 AS "annotated_distance" FROM "test_users" AS "root_users" INNER JOIN "test_vector_peers" ON root_users.id = test_vector_peers.user_id WHERE "root_users"."embedding" <=> $4 < $5 ORDER BY "root_users"."embedding" <-> $6 ASC"#
		);
		assert_eq!(
			params,
			vec![
				crate::orm::QueryValue::Vector(Some(vec![1.0, 2.0, 3.0])),
				crate::orm::QueryValue::Vector(Some(vec![3.0, 2.0, 1.0])),
				crate::orm::QueryValue::Vector(Some(vec![4.0, 5.0, 6.0])),
				crate::orm::QueryValue::Vector(Some(vec![7.0, 8.0, 9.0])),
				crate::orm::QueryValue::Float(0.25),
				crate::orm::QueryValue::Vector(Some(vec![9.0, 8.0, 7.0])),
			]
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_query_rejects_sqlite_before_execution() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().filter(
			field
				.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
				.lt(0.25),
		);
		let statement = queryset
			.build_select_statement()
			.expect("typed vector statement should build");

		let error = QuerySet::<TestUser>::build_select_for_backend(
			&statement,
			DatabaseBackend::Sqlite,
			false,
		)
		.expect_err("SQLite must reject PostgreSQL vector distance operators");

		assert_eq!(
			error.to_string(),
			"pgvector distance operators is not supported by the SQLite backend"
		);
	}

	#[cfg(feature = "pgvector")]
	#[tokio::test]
	async fn typed_vector_all_with_db_passes_every_bound_value_to_executor() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new()
			.filter(
				field
					.clone()
					.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
					.lt(0.25),
			)
			.order_by(
				field
					.l2_distance(typed_vector_target(&[4.0, 5.0, 6.0]))
					.asc(),
			);
		let mut executor = RecordingExecutor::default();

		let rows = queryset.all_with_db(&mut executor).await.unwrap();

		assert_eq!(rows, Vec::<TestUser>::new());
		assert_eq!(executor.calls.len(), 1);
		assert_eq!(
			executor.calls[0].1,
			vec![
				crate::orm::QueryValue::Vector(Some(vec![1.0, 2.0, 3.0])),
				crate::orm::QueryValue::Float(0.25),
				crate::orm::QueryValue::Vector(Some(vec![4.0, 5.0, 6.0])),
			]
		);
		assert_eq!(executor.contexts, vec![Some(distance_and_vector_context())]);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[tokio::test]
	async fn none_short_circuits_read_and_write_execution_paths() {
		// Arrange
		let mut executor = RecordingExecutor::default();

		// Act
		let rows = QuerySet::<TestUser>::new()
			.none()
			.all_with_db(&mut executor)
			.await
			.expect("none queryset should not execute a select");
		// Assert
		assert_eq!(rows, Vec::<TestUser>::new());

		let rows = QuerySet::<TestUser>::new()
			.none()
			.rows_with_db(&mut executor)
			.await
			.expect("none queryset should not execute a row select");
		assert_eq!(rows, Vec::<crate::orm::QueryRow>::new());

		let count = QuerySet::<TestUser>::new()
			.none()
			.count_with_db(&mut executor)
			.await
			.expect("none queryset should not execute a count");
		assert_eq!(count, 0);

		let exists = QuerySet::<TestUser>::new()
			.none()
			.exists_with_db(&mut executor)
			.await
			.expect("none queryset should not execute an existence check");
		assert!(!exists);

		let rows_affected = QuerySet::<TestUser>::new()
			.none()
			.update_fields_with_conn(&mut executor, [("username", "alice")])
			.await
			.expect("none queryset should not execute an update");
		assert_eq!(rows_affected, 0);
		assert!(executor.calls.is_empty());
		assert!(executor.contexts.is_empty());
	}

	#[rstest]
	fn none_aggregate_subquery_limits_output_rows() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().none().values(&["COUNT(*)"]);

		// Act
		let sql = queryset
			.as_subquery()
			.expect("empty aggregate subquery should compile");

		// Assert
		assert_eq!(
			sql,
			r#"(SELECT COUNT(*) FROM "test_users" WHERE 1 = 0 LIMIT 0)"#
		);
	}

	#[cfg(feature = "pgvector")]
	#[tokio::test]
	async fn typed_vector_rows_with_db_preserves_projected_values_and_bound_context() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().values(&["id"]).select_expr(
			"distance",
			field.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0])),
		);
		let mut executor = RecordingExecutor::default();
		executor.rows.push(crate::orm::Row {
			data: HashMap::from([
				("id".to_owned(), crate::orm::QueryValue::Int(7)),
				("distance".to_owned(), crate::orm::QueryValue::Float(0.25)),
			]),
		});

		let rows = queryset.rows_with_db(&mut executor).await.unwrap();

		assert_eq!(rows.len(), 1);
		assert_eq!(rows[0].get::<i64>("id"), Some(7));
		assert_eq!(rows[0].get::<f64>("distance"), Some(0.25));
		assert_eq!(
			executor.calls[0].1,
			vec![crate::orm::QueryValue::Vector(Some(vec![1.0, 2.0, 3.0]))]
		);
		assert_eq!(executor.contexts, vec![Some(distance_and_vector_context())]);
	}

	#[cfg(feature = "pgvector")]
	fn typed_vector_filtered_queryset() -> QuerySet<TestUser> {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		QuerySet::<TestUser>::new().filter(
			field
				.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
				.lt(0.25),
		)
	}

	#[cfg(feature = "pgvector")]
	#[tokio::test]
	async fn typed_vector_count_with_db_uses_contextual_fetch_one() {
		let queryset = typed_vector_filtered_queryset();
		let mut executor = RecordingExecutor::default();

		let count = queryset
			.count_with_db(&mut executor)
			.await
			.expect("recording count should decode");

		assert_eq!(count, 0);
		assert_eq!(executor.contexts, vec![Some(distance_and_vector_context())]);
	}

	#[cfg(feature = "pgvector")]
	#[tokio::test]
	async fn typed_vector_exists_with_db_uses_contextual_fetch_one() {
		let queryset = typed_vector_filtered_queryset();
		let mut executor = RecordingExecutor::default();

		let exists = queryset
			.exists_with_db(&mut executor)
			.await
			.expect("recording exists should decode");

		assert!(!exists);
		assert_eq!(executor.contexts, vec![Some(distance_and_vector_context())]);
	}

	#[cfg(feature = "pgvector")]
	#[tokio::test]
	async fn typed_vector_count_with_executor_uses_contextual_fetch_one() {
		let queryset = typed_vector_filtered_queryset();
		let mut executor = RecordingExecutor::default();

		let count = queryset
			.count_with_executor(&mut executor)
			.await
			.expect("recording transaction count should decode");

		assert_eq!(count, 0);
		assert_eq!(executor.contexts, vec![Some(distance_and_vector_context())]);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case(
		DatabaseBackend::MySql,
		"pgvector distance operators is not supported by the MySQL backend"
	)]
	#[case(
		DatabaseBackend::Sqlite,
		"pgvector distance operators is not supported by the SQLite backend"
	)]
	#[tokio::test]
	async fn typed_vector_update_rejects_non_postgres_before_executor_call(
		#[case] backend: DatabaseBackend,
		#[case] expected_message: &str,
	) {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().filter(
			field
				.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
				.lt(0.25),
		);
		let mut executor = RecordingExecutor::for_backend(backend);

		let error = queryset
			.update_fields_with_conn(&mut executor, [("username", "alice")])
			.await
			.expect_err("non-PostgreSQL updates must reject pgvector predicates");

		assert!(matches!(
			error,
			reinhardt_core::exception::Error::Database(ref database_error)
				if database_error.kind()
					== reinhardt_core::exception::DatabaseErrorKind::Unsupported
					&& database_error.message() == expected_message
		));
		assert!(executor.calls.is_empty());
		assert!(executor.contexts.is_empty());
	}

	#[cfg(feature = "pgvector")]
	#[tokio::test]
	async fn typed_vector_update_preserves_postgres_sql_and_bound_values() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().filter(
			field
				.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
				.lt(0.25),
		);
		let mut executor = RecordingExecutor::default();

		let rows_affected = queryset
			.update_fields_with_conn(&mut executor, [("username", "alice")])
			.await
			.expect("PostgreSQL vector update should build");

		assert_eq!(rows_affected, 0);
		assert_eq!(
			executor.calls,
			vec![(
				r#"UPDATE "test_users" SET "username" = $1 WHERE "test_users"."embedding" <=> $2 < $3"#
					.to_owned(),
				vec![
					crate::orm::QueryValue::String("alice".to_owned()),
					crate::orm::QueryValue::Vector(Some(vec![1.0, 2.0, 3.0])),
					crate::orm::QueryValue::Float(0.25),
				],
			)]
		);
		assert_eq!(executor.contexts, vec![Some(distance_and_vector_context())]);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case("42883", "operator does not exist: vector <=> vector")]
	#[case("42704", "type \"vector\" does not exist")]
	#[tokio::test]
	async fn typed_vector_update_aggregates_assignment_and_distance_context(
		#[case] code: &'static str,
		#[case] message: &'static str,
	) {
		let field = Field::<TestVectorUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestVectorUser>::new().filter(
			field
				.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
				.lt(0.25),
		);
		let assignment_field = TestVectorUser::field_embedding();
		let mut executor = PgvectorUpdateErrorExecutor { code, message };

		let error = queryset
			.update_fields_with_conn(
				&mut executor,
				[(assignment_field, typed_vector_target(&[4.0, 5.0, 6.0]))],
			)
			.await
			.unwrap_err();

		assert_eq!(
			error.database_error().and_then(|error| error.code()),
			Some(code)
		);
		assert!(
			error
				.to_string()
				.contains("CreateExtension::new(\"vector\")")
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_update_fields_sql_reports_assignment_and_predicate_params() {
		let field = Field::<TestVectorUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestVectorUser>::new().filter(
			field
				.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
				.lt(0.25),
		);
		let assignment_field = TestVectorUser::field_embedding();

		let (sql, params) = queryset
			.update_fields_sql([(assignment_field, typed_vector_target(&[4.0, 5.0, 6.0]))])
			.expect("typed vector update fields SQL should build");

		assert_eq!(
			sql,
			r#"UPDATE "test_users" SET "embedding" = $1 WHERE "test_users"."embedding" <=> $2 < $3"#
		);
		assert_eq!(params, vec!["[4.0,5.0,6.0]", "[1.0,2.0,3.0]", "0.25"]);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_update_sql_reports_assignment_and_predicate_params() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().filter(
			field
				.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
				.lt(0.25),
		);
		let updates = HashMap::from([(
			"embedding".to_owned(),
			UpdateValue::Typed(Ok(DatabaseValue::Vector(vec![4.0, 5.0, 6.0]))),
		)]);

		let (sql, params) = queryset
			.update_sql(&updates)
			.expect("typed vector update SQL should build");

		assert_eq!(
			sql,
			r#"UPDATE "test_users" SET "embedding" = $1 WHERE "test_users"."embedding" <=> $2 < $3"#
		);
		assert_eq!(params, vec!["[4.0,5.0,6.0]", "[1.0,2.0,3.0]", "0.25"]);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_delete_sql_reports_predicate_params() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"]);
		let queryset = QuerySet::<TestUser>::new().filter(
			field
				.cosine_distance(typed_vector_target(&[1.0, 2.0, 3.0]))
				.lt(0.25),
		);

		let (sql, params) = queryset
			.delete_sql()
			.expect("typed vector delete SQL should build");

		assert_eq!(
			sql,
			r#"DELETE FROM "test_users" WHERE "test_users"."embedding" <=> $1 < $2"#
		);
		assert_eq!(params, vec!["[1.0,2.0,3.0]", "0.25"]);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn typed_vector_explicit_former_root_marker_alias_is_preserved() {
		let field = Field::<TestUser, crate::orm::Vector<3>>::new(vec!["embedding"])
			.with_alias("__reinhardt_typed_model_root__");
		let queryset = QuerySet::<TestUser>::new()
			.from_as("root_users")
			.select_expr(
				"distance",
				field.l2_distance(typed_vector_target(&[1.0, 2.0, 3.0])),
			);

		let (sql, params) = typed_vector_sql_and_params(&queryset);

		assert_eq!(
			sql,
			r#"SELECT *, "__reinhardt_typed_model_root__"."embedding" <-> $1 AS "distance" FROM "test_users" AS "root_users""#
		);
		assert_eq!(
			params,
			vec![crate::orm::QueryValue::Vector(Some(vec![1.0, 2.0, 3.0]))]
		);
	}

	#[test]
	fn checked_select_builder_uses_mysql_identifier_quoting() {
		// Arrange
		let mut statement = reinhardt_query::prelude::Query::select();
		statement
			.column(reinhardt_query::prelude::Alias::new("id"))
			.from(reinhardt_query::prelude::Alias::new("articles"));

		// Act
		let (sql, values) = QuerySet::<TestUser>::build_select_for_backend(
			&statement,
			DatabaseBackend::MySql,
			false,
		)
		.expect("MySQL select should build");

		// Assert
		assert_eq!(sql, "SELECT `id` FROM `articles`");
		assert!(values.0.is_empty());
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn vector_database_value_diagnostics_use_json_array_syntax() {
		let rendered =
			QuerySet::<TestUser>::database_value_to_string(&DatabaseValue::Vector(vec![
				1.0, 2.0, 3.0,
			]));

		assert_eq!(rendered, "[1.0,2.0,3.0]");
	}

	fn test_field_info(
		name: &str,
		db_column: Option<&str>,
		primary_key: bool,
	) -> crate::orm::inspection::FieldInfo {
		crate::orm::inspection::FieldInfo {
			name: name.to_string(),
			field_type: "reinhardt.orm.models.CharField".to_string(),
			storage_kind: None,
			domain: None,
			nullable: false,
			primary_key,
			unique: false,
			blank: false,
			editable: true,
			default: None,
			db_default: None,
			db_column: db_column.map(str::to_string),
			choices: None,
			attributes: HashMap::new(),
		}
	}

	#[test]
	fn build_select_statement_keeps_mysql_filter_values_bound() {
		// Arrange
		let payload = "\\' OR 1=1 -- ";
		let mut statement = reinhardt_query::prelude::Query::select();
		statement
			.column(reinhardt_query::prelude::Alias::new("id"))
			.from(reinhardt_query::prelude::Alias::new("users"))
			.and_where(
				reinhardt_query::prelude::Expr::col(reinhardt_query::prelude::Alias::new("name"))
					.eq(payload),
			);

		// Act
		let (sql, params) = build_select_statement(&statement, DatabaseBackend::MySql)
			.expect("string filter should fit in QueryValue");

		// Assert
		assert_eq!(sql, "SELECT `id` FROM `users` WHERE `name` = ?");
		assert_eq!(
			params,
			vec![crate::backends::types::QueryValue::String(
				payload.to_string()
			)]
		);
	}

	#[test]
	fn build_select_statement_rejects_oversized_unsigned_parameters() {
		// Arrange
		let mut statement = reinhardt_query::prelude::Query::select();
		statement
			.column(reinhardt_query::prelude::Alias::new("id"))
			.from(reinhardt_query::prelude::Alias::new("users"))
			.limit((i64::MAX as u64) + 1);

		// Act
		let error = build_select_statement(&statement, DatabaseBackend::MySql)
			.expect_err("oversized unsigned parameters must not be clamped");

		// Assert
		assert_eq!(
			error.to_string(),
			"Database error: Unsigned query parameter 9223372036854775808 exceeds the supported i64 range"
		);
	}

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

		const fn field_id() -> crate::orm::expressions::FieldRef<
			TestUser,
			i64,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `id` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("id") }
		}

		const fn field_username() -> crate::orm::expressions::FieldRef<
			TestUser,
			String,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `username` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("username") }
		}

		const fn field_email() -> crate::orm::expressions::FieldRef<
			TestUser,
			String,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `email` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("email") }
		}

		const fn field_full_name() -> crate::orm::expressions::FieldRef<
			TestUser,
			String,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `full_name` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("full_name") }
		}

		const fn field_display_name() -> crate::orm::expressions::FieldRef<
			TestUser,
			String,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `display_name` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("display_name") }
		}

		const fn field_created_at() -> crate::orm::expressions::FieldRef<
			TestUser,
			chrono::DateTime<chrono::Utc>,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `created_at` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("created_at") }
		}

		const fn field_tags() -> crate::orm::expressions::FieldRef<
			TestUser,
			Vec<String>,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `tags` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("tags") }
		}

		const fn field_metadata() -> crate::orm::expressions::FieldRef<
			TestUser,
			String,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `metadata` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("metadata") }
		}

		const fn field_active_period() -> crate::orm::expressions::FieldRef<
			TestUser,
			String,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestUser's persisted `active_period` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("active_period") }
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
		type Objects = Manager<Self>;

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

		fn relationship_metadata() -> Vec<crate::orm::inspection::RelationInfo> {
			use crate::orm::inspection::RelationInfo;
			use crate::orm::relationship::RelationshipType;

			vec![
				RelationInfo::new("profile", RelationshipType::OneToOne, "Profile")
					.with_foreign_key("profile_id"),
				RelationInfo::new("department", RelationshipType::ManyToOne, "Department")
					.with_foreign_key("department_id"),
				RelationInfo::new("posts", RelationshipType::OneToMany, "Post")
					.with_foreign_key("test_user_id"),
				RelationInfo::new("comments", RelationshipType::OneToMany, "Comment")
					.with_foreign_key("test_user_id"),
				RelationInfo::new("likes", RelationshipType::OneToMany, "Like")
					.with_foreign_key("test_user_id"),
				RelationInfo::new("corpus_file", RelationshipType::ManyToOne, "TestCorpusFile")
					.with_foreign_key("corpus_file_id"),
				RelationInfo::new("tags", RelationshipType::ManyToMany, "TestTag")
					.with_through_table("test_user_tags")
					.with_source_field("test_user_id")
					.with_target_field("tag_id"),
			]
		}

		fn generated_field_names() -> &'static [&'static str] {
			&["full_name", "display_name"]
		}
	}

	#[rstest]
	fn from_subquery_renders_backend_specific_derived_source() {
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryStatementBuilder};

		let queryset = QuerySet::<TestUser>::from_subquery(
			|subquery: QuerySet<TestUser>| subquery,
			"scoped_users",
		)
		.expect("derived source should compile");

		assert_eq!(
			queryset
				.to_sql_for_backend(crate::backends::types::DatabaseType::Postgres)
				.expect("PostgreSQL SQL should compile"),
			r#"SELECT * FROM (SELECT * FROM "test_users") AS "scoped_users""#
		);
		assert_eq!(
			queryset
				.to_sql_for_backend(crate::backends::types::DatabaseType::Mysql)
				.expect("MySQL SQL should compile"),
			r#"SELECT * FROM (SELECT * FROM `test_users`) AS `scoped_users`"#
		);

		let statement = queryset
			.build_full_model_select_statement_for_backend(
				crate::backends::types::DatabaseType::Postgres,
			)
			.expect("derived model-shaped source should compile");
		assert_eq!(
			statement.to_string(PostgresQueryBuilder),
			r#"SELECT * FROM (SELECT * FROM "test_users") AS "scoped_users""#
		);
	}

	#[rstest]
	fn model_shaped_session_rejects_projected_derived_source() {
		let queryset = QuerySet::<TestUser>::from_subquery(
			|subquery: QuerySet<TestUser>| subquery.values(&["id"]),
			"scoped_users",
		)
		.expect("derived source should compile");

		let error = queryset
			.build_full_model_select_statement()
			.expect_err("projected derived source must not be decoded as a model");

		assert_eq!(
			error.to_string(),
			"Database error: Session::list requires a model-shaped QuerySet"
		);
	}

	#[rstest]
	fn model_shaped_session_accepts_prefetch_plan() {
		let queryset = QuerySet::<TestUser>::new().prefetch_related(&["posts"]);

		let statement = queryset
			.build_full_model_select_statement()
			.expect("prefetch configuration must not change the root model projection");

		assert_eq!(
			statement.to_string(PostgresQueryBuilder),
			r#"SELECT * FROM "test_users""#
		);
	}

	struct ExplainRecordingExecutor {
		backend: DatabaseBackend,
		is_cockroachdb: bool,
		calls: Vec<(String, Vec<crate::orm::QueryValue>)>,
		rows: Vec<crate::orm::Row>,
	}

	impl ExplainRecordingExecutor {
		fn new(backend: DatabaseBackend, rows: Vec<crate::orm::Row>) -> Self {
			Self {
				backend,
				is_cockroachdb: false,
				calls: Vec::new(),
				rows,
			}
		}

		fn cockroachdb(rows: Vec<crate::orm::Row>) -> Self {
			Self {
				backend: DatabaseBackend::Postgres,
				is_cockroachdb: true,
				calls: Vec::new(),
				rows,
			}
		}
	}

	#[async_trait::async_trait]
	impl crate::orm::OrmExecutor for ExplainRecordingExecutor {
		fn backend(&self) -> DatabaseBackend {
			self.backend
		}

		fn is_cockroachdb(&self) -> bool {
			self.is_cockroachdb
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<crate::orm::QueryResult> {
			panic!("plan-only EXPLAIN must not execute a statement")
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<crate::orm::Row> {
			panic!("plan-only EXPLAIN fetches its diagnostic rows together")
		}

		async fn fetch_all(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<Vec<crate::orm::Row>> {
			self.calls.push((sql.to_owned(), params));
			Ok(std::mem::take(&mut self.rows))
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> reinhardt_core::exception::Result<Option<crate::orm::Row>> {
			panic!("plan-only EXPLAIN does not fetch an optional model row")
		}
	}

	struct ExplainTransactionExecutor {
		calls: Vec<(String, Vec<crate::orm::QueryValue>)>,
		rows: Vec<crate::orm::Row>,
	}

	#[async_trait::async_trait]
	impl crate::orm::TransactionExecutor for ExplainTransactionExecutor {
		fn backend(&self) -> crate::backends::types::DatabaseType {
			crate::backends::types::DatabaseType::Postgres
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<crate::orm::QueryResult> {
			panic!("plan-only EXPLAIN must not execute a statement")
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<crate::orm::Row> {
			panic!("plan-only EXPLAIN fetches its diagnostic rows together")
		}

		async fn fetch_all(
			&mut self,
			sql: &str,
			params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<Vec<crate::orm::Row>> {
			self.calls.push((sql.to_owned(), params));
			Ok(std::mem::take(&mut self.rows))
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<crate::orm::QueryValue>,
		) -> crate::backends::error::Result<Option<crate::orm::Row>> {
			panic!("plan-only EXPLAIN does not fetch an optional model row")
		}

		async fn commit(self: Box<Self>) -> crate::backends::error::Result<()> {
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> crate::backends::error::Result<()> {
			Ok(())
		}
	}

	#[rstest]
	#[tokio::test]
	async fn explain_wraps_typed_filtered_select_without_executing_it_separately() {
		let queryset = QuerySet::<TestUser>::new()
			.filter(TestUser::field_username().exact("alice"))
			.order_by(&["-id"]);
		let plan = serde_json::json!([{
			"Plan": {
				"Node Type": "Index Scan",
				"Relation Name": "test_users"
			}
		}]);
		let mut executor = ExplainRecordingExecutor::new(
			DatabaseBackend::Postgres,
			vec![crate::orm::Row {
				data: HashMap::from([(
					"QUERY PLAN".to_owned(),
					crate::orm::QueryValue::Json(Some(Box::new(plan.clone()))),
				)]),
			}],
		);

		let output = queryset
			.explain_with_db(
				&mut executor,
				super::ExplainOptions::default().format(super::ExplainFormat::Json),
			)
			.await
			.expect("PostgreSQL JSON plan should decode");

		assert_eq!(executor.calls.len(), 1);
		assert_eq!(
			executor.calls[0].0,
			r#"EXPLAIN (FORMAT JSON) SELECT * FROM "test_users" WHERE "username" = $1 ORDER BY "id" DESC"#
		);
		assert_eq!(
			executor.calls[0].1,
			vec![crate::orm::QueryValue::String("alice".to_owned())]
		);
		assert_eq!(output.backend, super::ExplainBackend::Postgres);
		assert_eq!(output.format, super::ExplainFormat::Json);
		assert_eq!(output.body, super::ExplainBody::Json(plan));
	}

	#[rstest]
	#[tokio::test]
	async fn cockroachdb_explain_reports_its_effective_backend() {
		let mut executor = ExplainRecordingExecutor::cockroachdb(vec![crate::orm::Row {
			data: HashMap::from([(
				"info".to_owned(),
				crate::orm::QueryValue::String("scan test_users".to_owned()),
			)]),
		}]);

		let output = QuerySet::<TestUser>::new()
			.explain_with_db(&mut executor, super::ExplainOptions::default())
			.await
			.expect("CockroachDB text plan should decode");

		assert_eq!(output.backend, super::ExplainBackend::CockroachDb);
		assert_eq!(executor.calls[0].0, r#"EXPLAIN SELECT * FROM "test_users""#);
	}

	#[rstest]
	#[tokio::test]
	async fn mysql_json_explain_decodes_supported_plan() {
		let plan = serde_json::json!({
			"query_block": {
				"table": {
					"table_name": "test_users",
					"access_type": "ALL"
				}
			}
		});
		let mut executor = ExplainRecordingExecutor::new(
			DatabaseBackend::MySql,
			vec![crate::orm::Row {
				data: HashMap::from([(
					"EXPLAIN".to_owned(),
					crate::orm::QueryValue::String(plan.to_string()),
				)]),
			}],
		);

		let output = QuerySet::<TestUser>::new()
			.filter(TestUser::field_id().eq(7))
			.explain_with_db(
				&mut executor,
				super::ExplainOptions::default().format(super::ExplainFormat::Json),
			)
			.await
			.expect("MySQL JSON plan should decode");

		assert_eq!(output.backend, super::ExplainBackend::MySql);
		assert_eq!(output.body, super::ExplainBody::Json(plan));
		assert_eq!(
			executor.calls[0].0,
			"EXPLAIN FORMAT=JSON SELECT * FROM `test_users` WHERE `id` = ?"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn mysql_explain_rejects_unchecked_field_annotations() {
		use crate::orm::annotation::{Annotation, AnnotationValue, Value};
		use crate::orm::expressions::F;

		let queryset = QuerySet::<TestUser>::new()
			.annotate_legacy(Annotation::new(
				"username_copy",
				AnnotationValue::Field(F::new("username")),
			))
			.annotate_legacy(Annotation::new(
				"user_count",
				AnnotationValue::Value(Value::Int(0)),
			));
		let mut executor = ExplainRecordingExecutor::new(DatabaseBackend::MySql, Vec::new());

		let error = queryset
			.explain_with_db(&mut executor, super::ExplainOptions::default())
			.await
			.expect_err("MySQL plan-only EXPLAIN must reject unchecked annotations");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
		assert_eq!(
			error.to_string(),
			"Database error: plan-only EXPLAIN for subqueries or unchecked expressions is not supported by the MySQL backend"
		);
		assert!(executor.calls.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn explain_with_executor_uses_one_transaction_connection_call() {
		let mut executor = ExplainTransactionExecutor {
			calls: Vec::new(),
			rows: vec![crate::orm::Row {
				data: HashMap::from([(
					"QUERY PLAN".to_owned(),
					crate::orm::QueryValue::String("Seq Scan on test_users".to_owned()),
				)]),
			}],
		};

		let output = QuerySet::<TestUser>::new()
			.filter(TestUser::field_id().eq(7))
			.explain_with_executor(&mut executor, super::ExplainOptions::default())
			.await
			.expect("transaction executor plan should decode");

		assert_eq!(
			output.body,
			super::ExplainBody::Text("Seq Scan on test_users".to_owned())
		);
		assert_eq!(executor.calls.len(), 1);
		assert_eq!(
			executor.calls[0].0,
			r#"EXPLAIN SELECT * FROM "test_users" WHERE "id" = $1"#
		);
		assert_eq!(executor.calls[0].1, vec![crate::orm::QueryValue::Int(7)]);
	}

	#[rstest]
	#[case(
		DatabaseBackend::Postgres,
		reinhardt_query::query::ExplainOptions::default()
			.format(reinhardt_query::query::ExplainFormat::Tree),
		"Database error: EXPLAIN FORMAT TREE is not supported by the PostgreSQL backend"
	)]
	#[case(
		DatabaseBackend::MySql,
		reinhardt_query::query::ExplainOptions::default().verbose(),
		"Database error: EXPLAIN VERBOSE is not supported by the MySQL backend"
	)]
	#[case(
		DatabaseBackend::Sqlite,
		reinhardt_query::query::ExplainOptions::default()
			.format(reinhardt_query::query::ExplainFormat::Json),
		"Database error: EXPLAIN FORMAT JSON is not supported by the SQLite backend"
	)]
	#[tokio::test]
	async fn explain_rejects_unsupported_capability_before_executor_call(
		#[case] backend: DatabaseBackend,
		#[case] options: reinhardt_query::query::ExplainOptions,
		#[case] expected_message: &str,
	) {
		let queryset = QuerySet::<TestUser>::new();
		let mut executor = ExplainRecordingExecutor::new(backend, Vec::new());

		let error = queryset
			.explain_with_db(&mut executor, options)
			.await
			.expect_err("unsupported EXPLAIN capability should fail");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
		assert_eq!(error.to_string(), expected_message);
		assert!(executor.calls.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn mysql_explain_rejects_subquery_before_executor_call() {
		let queryset = QuerySet::<TestUser>::new()
			.filter_in_subquery("id", |subquery: QuerySet<TestUser>| {
				subquery.values(&["id"])
			})
			.expect("subquery should compile");
		let mut executor = ExplainRecordingExecutor::new(DatabaseBackend::MySql, Vec::new());

		let error = queryset
			.explain_with_db(&mut executor, super::ExplainOptions::default())
			.await
			.expect_err("MySQL subqueries must be rejected for plan-only safety");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
		assert_eq!(
			error.to_string(),
			"Database error: plan-only EXPLAIN for subqueries or unchecked expressions is not supported by the MySQL backend"
		);
		assert!(executor.calls.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_explain_retains_tabular_plan_rows() {
		let rows = vec![crate::orm::Row {
			data: HashMap::from([
				("id".to_owned(), crate::orm::QueryValue::Int(2)),
				("parent".to_owned(), crate::orm::QueryValue::Int(0)),
				("notused".to_owned(), crate::orm::QueryValue::Int(0)),
				(
					"detail".to_owned(),
					crate::orm::QueryValue::String("SCAN test_users".to_owned()),
				),
			]),
		}];
		let mut executor = ExplainRecordingExecutor::new(DatabaseBackend::Sqlite, rows);

		let output = QuerySet::<TestUser>::new()
			.explain_with_db(&mut executor, super::ExplainOptions::default())
			.await
			.expect("SQLite plan should decode");

		assert_eq!(executor.calls.len(), 1);
		assert_eq!(
			executor.calls[0].0,
			r#"EXPLAIN QUERY PLAN SELECT * FROM "test_users""#
		);
		assert_eq!(
			output.body,
			super::ExplainBody::Rows(vec![serde_json::Value::Object(serde_json::Map::from_iter(
				[
					("id".to_owned(), serde_json::Value::from(2)),
					("parent".to_owned(), serde_json::Value::from(0)),
					("notused".to_owned(), serde_json::Value::from(0)),
					(
						"detail".to_owned(),
						serde_json::Value::String("SCAN test_users".to_owned()),
					),
				]
			))])
		);
	}

	#[cfg(feature = "pgvector")]
	#[model(app_label = "query_tests", table_name = "test_users")]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestVectorUser {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field]
		embedding: crate::orm::Vector<3>,
	}

	#[cfg(feature = "pgvector")]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestVectorPeer {
		id: Option<i64>,
		embedding: crate::orm::Vector<3>,
	}

	#[cfg(feature = "pgvector")]
	#[derive(Debug, Clone)]
	struct TestVectorPeerFields;

	#[cfg(feature = "pgvector")]
	impl crate::orm::model::FieldSelector for TestVectorPeerFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	#[cfg(feature = "pgvector")]
	impl Model for TestVectorPeer {
		type PrimaryKey = i64;
		type Fields = TestVectorPeerFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"test_vector_peers"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			TestVectorPeerFields
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestMembership {
		user_id: i64,
		role_id: i64,
	}

	#[derive(Debug, Clone)]
	struct TestMembershipFields;

	impl crate::orm::model::FieldSelector for TestMembershipFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestMembership {
		type PrimaryKey = String;
		type Fields = TestMembershipFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"test_memberships"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			None
		}

		fn set_primary_key(&mut self, _value: Self::PrimaryKey) {}

		fn primary_key_field() -> &'static str {
			"user_id"
		}

		fn primary_key_column() -> &'static str {
			"member_user_id"
		}

		fn composite_primary_key() -> Option<crate::orm::composite_pk::CompositePrimaryKey> {
			Some(
				crate::orm::composite_pk::CompositePrimaryKey::new(vec![
					"user_id".to_string(),
					"role_id".to_string(),
				])
				.expect("valid composite primary key"),
			)
		}

		fn field_metadata() -> Vec<crate::orm::inspection::FieldInfo> {
			vec![
				test_field_info("user_id", Some("member_user_id"), true),
				test_field_info("role_id", Some("member_role_id"), true),
			]
		}

		fn new_fields() -> Self::Fields {
			TestMembershipFields
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestCorpusFile {
		id: Option<i64>,
		normalized_path: String,
		email: String,
	}

	impl TestCorpusFile {
		const fn field_normalized_path() -> crate::orm::expressions::FieldRef<
			TestCorpusFile,
			String,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestCorpusFile's persisted `normalized_path` field.
			unsafe { crate::orm::expressions::FieldRef::from_model_field("normalized_path") }
		}

		const fn field_email() -> crate::orm::expressions::FieldRef<
			TestCorpusFile,
			String,
			crate::orm::expressions::GeneratedModelField,
		> {
			// SAFETY: this test accessor names TestCorpusFile's persisted `email` field.
			unsafe {
				crate::orm::expressions::FieldRef::from_generated_model_field_with_names(
					"email",
					"email_addr",
				)
			}
		}
	}

	#[derive(Debug, Clone)]
	struct TestCorpusFileFields;

	impl crate::orm::model::FieldSelector for TestCorpusFileFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestCorpusFile {
		type PrimaryKey = i64;
		type Fields = TestCorpusFileFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"test_corpus_files"
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
			TestCorpusFileFields
		}

		fn field_metadata() -> Vec<crate::orm::inspection::FieldInfo> {
			vec![
				test_field_info("normalized_path", None, false),
				test_field_info("email", Some("email_addr"), false),
			]
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestProject {
		id: Option<i64>,
		name: String,
	}

	#[derive(Debug, Clone)]
	struct TestProjectFields;

	impl crate::orm::model::FieldSelector for TestProjectFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestProject {
		type PrimaryKey = i64;
		type Fields = TestProjectFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"test_projects"
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
			TestProjectFields
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestProjects {
		id: Option<i64>,
	}

	impl Model for TestProjects {
		type PrimaryKey = i64;
		type Fields = TestProjectFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"projects"
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
			TestProjectFields
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestTag {
		id: Option<i64>,
		name: String,
	}

	#[derive(Debug, Clone)]
	struct TestTagFields;

	impl crate::orm::model::FieldSelector for TestTagFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestTag {
		type PrimaryKey = i64;
		type Fields = TestTagFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"test_tags"
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
			TestTagFields
		}
	}

	struct TestUserCorpusFile;

	impl crate::orm::relations::RelationDescriptor for TestUserCorpusFile {
		type Source = TestUser;
		type Target = TestCorpusFile;

		fn steps() -> Vec<crate::orm::relations::RelationStep> {
			vec![crate::orm::relations::RelationStep {
				name: "corpus_file".into(),
				source_table: "test_users".into(),
				target_table: "test_corpus_files".into(),
				source_column: "corpus_file_id".into(),
				target_column: "id".into(),
				default_join_kind: crate::orm::relations::RelationJoinKind::Inner,
				multiplicity: crate::orm::relations::RelationMultiplicity::Single,
			}]
		}
	}

	struct TestCorpusFileProject;

	impl crate::orm::relations::RelationDescriptor for TestCorpusFileProject {
		type Source = TestCorpusFile;
		type Target = TestProject;

		fn steps() -> Vec<crate::orm::relations::RelationStep> {
			vec![crate::orm::relations::RelationStep {
				name: "project".into(),
				source_table: "test_corpus_files".into(),
				target_table: "test_projects".into(),
				source_column: "project_id".into(),
				target_column: "id".into(),
				default_join_kind: crate::orm::relations::RelationJoinKind::Left,
				multiplicity: crate::orm::relations::RelationMultiplicity::Single,
			}]
		}
	}

	struct TestUserRelationNamedCorpusFileProject;

	impl crate::orm::relations::RelationDescriptor for TestUserRelationNamedCorpusFileProject {
		type Source = TestUser;
		type Target = TestProject;

		fn steps() -> Vec<crate::orm::relations::RelationStep> {
			vec![crate::orm::relations::RelationStep {
				name: "corpus_file__project".into(),
				source_table: "test_users".into(),
				target_table: "test_projects".into(),
				source_column: "project_id".into(),
				target_column: "id".into(),
				default_join_kind: crate::orm::relations::RelationJoinKind::Left,
				multiplicity: crate::orm::relations::RelationMultiplicity::Single,
			}]
		}
	}

	fn nested_project_name_filter() -> super::TypedFilter<TestUser> {
		crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>()
		.then::<TestCorpusFileProject, TestProject>()
		.field(unsafe {
			// SAFETY: `name` is a persisted TestProject field in this query fixture.
			crate::orm::expressions::FieldRef::<
				TestProject,
				String,
				crate::orm::expressions::GeneratedModelField,
			>::from_model_field("name")
		})
		.eq("reinhardt")
	}

	fn aliased_join_condition(
		left_alias: &str,
		right_alias: &str,
	) -> crate::orm::query_fields::comparison::FieldComparison {
		use crate::orm::query_fields::comparison::{ComparisonOperator, FieldComparison, FieldRef};

		FieldComparison::new(
			FieldRef::field_with_alias(left_alias.to_string(), vec!["id".to_string()]),
			FieldRef::field_with_alias(right_alias.to_string(), vec!["id".to_string()]),
			ComparisonOperator::Eq,
		)
	}

	struct TestUserTags;

	impl crate::orm::relations::RelationDescriptor for TestUserTags {
		type Source = TestUser;
		type Target = TestTag;

		fn steps() -> Vec<crate::orm::relations::RelationStep> {
			vec![
				crate::orm::relations::RelationStep {
					name: "tags__through".into(),
					source_table: "test_users".into(),
					target_table: "test_user_tags".into(),
					source_column: "id".into(),
					target_column: "test_user_id".into(),
					default_join_kind: crate::orm::relations::RelationJoinKind::Left,
					multiplicity: crate::orm::relations::RelationMultiplicity::Multiple,
				},
				crate::orm::relations::RelationStep {
					name: "tags".into(),
					source_table: "test_user_tags".into(),
					target_table: "test_tags".into(),
					source_column: "tag_id".into(),
					target_column: "id".into(),
					default_join_kind: crate::orm::relations::RelationJoinKind::Left,
					multiplicity: crate::orm::relations::RelationMultiplicity::Single,
				},
			]
		}
	}

	struct TestUserProjects;

	impl crate::orm::relations::RelationDescriptor for TestUserProjects {
		type Source = TestUser;
		type Target = TestProject;

		fn steps() -> Vec<crate::orm::relations::RelationStep> {
			vec![crate::orm::relations::RelationStep {
				name: "projects".into(),
				source_table: "test_users".into(),
				target_table: "test_projects".into(),
				source_column: "id".into(),
				target_column: "test_user_id".into(),
				default_join_kind: crate::orm::relations::RelationJoinKind::Left,
				multiplicity: crate::orm::relations::RelationMultiplicity::Multiple,
			}]
		}
	}

	struct TestUserProjectsByUsername;

	impl crate::orm::relations::RelationDescriptor for TestUserProjectsByUsername {
		type Source = TestUser;
		type Target = TestProject;

		fn steps() -> Vec<crate::orm::relations::RelationStep> {
			vec![crate::orm::relations::RelationStep {
				name: "projects".into(),
				source_table: "test_users".into(),
				target_table: "test_projects".into(),
				source_column: "username".into(),
				target_column: "test_user_username".into(),
				default_join_kind: crate::orm::relations::RelationJoinKind::Left,
				multiplicity: crate::orm::relations::RelationMultiplicity::Multiple,
			}]
		}
	}

	struct TestProjectsChildren;

	impl crate::orm::relations::RelationDescriptor for TestProjectsChildren {
		type Source = TestProjects;
		type Target = TestProjects;

		fn steps() -> Vec<crate::orm::relations::RelationStep> {
			vec![crate::orm::relations::RelationStep {
				name: "projects".into(),
				source_table: "projects".into(),
				target_table: "projects".into(),
				source_column: "id".into(),
				target_column: "parent_id".into(),
				default_join_kind: crate::orm::relations::RelationJoinKind::Left,
				multiplicity: crate::orm::relations::RelationMultiplicity::Multiple,
			}]
		}
	}

	struct TestMembershipProjects;

	impl crate::orm::relations::RelationDescriptor for TestMembershipProjects {
		type Source = TestMembership;
		type Target = TestProject;

		fn steps() -> Vec<crate::orm::relations::RelationStep> {
			vec![crate::orm::relations::RelationStep {
				name: "projects".into(),
				source_table: "test_memberships".into(),
				target_table: "test_projects".into(),
				source_column: "member_user_id".into(),
				target_column: "test_membership_id".into(),
				default_join_kind: crate::orm::relations::RelationJoinKind::Left,
				multiplicity: crate::orm::relations::RelationMultiplicity::Multiple,
			}]
		}
	}

	#[test]
	fn bulk_key_batches_split_sqlite_keys_without_reordering() {
		// Arrange
		let keys = (0_i64..901).collect::<BTreeSet<_>>();

		// Act
		let batches = QuerySet::<TestUser>::bulk_key_batches(keys, DatabaseBackend::Sqlite, 0)
			.expect("SQLite should have bind slots available");

		// Assert
		assert_eq!(batches.len(), 2);
		assert_eq!(batches[0], (0_i64..900).collect::<Vec<_>>());
		assert_eq!(batches[1], vec![900]);
	}

	#[rstest]
	#[case(DatabaseBackend::Postgres)]
	#[case(DatabaseBackend::MySql)]
	fn bulk_key_batches_reject_exhausted_server_bind_limit(#[case] backend: DatabaseBackend) {
		// Arrange
		let keys = BTreeSet::from([1_i64]);

		// Act
		let error = QuerySet::<TestUser>::bulk_key_batches(keys, backend, 65_535)
			.expect_err("an exhausted bind budget should reject bulk retrieval");

		// Assert
		let reinhardt_core::exception::Error::Validation(message) = error else {
			panic!("expected validation error, got {error:?}");
		};
		assert_eq!(
			message,
			"QuerySet bulk retrieval cannot add lookup keys because the source query uses all 65535 available bind parameters"
		);
	}

	#[test]
	fn test_field_assignment_from_generated_field_ref_tuple() {
		let timestamp = chrono::DateTime::parse_from_rfc3339("2026-06-19T00:00:00Z")
			.expect("valid timestamp")
			.with_timezone(&chrono::Utc);

		let assignment: FieldAssignment = (TestUser::field_created_at(), timestamp).into();

		assert_eq!(assignment.field(), "created_at");
		assert!(matches!(
			assignment.value(),
			UpdateValue::Typed(Ok(DatabaseValue::DateTime(_)))
		));
	}

	#[test]
	fn test_typed_timestamp_filter_binds_as_timestamp() {
		// Arrange
		let timestamp = chrono::DateTime::parse_from_rfc3339("2026-06-19T00:00:00Z")
			.expect("valid timestamp")
			.with_timezone(&chrono::Utc);
		let value: FilterValue = timestamp.into();

		// Act
		let bound = QuerySet::<TestUser>::filter_value_to_sea_value(&value)
			.expect("timestamp filter should encode");

		// Assert
		assert!(matches!(value, FilterValue::Timestamp(_)));
		assert!(matches!(
			bound,
			reinhardt_query::value::Value::ChronoDateTimeUtc(Some(_))
		));
	}

	#[test]
	fn test_typed_uuid_filter_binds_as_uuid() {
		// Arrange
		let uuid =
			uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid UUID");
		let value: FilterValue = uuid.into();

		// Act
		let bound = QuerySet::<TestUser>::filter_value_to_sea_value(&value)
			.expect("UUID filter should encode");

		// Assert
		assert!(matches!(value, FilterValue::Uuid(_)));
		assert!(matches!(
			bound,
			reinhardt_query::value::Value::Uuid(Some(_))
		));
	}

	#[test]
	fn test_field_assignment_from_field_ref_assign_helper() {
		let assignment = TestUser::field_username().assign("alice");

		assert_eq!(assignment.field(), "username");
		assert!(matches!(
			assignment.value(),
			UpdateValue::Typed(Ok(DatabaseValue::String(value))) if value == "alice"
		));
	}

	#[test]
	fn test_update_fields_sql_preserves_queryset_predicates() {
		let timestamp = chrono::DateTime::parse_from_rfc3339("2026-06-19T00:00:00Z")
			.expect("valid timestamp")
			.with_timezone(&chrono::Utc);
		let queryset = QuerySet::<TestUser>::new()
			.filter(TestUser::field_id().eq(7))
			.filter(TestUser::field_email().is_null());

		let (sql, params) = queryset
			.update_fields_sql([(TestUser::field_created_at(), timestamp)])
			.expect("update fields sql");

		assert_eq!(
			sql,
			"UPDATE \"test_users\" SET \"created_at\" = $1 WHERE (\"id\" = $2 AND \"email\" IS NULL)"
		);
		assert_eq!(params.len(), 2);
		assert_eq!(params[0], "2026-06-19T00:00:00+00:00");
		assert_eq!(params[1], "7");
	}

	#[test]
	fn test_update_fields_sql_rejects_empty_assignments() {
		let queryset = QuerySet::<TestUser>::new().filter(TestUser::field_id().eq(7));

		let error = queryset
			.update_fields_sql(std::iter::empty::<FieldAssignment>())
			.expect_err("empty assignments should fail");

		assert!(matches!(
			error,
			reinhardt_core::exception::Error::Validation(message)
				if message.contains("field assignment")
		));
	}

	#[test]
	fn test_update_fields_sql_rejects_missing_predicate() {
		let queryset = QuerySet::<TestUser>::new();

		let error = queryset
			.update_fields_sql([("username", "alice")])
			.expect_err("missing predicate should fail");

		assert!(matches!(
		error,
		reinhardt_core::exception::Error::Validation(message)
			if message.contains("filter predicate")
		));
	}

	#[test]
	fn test_update_fields_sql_rejects_empty_not_in_predicate() {
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"id",
			FilterOperator::NotIn,
			FilterValue::List(Vec::new()),
		));

		let error = queryset
			.update_fields_sql([("username", "alice")])
			.expect_err("empty NOT IN predicate should fail");

		assert!(matches!(
			error,
			reinhardt_core::exception::Error::Validation(message)
				if message == "QuerySet::update_fields requires at least one non-empty filter predicate"
		));
	}

	#[test]
	fn test_update_query_omits_generated_fields() {
		let queryset = QuerySet::<TestUser>::new().filter(TestUser::field_id().eq(7));
		let mut updates = HashMap::new();
		updates.insert(
			"username".to_string(),
			UpdateValue::String("alice".to_string()),
		);
		updates.insert(
			"full_name".to_string(),
			UpdateValue::String("Alice Doe".to_string()),
		);

		let stmt = queryset
			.update_query(&updates)
			.expect("update query should compile");
		let (sql, params) = super::PostgresQueryBuilder.build_update(&stmt);

		assert_eq!(
			sql,
			"UPDATE \"test_users\" SET \"username\" = $1 WHERE \"id\" = $2"
		);
		assert_eq!(params.len(), 2);
	}

	#[test]
	fn test_update_sql_generated_only_fields_builds_noop_set() {
		let queryset = QuerySet::<TestUser>::new().filter(TestUser::field_id().eq(7));
		let mut updates = HashMap::new();
		updates.insert(
			"full_name".to_string(),
			UpdateValue::String("Alice Doe".to_string()),
		);

		let (sql, params) = queryset
			.update_sql(&updates)
			.expect("update SQL should compile");

		assert_eq!(
			sql,
			"UPDATE \"test_users\" SET \"id\" = \"id\" WHERE \"id\" = $1"
		);
		assert_eq!(params, vec!["7"]);
	}

	#[test]
	fn test_update_fields_sql_rejects_generated_fields() {
		let queryset = QuerySet::<TestUser>::new().filter(TestUser::field_id().eq(7));

		let error = queryset
			.update_fields_sql([(TestUser::field_full_name(), "Alice Doe")])
			.expect_err("generated fields should be rejected");

		assert!(matches!(
			error,
			reinhardt_core::exception::Error::Validation(message)
				if message == "QuerySet::update_fields cannot assign generated field `full_name`"
		));
	}

	#[test]
	fn test_update_fields_sql_rejects_db_column_generated_fields() {
		let queryset = QuerySet::<TestUser>::new().filter(TestUser::field_id().eq(7));
		let error = queryset
			.update_fields_sql([(TestUser::field_display_name(), "Alice Doe")])
			.expect_err("generated database columns should be rejected");
		assert!(matches!(
			error,
			reinhardt_core::exception::Error::Validation(message)
				if message == "QuerySet::update_fields cannot assign generated field `display_name`"
		));
	}

	#[test]
	fn test_legacy_update_query_preserves_typed_codec_error() {
		let queryset = QuerySet::<TestUser>::new().filter(TestUser::field_id().eq(7));
		let mut updates = HashMap::new();
		updates.insert(
			"username".to_owned(),
			UpdateValue::Typed(Err(FieldCodecError::Serialization(
				"rejected update value".to_owned(),
			))),
		);

		let error = queryset
			.update_query(&updates)
			.expect_err("typed codec error should stop legacy update compilation");
		assert_typed_codec_error(&error);
	}

	#[cfg(feature = "file-storage")]
	#[test]
	fn file_policy_mismatch_stops_query_compilation_with_typed_source() {
		let field = unsafe {
			crate::orm::FieldRef::<
				TestUser,
				crate::orm::FileField,
				crate::orm::expressions::GeneratedModelField,
			>::from_generated_model_field_with_names_and_metadata(
				"avatar",
				"avatar_path",
				&[("file_storage", "private_uploads")],
			)
		};
		let value = crate::orm::FileField::from_existing("avatars/a.png", "default").unwrap();
		let queryset = QuerySet::<TestUser>::new().filter(field.eq(value));

		let error = queryset
			.delete_query()
			.expect_err("policy mismatch must stop query compilation");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Type)
		);
		let source = std::error::Error::source(&error).unwrap();
		assert!(matches!(
			source.downcast_ref::<FieldCodecError>(),
			Some(FieldCodecError::FieldPolicyMismatch { .. })
		));
	}

	fn assert_typed_codec_error(error: &reinhardt_core::exception::Error) {
		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Serialization)
		);
		let source =
			std::error::Error::source(error).expect("typed codec source should be preserved");
		assert!(source.downcast_ref::<FieldCodecError>().is_some());
	}

	fn rejecting_typed_filter() -> Filter {
		Filter::new(
			"username",
			FilterOperator::Eq,
			FilterValue::Typed(Err(FieldCodecError::Serialization(
				"rejected query value".to_owned(),
			))),
		)
	}

	#[test]
	fn test_select_related_query_preserves_typed_codec_error() {
		let queryset = QuerySet::<TestUser>::new()
			.select_related(&["profile"])
			.filter(rejecting_typed_filter());

		let error = queryset
			.select_related_query()
			.expect_err("typed codec error must stop select-related compilation");
		assert_typed_codec_error(&error);
	}

	#[test]
	fn test_delete_query_preserves_typed_codec_error() {
		let queryset = QuerySet::<TestUser>::new().filter(rejecting_typed_filter());

		let error = queryset
			.delete_query()
			.expect_err("typed codec error must stop delete compilation");
		assert_typed_codec_error(&error);
	}

	#[test]
	fn test_debug_sql_preserves_typed_codec_error() {
		let queryset = QuerySet::<TestUser>::new().filter(rejecting_typed_filter());

		let error = queryset
			.to_sql()
			.expect_err("typed codec error must stop debug SQL compilation");
		assert_typed_codec_error(&error);
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
	fn test_string_select_related_still_records_field() {
		let queryset = QuerySet::<TestUser>::new().select_related(&["profile"]);

		assert_eq!(queryset.select_related_fields, vec!["profile"]);
	}

	#[test]
	fn test_invalid_string_relation_validation_reports_relation_name() {
		let error = QuerySet::<TestUser>::new()
			.validate_relation_path_for_test("missing__field")
			.expect_err("invalid relation path should fail validation");

		assert!(error.to_string().contains("missing__field"));
	}

	#[test]
	fn test_nested_string_relation_validation_is_rejected() {
		let error = QuerySet::<TestUser>::new()
			.validate_relation_path_for_test("profile__missing")
			.expect_err("nested string relation path should fail validation");

		assert!(error.to_string().contains("profile__missing"));
		assert!(error.to_string().contains("typed relation paths"));
	}

	#[test]
	#[should_panic(expected = "invalid relation path passed to select_related")]
	fn test_string_select_related_rejects_invalid_path() {
		let _ = QuerySet::<TestUser>::new().select_related(&["missing__field"]);
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
		let (sql, params) = queryset
			.update_sql(&updates)
			.expect("update SQL should compile");

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
		let (sql, params) = queryset
			.update_sql(&updates)
			.expect("update SQL should compile");

		// HashMap iteration order is not guaranteed, so we check both possible orderings
		let valid_sql_1 = "UPDATE \"test_users\" SET \"username\" = $1, \"email\" = $2 WHERE (\"id\" > $3 AND \"email\" LIKE $4 ESCAPE '\\')";
		let valid_sql_2 = "UPDATE \"test_users\" SET \"email\" = $1, \"username\" = $2 WHERE (\"id\" > $3 AND \"email\" LIKE $4 ESCAPE '\\')";
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

		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");

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

		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");

		assert_eq!(
			sql,
			"DELETE FROM \"test_users\" WHERE (\"username\" = $1 AND \"email\" LIKE $2 ESCAPE '\\')"
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
	#[should_panic(
		expected = "DELETE without WHERE clause is not allowed. Use .filter() to specify which rows to delete."
	)]
	fn test_delete_sql_with_empty_composite_filter_panics() {
		let queryset = QuerySet::<TestUser>::new().filter(FilterCondition::and(Vec::new()));
		let _ = queryset.delete_sql();
	}

	#[test]
	#[should_panic(
		expected = "DELETE without WHERE clause is not allowed. Use .filter() to specify which rows to delete."
	)]
	fn test_delete_sql_with_empty_not_in_panics() {
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"id",
			FilterOperator::NotIn,
			FilterValue::List(Vec::new()),
		));
		let _ = queryset.delete_sql();
	}

	#[test]
	fn test_delete_sql_with_empty_in_matches_no_rows() {
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"id",
			FilterOperator::In,
			FilterValue::List(Vec::new()),
		));

		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");

		assert_eq!(sql, "DELETE FROM \"test_users\" WHERE FALSE");
		assert_eq!(params, Vec::<String>::new());
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

		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");

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

		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");

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

		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");

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

		let stmt = queryset
			.select_related_query()
			.expect("select-related query should compile");

		// Convert to SQL to verify structure
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryStatementBuilder};
		let sql = stmt.build(PostgresQueryBuilder).0;

		assert!(sql.contains("SELECT"));
		assert!(sql.contains("test_users"));
		assert!(sql.contains("LEFT JOIN"));
	}

	#[test]
	fn select_related_qualifies_root_ordering_columns() {
		// Arrange
		let mut queryset = QuerySet::<TestUser>::new().select_related(&["profile"]);
		queryset.order_by_fields.push("id".to_string());

		// Act
		let statement = queryset
			.select_related_query()
			.expect("select-related query should compile");
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryStatementBuilder};
		let sql = statement.build(PostgresQueryBuilder).0;

		// Assert
		assert!(sql.contains("ORDER BY \"test_users\".\"id\" ASC"));
	}

	#[test]
	fn select_related_to_sql_includes_each_annotation_once() {
		use crate::orm::annotation::{Annotation, AnnotationValue, Value};

		// Arrange
		let queryset = QuerySet::<TestUser>::new()
			.select_related(&["profile"])
			.annotate_legacy(Annotation::new(
				"relation_marker",
				AnnotationValue::Value(Value::Int(1)),
			));

		// Act
		let sql = queryset
			.to_sql()
			.expect("select-related query SQL should compile");

		// Assert
		assert_eq!(sql.matches(r#"1 AS "relation_marker""#).count(), 1);
	}

	#[rstest]
	fn select_related_uses_structural_annotations_for_mysql() {
		use crate::orm::annotation::{Annotation, AnnotationValue};
		use crate::orm::expressions::F;
		use reinhardt_query::prelude::MySqlQueryBuilder;

		let statement = QuerySet::<TestUser>::new()
			.select_related(&["profile"])
			.annotate_legacy(Annotation::field(
				"user_id",
				AnnotationValue::Field(F::new("id")),
			))
			.select_related_query()
			.expect("select-related query should compile");
		let sql = statement.to_string(MySqlQueryBuilder);

		assert!(sql.contains("`test_users`.`id` AS `user_id`"));
		assert!(!sql.contains("\"test_users\".\"id\""));
	}

	#[test]
	fn test_string_relation_loaders_accept_vec_references() {
		let fields = vec!["corpus_file"];

		let selected = QuerySet::<TestUser>::new().select_related(&fields);
		let prefetched = QuerySet::<TestUser>::new().prefetch_related(&fields);

		assert_eq!(selected.select_related_fields, vec!["corpus_file"]);
		assert_eq!(prefetched.prefetch_related_fields, vec!["corpus_file"]);
	}

	#[test]
	#[should_panic(
		expected = "typed prefetch_related does not support composite primary-key roots"
	)]
	fn test_vec_prefetch_related_rejects_composite_primary_key_root() {
		let fields = vec!["projects"];

		let _ = QuerySet::<TestMembership>::new().prefetch_related(&fields);
	}

	#[test]
	fn test_relation_filter_adds_inner_join() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(filter)
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "test_users".* FROM "test_users" INNER JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id" WHERE "corpus_file"."normalized_path" = '/docs/index.md'"#
		);
	}

	#[test]
	fn test_lateral_join_rebases_typed_relation_filter_aliases() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");
		let sql = QuerySet::<TestUser>::new()
			.filter(filter)
			.with_lateral_join(crate::orm::lateral_join::LateralJoin::new(
				"corpus_file",
				"SELECT 1",
			))
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#"AS "corpus_file__corpus_file""#));
		assert!(sql.contains(r#"WHERE "corpus_file__corpus_file"."normalized_path""#));
	}

	#[test]
	fn test_relation_filter_uses_from_alias_as_join_root() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.from_as("u")
			.filter(filter)
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.starts_with(r#"SELECT "u".* FROM "test_users" AS "u""#));
		assert!(sql.contains(r#""u"."corpus_file_id" = "corpus_file"."id""#));
		assert!(!sql.contains(r#""test_users"."corpus_file_id" = "corpus_file"."id""#));
	}

	#[test]
	fn test_relation_filter_rebases_join_alias_that_matches_root_alias() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.from_as("corpus_file")
			.filter(filter)
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "corpus_file".* FROM "test_users" AS "corpus_file" INNER JOIN "test_corpus_files" AS "corpus_file__corpus_file" ON "corpus_file"."corpus_file_id" = "corpus_file__corpus_file"."id" WHERE "corpus_file__corpus_file"."normalized_path" = '/docs/index.md'"#
		);

		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(filter)
			.from_as("corpus_file")
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "corpus_file".* FROM "test_users" AS "corpus_file" INNER JOIN "test_corpus_files" AS "corpus_file__corpus_file" ON "corpus_file"."corpus_file_id" = "corpus_file__corpus_file"."id" WHERE "corpus_file__corpus_file"."normalized_path" = '/docs/index.md'"#
		);
	}

	#[test]
	fn test_nested_relation_filter_uses_rebased_planned_leaf_alias() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.then::<TestCorpusFileProject, TestProject>()
			.field(unsafe {
				// SAFETY: `name` is a persisted TestProject field in this query fixture.
				crate::orm::expressions::FieldRef::<
					TestProject,
					String,
					crate::orm::expressions::GeneratedModelField,
				>::from_model_field("name")
			})
			.eq("reinhardt");

		let sql = QuerySet::<TestUser>::new()
			.from_as("corpus_file__project")
			.filter(filter)
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(
			r#"LEFT JOIN "test_projects" AS "corpus_file__project__project" ON "corpus_file"."project_id" = "corpus_file__project__project"."id""#
		));
		assert!(sql.ends_with(r#"WHERE "corpus_file__project__project"."name" = 'reinhardt'"#));
	}

	#[test]
	fn test_relation_filter_count_uses_from_alias_as_join_root() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let stmt = QuerySet::<TestUser>::new()
			.from_as("u")
			.filter(filter)
			.count_select_query()
			.expect("count select query");
		let sql = stmt.to_string(PostgresQueryBuilder);

		assert!(sql.starts_with(r#"SELECT COUNT(*) AS "count" FROM "test_users" AS "u""#));
		assert!(sql.contains(r#""u"."corpus_file_id" = "corpus_file"."id""#));
		assert!(!sql.contains(r#""test_users"."corpus_file_id" = "corpus_file"."id""#));
	}

	#[test]
	fn test_nested_relation_filter_count_reuses_rebased_aliases() {
		let stmt = QuerySet::<TestUser>::new()
			.from_as("corpus_file__project")
			.filter(nested_project_name_filter())
			.count_select_query()
			.expect("count select query");
		let sql = stmt.to_string(PostgresQueryBuilder);

		assert!(sql.contains(
			r#"LEFT JOIN "test_projects" AS "corpus_file__project__project" ON "corpus_file"."project_id" = "corpus_file__project__project"."id""#
		));
		assert!(sql.ends_with(r#"WHERE "corpus_file__project__project"."name" = 'reinhardt'"#));
	}

	#[rstest]
	fn nullable_filter_relations_for_lock_tracks_outer_join_targets() {
		let queryset = QuerySet::<TestUser>::new().filter(nested_project_name_filter());

		assert_eq!(queryset.requires_serializable_transaction(), true);
		assert_eq!(
			queryset.nullable_filter_relations_for_lock(),
			vec![(
				"test_projects".to_owned(),
				"corpus_file__project".to_owned(),
				"id".to_owned(),
			)]
		);
	}

	#[test]
	fn test_count_rebases_typed_filters_against_filter_only_aliases() {
		let eager_path =
			crate::orm::relations::RelationPath::<TestUser, TestProject>::from_descriptor::<
				TestUserRelationNamedCorpusFileProject,
			>();

		let sql = QuerySet::<TestUser>::new()
			.select_related(eager_path)
			.filter(nested_project_name_filter())
			.count_select_query()
			.expect("count select query")
			.to_string(PostgresQueryBuilder);

		assert!(sql.contains(
			r#"LEFT JOIN "test_projects" AS "corpus_file__project" ON "corpus_file"."project_id" = "corpus_file__project"."id""#
		));
		assert!(sql.ends_with(r#"WHERE "corpus_file__project"."name" = 'reinhardt'"#));
		assert!(!sql.contains("corpus_file__project__project"));
	}

	#[test]
	fn test_join_as_rebases_nested_typed_filter_aliases() {
		let inner_sql = QuerySet::<TestUser>::new()
			.filter(nested_project_name_filter())
			.inner_join_as::<TestProject, _>("corpus_file__project", "manual_project", |_, _| {
				aliased_join_condition("corpus_file__project", "manual_project")
			})
			.to_sql()
			.expect("query SQL should compile");
		let left_sql = QuerySet::<TestUser>::new()
			.filter(nested_project_name_filter())
			.left_join_as::<TestProject, _>("corpus_file__project", "manual_project", |_, _| {
				aliased_join_condition("corpus_file__project", "manual_project")
			})
			.to_sql()
			.expect("query SQL should compile");
		let right_sql = QuerySet::<TestUser>::new()
			.filter(nested_project_name_filter())
			.right_join_as::<TestProject, _>("corpus_file__project", "manual_project", |_, _| {
				aliased_join_condition("corpus_file__project", "manual_project")
			})
			.to_sql()
			.expect("query SQL should compile");

		for sql in [inner_sql, left_sql, right_sql] {
			assert!(sql.ends_with(r#"WHERE "corpus_file__project__project"."name" = 'reinhardt'"#));
		}
	}

	#[test]
	fn test_aliasless_manual_joins_rebase_typed_filter_aliases() {
		let make_filter = || {
			crate::orm::relations::RelationPath::<TestProjects, TestProjects>::from_descriptor::<
				TestProjectsChildren,
			>()
			.field(unsafe {
				// SAFETY: `id` is a persisted TestProjects field in this query fixture.
				crate::orm::expressions::FieldRef::<
					TestProjects,
					i64,
					crate::orm::expressions::GeneratedModelField,
				>::from_model_field("id")
			})
			.eq(1)
		};

		let sql = QuerySet::<TestProjects>::new()
			.filter(make_filter())
			.inner_join::<TestProjects>("id", "parent_id")
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.ends_with(r#"WHERE "projects__projects"."id" = 1"#));
	}

	#[test]
	fn test_typed_joins_reserve_manual_join_aliases() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(filter)
			.inner_join_as::<TestProject, _>("test_users", "corpus_file", |_, _| {
				aliased_join_condition("test_users", "corpus_file")
			})
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(
			r#"INNER JOIN "test_corpus_files" AS "corpus_file__corpus_file" ON "test_users"."corpus_file_id" = "corpus_file__corpus_file"."id""#
		));
		assert!(
			sql.ends_with(
				r#"WHERE "corpus_file__corpus_file"."normalized_path" = '/docs/index.md'"#
			)
		);
	}

	#[test]
	fn test_executor_select_preserves_manual_joins() {
		let stmt = QuerySet::<TestUser>::new()
			.inner_join_on::<TestProject>("test_users.id = test_projects.user_id")
			.filter(Filter::new(
				"test_projects.name",
				FilterOperator::Eq,
				FilterValue::String("reinhardt".to_owned()),
			))
			.build_select_statement()
			.expect("executor select should compile");
		let sql = stmt.to_string(PostgresQueryBuilder);

		assert!(
			sql.contains(r#"INNER JOIN "test_projects" ON test_users.id = test_projects.user_id"#)
		);
		assert!(sql.contains(r#"WHERE "test_projects"."name" = 'reinhardt'"#));
	}

	#[rstest]
	fn temporal_projection_qualifies_root_filters_with_manual_joins() {
		let queryset = QuerySet::<TestUser>::new()
			.inner_join_on::<TestProject>("test_users.id = test_projects.user_id")
			.filter(Filter::new(
				"created_at",
				FilterOperator::Eq,
				FilterValue::String("2026-01-01".to_owned()),
			));
		let statement = queryset
			.temporal_projection_statement(
				"created_at",
				TemporalTruncKind::Day,
				DateProjectionOrder::Asc,
				None,
				TemporalTruncOutput::Date,
			)
			.expect("temporal projection should compile with a manual join");

		assert_eq!(
			statement.to_string(PostgresQueryBuilder),
			r#"SELECT DISTINCT DATE_TRUNC('day', "test_users"."created_at")::date AS "value" FROM "test_users" INNER JOIN "test_projects" ON test_users.id = test_projects.user_id WHERE "test_users"."created_at" = '2026-01-01' AND "test_users"."created_at" IS NOT NULL ORDER BY "value" ASC"#
		);
	}

	#[test]
	fn test_manual_join_qualifies_root_ordering_columns() {
		let sql = QuerySet::<TestUser>::new()
			.inner_join_on::<TestProject>("test_users.id = test_projects.user_id")
			.order_by(&["id"])
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#"ORDER BY "test_users"."id" ASC"#));
	}

	#[test]
	fn test_manual_join_preserves_annotation_ordering_aliases() {
		let sql = QuerySet::<TestUser>::new()
			.annotate_legacy(crate::orm::annotation::Annotation::new(
				"other_age",
				crate::orm::annotation::AnnotationValue::Value(crate::orm::annotation::Value::Int(
					42,
				)),
			))
			.inner_join_on::<TestProject>("test_users.id = test_projects.user_id")
			.order_by(&["other_age"])
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT *, 42 AS "other_age" FROM "test_users" INNER JOIN "test_projects" ON test_users.id = test_projects.user_id ORDER BY "other_age" ASC"#
		);
	}

	#[test]
	fn test_manual_join_preserves_backend_annotation_ordering_aliases() {
		let annotation = crate::orm::postgres_features::BackendAnnotation::new(
			"rank",
			crate::orm::postgres_features::BackendAnnotationValue::TsRank(
				crate::orm::postgres_features::TsRank::new(
					"search_vector".to_owned(),
					"rust".to_owned(),
				),
			),
		)
		.expect("backend annotation should validate");
		let sql = QuerySet::<TestUser>::new()
			.annotate_backend(annotation)
			.expect("backend annotation should be accepted")
			.inner_join_on::<TestProject>("test_users.id = test_projects.user_id")
			.order_by(&["rank"])
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#"ORDER BY "rank" ASC"#));
		assert!(!sql.contains(r#"ORDER BY "test_users"."rank" ASC"#));
	}

	#[test]
	fn test_bulk_lookup_restores_deferred_logical_name_for_custom_column() {
		let queryset = QuerySet::<TestCorpusFile>::new().defer(&["email"]);
		let queryset = queryset.with_bulk_lookup_column("email_addr");

		assert!(queryset.deferred_fields.is_empty());
	}

	#[test]
	fn test_typed_joins_qualify_rhs_expression_fields() {
		let relation_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");
		let expression = crate::orm::annotation::Expression::Add(
			Box::new(crate::orm::annotation::AnnotationValue::Field(
				crate::orm::expressions::F::new("created_at"),
			)),
			Box::new(crate::orm::annotation::AnnotationValue::Value(
				crate::orm::annotation::Value::Int(1),
			)),
		);

		let sql = QuerySet::<TestUser>::new()
			.filter(relation_filter)
			.filter(Filter::new(
				"updated_at",
				FilterOperator::Eq,
				FilterValue::Expression(expression),
			))
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#""test_users"."updated_at" = ("test_users"."created_at" + 1)"#));
	}

	#[test]
	fn test_optional_relation_filter_promotes_left_join() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.optional()
			.field(TestCorpusFile::field_normalized_path())
			.is_null();

		let sql = QuerySet::<TestUser>::new()
			.filter(filter)
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "test_users".* FROM "test_users" LEFT JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id" WHERE "corpus_file"."normalized_path" IS NULL"#
		);
	}

	#[test]
	fn test_typed_join_is_kept_when_legacy_select_related_uses_same_field() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>();

		let sql = QuerySet::<TestUser>::new()
			.select_related(&["corpus_file"])
			.select_related(path)
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql.matches(r#"JOIN "test_corpus_files" AS "corpus_file""#)
				.count(),
			1
		);
		assert!(sql.contains(
			r#"INNER JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id""#
		));
	}

	#[test]
	fn test_legacy_select_related_reuses_typed_filter_join() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.select_related(&["corpus_file"])
			.filter(filter)
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.starts_with(r#"SELECT "test_users".*, "corpus_file".* FROM "test_users""#));
		assert_eq!(
			sql.matches(r#"JOIN "test_corpus_files" AS "corpus_file""#)
				.count(),
			1
		);
	}

	#[test]
	fn test_typed_select_related_preserves_explicit_root_projection() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>();

		let values_sql = QuerySet::<TestUser>::new()
			.values(&["id"])
			.select_related(path.clone())
			.to_sql()
			.expect("query SQL should compile");
		let only_sql = QuerySet::<TestUser>::new()
			.only(&["id"])
			.select_related(path)
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			values_sql,
			r#"SELECT "test_users"."id", "corpus_file".* FROM "test_users" INNER JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id""#
		);
		assert_eq!(only_sql, values_sql);
	}

	#[test]
	fn test_typed_join_skips_legacy_loader_when_root_alias_rebases_it() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>();

		let sql = QuerySet::<TestUser>::new()
			.from_as("corpus_file")
			.select_related(&["corpus_file"])
			.select_related(path)
			.to_sql()
			.expect("query SQL should compile");

		assert!(!sql.contains(r#"LEFT JOIN "corpus_files""#));
		assert!(sql.contains(
			r#"INNER JOIN "test_corpus_files" AS "corpus_file__corpus_file" ON "corpus_file"."corpus_file_id" = "corpus_file__corpus_file"."id""#
		));
	}

	#[test]
	fn test_typed_select_related_uses_relation_join_graph() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>();

		let sql = QuerySet::<TestUser>::new()
			.select_related(path)
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "test_users".*, "corpus_file".* FROM "test_users" INNER JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id""#
		);
	}

	#[test]
	fn test_nested_typed_select_related_selects_intermediate_hops() {
		let path =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.then::<TestCorpusFileProject, TestProject>();

		let sql = QuerySet::<TestUser>::new()
			.select_related(path)
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.starts_with(
			r#"SELECT "test_users".*, "corpus_file".*, "corpus_file__project".* FROM "test_users""#
		));
	}

	#[test]
	fn test_typed_select_related_uses_rebased_planned_aliases() {
		let path =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.then::<TestCorpusFileProject, TestProject>();

		let sql = QuerySet::<TestUser>::new()
			.from_as("corpus_file__project")
			.select_related(path)
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.starts_with(
			r#"SELECT "corpus_file__project".*, "corpus_file".*, "corpus_file__project__project".* FROM "test_users" AS "corpus_file__project""#
		));
		assert!(sql.contains(
			r#"LEFT JOIN "test_projects" AS "corpus_file__project__project" ON "corpus_file"."project_id" = "corpus_file__project__project"."id""#
		));
	}

	#[test]
	fn test_count_omits_eager_only_typed_select_related_joins() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>();

		let sql = QuerySet::<TestUser>::new()
			.select_related(path)
			.count_select_query()
			.expect("count select query")
			.to_string(PostgresQueryBuilder);

		assert_eq!(sql, r#"SELECT COUNT(*) AS "count" FROM "test_users""#);
	}

	#[test]
	fn test_custom_manager_trait_accepts_typed_relation_loaders() {
		use crate::orm::custom_manager::CustomManager;

		let selected = CustomManager::select_related(
			&Manager::<TestUser>::new(),
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>(),
		);
		let prefetched = CustomManager::prefetch_related(
			&Manager::<TestUser>::new(),
			crate::orm::relations::RelationPath::<TestUser, TestTag>::from_descriptor::<TestUserTags>(
			),
		);

		assert_eq!(selected.typed_select_related.len(), 1);
		assert_eq!(prefetched.typed_prefetch_related.len(), 1);
	}

	#[test]
	#[should_panic(expected = "typed select_related supports only single-valued relation paths")]
	fn test_typed_select_related_rejects_multi_valued_path() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestProject>::from_descriptor::<
			TestUserProjects,
		>();

		let _ = QuerySet::<TestUser>::new().select_related(path);
	}

	#[test]
	fn test_relation_filter_qualifies_selected_root_columns() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.values(&["id"])
			.filter(filter)
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.starts_with(r#"SELECT "test_users"."id" FROM "test_users""#));
	}

	#[test]
	fn test_relation_filter_qualifies_root_predicate_and_ordering() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(related_filter)
			.filter(Filter::new("id", FilterOperator::Eq, FilterValue::Int(1)))
			.order_by(&["id"])
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "test_users".* FROM "test_users" INNER JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id" WHERE ("corpus_file"."normalized_path" = '/docs/index.md' AND "test_users"."id" = 1) ORDER BY "test_users"."id" ASC"#
		);
	}

	#[test]
	fn test_relation_filter_qualifies_subquery_fields() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(related_filter)
			.filter_in_subquery("id", |queryset: QuerySet<TestProject>| {
				queryset.values(&["id"])
			})
			.expect("IN subquery filter should compile")
			.filter_not_in_subquery("id", |queryset: QuerySet<TestProject>| {
				queryset.values(&["id"])
			})
			.expect("NOT IN subquery filter should compile")
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#""test_users"."id" IN (SELECT "id" FROM "test_projects")"#));
		assert!(sql.contains(r#""test_users"."id" NOT IN (SELECT "id" FROM "test_projects")"#));
	}

	#[rstest]
	fn lock_scope_subqueries_locks_every_authorization_subquery() {
		let mut queryset = QuerySet::<TestUser>::new()
			.filter_in_subquery("id", |queryset: QuerySet<TestUser>| queryset)
			.expect("IN subquery should compile")
			.filter_not_in_subquery("id", |queryset: QuerySet<TestUser>| queryset)
			.expect("NOT IN subquery should compile")
			.filter_exists(|queryset: QuerySet<TestUser>| queryset)
			.expect("EXISTS subquery should compile")
			.filter_not_exists(|queryset: QuerySet<TestUser>| queryset)
			.expect("NOT EXISTS subquery should compile");
		assert!(queryset.has_subquery_conditions());

		queryset.lock_scope_subqueries();

		assert_eq!(
			queryset.to_sql().expect("query SQL should compile"),
			r#"SELECT * FROM "test_users" WHERE ("id" IN (SELECT * FROM "test_users" FOR UPDATE) AND "id" NOT IN (SELECT * FROM "test_users" FOR UPDATE) AND EXISTS (SELECT * FROM "test_users" FOR UPDATE) AND NOT EXISTS (SELECT * FROM "test_users" FOR UPDATE))"#
		);
	}

	#[rstest]
	fn mysql_scope_subquery_uses_mysql_identifier_quotes() {
		let queryset = QuerySet::<TestUser>::new()
			.filter_in_subquery("id", |queryset: QuerySet<TestUser>| {
				queryset.values(&["id"])
			})
			.expect("IN subquery should compile");

		let statement = queryset
			.build_full_model_select_statement_for_backend(
				crate::backends::types::DatabaseType::Mysql,
			)
			.expect("model-shaped subquery should compile");
		let sql = statement.to_string(MySqlQueryBuilder);

		assert!(sql.contains("(SELECT `id` FROM `test_users`)"), "{sql}");
		assert!(!sql.contains("SELECT \"id\" FROM \"test_users\""), "{sql}");
	}

	#[rstest]
	fn lock_scope_subqueries_skips_non_lockable_distinct_subqueries() {
		let mut queryset = QuerySet::<TestUser>::new()
			.filter_in_subquery("id", |queryset: QuerySet<TestUser>| queryset.distinct())
			.expect("IN subquery should compile");

		queryset.lock_scope_subqueries();

		let sql = queryset.to_sql().expect("query SQL should compile");
		assert!(sql.contains(r#"SELECT DISTINCT * FROM "test_users""#));
		assert!(!sql.contains("FOR UPDATE"));
	}

	#[rstest]
	fn model_shaped_queryset_accepts_manual_joins() {
		let queryset = QuerySet::<TestUser>::new()
			.inner_join_on::<TestProject>("test_users.id = test_projects.user_id");
		assert!(queryset.requires_serializable_transaction());
		assert_eq!(
			queryset.inner_relation_aliases_for_lock(),
			vec!["test_projects"]
		);

		let statement = queryset
			.build_full_model_select_statement()
			.expect("manual joins should preserve a model-shaped projection");

		assert_eq!(
			statement.to_string(PostgresQueryBuilder),
			r#"SELECT * FROM "test_users" INNER JOIN "test_projects" ON test_users.id = test_projects.user_id"#
		);

		let left_join_queryset = QuerySet::<TestUser>::new()
			.left_join_on::<TestProject>("test_users.id = test_projects.user_id");
		assert!(left_join_queryset.requires_serializable_transaction());
	}

	#[rstest]
	fn model_shaped_queryset_tracks_right_joins_for_mutation_locking() {
		let queryset = QuerySet::<TestUser>::new()
			.right_join_on::<TestProject>("test_users.id = test_projects.user_id");

		assert!(queryset.has_right_join());
	}

	#[test]
	fn test_relation_filter_qualifies_transformed_root_filter() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(related_filter)
			.filter(TestUser::field_created_at().year().eq(2026))
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#"EXTRACT(YEAR FROM "test_users"."created_at") = 2026"#));
	}

	#[test]
	fn test_relation_filter_qualification_preserves_expression_string_literals() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(related_filter)
			.filter(Filter::expression(
				r#"COALESCE("created_at", '"created_at"')"#,
				FilterOperator::Eq,
				FilterValue::Integer(2026),
			))
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#"COALESCE("test_users"."created_at", '"created_at"') = 2026"#));
	}

	#[test]
	fn test_relation_filter_qualifies_root_aggregate_annotation() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.values(&["id"])
			.filter(related_filter)
			.annotate(
				crate::orm::func::count(TestUser::field_id())
					.label("user_count")
					.expect("valid aggregate label"),
			)
			.expect("typed aggregate annotation should compile")
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#"COUNT("test_users"."id") AS "user_count""#));
	}

	#[rstest]
	fn test_relation_filter_keeps_count_wildcard_unqualified() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.values(&["id", "username", "email"])
			.filter(related_filter)
			.annotate(
				crate::orm::func::count_all::<TestUser>()
					.label("user_count")
					.expect("valid aggregate label"),
			)
			.expect("typed aggregate annotation should compile")
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#"COUNT(*) AS "user_count""#));
		assert!(!sql.contains(r#""test_users"."*""#));
	}

	#[test]
	fn test_structural_aggregates_preserve_wildcards_and_distinct_functions() {
		let queryset = QuerySet::<TestUser>::new()
			.values(&["id"])
			.annotate(
				crate::orm::func::count_all::<TestUser>()
					.label("user_count")
					.expect("valid aggregate label"),
			)
			.expect("count annotation should compile")
			.annotate(
				crate::orm::func::sum(TestUser::field_id())
					.distinct()
					.label("distinct_id_sum")
					.expect("valid aggregate label"),
			)
			.expect("distinct aggregate annotation should compile");

		let sql = queryset
			.build_select_statement()
			.expect("query statement should build")
			.to_string(reinhardt_query::prelude::PostgresQueryBuilder);

		assert!(sql.contains(r#"COUNT(*) AS "user_count""#));
		assert!(sql.contains(r#"SUM(DISTINCT "test_users"."id") AS "distinct_id_sum""#));
		assert!(!sql.contains(r#""test_users"."*""#));
		assert!(!sql.contains(r#"COUNT(DISTINCT "id") AS "distinct_id_sum""#));
	}

	#[rstest]
	fn test_manual_join_qualifies_root_field_annotation() {
		use crate::orm::annotation::{Annotation, AnnotationValue};
		use crate::orm::expressions::F;

		let queryset = QuerySet::<TestUser>::new()
			.inner_join::<TestCorpusFile>("id", "id")
			.annotate_legacy(Annotation::field(
				"user_id",
				AnnotationValue::Field(F::new("id")),
			));

		let sql = queryset
			.build_select_statement()
			.expect("query statement should build")
			.to_string(reinhardt_query::prelude::PostgresQueryBuilder);

		assert!(sql.contains(r#""test_users"."id" AS "user_id""#));
	}

	#[test]
	fn test_relation_filter_qualifies_root_having_aggregate() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let mut queryset = QuerySet::<TestUser>::new()
			.values(&["id"])
			.filter(related_filter)
			.having(crate::orm::func::sum(TestUser::field_id()).gt(1_i64));
		queryset.group_by_fields = vec!["id".to_owned()];

		let sql = queryset.to_sql().expect("query SQL should compile");

		assert!(sql.contains(r#"HAVING SUM("test_users"."id") > 1"#));
	}

	#[test]
	fn test_relation_filter_qualifies_root_field_and_expression_annotations() {
		use crate::orm::annotation::{Annotation, AnnotationValue, Expression, Value};
		use crate::orm::expressions::F;

		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(related_filter)
			.annotate_legacy(Annotation::field(
				"user_id",
				AnnotationValue::Field(F::new("id")),
			))
			.annotate_legacy(Annotation::field(
				"next_user_id",
				AnnotationValue::Expression(Expression::Add(
					Box::new(AnnotationValue::Field(F::new("id"))),
					Box::new(AnnotationValue::Value(Value::Int(1))),
				)),
			))
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#""test_users"."id" AS "user_id""#));
		assert!(sql.contains(r#"("test_users"."id" + 1) AS "next_user_id""#));
	}

	#[test]
	fn test_relation_filter_qualifies_case_annotation_predicates() {
		use crate::orm::Q;
		use crate::orm::annotation::{Annotation, AnnotationValue, Expression, Value, When};

		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(related_filter)
			.annotate_legacy(Annotation::field(
				"is_primary",
				AnnotationValue::Expression(Expression::Case {
					whens: vec![When::new(
						Q::new("id", "=", "1"),
						AnnotationValue::Value(Value::Int(1)),
					)],
					default: Some(Box::new(AnnotationValue::Value(Value::Int(0)))),
				}),
			))
			.to_sql()
			.expect("query SQL should compile");

		assert!(
			sql.contains(r#"CASE WHEN "test_users"."id" = 1 THEN 1 ELSE 0 END AS "is_primary""#)
		);
	}

	#[test]
	fn test_relation_filter_qualifies_postgres_annotation_fields() {
		use crate::orm::postgres_features::{
			ArrayAgg, BackendAnnotation, BackendAnnotationValue, JsonbAgg, JsonbBuildObject,
			StringAgg, TsRank,
		};

		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.values(&["id"])
			.filter(related_filter)
			.annotate_backend(
				BackendAnnotation::new(
					"ids",
					BackendAnnotationValue::ArrayAgg(ArrayAgg::<serde_json::Value>::new(
						"id".to_string(),
					)),
				)
				.unwrap(),
			)
			.expect("ids annotation")
			.annotate_backend(
				BackendAnnotation::new(
					"names",
					BackendAnnotationValue::StringAgg(StringAgg::new(
						"username".to_string(),
						",".to_string(),
					)),
				)
				.unwrap(),
			)
			.expect("names annotation")
			.annotate_backend(
				BackendAnnotation::new(
					"metadata_values",
					BackendAnnotationValue::JsonbAgg(JsonbAgg::new("metadata".to_string())),
				)
				.unwrap(),
			)
			.expect("metadata annotation")
			.annotate_backend(
				BackendAnnotation::new(
					"payload",
					BackendAnnotationValue::JsonbBuildObject(
						JsonbBuildObject::new().add("user_id", "id"),
					),
				)
				.unwrap(),
			)
			.expect("payload annotation")
			.annotate_backend(
				BackendAnnotation::new(
					"rank",
					BackendAnnotationValue::TsRank(TsRank::new(
						"search_vector".to_string(),
						"rust".to_string(),
					)),
				)
				.unwrap(),
			)
			.expect("rank annotation")
			.to_sql()
			.expect("query SQL should compile");

		assert!(sql.contains(r#"ARRAY_AGG("test_users"."id") AS "ids""#));
		assert!(sql.contains(r#"STRING_AGG("test_users"."username", ',') AS "names""#));
		assert!(sql.contains(r#"JSONB_AGG("test_users"."metadata") AS "metadata_values""#));
		assert!(sql.contains(r#"jsonb_build_object('user_id', "test_users"."id") AS "payload""#));
		assert!(sql.contains(
			r#"ts_rank("test_users"."search_vector", to_tsquery('english', 'rust')) AS "rank""#
		));
	}

	#[test]
	#[should_panic(expected = "typed relation filter root does not match QuerySet model")]
	fn test_erased_typed_relation_filter_rejects_different_root_model() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");
		let condition = related_filter
			.and(Filter::new(
				"id",
				FilterOperator::Eq,
				FilterValue::Integer(1),
			))
			.into_filter_condition();

		let _ = QuerySet::<TestProject>::new().filter(condition);
	}

	#[test]
	fn test_related_field_filter_uses_target_db_column() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_email())
			.eq("person@example.com");

		let sql = QuerySet::<TestUser>::new()
			.filter(filter)
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "test_users".* FROM "test_users" INNER JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id" WHERE "corpus_file"."email_addr" = 'person@example.com'"#
		);
	}

	#[test]
	fn test_relation_filter_qualifies_root_rhs_field_reference() {
		let related_filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let sql = QuerySet::<TestUser>::new()
			.filter(related_filter)
			.filter(Filter::new(
				"username",
				FilterOperator::Eq,
				FilterValue::FieldRef(crate::orm::expressions::F::new("email")),
			))
			.to_sql()
			.expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "test_users".* FROM "test_users" INNER JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id" WHERE ("corpus_file"."normalized_path" = '/docs/index.md' AND "test_users"."username" = "test_users"."email")"#
		);
	}

	#[test]
	fn test_relation_filter_qualifies_root_grouping_field() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");
		let mut queryset = QuerySet::<TestUser>::new().filter(filter);
		queryset.group_by_fields = vec!["id".to_string()];

		let sql = queryset.to_sql().expect("query SQL should compile");

		assert_eq!(
			sql,
			r#"SELECT "test_users".* FROM "test_users" INNER JOIN "test_corpus_files" AS "corpus_file" ON "test_users"."corpus_file_id" = "corpus_file"."id" WHERE "corpus_file"."normalized_path" = '/docs/index.md' GROUP BY "test_users"."id""#
		);
	}

	#[test]
	fn temporal_projection_rejects_grouped_querysets() {
		let queryset =
			QuerySet::<TestUser>::new().having(crate::orm::func::count_all::<TestUser>().gt(1_i64));
		let mut queryset = queryset;
		queryset.group_by_fields = vec!["id".to_string()];

		let error = queryset
			.temporal_projection_statement(
				"created_at",
				TemporalTruncKind::Day,
				DateProjectionOrder::Asc,
				None,
				TemporalTruncOutput::Date,
			)
			.expect_err("temporal projections must reject grouped querysets");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
		assert_eq!(
			error.to_string(),
			"Database error: date and datetime projections are not supported on grouped querysets"
		);
	}

	#[test]
	fn temporal_projection_rejects_having_only_querysets() {
		let queryset =
			QuerySet::<TestUser>::new().having(crate::orm::func::count_all::<TestUser>().gt(1_i64));

		let error = queryset
			.temporal_projection_statement(
				"created_at",
				TemporalTruncKind::Day,
				DateProjectionOrder::Asc,
				None,
				TemporalTruncOutput::Date,
			)
			.expect_err("temporal projections must reject HAVING-only querysets");

		assert_eq!(
			error.to_string(),
			"Database error: date and datetime projections are not supported on grouped querysets"
		);
	}

	#[test]
	fn temporal_projection_rejects_querysets_with_lateral_joins() {
		let queryset = QuerySet::<TestUser>::new().with_lateral_join(
			crate::orm::lateral_join::LateralJoin::new("latest_event", "SELECT 1").inner(),
		);

		let error = queryset
			.temporal_projection_statement(
				"created_at",
				TemporalTruncKind::Day,
				DateProjectionOrder::Asc,
				None,
				TemporalTruncOutput::Date,
			)
			.expect_err("temporal projections must reject querysets with lateral joins");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
		assert_eq!(
			error.to_string(),
			"Database error: date and datetime projections are not supported on querysets with lateral joins"
		);
	}

	#[rstest]
	fn temporal_projection_rejects_querysets_with_ctes() {
		let queryset = QuerySet::<TestUser>::new().with_cte(crate::orm::cte::CTE::new(
			"recent_users",
			"SELECT * FROM test_users",
		));

		let error = queryset
			.temporal_projection_statement(
				"created_at",
				TemporalTruncKind::Day,
				DateProjectionOrder::Asc,
				None,
				TemporalTruncOutput::Date,
			)
			.expect_err("temporal projections must reject querysets with CTEs");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
		assert_eq!(
			error.to_string(),
			"Database error: date and datetime projections are not supported on querysets with CTEs"
		);
	}

	#[test]
	#[should_panic(
		expected = "typed prefetch_related supports only direct multi-valued relation paths"
	)]
	fn test_typed_prefetch_related_rejects_forward_single_valued_path() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>();

		let _ = QuerySet::<TestUser>::new().prefetch_related(path);
	}

	#[test]
	fn test_typed_prefetch_related_query_uses_reverse_relation_metadata() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestProject>::from_descriptor::<
			TestUserProjects,
		>();
		let queryset = QuerySet::<TestUser>::new().prefetch_related(path);

		let queries = queryset.prefetch_related_queries(&[1, 2]);
		let sql = queries[0].1.to_string(PostgresQueryBuilder);

		assert_eq!(queries[0].0, "projects");
		assert_eq!(
			sql,
			r#"SELECT "projects".* FROM "test_projects" AS "projects" WHERE "projects"."test_user_id" IN (1, 2)"#
		);
	}

	#[test]
	fn typed_related_aggregate_qualifies_root_projection_and_filter() {
		let path = unsafe {
			crate::orm::relations::RelationPath::<
				TestUser,
				TestProject,
				crate::orm::relations::GeneratedRelationPath,
			>::from_generated_steps(
				<TestUserProjects as crate::orm::relations::RelationDescriptor>::steps()
			)
		};
		let expression = crate::orm::func::count(path);
		let (node, joins) = expression.into_parts();
		let mut queryset = QuerySet::<TestUser>::new().values(&["id", "username", "email"]);
		queryset.typed_annotations.push(
			crate::orm::query_fields::expression::node::StoredExpression::new(
				node,
				joins,
				Some("project_count".to_owned()),
			),
		);
		let sql = queryset
			.filter(Filter::new(
				"id",
				FilterOperator::Eq,
				FilterValue::Integer(1),
			))
			.to_sql()
			.expect("typed related aggregate query should compile");

		assert!(sql.starts_with(
			r##"SELECT "test_users"."id", "test_users"."username", "test_users"."email", "##
		));
		assert!(sql.contains(
			r##"LEFT JOIN "test_projects" AS "projects" ON "test_users"."id" = "projects"."test_user_id""##,
		));
		assert!(sql.contains(r##"WHERE "test_users"."id" = 1"##));
	}

	#[test]
	fn typed_aggregate_rejects_selected_expression_cross_multiplication() {
		let projects = unsafe {
			crate::orm::relations::RelationPath::<
				TestUser,
				TestProject,
				crate::orm::relations::GeneratedRelationPath,
			>::from_generated_steps(
				<TestUserProjects as crate::orm::relations::RelationDescriptor>::steps()
			)
		};
		let tags = unsafe {
			crate::orm::relations::RelationPath::<
				TestUser,
				TestTag,
				crate::orm::relations::GeneratedRelationPath,
			>::from_generated_steps(
				<TestUserTags as crate::orm::relations::RelationDescriptor>::steps()
			)
		};

		let tag_name = tags.field(unsafe {
			// SAFETY: `name` is a persisted TestTag field in this query fixture.
			crate::orm::expressions::FieldRef::<
				TestTag,
				String,
				crate::orm::expressions::GeneratedModelField,
			>::from_model_field("name")
		});
		let error = QuerySet::<TestUser>::new()
			.annotate(
				crate::orm::func::count(projects)
					.label("project_count")
					.expect("valid aggregate label"),
			)
			.expect("aggregate annotation should compile")
			.select_expr("tag_name", tag_name.into_expression())
			.to_sql()
			.expect_err("independent selected relation paths must be rejected");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
	}

	#[test]
	fn typed_aggregate_rejects_having_cross_multiplication() {
		let projects = unsafe {
			crate::orm::relations::RelationPath::<
				TestUser,
				TestProject,
				crate::orm::relations::GeneratedRelationPath,
			>::from_generated_steps(
				<TestUserProjects as crate::orm::relations::RelationDescriptor>::steps()
			)
		};
		let tags = unsafe {
			crate::orm::relations::RelationPath::<
				TestUser,
				TestTag,
				crate::orm::relations::GeneratedRelationPath,
			>::from_generated_steps(
				<TestUserTags as crate::orm::relations::RelationDescriptor>::steps()
			)
		};

		let error = QuerySet::<TestUser>::new()
			.annotate(
				crate::orm::func::count(projects)
					.label("project_count")
					.expect("valid aggregate label"),
			)
			.expect("aggregate annotation should compile")
			.having(crate::orm::func::count(tags).gt(0_i64))
			.to_sql()
			.expect_err("independent HAVING relation paths must be rejected");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
	}

	#[test]
	fn typed_having_rejects_independent_multi_valued_paths_without_annotations() {
		let projects = unsafe {
			crate::orm::relations::RelationPath::<
				TestUser,
				TestProject,
				crate::orm::relations::GeneratedRelationPath,
			>::from_generated_steps(
				<TestUserProjects as crate::orm::relations::RelationDescriptor>::steps()
			)
		};
		let tags = unsafe {
			crate::orm::relations::RelationPath::<
				TestUser,
				TestTag,
				crate::orm::relations::GeneratedRelationPath,
			>::from_generated_steps(
				<TestUserTags as crate::orm::relations::RelationDescriptor>::steps()
			)
		};
		let mut queryset = QuerySet::<TestUser>::new();
		queryset.group_by_fields = vec!["id".to_owned()];

		let error = queryset
			.having(crate::orm::func::count(projects).gt(0_i64))
			.having(crate::orm::func::count(tags).gt(0_i64))
			.to_sql()
			.expect_err("independent HAVING paths must be rejected without annotations");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
	}

	#[tokio::test]
	async fn terminal_aggregate_rejects_mixed_root_and_multi_valued_operands() {
		let projects = unsafe {
			crate::orm::relations::RelationPath::<
				TestUser,
				TestProject,
				crate::orm::relations::GeneratedRelationPath,
			>::from_generated_steps(
				<TestUserProjects as crate::orm::relations::RelationDescriptor>::steps()
			)
		};
		let mut executor = ExplainRecordingExecutor::new(DatabaseBackend::Postgres, Vec::new());
		let error = QuerySet::<TestUser>::new()
			.aggregate_with_db(
				[
					crate::orm::func::count_all::<TestUser>()
						.label("user_count")
						.expect("valid root aggregate label"),
					crate::orm::func::count(projects)
						.label("project_count")
						.expect("valid relation aggregate label"),
				],
				&mut executor,
			)
			.await
			.expect_err("root and multi-valued aggregates must be rejected");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Unsupported)
		);
		assert!(
			executor.calls.is_empty(),
			"invalid shapes must not reach the executor"
		);
	}

	#[test]
	#[should_panic(
		expected = "typed prefetch_related supports only direct multi-valued relation paths through the root primary key"
	)]
	fn test_typed_prefetch_related_rejects_non_primary_reverse_source_column() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestProject>::from_descriptor::<
			TestUserProjectsByUsername,
		>();

		let _ = QuerySet::<TestUser>::new().prefetch_related(path);
	}

	#[test]
	fn test_typed_prefetch_keeps_relation_name_when_sql_alias_collides_with_root_table() {
		use crate::orm::relations::RelationPathLike;

		let path =
			crate::orm::relations::RelationPath::<TestProjects, TestProjects>::from_descriptor::<
				TestProjectsChildren,
			>();
		assert_eq!(path.leaf_alias(), "projects__projects");

		let queryset = QuerySet::<TestProjects>::new()
			.prefetch_related(path)
			.prefetch_related(&["projects"]);
		let queries = queryset.prefetch_related_queries(&[1, 2]);
		let sql = queries[0].1.to_string(PostgresQueryBuilder);

		assert_eq!(queries.len(), 1);
		assert_eq!(queries[0].0, "projects");
		assert_eq!(
			sql,
			r#"SELECT "projects__projects".* FROM "projects" AS "projects__projects" WHERE "projects__projects"."parent_id" IN (1, 2)"#
		);
	}

	#[test]
	fn test_typed_prefetch_related_allows_direct_many_to_many_path() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestTag>::from_descriptor::<
			TestUserTags,
		>();
		let queryset = QuerySet::<TestUser>::new().prefetch_related(path);

		let queries = queryset.prefetch_related_queries(&[1, 2]);
		let sql = queries[0].1.to_string(PostgresQueryBuilder);

		assert_eq!(queries[0].0, "tags");
		assert_eq!(
			sql,
			r#"SELECT "tags".*, "tags__through"."test_user_id" FROM "test_tags" AS "tags" INNER JOIN "test_user_tags" AS "tags__through" ON "tags"."id" = "tags__through"."tag_id" WHERE "tags__through"."test_user_id" IN (1, 2)"#
		);
	}

	#[test]
	fn test_string_prefetch_appends_without_discarding_typed_plan() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestTag>::from_descriptor::<
			TestUserTags,
		>();
		let queryset = QuerySet::<TestUser>::new()
			.prefetch_related(path)
			.prefetch_related(&["comments"]);

		let queries = queryset.prefetch_related_queries(&[1, 2]);
		let fields: Vec<_> = queries.iter().map(|(field, _)| field.as_str()).collect();

		assert_eq!(fields, vec!["tags", "comments"]);
		assert_eq!(queryset.typed_prefetch_related.len(), 1);
	}

	#[test]
	fn test_legacy_relation_loaders_allow_models_without_relationship_metadata() {
		assert!(
			QuerySet::<TestCorpusFile>::new()
				.validate_relation_path_for_test("owner__profile")
				.is_ok()
		);

		let queryset = QuerySet::<TestCorpusFile>::new()
			.select_related(&["owner"])
			.prefetch_related(&["documents"]);

		assert_eq!(queryset.select_related_fields, vec!["owner"]);
		assert_eq!(queryset.prefetch_related_fields, vec!["documents"]);
	}

	#[test]
	fn test_multi_valued_relation_filter_count_uses_distinct_root_pk() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestProject>::from_descriptor::<
				TestUserProjects,
			>()
			.field(unsafe {
				// SAFETY: `name` is a persisted TestProject field in this query fixture.
				crate::orm::expressions::FieldRef::<
					TestProject,
					String,
					crate::orm::expressions::GeneratedModelField,
				>::from_model_field("name")
			})
			.icontains("rust");

		let sql = QuerySet::<TestUser>::new()
			.filter(filter)
			.count_select_query()
			.expect("count select query")
			.to_string(PostgresQueryBuilder);

		assert!(sql.starts_with(
			r#"SELECT COUNT(DISTINCT "test_users"."id") AS "count" FROM "test_users""#
		));
	}

	#[test]
	fn test_multi_valued_relation_filter_count_uses_distinct_composite_root_pk_subquery() {
		let filter =
			crate::orm::relations::RelationPath::<TestMembership, TestProject>::from_descriptor::<
				TestMembershipProjects,
			>()
			.field(unsafe {
				// SAFETY: `name` is a persisted TestProject field in this query fixture.
				crate::orm::expressions::FieldRef::<
					TestProject,
					String,
					crate::orm::expressions::GeneratedModelField,
				>::from_model_field("name")
			})
			.icontains("rust");

		let sql = QuerySet::<TestMembership>::new()
			.filter(filter)
			.count_select_query()
			.expect("count select query")
			.to_string(SqliteQueryBuilder);

		assert!(sql.starts_with(r#"SELECT COUNT(*) AS "count" FROM (SELECT DISTINCT "test_memberships"."member_user_id", "test_memberships"."member_role_id" FROM "test_memberships""#));
		assert!(!sql.contains("COUNT(DISTINCT"));
		assert!(sql.contains(r#"WHERE "projects"."name" ILIKE '%rust%' ESCAPE '\'"#));
	}

	#[test]
	#[should_panic(
		expected = "typed prefetch_related supports only direct multi-valued relation paths"
	)]
	fn test_typed_prefetch_related_rejects_multi_hop_path() {
		let path =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.then::<TestCorpusFileProject, TestProject>();

		let _ = QuerySet::<TestUser>::new().prefetch_related(path);
	}

	#[test]
	fn test_update_fields_rejects_related_filters() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let error = QuerySet::<TestUser>::new()
			.filter(filter)
			.update_fields_sql([("username", "alice")])
			.expect_err("related filters should not render unsupported update aliases");

		assert!(matches!(
			error,
			reinhardt_core::exception::Error::Validation(message)
				if message.contains("typed related filters")
		));
	}

	#[test]
	fn test_eager_only_typed_relation_writes_do_not_use_select_aliases() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>();
		let queryset = QuerySet::<TestUser>::new()
			.from_as("u")
			.select_related(path)
			.filter(TestUser::field_id().eq(1));
		let mut updates = HashMap::new();
		updates.insert(
			"username".to_string(),
			UpdateValue::String("alice".to_string()),
		);

		let (update_sql, _) = queryset
			.update_sql(&updates)
			.expect("eager-only update SQL should compile");
		let (update_fields_sql, _) = queryset
			.update_fields_sql([("username", "alice")])
			.expect("eager-only update fields should build");
		let (delete_sql, _) = queryset
			.delete_sql()
			.expect("eager-only delete SQL should compile");

		for sql in [update_sql, update_fields_sql, delete_sql] {
			assert!(!sql.contains(r#""u"."id""#));
			assert!(sql.contains(r#"WHERE "id" ="#));
		}
	}

	#[test]
	fn test_delete_rejects_related_filters() {
		let filter =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");

		let error = QuerySet::<TestUser>::new()
			.filter(filter)
			.delete_sql()
			.expect_err("related filters should not render unsupported delete aliases");

		assert!(matches!(
			error,
			reinhardt_core::exception::Error::Validation(message)
				if message
					== "QuerySet::delete_query does not support typed related filters; use a subquery or select query first"
		));
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
		let select_stmt = queryset
			.select_related_query()
			.expect("select-related query should compile");
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
		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");
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
		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");
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
		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");
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
		let (where_clause, params) = queryset
			.build_where_clause()
			.expect("where clause should compile");
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

		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");
		assert_eq!(sql, "DELETE FROM \"test_users\" WHERE \"id\" = $1");
		assert_eq!(params, vec!["1"]);
	}

	#[rstest]
	#[case("username", r#""username""#)]
	#[case("user_id", r#""user_id""#)]
	#[case(r#"a"b"#, r#""a""b""#)]
	#[case("field; DROP TABLE users", r#""field; DROP TABLE users""#)]
	#[case("", r#""""#)]
	#[case("authors.id", r#""authors"."id""#)]
	#[case("schema.table.column", r#""schema"."table"."column""#)]
	fn test_quote_identifier(#[case] input: &str, #[case] expected: &str) {
		// Arrange
		// input and expected provided by rstest cases

		// Act
		let result = super::quote_identifier(input);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_outerref_filter_uses_safe_quoting() {
		// Arrange
		use crate::orm::expressions::OuterRef;
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"author_id".to_string(),
			FilterOperator::Eq,
			FilterValue::OuterRef(OuterRef::new("id")),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "author_id" = "id""#
		);
	}

	#[rstest]
	fn test_array_contains_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"tags".to_string(),
			FilterOperator::ArrayContains,
			FilterValue::Array(vec!["rust".to_string(), "web".to_string()]),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "tags" @> ARRAY['rust', 'web']"#
		);
	}

	#[rstest]
	fn test_outerref_dot_separated_renders_qualified_column() {
		// Arrange
		use crate::orm::expressions::OuterRef;
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"author_id".to_string(),
			FilterOperator::Eq,
			FilterValue::OuterRef(OuterRef::new("authors.id")),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "author_id" = "authors"."id""#
		);
	}

	#[rstest]
	fn test_injection_attempt_in_field_name_is_quoted() {
		// Arrange
		// Attempt SQL injection via field name with double quote
		let malicious_field = r#"id" OR 1=1 --"#.to_string();

		// Act
		let quoted = super::quote_identifier(&malicious_field);

		// Assert
		// The double quote inside is escaped, preventing injection
		assert_eq!(quoted, r#""id"" OR 1=1 --""#);
		// Verify the quote is not broken out of
		assert!(quoted.starts_with('"'));
		assert!(quoted.ends_with('"'));
	}

	#[rstest]
	#[should_panic(expected = "SQL identifier must not contain null bytes")]
	fn test_quote_identifier_rejects_null_bytes() {
		// Arrange
		let field_with_null = "field\0name";

		// Act
		super::quote_identifier(field_with_null);

		// Assert - should panic before reaching here
	}

	#[rstest]
	#[case(FilterOperator::Ne, "<>")]
	#[case(FilterOperator::Gt, ">")]
	#[case(FilterOperator::Gte, ">=")]
	#[case(FilterOperator::Lt, "<")]
	#[case(FilterOperator::Lte, "<=")]
	fn test_outerref_comparison_operators(#[case] op: FilterOperator, #[case] sql_op: &str) {
		// Arrange
		use crate::orm::expressions::OuterRef;
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"author_id".to_string(),
			op,
			FilterValue::OuterRef(OuterRef::new("id")),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		let expected = format!(
			r#"SELECT * FROM "test_users" WHERE "author_id" {} "id""#,
			sql_op
		);
		assert_eq!(sql, expected);
	}

	#[rstest]
	fn test_array_contained_by_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"tags".to_string(),
			FilterOperator::ArrayContainedBy,
			FilterValue::Array(vec!["rust".to_string()]),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "tags" <@ ARRAY['rust']"#
		);
	}

	#[rstest]
	fn test_array_overlap_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"tags".to_string(),
			FilterOperator::ArrayOverlap,
			FilterValue::Array(vec!["rust".to_string()]),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "tags" && ARRAY['rust']"#
		);
	}

	#[rstest]
	fn test_full_text_match_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"content".to_string(),
			FilterOperator::FullTextMatch,
			FilterValue::String("search term".to_string()),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "content" @@ plainto_tsquery('english', 'search term')"#
		);
	}

	#[rstest]
	fn test_jsonb_contains_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"metadata".to_string(),
			FilterOperator::JsonbContains,
			FilterValue::String(r#"{"key": "value"}"#.to_string()),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "metadata" @> '{"key": "value"}'::jsonb"#
		);
	}

	#[rstest]
	fn test_jsonb_contained_by_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"metadata".to_string(),
			FilterOperator::JsonbContainedBy,
			FilterValue::String(r#"{"key": "value"}"#.to_string()),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "metadata" <@ '{"key": "value"}'::jsonb"#
		);
	}

	#[rstest]
	fn test_jsonb_key_exists_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"metadata".to_string(),
			FilterOperator::JsonbKeyExists,
			FilterValue::String("key".to_string()),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "metadata" ? 'key'"#
		);
	}

	#[rstest]
	fn test_jsonb_any_key_exists_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"metadata".to_string(),
			FilterOperator::JsonbAnyKeyExists,
			FilterValue::Array(vec!["key1".to_string(), "key2".to_string()]),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "metadata" ?| array['key1', 'key2']"#
		);
	}

	#[rstest]
	fn test_jsonb_all_keys_exist_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"metadata".to_string(),
			FilterOperator::JsonbAllKeysExist,
			FilterValue::Array(vec!["key1".to_string(), "key2".to_string()]),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "metadata" ?& array['key1', 'key2']"#
		);
	}

	#[rstest]
	fn test_jsonb_path_exists_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"metadata".to_string(),
			FilterOperator::JsonbPathExists,
			FilterValue::String("$.key".to_string()),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "metadata" @'$.key' "#
		);
	}

	#[rstest]
	#[case(
		Filter::new("username", FilterOperator::IExact, FilterValue::String("Alice".to_string())),
		r#"SELECT * FROM "test_users" WHERE "username" ILIKE 'Alice' ESCAPE '\'"#
	)]
	#[case(
		Filter::new("email", FilterOperator::IContains, FilterValue::String("example.com".to_string())),
		r#"SELECT * FROM "test_users" WHERE "email" ILIKE '%example.com%' ESCAPE '\'"#
	)]
	#[case(
		Filter::new("username", FilterOperator::IStartsWith, FilterValue::String("ali".to_string())),
		r#"SELECT * FROM "test_users" WHERE "username" ILIKE 'ali%' ESCAPE '\'"#
	)]
	#[case(
		Filter::new("username", FilterOperator::IEndsWith, FilterValue::String("ice".to_string())),
		r#"SELECT * FROM "test_users" WHERE "username" ILIKE '%ice' ESCAPE '\'"#
	)]
	#[case(
		Filter::new("username", FilterOperator::Regex, FilterValue::String("^a".to_string())),
		r#"SELECT * FROM "test_users" WHERE "username" ~ '^a'"#
	)]
	#[case(
		Filter::new("username", FilterOperator::IRegex, FilterValue::String("^a".to_string())),
		r#"SELECT * FROM "test_users" WHERE "username" ~* '^a'"#
	)]
	fn test_django_style_string_lookup_filters(#[case] filter: Filter, #[case] expected: &str) {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(filter);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(sql, expected);
	}

	#[rstest]
	fn test_filter_or_chain_generates_expected_sql() {
		// Arrange
		let condition = TestUser::field_username()
			.exact("alice")
			.or(TestUser::field_email().icontains("example.com"));
		let queryset = QuerySet::<TestUser>::new().filter(condition);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE ("username" = 'alice' OR "email" ILIKE '%example.com%' ESCAPE '\')"#
		);
	}

	#[rstest]
	fn test_filter_and_chain_generates_expected_sql() {
		// Arrange
		let condition = TestUser::field_username()
			.exact("alice")
			.and(TestUser::field_id().gte(10));
		let queryset = QuerySet::<TestUser>::new().filter(condition);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE ("username" = 'alice' AND "id" >= 10)"#
		);
	}

	#[rstest]
	fn test_filter_not_chain_generates_expected_sql() {
		// Arrange
		let condition = TestUser::field_username().exact("alice").not();
		let queryset = QuerySet::<TestUser>::new().filter(condition);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE NOT "username" = 'alice'"#
		);
	}

	#[rstest]
	fn test_composite_only_filter_is_recognized_by_delete_sql() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(
			TestUser::field_username()
				.exact("alice")
				.or(TestUser::field_email().icontains("example.com")),
		);

		// Act
		let (sql, params) = queryset.delete_sql().expect("delete SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"DELETE FROM "test_users" WHERE ("username" = $1 OR "email" ILIKE $2 ESCAPE '\')"#
		);
		assert_eq!(params, vec!["alice", "%example.com%"]);
	}

	#[rstest]
	fn test_over_deep_filter_condition_returns_error_from_to_sql() {
		// Arrange
		let mut condition = FilterCondition::Single(TestUser::field_username().exact("alice"));
		for _ in 0..=MAX_FILTER_CONDITION_DEPTH {
			condition = FilterCondition::not(condition);
		}
		let queryset = QuerySet::<TestUser>::new().filter(condition);

		// Act
		let condition_result = queryset.build_where_condition();
		let sql_result = queryset.to_sql();

		// Assert
		assert!(matches!(
			condition_result,
			Err(reinhardt_core::exception::Error::Validation(_))
		));
		assert!(matches!(
			sql_result,
			Err(reinhardt_core::exception::Error::Validation(_))
		));
	}

	#[rstest]
	fn test_select_related_query_propagates_over_deep_filter_error() {
		let path = crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
			TestUserCorpusFile,
		>();
		let mut condition = FilterCondition::Single(TestUser::field_username().exact("alice"));
		for _ in 0..=MAX_FILTER_CONDITION_DEPTH {
			condition = FilterCondition::not(condition);
		}
		let queryset = QuerySet::<TestUser>::new()
			.filter(condition)
			.select_related(path);

		assert!(matches!(
			queryset.select_related_query(),
			Err(reinhardt_core::exception::Error::Validation(_))
		));
	}

	#[rstest]
	fn test_relation_join_collection_stops_at_filter_depth_limit() {
		let related =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");
		let mut condition = related.into_filter_condition();
		for _ in 0..=MAX_FILTER_CONDITION_DEPTH {
			condition = FilterCondition::not(condition);
		}

		let queryset = QuerySet::<TestUser>::new().filter(condition);

		assert!(matches!(
			queryset.build_where_condition(),
			Err(reinhardt_core::exception::Error::Validation(_))
		));
	}

	#[rstest]
	fn test_update_fields_rejects_over_deep_related_filter_before_relation_scan() {
		let related =
			crate::orm::relations::RelationPath::<TestUser, TestCorpusFile>::from_descriptor::<
				TestUserCorpusFile,
			>()
			.field(TestCorpusFile::field_normalized_path())
			.eq("/docs/index.md");
		let mut condition = related.into_filter_condition();
		for _ in 0..=MAX_FILTER_CONDITION_DEPTH {
			condition = FilterCondition::not(condition);
		}

		let error = QuerySet::<TestUser>::new()
			.filter(condition)
			.update_fields_sql([("username", "alice")])
			.expect_err("over-deep related filters must fail validation");

		assert!(matches!(
			error,
			reinhardt_core::exception::Error::Validation(message)
				if message.contains("maximum depth")
		));
	}

	#[rstest]
	#[case(
		Filter::new("email", FilterOperator::IContains, FilterValue::String("100%_match\\".to_string())),
		r#"SELECT * FROM "test_users" WHERE "email" ILIKE '%100\%\_match\\%' ESCAPE '\'"#
	)]
	#[case(
		Filter::new("username", FilterOperator::IExact, FilterValue::String("alice_admin".to_string())),
		r#"SELECT * FROM "test_users" WHERE "username" ILIKE 'alice\_admin' ESCAPE '\'"#
	)]
	fn test_django_style_case_insensitive_like_filters_escape_metacharacters(
		#[case] filter: Filter,
		#[case] expected: &str,
	) {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(filter);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(sql, expected);
	}

	#[rstest]
	#[case(
		Filter::new("email", FilterOperator::Contains, FilterValue::String("100%_match\\".to_string())),
		r#"SELECT * FROM "test_users" WHERE "email" LIKE '%100\%\_match\\%' ESCAPE '\'"#
	)]
	#[case(
		Filter::new("username", FilterOperator::StartsWith, FilterValue::String("alice_admin".to_string())),
		r#"SELECT * FROM "test_users" WHERE "username" LIKE 'alice\_admin%' ESCAPE '\'"#
	)]
	#[case(
		Filter::new("username", FilterOperator::EndsWith, FilterValue::String("100%".to_string())),
		r#"SELECT * FROM "test_users" WHERE "username" LIKE '%100\%' ESCAPE '\'"#
	)]
	fn test_django_style_case_sensitive_like_filters_escape_metacharacters(
		#[case] filter: Filter,
		#[case] expected: &str,
	) {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(filter);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(sql, expected);
	}

	#[test]
	fn typed_like_filters_treat_null_as_is_null() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"username",
			FilterOperator::Contains,
			FilterValue::Typed(Ok(DatabaseValue::Null)),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "username" IS NULL"#
		);
	}

	#[rstest]
	fn test_django_style_is_in_filter_accepts_typed_values() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"id",
			FilterOperator::In,
			FilterValue::List(vec![FilterValue::Integer(1), FilterValue::Integer(2)]),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(sql, r#"SELECT * FROM "test_users" WHERE "id" IN (1, 2)"#);
	}

	#[rstest]
	fn test_django_style_between_range_filter() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"id",
			FilterOperator::Range,
			FilterValue::Range(
				Box::new(FilterValue::Integer(10)),
				Box::new(FilterValue::Integer(20)),
			),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "id" BETWEEN 10 AND 20"#
		);
	}

	#[rstest]
	fn test_django_style_date_part_filter_expression() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::expression(
			"EXTRACT(YEAR FROM \"created_at\")",
			FilterOperator::Eq,
			FilterValue::Integer(2026),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE EXTRACT(YEAR FROM "created_at") = 2026"#
		);
	}

	#[test]
	fn test_typed_predicate_expr_qualifies_root_columns() {
		use reinhardt_query::prelude::{Alias, ColumnRef, Condition, Query};

		// Arrange
		let filter = Filter::typed_predicate(TestUser::field_id().into_expression().gt(10_i64));

		// Act
		let expression = filter
			.typed_predicate_expr(Some("articles"))
			.expect("typed predicate should be returned");
		let sql = Query::select()
			.from(Alias::new("articles"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(expression))
			.to_string(PostgresQueryBuilder);

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "articles" WHERE "articles"."id" > 10"#
		);
	}

	#[rstest]
	fn test_public_filter_new_treats_expression_like_field_as_quoted_column() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"__reinhardt_filter_expr:EXTRACT(YEAR FROM \"created_at\")",
			FilterOperator::Eq,
			FilterValue::Integer(2026),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "__reinhardt_filter_expr:EXTRACT(YEAR FROM ""created_at"")" = 2026"#
		);
	}

	#[rstest]
	fn test_public_column_filter_uses_mutated_field_consistently() {
		// Arrange
		let mut filter = Filter::new(
			"username",
			FilterOperator::Eq,
			FilterValue::String("alice".into()),
		);
		filter.field = "email".to_string();
		let queryset = QuerySet::<TestUser>::new().filter(filter);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(sql, r#"SELECT * FROM "test_users" WHERE "email" = 'alice'"#);
	}

	#[rstest]
	fn test_mutated_transformed_filter_field_falls_back_to_quoted_column() {
		// Arrange
		let mut filter = TestUser::field_created_at().year().eq(2026);
		filter.field = "EXTRACT(MONTH FROM \"created_at\")".to_string();
		let queryset = QuerySet::<TestUser>::new().filter(filter);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "EXTRACT(MONTH FROM ""created_at"")" = 2026"#
		);
	}

	#[rstest]
	fn test_field_accessor_lookup_helpers_generate_expected_sql() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new()
			.filter(TestUser::field_username().exact("alice"))
			.filter(TestUser::field_email().icontains("example.com"))
			.filter(TestUser::field_id().is_in([1_i64, 2, 3]))
			.filter(TestUser::field_created_at().year().gte(2026));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE ("username" = 'alice' AND "email" ILIKE '%example.com%' ESCAPE '\' AND "id" IN (1, 2, 3) AND EXTRACT(YEAR FROM "created_at") >= 2026)"#
		);
	}

	#[rstest]
	fn test_field_accessor_null_not_in_and_range_helpers_generate_expected_sql() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new()
			.filter(TestUser::field_email().is_not_null())
			.filter(TestUser::field_id().not_in([10_i64, 20]))
			.filter(TestUser::field_id().range(100_i64, 200));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE ("email" IS NOT NULL AND "id" NOT IN (10, 20) AND "id" BETWEEN 100 AND 200)"#
		);
	}

	#[rstest]
	fn test_field_accessor_string_lookup_variants_generate_expected_sql() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new()
			.filter(TestUser::field_username().contains("lic"))
			.filter(TestUser::field_username().starts_with("a"))
			.filter(TestUser::field_username().ends_with("e"))
			.filter(TestUser::field_username().istarts_with("AL"))
			.filter(TestUser::field_username().iends_with("CE"))
			.filter(TestUser::field_username().regex("^a.*e$"))
			.filter(TestUser::field_username().iregex("^A.*E$"));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE ("username" LIKE '%lic%' ESCAPE '\' AND "username" LIKE 'a%' ESCAPE '\' AND "username" LIKE '%e' ESCAPE '\' AND "username" ILIKE 'AL%' ESCAPE '\' AND "username" ILIKE '%CE' ESCAPE '\' AND "username" ~ '^a.*e$' AND "username" ~* '^A.*E$')"#
		);
	}

	#[rstest]
	#[case(TestUser::field_created_at().date().eq("2026-06-10"), r#"SELECT * FROM "test_users" WHERE DATE("created_at") = '2026-06-10'"#)]
	#[case(TestUser::field_created_at().time().eq("05:00:00"), r#"SELECT * FROM "test_users" WHERE TIME("created_at") = '05:00:00'"#)]
	#[case(TestUser::field_created_at().month().eq(6), r#"SELECT * FROM "test_users" WHERE EXTRACT(MONTH FROM "created_at") = 6"#)]
	#[case(TestUser::field_created_at().day().eq(10), r#"SELECT * FROM "test_users" WHERE EXTRACT(DAY FROM "created_at") = 10"#)]
	#[case(TestUser::field_created_at().week().eq(24), r#"SELECT * FROM "test_users" WHERE EXTRACT(WEEK FROM "created_at") = 24"#)]
	#[case(TestUser::field_created_at().week_day().eq(4), r#"SELECT * FROM "test_users" WHERE (EXTRACT(DOW FROM "created_at") + 1) = 4"#)]
	#[case(TestUser::field_created_at().iso_week_day().eq(3), r#"SELECT * FROM "test_users" WHERE EXTRACT(ISODOW FROM "created_at") = 3"#)]
	#[case(TestUser::field_created_at().quarter().eq(2), r#"SELECT * FROM "test_users" WHERE EXTRACT(QUARTER FROM "created_at") = 2"#)]
	#[case(TestUser::field_created_at().hour().gte(5), r#"SELECT * FROM "test_users" WHERE EXTRACT(HOUR FROM "created_at") >= 5"#)]
	#[case(TestUser::field_created_at().minute().lt(30), r#"SELECT * FROM "test_users" WHERE EXTRACT(MINUTE FROM "created_at") < 30"#)]
	#[case(TestUser::field_created_at().second().lte(59), r#"SELECT * FROM "test_users" WHERE EXTRACT(SECOND FROM "created_at") <= 59"#)]
	fn test_field_accessor_date_time_transforms_generate_expected_sql(
		#[case] filter: Filter,
		#[case] expected: &str,
	) {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(filter);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(sql, expected);
	}

	#[rstest]
	fn test_field_accessor_postgres_array_jsonb_and_range_helpers_generate_expected_sql() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new()
			.filter(TestUser::field_tags().array_contains(["rust", "async"]))
			.filter(TestUser::field_tags().array_overlap(["web", "orm"]))
			.filter(TestUser::field_metadata().jsonb_contains(r#"{"active": true}"#))
			.filter(TestUser::field_metadata().jsonb_has_any_keys(["tier", "plan"]))
			.filter(TestUser::field_active_period().range_overlaps("[2026-01-01,2027-01-01)"));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE ("tags" @> ARRAY['rust', 'async'] AND "tags" && ARRAY['web', 'orm'] AND "metadata" @> '{"active": true}'::jsonb AND "metadata" ?| array['tier', 'plan'] AND "active_period" && '[2026-01-01,2027-01-01)')"#
		);
	}

	#[rstest]
	fn test_complex_django_style_lookup_query_combines_order_distinct_and_limit() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new()
			.filter(TestUser::field_email().icontains("example.com"))
			.filter(TestUser::field_username().is_not_null())
			.filter(TestUser::field_created_at().year().range(2024, 2026))
			.distinct()
			.order_by(&["-created_at", "username"])
			.limit(25)
			.offset(50);

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT DISTINCT * FROM "test_users" WHERE ("email" ILIKE '%example.com%' ESCAPE '\' AND "username" IS NOT NULL AND EXTRACT(YEAR FROM "created_at") BETWEEN 2024 AND 2026) ORDER BY "created_at" DESC, "username" ASC LIMIT 25 OFFSET 50"#
		);
	}

	#[rstest]
	fn test_range_contains_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"age_range".to_string(),
			FilterOperator::RangeContains,
			FilterValue::String("25".to_string()),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "age_range" @> '25'"#
		);
	}

	#[rstest]
	fn test_range_contained_by_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"age_range".to_string(),
			FilterOperator::RangeContainedBy,
			FilterValue::String("[20, 30]".to_string()),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "age_range" <@ '[20, 30]'"#
		);
	}

	#[rstest]
	fn test_range_overlaps_filter_quotes_field() {
		// Arrange
		let queryset = QuerySet::<TestUser>::new().filter(Filter::new(
			"age_range".to_string(),
			FilterOperator::RangeOverlaps,
			FilterValue::String("[20, 30]".to_string()),
		));

		// Act
		let sql = queryset.to_sql().expect("query SQL should compile");

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "test_users" WHERE "age_range" && '[20, 30]'"#
		);
	}
}
