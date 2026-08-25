use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Error, Field, Fields, ItemStruct, Result, parse_macro_input};

use crate::crate_paths::get_reinhardt_pages_crate;
use crate::type_utils::option_inner_type;

pub(crate) fn page_props_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	if !args.is_empty() {
		return Error::new(
			proc_macro2::Span::call_site(),
			"#[page_props] does not accept arguments",
		)
		.to_compile_error()
		.into();
	}

	let input = parse_macro_input!(input as ItemStruct);
	expand_page_props(input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

fn expand_page_props(mut item: ItemStruct) -> Result<proc_macro2::TokenStream> {
	let ident = &item.ident;
	let pages_crate = get_reinhardt_pages_crate();
	let fields = match &mut item.fields {
		Fields::Named(fields) => &mut fields.named,
		_ => {
			return Err(Error::new_spanned(
				&item.fields,
				"#[page_props] requires a struct with named fields",
			));
		}
	};

	let mut keys = HashSet::new();
	let mut initializers = Vec::new();
	for field in fields.iter_mut() {
		let field_ident = field
			.ident
			.as_ref()
			.ok_or_else(|| Error::new_spanned(&field, "#[page_props] requires named fields"))?
			.clone();
		let source = take_source(field)?;
		let ty = &field.ty;
		let key = source.key.unwrap_or_else(|| field_ident.to_string());
		if !keys.insert(key.clone()) {
			return Err(Error::new_spanned(
				&field,
				format!("duplicate from_request key `{key}`"),
			));
		}
		let extractor = match source.kind {
			SourceKind::Path => quote! {
				#pages_crate::router::request::PathParam::<#ty>::extract(ctx, #key)?.into_inner()
			},
			SourceKind::Query => {
				if let Some(inner) = option_inner_type(ty) {
					quote! {
						#pages_crate::router::request::OptionalQueryParam::<#inner>::extract(ctx, #key)?.into_inner()
					}
				} else {
					quote! {
						#pages_crate::router::request::QueryParam::<#ty>::extract(ctx, #key)?.into_inner()
					}
				}
			}
		};
		initializers.push(quote! { #field_ident: #extractor });
	}
	let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();

	Ok(quote! {
		#[derive(#pages_crate::__private::bon::Builder)]
		#[builder(crate = #pages_crate::__private::bon)]
		#item

		impl #impl_generics #pages_crate::router::request::FromRequest for #ident #ty_generics
			#where_clause
		{
			fn from_request(
				ctx: &#pages_crate::router::request::RouteContext,
			) -> ::std::result::Result<Self, #pages_crate::router::request::ExtractError> {
				::std::result::Result::Ok(Self {
					#(#initializers,)*
				})
			}
		}
	})
}

#[derive(Clone, Copy)]
enum SourceKind {
	Path,
	Query,
}

struct Source {
	kind: SourceKind,
	key: Option<String>,
}

fn take_source(field: &mut Field) -> Result<Source> {
	let mut source = None;
	let mut key = None;
	let mut kept = Vec::new();
	for attr in field.attrs.drain(..) {
		if !attr.path().is_ident("from_request") {
			kept.push(attr);
			continue;
		}
		attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("path") {
				set_source(&mut source, SourceKind::Path)?;
				return Ok(());
			}
			if meta.path.is_ident("query") {
				set_source(&mut source, SourceKind::Query)?;
				return Ok(());
			}
			if meta.path.is_ident("name") {
				let value = meta.value()?;
				let lit: syn::LitStr = value.parse()?;
				key = Some(lit.value());
				return Ok(());
			}
			Err(meta.error("expected `path`, `query`, or `name = \"...\"`"))
		})?;
	}
	field.attrs = kept;
	let kind = source.ok_or_else(|| {
		Error::new_spanned(
			field,
			"#[page_props] fields require #[from_request(path)] or #[from_request(query)]",
		)
	})?;
	Ok(Source { kind, key })
}

fn set_source(source: &mut Option<SourceKind>, kind: SourceKind) -> Result<()> {
	if source.is_some() {
		return Err(Error::new(
			proc_macro2::Span::call_site(),
			"expected only one of `path` or `query`",
		));
	}
	*source = Some(kind);
	Ok(())
}
