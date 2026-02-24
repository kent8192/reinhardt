//! CREATE TRIGGER statement builder
//!
//! This module provides the `CreateTriggerStatement` type for building SQL CREATE TRIGGER queries.

use crate::{
	backend::QueryBuilder,
	expr::SimpleExpr,
	types::{
		DynIden, IntoIden, IntoTableRef, TableRef, TriggerBody, TriggerEvent, TriggerOrder,
		TriggerScope, TriggerTiming,
	},
};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE TRIGGER statement builder
///
/// This struct provides a fluent API for constructing CREATE TRIGGER queries.
///
/// # Backend Support
///
/// - **PostgreSQL**: Full support (BEFORE/AFTER/INSTEAD OF, FOR EACH ROW/STATEMENT, WHEN clause)
/// - **MySQL**: Basic support (BEFORE/AFTER, FOR EACH ROW only, FOLLOWS/PRECEDES)
/// - **SQLite**: Basic support (BEFORE/AFTER/INSTEAD OF, FOR EACH ROW, WHEN clause)
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
/// use reinhardt_query::types::{TriggerEvent, TriggerTiming, TriggerScope, TriggerBody};
///
/// // PostgreSQL: Call a function when a row is inserted
/// let query = Query::create_trigger()
///     .name("audit_insert")
///     .timing(TriggerTiming::After)
///     .event(TriggerEvent::Insert)
///     .on_table("users")
///     .for_each(TriggerScope::Row)
///     .execute_function("audit_log_insert");
///
/// // MySQL: Multiple statements on update
/// let query = Query::create_trigger()
///     .name("update_timestamp")
///     .timing(TriggerTiming::Before)
///     .event(TriggerEvent::Update { columns: None })
///     .on_table("users")
///     .for_each(TriggerScope::Row)
///     .body(TriggerBody::multiple(vec![
///         "SET NEW.updated_at = NOW()",
///     ]));
/// ```
#[derive(Debug, Clone)]
pub struct CreateTriggerStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) timing: Option<TriggerTiming>,
	pub(crate) events: Vec<TriggerEvent>,
	pub(crate) table: Option<TableRef>,
	pub(crate) scope: Option<TriggerScope>,
	pub(crate) when_condition: Option<SimpleExpr>,
	pub(crate) body: Option<TriggerBody>,
	pub(crate) order: Option<TriggerOrder>,
}

impl CreateTriggerStatement {
	/// Create a new CREATE TRIGGER statement
	pub fn new() -> Self {
		Self {
			name: None,
			timing: None,
			events: Vec::new(),
			table: None,
			scope: None,
			when_condition: None,
			body: None,
			order: None,
		}
	}

	/// Take the ownership of data in the current [`CreateTriggerStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			timing: self.timing.take(),
			events: std::mem::take(&mut self.events),
			table: self.table.take(),
			scope: self.scope.take(),
			when_condition: self.when_condition.take(),
			body: self.body.take(),
			order: self.order.take(),
		}
	}

	/// Set the trigger name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_trigger()
	///     .name("audit_insert");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Set the trigger timing (BEFORE, AFTER, INSTEAD OF)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::TriggerTiming;
	///
	/// let query = Query::create_trigger()
	///     .timing(TriggerTiming::Before);
	/// ```
	pub fn timing(&mut self, timing: TriggerTiming) -> &mut Self {
		self.timing = Some(timing);
		self
	}

	/// Add a trigger event (INSERT, UPDATE, DELETE)
	///
	/// Can be called multiple times to add multiple events (PostgreSQL only).
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::TriggerEvent;
	///
	/// let query = Query::create_trigger()
	///     .event(TriggerEvent::Insert)
	///     .event(TriggerEvent::Update { columns: None });
	/// ```
	pub fn event(&mut self, event: TriggerEvent) -> &mut Self {
		self.events.push(event);
		self
	}

	/// Set the table on which the trigger operates
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_trigger()
	///     .on_table("users");
	/// ```
	pub fn on_table<T>(&mut self, table: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.table = Some(table.into_table_ref());
		self
	}

	/// Set the trigger scope (FOR EACH ROW or FOR EACH STATEMENT)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::TriggerScope;
	///
	/// let query = Query::create_trigger()
	///     .for_each(TriggerScope::Row);
	/// ```
	pub fn for_each(&mut self, scope: TriggerScope) -> &mut Self {
		self.scope = Some(scope);
		self
	}

	/// Set the WHEN condition (optional filter for when trigger fires)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_trigger()
	///     .when_condition(Expr::col("status").eq("active"));
	/// ```
	pub fn when_condition(&mut self, condition: SimpleExpr) -> &mut Self {
		self.when_condition = Some(condition);
		self
	}

	/// Set the trigger body (SQL statements to execute)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::TriggerBody;
	///
	/// let query = Query::create_trigger()
	///     .body(TriggerBody::single("UPDATE counters SET count = count + 1"));
	/// ```
	pub fn body(&mut self, body: TriggerBody) -> &mut Self {
		self.body = Some(body);
		self
	}

	/// Set the PostgreSQL function to execute (EXECUTE FUNCTION function_name())
	///
	/// This is a convenience method for PostgreSQL triggers.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_trigger()
	///     .execute_function("audit_log_insert");
	/// ```
	pub fn execute_function<S: Into<String>>(&mut self, function_name: S) -> &mut Self {
		self.body = Some(TriggerBody::postgres_function(function_name));
		self
	}

	/// Set the MySQL trigger order (FOLLOWS or PRECEDES)
	///
	/// MySQL-specific feature to control trigger execution order.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::TriggerOrder;
	///
	/// let query = Query::create_trigger()
	///     .order(TriggerOrder::Follows("other_trigger".to_string()));
	/// ```
	pub fn order(&mut self, order: TriggerOrder) -> &mut Self {
		self.order = Some(order);
		self
	}
}

impl Default for CreateTriggerStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateTriggerStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_trigger(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_trigger(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_trigger(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateTriggerStatement {}
