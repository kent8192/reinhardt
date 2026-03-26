//! Admin macro implementation
//!
//! This module provides the `#[admin(model, ...)]` attribute macro for
//! automatically implementing the `ModelAdmin` trait.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
	Ident, ItemStruct, LitBool, LitInt, LitStr, Result, Token, Type, bracketed, parenthesized,
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
};

/// Custom keywords for admin macro
mod kw {
	syn::custom_keyword!(model);
	syn::custom_keyword!(asc);
	syn::custom_keyword!(desc);
	syn::custom_keyword!(allow_all);
}

/// Order direction for sorting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Order {
	Asc,
	Desc,
}

impl Parse for Order {
	fn parse(input: ParseStream) -> Result<Self> {
		let lookahead = input.lookahead1();
		if lookahead.peek(kw::asc) {
			input.parse::<kw::asc>()?;
			Ok(Order::Asc)
		} else if lookahead.peek(kw::desc) {
			input.parse::<kw::desc>()?;
			Ok(Order::Desc)
		} else {
			Err(lookahead.error())
		}
	}
}

/// Ordering specification: (field_name, order)
#[derive(Debug, Clone)]
pub(crate) struct OrderingSpec {
	pub field: Ident,
	pub order: Order,
}

impl Parse for OrderingSpec {
	fn parse(input: ParseStream) -> Result<Self> {
		let content;
		parenthesized!(content in input);
		let field: Ident = content.parse()?;
		content.parse::<Token![,]>()?;
		let order: Order = content.parse()?;
		Ok(OrderingSpec { field, order })
	}
}

/// Parsed configuration from `#[admin(model, ...)]`
#[derive(Debug)]
pub(crate) struct AdminModelConfig {
	/// The model type (for = ModelType)
	pub model_type: Type,
	/// The model name (name = "ModelName")
	pub name: String,
	/// Fields to display in list view
	pub list_display: Option<Vec<Ident>>,
	/// Fields that can be used for filtering
	pub list_filter: Option<Vec<Ident>>,
	/// Fields that can be searched
	pub search_fields: Option<Vec<Ident>>,
	/// Fields to display in forms
	pub fields: Option<Vec<Ident>>,
	/// Read-only fields
	pub readonly_fields: Option<Vec<Ident>>,
	/// Ordering specification
	pub ordering: Option<Vec<OrderingSpec>>,
	/// Number of items per page
	pub list_per_page: Option<usize>,
	/// Individual permission flags
	pub allow_view: Option<bool>,
	pub allow_add: Option<bool>,
	pub allow_change: Option<bool>,
	pub allow_delete: Option<bool>,
	/// Permission preset (e.g., "allow_all")
	pub permissions: Option<String>,
}

impl Parse for AdminModelConfig {
	fn parse(input: ParseStream) -> Result<Self> {
		let span = input.span();

		// Parse 'model' keyword first
		if !input.peek(kw::model) {
			return Err(syn::Error::new(
				span,
				"expected `model` keyword in #[admin(...)]\n\n  = help: use `#[admin(model, for = ModelType, name = \"ModelName\", ...)]`",
			));
		}
		input.parse::<kw::model>()?;

		// Comma after 'model'
		if input.peek(Token![,]) {
			input.parse::<Token![,]>()?;
		}

		let mut model_type: Option<Type> = None;
		let mut name: Option<String> = None;
		let mut list_display: Option<Vec<Ident>> = None;
		let mut list_filter: Option<Vec<Ident>> = None;
		let mut search_fields: Option<Vec<Ident>> = None;
		let mut fields: Option<Vec<Ident>> = None;
		let mut readonly_fields: Option<Vec<Ident>> = None;
		let mut ordering: Option<Vec<OrderingSpec>> = None;
		let mut list_per_page: Option<usize> = None;
		let mut allow_view: Option<bool> = None;
		let mut allow_add: Option<bool> = None;
		let mut allow_change: Option<bool> = None;
		let mut allow_delete: Option<bool> = None;
		let mut permissions: Option<String> = None;

		while !input.is_empty() {
			// Handle 'for' keyword specially since it's a reserved keyword
			if input.peek(Token![for]) {
				input.parse::<Token![for]>()?;
				input.parse::<Token![=]>()?;
				model_type = Some(input.parse()?);

				// Optional trailing comma
				if input.peek(Token![,]) {
					input.parse::<Token![,]>()?;
				}
				continue;
			}

			let key: Ident = input.parse()?;
			input.parse::<Token![=]>()?;

			match key.to_string().as_str() {
				"name" => {
					let lit: LitStr = input.parse()?;
					name = Some(lit.value());
				}
				"list_display" => {
					list_display = Some(parse_ident_array(input)?);
				}
				"list_filter" => {
					list_filter = Some(parse_ident_array(input)?);
				}
				"search_fields" => {
					search_fields = Some(parse_ident_array(input)?);
				}
				"fields" => {
					fields = Some(parse_ident_array(input)?);
				}
				"readonly_fields" => {
					readonly_fields = Some(parse_ident_array(input)?);
				}
				"ordering" => {
					ordering = Some(parse_ordering_array(input)?);
				}
				"list_per_page" => {
					let lit: LitInt = input.parse()?;
					list_per_page = Some(lit.base10_parse()?);
				}
				"allow_view" => {
					let lit: LitBool = input.parse()?;
					allow_view = Some(lit.value());
				}
				"allow_add" => {
					let lit: LitBool = input.parse()?;
					allow_add = Some(lit.value());
				}
				"allow_change" => {
					let lit: LitBool = input.parse()?;
					allow_change = Some(lit.value());
				}
				"allow_delete" => {
					let lit: LitBool = input.parse()?;
					allow_delete = Some(lit.value());
				}
				"permissions" => {
					let ident: Ident = input.parse()?;
					match ident.to_string().as_str() {
						"allow_all" => {
							permissions = Some("allow_all".to_string());
						}
						other => {
							return Err(syn::Error::new(
								ident.span(),
								format!(
									"unknown permission preset `{}`\n\n  = help: valid presets are: allow_all",
									other
								),
							));
						}
					}
				}
				unknown => {
					return Err(syn::Error::new(
						key.span(),
						format!(
							"unknown attribute `{}` for model admin\n\n  = help: valid attributes are: for, name, list_display, list_filter, search_fields, fields, readonly_fields, ordering, list_per_page, allow_view, allow_add, allow_change, allow_delete, permissions",
							unknown
						),
					));
				}
			}

			// Optional trailing comma
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		// Validate required fields
		let model_type = model_type.ok_or_else(|| {
			syn::Error::new(
				span,
				"`for` attribute is required for model admin\n\n  = help: add `for = ModelType` to specify the model type",
			)
		})?;

		let name = name.ok_or_else(|| {
			syn::Error::new(
				span,
				"`name` attribute is required for model admin\n\n  = help: add `name = \"ModelName\"` to specify the model name",
			)
		})?;

		Ok(AdminModelConfig {
			model_type,
			name,
			list_display,
			list_filter,
			search_fields,
			fields,
			readonly_fields,
			ordering,
			list_per_page,
			allow_view,
			allow_add,
			allow_change,
			allow_delete,
			permissions,
		})
	}
}

/// Parse an array of identifiers: [id, name, email]
fn parse_ident_array(input: ParseStream) -> Result<Vec<Ident>> {
	let content;
	bracketed!(content in input);

	let mut idents = Vec::new();
	while !content.is_empty() {
		idents.push(content.parse()?);
		if content.peek(Token![,]) {
			content.parse::<Token![,]>()?;
		} else {
			break;
		}
	}
	Ok(idents)
}

/// Parse an array of ordering specs: [(field, asc), (field, desc)]
fn parse_ordering_array(input: ParseStream) -> Result<Vec<OrderingSpec>> {
	let content;
	bracketed!(content in input);

	let specs: Punctuated<OrderingSpec, Token![,]> = content.call(Punctuated::parse_terminated)?;
	Ok(specs.into_iter().collect())
}

/// Generate the ModelAdmin trait implementation
pub(crate) fn admin_impl(args: TokenStream, input: ItemStruct) -> Result<TokenStream> {
	let admin_api = crate::crate_paths::get_reinhardt_admin_adapters_crate();
	let async_trait = crate::crate_paths::get_async_trait_crate();
	let orm_crate = crate::crate_paths::get_reinhardt_orm_crate();

	let config: AdminModelConfig = syn::parse2(args)?;
	let struct_name = &input.ident;
	let struct_vis = &input.vis;
	let struct_attrs = &input.attrs;

	let model_type = &config.model_type;
	let name = &config.name;

	// Collect all field identifiers for validation
	let mut all_fields: Vec<&Ident> = Vec::new();
	if let Some(ref fields) = config.list_display {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.list_filter {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.search_fields {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.fields {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.readonly_fields {
		all_fields.extend(fields.iter());
	}
	if let Some(ref ordering) = config.ordering {
		all_fields.extend(ordering.iter().map(|o| &o.field));
	}

	// Generate field validation code
	let field_checks: Vec<TokenStream> = all_fields
		.iter()
		.map(|field| {
			let method_name = Ident::new(&format!("field_{}", field), field.span());
			quote! {
				let _ = #model_type::#method_name;
			}
		})
		.collect();

	// Generate table_name method from Model trait (Issue #2929)
	let table_name_impl = quote! {
		fn table_name(&self) -> &str {
			<#model_type as #orm_crate::Model>::table_name()
		}
	};

	// Generate list_display method
	let list_display_impl = if let Some(ref fields) = config.list_display {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn list_display(&self) -> Vec<&str> {
				vec![#(#field_strs),*]
			}
		}
	} else {
		quote! {}
	};

	// Generate list_filter method
	let list_filter_impl = if let Some(ref fields) = config.list_filter {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn list_filter(&self) -> Vec<&str> {
				vec![#(#field_strs),*]
			}
		}
	} else {
		quote! {}
	};

	// Generate search_fields method
	let search_fields_impl = if let Some(ref fields) = config.search_fields {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn search_fields(&self) -> Vec<&str> {
				vec![#(#field_strs),*]
			}
		}
	} else {
		quote! {}
	};

	// Generate fields method
	let fields_impl = if let Some(ref fields) = config.fields {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn fields(&self) -> Option<Vec<&str>> {
				Some(vec![#(#field_strs),*])
			}
		}
	} else {
		quote! {}
	};

	// Generate readonly_fields method
	let readonly_fields_impl = if let Some(ref fields) = config.readonly_fields {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn readonly_fields(&self) -> Vec<&str> {
				vec![#(#field_strs),*]
			}
		}
	} else {
		quote! {}
	};

	// Generate ordering method
	let ordering_impl = if let Some(ref ordering) = config.ordering {
		let ordering_strs: Vec<String> = ordering
			.iter()
			.map(|o| {
				let prefix = if o.order == Order::Desc { "-" } else { "" };
				format!("{}{}", prefix, o.field)
			})
			.collect();
		quote! {
			fn ordering(&self) -> Vec<&str> {
				vec![#(#ordering_strs),*]
			}
		}
	} else {
		quote! {}
	};

	// Generate list_per_page method
	let list_per_page_impl = if let Some(count) = config.list_per_page {
		quote! {
			fn list_per_page(&self) -> Option<usize> {
				Some(#count)
			}
		}
	} else {
		quote! {}
	};

	// Generate permission methods (Issue #2931)
	let (perm_view, perm_add, perm_change, perm_delete) =
		if config.permissions.as_deref() == Some("allow_all") {
			(true, true, true, true)
		} else {
			(
				config.allow_view.unwrap_or(false),
				config.allow_add.unwrap_or(false),
				config.allow_change.unwrap_or(false),
				config.allow_delete.unwrap_or(false),
			)
		};

	let permission_impls = quote! {
		async fn has_view_permission(&self, _user: &dyn #admin_api::AdminUser) -> bool {
			#perm_view
		}

		async fn has_add_permission(&self, _user: &dyn #admin_api::AdminUser) -> bool {
			#perm_add
		}

		async fn has_change_permission(&self, _user: &dyn #admin_api::AdminUser) -> bool {
			#perm_change
		}

		async fn has_delete_permission(&self, _user: &dyn #admin_api::AdminUser) -> bool {
			#perm_delete
		}
	};

	Ok(quote! {
		#(#struct_attrs)*
		#struct_vis struct #struct_name;

		// Compile-time field validation
		const _: () = {
			#(#field_checks)*
		};

		#[#async_trait::async_trait]
		impl #admin_api::ModelAdmin for #struct_name {
			fn model_name(&self) -> &str {
				#name
			}

			#table_name_impl
			#list_display_impl
			#list_filter_impl
			#search_fields_impl
			#fields_impl
			#readonly_fields_impl
			#ordering_impl
			#list_per_page_impl
			#permission_impls
		}
	})
}
