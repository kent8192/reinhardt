//! Validation for head! macro.
//!
//! Validates head elements to ensure only safe, recognized HTML head elements
//! are used, and rejects potentially dangerous patterns such as inline scripts
//! or meta redirects to external URLs.

use syn::{Expr, Lit, Result};

use crate::core::{HeadMacro, TypedHeadAttr, TypedHeadContent, TypedHeadElement, TypedHeadMacro};

/// Known safe head element tag names.
const ALLOWED_HEAD_TAGS: &[&str] = &["title", "meta", "link", "style", "base", "noscript"];

/// Validates a `HeadMacro`.
///
/// Performs the following checks:
/// - Element tag names must be in the allowed set (rejects `script` and unknown elements)
/// - `meta` elements with `http_equiv = "refresh"` that contain URL redirects are rejected
/// - Attribute values are extracted using literal value extraction where possible,
///   falling back to `syn::Expr` representation for non-literal expressions
pub fn validate_head(ast: &HeadMacro) -> Result<TypedHeadMacro> {
	let mut validated_elements = Vec::with_capacity(ast.elements.len());

	for elem in &ast.elements {
		let tag_name = elem.tag.to_string();

		// Reject script elements to prevent XSS via inline scripts
		if tag_name == "script" {
			return Err(syn::Error::new(
				elem.span,
				"'script' elements are not allowed in head! macro. \
				 Use a 'link' element with a script src or load scripts via asset pipeline",
			));
		}

		// Reject unknown/dangerous elements
		if !ALLOWED_HEAD_TAGS.contains(&tag_name.as_str()) {
			return Err(syn::Error::new(
				elem.span,
				format!(
					"Unknown head element '{}'. Allowed elements: {}",
					tag_name,
					ALLOWED_HEAD_TAGS.join(", ")
				),
			));
		}

		// Build typed attributes, extracting literal values where possible
		let typed_attrs: Vec<TypedHeadAttr> = elem
			.attrs
			.iter()
			.map(|attr| {
				let name = attr.name.to_string();
				let value = extract_literal_value(&attr.value);
				TypedHeadAttr {
					name,
					value,
					span: attr.span,
				}
			})
			.collect();

		// Validate meta element: reject http-equiv="refresh" with URL content
		if tag_name == "meta" {
			validate_meta_element(&typed_attrs, elem.span)?;
		}

		let content = elem.content.as_ref().map(|content| match content {
			crate::core::HeadContent::Text(s) => TypedHeadContent::Static(s.clone()),
			crate::core::HeadContent::Expr(e) => TypedHeadContent::Dynamic(e.clone()),
		});

		validated_elements.push(TypedHeadElement {
			tag: tag_name,
			attrs: typed_attrs,
			content,
			span: elem.span,
		});
	}

	Ok(TypedHeadMacro {
		elements: validated_elements,
		span: ast.span,
	})
}

/// Extract a string value from a `syn::Expr` if it is a literal.
///
/// For string literals, returns the unquoted string value directly.
/// For other literal types, returns their display representation.
/// For non-literal expressions (variables, function calls, etc.),
/// falls back to the token stream string representation.
fn extract_literal_value(expr: &Expr) -> String {
	match expr {
		Expr::Lit(expr_lit) => match &expr_lit.lit {
			Lit::Str(s) => s.value(),
			Lit::Int(i) => i.base10_digits().to_string(),
			Lit::Float(f) => f.base10_digits().to_string(),
			Lit::Bool(b) => b.value.to_string(),
			other => {
				use quote::ToTokens;
				other.to_token_stream().to_string()
			}
		},
		other => {
			use quote::ToTokens;
			other.to_token_stream().to_string()
		}
	}
}

/// Validate a meta element for dangerous patterns.
///
/// Rejects `http-equiv="refresh"` when the content contains a URL redirect,
/// as this can be used for open redirect attacks.
fn validate_meta_element(attrs: &[TypedHeadAttr], span: proc_macro2::Span) -> Result<()> {
	// Normalize attribute names: convert underscores to hyphens for HTML attribute matching
	let has_refresh = attrs.iter().any(|a| {
		(a.name == "http_equiv" || a.name == "httpEquiv") && a.value.eq_ignore_ascii_case("refresh")
	});

	if has_refresh {
		// Check if content contains a URL redirect (e.g., "5; url=...")
		let has_url_redirect = attrs
			.iter()
			.any(|a| a.name == "content" && a.value.to_lowercase().contains("url="));

		if has_url_redirect {
			return Err(syn::Error::new(
				span,
				"meta http-equiv=\"refresh\" with URL redirect is not allowed. \
				 Use server-side redirects instead to prevent open redirect vulnerabilities",
			));
		}
	}

	Ok(())
}
