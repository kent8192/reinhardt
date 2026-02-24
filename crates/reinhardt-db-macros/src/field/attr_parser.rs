//! Attribute parsing for `#[field(...)]` macro.

use proc_macro2::Span;
use syn::{
	Error, Ident, Lit, Result, Token,
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
};

/// Parsed attributes from `#[field(...)]`.
#[derive(Debug, Clone, Default)]
pub(crate) struct FieldAttrs {
	/// Whether this field is the primary key
	pub(crate) primary_key: bool,
	/// Whether to create an index on this field
	pub(crate) index: bool,
	/// Whether to create a unique index on this field
	pub(crate) unique: bool,
	/// Whether this field is required
	pub(crate) required: bool,
	/// Default value expression
	pub(crate) default: Option<String>,
	/// Rename the field in BSON
	pub(crate) rename: Option<String>,
	/// Validation function name
	pub(crate) validate: Option<String>,
	/// Minimum value (for numbers)
	pub(crate) min: Option<Lit>,
	/// Maximum value (for numbers)
	pub(crate) max: Option<Lit>,
	/// Foreign key reference to another collection
	pub(crate) references: Option<String>,
}

impl Parse for FieldAttrs {
	fn parse(input: ParseStream) -> Result<Self> {
		let attrs = Punctuated::<FieldAttr, Token![,]>::parse_terminated(input)?;

		let mut result = Self::default();

		for attr in attrs {
			match attr {
				FieldAttr::Flag(name) => {
					let name_str = name.to_string();
					match name_str.as_str() {
						"primary_key" => {
							if result.primary_key {
								return Err(Error::new(
									name.span(),
									"duplicate `primary_key` attribute",
								));
							}
							result.primary_key = true;
						}
						"index" => {
							if result.index {
								return Err(Error::new(name.span(), "duplicate `index` attribute"));
							}
							result.index = true;
						}
						"unique" => {
							if result.unique {
								return Err(Error::new(
									name.span(),
									"duplicate `unique` attribute",
								));
							}
							result.unique = true;
						}
						"required" => {
							if result.required {
								return Err(Error::new(
									name.span(),
									"duplicate `required` attribute",
								));
							}
							result.required = true;
						}
						_ => {
							return Err(Error::new(
								name.span(),
								format!("unknown flag attribute `{}`", name),
							));
						}
					}
				}
				FieldAttr::NameValue { name, value } => {
					let name_str = name.to_string();
					match name_str.as_str() {
						"default" => {
							if result.default.is_some() {
								return Err(Error::new(
									name.span(),
									"duplicate `default` attribute",
								));
							}
							result.default = Some(lit_to_string(&value)?);
						}
						"rename" => {
							if result.rename.is_some() {
								return Err(Error::new(
									name.span(),
									"duplicate `rename` attribute",
								));
							}
							result.rename = Some(lit_to_string(&value)?);
						}
						"validate" => {
							if result.validate.is_some() {
								return Err(Error::new(
									name.span(),
									"duplicate `validate` attribute",
								));
							}
							result.validate = Some(lit_to_string(&value)?);
						}
						"min" => {
							if result.min.is_some() {
								return Err(Error::new(name.span(), "duplicate `min` attribute"));
							}
							result.min = Some(value);
						}
						"max" => {
							if result.max.is_some() {
								return Err(Error::new(name.span(), "duplicate `max` attribute"));
							}
							result.max = Some(value);
						}
						"references" => {
							if result.references.is_some() {
								return Err(Error::new(
									name.span(),
									"duplicate `references` attribute",
								));
							}
							result.references = Some(lit_to_string(&value)?);
						}
						_ => {
							return Err(Error::new(
								name.span(),
								format!("unknown attribute `{}`", name),
							));
						}
					}
				}
			}
		}

		// Validation: unique implies index
		if result.unique && !result.index {
			result.index = true;
		}

		Ok(result)
	}
}

/// Single field attribute: either a flag or a name-value pair.
enum FieldAttr {
	/// Flag attribute (e.g., `primary_key`, `required`)
	Flag(Ident),
	/// Name-value attribute (e.g., `default = "value"`)
	NameValue { name: Ident, value: Lit },
}

impl Parse for FieldAttr {
	fn parse(input: ParseStream) -> Result<Self> {
		let name: Ident = input.parse()?;

		// Check if this is a name-value pair
		if input.peek(Token![=]) {
			let _eq: Token![=] = input.parse()?;
			let value: Lit = input.parse()?;
			Ok(FieldAttr::NameValue { name, value })
		} else {
			Ok(FieldAttr::Flag(name))
		}
	}
}

/// Convert a literal to a string.
fn lit_to_string(lit: &Lit) -> Result<String> {
	match lit {
		Lit::Str(s) => Ok(s.value()),
		Lit::Int(i) => Ok(i.to_string()),
		Lit::Float(f) => Ok(f.to_string()),
		Lit::Bool(b) => Ok(b.value.to_string()),
		_ => Err(Error::new(
			Span::call_site(),
			format!("unsupported literal type: {:?}", lit),
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_quote;

	#[test]
	fn test_parse_primary_key() {
		let attrs: FieldAttrs = parse_quote! { primary_key };

		assert!(attrs.primary_key);
		assert!(!attrs.index);
		assert!(!attrs.unique);
		assert!(!attrs.required);
	}

	#[test]
	fn test_parse_multiple_flags() {
		let attrs: FieldAttrs = parse_quote! { required, unique };

		assert!(!attrs.primary_key);
		assert!(attrs.index); // unique implies index
		assert!(attrs.unique);
		assert!(attrs.required);
	}

	#[test]
	fn test_parse_name_values() {
		let attrs: FieldAttrs = parse_quote! {
			default = "hello", rename = "user_name", validate = "email"
		};

		assert_eq!(attrs.default, Some("hello".to_string()));
		assert_eq!(attrs.rename, Some("user_name".to_string()));
		assert_eq!(attrs.validate, Some("email".to_string()));
	}

	#[test]
	fn test_parse_mixed_attrs() {
		let attrs: FieldAttrs = parse_quote! {
			required, rename = "email_address", unique
		};

		assert!(attrs.required);
		assert!(attrs.unique);
		assert!(attrs.index); // unique implies index
		assert_eq!(attrs.rename, Some("email_address".to_string()));
	}

	#[test]
	fn test_parse_min_max() {
		let attrs: FieldAttrs = parse_quote! { min = 0, max = 100 };

		assert!(attrs.min.is_some());
		assert!(attrs.max.is_some());
	}

	#[test]
	fn test_duplicate_flag_error() {
		let result: Result<FieldAttrs> = syn::parse2(parse_quote! {
			primary_key, primary_key
		});

		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("duplicate `primary_key` attribute")
		);
	}

	#[test]
	fn test_duplicate_name_value_error() {
		let result: Result<FieldAttrs> = syn::parse2(parse_quote! {
			default = "a", default = "b"
		});

		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("duplicate `default` attribute")
		);
	}

	#[test]
	fn test_unknown_flag_error() {
		let result: Result<FieldAttrs> = syn::parse2(parse_quote! { unknown_flag });

		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("unknown flag attribute `unknown_flag`")
		);
	}

	#[test]
	fn test_unknown_name_value_error() {
		let result: Result<FieldAttrs> = syn::parse2(parse_quote! {
			unknown = "value"
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
