//! Admin macro implementation
//!
//! This module provides the `#[admin(model, ...)]` attribute macro for
//! automatically implementing the `ModelAdmin` trait.

use std::collections::HashSet;

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

/// Fieldset specification: (title = "Main", fields = [name], collapsed = true)
#[derive(Debug, Clone)]
pub(crate) struct FieldsetSpec {
	pub title: Option<String>,
	pub fields: Vec<Ident>,
	pub collapsed: bool,
}

impl Parse for FieldsetSpec {
	fn parse(input: ParseStream) -> Result<Self> {
		let content;
		parenthesized!(content in input);
		let span = content.span();
		let mut title = None;
		let mut fields = None;
		let mut collapsed = None;

		while !content.is_empty() {
			let key: Ident = content.parse()?;
			content.parse::<Token![=]>()?;

			match key.to_string().as_str() {
				"title" => {
					if title.is_some() {
						return Err(syn::Error::new(
							key.span(),
							"duplicate fieldset attribute `title`",
						));
					}
					let lit: LitStr = content.parse()?;
					title = Some(lit.value());
				}
				"fields" => {
					if fields.is_some() {
						return Err(syn::Error::new(
							key.span(),
							"duplicate fieldset attribute `fields`",
						));
					}
					fields = Some(parse_ident_array(&content)?);
				}
				"collapsed" => {
					if collapsed.is_some() {
						return Err(syn::Error::new(
							key.span(),
							"duplicate fieldset attribute `collapsed`",
						));
					}
					let lit: LitBool = content.parse()?;
					collapsed = Some(lit.value());
				}
				unknown => {
					return Err(syn::Error::new(
						key.span(),
						format!(
							"unknown fieldset attribute `{unknown}`\n\n  = help: valid attributes are: title, fields, collapsed"
						),
					));
				}
			}

			if !content.is_empty() {
				content.parse::<Token![,]>()?;
			}
		}

		let fields = fields
			.ok_or_else(|| syn::Error::new(span, "`fields` is required for each fieldset"))?;
		if fields.is_empty() {
			return Err(syn::Error::new(
				span,
				"fieldsets cannot contain empty groups",
			));
		}

		Ok(Self {
			title,
			fields,
			collapsed: collapsed.unwrap_or(false),
		})
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
	/// Relations to eager-load in list view
	pub list_select_related: Option<Vec<Ident>>,
	/// Date or datetime field used for hierarchical changelist navigation.
	pub date_hierarchy: Option<Ident>,
	/// Fields that can be edited directly in list view
	pub list_editable: Option<Vec<Ident>>,
	/// Fields that can be used for filtering
	pub list_filter: Option<Vec<Ident>>,
	/// Fields that can be searched
	pub search_fields: Option<Vec<Ident>>,
	/// Many-to-many fields rendered with a horizontal selector
	pub filter_horizontal: Option<Vec<Ident>>,
	/// Many-to-many fields rendered with a vertical selector
	pub filter_vertical: Option<Vec<Ident>>,
	/// Fields to display in forms
	pub fields: Option<Vec<Ident>>,
	/// Fieldsets to display in forms
	pub fieldsets: Option<Vec<FieldsetSpec>>,
	/// Read-only fields
	pub readonly_fields: Option<Vec<Ident>>,
	/// Relation fields rendered with autocomplete controls
	pub autocomplete_fields: Option<Vec<Ident>>,
	/// Relation fields rendered as raw ID inputs
	pub raw_id_fields: Option<Vec<Ident>>,
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
		let mut list_select_related: Option<Vec<Ident>> = None;
		let mut date_hierarchy: Option<Ident> = None;
		let mut list_editable: Option<Vec<Ident>> = None;
		let mut list_filter: Option<Vec<Ident>> = None;
		let mut search_fields: Option<Vec<Ident>> = None;
		let mut filter_horizontal: Option<Vec<Ident>> = None;
		let mut filter_vertical: Option<Vec<Ident>> = None;
		let mut fields: Option<Vec<Ident>> = None;
		let mut fieldsets: Option<Vec<FieldsetSpec>> = None;
		let mut readonly_fields: Option<Vec<Ident>> = None;
		let mut autocomplete_fields: Option<Vec<Ident>> = None;
		let mut raw_id_fields: Option<Vec<Ident>> = None;
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
				"list_select_related" => {
					list_select_related = Some(parse_ident_array(input)?);
				}
				"date_hierarchy" => {
					date_hierarchy = Some(input.parse()?);
				}
				"list_editable" => {
					list_editable = Some(parse_ident_array(input)?);
				}
				"list_filter" => {
					list_filter = Some(parse_ident_array(input)?);
				}
				"search_fields" => {
					search_fields = Some(parse_ident_array(input)?);
				}
				"filter_horizontal" => {
					filter_horizontal = Some(parse_ident_array(input)?);
				}
				"filter_vertical" => {
					filter_vertical = Some(parse_ident_array(input)?);
				}
				"fields" => {
					if fieldsets.is_some() {
						return Err(syn::Error::new(
							key.span(),
							"`fields` and `fieldsets` cannot be configured together",
						));
					}
					fields = Some(parse_ident_array(input)?);
				}
				"fieldsets" => {
					if fields.is_some() {
						return Err(syn::Error::new(
							key.span(),
							"`fields` and `fieldsets` cannot be configured together",
						));
					}
					if fieldsets.is_some() {
						return Err(syn::Error::new(
							key.span(),
							"duplicate admin attribute `fieldsets`",
						));
					}
					fieldsets = Some(parse_fieldsets_array(input)?);
				}
				"readonly_fields" => {
					readonly_fields = Some(parse_ident_array(input)?);
				}
				"autocomplete_fields" => {
					autocomplete_fields = Some(parse_ident_array(input)?);
				}
				"raw_id_fields" => {
					raw_id_fields = Some(parse_ident_array(input)?);
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
							"unknown attribute `{}` for model admin\n\n  = help: valid attributes are: for, name, list_display, list_select_related, date_hierarchy, list_editable, list_filter, search_fields, filter_horizontal, filter_vertical, fields, fieldsets, readonly_fields, autocomplete_fields, raw_id_fields, ordering, list_per_page, allow_view, allow_add, allow_change, allow_delete, permissions",
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

		if let (Some(horizontal), Some(vertical)) = (&filter_horizontal, &filter_vertical)
			&& let Some(duplicate) = vertical
				.iter()
				.find(|field| horizontal.iter().any(|other| other == *field))
		{
			return Err(syn::Error::new(
				duplicate.span(),
				format!(
					"field `{duplicate}` cannot appear in both filter_horizontal and filter_vertical"
				),
			));
		}

		Ok(AdminModelConfig {
			model_type,
			name,
			list_display,
			list_select_related,
			date_hierarchy,
			list_editable,
			list_filter,
			search_fields,
			filter_horizontal,
			filter_vertical,
			fields,
			fieldsets,
			readonly_fields,
			autocomplete_fields,
			raw_id_fields,
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

/// Parse an array of fieldset specs.
fn parse_fieldsets_array(input: ParseStream) -> Result<Vec<FieldsetSpec>> {
	let content;
	bracketed!(content in input);

	let specs: Punctuated<FieldsetSpec, Token![,]> = content.call(Punctuated::parse_terminated)?;
	let specs: Vec<_> = specs.into_iter().collect();
	if specs.is_empty() {
		return Err(syn::Error::new(content.span(), "fieldsets cannot be empty"));
	}
	let mut fields = HashSet::new();
	for spec in &specs {
		for field in &spec.fields {
			if !fields.insert(field.to_string()) {
				return Err(syn::Error::new(
					field.span(),
					format!("field `{field}` is repeated across fieldsets"),
				));
			}
		}
	}
	Ok(specs)
}

/// Generate the ModelAdmin trait implementation
pub(crate) fn admin_impl(args: TokenStream, input: ItemStruct) -> Result<TokenStream> {
	let admin_api = crate::crate_paths::get_reinhardt_admin_adapters_crate();
	let async_trait = crate::crate_paths::get_async_trait_crate();
	let db_crate = crate::crate_paths::get_reinhardt_db_crate();
	let orm_crate = crate::crate_paths::get_reinhardt_orm_crate();
	let serde_json_crate = crate::crate_paths::get_serde_json_crate();

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
	if let Some(ref field) = config.date_hierarchy {
		all_fields.push(field);
	}
	if let Some(ref fields) = config.list_editable {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.list_filter {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.search_fields {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.filter_horizontal {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.filter_vertical {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.fields {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fieldsets) = config.fieldsets {
		all_fields.extend(fieldsets.iter().flat_map(|fieldset| fieldset.fields.iter()));
	}
	if let Some(ref fields) = config.readonly_fields {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.autocomplete_fields {
		all_fields.extend(fields.iter());
	}
	if let Some(ref fields) = config.raw_id_fields {
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
	let date_hierarchy_check = config.date_hierarchy.as_ref().map(|field| {
		let method_name = Ident::new(&format!("field_{field}"), field.span());
		quote! {
			fn __reinhardt_assert_date_hierarchy_field<T: #orm_crate::DateTimeType>(
				_: #orm_crate::expressions::FieldRef<
					#model_type,
					T,
					#orm_crate::expressions::GeneratedModelField,
				>,
			) {}
			__reinhardt_assert_date_hierarchy_field(#model_type::#method_name());
		}
	});
	let relation_checks: Vec<TokenStream> = config
		.list_select_related
		.as_deref()
		.unwrap_or_default()
		.iter()
		.map(|relation| {
			let method_name = Ident::new(&format!("field_{}", relation), relation.span());
			quote! {
				let _: fn() -> #orm_crate::expressions::FieldRef<
					#model_type,
					#db_crate::associations::ForeignKeyField<_>,
					#orm_crate::expressions::GeneratedModelField,
				> = #model_type::#method_name;
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

	// Generate list_select_related method
	let list_select_related_impl = if let Some(ref relations) = config.list_select_related {
		let relation_strs: Vec<String> = relations
			.iter()
			.map(|relation| relation.to_string())
			.collect();
		quote! {
			fn list_select_related(&self) -> Vec<&str> {
				vec![#(#relation_strs),*]
			}
		}
	} else {
		quote! {}
	};

	let date_hierarchy_impl = if let Some(ref field) = config.date_hierarchy {
		let field = field.to_string();
		quote! {
			fn date_hierarchy(&self) -> Option<&str> {
				Some(#field)
			}
		}
	} else {
		quote! {}
	};

	let object_label_impl = if config.list_display.is_some() {
		quote! {
			fn object_label(
				&self,
				record: &::std::collections::HashMap<::std::string::String, #serde_json_crate::Value>,
			) -> ::std::option::Option<::std::string::String> {
				fn scalar(
					value: &#serde_json_crate::Value,
				) -> ::std::option::Option<::std::string::String> {
					match value {
						#serde_json_crate::Value::String(value) => Some(value.clone()),
						#serde_json_crate::Value::Number(value) => Some(value.to_string()),
						#serde_json_crate::Value::Bool(value) => Some(value.to_string()),
						_ => None,
					}
				}

				self.list_display()
					.into_iter()
					.filter(|field| *field != self.pk_field())
					.find_map(|field| record.get(field).and_then(scalar))
					.or_else(|| record.get(self.pk_field()).and_then(scalar))
			}
		}
	} else {
		quote! {}
	};

	let list_editable_impl = if let Some(ref fields) = config.list_editable {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn list_editable(&self) -> Vec<&str> {
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

	// Generate filter_horizontal method
	let filter_horizontal_impl = if let Some(ref fields) = config.filter_horizontal {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn filter_horizontal(&self) -> Vec<&str> {
				vec![#(#field_strs),*]
			}
		}
	} else {
		quote! {}
	};

	// Generate filter_vertical method
	let filter_vertical_impl = if let Some(ref fields) = config.filter_vertical {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn filter_vertical(&self) -> Vec<&str> {
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

	// Generate fieldsets method
	let fieldsets_impl = if let Some(ref fieldsets) = config.fieldsets {
		let fieldsets = fieldsets.iter().map(|fieldset| {
			let collapsed = fieldset.collapsed;
			let title = if let Some(title) = &fieldset.title {
				quote!(Some(#title))
			} else {
				quote!(None)
			};
			let fields: Vec<String> = fieldset.fields.iter().map(Ident::to_string).collect();
			let tokens = quote!(#admin_api::Fieldset::new(#title, &[#(#fields),*]));
			if collapsed {
				quote!(#tokens.collapsed())
			} else {
				tokens
			}
		});
		quote! {
			fn fieldsets(&self) -> Option<Vec<#admin_api::Fieldset>> {
				Some(vec![#(#fieldsets),*])
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

	// Generate autocomplete_fields method
	let autocomplete_fields_impl = if let Some(ref fields) = config.autocomplete_fields {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn autocomplete_fields(&self) -> Vec<&str> {
				vec![#(#field_strs),*]
			}
		}
	} else {
		quote! {}
	};

	// Generate raw_id_fields method
	let raw_id_fields_impl = if let Some(ref fields) = config.raw_id_fields {
		let field_strs: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
		quote! {
			fn raw_id_fields(&self) -> Vec<&str> {
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
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		const _: () = {
			#(#field_checks)*
			#date_hierarchy_check
			#(#relation_checks)*
		};

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#[#async_trait::async_trait]
		impl #admin_api::ModelAdmin for #struct_name {
			fn model_name(&self) -> &str {
				#name
			}

			#table_name_impl
			#list_display_impl
			#list_select_related_impl
			#date_hierarchy_impl
			#object_label_impl
			#list_editable_impl
			#list_filter_impl
			#search_fields_impl
			#filter_horizontal_impl
			#filter_vertical_impl
			#fields_impl
			#fieldsets_impl
			#readonly_fields_impl
			#autocomplete_fields_impl
			#raw_id_fields_impl
			#ordering_impl
			#list_per_page_impl
			#permission_impls
		}
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use proc_macro2::Span;
	use quote::quote;
	use rstest::rstest;

	#[test]
	fn list_select_related_generates_foreign_key_validation_and_admin_method() {
		let args = quote! {
			model,
			for = Article,
			name = "Article",
			list_select_related = [author]
		};
		let input = syn::parse_quote! {
			pub struct ArticleAdmin;
		};

		let output = admin_impl(args, input)
			.expect("list_select_related should expand")
			.to_string()
			.replace(' ', "");

		assert_eq!(output.matches("Article::field_author").count(), 1);
		assert_eq!(output.matches("ForeignKeyField<_>").count(), 1);
		assert_eq!(
			output
				.matches("fnlist_select_related(&self)->Vec<&str>{vec![\"author\"]}")
				.count(),
			1
		);
	}

	#[test]
	fn date_hierarchy_generates_field_validation_and_admin_method() {
		let args = quote! {
			model,
			for = Article,
			name = "Article",
			date_hierarchy = created_at
		};
		let input = syn::parse_quote! {
			pub struct ArticleAdmin;
		};

		let output = admin_impl(args, input)
			.expect("date_hierarchy should expand")
			.to_string()
			.replace(' ', "");

		assert_eq!(output.matches("Article::field_created_at").count(), 2);
		assert_eq!(
			output
				.matches("fndate_hierarchy(&self)->Option<&str>{Some(\"created_at\")}")
				.count(),
			1
		);
	}

	#[rstest]
	fn parses_and_generates_many_to_many_selector_configuration() {
		let args = quote::quote! {
			model,
			for = Article,
			name = "Article",
			list_display = [id, name],
			filter_horizontal = [tags],
			filter_vertical = [reviewers]
		};
		let config: AdminModelConfig = syn::parse2(args.clone()).unwrap();

		assert_eq!(
			config.filter_horizontal.unwrap(),
			vec![Ident::new("tags", Span::call_site())]
		);
		assert_eq!(
			config.filter_vertical.unwrap(),
			vec![Ident::new("reviewers", Span::call_site())]
		);

		let generated = admin_impl(
			args,
			syn::parse_quote!(
				struct ArticleAdmin;
			),
		)
		.unwrap()
		.to_string();
		assert!(generated.contains("fn filter_horizontal"));
		assert!(generated.contains("fn filter_vertical"));
		assert!(generated.contains("fn object_label"));
		assert!(generated.contains("field_tags"));
		assert!(generated.contains("field_reviewers"));
	}

	#[rstest]
	fn rejects_overlapping_many_to_many_selector_configuration() {
		let result = syn::parse2::<AdminModelConfig>(quote::quote! {
			model,
			for = Article,
			name = "Article",
			filter_horizontal = [tags],
			filter_vertical = [tags]
		});

		assert!(result.is_err());
	}

	#[rstest]
	fn parses_list_editable_fields() {
		let config: AdminModelConfig = syn::parse2(quote! {
			model, for = User, name = "User", list_editable = [email, is_active]
		})
		.expect("list_editable should parse");

		assert_eq!(
			config
				.list_editable
				.expect("list_editable should be present")
				.iter()
				.map(|field| field.to_string())
				.collect::<Vec<_>>(),
			vec!["email", "is_active"]
		);
	}

	#[rstest]
	fn generates_list_editable_method_and_field_checks() {
		let generated = admin_impl(
			quote! { model, for = User, name = "User", list_editable = [email] },
			syn::parse2(quote! { struct UserAdmin; }).expect("admin input should parse"),
		)
		.expect("list_editable should generate");

		let generated = generated.to_string();
		assert!(generated.contains("fn list_editable"));
		assert!(generated.contains("email"));
		assert!(generated.contains("field_email"));
	}

	#[rstest]
	fn test_admin_config_parses_relation_fields() {
		// Arrange
		let input = "model, for = User, name = \"User\", autocomplete_fields = [owner], raw_id_fields = [team_id]";

		// Act
		let config: AdminModelConfig = syn::parse_str(input).unwrap();

		// Assert
		assert_eq!(
			config
				.autocomplete_fields
				.unwrap()
				.into_iter()
				.map(|field| field.to_string())
				.collect::<Vec<_>>(),
			vec!["owner"]
		);
		assert_eq!(
			config
				.raw_id_fields
				.unwrap()
				.into_iter()
				.map(|field| field.to_string())
				.collect::<Vec<_>>(),
			vec!["team_id"]
		);
	}

	#[rstest]
	fn test_admin_impl_generates_relation_field_getters() {
		// Arrange
		let args = quote! {
			model,
			for = User,
			name = "User",
			autocomplete_fields = [owner],
			raw_id_fields = [team_id]
		};
		let input: ItemStruct = syn::parse_quote! {
			pub struct UserAdmin;
		};

		// Act
		let generated: syn::File = syn::parse2(admin_impl(args, input).unwrap()).unwrap();
		let admin_impl = generated
			.items
			.iter()
			.find_map(|item| match item {
				syn::Item::Impl(item_impl) => Some(item_impl),
				_ => None,
			})
			.unwrap();
		let autocomplete_fields = admin_impl
			.items
			.iter()
			.find_map(|item| match item {
				syn::ImplItem::Fn(method) if method.sig.ident == "autocomplete_fields" => {
					Some(method)
				}
				_ => None,
			})
			.unwrap();
		let raw_id_fields = admin_impl
			.items
			.iter()
			.find_map(|item| match item {
				syn::ImplItem::Fn(method) if method.sig.ident == "raw_id_fields" => Some(method),
				_ => None,
			})
			.unwrap();
		let autocomplete_output = &autocomplete_fields.sig.output;
		let autocomplete_inputs = &autocomplete_fields.sig.inputs;
		let autocomplete_block = &autocomplete_fields.block;
		let raw_id_output = &raw_id_fields.sig.output;
		let raw_id_inputs = &raw_id_fields.sig.inputs;
		let raw_id_block = &raw_id_fields.block;

		// Assert
		assert_eq!(quote!(#autocomplete_inputs).to_string(), "& self");
		assert_eq!(quote!(#autocomplete_output).to_string(), "-> Vec < & str >");
		assert_eq!(
			quote!(#autocomplete_block).to_string(),
			"{ vec ! [\"owner\"] }"
		);
		assert_eq!(quote!(#raw_id_inputs).to_string(), "& self");
		assert_eq!(quote!(#raw_id_output).to_string(), "-> Vec < & str >");
		assert_eq!(quote!(#raw_id_block).to_string(), "{ vec ! [\"team_id\"] }");
	}
}
