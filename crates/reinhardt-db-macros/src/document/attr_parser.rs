//! Attribute parsing for `#[document(...)]` macro.

use proc_macro2::Span;
use syn::{
	Error, Ident, LitStr, Result, Token,
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
};

/// Parsed attributes from `#[document(...)]`.
#[derive(Debug, Clone)]
pub(crate) struct DocumentAttrs {
	/// Collection name (required)
	pub(crate) collection: String,
	/// Backend type (required, must be "mongodb")
	pub(crate) backend: String,
	/// Database name (optional)
	pub(crate) database: Option<String>,
}

impl Parse for DocumentAttrs {
	fn parse(input: ParseStream) -> Result<Self> {
		let attrs = Punctuated::<Attr, Token![,]>::parse_terminated(input)?;

		let mut collection = None;
		let mut backend = None;
		let mut database = None;

		for attr in attrs {
			match attr.name.to_string().as_str() {
				"collection" => {
					if collection.is_some() {
						return Err(Error::new(
							attr.name.span(),
							"duplicate `collection` attribute",
						));
					}
					collection = Some(attr.value.value());
				}
				"backend" => {
					if backend.is_some() {
						return Err(Error::new(
							attr.name.span(),
							"duplicate `backend` attribute",
						));
					}
					backend = Some(attr.value.value());
				}
				"database" => {
					if database.is_some() {
						return Err(Error::new(
							attr.name.span(),
							"duplicate `database` attribute",
						));
					}
					database = Some(attr.value.value());
				}
				_ => {
					return Err(Error::new(
						attr.name.span(),
						format!("unknown attribute `{}`", attr.name),
					));
				}
			}
		}

		// Validate required attributes
		let collection = collection.ok_or_else(|| {
			Error::new(Span::call_site(), "missing required attribute `collection`")
		})?;

		let backend = backend
			.ok_or_else(|| Error::new(Span::call_site(), "missing required attribute `backend`"))?;

		// Validate backend value
		if backend != "mongodb" {
			return Err(Error::new(
				Span::call_site(),
				format!("unsupported backend `{}`, expected `mongodb`", backend),
			));
		}

		Ok(Self {
			collection,
			backend,
			database,
		})
	}
}

/// Single attribute: `name = "value"`.
struct Attr {
	name: Ident,
	_eq: Token![=],
	value: LitStr,
}

impl Parse for Attr {
	fn parse(input: ParseStream) -> Result<Self> {
		Ok(Self {
			name: input.parse()?,
			_eq: input.parse()?,
			value: input.parse()?,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_quote;

	#[test]
	fn test_parse_full_attrs() {
		let attrs: DocumentAttrs = parse_quote! {
			collection = "users", backend = "mongodb", database = "myapp"
		};

		assert_eq!(attrs.collection, "users");
		assert_eq!(attrs.backend, "mongodb");
		assert_eq!(attrs.database, Some("myapp".to_string()));
	}

	#[test]
	fn test_parse_minimal_attrs() {
		let attrs: DocumentAttrs = parse_quote! {
			collection = "users", backend = "mongodb"
		};

		assert_eq!(attrs.collection, "users");
		assert_eq!(attrs.backend, "mongodb");
		assert_eq!(attrs.database, None);
	}

	#[test]
	fn test_missing_collection() {
		let result: Result<DocumentAttrs> = syn::parse2(parse_quote! {
			backend = "mongodb"
		});

		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("missing required attribute `collection`")
		);
	}

	#[test]
	fn test_missing_backend() {
		let result: Result<DocumentAttrs> = syn::parse2(parse_quote! {
			collection = "users"
		});

		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("missing required attribute `backend`")
		);
	}

	#[test]
	fn test_unsupported_backend() {
		let result: Result<DocumentAttrs> = syn::parse2(parse_quote! {
			collection = "users", backend = "redis"
		});

		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("unsupported backend `redis`, expected `mongodb`")
		);
	}

	#[test]
	fn test_duplicate_collection() {
		let result: Result<DocumentAttrs> = syn::parse2(parse_quote! {
			collection = "users", collection = "posts", backend = "mongodb"
		});

		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("duplicate `collection` attribute")
		);
	}

	#[test]
	fn test_unknown_attribute() {
		let result: Result<DocumentAttrs> = syn::parse2(parse_quote! {
			collection = "users", backend = "mongodb", unknown = "value"
		});

		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("unknown attribute `unknown`")
		);
	}
}
