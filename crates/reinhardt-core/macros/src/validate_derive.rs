//! Derive macro for struct-level validation
//!
//! Supports `#[validate(email)]`, `#[validate(url)]`,
//! `#[validate(length(min = N, max = M))]`, and
//! `#[validate(range(min = N, max = M))]` field attributes
//! with optional `message = "..."` for custom error messages.
//! `Option<T>` fields are skipped when `None`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
	Attribute, Data, DeriveInput, Expr, ExprLit, Fields, Ident, Lit, Meta, Token, Type,
	parse::Parser, punctuated::Punctuated,
};

use crate::crate_paths::get_reinhardt_core_crate;

enum ValidationRule {
	Email {
		message: Option<String>,
	},
	Url {
		message: Option<String>,
	},
	Length {
		min: Option<usize>,
		max: Option<usize>,
		message: Option<String>,
	},
	Range {
		min: Option<Box<Expr>>,
		max: Option<Box<Expr>>,
		message: Option<String>,
	},
}

pub(crate) fn validate_derive_impl(input: DeriveInput) -> syn::Result<TokenStream> {
	let name = &input.ident;
	let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
	let core_crate = get_reinhardt_core_crate();

	let fields = match &input.data {
		Data::Struct(data) => match &data.fields {
			Fields::Named(fields) => &fields.named,
			_ => {
				return Err(syn::Error::new_spanned(
					&input,
					"#[derive(Validate)] requires named fields",
				));
			}
		},
		_ => {
			return Err(syn::Error::new_spanned(
				&input,
				"#[derive(Validate)] can only be used on structs",
			));
		}
	};

	let mut field_validations = Vec::new();

	for field in fields {
		let field_name = field.ident.as_ref().unwrap();
		let field_name_str = field_name.to_string();
		let is_option = is_option_type(&field.ty);

		for attr in &field.attrs {
			if !attr.path().is_ident("validate") {
				continue;
			}

			let rules = parse_validate_attr(attr)?;
			for rule in rules {
				let code =
					generate_validation(&core_crate, field_name, &field_name_str, is_option, &rule);
				field_validations.push(code);
			}
		}
	}

	Ok(quote! {
		impl #impl_generics #core_crate::validators::Validate for #name #ty_generics #where_clause {
			fn validate(&self) -> ::core::result::Result<(), #core_crate::validators::ValidationErrors> {
				use #core_crate::validators::Validator as _;
				let mut errors = #core_crate::validators::ValidationErrors::new();
				#(#field_validations)*
				if errors.is_empty() { Ok(()) } else { Err(errors) }
			}
		}
	})
}

fn parse_validate_attr(attr: &Attribute) -> syn::Result<Vec<ValidationRule>> {
	let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
	let mut rules = Vec::new();

	for meta in nested {
		let rule = parse_single_rule(&meta)?;
		rules.push(rule);
	}

	Ok(rules)
}

fn parse_single_rule(meta: &Meta) -> syn::Result<ValidationRule> {
	match meta {
		Meta::Path(path) => {
			if path.is_ident("email") {
				Ok(ValidationRule::Email { message: None })
			} else if path.is_ident("url") {
				Ok(ValidationRule::Url { message: None })
			} else {
				Err(syn::Error::new_spanned(
					path,
					"unsupported validation rule (expected `email`, `url`, `length`, or `range`)",
				))
			}
		}
		Meta::List(list) => {
			if list.path.is_ident("email") {
				let message = extract_message_from_tokens(&list.tokens)?;
				Ok(ValidationRule::Email { message })
			} else if list.path.is_ident("url") {
				let message = extract_message_from_tokens(&list.tokens)?;
				Ok(ValidationRule::Url { message })
			} else if list.path.is_ident("length") {
				parse_length_rule(&list.tokens)
			} else if list.path.is_ident("range") {
				parse_range_rule(&list.tokens)
			} else {
				Err(syn::Error::new_spanned(
					&list.path,
					"unsupported validation rule (expected `email`, `url`, `length`, or `range`)",
				))
			}
		}
		Meta::NameValue(nv) => Err(syn::Error::new_spanned(
			nv,
			"unexpected name = value in #[validate(...)]",
		)),
	}
}

fn extract_message_from_tokens(tokens: &TokenStream) -> syn::Result<Option<String>> {
	let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(tokens.clone())?;
	for meta in metas {
		if let Meta::NameValue(nv) = meta
			&& nv.path.is_ident("message")
			&& let Expr::Lit(ExprLit {
				lit: Lit::Str(lit_str),
				..
			}) = &nv.value
		{
			return Ok(Some(lit_str.value()));
		}
	}
	Ok(None)
}

fn parse_length_rule(tokens: &TokenStream) -> syn::Result<ValidationRule> {
	let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(tokens.clone())?;

	let mut min = None;
	let mut max = None;
	let mut message = None;

	for meta in metas {
		match meta {
			Meta::NameValue(nv) => {
				if nv.path.is_ident("min") {
					min = Some(parse_usize_expr(&nv.value)?);
				} else if nv.path.is_ident("max") {
					max = Some(parse_usize_expr(&nv.value)?);
				} else if nv.path.is_ident("message")
					&& let Expr::Lit(ExprLit {
						lit: Lit::Str(lit_str),
						..
					}) = &nv.value
				{
					message = Some(lit_str.value());
				}
			}
			other => {
				return Err(syn::Error::new_spanned(
					other,
					"expected `min = N`, `max = N`, or `message = \"...\"` in length()",
				));
			}
		}
	}

	if min.is_none() && max.is_none() {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"length() requires at least `min` or `max`",
		));
	}

	Ok(ValidationRule::Length { min, max, message })
}

fn parse_usize_expr(expr: &Expr) -> syn::Result<usize> {
	if let Expr::Lit(ExprLit {
		lit: Lit::Int(lit_int),
		..
	}) = expr
	{
		lit_int.base10_parse::<usize>()
	} else {
		Err(syn::Error::new_spanned(expr, "expected integer literal"))
	}
}

fn parse_range_rule(tokens: &TokenStream) -> syn::Result<ValidationRule> {
	let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(tokens.clone())?;

	let mut min = None;
	let mut max = None;
	let mut message = None;

	for meta in metas {
		match meta {
			Meta::NameValue(nv) => {
				if nv.path.is_ident("min") {
					min = Some(Box::new(nv.value.clone()));
				} else if nv.path.is_ident("max") {
					max = Some(Box::new(nv.value.clone()));
				} else if nv.path.is_ident("message")
					&& let Expr::Lit(ExprLit {
						lit: Lit::Str(lit_str),
						..
					}) = &nv.value
				{
					message = Some(lit_str.value());
				}
			}
			other => {
				return Err(syn::Error::new_spanned(
					other,
					"expected `min = N`, `max = N`, or `message = \"...\"` in range()",
				));
			}
		}
	}

	if min.is_none() && max.is_none() {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"range() requires at least `min` or `max`",
		));
	}

	Ok(ValidationRule::Range { min, max, message })
}

fn generate_validation(
	core_crate: &TokenStream,
	field_name: &Ident,
	field_name_str: &str,
	is_option: bool,
	rule: &ValidationRule,
) -> TokenStream {
	let checks = generate_checks(core_crate, rule);

	if is_option {
		quote! {
			if let Some(ref __value) = self.#field_name {
				#(
					{
						let __check_result: ::core::result::Result<(), #core_crate::validators::ValidationError> = #checks;
						if let Err(__e) = __check_result {
							errors.add(#field_name_str, __e);
						}
					}
				)*
			}
		}
	} else {
		quote! {
			{
				let __value = &self.#field_name;
				#(
					{
						let __check_result: ::core::result::Result<(), #core_crate::validators::ValidationError> = #checks;
						if let Err(__e) = __check_result {
							errors.add(#field_name_str, __e);
						}
					}
				)*
			}
		}
	}
}

fn generate_checks(core_crate: &TokenStream, rule: &ValidationRule) -> Vec<TokenStream> {
	match rule {
		ValidationRule::Email { message } => {
			vec![wrap_message(
				core_crate,
				quote! {
					#core_crate::validators::EmailValidator::new().validate(__value)
				},
				message,
			)]
		}
		ValidationRule::Url { message } => {
			vec![wrap_message(
				core_crate,
				quote! {
					#core_crate::validators::UrlValidator::new().validate(__value)
				},
				message,
			)]
		}
		ValidationRule::Length { min, max, message } => {
			let mut checks = Vec::new();
			if let Some(min_val) = min {
				checks.push(wrap_message(
					core_crate,
					quote! {
						#core_crate::validators::MinLengthValidator::new(#min_val).validate(__value)
					},
					message,
				));
			}
			if let Some(max_val) = max {
				checks.push(wrap_message(
					core_crate,
					quote! {
						#core_crate::validators::MaxLengthValidator::new(#max_val).validate(__value)
					},
					message,
				));
			}
			checks
		}
		ValidationRule::Range { min, max, message } => {
			let mut checks = Vec::new();
			if let Some(min_val) = min {
				checks.push(wrap_message(
					core_crate,
					quote! {
						#core_crate::validators::MinValueValidator::new(#min_val).validate(__value)
					},
					message,
				));
			}
			if let Some(max_val) = max {
				checks.push(wrap_message(
					core_crate,
					quote! {
						#core_crate::validators::MaxValueValidator::new(#max_val).validate(__value)
					},
					message,
				));
			}
			checks
		}
	}
}

/// Wraps a validator call to use a custom message if specified.
fn wrap_message(
	core_crate: &TokenStream,
	validator_call: TokenStream,
	message: &Option<String>,
) -> TokenStream {
	if let Some(msg) = message {
		quote! {
			#validator_call.map_err(|_| #core_crate::validators::ValidationError::Custom(#msg.to_string()))
		}
	} else {
		validator_call
	}
}

fn is_option_type(ty: &Type) -> bool {
	if let Type::Path(type_path) = ty
		&& let Some(segment) = type_path.path.segments.last()
	{
		return segment.ident == "Option";
	}
	false
}
