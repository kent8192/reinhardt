//! SQL aggregate function builders.
//!
//! This module provides the [`Func`] struct with static methods for
//! constructing common SQL aggregate function calls.

use super::simple_expr::SimpleExpr;
use crate::types::IntoIden;

/// SQL aggregate function builder.
///
/// Provides static methods for building common aggregate function expressions
/// such as COUNT, SUM, AVG, MIN, MAX, and COALESCE.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // COUNT(*)
/// let count_all = Func::count(Expr::asterisk().into_simple_expr());
///
/// // SUM(price)
/// let total = Func::sum(Expr::col("price").into_simple_expr());
///
/// // COALESCE(name, 'Unknown')
/// let name = Func::coalesce(vec![
///     Expr::col("name").into_simple_expr(),
///     Expr::val("Unknown").into_simple_expr(),
/// ]);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Func;

impl Func {
	/// Create a COUNT(expr) function call.
	pub fn count(expr: SimpleExpr) -> SimpleExpr {
		SimpleExpr::FunctionCall("COUNT".into_iden(), vec![expr])
	}

	/// Create a SUM(expr) function call.
	pub fn sum(expr: SimpleExpr) -> SimpleExpr {
		SimpleExpr::FunctionCall("SUM".into_iden(), vec![expr])
	}

	/// Create an AVG(expr) function call.
	pub fn avg(expr: SimpleExpr) -> SimpleExpr {
		SimpleExpr::FunctionCall("AVG".into_iden(), vec![expr])
	}

	/// Create a MIN(expr) function call.
	pub fn min(expr: SimpleExpr) -> SimpleExpr {
		SimpleExpr::FunctionCall("MIN".into_iden(), vec![expr])
	}

	/// Create a MAX(expr) function call.
	pub fn max(expr: SimpleExpr) -> SimpleExpr {
		SimpleExpr::FunctionCall("MAX".into_iden(), vec![expr])
	}

	/// Create a COALESCE(expr1, expr2, ...) function call.
	pub fn coalesce(exprs: Vec<SimpleExpr>) -> SimpleExpr {
		SimpleExpr::FunctionCall("COALESCE".into_iden(), exprs)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::expr::Expr;
	use crate::value::Value;
	use rstest::rstest;

	#[rstest]
	fn test_func_count_creates_function_call() {
		// Arrange
		let expr = Expr::asterisk().into_simple_expr();

		// Act
		let result = Func::count(expr);

		// Assert
		if let SimpleExpr::FunctionCall(name, args) = result {
			assert_eq!(name.to_string(), "COUNT");
			assert_eq!(args.len(), 1);
			assert!(matches!(args[0], SimpleExpr::Asterisk));
		} else {
			panic!("Expected FunctionCall variant");
		}
	}

	#[rstest]
	fn test_func_sum_creates_function_call() {
		// Arrange
		let expr = Expr::col("price").into_simple_expr();

		// Act
		let result = Func::sum(expr);

		// Assert
		if let SimpleExpr::FunctionCall(name, args) = result {
			assert_eq!(name.to_string(), "SUM");
			assert_eq!(args.len(), 1);
			assert!(matches!(args[0], SimpleExpr::Column(_)));
		} else {
			panic!("Expected FunctionCall variant");
		}
	}

	#[rstest]
	fn test_func_avg_creates_function_call() {
		// Arrange
		let expr = Expr::col("score").into_simple_expr();

		// Act
		let result = Func::avg(expr);

		// Assert
		if let SimpleExpr::FunctionCall(name, args) = result {
			assert_eq!(name.to_string(), "AVG");
			assert_eq!(args.len(), 1);
			assert!(matches!(args[0], SimpleExpr::Column(_)));
		} else {
			panic!("Expected FunctionCall variant");
		}
	}

	#[rstest]
	fn test_func_min_creates_function_call() {
		// Arrange
		let expr = Expr::col("age").into_simple_expr();

		// Act
		let result = Func::min(expr);

		// Assert
		if let SimpleExpr::FunctionCall(name, args) = result {
			assert_eq!(name.to_string(), "MIN");
			assert_eq!(args.len(), 1);
			assert!(matches!(args[0], SimpleExpr::Column(_)));
		} else {
			panic!("Expected FunctionCall variant");
		}
	}

	#[rstest]
	fn test_func_max_creates_function_call() {
		// Arrange
		let expr = Expr::col("salary").into_simple_expr();

		// Act
		let result = Func::max(expr);

		// Assert
		if let SimpleExpr::FunctionCall(name, args) = result {
			assert_eq!(name.to_string(), "MAX");
			assert_eq!(args.len(), 1);
			assert!(matches!(args[0], SimpleExpr::Column(_)));
		} else {
			panic!("Expected FunctionCall variant");
		}
	}

	#[rstest]
	fn test_func_coalesce_creates_function_call() {
		// Arrange
		let exprs = vec![
			Expr::col("name").into_simple_expr(),
			Expr::val("Unknown").into_simple_expr(),
		];

		// Act
		let result = Func::coalesce(exprs);

		// Assert
		if let SimpleExpr::FunctionCall(name, args) = result {
			assert_eq!(name.to_string(), "COALESCE");
			assert_eq!(args.len(), 2);
			assert!(matches!(args[0], SimpleExpr::Column(_)));
			assert!(matches!(args[1], SimpleExpr::Value(Value::String(Some(_)))));
		} else {
			panic!("Expected FunctionCall variant");
		}
	}

	#[rstest]
	fn test_func_count_with_column() {
		// Arrange
		let expr = Expr::col("id").into_simple_expr();

		// Act
		let result = Func::count(expr);

		// Assert
		if let SimpleExpr::FunctionCall(name, args) = result {
			assert_eq!(name.to_string(), "COUNT");
			assert_eq!(args.len(), 1);
			assert!(matches!(args[0], SimpleExpr::Column(_)));
		} else {
			panic!("Expected FunctionCall variant");
		}
	}
}
