// Copyright 2024-2025 reinhardt-query authors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at
//
// Unless required by applicable law or agreed to in writing, software distributed under
// the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. See the License for the specific language governing
// permissions and limitations under the License.

//! Procedural macros for reinhardt-query
//!
//! Provides custom derive macros for SQL identifier name generation:
//! - `#[derive(Iden)]` - Automatically generates SQL-safe identifiers
//!
//! ## Features
//!
//! - Supports both list-style `#[iden("custom_name")]` and name-value style `#[iden = "custom_name"]` attributes
//! - Generates snake_case identifiers for SQL compatibility
//! - Supports unit variants, named variants, and tuple variants in enums
//! - Supports all struct types (unit, named fields, and tuple structs)
//!
//! For enums:
//! - `Table` variant uses enum name converted to snake_case
//! - Other variants use their name converted to snake_case
//!
//! For structs:
//! - Uses the struct name converted to snake_case
//! - `#[iden = "custom_name"]` overrides the generated name
//!
//! # Examples
//!
//! ## Enum with custom identifier
//!
//! ```rust,ignore
//! #[derive(Iden)]
//! enum Users {
//!     Table,           // -> "users"
//!     #[iden = "primary_key"]
//!     Id,              // -> "id"
//!     FirstName,       // -> "first_name"
//!     #[iden("email_address")]
//!     Email,           // -> "email_address"
//! }
//!
//! let users = Users::Table;
//! assert_eq!(users.iden(), "users");
//! assert_eq!(users.id(), "id");
//! assert_eq!(users.first_name.iden(), "first_name");
//! assert_eq!(users.email_address.iden(), "email_address");
//! ```
//!
//! ## Struct with custom identifier
//!
//! ```rust,ignore
//! #[derive(Iden)]
//! struct Customer {
//!     #[iden = "customer_id"]
//!     Id,
//!     Name,
//! }
//!
//! let customer = Customer::Iden;
//! assert_eq!(customer.iden(), "customer_id");
//! ```
//!
//! ## Struct with named fields
//!
//! ```rust,ignore
//! #[derive(Iden)]
//! struct User {
//!     Id,
//!     #[iden = "uuid"]
//!     ExternalId,
//!     Email,
//!     Verified,
//! }
//!
//! let user = User::Iden;
//! assert_eq!(user.iden(), "uuid");
//! assert_eq!(user.external_iden(), "external_id");
//! assert_eq!(user.emailen(), "email");
//! assert_eq!(user.verifieden(), "verified");
//! ```

use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, parse_macro_input};

/// Characters that are unsafe in SQL identifiers.
const SQL_UNSAFE_CHARS: [char; 5] = [';', '\'', '"', '\\', '`'];

/// Validate a custom identifier name for SQL safety.
/// Rejects empty strings, null bytes, control characters, and SQL-unsafe characters.
// Fixes #794
fn validate_iden_name(name: &str, span: proc_macro2::Span) -> Result<(), syn::Error> {
	if name.is_empty() {
		return Err(syn::Error::new(span, "identifier name must not be empty"));
	}
	if name.contains('\0') {
		return Err(syn::Error::new(
			span,
			"identifier name must not contain null bytes",
		));
	}
	if name.chars().any(|c| c.is_control()) {
		return Err(syn::Error::new(
			span,
			"identifier name must not contain control characters",
		));
	}
	if let Some(c) = name.chars().find(|c| SQL_UNSAFE_CHARS.contains(c)) {
		return Err(syn::Error::new(
			span,
			format!("identifier name contains SQL-unsafe character: '{c}'"),
		));
	}
	if name.contains("--") || name.contains("/*") {
		return Err(syn::Error::new(
			span,
			"identifier name contains SQL comment sequence",
		));
	}
	Ok(())
}

/// Extract and validate custom identifier name from `#[iden = "..."]` attribute.
// Fixes #794
fn extract_custom_name(attrs: &[syn::Attribute]) -> Result<Option<String>, syn::Error> {
	for attr in attrs {
		if !attr.path().is_ident("iden") {
			continue;
		}
		match &attr.meta {
			syn::Meta::NameValue(name_value) => {
				if let syn::Expr::Lit(lit) = &name_value.value
					&& let syn::Lit::Str(lit_str) = &lit.lit
				{
					let value = lit_str.value();
					validate_iden_name(&value, lit_str.span())?;
					return Ok(Some(value));
				}
				return Err(syn::Error::new_spanned(
					&name_value.value,
					"#[iden = ...] expects a string literal, e.g., #[iden = \"custom_name\"]",
				));
			}
			syn::Meta::List(list) => {
				let lit: syn::LitStr = list.parse_args().map_err(|_| {
					syn::Error::new_spanned(
						list,
						"#[iden(...)] expects a single string literal, e.g., #[iden(\"custom_name\")]",
					)
				})?;
				let value = lit.value();
				validate_iden_name(&value, lit.span())?;
				return Ok(Some(value));
			}
			syn::Meta::Path(_) => {
				return Err(syn::Error::new_spanned(
					attr,
					"#[iden] requires a value, use #[iden = \"name\"] or #[iden(\"name\")]",
				));
			}
		}
	}
	Ok(None)
}

/// Derive `Iden` trait for enums and structs.
///
/// ## Features
///
/// - Supports both list-style `#[iden("custom_name")]` and name-value style `#[iden = "custom_name"]` attributes
/// - Generates snake_case identifiers for SQL compatibility
/// - Supports unit variants, named variants, and tuple variants in enums
/// - Supports all struct types (unit, named fields, and tuple structs)
///
/// For enums:
/// - `Table` variant uses enum name converted to snake_case
/// - Other variants use their name converted to snake_case
///
/// For structs:
/// - Uses the struct name converted to snake_case
/// - `#[iden = "custom_name"]` overrides the generated name
///
/// # Examples
///
/// ## Enum with custom identifier
///
/// ```rust,ignore
/// #[derive(Iden)]
/// enum Users {
///     Table,           // -> "users"
///     #[iden = "primary_key"]
///     Id,              // -> "id"
///     FirstName,       // -> "first_name"
///     #[iden("email_address")]
///     Email,           // -> "email_address"
/// }
///
/// let users = Users::Table;
/// assert_eq!(users.iden(), "users");
/// assert_eq!(users.id(), "id");
/// assert_eq!(users.first_name.iden(), "first_name");
/// assert_eq!(users.email_address.iden(), "email_address");
/// ```
///
/// ## Struct with custom identifier
///
/// ```rust,ignore
/// #[derive(Iden)]
/// struct Customer {
///     #[iden = "customer_id"]
///     Id,
///     Name,
/// }
///
/// let customer = Customer::Iden;
/// assert_eq!(customer.iden(), "customer_id");
/// ```
///
/// ## Struct with named fields
///
/// ```rust,ignore
/// #[derive(Iden)]
/// struct User {
///     Id,
///     #[iden = "uuid"]
///     ExternalId,
///     Email,
///     Verified,
/// }
///
/// let user = User::Iden;
/// assert_eq!(user.iden(), "uuid");
/// assert_eq!(user.external_iden(), "external_id");
/// assert_eq!(user.emailen(), "email");
/// assert_eq!(user.verifieden(), "verified");
/// ```
#[proc_macro_derive(Iden, attributes(iden))]
pub fn derive_iden(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	match derive_iden_impl(&input) {
		Ok(tokens) => tokens.into(),
		Err(err) => err.to_compile_error().into(),
	}
}

// Fixes #792
/// Generate a match pattern for an enum variant, handling associated data.
/// - Unit variants: `VariantName`
/// - Tuple variants: `VariantName(..)`
/// - Named variants: `VariantName { .. }`
fn generate_variant_pattern(
	enum_name: &syn::Ident,
	variant: &syn::Variant,
) -> proc_macro2::TokenStream {
	let variant_ident = &variant.ident;
	match &variant.fields {
		syn::Fields::Unit => quote! { #enum_name::#variant_ident },
		syn::Fields::Unnamed(_) => quote! { #enum_name::#variant_ident(..) },
		syn::Fields::Named(_) => quote! { #enum_name::#variant_ident { .. } },
	}
}

/// Internal implementation of `derive_iden` that returns a `Result` for proper error propagation.
fn derive_iden_impl(input: &DeriveInput) -> Result<proc_macro2::TokenStream, syn::Error> {
	let name = &input.ident;

	match &input.data {
		Data::Enum(data_enum) => {
			// Collect variant data: (pattern, identifier name)
			// Fixes #792: Include variant pattern to handle associated data
			let variant_data: Vec<_> = data_enum
				.variants
				.iter()
				.map(|variant| {
					let variant_ident = &variant.ident;
					let pattern = generate_variant_pattern(name, variant);
					let iden_name = extract_custom_name(&variant.attrs)?.unwrap_or_else(|| {
						if variant_ident == "Table" {
							// Convention: Table variant uses enum name as table identifier
							name.to_string().to_snake_case()
						} else {
							variant_ident.to_string().to_snake_case()
						}
					});
					Ok((pattern, iden_name))
				})
				.collect::<Result<_, syn::Error>>()?;

			// Generate Display match arms
			// Fixes #792: Use pattern instead of simple variant ident
			let display_arms = variant_data.iter().map(|(pattern, iden_name)| {
				quote! {
					#pattern => f.write_str(#iden_name),
				}
			});

			// Generate Iden::unquoted match arms
			// Fixes #792: Use pattern instead of simple variant ident
			// Iden::unquoted contract: writer is expected to not fail
			let iden_arms = variant_data.iter().map(|(pattern, iden_name)| {
				quote! {
					#pattern => s.write_str(#iden_name).expect("write to String is infallible"),
				}
			});

			Ok(quote! {
				// Fixes #808: Compile-time assertion that Debug is implemented,
				// required by the Iden supertrait
				const _: () = {
					fn __assert_debug<T: ::std::fmt::Debug>() {}
					fn __check() { __assert_debug::<#name>(); }
				};

				impl ::std::fmt::Display for #name {
					fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
						match self {
							#(#display_arms)*
						}
					}
				}

				impl reinhardt_query::types::Iden for #name {
					fn unquoted(&self, s: &mut dyn ::std::fmt::Write) {
						match self {
							#(#iden_arms)*
						}
					}
				}
			})
		}

		Data::Struct(_data_struct) => {
			let iden_name = extract_custom_name(&input.attrs)?
				.unwrap_or_else(|| name.to_string().to_snake_case());

			Ok(quote! {
				// Fixes #808: Compile-time assertion that Debug is implemented,
				// required by the Iden supertrait
				const _: () = {
					fn __assert_debug<T: ::std::fmt::Debug>() {}
					fn __check() { __assert_debug::<#name>(); }
				};

				impl ::std::fmt::Display for #name {
					fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
						f.write_str(#iden_name)
					}
				}

				impl reinhardt_query::types::Iden for #name {
					fn unquoted(&self, s: &mut dyn ::std::fmt::Write) {
						// Iden::unquoted contract: writer is expected to not fail
						s.write_str(#iden_name).expect("write to String is infallible");
					}
				}
			})
		}

		_ => Err(syn::Error::new_spanned(
			&input.ident,
			"`#[derive(Iden)]` only supports enums and structs",
		)),
	}
}
