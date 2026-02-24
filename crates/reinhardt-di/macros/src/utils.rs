//! Common utilities for DI macros

use crate::crate_paths::get_reinhardt_di_crate;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, MetaNameValue, Result, Token, parse::Parse, punctuated::Punctuated};

/// Dependency scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Scope {
	#[default]
	Singleton,
	Request,
	Transient,
}

impl Scope {
	/// Convert to TokenStream for code generation
	pub(crate) fn into_tokens(self) -> TokenStream {
		let di_crate = get_reinhardt_di_crate();
		match self {
			Scope::Singleton => quote! { #di_crate::DependencyScope::Singleton },
			Scope::Request => quote! { #di_crate::DependencyScope::Request },
			Scope::Transient => quote! { #di_crate::DependencyScope::Transient },
		}
	}
}

/// Check if an attribute is `#[inject]`
pub(crate) fn is_inject_attr(attr: &Attribute) -> bool {
	attr.path().is_ident("inject")
}

/// Macro arguments structure
pub(crate) struct MacroArgs {
	pub scope: Option<Scope>,
}

/// Known argument names for DI macros
const KNOWN_ARGS: &[&str] = &["scope"];

impl Parse for MacroArgs {
	fn parse(input: syn::parse::ParseStream) -> Result<Self> {
		let mut scope = None;

		// Parse key-value pairs separated by commas
		let parsed = Punctuated::<MetaNameValue, Token![,]>::parse_terminated(input)?;

		for pair in parsed {
			if pair.path.is_ident("scope") {
				if let syn::Expr::Lit(syn::ExprLit {
					lit: syn::Lit::Str(lit_str),
					..
				}) = &pair.value
				{
					let value = lit_str.value();
					scope = Some(match value.as_str() {
						"singleton" => Scope::Singleton,
						"request" => Scope::Request,
						"transient" => Scope::Transient,
						_ => {
							return Err(syn::Error::new_spanned(
								lit_str,
								"Invalid scope. Expected 'singleton', 'request', or 'transient'",
							));
						}
					});
				} else {
					return Err(syn::Error::new_spanned(
						&pair.value,
						"Expected string literal for scope",
					));
				}
			} else {
				// Reject unknown arguments
				let known_list = KNOWN_ARGS.join(", ");
				return Err(syn::Error::new_spanned(
					&pair.path,
					format!(
						"unknown argument `{}`. Valid arguments are: {}",
						pair.path
							.get_ident()
							.map(|i| i.to_string())
							.unwrap_or_else(|| "?".to_string()),
						known_list,
					),
				));
			}
		}

		Ok(MacroArgs { scope })
	}
}

/// Extract scope from macro arguments
pub(crate) fn extract_scope_from_args(args: TokenStream) -> Result<Scope> {
	if args.is_empty() {
		return Ok(Scope::default());
	}

	let parsed: MacroArgs = syn::parse2(args)?;
	Ok(parsed.scope.unwrap_or_default())
}
