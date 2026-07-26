//! Model-rooted expressions for type-safe ORM queries.

use crate::orm::Model;
use reinhardt_query::prelude::{Alias, ColumnRef, ExprTrait, IntoIden, Order, SimpleExpr};
use std::marker::PhantomData;

pub(crate) const TYPED_MODEL_ROOT_ALIAS: &str = "__reinhardt_typed_model_root__";

pub(crate) fn qualify_model_root(expr: &SimpleExpr, root_alias: &str) -> SimpleExpr {
	let mut qualified = expr.clone();
	qualify_model_root_in_place(&mut qualified, root_alias);
	qualified
}

fn qualify_model_root_in_place(expr: &mut SimpleExpr, root_alias: &str) {
	match expr {
		SimpleExpr::Column(ColumnRef::TableColumn(table, _))
		| SimpleExpr::TableColumn(table, _)
			if table.to_string() == TYPED_MODEL_ROOT_ALIAS =>
		{
			*table = Alias::new(root_alias).into_iden();
		}
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _)
		| SimpleExpr::WindowNamed {
			func: expression, ..
		} => qualify_model_root_in_place(expression, root_alias),
		SimpleExpr::Binary(left, _, right) => {
			qualify_model_root_in_place(left, root_alias);
			qualify_model_root_in_place(right, root_alias);
		}
		SimpleExpr::FunctionCall(_, expressions)
		| SimpleExpr::Tuple(expressions)
		| SimpleExpr::CustomWithExpr(_, expressions) => {
			for expression in expressions {
				qualify_model_root_in_place(expression, root_alias);
			}
		}
		SimpleExpr::Case(case) => {
			for (condition, result) in &mut case.when_clauses {
				qualify_model_root_in_place(condition, root_alias);
				qualify_model_root_in_place(result, root_alias);
			}
			if let Some(result) = &mut case.else_clause {
				qualify_model_root_in_place(result, root_alias);
			}
		}
		SimpleExpr::Window { func, window } => {
			qualify_model_root_in_place(func, root_alias);
			for expression in &mut window.partition_by {
				qualify_model_root_in_place(expression, root_alias);
			}
			for ordering in &mut window.order_by {
				if let reinhardt_query::types::OrderExprKind::Expr(expression) = &mut ordering.expr
				{
					qualify_model_root_in_place(expression, root_alias);
				}
			}
		}
		_ => {}
	}
}

/// A SQL expression rooted in model `M` and producing values of type `R`.
#[derive(Debug, Clone)]
pub struct TypedExpression<M, R> {
	pub(crate) expr: SimpleExpr,
	marker: PhantomData<fn() -> (M, R)>,
}

impl<M, R> TypedExpression<M, R> {
	#[cfg(feature = "pgvector")]
	pub(crate) fn new(expr: SimpleExpr) -> Self {
		Self {
			expr,
			marker: PhantomData,
		}
	}

	/// Order this expression in ascending order.
	pub fn asc(self) -> OrderedExpression<M> {
		OrderedExpression::new(self.expr, Order::Asc)
	}

	/// Order this expression in descending order.
	pub fn desc(self) -> OrderedExpression<M> {
		OrderedExpression::new(self.expr, Order::Desc)
	}
}

impl<M> TypedExpression<M, f64> {
	/// Compare this numeric expression for equality.
	pub fn eq(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.expr.eq(value))
	}

	/// Compare this numeric expression using less-than.
	pub fn lt(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.expr.lt(value))
	}

	/// Compare this numeric expression using less-than-or-equal.
	pub fn le(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.expr.lte(value))
	}

	/// Compare this numeric expression using greater-than.
	pub fn gt(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.expr.gt(value))
	}

	/// Compare this numeric expression using greater-than-or-equal.
	pub fn ge(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.expr.gte(value))
	}
}

/// A boolean SQL expression rooted in model `M`.
#[derive(Debug, Clone)]
pub struct TypedPredicate<M> {
	pub(crate) expr: SimpleExpr,
	marker: PhantomData<fn() -> M>,
}

impl<M> TypedPredicate<M> {
	fn new(expr: SimpleExpr) -> Self {
		Self {
			expr,
			marker: PhantomData,
		}
	}
}

/// A model-rooted expression with an ordering direction.
#[derive(Debug, Clone)]
pub struct OrderedExpression<M> {
	pub(crate) expr: SimpleExpr,
	pub(crate) order: Order,
	marker: PhantomData<fn() -> M>,
}

impl<M> OrderedExpression<M> {
	fn new(expr: SimpleExpr, order: Order) -> Self {
		Self {
			expr,
			order,
			marker: PhantomData,
		}
	}
}

impl<M> crate::orm::query::QueryFilterInput<M> for TypedPredicate<M>
where
	M: Model,
{
	fn into_filter_condition(self) -> crate::orm::query::FilterCondition {
		crate::orm::query::FilterCondition::Single(crate::orm::query::Filter::typed_predicate(
			self.expr,
		))
	}
}
