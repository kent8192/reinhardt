//! Condition system for WHERE and HAVING clauses.
//!
//! This module provides [`Condition`] and [`Cond`] for building complex
//! filter conditions.

use super::simple_expr::SimpleExpr;
use crate::types::LogicalChainOper;

/// Type of condition combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConditionType {
	/// All conditions must be true (AND)
	#[default]
	All,
	/// Any condition must be true (OR)
	Any,
}

/// A single condition expression in a condition chain.
#[derive(Debug, Clone)]
pub enum ConditionExpression {
	/// A simple expression
	SimpleExpr(SimpleExpr),
	/// A nested condition
	Condition(Condition),
}

/// A condition chain for WHERE or HAVING clauses.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_query::{Cond, Expr};
///
/// // All conditions (AND)
/// let cond = Cond::all()
///     .add(Expr::col("active").eq(true))
///     .add(Expr::col("age").gte(18));
///
/// // Any condition (OR)
/// let cond = Cond::any()
///     .add(Expr::col("role").eq("admin"))
///     .add(Expr::col("role").eq("moderator"));
///
/// // Nested conditions
/// let cond = Cond::all()
///     .add(Expr::col("verified").eq(true))
///     .add(Cond::any()
///         .add(Expr::col("role").eq("admin"))
///         .add(Expr::col("role").eq("moderator")));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Condition {
	/// Type of condition chain (AND or OR)
	pub condition_type: ConditionType,
	/// Whether to negate the entire condition
	pub negate: bool,
	/// The conditions in this chain
	pub conditions: Vec<ConditionExpression>,
}

impl Condition {
	/// Create a new empty condition with the specified type.
	pub fn new(condition_type: ConditionType) -> Self {
		Self {
			condition_type,
			negate: false,
			conditions: Vec::new(),
		}
	}

	/// Create a new condition that requires all sub-conditions (AND).
	pub fn all() -> Self {
		Self::new(ConditionType::All)
	}

	/// Create a new condition that requires any sub-condition (OR).
	pub fn any() -> Self {
		Self::new(ConditionType::Any)
	}

	/// Add a condition expression.
	#[must_use]
	// Intentional builder-pattern method, not std::ops::Add
	#[allow(clippy::should_implement_trait)]
	pub fn add<C>(mut self, condition: C) -> Self
	where
		C: IntoCondition,
	{
		self.conditions.push(condition.into_condition_expression());
		self
	}

	/// Add a condition only if the option is Some.
	#[must_use]
	pub fn add_option<C>(self, condition: Option<C>) -> Self
	where
		C: IntoCondition,
	{
		if let Some(c) = condition {
			self.add(c)
		} else {
			self
		}
	}

	/// Negate the entire condition.
	#[must_use]
	// Intentional builder-pattern method, not std::ops::Not
	#[allow(clippy::should_implement_trait)]
	pub fn not(mut self) -> Self {
		self.negate = !self.negate;
		self
	}

	/// Returns true if this condition has no sub-conditions.
	pub fn is_empty(&self) -> bool {
		self.conditions.is_empty()
	}

	/// Returns the number of sub-conditions.
	pub fn len(&self) -> usize {
		self.conditions.len()
	}

	/// Returns the logical operator for this condition type.
	pub fn logical_oper(&self) -> LogicalChainOper {
		match self.condition_type {
			ConditionType::All => LogicalChainOper::And,
			ConditionType::Any => LogicalChainOper::Or,
		}
	}
}

/// Helper for creating conditions.
///
/// This is a convenience wrapper around [`Condition`].
pub struct Cond;

impl Cond {
	/// Create a condition that requires all sub-conditions (AND).
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::{Cond, Expr};
	///
	/// let cond = Cond::all()
	///     .add(Expr::col("active").eq(true))
	///     .add(Expr::col("verified").eq(true));
	/// ```
	pub fn all() -> Condition {
		Condition::all()
	}

	/// Create a condition that requires any sub-condition (OR).
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::{Cond, Expr};
	///
	/// let cond = Cond::any()
	///     .add(Expr::col("role").eq("admin"))
	///     .add(Expr::col("role").eq("moderator"));
	/// ```
	pub fn any() -> Condition {
		Condition::any()
	}
}

/// Trait for types that can be converted into a condition expression.
pub trait IntoCondition {
	/// Convert into a ConditionExpression.
	fn into_condition_expression(self) -> ConditionExpression;

	/// Convert into a Condition (wrapping if necessary).
	fn into_condition(self) -> Condition
	where
		Self: Sized,
	{
		let expr = self.into_condition_expression();
		match expr {
			ConditionExpression::Condition(c) => c,
			ConditionExpression::SimpleExpr(e) => Condition::all().add(e),
		}
	}
}

impl IntoCondition for Condition {
	fn into_condition_expression(self) -> ConditionExpression {
		ConditionExpression::Condition(self)
	}

	fn into_condition(self) -> Condition {
		self
	}
}

impl IntoCondition for SimpleExpr {
	fn into_condition_expression(self) -> ConditionExpression {
		ConditionExpression::SimpleExpr(self)
	}
}

impl IntoCondition for super::expr::Expr {
	fn into_condition_expression(self) -> ConditionExpression {
		ConditionExpression::SimpleExpr(self.into_simple_expr())
	}
}

/// Holder for conditions in query builders.
///
/// This is used internally by query builders to hold WHERE and HAVING clauses.
#[derive(Debug, Clone, Default)]
pub struct ConditionHolder {
	/// The conditions
	pub conditions: Vec<ConditionExpression>,
}

impl ConditionHolder {
	/// Create a new empty condition holder.
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a condition with AND.
	pub fn add_and<C>(&mut self, condition: C)
	where
		C: IntoCondition,
	{
		self.conditions.push(condition.into_condition_expression());
	}

	/// Add a condition with OR (wraps existing conditions).
	pub fn add_or<C>(&mut self, condition: C)
	where
		C: IntoCondition,
	{
		// If we have existing conditions, wrap them in an OR
		if !self.conditions.is_empty() {
			let existing = std::mem::take(&mut self.conditions);
			let mut or_cond = Condition::any();
			for c in existing {
				or_cond.conditions.push(c);
			}
			or_cond
				.conditions
				.push(condition.into_condition_expression());
			self.conditions
				.push(ConditionExpression::Condition(or_cond));
		} else {
			self.conditions.push(condition.into_condition_expression());
		}
	}

	/// Set all conditions from a Condition.
	pub fn set_condition(&mut self, condition: Condition) {
		self.conditions = vec![ConditionExpression::Condition(condition)];
	}

	/// Returns true if there are no conditions.
	pub fn is_empty(&self) -> bool {
		self.conditions.is_empty()
	}

	/// Returns the number of conditions.
	pub fn len(&self) -> usize {
		self.conditions.len()
	}

	/// Build into a single Condition.
	pub fn into_condition(mut self) -> Option<Condition> {
		if self.conditions.is_empty() {
			return None;
		}

		if self.conditions.len() == 1 {
			// Take the single condition out
			match self.conditions.pop() {
				Some(ConditionExpression::Condition(c)) => return Some(c),
				Some(ConditionExpression::SimpleExpr(e)) => {
					return Some(Condition::all().add(e));
				}
				None => return None,
			}
		}

		let mut cond = Condition::all();
		cond.conditions = self.conditions;
		Some(cond)
	}
}

/// Create an ALL (AND) condition from multiple expressions.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_query::{all, Expr};
///
/// let cond = all![
///     Expr::col("active").eq(true),
///     Expr::col("verified").eq(true),
/// ];
/// ```
#[macro_export]
macro_rules! all {
    ($($expr:expr),* $(,)?) => {
        {
            let mut cond = $crate::expr::Cond::all();
            $(
                cond = cond.add($expr);
            )*
            cond
        }
    };
}

/// Create an ANY (OR) condition from multiple expressions.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_query::{any, Expr};
///
/// let cond = any![
///     Expr::col("role").eq("admin"),
///     Expr::col("role").eq("moderator"),
/// ];
/// ```
#[macro_export]
macro_rules! any {
    ($($expr:expr),* $(,)?) => {
        {
            let mut cond = $crate::expr::Cond::any();
            $(
                cond = cond.add($expr);
            )*
            cond
        }
    };
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::expr::{Expr, ExprTrait};
	use rstest::rstest;

	#[rstest]
	fn test_condition_all() {
		let cond = Cond::all()
			.add(Expr::col("active").eq(true))
			.add(Expr::col("verified").eq(true));

		assert_eq!(cond.condition_type, ConditionType::All);
		assert_eq!(cond.len(), 2);
		assert!(!cond.is_empty());
	}

	#[rstest]
	fn test_condition_any() {
		let cond = Cond::any()
			.add(Expr::col("role").eq("admin"))
			.add(Expr::col("role").eq("moderator"));

		assert_eq!(cond.condition_type, ConditionType::Any);
		assert_eq!(cond.len(), 2);
	}

	#[rstest]
	fn test_condition_nested() {
		let cond = Cond::all().add(Expr::col("verified").eq(true)).add(
			Cond::any()
				.add(Expr::col("role").eq("admin"))
				.add(Expr::col("role").eq("moderator")),
		);

		assert_eq!(cond.len(), 2);
	}

	#[rstest]
	fn test_condition_not() {
		let cond = Cond::all().add(Expr::col("deleted").eq(true)).not();

		assert!(cond.negate);
	}

	#[rstest]
	fn test_condition_add_option() {
		let filter_active: Option<bool> = Some(true);
		let filter_role: Option<&str> = None;

		let cond = Cond::all()
			.add_option(filter_active.map(|v| Expr::col("active").eq(v)))
			.add_option(filter_role.map(|v| Expr::col("role").eq(v)));

		assert_eq!(cond.len(), 1); // Only active filter added
	}

	#[rstest]
	fn test_condition_empty() {
		let cond = Cond::all();
		assert!(cond.is_empty());
		assert_eq!(cond.len(), 0);
	}

	#[rstest]
	fn test_condition_holder() {
		let mut holder = ConditionHolder::new();
		assert!(holder.is_empty());

		holder.add_and(Expr::col("active").eq(true));
		holder.add_and(Expr::col("verified").eq(true));

		assert!(!holder.is_empty());
		assert_eq!(holder.len(), 2);
	}

	#[rstest]
	fn test_condition_holder_into_condition() {
		let mut holder = ConditionHolder::new();
		holder.add_and(Expr::col("active").eq(true));
		holder.add_and(Expr::col("verified").eq(true));

		let cond = holder.into_condition();
		assert!(cond.is_some());
	}

	#[rstest]
	fn test_all_macro() {
		let cond = all![Expr::col("a").eq(1), Expr::col("b").eq(2),];

		assert_eq!(cond.condition_type, ConditionType::All);
		assert_eq!(cond.len(), 2);
	}

	#[rstest]
	fn test_any_macro() {
		let cond = any![Expr::col("a").eq(1), Expr::col("b").eq(2),];

		assert_eq!(cond.condition_type, ConditionType::Any);
		assert_eq!(cond.len(), 2);
	}

	#[rstest]
	fn test_logical_oper() {
		assert_eq!(Cond::all().logical_oper(), LogicalChainOper::And);
		assert_eq!(Cond::any().logical_oper(), LogicalChainOper::Or);
	}
}
