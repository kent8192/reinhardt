//! Code generation for guard expressions.
//!
//! Converts a [`GuardExpr`] AST into a `proc_macro2::TokenStream` representing
//! a Rust TYPE suitable for use as a `Guard<...>` parameter.

use proc_macro2::TokenStream;
use quote::quote;

use crate::guard_parser::GuardExpr;

/// Converts a [`GuardExpr`] AST into a [`TokenStream`] representing the guard type.
///
/// The generated type wraps the expression in `reinhardt_auth::guard::Guard<...>`.
pub(crate) fn generate_guard_type(expr: &GuardExpr) -> TokenStream {
	let inner = generate_inner_type(expr);
	quote! { reinhardt_auth::guard::Guard<#inner> }
}

/// Recursively generates the inner permission type from the AST.
fn generate_inner_type(expr: &GuardExpr) -> TokenStream {
	match expr {
		GuardExpr::TypePath(segments) => {
			let idents: Vec<proc_macro2::Ident> = segments
				.iter()
				.map(|s| proc_macro2::Ident::new(s, proc_macro2::Span::call_site()))
				.collect();
			quote! { #(#idents)::* }
		}
		GuardExpr::And(exprs) => fold_binary(exprs, |a, b| {
			quote! { reinhardt_auth::guard::All<(#a, #b)> }
		}),
		GuardExpr::Or(exprs) => fold_binary(exprs, |a, b| {
			quote! { reinhardt_auth::guard::Any<(#a, #b)> }
		}),
		GuardExpr::Not(inner) => {
			let inner_ts = generate_inner_type(inner);
			quote! { reinhardt_auth::guard::Not<#inner_ts> }
		}
		GuardExpr::HasPerm(_perm) => {
			quote! {
				compile_error!(
					"HasPerm(\"...\") is not yet supported in guard!() macro. \
					 Define a custom Permission type instead."
				)
			}
		}
	}
}

/// Left-folds a list of expressions with a binary combinator.
///
/// For `[A, B, C]`, produces `Combinator<(Combinator<(A, B)>, C)>`.
fn fold_binary<F>(exprs: &[GuardExpr], combinator: F) -> TokenStream
where
	F: Fn(TokenStream, TokenStream) -> TokenStream,
{
	assert!(
		exprs.len() >= 2,
		"And/Or combinator requires at least 2 operands"
	);

	let mut iter = exprs.iter();
	let first = generate_inner_type(iter.next().unwrap());
	let second = generate_inner_type(iter.next().unwrap());
	let mut acc = combinator(first, second);

	for expr in iter {
		let next = generate_inner_type(expr);
		acc = combinator(acc, next);
	}

	acc
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn codegen_single_type() {
		// Arrange
		let expr = GuardExpr::TypePath(vec!["IsAdminUser".to_owned()]);

		// Act
		let result = generate_guard_type(&expr).to_string();

		// Assert
		assert_eq!(
			normalize_tokens(&result),
			normalize_tokens("reinhardt_auth :: guard :: Guard < IsAdminUser >")
		);
	}

	#[test]
	fn codegen_and_two() {
		// Arrange
		let expr = GuardExpr::And(vec![
			GuardExpr::TypePath(vec!["A".to_owned()]),
			GuardExpr::TypePath(vec!["B".to_owned()]),
		]);

		// Act
		let result = generate_guard_type(&expr).to_string();

		// Assert
		assert!(result.contains("All"));
		assert!(result.contains("Guard"));
	}

	#[test]
	fn codegen_and_three_folds_left() {
		// Arrange
		let expr = GuardExpr::And(vec![
			GuardExpr::TypePath(vec!["A".to_owned()]),
			GuardExpr::TypePath(vec!["B".to_owned()]),
			GuardExpr::TypePath(vec!["C".to_owned()]),
		]);

		// Act
		let result = generate_guard_type(&expr).to_string();

		// Assert: should contain nested All (left-folded)
		let normalized = normalize_tokens(&result);
		// Verify the structure contains nested All with left-folding
		assert!(normalized.contains("All"), "expected All combinator");
		assert!(normalized.contains("A"), "expected type A");
		assert!(normalized.contains("B"), "expected type B");
		assert!(normalized.contains("C"), "expected type C");
		// Count occurrences of "All" - should be at least 2 for left-folding
		let all_count = normalized.matches("All").count();
		assert_eq!(
			all_count, 2,
			"expected 2 nested All combinators for 3 operands"
		);
	}

	#[test]
	fn codegen_or() {
		// Arrange
		let expr = GuardExpr::Or(vec![
			GuardExpr::TypePath(vec!["A".to_owned()]),
			GuardExpr::TypePath(vec!["B".to_owned()]),
		]);

		// Act
		let result = generate_guard_type(&expr).to_string();

		// Assert
		assert!(result.contains("Any"));
	}

	#[test]
	fn codegen_not() {
		// Arrange
		let expr = GuardExpr::Not(Box::new(GuardExpr::TypePath(vec!["A".to_owned()])));

		// Act
		let result = generate_guard_type(&expr).to_string();

		// Assert
		assert!(result.contains("Not"));
	}

	#[test]
	fn codegen_has_perm_emits_compile_error() {
		// Arrange
		let expr = GuardExpr::HasPerm("blog.add".to_owned());

		// Act
		let result = generate_guard_type(&expr).to_string();

		// Assert
		assert!(result.contains("compile_error"));
	}

	#[test]
	fn codegen_type_path_with_module() {
		// Arrange
		let expr = GuardExpr::TypePath(vec!["my_mod".to_owned(), "MyPerm".to_owned()]);

		// Act
		let result = generate_guard_type(&expr).to_string();

		// Assert
		let normalized = normalize_tokens(&result);
		assert!(normalized.contains("my_mod :: MyPerm"));
	}

	/// Normalizes token stream string by collapsing whitespace.
	fn normalize_tokens(s: &str) -> String {
		s.split_whitespace().collect::<Vec<_>>().join(" ")
	}
}
