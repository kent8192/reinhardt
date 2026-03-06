//! Parser for head! macro.
//!
//! This module provides parsing logic for the `head!` DSL macro,
//! converting token streams into untyped AST nodes.

use proc_macro2::TokenStream;
use syn::{
	Expr, Ident, LitStr, Token, braced,
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
};

use crate::core::{HeadAttr, HeadContent, HeadElement, HeadMacro};

/// Parses a `head!` macro invocation into an untyped AST.
///
/// # Example
///
/// ```ignore
/// use reinhardt_manouche::parser::parse_head;
/// use quote::quote;
///
/// let tokens = quote!(|| {
///     title { "My Page" }
///     meta { name: "description", content: "Page description" }
/// });
///
/// let ast = parse_head(tokens).unwrap();
/// assert_eq!(ast.elements.len(), 2);
/// ```
pub fn parse_head(input: TokenStream) -> syn::Result<HeadMacro> {
	syn::parse2(input)
}

impl Parse for HeadMacro {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let span = input.span();

		// Parse optional closure syntax: || { ... }
		if input.peek(Token![|]) {
			input.parse::<Token![|]>()?;
			input.parse::<Token![|]>()?;
		}

		// Parse the body in braces
		let content;
		braced!(content in input);

		let mut elements = Vec::new();

		while !content.is_empty() {
			let element: HeadElement = content.parse()?;
			elements.push(element);
		}

		Ok(HeadMacro { elements, span })
	}
}

impl Parse for HeadElement {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let tag: Ident = input.parse()?;
		let tag_str = tag.to_string();
		let span = tag.span();

		let content_stream;
		braced!(content_stream in input);

		// Parse based on tag type
		let (attrs, content) = match tag_str.as_str() {
			"title" => parse_title_content(&content_stream)?,
			"meta" => (parse_attrs(&content_stream)?, None),
			"link" => (parse_attrs(&content_stream)?, None),
			"script" => parse_script_content(&content_stream)?,
			"style" => parse_style_content(&content_stream)?,
			_ => {
				return Err(syn::Error::new(
					span,
					format!(
						"Unknown head element '{}'. Expected: title, meta, link, script, style",
						tag_str
					),
				));
			}
		};

		Ok(HeadElement {
			tag,
			attrs,
			content,
			span,
		})
	}
}

impl Parse for HeadAttr {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let name: Ident = input.parse()?;
		let span = name.span();

		// Check for boolean attribute (no value) vs key-value attribute
		let value = if input.peek(Token![:]) {
			input.parse::<Token![:]>()?;
			input.parse()?
		} else {
			// Boolean attribute - create a `true` expression
			syn::parse_quote!(true)
		};

		Ok(HeadAttr { name, value, span })
	}
}

/// Parses title element content.
///
/// title { "Page Title" } or title { expression }
fn parse_title_content(input: ParseStream) -> syn::Result<(Vec<HeadAttr>, Option<HeadContent>)> {
	let expr: Expr = input.parse()?;

	// Check if it's a string literal for static text
	let content = if let Expr::Lit(lit) = &expr
		&& let syn::Lit::Str(s) = &lit.lit
	{
		HeadContent::Text(s.value())
	} else {
		HeadContent::Expr(expr)
	};

	Ok((Vec::new(), Some(content)))
}

/// Parses script element content.
///
/// script { src: "...", defer } or script { "inline code" }
fn parse_script_content(input: ParseStream) -> syn::Result<(Vec<HeadAttr>, Option<HeadContent>)> {
	// Check if it's inline content (string literal or expression starting with brace)
	if input.peek(LitStr) {
		let lit: LitStr = input.parse()?;
		return Ok((Vec::new(), Some(HeadContent::Text(lit.value()))));
	}

	// Check for block expression
	if input.peek(syn::token::Brace) {
		let expr: Expr = input.parse()?;
		return Ok((Vec::new(), Some(HeadContent::Expr(expr))));
	}

	// Otherwise parse as attributes for external script
	let attrs = parse_attrs(input)?;
	Ok((attrs, None))
}

/// Parses style element content.
///
/// style { "inline css" } or style { expression }
fn parse_style_content(input: ParseStream) -> syn::Result<(Vec<HeadAttr>, Option<HeadContent>)> {
	let expr: Expr = input.parse()?;

	// Check if it's a string literal for static text
	let content = if let Expr::Lit(lit) = &expr
		&& let syn::Lit::Str(s) = &lit.lit
	{
		HeadContent::Text(s.value())
	} else {
		HeadContent::Expr(expr)
	};

	Ok((Vec::new(), Some(content)))
}

/// Parses attributes inside braces.
fn parse_attrs(input: ParseStream) -> syn::Result<Vec<HeadAttr>> {
	let attrs: Punctuated<HeadAttr, Token![,]> = Punctuated::parse_terminated(input)?;
	Ok(attrs.into_iter().collect())
}

#[cfg(test)]
mod tests {
	use super::*;
	use quote::quote;
	use rstest::rstest;

	#[rstest]
	fn test_parse_head_title_string_literal() {
		// Arrange
		let input = quote!(|| { title { "My Page" } });

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "title");
		assert!(ast.elements[0].attrs.is_empty());
		assert!(matches!(
			ast.elements[0].content,
			Some(HeadContent::Text(ref s)) if s == "My Page"
		));
	}

	#[rstest]
	fn test_parse_head_title_expression() {
		// Arrange
		let input = quote!(|| { title { page_title } });

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "title");
		assert!(matches!(
			ast.elements[0].content,
			Some(HeadContent::Expr(_))
		));
	}

	#[rstest]
	fn test_parse_head_meta_with_attributes() {
		// Arrange
		let input = quote!(|| {
			meta {
				name: "description",
				content: "Page description",
			}
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "meta");
		assert_eq!(ast.elements[0].attrs.len(), 2);
		assert_eq!(ast.elements[0].attrs[0].name.to_string(), "name");
		assert_eq!(ast.elements[0].attrs[1].name.to_string(), "content");
		assert!(ast.elements[0].content.is_none());
	}

	#[rstest]
	fn test_parse_head_link_with_attributes() {
		// Arrange
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "/static/style.css",
			}
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "link");
		assert_eq!(ast.elements[0].attrs.len(), 2);
		assert_eq!(ast.elements[0].attrs[0].name.to_string(), "rel");
		assert_eq!(ast.elements[0].attrs[1].name.to_string(), "href");
	}

	#[rstest]
	fn test_parse_head_script_external_with_boolean_attr() {
		// Arrange
		let input = quote!(|| {
			script {
				src: "/static/app.js",
				defer,
			}
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "script");
		assert_eq!(ast.elements[0].attrs.len(), 2);
		assert_eq!(ast.elements[0].attrs[0].name.to_string(), "src");
		assert_eq!(ast.elements[0].attrs[1].name.to_string(), "defer");
		assert!(ast.elements[0].content.is_none());
	}

	#[rstest]
	fn test_parse_head_script_inline() {
		// Arrange
		let input = quote!(|| {
			script { "console.log('hello');" }
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "script");
		assert!(ast.elements[0].attrs.is_empty());
		assert!(matches!(
			ast.elements[0].content,
			Some(HeadContent::Text(ref s)) if s == "console.log('hello');"
		));
	}

	#[rstest]
	fn test_parse_head_style_inline() {
		// Arrange
		let input = quote!(|| {
			style { "body { margin: 0; }" }
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "style");
		assert!(ast.elements[0].attrs.is_empty());
		assert!(matches!(
			ast.elements[0].content,
			Some(HeadContent::Text(ref s)) if s == "body { margin: 0; }"
		));
	}

	#[rstest]
	fn test_parse_head_multiple_elements() {
		// Arrange
		let input = quote!(|| {
			title { "My Page" }
			meta { name: "description", content: "Page description" }
			link { rel: "stylesheet", href: "/static/style.css" }
			script { src: "/static/app.js", defer }
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 4);
		assert_eq!(ast.elements[0].tag.to_string(), "title");
		assert_eq!(ast.elements[1].tag.to_string(), "meta");
		assert_eq!(ast.elements[2].tag.to_string(), "link");
		assert_eq!(ast.elements[3].tag.to_string(), "script");
	}

	#[rstest]
	fn test_parse_head_without_closure_syntax() {
		// Arrange
		let input = quote!({
			title { "My Page" }
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "title");
	}

	#[rstest]
	fn test_parse_head_unknown_element_error() {
		// Arrange
		let input = quote!(|| {
			unknown { "content" }
		});

		// Act
		let result = parse_head(input);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("Unknown head element"));
	}

	#[rstest]
	fn test_parse_head_meta_charset() {
		// Arrange
		let input = quote!(|| { meta { charset: "utf-8" } });

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].tag.to_string(), "meta");
		assert_eq!(ast.elements[0].attrs.len(), 1);
		assert_eq!(ast.elements[0].attrs[0].name.to_string(), "charset");
	}

	#[rstest]
	fn test_parse_head_link_with_integrity() {
		// Arrange
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "https://cdn.example.com/style.css",
				integrity: "sha384-abc123",
				crossorigin: "anonymous",
			}
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].attrs.len(), 4);
	}

	#[rstest]
	fn test_parse_head_script_module() {
		// Arrange
		let input = quote!(|| {
			script {
				src: "/static/app.mjs",
				r#type: "module",
			}
		});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert_eq!(ast.elements.len(), 1);
		assert_eq!(ast.elements[0].attrs.len(), 2);
		// Raw identifier r#type is preserved as-is
		assert_eq!(ast.elements[0].attrs[1].name.to_string(), "r#type");
	}

	#[rstest]
	fn test_parse_head_empty() {
		// Arrange
		let input = quote!(|| {});

		// Act
		let ast = parse_head(input).unwrap();

		// Assert
		assert!(ast.elements.is_empty());
	}
}
