//! Type definitions for typed AST nodes.
//!
//! This module provides type representations for attribute values,
//! enabling compile-time type checking of HTML attributes.

use syn::{Expr, ExprLit, Lit, LitBool, LitFloat, LitInt, LitStr};

/// Typed representation of attribute values.
///
/// This enum distinguishes between different types of attribute values,
/// allowing for type-specific validation during AST transformation.
///
/// # Examples
///
/// ```ignore
/// // String literal: src: "/image.png"
/// AttrValue::StringLit(syn::LitStr)
///
/// // Boolean literal: disabled: true
/// AttrValue::BoolLit(syn::LitBool)
///
/// // Dynamic expression: src: image_url
/// AttrValue::Dynamic(Expr)
/// ```
#[derive(Debug, Clone)]
pub enum AttrValue {
	/// String literal: "literal"
	StringLit(LitStr),

	/// Boolean literal: true, false
	BoolLit(LitBool),

	/// Integer literal: 42
	IntLit(LitInt),

	/// Floating-point literal: 3.14
	FloatLit(LitFloat),

	/// Dynamic expression: variables, function calls, etc.
	Dynamic(Expr),
}

impl AttrValue {
	/// Converts an `Expr` into a typed `AttrValue`.
	///
	/// This function examines the expression and categorizes it based on
	/// its literal type. Non-literal expressions are categorized as `Dynamic`.
	///
	/// # Arguments
	///
	/// * `expr` - The expression to convert
	///
	/// # Returns
	///
	/// A typed `AttrValue` representing the expression
	///
	/// # Examples
	///
	/// ```ignore
	/// use syn::parse_quote;
	///
	/// let expr = parse_quote!("hello");
	/// let value = AttrValue::from_expr(expr);
	/// assert!(matches!(value, AttrValue::StringLit(_)));
	///
	/// let expr = parse_quote!(true);
	/// let value = AttrValue::from_expr(expr);
	/// assert!(matches!(value, AttrValue::BoolLit(_)));
	///
	/// let expr = parse_quote!(variable_name);
	/// let value = AttrValue::from_expr(expr);
	/// assert!(matches!(value, AttrValue::Dynamic(_)));
	/// ```
	pub fn from_expr(expr: Expr) -> Self {
		match expr {
			Expr::Lit(ExprLit { lit, .. }) => match lit {
				Lit::Str(s) => Self::StringLit(s),
				Lit::Bool(b) => Self::BoolLit(b),
				Lit::Int(i) => Self::IntLit(i),
				Lit::Float(f) => Self::FloatLit(f),
				_ => Self::Dynamic(Expr::Lit(ExprLit { attrs: vec![], lit })),
			},
			_ => Self::Dynamic(expr),
		}
	}

	/// Checks if the value is a string literal.
	///
	/// # Returns
	///
	/// `true` if the value is a string literal, `false` otherwise
	///
	/// # Examples
	///
	/// ```ignore
	/// use syn::parse_quote;
	///
	/// let value = AttrValue::from_expr(parse_quote!("hello"));
	/// assert!(value.is_string_literal());
	///
	/// let value = AttrValue::from_expr(parse_quote!(variable));
	/// assert!(!value.is_string_literal());
	/// ```
	pub fn is_string_literal(&self) -> bool {
		matches!(self, Self::StringLit(_))
	}

	/// Checks if the value is a boolean literal.
	///
	/// # Returns
	///
	/// `true` if the value is a boolean literal, `false` otherwise
	pub fn is_bool_literal(&self) -> bool {
		matches!(self, Self::BoolLit(_))
	}

	/// Checks if the value is an integer literal.
	///
	/// # Returns
	///
	/// `true` if the value is an integer literal, `false` otherwise
	pub fn is_int_literal(&self) -> bool {
		matches!(self, Self::IntLit(_))
	}

	/// Checks if the value is a floating-point literal.
	///
	/// # Returns
	///
	/// `true` if the value is a floating-point literal, `false` otherwise
	pub fn is_float_literal(&self) -> bool {
		matches!(self, Self::FloatLit(_))
	}

	/// Checks if the value is a dynamic expression.
	///
	/// # Returns
	///
	/// `true` if the value is a dynamic expression, `false` otherwise
	pub fn is_dynamic(&self) -> bool {
		matches!(self, Self::Dynamic(_))
	}

	/// Converts the typed value back to an `Expr`.
	///
	/// This is useful for code generation, where we need to convert
	/// the typed representation back to a Rust expression.
	///
	/// # Returns
	///
	/// The original `Expr` representation
	///
	/// # Examples
	///
	/// ```ignore
	/// use syn::parse_quote;
	///
	/// let original: Expr = parse_quote!("hello");
	/// let value = AttrValue::from_expr(original.clone());
	/// let reconstructed = value.to_expr();
	/// // reconstructed is equivalent to original
	/// ```
	pub fn to_expr(&self) -> Expr {
		match self {
			Self::StringLit(s) => Expr::Lit(ExprLit {
				attrs: vec![],
				lit: Lit::Str(s.clone()),
			}),
			Self::BoolLit(b) => Expr::Lit(ExprLit {
				attrs: vec![],
				lit: Lit::Bool(b.clone()),
			}),
			Self::IntLit(i) => Expr::Lit(ExprLit {
				attrs: vec![],
				lit: Lit::Int(i.clone()),
			}),
			Self::FloatLit(f) => Expr::Lit(ExprLit {
				attrs: vec![],
				lit: Lit::Float(f.clone()),
			}),
			Self::Dynamic(e) => e.clone(),
		}
	}

	/// Gets the string value if this is a string literal.
	///
	/// # Returns
	///
	/// `Some(String)` if this is a string literal, `None` otherwise
	pub fn as_string(&self) -> Option<String> {
		match self {
			Self::StringLit(lit) => Some(lit.value()),
			_ => None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use syn::parse_quote;

	#[rstest]
	fn test_from_expr_string_lit() {
		// Arrange
		let expr: Expr = parse_quote!("hello");

		// Act
		let value = AttrValue::from_expr(expr);

		// Assert
		assert!(matches!(value, AttrValue::StringLit(_)));
		assert!(value.is_string_literal());
		assert_eq!(value.as_string(), Some("hello".to_string()));
	}

	#[rstest]
	fn test_from_expr_bool_lit() {
		// Arrange
		let expr: Expr = parse_quote!(true);

		// Act
		let value = AttrValue::from_expr(expr);

		// Assert
		assert!(matches!(value, AttrValue::BoolLit(_)));
		assert!(value.is_bool_literal());
	}

	#[rstest]
	fn test_from_expr_int_lit() {
		// Arrange
		let expr: Expr = parse_quote!(42);

		// Act
		let value = AttrValue::from_expr(expr);

		// Assert
		assert!(matches!(value, AttrValue::IntLit(_)));
		assert!(value.is_int_literal());
	}

	#[rstest]
	fn test_from_expr_float_lit() {
		// Arrange
		let expr: Expr = parse_quote!(3.14);

		// Act
		let value = AttrValue::from_expr(expr);

		// Assert
		assert!(matches!(value, AttrValue::FloatLit(_)));
		assert!(value.is_float_literal());
	}

	#[rstest]
	fn test_from_expr_dynamic() {
		// Arrange
		let expr: Expr = parse_quote!(variable_name);

		// Act
		let value = AttrValue::from_expr(expr);

		// Assert
		assert!(matches!(value, AttrValue::Dynamic(_)));
		assert!(value.is_dynamic());
	}

	#[rstest]
	fn test_from_expr_function_call() {
		// Arrange
		let expr: Expr = parse_quote!(get_value());

		// Act
		let value = AttrValue::from_expr(expr);

		// Assert
		assert!(matches!(value, AttrValue::Dynamic(_)));
		assert!(value.is_dynamic());
	}

	#[rstest]
	fn test_to_expr_roundtrip() {
		// Arrange
		let original: Expr = parse_quote!("test");

		// Act
		let value = AttrValue::from_expr(original.clone());
		let reconstructed = value.to_expr();

		// Assert
		// Both should be Lit(ExprLit { lit: Str(...) })
		if let (Expr::Lit(orig_lit), Expr::Lit(recon_lit)) = (original, reconstructed) {
			if let (Lit::Str(orig_str), Lit::Str(recon_str)) = (orig_lit.lit, recon_lit.lit) {
				assert_eq!(orig_str.value(), recon_str.value());
			} else {
				panic!("Expected Str literals");
			}
		} else {
			panic!("Expected Lit expressions");
		}
	}
}
