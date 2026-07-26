//! Model-rooted expressions for type-safe ORM queries.

use crate::orm::Model;
use reinhardt_query::prelude::{ExprTrait, Order, SimpleExpr};
use std::marker::PhantomData;

/// A SQL expression rooted in model `M` and producing values of type `R`.
#[derive(Debug, Clone)]
pub struct TypedExpression<M, R> {
	pub(crate) expr: SimpleExpr,
	marker: PhantomData<fn() -> (M, R)>,
}

impl<M, R> TypedExpression<M, R> {
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
