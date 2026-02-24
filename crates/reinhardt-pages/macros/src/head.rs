//! The head! macro implementation.
//!
//! This module provides the `head!` procedural macro for creating HTML head sections
//! with a concise, ergonomic DSL.
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::head;
//!
//! // Define a head section using JSX-like syntax
//! let head = head!(|| {
//!     title { "My Page" }
//!     meta { name: "description", content: "Page description" }
//!     link { rel: "stylesheet", href: "/static/style.css" }
//!     script { src: "/static/app.js", defer }
//! });
//! ```

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, Ident, LitStr, Token, braced};

use crate::crate_paths::get_reinhardt_pages_crate;

/// AST node for a head element (title, meta, link, script, style).
#[derive(Debug)]
enum HeadElement {
	/// `title { "Page Title" }`
	Title(Expr),
	/// `meta { name: "...", content: "..." }` or `meta { property: "...", content: "..." }`
	Meta(Vec<HeadAttr>),
	/// `link { rel: "...", href: "...", ... }`
	Link(Vec<HeadAttr>),
	/// `script { src: "...", defer }` or `script { "inline code" }`
	Script(Vec<HeadAttr>, Option<Expr>),
	/// `style { "inline css" }`
	Style(Expr),
}

/// An attribute in a head element.
#[derive(Debug)]
struct HeadAttr {
	name: Ident,
	value: Option<Expr>,
}

impl Parse for HeadAttr {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let name: Ident = input.parse()?;

		// Check for boolean attribute (no value)
		let value = if input.peek(Token![:]) {
			input.parse::<Token![:]>()?;
			Some(input.parse()?)
		} else {
			None
		};

		Ok(HeadAttr { name, value })
	}
}

/// The complete head macro AST.
#[derive(Debug)]
struct HeadMacro {
	elements: Vec<HeadElement>,
}

impl Parse for HeadMacro {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		// Parse optional closure syntax: || { ... }
		if input.peek(Token![|]) {
			input.parse::<Token![|]>()?;
			input.parse::<Token![|]>()?;
		}

		// Parse the body
		let content;
		braced!(content in input);

		let mut elements = Vec::new();

		while !content.is_empty() {
			let element = parse_head_element(&content)?;
			elements.push(element);
		}

		Ok(HeadMacro { elements })
	}
}

/// Parse a single head element.
fn parse_head_element(input: ParseStream) -> syn::Result<HeadElement> {
	let tag: Ident = input.parse()?;
	let tag_str = tag.to_string();

	let content;
	braced!(content in input);

	match tag_str.as_str() {
		"title" => {
			// title { "Page Title" } or title { expression }
			let expr: Expr = content.parse()?;
			Ok(HeadElement::Title(expr))
		}
		"meta" => {
			// meta { name: "...", content: "..." }
			let attrs = parse_attrs(&content)?;
			Ok(HeadElement::Meta(attrs))
		}
		"link" => {
			// link { rel: "...", href: "..." }
			let attrs = parse_attrs(&content)?;
			Ok(HeadElement::Link(attrs))
		}
		"script" => {
			// script { src: "...", defer } or script { "inline code" }
			if content.peek(LitStr) || content.peek(syn::token::Brace) {
				// Inline script
				let expr: Expr = content.parse()?;
				Ok(HeadElement::Script(Vec::new(), Some(expr)))
			} else {
				// External script with attributes
				let attrs = parse_attrs(&content)?;
				Ok(HeadElement::Script(attrs, None))
			}
		}
		"style" => {
			// style { "inline css" }
			let expr: Expr = content.parse()?;
			Ok(HeadElement::Style(expr))
		}
		_ => Err(syn::Error::new(
			tag.span(),
			format!(
				"Unknown head element '{}'. Expected: title, meta, link, script, style",
				tag_str
			),
		)),
	}
}

/// Parse attributes inside braces.
fn parse_attrs(input: ParseStream) -> syn::Result<Vec<HeadAttr>> {
	let attrs: Punctuated<HeadAttr, Token![,]> = Punctuated::parse_terminated(input)?;
	Ok(attrs.into_iter().collect())
}

/// Generate code for the head macro.
///
/// Fixes #844: returns `syn::Result` instead of panicking on invalid input.
fn generate(ast: &HeadMacro) -> syn::Result<TokenStream2> {
	let pages_crate = get_reinhardt_pages_crate();
	let mut builder_calls = Vec::new();

	for element in &ast.elements {
		match element {
			HeadElement::Title(expr) => {
				builder_calls.push(quote! { .title(#expr) });
			}
			HeadElement::Meta(attrs) => {
				let meta_call = generate_meta_call(attrs, &pages_crate)?;
				builder_calls.push(meta_call);
			}
			HeadElement::Link(attrs) => {
				let link_call = generate_link_call(attrs, &pages_crate)?;
				builder_calls.push(link_call);
			}
			HeadElement::Script(attrs, inline_content) => {
				let script_call = generate_script_call(attrs, inline_content, &pages_crate)?;
				builder_calls.push(script_call);
			}
			HeadElement::Style(expr) => {
				builder_calls.push(quote! { .inline_css(#expr) });
			}
		}
	}

	Ok(quote! {
		{
			#pages_crate::component::Head::new()
				#(#builder_calls)*
		}
	})
}

/// Generate a meta tag builder call.
///
/// Fixes #844: returns `syn::Result` instead of panicking via `expect()`.
fn generate_meta_call(attrs: &[HeadAttr], pages_crate: &TokenStream2) -> syn::Result<TokenStream2> {
	// Check for common meta tag patterns
	let name_attr = attrs.iter().find(|a| a.name == "name");
	let property_attr = attrs.iter().find(|a| a.name == "property");
	let content_attr = attrs.iter().find(|a| a.name == "content");
	let charset_attr = attrs.iter().find(|a| a.name == "charset");
	let http_equiv_attr = attrs.iter().find(|a| a.name == "http_equiv");

	let fallback_span = attrs
		.first()
		.map(|a| a.name.span())
		.unwrap_or_else(Span::call_site);

	if let Some(charset) = charset_attr {
		let value = charset
			.value
			.as_ref()
			.ok_or_else(|| syn::Error::new(charset.name.span(), "charset requires a value"))?;
		return Ok(quote! { .meta(#pages_crate::component::MetaTag::charset(#value)) });
	}

	if let Some(http_equiv) = http_equiv_attr {
		let equiv_value = http_equiv.value.as_ref().ok_or_else(|| {
			syn::Error::new(http_equiv.name.span(), "http_equiv requires a value")
		})?;
		let content_value = content_attr.and_then(|a| a.value.as_ref()).ok_or_else(|| {
			syn::Error::new(
				http_equiv.name.span(),
				"http_equiv meta requires content attribute",
			)
		})?;
		return Ok(
			quote! { .meta(#pages_crate::component::MetaTag::http_equiv(#equiv_value, #content_value)) },
		);
	}

	if let Some(property) = property_attr {
		let prop_value = property
			.value
			.as_ref()
			.ok_or_else(|| syn::Error::new(property.name.span(), "property requires a value"))?;
		let content_value = content_attr.and_then(|a| a.value.as_ref()).ok_or_else(|| {
			syn::Error::new(
				property.name.span(),
				"property meta requires content attribute",
			)
		})?;
		return Ok(
			quote! { .meta(#pages_crate::component::MetaTag::property(#prop_value, #content_value)) },
		);
	}

	if let Some(name) = name_attr {
		let name_value = name
			.value
			.as_ref()
			.ok_or_else(|| syn::Error::new(name.name.span(), "name requires a value"))?;
		let content_value = content_attr.and_then(|a| a.value.as_ref()).ok_or_else(|| {
			syn::Error::new(name.name.span(), "name meta requires content attribute")
		})?;
		return Ok(
			quote! { .meta(#pages_crate::component::MetaTag::new(#name_value, #content_value)) },
		);
	}

	Err(syn::Error::new(
		fallback_span,
		"meta tag requires either 'name', 'property', 'charset', or 'http_equiv' attribute",
	))
}

/// Dangerous URL schemes that must be rejected in link href and script src attributes.
const DANGEROUS_URL_SCHEMES: &[&str] = &["javascript:", "data:", "vbscript:"];

/// Validate a URL expression against dangerous schemes.
///
/// If the expression is a string literal, checks that it does not start with
/// a dangerous URL scheme (`javascript:`, `data:`, `vbscript:`).
/// Non-literal expressions cannot be validated at compile time and are allowed.
fn validate_url_scheme(expr: &Expr, attr_name: &str, tag_name: &str) -> syn::Result<()> {
	if let Expr::Lit(lit) = expr
		&& let syn::Lit::Str(s) = &lit.lit
	{
		let value = s.value().to_ascii_lowercase();
		let trimmed = value.trim_start();
		for scheme in DANGEROUS_URL_SCHEMES {
			if trimmed.starts_with(scheme) {
				return Err(syn::Error::new(
					s.span(),
					format!(
						"dangerous URL scheme '{}' is not allowed in {} {} attribute",
						scheme.trim_end_matches(':'),
						tag_name,
						attr_name,
					),
				));
			}
		}
	}
	Ok(())
}

/// Generate a link tag builder call.
///
/// Fixes #844: returns `syn::Result` instead of panicking via `expect()`.
fn generate_link_call(attrs: &[HeadAttr], pages_crate: &TokenStream2) -> syn::Result<TokenStream2> {
	let rel_attr = attrs.iter().find(|a| a.name == "rel");
	let href_attr = attrs.iter().find(|a| a.name == "href");
	let type_attr = attrs
		.iter()
		.find(|a| a.name == "type" || a.name == "r#type");
	let integrity_attr = attrs.iter().find(|a| a.name == "integrity");
	let crossorigin_attr = attrs.iter().find(|a| a.name == "crossorigin");
	let media_attr = attrs.iter().find(|a| a.name == "media");
	let sizes_attr = attrs.iter().find(|a| a.name == "sizes");
	let as_attr = attrs.iter().find(|a| a.name == "r#as" || a.name == "as_");

	let fallback_span = attrs
		.first()
		.map(|a| a.name.span())
		.unwrap_or_else(Span::call_site);

	let rel_value = rel_attr.and_then(|a| a.value.as_ref()).ok_or_else(|| {
		syn::Error::new(
			rel_attr.map(|a| a.name.span()).unwrap_or(fallback_span),
			"link tag requires 'rel' attribute with a value",
		)
	})?;
	let href_value = href_attr.and_then(|a| a.value.as_ref()).ok_or_else(|| {
		syn::Error::new(
			href_attr.map(|a| a.name.span()).unwrap_or(fallback_span),
			"link tag requires 'href' attribute with a value",
		)
	})?;

	// Reject dangerous URL schemes in href attribute
	validate_url_scheme(href_value, "href", "link")?;

	let mut chain = quote! {
		#pages_crate::component::LinkTag::new(#rel_value, #href_value)
	};

	if let Some(type_a) = type_attr
		&& let Some(v) = &type_a.value
	{
		chain = quote! { #chain.with_type(#v) };
	}

	if let Some(integrity_a) = integrity_attr
		&& let Some(v) = &integrity_a.value
	{
		chain = quote! { #chain.with_integrity(#v) };
	}

	if let Some(crossorigin_a) = crossorigin_attr
		&& let Some(v) = &crossorigin_a.value
	{
		chain = quote! { #chain.with_crossorigin(#v) };
	}

	if let Some(media_a) = media_attr
		&& let Some(v) = &media_a.value
	{
		chain = quote! { #chain.with_media(#v) };
	}

	if let Some(sizes_a) = sizes_attr
		&& let Some(v) = &sizes_a.value
	{
		chain = quote! { #chain.with_sizes(#v) };
	}

	if let Some(as_a) = as_attr
		&& let Some(v) = &as_a.value
	{
		// Fixes #845: use builder method instead of direct field assignment
		chain = quote! { #chain.with_as(#v) };
	}

	Ok(quote! { .link(#chain) })
}

/// Generate a script tag builder call.
///
/// Fixes #844: returns `syn::Result` instead of panicking via `expect()`.
fn generate_script_call(
	attrs: &[HeadAttr],
	inline_content: &Option<Expr>,
	pages_crate: &TokenStream2,
) -> syn::Result<TokenStream2> {
	if let Some(content) = inline_content {
		return Ok(quote! { .script(#pages_crate::component::ScriptTag::inline(#content)) });
	}

	let src_attr = attrs.iter().find(|a| a.name == "src");
	let type_attr = attrs
		.iter()
		.find(|a| a.name == "type" || a.name == "r#type");
	let defer_attr = attrs.iter().find(|a| a.name == "defer");
	let async_attr = attrs
		.iter()
		.find(|a| a.name == "r#async" || a.name == "async_");
	let integrity_attr = attrs.iter().find(|a| a.name == "integrity");
	let crossorigin_attr = attrs.iter().find(|a| a.name == "crossorigin");
	let nonce_attr = attrs.iter().find(|a| a.name == "nonce");

	let fallback_span = attrs
		.first()
		.map(|a| a.name.span())
		.unwrap_or_else(Span::call_site);

	let src_value = src_attr.and_then(|a| a.value.as_ref()).ok_or_else(|| {
		syn::Error::new(
			src_attr.map(|a| a.name.span()).unwrap_or(fallback_span),
			"script tag requires 'src' attribute for external scripts",
		)
	})?;

	// Reject dangerous URL schemes in src attribute
	validate_url_scheme(src_value, "src", "script")?;

	// Check if it's a module
	let is_module = type_attr
		.and_then(|a| a.value.as_ref())
		.map(|v| {
			if let Expr::Lit(lit) = v
				&& let syn::Lit::Str(s) = &lit.lit
			{
				return s.value() == "module";
			}
			false
		})
		.unwrap_or(false);

	let mut chain = if is_module {
		quote! { #pages_crate::component::ScriptTag::module(#src_value) }
	} else {
		quote! { #pages_crate::component::ScriptTag::external(#src_value) }
	};

	// Add type if not module
	if !is_module
		&& let Some(type_a) = type_attr
		&& let Some(v) = &type_a.value
	{
		chain = quote! { #chain.with_type(#v) };
	}

	// Boolean attributes (defer, async)
	if defer_attr.is_some() {
		chain = quote! { #chain.with_defer() };
	}

	if async_attr.is_some() {
		chain = quote! { #chain.with_async() };
	}

	if let Some(integrity_a) = integrity_attr
		&& let Some(v) = &integrity_a.value
	{
		chain = quote! { #chain.with_integrity(#v) };
	}

	if let Some(crossorigin_a) = crossorigin_attr
		&& let Some(v) = &crossorigin_a.value
	{
		chain = quote! { #chain.with_crossorigin(#v) };
	}

	if let Some(nonce_a) = nonce_attr
		&& let Some(v) = &nonce_a.value
	{
		chain = quote! { #chain.with_nonce(#v) };
	}

	Ok(quote! { .script(#chain) })
}

/// Implementation of the head! macro.
pub(crate) fn head_impl(input: TokenStream) -> TokenStream {
	let input2 = proc_macro2::TokenStream::from(input);

	let ast: HeadMacro = match syn::parse2(input2) {
		Ok(ast) => ast,
		Err(err) => return err.to_compile_error().into(),
	};

	// Fixes #844: propagate errors from generate() as compile errors
	match generate(&ast) {
		Ok(output) => output.into(),
		Err(err) => err.to_compile_error().into(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use quote::quote;

	#[test]
	fn test_head_macro_title() {
		let input = quote!(|| { title { "My Page" } });
		let ast: HeadMacro = syn::parse2(input).unwrap();
		assert_eq!(ast.elements.len(), 1);
	}

	#[test]
	fn test_head_macro_meta() {
		let input = quote!(|| {
			meta {
				name: "description",
				content: "Page description",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		assert_eq!(ast.elements.len(), 1);
	}

	#[test]
	fn test_head_macro_link() {
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "/static/style.css",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		assert_eq!(ast.elements.len(), 1);
	}

	#[test]
	fn test_head_macro_script_external() {
		let input = quote!(|| {
			script {
				src: "/static/app.js",
				defer,
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		assert_eq!(ast.elements.len(), 1);
	}

	#[test]
	fn test_head_macro_script_inline() {
		let input = quote!(|| {
			script { "console.log('hello');" }
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		assert_eq!(ast.elements.len(), 1);
	}

	#[test]
	fn test_head_macro_multiple_elements() {
		let input = quote!(|| {
			title { "My Page" }
			meta { name: "description", content: "Page description" }
			link { rel: "stylesheet", href: "/static/style.css" }
			script { src: "/static/app.js", defer }
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		assert_eq!(ast.elements.len(), 4);
	}

	#[test]
	fn test_codegen_title() {
		let input = quote!(|| { title { "Test Title" } });
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let output = generate(&ast).unwrap();
		let output_str = output.to_string();
		assert!(output_str.contains(". title"));
	}

	#[test]
	fn test_link_rejects_javascript_scheme() {
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "javascript:alert(1)",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let result = generate(&ast);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("dangerous URL scheme"));
		assert!(err.contains("javascript"));
	}

	#[test]
	fn test_link_rejects_data_scheme() {
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "data:text/css,body{}",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let result = generate(&ast);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("dangerous URL scheme"));
		assert!(err.contains("data"));
	}

	#[test]
	fn test_link_rejects_vbscript_scheme() {
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "vbscript:MsgBox",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let result = generate(&ast);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("dangerous URL scheme"));
		assert!(err.contains("vbscript"));
	}

	#[test]
	fn test_script_rejects_javascript_scheme() {
		let input = quote!(|| {
			script {
				src: "javascript:alert(1)",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let result = generate(&ast);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("dangerous URL scheme"));
		assert!(err.contains("javascript"));
	}

	#[test]
	fn test_script_rejects_data_scheme() {
		let input = quote!(|| {
			script {
				src: "data:text/javascript,alert(1)",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let result = generate(&ast);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("dangerous URL scheme"));
		assert!(err.contains("data"));
	}

	#[test]
	fn test_link_allows_safe_urls() {
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "/static/style.css",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let result = generate(&ast);
		assert!(result.is_ok());
	}

	#[test]
	fn test_link_allows_https_urls() {
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "https://cdn.example.com/style.css",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let result = generate(&ast);
		assert!(result.is_ok());
	}

	#[test]
	fn test_link_rejects_case_insensitive_javascript() {
		let input = quote!(|| {
			link {
				rel: "stylesheet",
				href: "JavaScript:alert(1)",
			}
		});
		let ast: HeadMacro = syn::parse2(input).unwrap();
		let result = generate(&ast);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("dangerous URL scheme"));
	}
}
