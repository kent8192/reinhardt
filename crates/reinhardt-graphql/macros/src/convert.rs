//! Implementation of GrpcGraphQLConvert derive macro

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields, Ident, LitStr, Result};

/// Field configuration parsed from attributes
#[derive(Debug, Default)]
struct FieldConfig {
	/// Renamed field name for GraphQL/Proto
	rename: Option<String>,
	/// Skip this field if condition is true
	skip_if: Option<String>,
	/// Proto field name (if different from GraphQL)
	proto_name: Option<String>,
}

/// Characters allowed in skip_if function path names.
///
/// Only allows Rust path syntax: alphanumeric, underscores, colons (for paths),
/// and angle brackets (for generic types). Rejects arbitrary expressions
/// to prevent code injection via macro attributes.
fn is_valid_skip_if_path(path: &str) -> bool {
	if path.is_empty() {
		return false;
	}
	// Must be a simple function path like "Option::is_none" or "String::is_empty"
	// Reject anything that looks like an expression (containing operators, parens, etc.)
	path.chars()
		.all(|c| c.is_alphanumeric() || c == '_' || c == ':' || c == '<' || c == '>')
		&& !path.contains(";;")
		&& !path.contains("//")
		&& !path.contains("/*")
}

impl FieldConfig {
	fn from_field(field: &Field) -> Result<Self> {
		let mut config = FieldConfig::default();

		for attr in &field.attrs {
			if attr.path().is_ident("graphql") {
				attr.parse_nested_meta(|meta| {
					if meta.path.is_ident("rename") {
						let value = meta.value()?;
						let name: LitStr = value.parse()?;
						config.rename = Some(name.value());
					} else if meta.path.is_ident("skip_if") {
						let value = meta.value()?;
						let condition: LitStr = value.parse()?;
						let path = condition.value();
						// Validate that skip_if value is a simple function path,
						// not an arbitrary expression
						if !is_valid_skip_if_path(&path) {
							return Err(syn::Error::new_spanned(
								&condition,
								"skip_if must be a simple function path (e.g., \"Option::is_none\"), \
								 arbitrary expressions are not allowed for security reasons",
							));
						}
						config.skip_if = Some(path);
					}
					Ok(())
				})?;
			} else if attr.path().is_ident("proto") {
				attr.parse_nested_meta(|meta| {
					if meta.path.is_ident("name") {
						let value = meta.value()?;
						let name: LitStr = value.parse()?;
						config.proto_name = Some(name.value());
					}
					Ok(())
				})?;
			}
		}

		Ok(config)
	}
}

pub(crate) fn expand_derive(input: DeriveInput) -> Result<TokenStream> {
	let name = &input.ident;

	// Only structs are supported
	let fields = match &input.data {
		Data::Struct(data) => match &data.fields {
			Fields::Named(fields) => &fields.named,
			_ => {
				return Err(syn::Error::new_spanned(
					input,
					"GrpcGraphQLConvert only supports structs with named fields",
				));
			}
		},
		_ => {
			return Err(syn::Error::new_spanned(
				input,
				"GrpcGraphQLConvert only supports structs",
			));
		}
	};

	// Parse field configurations
	let field_configs: Vec<_> = fields
		.iter()
		.map(FieldConfig::from_field)
		.collect::<Result<_>>()?;

	// Get list of field names with their configs
	let field_data: Vec<_> = fields
		.iter()
		.zip(field_configs.iter())
		.filter_map(|(f, config)| f.ident.as_ref().map(|ident| (ident, config)))
		.collect();

	// Get proto type name
	// Read from #[graphql(proto = "...")] attribute
	let mut proto_type: Option<TokenStream> = None;

	for attr in &input.attrs {
		if attr.path().is_ident("graphql") {
			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("proto") {
					let value = meta.value()?;
					let proto_path: syn::LitStr = value.parse()?;
					let proto_tokens: TokenStream = proto_path.value().parse().map_err(|_| {
						syn::Error::new_spanned(&proto_path, "Invalid proto type path")
					})?;
					proto_type = Some(proto_tokens);
				}
				Ok(())
			})?;
		}
	}

	let proto_type = proto_type.unwrap_or_else(|| {
		// Default is crate::proto::{name}
		quote! { crate::proto::#name }
	});

	// Build field conversions for From<proto> for GraphQL
	let from_proto_fields: Vec<TokenStream> = field_data
		.iter()
		.map(|(field_name, config)| -> Result<TokenStream> {
			let proto_field = if let Some(proto_name) = &config.proto_name {
				let proto_ident = Ident::new(proto_name, field_name.span());
				quote! { #proto_ident }
			} else {
				quote! { #field_name }
			};

			if let Some(skip_condition) = &config.skip_if {
				// Fixes #816
				let condition: TokenStream = skip_condition.parse().map_err(|_| {
					syn::Error::new(
						field_name.span(),
						format!("invalid skip_if expression: `{}`", skip_condition),
					)
				})?;
				Ok(quote! {
					#field_name: if #condition(&proto.#proto_field) {
						Default::default()
					} else {
						proto.#proto_field.into()
					}
				})
			} else {
				Ok(quote! {
					#field_name: proto.#proto_field.into()
				})
			}
		})
		.collect::<Result<_>>()?;

	// Build field conversions for From<GraphQL> for proto
	let into_proto_fields: Vec<TokenStream> = field_data
		.iter()
		.map(|(field_name, config)| -> Result<TokenStream> {
			let proto_field = if let Some(proto_name) = &config.proto_name {
				let proto_ident = Ident::new(proto_name, field_name.span());
				quote! { #proto_ident }
			} else {
				quote! { #field_name }
			};

			if let Some(skip_condition) = &config.skip_if {
				// Fixes #816
				let condition: TokenStream = skip_condition.parse().map_err(|_| {
					syn::Error::new(
						field_name.span(),
						format!("invalid skip_if expression: `{}`", skip_condition),
					)
				})?;
				Ok(quote! {
					#proto_field: if #condition(&graphql.#field_name) {
						Default::default()
					} else {
						graphql.#field_name.into()
					}
				})
			} else {
				Ok(quote! {
					#proto_field: graphql.#field_name.into()
				})
			}
		})
		.collect::<Result<_>>()?;

	// Implementation of From<proto> for GraphQL
	let from_proto = quote! {
		impl From<#proto_type> for #name {
			fn from(proto: #proto_type) -> Self {
				Self {
					#( #from_proto_fields ),*
				}
			}
		}
	};

	// Implementation of From<GraphQL> for proto
	let into_proto = quote! {
		impl From<#name> for #proto_type {
			fn from(graphql: #name) -> Self {
				Self {
					#( #into_proto_fields ),*
				}
			}
		}
	};

	Ok(quote! {
		#from_proto
		#into_proto
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_quote;

	#[test]
	fn test_basic_struct() {
		let input: DeriveInput = parse_quote! {
			struct User {
				id: String,
				name: String,
			}
		};

		let result = expand_derive(input);
		let output = result.unwrap();
		let output_str = output.to_string();

		// Check existence of From<proto> implementation
		assert!(output_str.contains("impl From < crate :: proto :: User > for User"));
		// Check existence of From<GraphQL> implementation
		assert!(output_str.contains("impl From < User > for crate :: proto :: User"));
	}

	#[test]
	fn test_field_rename() {
		let input: DeriveInput = parse_quote! {
			struct User {
				id: String,
				#[proto(name = "user_name")]
				name: String,
			}
		};

		let result = expand_derive(input);
		let output = result.unwrap();
		let output_str = output.to_string();

		// Check that proto field name is used
		assert!(output_str.contains("user_name"));
	}

	#[test]
	fn test_skip_if() {
		let input: DeriveInput = parse_quote! {
			struct User {
				id: String,
				#[graphql(skip_if = "Option::is_none")]
				email: Option<String>,
			}
		};

		let result = expand_derive(input);
		let output = result.unwrap();
		let output_str = output.to_string();

		// Check that skip condition is present
		assert!(output_str.contains("Option :: is_none"));
	}

	#[test]
	fn test_enum_error() {
		let input: DeriveInput = parse_quote! {
			enum UserType {
				Admin,
				Regular,
			}
		};

		let result = expand_derive(input);
		assert!(result.is_err());
	}

	#[test]
	fn test_skip_if_rejects_arbitrary_expression() {
		let input: DeriveInput = parse_quote! {
			struct User {
				id: String,
				#[graphql(skip_if = "|| { std::process::exit(1) }")]
				email: Option<String>,
			}
		};

		let result = expand_derive(input);
		assert!(
			result.is_err(),
			"should reject arbitrary expressions in skip_if"
		);
	}

	#[test]
	fn test_skip_if_accepts_valid_function_path() {
		let input: DeriveInput = parse_quote! {
			struct User {
				id: String,
				#[graphql(skip_if = "Option::is_none")]
				email: Option<String>,
			}
		};

		let result = expand_derive(input);
		assert!(
			result.is_ok(),
			"should accept valid function path in skip_if"
		);
	}

	#[test]
	fn test_valid_skip_if_path_validation() {
		assert!(is_valid_skip_if_path("Option::is_none"));
		assert!(is_valid_skip_if_path("String::is_empty"));
		assert!(is_valid_skip_if_path("my_module::my_func"));
		assert!(!is_valid_skip_if_path(""));
		assert!(!is_valid_skip_if_path("|| true"));
		assert!(!is_valid_skip_if_path("fn() { }"));
		assert!(!is_valid_skip_if_path("std::process::exit(1)"));
	}
}
