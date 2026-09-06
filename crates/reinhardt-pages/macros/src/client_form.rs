//! `ClientForm` derive implementation.

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{
	Data, DeriveInput, Fields, Ident, LitStr, Path, Token, Type, Visibility, parse_macro_input,
	parse_quote,
};

use crate::crate_paths::get_reinhardt_pages_crate;

const CLIENT_FORM_ATTRIBUTE_MARKER: &str = "__reinhardt_client_form_attribute";

/// Derives a `use_form` compatible companion form for a DTO request type.
pub(crate) fn derive_client_form_impl(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	if input
		.attrs
		.iter()
		.any(|attr| attr.path().is_ident(CLIENT_FORM_ATTRIBUTE_MARKER))
	{
		return TokenStream::new();
	}
	match expand_client_form(input) {
		Ok(tokens) => tokens.into(),
		Err(error) => error.to_compile_error().into(),
	}
}

/// Expands the attribute form while reusing the derive implementation.
pub(crate) fn client_form_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let args: proc_macro2::TokenStream = args.into();
	match expand_client_form_attribute(input, args) {
		Ok(tokens) => tokens.into(),
		Err(error) => error.to_compile_error().into(),
	}
}

fn expand_client_form_attribute(
	input: DeriveInput,
	args: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
	let mut derive_input = input.clone();
	derive_input
		.attrs
		.retain(|attr| !attr.path().is_ident("client_form"));
	derive_input.attrs.push(parse_quote!(#[client_form(#args)]));
	let generated = expand_client_form(derive_input)?;

	let mut input = input;
	strip_client_form_attributes(&mut input);
	let pages_crate = get_reinhardt_pages_crate();
	let marker = format_ident!("{CLIENT_FORM_ATTRIBUTE_MARKER}");
	// Register serde helper attributes for every emitted DTO and suppress any
	// legacy ClientForm derive, including derives imported under an alias.
	input
		.attrs
		.insert(0, parse_quote!(#[derive(#pages_crate::ClientForm)]));
	input.attrs.insert(1, parse_quote!(#[#marker]));
	Ok(quote! {
		#input
		#generated
	})
}

fn strip_client_form_attributes(input: &mut DeriveInput) {
	input
		.attrs
		.retain(|attr| !attr.path().is_ident("client_form"));

	if let Data::Struct(data_struct) = &mut input.data {
		for field in data_struct.fields.iter_mut() {
			field
				.attrs
				.retain(|attr| !attr.path().is_ident("client_form"));
		}
	}
}

fn expand_client_form(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
	let dto_ident = input.ident;
	let dto_vis = input.vis;
	let attrs = input.attrs;
	let data = input.data;
	let options = ClientFormOptions::parse(&attrs)?;

	if !input.generics.params.is_empty() {
		return Err(syn::Error::new_spanned(
			input.generics,
			"ClientForm does not support generic DTO structs",
		));
	}

	let Data::Struct(data_struct) = data else {
		return Err(syn::Error::new_spanned(
			dto_ident,
			"ClientForm can only be derived for structs",
		));
	};
	let Fields::Named(fields) = data_struct.fields else {
		return Err(syn::Error::new_spanned(
			dto_ident,
			"ClientForm requires a struct with named fields",
		));
	};

	let stem = options
		.name
		.unwrap_or_else(|| format_ident!("{}ClientForm", dto_ident));
	let form_ident = stem;
	let values_ident = format_ident!("{}Values", form_ident);
	let field_ident = format_ident!("{}Field", form_ident);
	let pages_crate = get_reinhardt_pages_crate();

	let mut editable_fields = Vec::new();
	let mut skipped_fields = Vec::new();

	for field in fields.named {
		let Some(field_ident) = field.ident else {
			return Err(syn::Error::new_spanned(
				field,
				"ClientForm requires named fields",
			));
		};
		let field_options = ClientFormFieldOptions::parse(&field.attrs)?;
		if options.server_fn.is_some() && field_options.omits_or_ignores_server_submit() {
			return Err(syn::Error::new_spanned(
				&field_ident,
				"ClientForm server_fn fields must not use serde(skip), serde(skip_serializing), serde(skip_serializing_if), or serde(skip_deserializing)",
			));
		}
		if field_options.is_skipped() {
			ensure_skippable(&field.ty)?;
			skipped_fields.push(SkippedField {
				name: field_ident,
				vis: field.vis,
				ty: field.ty,
				default_expr: field_options.skipped_default_expr(&dto_ident),
			});
			continue;
		}
		if !matches!(dto_vis, Visibility::Inherited) && matches!(field.vis, Visibility::Inherited) {
			return Err(syn::Error::new_spanned(
				&field_ident,
				"ClientForm exported DTOs cannot expose private editable fields; mark the field public or skip it explicitly",
			));
		}

		let kind = FieldKind::classify(&field.ty)?;
		let serialized_name = field_options.serialized_name(&field_ident, options.serde_rename_all);
		editable_fields.push(EditableField::new(
			field_ident,
			field.vis,
			field.ty,
			kind,
			serialized_name,
		));
	}

	reject_runtime_field_name_collisions(&editable_fields)?;

	if editable_fields.is_empty() {
		return Err(syn::Error::new_spanned(
			dto_ident,
			"ClientForm requires at least one editable field",
		));
	}

	let form_items = generate_form_items(FormItemContext {
		dto_vis: &dto_vis,
		dto_ident: &dto_ident,
		form_ident: &form_ident,
		values_ident: &values_ident,
		field_ident: &field_ident,
		fields: &editable_fields,
		skipped_fields: &skipped_fields,
		pages_crate: &pages_crate,
		validate: options.validate,
	});
	let submit_method = options
		.server_fn
		.as_ref()
		.map(|server_fn| generate_submit_method(&dto_ident, &form_ident, server_fn, &pages_crate))
		.unwrap_or_default();

	Ok(quote! {
		#form_items

		impl #form_ident {
			#submit_method
		}
	})
}

fn reject_runtime_field_name_collisions(fields: &[EditableField]) -> syn::Result<()> {
	let mut accepted_names: BTreeMap<String, &EditableField> = BTreeMap::new();

	for field in fields {
		let name = ident_name_without_raw_prefix(&field.name);
		let raw_name = field.name.to_string();
		for accepted_name in [field.serialized_name.clone(), name, raw_name] {
			if let Some(previous) = accepted_names.get(&accepted_name)
				&& previous.name != field.name
			{
				return Err(syn::Error::new_spanned(
					&field.name,
					format!(
						"ClientForm field name `{accepted_name}` collides with another field's runtime alias"
					),
				));
			}
			accepted_names.insert(accepted_name, field);
		}
	}

	Ok(())
}

struct FormItemContext<'a> {
	dto_vis: &'a Visibility,
	dto_ident: &'a Ident,
	form_ident: &'a Ident,
	values_ident: &'a Ident,
	field_ident: &'a Ident,
	fields: &'a [EditableField],
	skipped_fields: &'a [SkippedField],
	pages_crate: &'a proc_macro2::TokenStream,
	validate: bool,
}

fn generate_form_items(context: FormItemContext<'_>) -> proc_macro2::TokenStream {
	let FormItemContext {
		dto_vis,
		dto_ident,
		form_ident,
		values_ident,
		field_ident,
		fields,
		skipped_fields,
		pages_crate,
		validate,
	} = context;
	let field_token_fields = fields
		.iter()
		.filter(|field| field.exposes_field_token(dto_vis))
		.collect::<Vec<_>>();

	let value_field_defs = fields.iter().map(|field| {
		let name = &field.name;
		let vis = &field.vis;
		let value_ty = field.value_ty();
		quote! { #vis #name: #value_ty }
	});
	let skipped_value_field_defs = skipped_fields.iter().map(|field| {
		let name = &field.name;
		let vis = &field.vis;
		let ty = &field.ty;
		quote! { #vis #name: #ty }
	});
	let form_field_defs = fields.iter().map(|field| {
		let name = &field.name;
		let value_ty = field.value_ty();
		quote! { #name: #pages_crate::reactive::Signal<#value_ty> }
	});
	let field_variants = field_token_fields.iter().map(|field| &field.variant);
	let field_accessor_methods = field_token_fields.iter().map(|field| {
		let method = format_ident!("{}_field", ident_name_without_raw_prefix(&field.name));
		let variant = &field.variant;
		quote! {
			pub fn #method(&self) -> #field_ident {
				#field_ident::#variant
			}
		}
	});
	let choice_methods = field_token_fields.iter().filter_map(|field| {
		let choice_ty = field.choice_ty()?;
		let method = format_ident!("{}_choices", ident_name_without_raw_prefix(&field.name));
		Some(quote! {
			pub fn #method(&self) -> &'static [#pages_crate::ClientFormChoice<#choice_ty>] {
				<#choice_ty as #pages_crate::ClientFormChoiceSource>::client_form_choices()
			}
		})
	});

	let default_value_fields = fields.iter().map(|field| {
		let name = &field.name;
		let default_expr = field.default_expr(pages_crate);
		quote! { #name: #default_expr }
	});
	let skipped_default_value_fields = skipped_fields.iter().map(|field| {
		let name = &field.name;
		let default_expr = &field.default_expr;
		quote! { #name: #default_expr }
	});
	let signal_initializers = fields.iter().map(|field| {
		let name = &field.name;
		quote! { #name: #pages_crate::reactive::Signal::new(__initial_values.#name.clone()) }
	});
	let current_value_fields = fields.iter().map(|field| {
		let name = &field.name;
		quote! { #name: self.#name.get() }
	});
	let current_skipped_value_fields = skipped_fields.iter().map(|field| {
		let name = &field.name;
		quote! { #name: __initial_values.#name.clone() }
	});
	let apply_values = fields.iter().map(|field| {
		let name = &field.name;
		quote! { self.#name.set(values.#name.clone()); }
	});
	let apply_skipped_values = skipped_fields.iter().map(|field| {
		let name = &field.name;
		quote! { __initial_values.#name = values.#name.clone(); }
	});
	let apply_pristine_skipped_values = skipped_fields.iter().map(|field| {
		let name = &field.name;
		quote! { __initial_values.#name = new_defaults.#name.clone(); }
	});
	let apply_pristine_private_values = fields
		.iter()
		.filter(|field| !field.exposes_field_token(dto_vis))
		.map(|field| {
			let name = &field.name;
			quote! { self.#name.set(new_defaults.#name.clone()); }
		});
	let value_eq_fields = fields.iter().map(|field| {
		let name = &field.name;
		quote! { self.#name == other.#name }
	});
	let set_field_arms = field_token_fields.iter().map(|field| {
		let name = &field.name;
		let variant = &field.variant;
		let value_ty = field.value_ty();
		quote! {
			#field_ident::#variant => {
				let value = ::std::boxed::Box::new(value) as ::std::boxed::Box<dyn ::core::any::Any>;
				match value.downcast::<#value_ty>() {
					::core::result::Result::Ok(value) => self.#name.set(*value),
					::core::result::Result::Err(_) => {
						panic!(
							"field {:?} is not compatible with provided value type {}",
							field,
							::core::any::type_name::<T>()
						);
					}
				}
			}
		}
	});
	let apply_field_arms = field_token_fields.iter().map(|field| {
		let name = &field.name;
		let variant = &field.variant;
		quote! { #field_ident::#variant => self.#name.set(values.#name.clone()) }
	});
	let dirty_arms = field_token_fields.iter().map(|field| {
		let name = &field.name;
		let variant = &field.variant;
		quote! { #field_ident::#variant => current.#name != defaults.#name }
	});
	let watch_arms = field_token_fields.iter().map(|field| {
		let name = &field.name;
		let variant = &field.variant;
		quote! {
			#field_ident::#variant => {
				let signal = self.#name.clone();
				let signal = ::std::boxed::Box::new(signal) as ::std::boxed::Box<dyn ::core::any::Any>;
				signal
					.downcast::<#pages_crate::reactive::Signal<T>>()
					.ok()
					.map(|signal| *signal)
			}
		}
	});
	let fields_slice = field_token_fields.iter().map(|field| {
		let variant = &field.variant;
		quote! { #field_ident::#variant }
	});
	let defaults_from_request = fields.iter().map(|field| {
		let name = &field.name;
		let expr = field.default_from_request_expr();
		quote! { #name: #expr }
	});
	let skipped_defaults_from_request = skipped_fields.iter().map(|field| {
		let name = &field.name;
		quote! { #name: defaults.#name }
	});
	let request_fields = fields.iter().map(|field| {
		let name = &field.name;
		let expr = field.request_expr();
		quote! { #name: #expr }
	});
	let skipped_request_fields = skipped_fields.iter().map(|field| {
		let name = &field.name;
		quote! { #name: values.#name.clone() }
	});
	let field_name_arms = field_token_fields.iter().map(|field| {
		let raw_name = field.name.to_string();
		let name = ident_name_without_raw_prefix(&field.name);
		let serialized_name = &field.serialized_name;
		let variant = &field.variant;
		let mut names = vec![serialized_name.clone(), name.clone(), raw_name];
		names.sort();
		names.dedup();
		quote! { #(#names)|* => ::core::option::Option::Some(#field_ident::#variant) }
	});
	let runtime_validate_method = if validate {
		quote! {
			fn runtime_validate(&self) -> ::core::result::Result<(), #pages_crate::FormValidationError<Self::Field>> {
				let request = #form_ident::values_to_request(&self.runtime_current_values());
				#pages_crate::__private::client_form::validate_dto_request(
					&request,
					#form_ident::field_from_name,
				)
			}
		}
	} else {
		quote! {}
	};

	quote! {
		#[derive(Clone)]
		#dto_vis struct #form_ident {
			__initial_values: ::std::rc::Rc<::std::cell::RefCell<#values_ident>>,
			#(#form_field_defs,)*
		}

		#[derive(Clone)]
		#dto_vis struct #values_ident {
			#(#value_field_defs,)*
			#(#skipped_value_field_defs,)*
		}

		impl ::core::cmp::PartialEq for #values_ident {
			fn eq(&self, other: &Self) -> bool {
				#(#value_eq_fields)&&*
			}
		}

		#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
		#dto_vis enum #field_ident {
			#(#field_variants,)*
		}

		impl #form_ident {
			pub fn new() -> Self {
				let __initial_values = #values_ident {
					#(#default_value_fields,)*
					#(#skipped_default_value_fields,)*
				};
				Self {
					__initial_values: ::std::rc::Rc::new(::std::cell::RefCell::new(__initial_values.clone())),
					#(#signal_initializers,)*
				}
			}

			pub fn with_defaults(self, defaults: #dto_ident) -> Self {
				let values = #values_ident {
					#(#defaults_from_request,)*
					#(#skipped_defaults_from_request,)*
				};
				<#form_ident as #pages_crate::FormRuntimeSource>::runtime_apply_values(&self, &values);
				*self.__initial_values.borrow_mut() = values;
				self
			}

			pub fn to_request<Deps>(runtime: &#pages_crate::UseFormReturn<Self, Deps>) -> #dto_ident
			where
				Deps: ::core::clone::Clone + ::core::cmp::PartialEq + 'static,
			{
				Self::values_to_request(&runtime.get_values())
			}

			fn values_to_request(values: &#values_ident) -> #dto_ident {
				#dto_ident {
					#(#request_fields,)*
					#(#skipped_request_fields,)*
				}
			}

			fn field_from_name(name: &str) -> ::core::option::Option<#field_ident> {
				match name {
					#(#field_name_arms,)*
					_ => ::core::option::Option::None,
				}
			}

			#(#field_accessor_methods)*
			#(#choice_methods)*
		}

		impl #pages_crate::FormRuntimeSource for #form_ident {
			type Values = #values_ident;
			type Field = #field_ident;

			fn runtime_field_by_name(&self, name: &str) -> ::core::option::Option<Self::Field> {
				Self::field_from_name(name)
			}

			fn runtime_initial_values(&self) -> Self::Values {
				self.__initial_values.borrow().clone()
			}

			fn runtime_current_values(&self) -> Self::Values {
				let __initial_values = self.__initial_values.borrow();
				#values_ident {
					#(#current_value_fields,)*
					#(#current_skipped_value_fields,)*
				}
			}

			fn runtime_apply_values(&self, values: &Self::Values) {
				{
					let mut __initial_values = self.__initial_values.borrow_mut();
					#(#apply_skipped_values)*
				}
				#(#apply_values)*
			}

			fn runtime_apply_pristine_values(
				&self,
				current: &Self::Values,
				old_defaults: &Self::Values,
				new_defaults: &Self::Values,
			) {
				{
					let mut __initial_values = self.__initial_values.borrow_mut();
					#(#apply_pristine_skipped_values)*
				}
				#(#apply_pristine_private_values)*
				for field in self.runtime_fields() {
					let field = *field;
					if !self.runtime_field_is_dirty(field, current, old_defaults) {
						self.runtime_apply_field_value(field, new_defaults);
					}
				}
			}

			fn runtime_set_field_value<T>(&self, field: Self::Field, value: T)
			where
				T: ::core::any::Any + 'static,
			{
				match field {
					#(#set_field_arms,)*
				}
			}

			fn runtime_apply_field_value(&self, field: Self::Field, values: &Self::Values) {
				match field {
					#(#apply_field_arms,)*
				}
			}

			fn runtime_field_is_dirty(
				&self,
				field: Self::Field,
				current: &Self::Values,
				defaults: &Self::Values,
			) -> bool {
				match field {
					#(#dirty_arms,)*
				}
			}

			fn runtime_watch_field<T>(
				&self,
				field: Self::Field,
			) -> ::core::option::Option<#pages_crate::reactive::Signal<T>>
			where
				T: ::core::clone::Clone + 'static,
			{
				match field {
					#(#watch_arms,)*
				}
			}

			#runtime_validate_method

			fn runtime_fields(&self) -> &'static [Self::Field] {
				&[#(#fields_slice),*]
			}
		}
	}
}

fn generate_submit_method(
	dto_ident: &Ident,
	form_ident: &Ident,
	server_fn: &Path,
	pages_crate: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	quote! {
		// Compile the response metadata requirement on native targets too so
		// scoped server_fn bindings do not fail only in the wasm-only submit
		// helper.
		#[allow(dead_code)]
		fn __assert_server_fn_response_metadata()
		where
			#dto_ident: ::serde::Serialize,
			#server_fn::marker: #pages_crate::server_fn::ServerFnResponseMetadata
				+ #pages_crate::server_fn::ServerFnRequestMetadata<Request = #dto_ident>,
			<#server_fn::marker as #pages_crate::server_fn::ServerFnResponseMetadata>::Response:
				::serde::de::DeserializeOwned,
			<#server_fn::marker as #pages_crate::server_fn::ServerFnResponseMetadata>::Error:
				::core::convert::Into<#pages_crate::server_fn::ServerFnError>,
		{
		}

		pub fn server_mutation<Deps>(
			&self,
			runtime: &#pages_crate::UseFormReturn<Self, Deps>,
		) -> #pages_crate::FormServerMutationBuilder<
			Self,
			Deps,
			#dto_ident,
			<#server_fn::marker as #pages_crate::server_fn::ServerFnResponseMetadata>::Response,
		>
		where
			Deps: ::core::clone::Clone + ::core::cmp::PartialEq + 'static,
			<#server_fn::marker as #pages_crate::server_fn::ServerFnResponseMetadata>::Error:
				::core::convert::Into<#pages_crate::ServerFnError>,
		{
			let _ = self;
			#pages_crate::use_server_mutation(#server_fn::mutation())
				.with_generated_form(runtime, |runtime| {
					::core::result::Result::Ok(#form_ident::to_request(runtime))
				})
		}

		pub async fn submit<Deps>(
			&self,
			runtime: &#pages_crate::UseFormReturn<Self, Deps>,
		) -> ::core::result::Result<
			#pages_crate::UseFormAsyncSubmitOutcome<
				<#server_fn::marker as #pages_crate::server_fn::ServerFnResponseMetadata>::Response,
			>,
			#pages_crate::server_fn::ServerFnError,
		>
			where
				Deps: ::core::clone::Clone + ::core::cmp::PartialEq + 'static,
				<#server_fn::marker as #pages_crate::server_fn::ServerFnResponseMetadata>::Error:
					::core::convert::Into<#pages_crate::server_fn::ServerFnError>,
			{
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			{
				let _ = self;
				runtime
					.submit_server_fn(|| {
						let request = #form_ident::to_request(runtime);
						async move {
							#pages_crate::server_mutation::execute_server_mutation_once(
								request,
								#server_fn::mutation(),
							)
							.await
						}
					})
					.await
			}
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			{
				let _ = (self, runtime);
				::core::unreachable!()
			}
		}
	}
}

struct ClientFormOptions {
	name: Option<Ident>,
	server_fn: Option<Path>,
	validate: bool,
	serde_rename_all: Option<SerdeRenameRule>,
}

impl ClientFormOptions {
	fn parse(attrs: &[syn::Attribute]) -> syn::Result<Self> {
		let mut options = Self {
			name: None,
			server_fn: None,
			validate: false,
			serde_rename_all: None,
		};
		for attr in attrs {
			if !attr.path().is_ident("client_form") {
				if attr.path().is_ident("serde") {
					attr.parse_nested_meta(|meta| {
						if meta.path.is_ident("rename_all") {
							options.serde_rename_all = parse_serde_rename_all(meta)?;
						} else {
							consume_serde_meta(meta)?;
						}
						Ok(())
					})?;
				}
				continue;
			}
			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("name") {
					let value = meta.value()?.parse::<Ident>()?;
					options.name = Some(value);
				} else if meta.path.is_ident("server_fn") {
					let value = meta.value()?.parse::<Path>()?;
					options.server_fn = Some(value);
				} else if meta.path.is_ident("validate") {
					options.validate = true;
				} else {
					return Err(meta.error("unsupported client_form attribute"));
				}
				Ok(())
			})?;
		}
		Ok(options)
	}
}

#[derive(Clone, Copy)]
enum SerdeRenameRule {
	Lowercase,
	Uppercase,
	PascalCase,
	CamelCase,
	SnakeCase,
	ScreamingSnakeCase,
	KebabCase,
	ScreamingKebabCase,
}

impl SerdeRenameRule {
	fn parse(value: &str) -> syn::Result<Self> {
		match value {
			"lowercase" => Ok(Self::Lowercase),
			"UPPERCASE" => Ok(Self::Uppercase),
			"PascalCase" => Ok(Self::PascalCase),
			"camelCase" => Ok(Self::CamelCase),
			"snake_case" => Ok(Self::SnakeCase),
			"SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnakeCase),
			"kebab-case" => Ok(Self::KebabCase),
			"SCREAMING-KEBAB-CASE" => Ok(Self::ScreamingKebabCase),
			_ => Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"unsupported serde rename_all rule for ClientForm",
			)),
		}
	}

	fn apply(self, field: &str) -> String {
		match self {
			Self::Lowercase | Self::SnakeCase => field.to_string(),
			Self::Uppercase | Self::ScreamingSnakeCase => field.to_ascii_uppercase(),
			Self::PascalCase => {
				let mut value = String::new();
				let mut capitalize = true;
				for character in field.chars() {
					if character == '_' {
						capitalize = true;
					} else if capitalize {
						value.push(character.to_ascii_uppercase());
						capitalize = false;
					} else {
						value.push(character);
					}
				}
				value
			}
			Self::CamelCase => {
				let pascal = Self::PascalCase.apply(field);
				pascal[..1].to_ascii_lowercase() + &pascal[1..]
			}
			Self::KebabCase => field.replace('_', "-"),
			Self::ScreamingKebabCase => field.to_ascii_uppercase().replace('_', "-"),
		}
	}
}

fn parse_serde_rename_all(
	meta: syn::meta::ParseNestedMeta<'_>,
) -> syn::Result<Option<SerdeRenameRule>> {
	parse_serde_serialize_name(meta)?
		.map(|name| SerdeRenameRule::parse(&name))
		.transpose()
}

fn parse_serde_serialize_name(meta: syn::meta::ParseNestedMeta<'_>) -> syn::Result<Option<String>> {
	if meta.input.peek(Token![=]) {
		return Ok(Some(meta.value()?.parse::<LitStr>()?.value()));
	}
	if !meta.input.peek(syn::token::Paren) {
		return Err(meta.error("expected a serde rename value"));
	}

	let mut serialize_name = None;
	meta.parse_nested_meta(|nested| {
		if nested.path.is_ident("serialize") {
			serialize_name = Some(nested.value()?.parse::<LitStr>()?.value());
		} else {
			consume_serde_meta(nested)?;
		}
		Ok(())
	})?;
	Ok(serialize_name)
}

struct ClientFormFieldOptions {
	skip: bool,
	serde_skip: bool,
	serde_skip_serializing: bool,
	serde_skip_serializing_if: bool,
	serde_skip_deserializing: bool,
	serde_default: Option<SerdeDefaultExpr>,
	serde_rename: Option<String>,
}

impl ClientFormFieldOptions {
	fn parse(attrs: &[syn::Attribute]) -> syn::Result<Self> {
		let mut options = Self {
			skip: false,
			serde_skip: false,
			serde_skip_serializing: false,
			serde_skip_serializing_if: false,
			serde_skip_deserializing: false,
			serde_default: None,
			serde_rename: None,
		};
		for attr in attrs {
			if attr.path().is_ident("client_form") {
				attr.parse_nested_meta(|meta| {
					if meta.path.is_ident("skip") {
						options.skip = true;
					} else {
						return Err(meta.error("unsupported client_form field attribute"));
					}
					Ok(())
				})?;
			} else if attr.path().is_ident("serde") {
				attr.parse_nested_meta(|meta| {
					if meta.path.is_ident("skip") {
						options.serde_skip = true;
						options.serde_skip_serializing = true;
						options.serde_skip_deserializing = true;
					} else if meta.path.is_ident("skip_serializing") {
						options.serde_skip_serializing = true;
					} else if meta.path.is_ident("skip_serializing_if") {
						options.serde_skip_serializing_if = true;
						consume_serde_meta(meta)?;
					} else if meta.path.is_ident("skip_deserializing") {
						options.serde_skip_deserializing = true;
					} else if meta.path.is_ident("default") {
						options.serde_default = Some(parse_serde_default_expr(meta)?);
					} else if meta.path.is_ident("rename") {
						options.serde_rename = parse_serde_serialize_name(meta)?;
					} else {
						consume_serde_meta(meta)?;
					}
					Ok(())
				})?;
			}
		}
		Ok(options)
	}

	fn is_skipped(&self) -> bool {
		self.skip || self.serde_skip || self.serde_skip_serializing || self.serde_skip_deserializing
	}

	fn omits_or_ignores_server_submit(&self) -> bool {
		self.serde_skip_serializing
			|| self.serde_skip_serializing_if
			|| self.serde_skip_deserializing
	}

	fn skipped_default_expr(&self, dto_ident: &Ident) -> proc_macro2::TokenStream {
		self.serde_default
			.as_ref()
			.unwrap_or(&SerdeDefaultExpr::Default)
			.to_tokens(dto_ident)
	}

	fn serialized_name(&self, field: &Ident, rename_all: Option<SerdeRenameRule>) -> String {
		let name = ident_name_without_raw_prefix(field);
		self.serde_rename
			.clone()
			.unwrap_or_else(|| rename_all.map_or(name.clone(), |rule| rule.apply(&name)))
	}
}

enum SerdeDefaultExpr {
	Default,
	Path(Path),
}

impl SerdeDefaultExpr {
	fn to_tokens(&self, dto_ident: &Ident) -> proc_macro2::TokenStream {
		match self {
			Self::Default => quote! { ::core::default::Default::default() },
			Self::Path(path) => {
				let mut path = path.clone();
				if let Some(first_segment) = path.segments.first_mut()
					&& first_segment.ident == "Self"
				{
					first_segment.ident = dto_ident.clone();
				}
				quote! { #path() }
			}
		}
	}
}

fn parse_serde_default_expr(meta: syn::meta::ParseNestedMeta<'_>) -> syn::Result<SerdeDefaultExpr> {
	if meta.input.peek(Token![=]) {
		let value = meta.value()?;
		let path = value.parse::<LitStr>()?.parse::<Path>()?;
		Ok(SerdeDefaultExpr::Path(path))
	} else {
		Ok(SerdeDefaultExpr::Default)
	}
}

fn consume_serde_meta(meta: syn::meta::ParseNestedMeta<'_>) -> syn::Result<()> {
	if meta.input.peek(Token![=]) {
		let _value = meta.value()?.parse::<syn::Expr>()?;
	} else if meta.input.peek(syn::token::Paren) {
		meta.parse_nested_meta(consume_serde_meta)?;
	}
	Ok(())
}

struct EditableField {
	name: Ident,
	serialized_name: String,
	variant: Ident,
	vis: Visibility,
	ty: Type,
	kind: FieldKind,
}

impl EditableField {
	fn new(
		name: Ident,
		vis: Visibility,
		ty: Type,
		kind: FieldKind,
		serialized_name: String,
	) -> Self {
		let variant = format_ident!(
			"{}",
			ident_name_without_raw_prefix(&name).to_case(Case::Pascal)
		);
		Self {
			name,
			serialized_name,
			variant,
			vis,
			ty,
			kind,
		}
	}

	fn value_ty(&self) -> proc_macro2::TokenStream {
		match &self.kind {
			FieldKind::String
			| FieldKind::OptionScalar
			| FieldKind::OptionBool
			| FieldKind::Scalar
			| FieldKind::Bool
			| FieldKind::Enum
			| FieldKind::OptionEnum => {
				let ty = &self.ty;
				quote! { #ty }
			}
			FieldKind::OptionString => quote! { ::std::string::String },
		}
	}

	fn choice_ty(&self) -> Option<proc_macro2::TokenStream> {
		match &self.kind {
			FieldKind::Enum => {
				let ty = &self.ty;
				Some(quote! { #ty })
			}
			FieldKind::OptionEnum => option_inner_type(&self.ty).map(|ty| quote! { #ty }),
			_ => None,
		}
	}

	fn exposes_field_token(&self, dto_vis: &Visibility) -> bool {
		matches!(dto_vis, Visibility::Inherited) || !matches!(self.vis, Visibility::Inherited)
	}

	fn default_expr(&self, pages_crate: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
		match &self.kind {
			FieldKind::String | FieldKind::OptionString => quote! { ::std::string::String::new() },
			FieldKind::Bool => quote! { false },
			FieldKind::Scalar => quote! { ::core::default::Default::default() },
			FieldKind::OptionScalar | FieldKind::OptionBool => {
				quote! { ::core::option::Option::None }
			}
			FieldKind::Enum => {
				let ty = &self.ty;
				quote! { <#ty as #pages_crate::ClientFormChoiceSource>::client_form_default() }
			}
			FieldKind::OptionEnum => quote! { ::core::option::Option::None },
		}
	}

	fn default_from_request_expr(&self) -> proc_macro2::TokenStream {
		let name = &self.name;
		match &self.kind {
			FieldKind::OptionString => quote! { defaults.#name.unwrap_or_default() },
			_ => quote! { defaults.#name },
		}
	}

	fn request_expr(&self) -> proc_macro2::TokenStream {
		let name = &self.name;
		match &self.kind {
			FieldKind::OptionString => quote! {
				{
					let value = values.#name.trim();
					if value.is_empty() {
						::core::option::Option::None
					} else {
						::core::option::Option::Some(value.to_string())
					}
				}
			},
			_ => quote! { values.#name.clone() },
		}
	}
}

fn ident_name_without_raw_prefix(ident: &Ident) -> String {
	let name = ident.to_string();
	name.strip_prefix("r#").unwrap_or(&name).to_string()
}

struct SkippedField {
	name: Ident,
	vis: Visibility,
	ty: Type,
	default_expr: proc_macro2::TokenStream,
}

enum FieldKind {
	String,
	OptionString,
	Scalar,
	OptionScalar,
	Bool,
	OptionBool,
	Enum,
	OptionEnum,
}

impl FieldKind {
	fn classify(ty: &Type) -> syn::Result<Self> {
		if is_string_type(ty) {
			return Ok(Self::String);
		}
		if is_bool_type(ty) {
			return Ok(Self::Bool);
		}
		if is_numeric_primitive(ty) {
			return Ok(Self::Scalar);
		}
		if let Some(inner) = option_inner_type(ty) {
			if is_string_type(inner) {
				return Ok(Self::OptionString);
			}
			if is_bool_type(inner) {
				return Ok(Self::OptionBool);
			}
			if is_numeric_primitive(inner) {
				return Ok(Self::OptionScalar);
			}
			if is_unsupported_container(inner) || has_type_arguments(inner) {
				return Err(syn::Error::new_spanned(
					ty,
					"ClientForm does not support optional collection, map, or generic fields",
				));
			}
			return Ok(Self::OptionEnum);
		}
		if is_unsupported_container(ty) || has_type_arguments(ty) {
			return Err(syn::Error::new_spanned(
				ty,
				"ClientForm does not support collection, map, or generic fields",
			));
		}
		if matches!(ty, Type::Path(_)) {
			return Ok(Self::Enum);
		}
		Err(syn::Error::new_spanned(
			ty,
			"ClientForm supports String, Option<String>, primitive scalars, bool, and ClientFormChoices enums",
		))
	}
}

fn ensure_skippable(ty: &Type) -> syn::Result<()> {
	if option_inner_type(ty).is_some() || matches!(ty, Type::Path(_)) {
		Ok(())
	} else {
		Err(syn::Error::new_spanned(
			ty,
			"client_form(skip) requires an Option<T> or Default field type",
		))
	}
}

fn is_string_type(ty: &Type) -> bool {
	type_last_ident(ty).is_some_and(|ident| ident == "String")
}

fn is_bool_type(ty: &Type) -> bool {
	type_last_ident(ty).is_some_and(|ident| ident == "bool")
}

fn is_numeric_primitive(ty: &Type) -> bool {
	type_last_ident(ty).is_some_and(|ident| {
		matches!(
			ident.as_str(),
			"u8" | "u16"
				| "u32" | "u64"
				| "u128" | "usize"
				| "i8" | "i16"
				| "i32" | "i64"
				| "i128" | "isize"
				| "f32" | "f64"
		)
	})
}

fn is_unsupported_container(ty: &Type) -> bool {
	type_last_ident(ty).is_some_and(|ident| {
		matches!(
			ident.as_str(),
			"Vec" | "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "VecDeque" | "LinkedList"
		)
	})
}

fn has_type_arguments(ty: &Type) -> bool {
	let Type::Path(type_path) = ty else {
		return false;
	};
	type_path.path.segments.iter().any(|segment| {
		matches!(
			segment.arguments,
			syn::PathArguments::AngleBracketed(_) | syn::PathArguments::Parenthesized(_)
		)
	})
}

fn option_inner_type(ty: &Type) -> Option<&Type> {
	let Type::Path(type_path) = ty else {
		return None;
	};
	let segment = type_path.path.segments.last()?;
	if segment.ident != "Option" {
		return None;
	}
	let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
		return None;
	};
	if args.args.len() != 1 {
		return None;
	}
	let syn::GenericArgument::Type(inner) = args.args.first()? else {
		return None;
	};
	Some(inner)
}

fn type_last_ident(ty: &Type) -> Option<String> {
	let Type::Path(type_path) = ty else {
		return None;
	};
	type_path
		.path
		.segments
		.last()
		.map(|segment| segment.ident.to_string())
}
