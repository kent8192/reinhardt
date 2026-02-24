//! ExprTrait - Expression operations trait.
//!
//! This module provides [`ExprTrait`], which defines all the operations
//! that can be performed on expressions (comparisons, logical ops, etc.).

use super::simple_expr::{Keyword, SimpleExpr};
use crate::types::{BinOper, UnOper};
use crate::value::Value;

/// Trait for expression operations.
///
/// This trait provides methods for building complex expressions through
/// operator chaining. It is implemented for [`Expr`](crate::expr::Expr) and [`SimpleExpr`].
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_query::{Expr, ExprTrait};
///
/// // Comparison
/// let expr = Expr::col("age").gte(18);
///
/// // Logical operations
/// let expr = Expr::col("active").eq(true).and(Expr::col("verified").eq(true));
///
/// // Arithmetic
/// let expr = Expr::col("price").mul(Expr::col("quantity"));
/// ```
// Expression trait methods consume self for builder-pattern chaining,
// so is_*/as_* methods intentionally take self by value.
#[allow(clippy::wrong_self_convention)]
pub trait ExprTrait: Sized {
	/// Build the final SimpleExpr.
	fn into_simple_expr(self) -> SimpleExpr;

	// =========================================================================
	// Comparison operations
	// =========================================================================

	/// Equal (`=`).
	///
	/// # Example
	///
	/// ```rust,ignore
	/// Expr::col("name").eq("Alice")
	/// // Generates: "name" = 'Alice'
	/// ```
	fn eq<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Equal,
			Box::new(v.into()),
		)
	}

	/// Not equal (`<>`).
	fn ne<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::NotEqual,
			Box::new(v.into()),
		)
	}

	/// Less than (`<`).
	fn lt<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::SmallerThan,
			Box::new(v.into()),
		)
	}

	/// Less than or equal (`<=`).
	fn lte<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::SmallerThanOrEqual,
			Box::new(v.into()),
		)
	}

	/// Greater than (`>`).
	fn gt<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::GreaterThan,
			Box::new(v.into()),
		)
	}

	/// Greater than or equal (`>=`).
	fn gte<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::GreaterThanOrEqual,
			Box::new(v.into()),
		)
	}

	// =========================================================================
	// NULL checks
	// =========================================================================

	/// IS NULL.
	fn is_null(self) -> SimpleExpr {
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Is,
			Box::new(SimpleExpr::Constant(Keyword::Null)),
		)
	}

	/// IS NOT NULL.
	fn is_not_null(self) -> SimpleExpr {
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::IsNot,
			Box::new(SimpleExpr::Constant(Keyword::Null)),
		)
	}

	// =========================================================================
	// Range operations
	// =========================================================================

	/// BETWEEN.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// Expr::col("age").between(18, 65)
	/// // Generates: "age" BETWEEN 18 AND 65
	/// ```
	fn between<A, B>(self, a: A, b: B) -> SimpleExpr
	where
		A: Into<SimpleExpr>,
		B: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Between,
			Box::new(SimpleExpr::Tuple(vec![a.into(), b.into()])),
		)
	}

	/// NOT BETWEEN.
	fn not_between<A, B>(self, a: A, b: B) -> SimpleExpr
	where
		A: Into<SimpleExpr>,
		B: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::NotBetween,
			Box::new(SimpleExpr::Tuple(vec![a.into(), b.into()])),
		)
	}

	// =========================================================================
	// Set membership
	// =========================================================================

	/// IN.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// Expr::col("status").is_in(["active", "pending"])
	/// // Generates: "status" IN ('active', 'pending')
	/// ```
	fn is_in<I, V>(self, values: I) -> SimpleExpr
	where
		I: IntoIterator<Item = V>,
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::In,
			Box::new(SimpleExpr::Tuple(
				values.into_iter().map(|v| v.into()).collect(),
			)),
		)
	}

	/// NOT IN.
	fn is_not_in<I, V>(self, values: I) -> SimpleExpr
	where
		I: IntoIterator<Item = V>,
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::NotIn,
			Box::new(SimpleExpr::Tuple(
				values.into_iter().map(|v| v.into()).collect(),
			)),
		)
	}

	// =========================================================================
	// Pattern matching
	// =========================================================================

	/// LIKE.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// Expr::col("name").like("%john%")
	/// // Generates: "name" LIKE '%john%'
	/// ```
	fn like<V>(self, pattern: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Like,
			Box::new(pattern.into()),
		)
	}

	/// NOT LIKE.
	fn not_like<V>(self, pattern: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::NotLike,
			Box::new(pattern.into()),
		)
	}

	/// ILIKE (case-insensitive LIKE, PostgreSQL).
	fn ilike<V>(self, pattern: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::ILike,
			Box::new(pattern.into()),
		)
	}

	/// NOT ILIKE (PostgreSQL).
	fn not_ilike<V>(self, pattern: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::NotILike,
			Box::new(pattern.into()),
		)
	}

	/// Helper for LIKE with prefix wildcard.
	fn starts_with<S>(self, prefix: S) -> SimpleExpr
	where
		S: Into<String>,
	{
		let pattern = format!("{}%", prefix.into());
		self.like(Value::String(Some(Box::new(pattern))))
	}

	/// Helper for LIKE with suffix wildcard.
	fn ends_with<S>(self, suffix: S) -> SimpleExpr
	where
		S: Into<String>,
	{
		let pattern = format!("%{}", suffix.into());
		self.like(Value::String(Some(Box::new(pattern))))
	}

	/// Helper for LIKE with both wildcards.
	fn contains<S>(self, substring: S) -> SimpleExpr
	where
		S: Into<String>,
	{
		let pattern = format!("%{}%", substring.into());
		self.like(Value::String(Some(Box::new(pattern))))
	}

	// =========================================================================
	// Logical operations
	// =========================================================================

	/// AND.
	fn and<E>(self, other: E) -> SimpleExpr
	where
		E: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::And,
			Box::new(other.into()),
		)
	}

	/// OR.
	fn or<E>(self, other: E) -> SimpleExpr
	where
		E: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Or,
			Box::new(other.into()),
		)
	}

	/// NOT (unary).
	fn not(self) -> SimpleExpr {
		SimpleExpr::Unary(UnOper::Not, Box::new(self.into_simple_expr()))
	}

	// =========================================================================
	// Arithmetic operations
	// =========================================================================

	/// Addition (`+`).
	fn add<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Add,
			Box::new(v.into()),
		)
	}

	/// Subtraction (`-`).
	fn sub<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Sub,
			Box::new(v.into()),
		)
	}

	/// Multiplication (`*`).
	fn mul<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Mul,
			Box::new(v.into()),
		)
	}

	/// Division (`/`).
	fn div<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Div,
			Box::new(v.into()),
		)
	}

	/// Modulo (`%`).
	fn modulo<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::Mod,
			Box::new(v.into()),
		)
	}

	// =========================================================================
	// Bitwise operations
	// =========================================================================

	/// Bitwise AND (`&`).
	fn bit_and<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::BitAnd,
			Box::new(v.into()),
		)
	}

	/// Bitwise OR (`|`).
	fn bit_or<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::BitOr,
			Box::new(v.into()),
		)
	}

	/// Left shift (`<<`).
	fn left_shift<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::LShift,
			Box::new(v.into()),
		)
	}

	/// Right shift (`>>`).
	fn right_shift<V>(self, v: V) -> SimpleExpr
	where
		V: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(
			Box::new(self.into_simple_expr()),
			BinOper::RShift,
			Box::new(v.into()),
		)
	}

	// =========================================================================
	// Type casting
	// =========================================================================

	/// CAST expression.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// Expr::col("age").cast_as("TEXT")
	/// // Generates: CAST("age" AS TEXT)
	/// ```
	fn cast_as<T>(self, type_name: T) -> SimpleExpr
	where
		T: crate::types::IntoIden,
	{
		SimpleExpr::Cast(Box::new(self.into_simple_expr()), type_name.into_iden())
	}

	/// AS ENUM expression (PostgreSQL).
	fn as_enum<T>(self, type_name: T) -> SimpleExpr
	where
		T: crate::types::IntoIden,
	{
		SimpleExpr::AsEnum(type_name.into_iden(), Box::new(self.into_simple_expr()))
	}
}

// Implement ExprTrait for SimpleExpr
impl ExprTrait for SimpleExpr {
	fn into_simple_expr(self) -> SimpleExpr {
		self
	}
}

// Implement ExprTrait for Expr
impl ExprTrait for super::expr::Expr {
	fn into_simple_expr(self) -> SimpleExpr {
		self.into_simple_expr()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::expr::Expr;
	use rstest::rstest;

	#[rstest]
	fn test_eq() {
		let expr = Expr::col("name").eq("Alice");
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Equal, _)));
	}

	#[rstest]
	fn test_ne() {
		let expr = Expr::col("name").ne("Bob");
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::NotEqual, _)));
	}

	#[rstest]
	fn test_lt() {
		let expr = Expr::col("age").lt(18);
		assert!(matches!(
			expr,
			SimpleExpr::Binary(_, BinOper::SmallerThan, _)
		));
	}

	#[rstest]
	fn test_lte() {
		let expr = Expr::col("age").lte(65);
		assert!(matches!(
			expr,
			SimpleExpr::Binary(_, BinOper::SmallerThanOrEqual, _)
		));
	}

	#[rstest]
	fn test_gt() {
		let expr = Expr::col("age").gt(18);
		assert!(matches!(
			expr,
			SimpleExpr::Binary(_, BinOper::GreaterThan, _)
		));
	}

	#[rstest]
	fn test_gte() {
		let expr = Expr::col("age").gte(18);
		assert!(matches!(
			expr,
			SimpleExpr::Binary(_, BinOper::GreaterThanOrEqual, _)
		));
	}

	#[rstest]
	fn test_is_null() {
		let expr = Expr::col("deleted_at").is_null();
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Is, _)));
	}

	#[rstest]
	fn test_is_not_null() {
		let expr = Expr::col("name").is_not_null();
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::IsNot, _)));
	}

	#[rstest]
	fn test_between() {
		let expr = Expr::col("age").between(18, 65);
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Between, _)));
	}

	#[rstest]
	fn test_not_between() {
		let expr = Expr::col("age").not_between(0, 17);
		assert!(matches!(
			expr,
			SimpleExpr::Binary(_, BinOper::NotBetween, _)
		));
	}

	#[rstest]
	fn test_is_in() {
		let expr = Expr::col("status").is_in(["active", "pending"]);
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::In, _)));
	}

	#[rstest]
	fn test_is_not_in() {
		let expr = Expr::col("status").is_not_in(["deleted", "banned"]);
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::NotIn, _)));
	}

	#[rstest]
	fn test_like() {
		let expr = Expr::col("name").like("%john%");
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Like, _)));
	}

	#[rstest]
	fn test_not_like() {
		let expr = Expr::col("name").not_like("%admin%");
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::NotLike, _)));
	}

	#[rstest]
	fn test_starts_with() {
		let expr = Expr::col("name").starts_with("John");
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Like, _)));
	}

	#[rstest]
	fn test_ends_with() {
		let expr = Expr::col("email").ends_with("@example.com");
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Like, _)));
	}

	#[rstest]
	fn test_contains() {
		let expr = Expr::col("description").contains("important");
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Like, _)));
	}

	#[rstest]
	fn test_and() {
		let expr = Expr::col("active")
			.eq(true)
			.and(Expr::col("verified").eq(true));
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::And, _)));
	}

	#[rstest]
	fn test_or() {
		let expr = Expr::col("role")
			.eq("admin")
			.or(Expr::col("role").eq("moderator"));
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Or, _)));
	}

	#[rstest]
	fn test_not() {
		let expr = Expr::col("deleted").not();
		assert!(matches!(expr, SimpleExpr::Unary(UnOper::Not, _)));
	}

	#[rstest]
	fn test_add() {
		let expr = Expr::col("price").add(10);
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Add, _)));
	}

	#[rstest]
	fn test_sub() {
		let expr = Expr::col("quantity").sub(1);
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Sub, _)));
	}

	#[rstest]
	fn test_mul() {
		let expr = Expr::col("price").mul(Expr::col("quantity"));
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Mul, _)));
	}

	#[rstest]
	fn test_div() {
		let expr = Expr::col("total").div(Expr::col("count"));
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Div, _)));
	}

	#[rstest]
	fn test_modulo() {
		let expr = Expr::col("value").modulo(2);
		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Mod, _)));
	}

	#[rstest]
	fn test_cast_as() {
		let expr = Expr::col("age").cast_as("TEXT");
		assert!(matches!(expr, SimpleExpr::Cast(_, _)));
	}

	#[rstest]
	fn test_chained_operations() {
		// Test complex expression chaining
		let expr = Expr::col("age")
			.gte(18)
			.and(Expr::col("active").eq(true))
			.and(Expr::col("verified").is_not_null());

		assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::And, _)));
	}
}
