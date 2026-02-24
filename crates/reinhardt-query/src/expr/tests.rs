//! Integration tests for the expression module.
//!
//! These tests verify that all components of the expression system
//! work together correctly.

use super::*;
use crate::types::BinOper;
use crate::value::Value;
use crate::{all, any};
use rstest::rstest;

// =============================================================================
// Integration tests: Expr + ExprTrait
// =============================================================================

#[rstest]
fn test_expr_trait_integration() {
	// Verify that Expr implements ExprTrait correctly
	let expr = Expr::col("age").gte(18);
	assert!(matches!(
		expr,
		SimpleExpr::Binary(_, BinOper::GreaterThanOrEqual, _)
	));
}

#[rstest]
fn test_simple_expr_trait_integration() {
	// Verify that SimpleExpr implements ExprTrait correctly
	let simple = SimpleExpr::Column(crate::types::ColumnRef::column("age"));
	let expr = simple.gte(18);
	assert!(matches!(
		expr,
		SimpleExpr::Binary(_, BinOper::GreaterThanOrEqual, _)
	));
}

// =============================================================================
// Integration tests: Expr + Condition
// =============================================================================

#[rstest]
fn test_condition_with_expr() {
	let cond = Cond::all()
		.add(Expr::col("active").eq(true))
		.add(Expr::col("verified").eq(true));

	assert_eq!(cond.condition_type, ConditionType::All);
	assert_eq!(cond.len(), 2);
}

#[rstest]
fn test_nested_condition_with_expr() {
	let cond = Cond::all().add(Expr::col("active").eq(true)).add(
		Cond::any()
			.add(Expr::col("role").eq("admin"))
			.add(Expr::col("role").eq("moderator")),
	);

	assert_eq!(cond.len(), 2);
	// Verify nested condition
	if let ConditionExpression::Condition(inner) = &cond.conditions[1] {
		assert_eq!(inner.condition_type, ConditionType::Any);
		assert_eq!(inner.len(), 2);
	} else {
		panic!("Expected nested Condition");
	}
}

#[rstest]
fn test_condition_with_negation() {
	let cond = Cond::all().add(Expr::col("deleted").eq(true)).not();

	assert!(cond.negate);
	assert_eq!(cond.len(), 1);
}

// =============================================================================
// Integration tests: Expr + SimpleExpr conversions
// =============================================================================

#[rstest]
fn test_expr_to_simple_expr() {
	let expr = Expr::col("name");
	let simple: SimpleExpr = expr.into();
	assert!(matches!(simple, SimpleExpr::Column(_)));
}

#[rstest]
fn test_simple_expr_to_expr() {
	let simple = SimpleExpr::Value(Value::Int(Some(42)));
	let expr: Expr = simple.into();
	assert!(matches!(expr.into_simple_expr(), SimpleExpr::Value(_)));
}

// =============================================================================
// Integration tests: Complex expressions
// =============================================================================

#[rstest]
fn test_complex_where_clause() {
	// Build: active = true AND (role = 'admin' OR role = 'moderator') AND age >= 18
	let cond = Cond::all()
		.add(Expr::col("active").eq(true))
		.add(
			Cond::any()
				.add(Expr::col("role").eq("admin"))
				.add(Expr::col("role").eq("moderator")),
		)
		.add(Expr::col("age").gte(18));

	assert_eq!(cond.len(), 3);
	assert_eq!(cond.condition_type, ConditionType::All);
}

#[rstest]
fn test_case_expression_in_condition() {
	let case_expr = Expr::case()
		.when(Expr::col("status").eq("active"), 1i32)
		.when(Expr::col("status").eq("pending"), 2i32)
		.else_result(0i32);

	// Can use case expression in condition
	let cond = Cond::all().add(case_expr.into_simple_expr().eq(1));
	assert_eq!(cond.len(), 1);
}

#[rstest]
fn test_arithmetic_expression_chain() {
	// Build: (price * quantity) + tax
	let expr = Expr::col("price")
		.mul(Expr::col("quantity"))
		.add(Expr::col("tax"));

	assert!(matches!(expr, SimpleExpr::Binary(_, BinOper::Add, _)));
}

#[rstest]
fn test_pattern_matching_helpers() {
	// Test starts_with
	let expr1 = Expr::col("name").starts_with("John");
	if let SimpleExpr::Binary(_, BinOper::Like, rhs) = expr1 {
		if let SimpleExpr::Value(Value::String(Some(s))) = *rhs {
			assert_eq!(*s, "John%");
		} else {
			panic!("Expected String value in LIKE pattern");
		}
	} else {
		panic!("Expected LIKE with 'John%' pattern");
	}

	// Test ends_with
	let expr2 = Expr::col("email").ends_with("@example.com");
	if let SimpleExpr::Binary(_, BinOper::Like, rhs) = expr2 {
		if let SimpleExpr::Value(Value::String(Some(s))) = *rhs {
			assert_eq!(*s, "%@example.com");
		} else {
			panic!("Expected String value in LIKE pattern");
		}
	} else {
		panic!("Expected LIKE with '%@example.com' pattern");
	}

	// Test contains
	let expr3 = Expr::col("description").contains("important");
	if let SimpleExpr::Binary(_, BinOper::Like, rhs) = expr3 {
		if let SimpleExpr::Value(Value::String(Some(s))) = *rhs {
			assert_eq!(*s, "%important%");
		} else {
			panic!("Expected String value in LIKE pattern");
		}
	} else {
		panic!("Expected LIKE with '%important%' pattern");
	}
}

// =============================================================================
// Integration tests: ConditionHolder
// =============================================================================

#[rstest]
fn test_condition_holder_build() {
	let mut holder = ConditionHolder::new();
	holder.add_and(Expr::col("active").eq(true));
	holder.add_and(Expr::col("verified").eq(true));

	let cond = holder.into_condition();
	assert!(cond.is_some());

	let cond = cond.unwrap();
	assert_eq!(cond.condition_type, ConditionType::All);
}

#[rstest]
fn test_condition_holder_or() {
	let mut holder = ConditionHolder::new();
	holder.add_and(Expr::col("active").eq(true));
	holder.add_or(Expr::col("superuser").eq(true));

	let cond = holder.into_condition();
	assert!(cond.is_some());
}

#[rstest]
fn test_condition_holder_set_condition() {
	let mut holder = ConditionHolder::new();
	holder.add_and(Expr::col("temp").eq(true)); // This will be replaced

	let new_cond = Cond::all()
		.add(Expr::col("active").eq(true))
		.add(Expr::col("verified").eq(true));

	holder.set_condition(new_cond);

	let cond = holder.into_condition();
	assert!(cond.is_some());
	let cond = cond.unwrap();
	assert_eq!(cond.len(), 2);
}

// =============================================================================
// Integration tests: Macros
// =============================================================================

#[rstest]
fn test_all_macro_integration() {
	let cond = all![
		Expr::col("active").eq(true),
		Expr::col("verified").eq(true),
		Expr::col("age").gte(18),
	];

	assert_eq!(cond.condition_type, ConditionType::All);
	assert_eq!(cond.len(), 3);
}

#[rstest]
fn test_any_macro_integration() {
	let cond = any![
		Expr::col("role").eq("admin"),
		Expr::col("role").eq("moderator"),
		Expr::col("role").eq("support"),
	];

	assert_eq!(cond.condition_type, ConditionType::Any);
	assert_eq!(cond.len(), 3);
}

#[rstest]
fn test_nested_macros() {
	let cond = all![
		Expr::col("active").eq(true),
		any![
			Expr::col("role").eq("admin"),
			Expr::col("role").eq("moderator"),
		],
	];

	assert_eq!(cond.len(), 2);
	if let ConditionExpression::Condition(inner) = &cond.conditions[1] {
		assert_eq!(inner.condition_type, ConditionType::Any);
	} else {
		panic!("Expected nested Condition");
	}
}
