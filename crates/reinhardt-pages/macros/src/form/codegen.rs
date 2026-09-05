//! Code generation for the form! macro.
//!
//! This module generates Rust code from the typed form AST. The generated code
//! supports both SSR (Server-Side Rendering) and CSR (Client-Side Rendering/WASM)
//! through conditional compilation.
//!
//! ## Generated Code Structure
//!
//! For SSR (native):
//! - Generates a Form struct with metadata
//! - Implements `into_page()` for View conversion
//! - Field accessors return dummy Signal wrappers for type compatibility
//!
//! For CSR (WASM):
//! - Generates a FormComponent with reactive Signal bindings
//! - Implements real `into_page()` with event handlers
//! - Field accessors return actual Signal references
//!
//! ## Example
//!
//! ```text
//! form! {
//!     name: LoginForm,
//!     action: "/api/login",
//!     fields: {
//!         username: CharField { required },
//!         password: CharField { widget: PasswordInput },
//!     },
//! }
//! ```
//!
//! Generates (simplified):
//!
//! ```text
//! {
//!     struct LoginForm { ... }
//!     impl LoginForm {
//!         fn username(&self) -> &Signal<String> { ... }
//!         fn password(&self) -> &Signal<String> { ... }
//!         fn into_page(self) -> View { ... }
//!     }
//!     LoginForm::new()
//! }
//! ```

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::ext::IdentExt;

use crate::crate_paths::get_reinhardt_pages_crate_info;
use reinhardt_manouche::core::{
	AmbientArgumentsSource, FormMethod, TypedButtonControlDef, TypedButtonKind, TypedChoiceItem,
	TypedChoiceOption, TypedCustomAttr, TypedDatalistDef, TypedFieldNativeAttrs, TypedFieldType,
	TypedFormAction, TypedFormCallbacks, TypedFormDerived, TypedFormFieldCollection,
	TypedFormFieldDef, TypedFormFieldEntry, TypedFormFieldGroup, TypedFormMacro, TypedFormSlots,
	TypedFormState, TypedFormWatch, TypedIcon, TypedIconChild, TypedIconPosition,
	TypedImageInputDef, TypedMeterDef, TypedModelFieldSelection, TypedModelFormSource,
	TypedOutputDef, TypedProgressDef, TypedSubmitButtonDef, TypedValidatorRule, TypedWidget,
	TypedWrapper,
};

/// Collects scalar fields from field entries, flattening groups.
///
/// This is useful for generating struct declarations and accessors,
/// where all fields need to be at the same level regardless of grouping.
fn collect_scalar_fields(entries: &[TypedFormFieldEntry]) -> Vec<&TypedFormFieldDef> {
	let mut fields = Vec::new();
	for entry in entries {
		match entry {
			TypedFormFieldEntry::Field(field) => fields.push(field.as_ref()),
			TypedFormFieldEntry::Group(group) => {
				fields.extend(collect_scalar_fields(&group.fields));
			}
			TypedFormFieldEntry::Collection(_) => {}
			TypedFormFieldEntry::SubmitButton(_)
			| TypedFormFieldEntry::ResetButton(_)
			| TypedFormFieldEntry::Button(_)
			| TypedFormFieldEntry::ImageInput(_)
			| TypedFormFieldEntry::Output(_)
			| TypedFormFieldEntry::Meter(_)
			| TypedFormFieldEntry::Progress(_)
			| TypedFormFieldEntry::Datalist(_) => {}
		}
	}
	fields
}

/// Collects all datalist entries from field entries, flattening groups.
fn collect_all_datalists(entries: &[TypedFormFieldEntry]) -> Vec<&TypedDatalistDef> {
	let mut datalists = Vec::new();
	for entry in entries {
		match entry {
			TypedFormFieldEntry::Group(group) => {
				datalists.extend(collect_all_datalists(&group.fields));
			}
			TypedFormFieldEntry::Collection(collection) => {
				datalists.extend(collect_all_datalists(&collection.fields));
			}
			TypedFormFieldEntry::Datalist(datalist) => datalists.push(datalist.as_ref()),
			TypedFormFieldEntry::Field(_)
			| TypedFormFieldEntry::SubmitButton(_)
			| TypedFormFieldEntry::ResetButton(_)
			| TypedFormFieldEntry::Button(_)
			| TypedFormFieldEntry::ImageInput(_)
			| TypedFormFieldEntry::Output(_)
			| TypedFormFieldEntry::Meter(_)
			| TypedFormFieldEntry::Progress(_) => {}
		}
	}
	datalists
}

fn custom_widget_touched_ident(field: &TypedFormFieldDef) -> Option<syn::Ident> {
	if matches!(field.widget, TypedWidget::CustomExperimental(_)) {
		Some(format_ident!(
			"__{}_custom_widget_touched",
			field.name,
			span = field.name.span()
		))
	} else {
		None
	}
}

/// Collects top-level field collections from field entries.
fn collect_collections(entries: &[TypedFormFieldEntry]) -> Vec<&TypedFormFieldCollection> {
	entries
		.iter()
		.filter_map(|entry| match entry {
			TypedFormFieldEntry::Collection(collection) => Some(collection.as_ref()),
			_ => None,
		})
		.collect()
}

fn collection_item_values_ident(
	form_name: &syn::Ident,
	collection_name: &syn::Ident,
) -> syn::Ident {
	format_ident!(
		"{}{}ItemValues",
		upper_camel_ident_fragment(form_name),
		upper_camel_ident_fragment(collection_name),
		span = collection_name.span()
	)
}

fn upper_camel_ident_fragment(ident: &syn::Ident) -> String {
	let raw = ident.to_string();
	let input = raw.strip_prefix("r#").unwrap_or(&raw);
	let mut out = String::new();
	let mut upper_next = true;
	for ch in input.chars() {
		if ch == '_' {
			upper_next = true;
			continue;
		}
		if upper_next {
			out.push(ch.to_ascii_uppercase());
			upper_next = false;
		} else {
			out.push(ch);
		}
	}
	out
}

fn generate_ambient_argument_deprecation_markers(
	macro_ast: &TypedFormMacro,
	ambient_arguments_source: Option<AmbientArgumentsSource>,
) -> TokenStream {
	let legacy_marker = if matches!(
		ambient_arguments_source,
		Some(AmbientArgumentsSource::StripArguments)
	) {
		let ident = format_ident!(
			"__REINHARDT_{}_STRIP_ARGUMENTS_DEPRECATED",
			upper_snake_ident_fragment(&macro_ast.name)
		);
		quote! {
			#[deprecated(
				note = "`strip_arguments` was renamed to `ambient_arguments`; use `ambient_arguments` for values supplied from ambient context"
			)]
			const #ident: () = ();
			let _ = #ident;
		}
	} else {
		quote! {}
	};

	let has_csrf_ambient_arg = macro_ast
		.strip_arguments
		.iter()
		.any(|arg| is_csrf_argument_name(&arg.name));
	let csrf_marker = if has_csrf_ambient_arg {
		let ident = format_ident!(
			"__REINHARDT_{}_CSRF_AMBIENT_DEPRECATED",
			upper_snake_ident_fragment(&macro_ast.name)
		);
		quote! {
			#[deprecated(
				note = "CSRF should be supplied by #[server_fn] client stubs via the X-CSRFToken header, not as a server_fn business argument"
			)]
			const #ident: () = ();
			let _ = #ident;
		}
	} else {
		quote! {}
	};

	quote! {
		#legacy_marker
		#csrf_marker
	}
}

fn upper_snake_ident_fragment(ident: &syn::Ident) -> String {
	let mut out = String::new();
	for (index, ch) in ident.to_string().chars().enumerate() {
		if ch.is_ascii_uppercase() && index > 0 {
			out.push('_');
		}
		out.push(ch.to_ascii_uppercase());
	}
	out
}

fn should_pass_csrf_as_business_argument(macro_ast: &TypedFormMacro) -> bool {
	macro_ast
		.strip_arguments
		.iter()
		.any(|arg| is_csrf_argument_name(&arg.name))
}

fn is_csrf_argument_name(name: &syn::Ident) -> bool {
	matches!(
		name.to_string().as_str(),
		"csrf_token" | "_csrf_token" | "csrfmiddlewaretoken"
	)
}

fn generate_form_runtime_contract(
	macro_ast: &TypedFormMacro,
	pages_crate: &TokenStream,
) -> TokenStream {
	let values_ident = format_ident!("{}Values", macro_ast.name);
	let field_ident = format_ident!("{}Field", macro_ast.name);
	let form_ident = &macro_ast.name;
	let all_fields = collect_scalar_fields(&macro_ast.fields);
	let collections = collect_collections(&macro_ast.fields);
	let mut unsupported_fields: Vec<String> = all_fields
		.iter()
		.filter(|field| !supports_generated_form_values(&field.field_type))
		.map(|field| {
			format!(
				"{} ({})",
				field.name,
				field_type_to_string(&field.field_type)
			)
		})
		.collect();
	for collection in &collections {
		unsupported_fields.extend(
			collect_scalar_fields(&collection.fields)
				.iter()
				.filter(|field| !supports_generated_form_values(&field.field_type))
				.map(|field| {
					format!(
						"{}.{} ({})",
						collection.name,
						field.name,
						field_type_to_string(&field.field_type)
					)
				}),
		);
	}
	if !unsupported_fields.is_empty() {
		let message = format!(
			"form! cannot generate a use_form runtime contract for unsupported field(s): {}.",
			unsupported_fields.join(", ")
		);
		return quote! {
			compile_error!(#message);
		};
	}

	let collection_item_value_structs: Vec<TokenStream> = collections
		.iter()
		.map(|collection| {
			let ident = collection_item_values_ident(&macro_ast.name, &collection.name);
			let item_fields = collect_scalar_fields(&collection.fields);
			let fields: Vec<TokenStream> = item_fields
				.iter()
				.map(|field| {
					let name = &field.name;
					let ty = field_type_to_value_type(&field.field_type);
					quote! { pub #name: #ty, }
				})
				.collect();
			let default_fields: Vec<TokenStream> = item_fields
				.iter()
				.map(|field| {
					let name = &field.name;
					let init = field_type_default_value(&field.field_type);
					quote! { #name: #init, }
				})
				.collect();
			quote! {
				#[derive(Clone, PartialEq)]
				struct #ident {
					#(#fields)*
				}

				impl ::core::default::Default for #ident {
					fn default() -> Self {
						Self {
							#(#default_fields)*
						}
					}
				}
			}
		})
		.collect();

	let mut value_fields: Vec<TokenStream> = all_fields
		.iter()
		.map(|field| {
			let name = &field.name;
			let ty = field_type_to_value_type(&field.field_type);
			quote! { pub #name: #ty, }
		})
		.collect();
	value_fields.extend(collections.iter().map(|collection| {
		let name = &collection.name;
		let item_values_ident = collection_item_values_ident(&macro_ast.name, &collection.name);
		quote! { pub #name: ::std::vec::Vec<#item_values_ident>, }
	}));

	let mut values_fields: Vec<TokenStream> = all_fields
		.iter()
		.map(|field| {
			let name = &field.name;
			quote! { #name: self.#name.get(), }
		})
		.collect();
	values_fields.extend(collections.iter().map(|collection| {
		let name = &collection.name;
		quote! {
			#name: self
				.#name
				.get()
				.into_iter()
				.map(#pages_crate::CollectionItem::into_value)
				.collect(),
		}
	}));
	let mut initial_values_fields: Vec<TokenStream> = all_fields
		.iter()
		.map(|field| {
			let name = &field.name;
			let init = field_type_default_value(&field.field_type);
			quote! { #name: #init, }
		})
		.collect();
	initial_values_fields.extend(collections.iter().map(|collection| {
		let name = &collection.name;
		quote! { #name: ::std::vec::Vec::new(), }
	}));

	let mut apply_values_fields: Vec<TokenStream> = all_fields
		.iter()
		.map(|field| {
			let name = &field.name;
			let custom_widget_touched_reset =
				custom_widget_touched_ident(field).map(|touched_name| {
					quote! { self.#touched_name.set(false); }
				});
			quote! {
				self.#name.set(values.#name.clone());
				#custom_widget_touched_reset
			}
		})
		.collect();
	apply_values_fields.extend(collections.iter().map(|collection| {
		let name = &collection.name;
		quote! {
				self.__path_signals.borrow_mut().clear();
				self.#name.set(
					values
						.#name
					.iter()
					.cloned()
					.enumerate()
					.map(|(index, value)| {
						#pages_crate::CollectionItem::new(
							#pages_crate::CollectionItemKey::from_runtime_index(index as u64),
							index,
							value,
						)
					})
					.collect(),
			);
		}
	}));

	let field_variants: Vec<syn::Ident> = all_fields
		.iter()
		.map(|field| field_variant_ident(&field.name))
		.collect();
	let field_name_arms: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			let name = field.name.to_string();
			let name = name.strip_prefix("r#").unwrap_or(&name).to_string();
			quote! {
				#name => ::core::option::Option::Some(#field_ident::#variant),
			}
		})
		.collect();
	let field_accessor_methods: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			let method = format_ident!("{}_field", field.name);
			quote! {
				pub fn #method(&self) -> #field_ident {
					#field_ident::#variant
				}
			}
		})
		.collect();
	let collection_ident = format_ident!("{}Collection", macro_ast.name);
	let field_path_ident = format_ident!("{}FieldPath", macro_ast.name);
	let collection_variants: Vec<syn::Ident> = collections
		.iter()
		.map(|collection| field_variant_ident(&collection.name))
		.collect();
	let collection_accessor_methods: Vec<TokenStream> = collections
		.iter()
		.zip(collection_variants.iter())
		.map(|(collection, variant)| {
			let method = format_ident!("{}_collection", collection.name);
			quote! {
				pub fn #method(&self) -> #collection_ident {
					#collection_ident::#variant
				}
			}
		})
		.collect();
	let collection_field_idents: Vec<syn::Ident> = collections
		.iter()
		.map(|collection| {
			format_ident!(
				"{}{}Field",
				upper_camel_ident_fragment(&macro_ast.name),
				upper_camel_ident_fragment(&collection.name),
				span = collection.name.span()
			)
		})
		.collect();
	let collection_field_definitions: Vec<TokenStream> = collections
		.iter()
		.zip(collection_field_idents.iter())
		.map(|(collection, field_ident)| {
			let variants: Vec<syn::Ident> = collect_scalar_fields(&collection.fields)
				.iter()
				.map(|field| field_variant_ident(&field.name))
				.collect();
			quote! {
				#[allow(
					dead_code,
					reason = "Generated field path variants are constructed only when callers use path accessors."
				)]
				#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
				enum #field_ident {
					#(#variants,)*
				}
			}
		})
		.collect();
	let field_path_variants: Vec<TokenStream> = collections
		.iter()
		.zip(collection_variants.iter())
		.zip(collection_field_idents.iter())
		.map(|((_collection, variant), field_ident)| {
			quote! {
				#variant {
					item: #pages_crate::CollectionItemKey,
					field: #field_ident,
				}
			}
		})
		.collect();
	let field_path_accessor_methods: Vec<TokenStream> = collections
		.iter()
		.zip(collection_variants.iter())
		.zip(collection_field_idents.iter())
		.flat_map(|((collection, collection_variant), field_ident)| {
			let collection_variant = collection_variant.clone();
			let field_ident = field_ident.clone();
			let field_path_ident = field_path_ident.clone();
			collect_scalar_fields(&collection.fields)
				.into_iter()
				.map(move |field| {
					let method = format_ident!("{}_{}_path", collection.name, field.name);
					let field_variant = field_variant_ident(&field.name);
					quote! {
						pub fn #method(
							&self,
							item: #pages_crate::CollectionItemKey,
						) -> #field_path_ident {
							#field_path_ident::#collection_variant {
								item,
								field: #field_ident::#field_variant,
							}
						}
					}
				})
		})
		.collect();
	let collection_token_definition = if collections.is_empty() {
		quote! {}
	} else {
		quote! {
			#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
			enum #collection_ident {
				#(#collection_variants,)*
			}

			impl #form_ident {
				#(#collection_accessor_methods)*
			}
		}
	};
	let field_path_token_definition = if collections.is_empty() {
		quote! {}
	} else {
		quote! {
			#(#collection_field_definitions)*

			#[derive(Clone, Debug, PartialEq, Eq, Hash)]
			enum #field_path_ident {
				#(#field_path_variants,)*
			}

			impl #form_ident {
				#(#field_path_accessor_methods)*
			}
		}
	};
	let collection_token_guard = if collections.is_empty() {
		quote! {}
	} else {
		quote! {
			let _ = ::core::mem::size_of::<#collection_ident>();
			let _ = ::core::mem::size_of::<#field_path_ident>();
			#(
				let _ = ::core::mem::size_of::<#collection_field_idents>();
			)*
		}
	};
	let field_setters: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			let name = &field.name;
			let ty = field_type_to_value_type(&field.field_type);
			let custom_widget_touched_set =
				custom_widget_touched_ident(field).map(|touched_name| {
					quote! { self.#touched_name.set(true); }
				});
			quote! {
				#field_ident::#variant => {
					let boxed: ::std::boxed::Box<dyn ::core::any::Any> =
						::std::boxed::Box::new(value);
					match boxed.downcast::<#ty>() {
						Ok(value) => {
							self.#name.set(*value);
							#custom_widget_touched_set
						}
						Err(_) => panic!(
							"form! field {:?} does not accept values of type {}",
							field,
							::core::any::type_name::<T>(),
						),
					}
				}
			}
		})
		.collect();
	let apply_field_values: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			let name = &field.name;
			let custom_widget_touched_reset =
				custom_widget_touched_ident(field).map(|touched_name| {
					quote! { self.#touched_name.set(false); }
				});
			quote! {
				#field_ident::#variant => {
					self.#name.set(values.#name.clone());
					#custom_widget_touched_reset
				}
			}
		})
		.collect();
	let field_dirty_arms: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			let name = &field.name;
			quote! {
				#field_ident::#variant => current.#name != defaults.#name,
			}
		})
		.collect();
	let field_focus_arms: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			let field_id = field.name.to_string();
			quote! {
				#field_ident::#variant => #field_id,
			}
		})
		.collect();
	let watch_field_arms: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			let name = &field.name;
			quote! {
				#field_ident::#variant => {
					let boxed: ::std::boxed::Box<dyn ::core::any::Any> =
						::std::boxed::Box::new(self.#name.clone());
					boxed
						.downcast::<#pages_crate::reactive::Signal<T>>()
						.ok()
						.map(|signal| *signal)
				}
			}
		})
		.collect();
	let custom_widget_error_arms: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			if matches!(field.widget, TypedWidget::CustomExperimental(_)) {
				let error_name = format_ident!(
					"__{}_custom_widget_error",
					field.name,
					span = field.name.span()
				);
				quote! {
					#field_ident::#variant => self.#error_name.get(),
				}
			} else {
				quote! {
					#field_ident::#variant => ::core::option::Option::None,
				}
			}
		})
		.collect();
	let custom_widget_error_setters: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.map(|(field, variant)| {
			if matches!(field.widget, TypedWidget::CustomExperimental(_)) {
				let error_name = format_ident!(
					"__{}_custom_widget_error",
					field.name,
					span = field.name.span()
				);
				quote! {
					#field_ident::#variant => self.#error_name.set(error),
				}
			} else {
				quote! {
					#field_ident::#variant => {
						let _ = error;
					}
				}
			}
		})
		.collect();
	let custom_widget_validation_checks: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.filter_map(|(field, variant)| {
			if matches!(field.widget, TypedWidget::CustomExperimental(_)) {
				let error_name = format_ident!(
					"__{}_custom_widget_error",
					field.name,
					span = field.name.span()
				);
				Some(quote! {
					if let ::core::option::Option::Some(__custom_widget_error) =
						self.#error_name.get()
					{
						error.add_field_error(
							#field_ident::#variant,
							__custom_widget_error.message(),
						);
					}
				})
			} else {
				None
			}
		})
		.collect();
	let values_dirty_impl = if collections.is_empty() {
		quote! {}
	} else {
		quote! {
			fn runtime_values_are_dirty(
				&self,
				current: &Self::Values,
				defaults: &Self::Values,
			) -> bool {
				current != defaults
			}
		}
	};
	let apply_pristine_values_impl = if collections.is_empty() {
		quote! {}
	} else {
		let scalar_pristine_assignments: Vec<TokenStream> = all_fields
			.iter()
			.map(|field| {
				let name = &field.name;
				quote! {
					if current.#name == old_defaults.#name {
						self.#name.set(new_defaults.#name.clone());
					}
				}
			})
			.collect();
		let collection_pristine_assignments: Vec<TokenStream> = collections
			.iter()
			.map(|collection| {
				let name = &collection.name;
				quote! {
						if current.#name == old_defaults.#name {
							self.__path_signals.borrow_mut().clear();
							self.#name.set(
								new_defaults
									.#name
								.iter()
								.cloned()
								.enumerate()
								.map(|(index, value)| {
									#pages_crate::CollectionItem::new(
										#pages_crate::CollectionItemKey::from_runtime_index(index as u64),
										index,
										value,
									)
								})
								.collect(),
						);
					}
				}
			})
			.collect();
		quote! {
		fn runtime_apply_pristine_values(
			&self,
			current: &Self::Values,
			old_defaults: &Self::Values,
			new_defaults: &Self::Values,
		) {
			#(#scalar_pristine_assignments)*
			#(#collection_pristine_assignments)*
		}
		}
	};
	let path_values_from_values_blocks: Vec<TokenStream> = collections
		.iter()
		.map(|collection| {
			let collection_name = &collection.name;
			let collection_name_text = collection.name.to_string();
			let value_inserts: Vec<TokenStream> = collect_scalar_fields(&collection.fields)
				.iter()
				.map(|field| {
					let field_name = &field.name;
					let field_name_text = field.name.to_string();
					quote! {
						{
							let value: ::std::rc::Rc<dyn ::core::any::Any> =
								::std::rc::Rc::new(value.#field_name.clone());
							path_values.insert(
								::std::format!(
									"{}:{:?}:{}",
									#collection_name_text,
									item.key(),
									#field_name_text,
								),
								value,
							);
						}
					}
				})
				.collect();
			quote! {
				for item in self.#collection_name.get() {
					if let ::core::option::Option::Some(value) =
						values.#collection_name.get(item.index())
					{
						#(#value_inserts)*
					}
				}
			}
		})
		.collect();
	let path_exists_blocks: Vec<TokenStream> = collections
		.iter()
		.map(|collection| {
			let collection_name = &collection.name;
			let collection_name_text = collection.name.to_string();
			let path_checks: Vec<TokenStream> = collect_scalar_fields(&collection.fields)
				.iter()
				.map(|field| {
					let field_name_text = field.name.to_string();
					quote! {
						if path_key
							== ::std::format!(
								"{}:{:?}:{}",
								#collection_name_text,
								item.key(),
								#field_name_text,
							)
						{
							return true;
						}
					}
				})
				.collect();
			quote! {
				for item in self.#collection_name.get() {
					#(#path_checks)*
				}
			}
		})
		.collect();
	let path_value_equals_blocks: Vec<TokenStream> = collections
		.iter()
		.map(|collection| {
			let collection_name = &collection.name;
			let collection_name_text = collection.name.to_string();
			let value_checks: Vec<TokenStream> = collect_scalar_fields(&collection.fields)
				.iter()
				.map(|field| {
					let field_name = &field.name;
					let field_name_text = field.name.to_string();
					let field_type = field_type_to_value_type(&field.field_type);
					quote! {
						if path_key
							== ::std::format!(
								"{}:{:?}:{}",
								#collection_name_text,
								item.key(),
								#field_name_text,
							)
						{
							return default
								.downcast_ref::<#field_type>()
								.map(|default| &item_value.#field_name == default);
						}
					}
				})
				.collect();
			quote! {
				for item in self.#collection_name.get() {
					let item_value = item.value();
					#(#value_checks)*
				}
			}
		})
		.collect();
	let path_signal_sync_blocks: Vec<TokenStream> = collections
		.iter()
		.map(|collection| {
			let collection_name = &collection.name;
			let collection_name_text = collection.name.to_string();
			let signal_updates: Vec<TokenStream> = collect_scalar_fields(&collection.fields)
				.iter()
				.map(|field| {
					let field_name = &field.name;
					let field_name_text = field.name.to_string();
					let field_type = field_type_to_value_type(&field.field_type);
					quote! {
						{
							let path_key = ::std::format!(
								"{}:{:?}:{}",
								#collection_name_text,
								item.key(),
								#field_name_text,
							);
							let path_signal = self
								.__path_signals
								.borrow()
								.get(&path_key)
								.and_then(|signal| {
									signal
										.downcast_ref::<#pages_crate::reactive::Signal<#field_type>>()
										.cloned()
								});
							if let ::core::option::Option::Some(path_signal) = path_signal {
								path_signal.set(item_value.#field_name.clone());
							}
						}
					}
				})
				.collect();
			quote! {
				for item in self.#collection_name.get() {
					let item_value = item.value();
					#(#signal_updates)*
				}
			}
		})
		.collect();
	let changed_collection_key_checks: Vec<TokenStream> = collections
		.iter()
		.zip(collection_variants.iter())
		.map(|(collection, variant)| {
			let collection_name = &collection.name;
			quote! {
				if current.#collection_name != previous.#collection_name {
					keys.push(
						<Self as #pages_crate::form_state::FormCollectionRuntimeSource>::runtime_collection_key(
							#collection_ident::#variant,
						),
					);
				}
			}
		})
		.collect();
	let collection_runtime_impl = if collections.is_empty() {
		quote! {}
	} else {
		let collection_len_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.map(|(collection, variant)| {
				let name = &collection.name;
				quote! {
				#collection_ident::#variant => self.#name.get().len(),
				}
			})
			.collect();
		let collection_key_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.map(|(collection, variant)| {
				let name = collection.name.to_string();
				quote! {
					#collection_ident::#variant => #name.to_string(),
				}
			})
			.collect();
		let collection_dirty_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.map(|(collection, variant)| {
				let name = &collection.name;
				quote! {
					#collection_ident::#variant => current.#name != defaults.#name,
				}
			})
			.collect();
		let collection_insert_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.map(|(collection, variant)| {
				let name = &collection.name;
				let item_ident = collection_item_values_ident(&macro_ast.name, &collection.name);
				let collection_name = name.to_string();
				quote! {
					#collection_ident::#variant => {
						let boxed: ::std::boxed::Box<dyn ::core::any::Any> =
							::std::boxed::Box::new(value);
						match boxed.downcast::<#item_ident>() {
							::core::result::Result::Ok(value) => {
								let mut items = self.#name.get();
								let index = index.min(items.len());
								items.insert(
									index,
									#pages_crate::CollectionItem::new(key, index, *value),
								);
								self.#name.set(
									items
										.into_iter()
										.enumerate()
										.map(|(index, item)| {
											#pages_crate::CollectionItem::new(
												item.key(),
												index,
												item.into_value(),
											)
										})
										.collect(),
								);
								self.__path_signals.borrow_mut().clear();
							}
							::core::result::Result::Err(_) => panic!(
								"form! collection '{}' does not accept values of type {}",
								#collection_name,
								::core::any::type_name::<T>(),
							),
						}
					}
				}
			})
			.collect();
		let collection_remove_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.map(|(collection, variant)| {
				let name = &collection.name;
				quote! {
					#collection_ident::#variant => {
						let mut items = self.#name.get();
						if let ::core::option::Option::Some(index) =
							items.iter().position(|item| item.key() == key)
						{
							items.remove(index);
							self.#name.set(
								items
									.into_iter()
									.enumerate()
									.map(|(index, item)| {
										#pages_crate::CollectionItem::new(
											item.key(),
											index,
											item.into_value(),
										)
									})
									.collect(),
							);
							self.__path_signals.borrow_mut().clear();
							true
						} else {
							false
						}
					}
				}
			})
			.collect();
		let collection_move_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.map(|(collection, variant)| {
				let name = &collection.name;
				quote! {
					#collection_ident::#variant => {
						let mut items = self.#name.get();
						if let ::core::option::Option::Some(from_index) =
							items.iter().position(|item| item.key() == key)
						{
							let item = items.remove(from_index);
							let to_index = target_index.min(items.len());
							items.insert(to_index, item);
							self.#name.set(
								items
									.into_iter()
									.enumerate()
									.map(|(index, item)| {
										#pages_crate::CollectionItem::new(
											item.key(),
											index,
											item.into_value(),
										)
									})
									.collect(),
							);
							self.__path_signals.borrow_mut().clear();
							::core::option::Option::Some((from_index, to_index))
						} else {
							::core::option::Option::None
						}
					}
				}
			})
			.collect();
		let field_path_key_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.zip(collection_field_idents.iter())
			.flat_map(|((collection, collection_variant), field_ident)| {
				let collection_variant = collection_variant.clone();
				let field_ident = field_ident.clone();
				let collection_name = collection.name.to_string();
				let field_path_ident = field_path_ident.clone();
				collect_scalar_fields(&collection.fields)
					.into_iter()
					.map(move |field| {
						let field_name = field.name.to_string();
						let field_variant = field_variant_ident(&field.name);
						quote! {
							#field_path_ident::#collection_variant {
								item,
								field: #field_ident::#field_variant,
							} => ::std::format!("{}:{:?}:{}", #collection_name, item, #field_name),
						}
					})
			})
			.collect();
		let field_path_keys_for_item_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.map(|(collection, collection_variant)| {
				let collection_name = collection.name.to_string();
				let field_names: Vec<String> = collect_scalar_fields(&collection.fields)
					.iter()
					.map(|field| field.name.to_string())
					.collect();
				quote! {
					#collection_ident::#collection_variant => {
						::std::vec![
							#(
								::std::format!("{}:{:?}:{}", #collection_name, key, #field_names)
							),*
						]
					}
				}
			})
			.collect();
		let watch_path_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.zip(collection_field_idents.iter())
			.flat_map(|((collection, collection_variant), field_ident)| {
				let collection_variant = collection_variant.clone();
				let field_ident = field_ident.clone();
				let collection_name = &collection.name;
				let field_path_ident = field_path_ident.clone();
				collect_scalar_fields(&collection.fields)
					.into_iter()
					.map(move |field| {
						let field_name = &field.name;
						let field_type = field_type_to_value_type(&field.field_type);
						let field_variant = field_variant_ident(&field.name);
						quote! {
							#field_path_ident::#collection_variant {
								item,
								field: #field_ident::#field_variant,
							} => {
								let path_key = Self::runtime_field_path_key(
									&#field_path_ident::#collection_variant {
										item,
										field: #field_ident::#field_variant,
									},
								);
								let value = self
									.#collection_name
									.get()
									.into_iter()
									.find(|entry| entry.key() == item)
									.map(|entry| entry.value().#field_name.clone())?;
								if let ::core::option::Option::Some(signal) = {
									self.__path_signals
										.borrow()
										.get(&path_key)
										.and_then(|signal| {
											signal
												.downcast_ref::<#pages_crate::reactive::Signal<#field_type>>()
												.cloned()
										})
								} {
									let boxed: ::std::boxed::Box<dyn ::core::any::Any> =
										::std::boxed::Box::new(signal.clone());
									match boxed.downcast::<#pages_crate::reactive::Signal<T>>() {
										::core::result::Result::Ok(typed_signal) => {
											signal.set(value);
											return ::core::option::Option::Some(*typed_signal);
										}
										::core::result::Result::Err(_) => {
											return ::core::option::Option::None;
										}
									}
								}

								let signal = #pages_crate::reactive::Signal::new(value);
								let boxed: ::std::boxed::Box<dyn ::core::any::Any> =
									::std::boxed::Box::new(signal);
								let signal = match boxed
									.downcast::<#pages_crate::reactive::Signal<T>>()
								{
									::core::result::Result::Ok(signal) => *signal,
									::core::result::Result::Err(_) => {
										return ::core::option::Option::None;
									}
								};
								self.__path_signals
									.borrow_mut()
									.insert(path_key, ::std::boxed::Box::new(signal.clone()));
								::core::option::Option::Some(signal)
							}
						}
					})
			})
			.collect();
		let set_path_value_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.zip(collection_field_idents.iter())
			.flat_map(|((collection, collection_variant), field_ident)| {
				let collection_variant = collection_variant.clone();
				let field_ident = field_ident.clone();
				let collection_name = &collection.name;
				let field_path_ident = field_path_ident.clone();
				collect_scalar_fields(&collection.fields)
					.into_iter()
					.map(move |field| {
						let field_name = &field.name;
						let field_type = field_type_to_value_type(&field.field_type);
						let field_variant = field_variant_ident(&field.name);
						quote! {
							#field_path_ident::#collection_variant {
								item,
								field: #field_ident::#field_variant,
							} => {
								let boxed: ::std::boxed::Box<dyn ::core::any::Any> =
									::std::boxed::Box::new(value);
								match boxed.downcast::<#field_type>() {
									::core::result::Result::Ok(value) => {
										let value = *value;
										let mut items = self.#collection_name.get();
											if let ::core::option::Option::Some(index) =
												items.iter().position(|entry| entry.key() == item)
											{
												let item_index = items[index].index();
											let mut item_value = items[index].value().clone();
											item_value.#field_name = value.clone();
											items[index] = #pages_crate::CollectionItem::new(
												item,
												item_index,
												item_value,
											);
											self.#collection_name.set(items);
											let signal = self
												.__path_signals
												.borrow()
												.get(&path_key)
												.and_then(|signal| {
													signal
														.downcast_ref::<
															#pages_crate::reactive::Signal<#field_type>,
														>()
														.cloned()
												});
											if let ::core::option::Option::Some(signal) = signal {
												signal.set(value);
											}
											true
											} else {
												false
											}
										}
									::core::result::Result::Err(_) => panic!(
										"form! field path {:?} does not accept values of type {}",
										#field_path_ident::#collection_variant {
											item,
											field: #field_ident::#field_variant,
										},
										::core::any::type_name::<T>(),
									),
								}
							}
						}
					})
			})
			.collect();
		let path_dirty_arms: Vec<TokenStream> = collections
			.iter()
			.zip(collection_variants.iter())
			.zip(collection_field_idents.iter())
			.flat_map(|((collection, collection_variant), field_ident)| {
				let collection_variant = collection_variant.clone();
				let field_ident = field_ident.clone();
				let collection_name = &collection.name;
				let field_path_ident = field_path_ident.clone();
				collect_scalar_fields(&collection.fields)
					.into_iter()
					.map(move |field| {
						let field_name = &field.name;
						let field_variant = field_variant_ident(&field.name);
						quote! {
							#field_path_ident::#collection_variant {
								item,
								field: #field_ident::#field_variant,
							} => {
								let items = self.#collection_name.get();
								if let ::core::option::Option::Some(index) =
									items.iter().position(|entry| entry.key() == item)
								{
									current
										.#collection_name
										.get(index)
										.map(|item| &item.#field_name)
										!= defaults
											.#collection_name
											.get(index)
											.map(|item| &item.#field_name)
								} else {
									false
								}
							}
						}
					})
			})
			.collect();
		quote! {
				impl #pages_crate::form_state::FormCollectionRuntimeSource for #form_ident {
					type Collection = #collection_ident;
					type FieldPath = #field_path_ident;

					fn runtime_field_path_key(path: &Self::FieldPath) -> ::std::string::String {
						match path.clone() {
							#(#field_path_key_arms)*
						}
					}

					fn runtime_collection_key(collection: Self::Collection) -> ::std::string::String {
						match collection {
							#(#collection_key_arms)*
						}
					}

					fn runtime_field_path_keys_for_item(
						collection: Self::Collection,
					key: #pages_crate::CollectionItemKey,
				) -> ::std::vec::Vec<::std::string::String> {
					match collection {
						#(#field_path_keys_for_item_arms)*
					}
				}

					fn runtime_collection_len(&self, collection: Self::Collection) -> usize {
						match collection {
							#(#collection_len_arms)*
						}
					}

					fn runtime_collection_is_dirty(
						&self,
						collection: Self::Collection,
						current: &Self::Values,
						defaults: &Self::Values,
					) -> bool {
						match collection {
							#(#collection_dirty_arms)*
						}
					}

					fn runtime_insert_collection_item<T>(
					&self,
					collection: Self::Collection,
					index: usize,
					key: #pages_crate::CollectionItemKey,
					value: T,
				)
				where
					T: ::core::any::Any + ::core::clone::Clone + 'static,
				{
					match collection {
						#(#collection_insert_arms)*
					}
				}

				fn runtime_remove_collection_item(
					&self,
					collection: Self::Collection,
					key: #pages_crate::CollectionItemKey,
				) -> bool {
					match collection {
						#(#collection_remove_arms)*
					}
				}

				fn runtime_move_collection_item(
					&self,
					collection: Self::Collection,
					key: #pages_crate::CollectionItemKey,
					target_index: usize,
				) -> ::core::option::Option<(usize, usize)> {
					match collection {
						#(#collection_move_arms)*
					}
				}

				fn runtime_watch_path<T>(
					&self,
					path: Self::FieldPath,
				) -> ::core::option::Option<#pages_crate::reactive::Signal<T>>
				where
					T: ::core::clone::Clone + 'static,
				{
					match path {
						#(#watch_path_arms)*
					}
				}

					fn runtime_set_path_value<T>(&self, path: Self::FieldPath, value: T) -> bool
					where
						T: ::core::any::Any + 'static,
					{
						let path_key = Self::runtime_field_path_key(&path);
						match path {
						#(#set_path_value_arms)*
					}
				}

				fn runtime_path_is_dirty(
					&self,
					path: &Self::FieldPath,
					current: &Self::Values,
					defaults: &Self::Values,
					) -> bool {
						match path.clone() {
							#(#path_dirty_arms)*
						}
					}
				}
		}
	};
	let required_validation_checks: Vec<TokenStream> = all_fields
		.iter()
		.zip(field_variants.iter())
		.filter_map(|(field, variant)| {
			if !field.validation.required {
				return None;
			}
			let name = &field.name;
			let message = format!("{} is required", name);
			let empty_check = runtime_required_empty_check(name, &field.field_type)?;
			Some(quote! {
				if #empty_check {
					error.add_field_error(#field_ident::#variant, #message);
				}
			})
		})
		.collect();
	let collection_constraints_validation_checks: Vec<TokenStream> = collections
		.iter()
		.zip(collection_variants.iter())
		.flat_map(|(collection, collection_variant)| {
			let collection_name = &collection.name;
			let collection_name_text = collection.name.to_string();
			let mut checks = Vec::new();
			if let Some(min_items) = collection.min_items {
				let message = format!(
					"{} requires at least {} item{}",
					collection_name_text,
					min_items,
					if min_items == 1 { "" } else { "s" }
				);
				checks.push(quote! {
					{
						let __len = self.#collection_name.get().len();
						if __len < #min_items {
							error.add_collection_error(
								<#form_ident as #pages_crate::form_state::FormCollectionRuntimeSource>::runtime_collection_key(
									#collection_ident::#collection_variant,
								),
								#message,
							);
						}
					}
				});
			}
			if let Some(max_items) = collection.max_items {
				let message = format!(
					"{} allows at most {} item{}",
					collection_name_text,
					max_items,
					if max_items == 1 { "" } else { "s" }
				);
				checks.push(quote! {
					{
						let __len = self.#collection_name.get().len();
						if __len > #max_items {
							error.add_collection_error(
								<#form_ident as #pages_crate::form_state::FormCollectionRuntimeSource>::runtime_collection_key(
									#collection_ident::#collection_variant,
								),
								#message,
							);
						}
					}
				});
			}
			checks
		})
		.collect();
	let collection_required_validation_checks: Vec<TokenStream> = collections
		.iter()
		.zip(collection_variants.iter())
		.zip(collection_field_idents.iter())
		.flat_map(|((collection, collection_variant), field_ident)| {
			let collection_name = collection.name.clone();
			let collection_name_text = collection.name.to_string();
			let collection_variant = collection_variant.clone();
			let field_ident = field_ident.clone();
			let field_path_ident = field_path_ident.clone();
			collect_scalar_fields(&collection.fields)
				.into_iter()
				.filter_map(move |field| {
					if !field.validation.required {
						return None;
					}
					let field_name = field.name.clone();
					let field_name_text = field.name.to_string();
					let field_variant = field_variant_ident(&field.name);
					let message = format!("{}.{} is required", collection_name_text, field_name_text);
					let empty_check =
						runtime_collection_required_empty_check(&field_name, &field.field_type)?;
					Some(quote! {
						for item in self.#collection_name.get() {
							let item_value = item.value();
							if #empty_check {
								let path = #field_path_ident::#collection_variant {
									item: item.key(),
									field: #field_ident::#field_variant,
								};
								error.add_path_error(
									<#form_ident as #pages_crate::form_state::FormCollectionRuntimeSource>::runtime_field_path_key(&path),
									#message,
								);
							}
						}
					})
				})
				.collect::<Vec<_>>()
		})
		.collect();
	let wasm_focus_body = if all_fields.is_empty() {
		quote! {
			match field {}
		}
	} else {
		quote! {
			let field_id = match field {
				#(#field_focus_arms)*
			};
			let window = ::web_sys::window()
				.ok_or(#pages_crate::FocusError::MissingTarget)?;
			let document = window
				.document()
				.ok_or(#pages_crate::FocusError::MissingTarget)?;
			let element = document
				.get_element_by_id(field_id)
				.ok_or(#pages_crate::FocusError::MissingTarget)?;
			use ::wasm_bindgen::JsCast;
			let element = element
				.dyn_into::<::web_sys::HtmlElement>()
				.map_err(|_| #pages_crate::FocusError::Unsupported)?;
			element
				.focus()
				.map_err(|_| #pages_crate::FocusError::Unsupported)
		}
	};

	quote! {
		#(#collection_item_value_structs)*

		#[derive(Clone, PartialEq)]
		struct #values_ident {
			#(#value_fields)*
		}

		#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
		enum #field_ident {
			#(#field_variants,)*
		}

		#collection_token_definition
		#field_path_token_definition

		impl #form_ident {
			#(#field_accessor_methods)*
		}

		impl #pages_crate::FormRuntimeSource for #form_ident {
			type Values = #values_ident;
			type Field = #field_ident;

			fn runtime_reactive_scope(
				&self,
			) -> ::core::option::Option<::std::rc::Rc<#pages_crate::reactive::ReactiveScope>> {
				self.__reinhardt_reactive_scope.clone()
			}

			fn runtime_initial_values(&self) -> Self::Values {
				self.__initial_values.borrow().clone()
			}

			fn runtime_field_by_name(&self, name: &str) -> ::core::option::Option<Self::Field> {
				match name {
					#(#field_name_arms)*
					_ => ::core::option::Option::None,
				}
			}

			fn runtime_current_values(&self) -> Self::Values {
				#values_ident {
					#(#values_fields)*
				}
			}

			fn runtime_apply_values(&self, values: &Self::Values) {
				#(#apply_values_fields)*
			}

			fn runtime_set_field_value<T>(&self, field: Self::Field, value: T)
			where
				T: ::core::any::Any + 'static,
			{
				match field {
					#(#field_setters)*
				}
			}

			fn runtime_apply_field_value(&self, field: Self::Field, values: &Self::Values) {
				match field {
					#(#apply_field_values)*
				}
			}

				fn runtime_field_is_dirty(
					&self,
					field: Self::Field,
					current: &Self::Values,
					defaults: &Self::Values,
				) -> bool {
					match field {
						#(#field_dirty_arms)*
					}
				}

					#values_dirty_impl

					#apply_pristine_values_impl

					fn runtime_watch_field<T>(&self, field: Self::Field) -> ::core::option::Option<#pages_crate::reactive::Signal<T>>
					where
						T: ::core::clone::Clone + 'static,
			{
					match field {
						#(#watch_field_arms)*
					}
				}

				fn runtime_custom_widget_error(&self, field: Self::Field) -> ::core::option::Option<#pages_crate::FieldError> {
					match field {
						#(#custom_widget_error_arms)*
				}
			}

			fn runtime_set_custom_widget_error(
				&self,
				field: Self::Field,
				error: ::core::option::Option<#pages_crate::FieldError>,
			) {
				match field {
					#(#custom_widget_error_setters)*
					}
				}

				fn runtime_path_values_from_values(
					&self,
					values: &Self::Values,
				) -> ::std::collections::HashMap<
					::std::string::String,
					::std::rc::Rc<dyn ::core::any::Any>,
				> {
					let mut path_values = ::std::collections::HashMap::new();
					#(#path_values_from_values_blocks)*
					path_values
				}

				fn runtime_path_exists(&self, path_key: &str) -> bool {
					#(#path_exists_blocks)*
					false
				}

				fn runtime_path_value_equals(
					&self,
					path_key: &str,
					default: &dyn ::core::any::Any,
				) -> ::core::option::Option<bool> {
					#(#path_value_equals_blocks)*
					::core::option::Option::None
				}

				fn runtime_sync_path_signals(&self) {
					#(#path_signal_sync_blocks)*
				}

				fn runtime_changed_collection_keys(
					&self,
					current: &Self::Values,
					previous: &Self::Values,
				) -> ::std::vec::Vec<::std::string::String> {
					let mut keys = ::std::vec::Vec::new();
					#(#changed_collection_key_checks)*
					keys
				}

				fn runtime_validate(&self) -> ::core::result::Result<(), #pages_crate::FormValidationError<Self::Field>> {
					let mut error = #pages_crate::FormValidationError::new();
					#(#required_validation_checks)*
					#(#collection_constraints_validation_checks)*
					#(#collection_required_validation_checks)*
					match self.validate() {
						::core::result::Result::Ok(()) => {}
					::core::result::Result::Err(errors) => {
						for (field, message) in errors {
							match field {
								#(
									stringify!(#field_variants) => {
										error.add_field_error(#field_ident::#field_variants, message);
									}
								)*
								_ => error.set_form_error(message),
							}
						}
					}
				}
				#(#custom_widget_validation_checks)*
				if error.is_empty() {
					::core::result::Result::Ok(())
				} else {
					::core::result::Result::Err(error)
				}
			}

			fn runtime_fields(&self) -> &'static [Self::Field] {
				&[#(#field_ident::#field_variants),*]
			}

			fn runtime_set_focus(&self, field: Self::Field) -> ::core::result::Result<(), #pages_crate::FocusError> {
				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				{
					#wasm_focus_body
				}
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				{
					let _ = field;
					::core::result::Result::Err(#pages_crate::FocusError::Unsupported)
				}
				}
			}

			#collection_runtime_impl

			let _ = #values_ident {
				#(#initial_values_fields)*
			};
		let _ = ::core::mem::size_of::<#values_ident>();
		let _ = ::core::mem::size_of::<#field_ident>();
		#collection_token_guard
	}
}

fn field_variant_ident(ident: &syn::Ident) -> syn::Ident {
	format_ident!(
		"{}",
		snake_to_pascal(&ident.unraw().to_string()),
		span = ident.span()
	)
}

fn snake_to_pascal(input: &str) -> String {
	let mut out = String::new();
	let mut upper_next = true;
	for ch in input.chars() {
		if ch == '_' {
			upper_next = true;
			continue;
		}
		if upper_next {
			out.push(ch.to_ascii_uppercase());
			upper_next = false;
		} else {
			out.push(ch);
		}
	}
	out
}

fn runtime_required_empty_check(
	name: &syn::Ident,
	field_type: &TypedFieldType,
) -> Option<TokenStream> {
	match field_type {
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField => Some(quote! {
			self.#name.get().trim().is_empty()
		}),
		TypedFieldType::DateField
		| TypedFieldType::TimeField
		| TypedFieldType::DateTimeField
		| TypedFieldType::FileField
		| TypedFieldType::ImageField
		| TypedFieldType::UuidField
		| TypedFieldType::IpAddressField => Some(quote! {
			self.#name.get().is_none()
		}),
		TypedFieldType::MultipleChoiceField { .. } => Some(quote! {
			self.#name.get().is_empty()
		}),
		TypedFieldType::IntegerField
		| TypedFieldType::FloatField
		| TypedFieldType::DecimalField
		| TypedFieldType::BooleanField
		| TypedFieldType::HiddenField { .. }
		| TypedFieldType::ChoiceField { .. }
		| TypedFieldType::JsonField { .. } => None,
	}
}

fn runtime_collection_required_empty_check(
	name: &syn::Ident,
	field_type: &TypedFieldType,
) -> Option<TokenStream> {
	match field_type {
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField => Some(quote! {
			item_value.#name.trim().is_empty()
		}),
		TypedFieldType::DateField
		| TypedFieldType::TimeField
		| TypedFieldType::DateTimeField
		| TypedFieldType::FileField
		| TypedFieldType::ImageField
		| TypedFieldType::UuidField
		| TypedFieldType::IpAddressField => Some(quote! {
			item_value.#name.is_none()
		}),
		TypedFieldType::MultipleChoiceField { .. } => Some(quote! {
			item_value.#name.is_empty()
		}),
		TypedFieldType::IntegerField
		| TypedFieldType::FloatField
		| TypedFieldType::DecimalField
		| TypedFieldType::BooleanField
		| TypedFieldType::HiddenField { .. }
		| TypedFieldType::ChoiceField { .. }
		| TypedFieldType::JsonField { .. } => None,
	}
}

fn supports_generated_form_values(field_type: &TypedFieldType) -> bool {
	match field_type {
		TypedFieldType::HiddenField { inner }
		| TypedFieldType::ChoiceField { inner }
		| TypedFieldType::MultipleChoiceField { inner } => is_basic_form_value_type(inner),
		_ => true,
	}
}

fn supports_form_runtime_contract(entries: &[TypedFormFieldEntry]) -> bool {
	let scalar_supported = collect_scalar_fields(entries)
		.iter()
		.all(|field| supports_generated_form_values(&field.field_type));
	let collection_items_supported = collect_collections(entries).iter().all(|collection| {
		collect_scalar_fields(&collection.fields)
			.iter()
			.all(|field| supports_generated_form_values(&field.field_type))
	});
	scalar_supported && collection_items_supported
}

fn is_basic_form_value_type(ty: &syn::Type) -> bool {
	let syn::Type::Path(path) = ty else {
		return false;
	};
	let Some(segment) = path.path.segments.last() else {
		return false;
	};
	matches!(
		segment.ident.to_string().as_str(),
		"String"
			| "bool" | "i8"
			| "i16" | "i32"
			| "i64" | "i128"
			| "isize" | "u8"
			| "u16" | "u32"
			| "u64" | "u128"
			| "usize" | "f32"
			| "f64"
	)
}

fn generated_model_support_path(model: &syn::Path, suffix: &str) -> syn::Path {
	let mut path = model.clone();
	let segment = path
		.segments
		.last_mut()
		.expect("validated model path must contain a segment");
	segment.ident = format_ident!("{}{}", segment.ident, suffix, span = segment.ident.span());
	segment.arguments = syn::PathArguments::None;
	path
}

fn generate_model_form(
	macro_ast: &TypedFormMacro,
	model_source: &TypedModelFormSource,
	pages_crate: &TokenStream,
	use_statement: &TokenStream,
) -> TokenStream {
	let form_ident = &macro_ast.name;
	let policy_ident = format_ident!("{}SelectionPolicy", form_ident);
	let data_ident = format_ident!("{}Data", form_ident);
	let schema_path = generated_model_support_path(&model_source.model, "FormSchema");
	let payload_path = generated_model_support_path(&model_source.model, "ModelFormData");
	let policy_path = &model_source.policy;
	let server_fn = match &macro_ast.action {
		TypedFormAction::ServerFn(server_fn) => server_fn,
		_ => unreachable!("model-backed forms require a server_fn after validation"),
	};

	let selected_idents: Vec<&syn::Ident> = match &model_source.selection {
		TypedModelFieldSelection::Fields(fields) | TypedModelFieldSelection::Exclude(fields) => {
			fields.iter().collect()
		}
	};
	let override_idents = model_source
		.overrides
		.iter()
		.map(|override_| &override_.field);
	let descriptor_guards = selected_idents
		.into_iter()
		.chain(override_idents)
		.map(|field| {
			quote! {
				let _: &#pages_crate::form::ModelFormFieldDescriptor = #schema_path::#field();
			}
		});
	let selection_policy_body = match &model_source.selection {
		TypedModelFieldSelection::Fields(fields) if fields.is_empty() => quote! { false },
		TypedModelFieldSelection::Fields(fields) => {
			let names = fields.iter().map(ToString::to_string);
			quote! { ::core::matches!(field, #(#names)|*) }
		}
		TypedModelFieldSelection::Exclude(fields) if fields.is_empty() => quote! { true },
		TypedModelFieldSelection::Exclude(fields) => {
			let names = fields.iter().map(ToString::to_string);
			quote! { !::core::matches!(field, #(#names)|*) }
		}
	};
	let policy_body = quote! {
		<#policy_path as #pages_crate::form::ModelFormPolicy>::allows(field)
			&& (#selection_policy_body)
	};
	let (model_form_selection_type, model_form_selection_impl) = match &model_source.selection {
		TypedModelFieldSelection::Fields(fields) => {
			let selection_ident = format_ident!("__ReinhardtModelFormSelection");
			let argument_impls = fields.iter().enumerate().map(|(index, field)| {
				let name = field.to_string();
				let name = name.strip_prefix("r#").unwrap_or(&name);
				quote! {
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					impl #pages_crate::form::ModelFormSelectionArgument<#index> for #selection_ident {
						type Name = ();
						const NAME: &'static str = #name;
						const KIND: ::core::option::Option<#pages_crate::form::ModelFormFieldKind> =
							::core::option::Option::Some(#schema_path::#field().kind);
						const REQUIRED: ::core::option::Option<bool> =
							::core::option::Option::Some(#schema_path::#field().required);
					}
				}
			});
			let argument_count = fields.len();
			(
				quote!(#selection_ident),
				quote! {
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					struct #selection_ident;

					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					impl #pages_crate::form::ModelFormSelectionCount<#argument_count>
						for #selection_ident {}

					#(#argument_impls)*

					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					impl #pages_crate::form::ModelFormSelectionPayload<
						#schema_path,
						#policy_ident,
					> for #selection_ident {
						type Payload = #data_ident;

						fn build_payload(
							state: &#pages_crate::form::ModelFormState<
								#schema_path,
								#policy_ident,
							>,
							) -> ::core::result::Result<
								Self::Payload,
							#pages_crate::form::ModelFormPayloadError,
							> {
								state.build_json_payload_for::<#data_ident, #policy_path>()
							}
					}
				},
			)
		}
		TypedModelFieldSelection::Exclude(_) => (
			quote!(#pages_crate::form::ModelFormPayloadSelection<#data_ident, #policy_path>),
			quote! {},
		),
	};
	let model_form_selection_check = quote! {
		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		const _: () = {
			<#server_fn::marker as #pages_crate::form::ModelFormServerFn<
				#model_form_selection_type,
				#schema_path,
				#policy_ident,
			>>::VALIDATE_SELECTION;
			#pages_crate::form::assert_model_form_error_compatibility::<
				#server_fn::marker,
				#model_form_selection_type,
				#schema_path,
				#policy_ident,
			>();
		};
	};
	let model_form_response_type = quote! {
		<#server_fn::marker as #pages_crate::form::ModelFormServerFn<
			#model_form_selection_type,
			#schema_path,
			#policy_ident,
		>>::Response
	};
	let model_form_policy_check = match &model_source.selection {
		TypedModelFieldSelection::Fields(fields) => {
			let names = fields.iter().map(|field| {
				let name = field.to_string();
				let name = name.strip_prefix("r#").unwrap_or(&name);
				quote!(#name)
			});
			quote! {
				for field in [#(#names),*] {
					if !<#policy_ident as #pages_crate::form::ModelFormPolicy>::allows(field) {
						let error = #pages_crate::ServerFnError::validation_with_message(
							::std::format!(
								"model-form field `{}` is not permitted by its policy",
								field,
							),
							::core::iter::empty::<(&str, &str)>(),
						);
						self.loading.set(false);
						self.error.set(::core::option::Option::Some(error.to_string()));
						self.set_server_error(::core::option::Option::Some(error.clone()));
						return ::core::result::Result::Err(error);
					}
				}
			}
		}
		TypedModelFieldSelection::Exclude(_) => quote! {},
	};
	let model_form_field_accessors = match &model_source.selection {
		TypedModelFieldSelection::Fields(fields) => fields
			.iter()
			.map(|field| {
				let method = format_ident!("{}_field", field);
				let name = field.to_string();
				let name = name.strip_prefix("r#").unwrap_or(&name);
				quote! {
					pub fn #method(&self) -> __ReinhardtModelFormField {
						__ReinhardtModelFormField(#name)
					}
				}
			})
			.collect::<Vec<_>>(),
		TypedModelFieldSelection::Exclude(_) => Vec::new(),
	};
	let (model_form_runtime_fields, model_form_runtime_fields_ref) = match &model_source.selection {
		TypedModelFieldSelection::Fields(fields) => {
			let names = fields.iter().map(|field| {
				let name = field.to_string();
				let name = name.strip_prefix("r#").unwrap_or(&name);
				quote!(__ReinhardtModelFormField(#name))
			});
			(
				quote! {
					const __REINHARDT_MODEL_FORM_FIELDS: &'static [__ReinhardtModelFormField] = &[
						#(#names),*
					];
				},
				quote!(__REINHARDT_MODEL_FORM_FIELDS),
			)
		}
		TypedModelFieldSelection::Exclude(_) => (
			quote! {
				static __REINHARDT_MODEL_FORM_FIELDS: ::std::sync::OnceLock<
					::std::boxed::Box<[__ReinhardtModelFormField]>
				> = ::std::sync::OnceLock::new();
			},
			quote! {
				__REINHARDT_MODEL_FORM_FIELDS.get_or_init(|| {
					<#schema_path as #pages_crate::form::ModelFormSchema>::fields()
						.iter()
						.filter(|descriptor| {
							descriptor.editable
								&& <#policy_ident as #pages_crate::form::ModelFormPolicy>::allows(
									descriptor.name,
								)
						})
						.map(|descriptor| __ReinhardtModelFormField(descriptor.name))
						.collect::<::std::vec::Vec<_>>()
						.into_boxed_slice()
				})
			},
		),
	};

	let override_arms = model_source.overrides.iter().map(|override_| {
		let name = override_.field.to_string();
		let widget = override_
			.widget
			.as_ref()
			.map(widget_to_string)
			.map(|widget| quote!(::core::option::Option::Some(#widget)))
			.unwrap_or_else(|| quote!(::core::option::Option::None));
		let label = override_
			.label
			.as_deref()
			.map(|label| quote!(::core::option::Option::Some(#label)))
			.unwrap_or_else(|| quote!(::core::option::Option::None));
		let help_text = override_
			.help_text
			.as_deref()
			.map(|help_text| quote!(::core::option::Option::Some(#help_text)))
			.unwrap_or_else(|| quote!(::core::option::Option::None));
		quote! {
			#name => (#widget, #label, #help_text),
		}
	});
	let range_override_names: Vec<String> = model_source
		.overrides
		.iter()
		.filter(|override_| matches!(override_.widget, Some(TypedWidget::RangeInput)))
		.map(|override_| override_.field.to_string())
		.collect();
	let is_range_override = if range_override_names.is_empty() {
		quote!(false)
	} else {
		quote!(::core::matches!(descriptor.name, #(#range_override_names)|*))
	};
	let color_override_names: Vec<String> = model_source
		.overrides
		.iter()
		.filter(|override_| matches!(override_.widget, Some(TypedWidget::ColorInput)))
		.map(|override_| override_.field.to_string())
		.collect();
	let is_color_override = if color_override_names.is_empty() {
		quote!(false)
	} else {
		quote!(::core::matches!(descriptor.name, #(#color_override_names)|*))
	};

	let form_id = form_id_kebab_case(form_ident);
	let form_class_attribute = macro_ast
		.styling
		.class
		.as_deref()
		.map(|class| quote! { .attr("class", #class) })
		.unwrap_or_default();
	let native_action = quote!(
		<#server_fn::marker as #pages_crate::server_fn::ServerFnMetadata>::PATH
	);
	// Server-function endpoints are registered exclusively for POST.
	let method = "post";
	let model_file_input_listener = generate_model_file_input_listener(pages_crate);

	quote! {
		{
			#use_statement

			struct #policy_ident;

			impl #pages_crate::form::ModelFormPolicy for #policy_ident {
				fn allows(field: &str) -> bool {
					#policy_body
				}
			}

			pub type #data_ident = #payload_path<#policy_path>;

			#model_form_selection_impl
			#model_form_selection_check

			#[derive(Clone, PartialEq)]
			struct __ReinhardtModelFormValues(
				::std::collections::HashMap<::std::string::String, #pages_crate::__private::serde_json::Value>
			);

			#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
			struct __ReinhardtModelFormField(&'static str);

			#model_form_runtime_fields

			#(#descriptor_guards)*

			#[derive(Clone)]
			struct #form_ident {
				__model_state: ::std::rc::Rc<
					::std::cell::RefCell<
						#pages_crate::form::ModelFormState<#schema_path, #policy_ident>
					>
				>,
				__form_id: ::std::string::String,
				__state_version: #pages_crate::Signal<u64>,
				__server_error: #pages_crate::Signal<
					::core::option::Option<#pages_crate::ServerFnError>
				>,
				__server_error_handlers: ::std::rc::Rc<
					::std::cell::RefCell<
						::std::vec::Vec<
							::std::rc::Weak<
								dyn Fn(::core::option::Option<#pages_crate::ServerFnError>)
							>
						>
					>
				>,
				loading: #pages_crate::Signal<bool>,
				error: #pages_crate::Signal<::core::option::Option<::std::string::String>>,
				success: #pages_crate::Signal<bool>,
			}

			impl #form_ident {
				#(#model_form_field_accessors)*

				fn new() -> Self {
					let __model_state = ::std::rc::Rc::new(
						::std::cell::RefCell::new(
							#pages_crate::form::ModelFormState::new()
						),
					);
					let __form_id = #pages_crate::reactive::hooks::id::use_id_with_prefix(#form_id);
					Self {
						__model_state,
						__form_id,
						__state_version: #pages_crate::Signal::new(0),
						__server_error: #pages_crate::Signal::new(::core::option::Option::None),
						__server_error_handlers: ::std::rc::Rc::new(
							::std::cell::RefCell::new(::std::vec::Vec::new()),
						),
						loading: #pages_crate::Signal::new(false),
						error: #pages_crate::Signal::new(::core::option::Option::None),
						success: #pages_crate::Signal::new(false),
					}
				}

				pub fn set_value(
					&self,
					field: &str,
					value: #pages_crate::__private::serde_json::Value,
				) -> ::core::result::Result<
					(),
					#pages_crate::form::ModelFormPayloadError,
				> {
					let mut state = self.__model_state.borrow_mut();
					let previous = state.value(field).cloned();
					let result = state.set_value(field, value);
					let changed = previous != state.value(field).cloned();
					drop(state);
					if changed {
						self.__state_version.update(|version| *version = version.wrapping_add(1));
					}
					result
				}

				pub fn value(
					&self,
					field: &str,
				) -> ::core::option::Option<
					#pages_crate::__private::serde_json::Value
				> {
					self.__model_state.borrow().value(field).cloned()
				}

				pub fn field(
					&self,
					name: &str,
				) -> ::core::option::Option<__ReinhardtModelFormField> {
					<#form_ident as #pages_crate::FormRuntimeSource>::runtime_field_by_name(self, name)
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				pub fn set_file(
					&self,
					field: &str,
					file: #pages_crate::__private::web_sys::File,
				) -> ::core::result::Result<
					(),
					#pages_crate::form::ModelFormPayloadError,
				> {
					let result = self.__model_state.borrow_mut().set_file(field, file);
					if result.is_ok() {
						self.__state_version.update(|version| *version = version.wrapping_add(1));
					}
					result
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				fn clear_file(
					&self,
					field: &str,
				) -> ::core::result::Result<
					(),
					#pages_crate::form::ModelFormPayloadError,
				> {
					let result = self.__model_state.borrow_mut().clear_file(field);
					if result.is_ok() {
						self.__state_version.update(|version| *version = version.wrapping_add(1));
					}
					result
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				fn clear_selected_files(&self) {
					let changed = self.__model_state.borrow_mut().clear_selected_files();
					if changed {
						self.__state_version.update(|version| *version = version.wrapping_add(1));
					}
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				fn clear_selected_files_matching(
					&self,
					submitted: &#pages_crate::form::ModelFormState<#schema_path, #policy_ident>,
				) {
					let changed = self
						.__model_state
						.borrow_mut()
						.clear_selected_files_matching(submitted);
					if changed {
						self.__state_version.update(|version| *version = version.wrapping_add(1));
					}
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				fn sync_mounted_field(
					&self,
					field: &str,
					value: ::core::option::Option<
						&#pages_crate::__private::serde_json::Value,
					>,
				) {
					use #pages_crate::__private::wasm_bindgen::JsCast;
					let Some(form) = #pages_crate::__private::web_sys::window()
						.and_then(|window| window.document())
						.and_then(|document| document.get_element_by_id(&self.__form_id))
						.and_then(|form| form.dyn_into::<#pages_crate::__private::web_sys::HtmlFormElement>().ok())
					else {
						return;
					};
					let Ok(elements) = form.query_selector_all("[name]") else {
						return;
					};
					let text = value
						.map(|value| match value {
							#pages_crate::__private::serde_json::Value::Null => ::std::string::String::new(),
							#pages_crate::__private::serde_json::Value::String(value) => value.clone(),
							value => value.to_string(),
						})
						.unwrap_or_default();
					for index in 0..elements.length() {
						let Some(element) = elements.item(index) else {
							continue;
						};
						let Some(element) = element.dyn_ref::<#pages_crate::__private::web_sys::Element>() else {
							continue;
						};
						if element.get_attribute("name").as_deref() != ::core::option::Option::Some(field) {
							continue;
						}
						if let Some(input) = element.dyn_ref::<#pages_crate::__private::web_sys::HtmlInputElement>() {
							match input.type_().as_str() {
								"checkbox" => input.set_checked(value.and_then(|value| value.as_bool()).unwrap_or(false)),
								"file" => {
									if value.is_none() || value.is_some_and(#pages_crate::__private::serde_json::Value::is_null) {
										input.set_value("");
									}
								}
								_ => input.set_value(&text),
							}
						} else if let Some(select) = element.dyn_ref::<#pages_crate::__private::web_sys::HtmlSelectElement>() {
							select.set_value(&text);
						} else if let Some(textarea) = element.dyn_ref::<#pages_crate::__private::web_sys::HtmlTextAreaElement>() {
							textarea.set_value(&text);
						}
					}
				}

				pub fn data(
					&self,
				) -> ::core::result::Result<
					#data_ident,
					#pages_crate::form::ModelFormPayloadError,
				> {
					self.__model_state.borrow().build_payload_for::<
						#data_ident,
						#policy_path,
					>()
				}

				pub fn loading(&self) -> &#pages_crate::Signal<bool> {
					&self.loading
				}

				pub fn error(
					&self,
				) -> &#pages_crate::Signal<
					::core::option::Option<::std::string::String>
				> {
					&self.error
				}

				pub fn success(&self) -> &#pages_crate::Signal<bool> {
					&self.success
				}

				fn set_server_error(
					&self,
					error: ::core::option::Option<#pages_crate::ServerFnError>,
				) {
					self.__server_error.set(error.clone());
					let mut active_handlers = ::std::vec::Vec::new();
					self.__server_error_handlers.borrow_mut().retain(|handler| {
						if let ::core::option::Option::Some(handler) = handler.upgrade() {
							active_handlers.push(handler);
							true
						} else {
							false
						}
					});
					for handler in active_handlers {
						handler(error.clone());
					}
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				pub async fn submit_response(
					&self,
				) -> ::core::result::Result<#model_form_response_type, #pages_crate::ServerFnError> {
					let state = self.__model_state.borrow().clone();
					self.submit_state(state, ::core::option::Option::None).await
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				pub async fn submit(
					&self,
				) -> ::core::result::Result<(), #pages_crate::ServerFnError> {
					self.submit_response().await.map(|_| ())
				}

				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				pub async fn submit(
					&self,
				) -> ::core::result::Result<(), #pages_crate::ServerFnError> {
					self.set_server_error(::core::option::Option::None);
					let state = self.__model_state.borrow().clone();
					if let ::core::result::Result::Err(errors) = state.build_validated_payload_for::<
						#payload_path<#policy_ident>,
						#policy_ident,
					>() {
						let error = #pages_crate::ServerFnError::from(errors);
						self.error.set(::core::option::Option::Some(error.to_string()));
						self.set_server_error(::core::option::Option::Some(error.clone()));
						return ::core::result::Result::Err(error);
					}
					self.error.set(::core::option::Option::None);
					::core::result::Result::Ok(())
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				fn clear_mounted_file_inputs_matching(
					&self,
					submitted: &#pages_crate::form::ModelFormState<#schema_path, #policy_ident>,
					form: ::core::option::Option<
						&#pages_crate::__private::web_sys::HtmlFormElement,
					>,
				) {
					use #pages_crate::__private::wasm_bindgen::JsCast;
					let inputs = form
						.and_then(|form| form.query_selector_all("input[type=\"file\"]").ok())
						.or_else(|| {
							#pages_crate::__private::web_sys::window()
								.and_then(|window| window.document())
								.and_then(|document| document.get_element_by_id(&self.__form_id))
								.and_then(|form| {
									form.dyn_into::<#pages_crate::__private::web_sys::HtmlFormElement>()
										.ok()
								})
								.and_then(|form| form.query_selector_all("input[type=\"file\"]").ok())
						});
					let Some(inputs) = inputs else {
						return;
					};
					for index in 0..inputs.length() {
						if let Some(input) = inputs.item(index)
							&& let Ok(input) = input
								.dyn_into::<#pages_crate::__private::web_sys::HtmlInputElement>()
						{
							let field = input.name();
							let matches_snapshot = submitted.file(&field).is_some_and(|submitted_file| {
								input
									.files()
									.and_then(|files| files.item(0))
									.is_some_and(|current_file| {
										let submitted_file = #pages_crate::__private::wasm_bindgen::JsValue::from(
											submitted_file.clone(),
										);
										let current_file = #pages_crate::__private::wasm_bindgen::JsValue::from(
											current_file.clone(),
										);
										submitted_file == current_file
									})
								});
							if matches_snapshot {
								input.set_value("");
							}
						}
					}
				}

				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				async fn submit_state(
					&self,
					state: #pages_crate::form::ModelFormState<#schema_path, #policy_ident>,
					form: ::core::option::Option<
						&#pages_crate::__private::web_sys::HtmlFormElement,
					>,
				) -> ::core::result::Result<#model_form_response_type, #pages_crate::ServerFnError> {
					self.loading.set(true);
					self.error.set(::core::option::Option::None);
					self.set_server_error(::core::option::Option::None);
					self.success.set(false);
					#model_form_policy_check
					let normalized_payload = match state.build_validated_payload_for::<
						#payload_path<#policy_ident>,
						#policy_ident,
					>() {
						::core::result::Result::Ok(payload) => payload,
						::core::result::Result::Err(errors) => {
							let error = #pages_crate::ServerFnError::from(errors);
							self.loading.set(false);
							self.error.set(::core::option::Option::Some(error.to_string()));
							self.set_server_error(::core::option::Option::Some(error.clone()));
							return ::core::result::Result::Err(error);
						}
					};
					let mut state = state;
					for descriptor in state.selected_descriptors() {
						if ::core::matches!(
							descriptor.kind,
							#pages_crate::form::ModelFormFieldKind::File
								| #pages_crate::form::ModelFormFieldKind::Image
						) {
							continue;
						}
						match <#payload_path<#policy_ident> as #pages_crate::form::ModelFormPayload<#policy_ident>>::get_json(
							&normalized_payload,
							descriptor.name,
						) {
							::core::option::Option::Some(value) => state
								.set_value(descriptor.name, value)
								.expect("validated model-form payload must rehydrate its selected field"),
							::core::option::Option::None => state
								.clear_value(descriptor.name)
								.expect("validated model-form payload must clear its selected field"),
						}
					}
					let result = <#server_fn::marker as #pages_crate::form::ModelFormServerFn<
						#model_form_selection_type,
						#schema_path,
						#policy_ident,
					>>::submit(&state)
						.await
						.map_err(|error| -> #pages_crate::ServerFnError {
							::core::convert::Into::into(error)
						});
					self.loading.set(false);
					match result {
						::core::result::Result::Ok(response) => {
							self.clear_selected_files_matching(&state);
							self.clear_mounted_file_inputs_matching(&state, form);
							self.success.set(true);
							::core::result::Result::Ok(response)
						}
						::core::result::Result::Err(error) => {
							self.error.set(::core::option::Option::Some(error.to_string()));
							self.set_server_error(::core::option::Option::Some(error.clone()));
							::core::result::Result::Err(error)
						}
					}
				}

				pub fn into_page(self) -> #pages_crate::Page {
					let _ = self.__state_version.get();
					let mut controls = ::std::vec::Vec::new();
					for descriptor in self.__model_state.borrow().selected_descriptors() {
						if !<#policy_ident as #pages_crate::form::ModelFormPolicy>::allows(descriptor.name) {
							continue;
						}
						let (
							widget_override,
							label_override,
							help_text,
						): (
							::core::option::Option<&'static str>,
							::core::option::Option<&'static str>,
							::core::option::Option<&'static str>,
						) =
							match descriptor.name {
								#(#override_arms)*
								_ => (
									::core::option::Option::None,
									::core::option::Option::None,
									::core::option::Option::None,
								),
							};
						let label = label_override.unwrap_or(descriptor.name);
						let widget_override = match widget_override {
							::core::option::Option::Some(widget) => match (widget, descriptor.kind) {
								("CheckboxInput", #pages_crate::form::ModelFormFieldKind::Boolean)
								| ("DateInput", #pages_crate::form::ModelFormFieldKind::Date)
								| ("TimeInput", #pages_crate::form::ModelFormFieldKind::Time)
								| ("DateTimeInput", #pages_crate::form::ModelFormFieldKind::DateTime | #pages_crate::form::ModelFormFieldKind::NaiveDateTime)
								| ("EmailInput", #pages_crate::form::ModelFormFieldKind::Email { .. })
								| ("UrlInput", #pages_crate::form::ModelFormFieldKind::Url { .. })
								| ("NumberInput" | "RangeInput", #pages_crate::form::ModelFormFieldKind::Integer { .. } | #pages_crate::form::ModelFormFieldKind::Float { .. } | #pages_crate::form::ModelFormFieldKind::Decimal { .. })
								| ("TextInput", #pages_crate::form::ModelFormFieldKind::Text { .. })
								| ("Textarea" | "TextArea", #pages_crate::form::ModelFormFieldKind::Text { .. } | #pages_crate::form::ModelFormFieldKind::Json)
								| ("PasswordInput" | "HiddenInput" | "ColorInput" | "TelInput" | "SearchInput", #pages_crate::form::ModelFormFieldKind::Text { .. }) => ::core::option::Option::Some(widget),
								_ => {
									#pages_crate::warn_log!(
										"model form widget override `{}` is incompatible with `{}`; using the generated default",
										widget,
										descriptor.name,
									);
									::core::option::Option::None
								}
							},
							::core::option::Option::None => ::core::option::Option::None,
						};
						let (tag, input_type) = match widget_override {
							::core::option::Option::Some("Textarea" | "TextArea") =>
								("textarea", "text"),
							::core::option::Option::Some("PasswordInput") =>
								("input", "password"),
							::core::option::Option::Some("EmailInput") =>
								("input", "email"),
							::core::option::Option::Some("NumberInput") =>
								("input", "number"),
							::core::option::Option::Some("CheckboxInput") =>
								("input", "checkbox"),
							::core::option::Option::Some("DateInput") =>
								("input", "date"),
							::core::option::Option::Some("TimeInput") =>
								("input", "time"),
							::core::option::Option::Some("DateTimeInput") =>
								("input", "datetime-local"),
							::core::option::Option::Some("UrlInput") =>
								("input", "url"),
							::core::option::Option::Some("HiddenInput") =>
								("input", "hidden"),
							::core::option::Option::Some("ColorInput") =>
								("input", "color"),
							::core::option::Option::Some("RangeInput") =>
								("input", "range"),
							::core::option::Option::Some("TelInput") =>
								("input", "tel"),
							::core::option::Option::Some("SearchInput") =>
								("input", "search"),
							::core::option::Option::Some(_) => ("input", "text"),
							::core::option::Option::None => match descriptor.kind {
								#pages_crate::form::ModelFormFieldKind::Text {
									multiline: true,
									..
								}
								| #pages_crate::form::ModelFormFieldKind::Json =>
									("textarea", "text"),
								#pages_crate::form::ModelFormFieldKind::Email { .. } =>
									("input", "email"),
								#pages_crate::form::ModelFormFieldKind::Url { .. } =>
									("input", "url"),
								#pages_crate::form::ModelFormFieldKind::Integer { .. }
								| #pages_crate::form::ModelFormFieldKind::Float { .. }
								| #pages_crate::form::ModelFormFieldKind::Decimal { .. } =>
									("input", "number"),
								#pages_crate::form::ModelFormFieldKind::Boolean =>
									("input", "checkbox"),
								#pages_crate::form::ModelFormFieldKind::Date =>
									("input", "date"),
								#pages_crate::form::ModelFormFieldKind::Time =>
									("input", "time"),
								#pages_crate::form::ModelFormFieldKind::DateTime
								| #pages_crate::form::ModelFormFieldKind::NaiveDateTime =>
									("input", "datetime-local"),
								#pages_crate::form::ModelFormFieldKind::Text { .. }
								| #pages_crate::form::ModelFormFieldKind::Uuid =>
									("input", "text"),
								#pages_crate::form::ModelFormFieldKind::File
								| #pages_crate::form::ModelFormFieldKind::Image =>
									("input", "file"),
							},
						};
						let field_name = descriptor.name;
						let stored_value = self.__model_state.borrow().value(field_name).cloned();
						let default_true = matches!(
							descriptor.kind,
							#pages_crate::form::ModelFormFieldKind::Boolean
						) && <#schema_path as #pages_crate::form::ModelFormSchema>::default_boolean_is_true(field_name);
						let uses_nullable_boolean_select = input_type == "checkbox"
							&& descriptor.nullable
							&& !default_true;
						let (tag, input_type) = if uses_nullable_boolean_select {
							("select", "select")
						} else {
							(tag, input_type)
						};
						let is_checkbox = input_type == "checkbox";
						let permits_subminute_precision = matches!(
							descriptor.kind,
							#pages_crate::form::ModelFormFieldKind::Float { .. }
								| #pages_crate::form::ModelFormFieldKind::Decimal { .. }
								| #pages_crate::form::ModelFormFieldKind::Time
								| #pages_crate::form::ModelFormFieldKind::DateTime
								| #pages_crate::form::ModelFormFieldKind::NaiveDateTime
						);

						let checkbox_sentinel = format!("__reinhardt_checkbox_{field_name}");
						let color_sentinel = format!("__reinhardt_color_{field_name}");
						let range_sentinel = format!("__reinhardt_range_{field_name}");
						let default_clear_sentinel = format!("__reinhardt_defaulted_{field_name}");
						let control_id = format!("{}-{}", #form_id, field_name);
						let range_default = if input_type == "range" {
							match descriptor.kind {
								#pages_crate::form::ModelFormFieldKind::Integer { min, max } => {
									let min = min.unwrap_or(0);
									let max = max.unwrap_or(100);
									let default = if max < min {
										min
									} else {
										((::core::primitive::i128::from(min)
											+ ::core::primitive::i128::from(max)
											+ 1)
											.div_euclid(2)) as i64
									};
									::core::option::Option::Some(default.to_string())
								}
								#pages_crate::form::ModelFormFieldKind::Float { min, max } => {
									let min = min.unwrap_or(0.0);
									let max = max.unwrap_or(100.0);
									::core::option::Option::Some(if max < min { min } else { min + (max - min) / 2.0 }.to_string())
								}
								#pages_crate::form::ModelFormFieldKind::Decimal { min, max } => {
									let min = min.unwrap_or("0").parse::<f64>().unwrap_or(0.0);
									let max = max.unwrap_or("100").parse::<f64>().unwrap_or(100.0);
									::core::option::Option::Some(if max < min { min } else { min + (max - min) / 2.0 }.to_string())
								}
								_ => ::core::option::Option::None,
							}
						} else {
							::core::option::Option::None
						};
						let mut control = #pages_crate::PageElement::new(tag)
							.attr("name", field_name)
							.attr("id", control_id.clone())
							.bool_attr("required", descriptor.required && !is_checkbox);
						if tag == "input" {
							control = control.attr("type", input_type);
						}
						let color_is_unset = input_type == "color" && stored_value.is_none();
						if uses_nullable_boolean_select {
							control = control
								.child(
									#pages_crate::PageElement::new("option")
										.attr("value", "")
										.bool_attr("selected", stored_value.is_none())
										.child("Unset")
								)
								.child(
									#pages_crate::PageElement::new("option")
										.attr("value", "true")
										.bool_attr(
											"selected",
											matches!(
												stored_value.as_ref(),
												::core::option::Option::Some(
													#pages_crate::__private::serde_json::Value::Bool(true)
												)
											),
										)
										.child("True")
								)
								.child(
									#pages_crate::PageElement::new("option")
										.attr("value", "false")
										.bool_attr(
											"selected",
											matches!(
												stored_value.as_ref(),
												::core::option::Option::Some(
													#pages_crate::__private::serde_json::Value::Bool(false)
												)
											),
										)
										.child("False")
								);
						} else if is_checkbox {
							control = control.bool_attr(
								"checked",
								matches!(
									stored_value.as_ref(),
									::core::option::Option::Some(
										#pages_crate::__private::serde_json::Value::Bool(true)
									)
								) || (stored_value.is_none() && default_true),
							);
						} else if input_type != "password"
							&& let ::core::option::Option::Some(value) = stored_value.as_ref()
						{
							let value = if matches!(
								descriptor.kind,
								#pages_crate::form::ModelFormFieldKind::Json
							) {
								#pages_crate::__private::serde_json::to_string(&value).unwrap_or_default()
							} else {
								match value {
									#pages_crate::__private::serde_json::Value::Null => ::std::string::String::new(),
									#pages_crate::__private::serde_json::Value::String(value) => value.clone(),
									value => value.to_string(),
								}
							};
							let value = if input_type == "datetime-local"
								&& matches!(
									descriptor.kind,
									#pages_crate::form::ModelFormFieldKind::DateTime
								)
							{
								value.strip_suffix('Z').unwrap_or(&value).to_owned()
							} else {
								value
							};
							if tag == "textarea" {
								control = control.child(value);
							} else {
								control = control.attr("value", value);
							}
						}
						if permits_subminute_precision {
							control = control.attr("step", "any");
						}
						if input_type == "range" {
							match descriptor.kind {
								#pages_crate::form::ModelFormFieldKind::Integer { min, max } => {
									if let ::core::option::Option::Some(min) = min {
										control = control.attr("min", min.to_string());
									}
									if let ::core::option::Option::Some(max) = max {
										control = control.attr("max", max.to_string());
									}
								}
								#pages_crate::form::ModelFormFieldKind::Float { min, max } => {
									if let ::core::option::Option::Some(min) = min {
										control = control.attr("min", min.to_string());
									}
									if let ::core::option::Option::Some(max) = max {
										control = control.attr("max", max.to_string());
									}
								}
								#pages_crate::form::ModelFormFieldKind::Decimal { min, max } => {
									if let ::core::option::Option::Some(min) = min {
										control = control.attr("min", min);
									}
									if let ::core::option::Option::Some(max) = max {
										control = control.attr("max", max);
									}
								}
								_ => {}
							}
						}
						if input_type == "color" {
							let color_edit_script = format!(
								"this.form.elements['{color_sentinel}'].value='true'"
							);
							control = control.attr("oninput", color_edit_script);
						}
						if matches!(
							descriptor.kind,
							#pages_crate::form::ModelFormFieldKind::Image
						) {
							control = control.attr("accept", "image/*");
						}
						if matches!(
							descriptor.kind,
							#pages_crate::form::ModelFormFieldKind::File
								| #pages_crate::form::ModelFormFieldKind::Image
						) {
							control = control #model_file_input_listener;
						} else if is_checkbox {
							control = control.on(
								#pages_crate::event::KnownEvent::Change,
								{
									let form = self.clone();
									#pages_crate::typed_event_handler::<
										#pages_crate::event::ChangeEvent,
										_,
									>(move |event: #pages_crate::event::ChangeEvent| {
										if let ::core::result::Result::Ok(value) = event.checked() {
											if let ::core::result::Result::Err(error) =
												form.set_value(
													field_name,
													#pages_crate::__private::serde_json::Value::Bool(value),
												)
											{
												#pages_crate::warn_log!(
													"model form field `{}` rejected input: {}",
													field_name,
													error,
												);
											}
										}
									})
								},
							);
						} else if uses_nullable_boolean_select {
							control = control.on(
								#pages_crate::event::KnownEvent::Change,
								{
									let form = self.clone();
									#pages_crate::typed_event_handler::<
										#pages_crate::event::ChangeEvent,
										_,
									>(move |event: #pages_crate::event::ChangeEvent| {
										if let ::core::result::Result::Ok(value) = event.value() {
											if let ::core::result::Result::Err(error) =
												form.set_value(
													field_name,
													#pages_crate::__private::serde_json::Value::String(value),
												)
											{
												#pages_crate::warn_log!(
													"model form field `{}` rejected input: {}",
													field_name,
													error,
												);
											}
										}
									})
								},
							);
						} else {
							control = control.on(
								#pages_crate::event::KnownEvent::Input,
								{
									let form = self.clone();
									#pages_crate::typed_event_handler::<
										#pages_crate::event::InputEvent,
										_,
									>(move |event: #pages_crate::event::InputEvent| {
										if let ::core::result::Result::Ok(value) = event.value() {
											if let ::core::result::Result::Err(error) =
												form.set_value(
													field_name,
													#pages_crate::__private::serde_json::Value::String(value),
												)
											{
												#pages_crate::warn_log!(
													"model form field `{}` rejected input: {}",
													field_name,
													error,
												);
											}
										}
									})
								},
							);
						}
						let checkbox_sentinel = if uses_nullable_boolean_select {
							::core::option::Option::Some(
								#pages_crate::PageElement::new("input")
									.attr("type", "hidden")
									.attr("name", checkbox_sentinel)
									.attr("value", "unset")
							)
						} else {
							is_checkbox.then(|| {
								#pages_crate::PageElement::new("input")
									.attr("type", "hidden")
									.attr("name", checkbox_sentinel)
									.attr("value", "false")
							})
						};
						let color_sentinel = (input_type == "color").then(|| {
							#pages_crate::PageElement::new("input")
								.attr("type", "hidden")
								.attr("name", color_sentinel)
								.attr("value", if color_is_unset { "false" } else { "true" })
						});
						let range_sentinel = (input_type == "range"
							&& !descriptor.required
							&& stored_value.is_none())
							.then(|| {
								#pages_crate::PageElement::new("input")
									.attr("type", "hidden")
									.attr("name", range_sentinel)
									.attr("value", range_default.clone().unwrap_or_default())
							});
						let default_clear_control_id = format!("{control_id}-clear");
						let can_clear_default = descriptor.nullable
							&& descriptor.has_default
							&& !matches!(
								descriptor.kind,
								#pages_crate::form::ModelFormFieldKind::File
									| #pages_crate::form::ModelFormFieldKind::Image
							);
						let default_clear_sentinel = can_clear_default
							.then(|| {
								#pages_crate::PageElement::new("input")
									.attr("type", "checkbox")
									.attr("id", default_clear_control_id.clone())
									.attr("name", default_clear_sentinel)
									.attr("value", "true")
									.bool_attr(
										"checked",
										matches!(
											stored_value,
											::core::option::Option::Some(
												#pages_crate::__private::serde_json::Value::Null
											)
										),
									)
							});
						let default_clear_label = can_clear_default
							.then(|| {
								#pages_crate::PageElement::new("label")
									.attr("for", default_clear_control_id)
									.child("Clear value")
							});

						let mut wrapper = #pages_crate::PageElement::new("div")
							.attr("class", "reinhardt-form-field")
							.children(checkbox_sentinel)
							.children(color_sentinel)
							.children(range_sentinel)
							.children(default_clear_sentinel)
							.children(default_clear_label)
							.child(
								#pages_crate::PageElement::new("label")
									.attr("for", control_id)
									.child(label.to_owned())
							)
							.child(control);
						if let ::core::option::Option::Some(help_text) = help_text {
							wrapper = wrapper.child(
								#pages_crate::PageElement::new("small")
									.attr("class", "reinhardt-form-help")
									.child(help_text.to_owned())
							);
						}
						controls.push(wrapper);
					}

					let submit_form = self.clone();
					let reset_form = self.clone();
						#pages_crate::IntoPage::into_page(
							{
								let mut form = #pages_crate::PageElement::new("form")
									.attr("id", self.__form_id.clone())
								#form_class_attribute
								.attr("method", #method);
							let has_file_fields = self.__model_state.borrow().selected_descriptors().iter().any(|descriptor| {
								matches!(
									descriptor.kind,
									#pages_crate::form::ModelFormFieldKind::File
										| #pages_crate::form::ModelFormFieldKind::Image
								)
							});
							if has_file_fields {
								form = form.attr("enctype", "multipart/form-data");
							} else {
								form = form.attr("action", #native_action);
							}
							form
						.children(controls)
						.child({
							let csrf_token = #pages_crate::csrf::get_csrf_token().unwrap_or_default();
							#pages_crate::PageElement::new("input")
								.attr("type", "hidden")
								.attr("name", #pages_crate::csrf::CSRF_FORM_FIELD)
								.attr("value", csrf_token)
						})
						.child(
							#pages_crate::PageElement::new("button")
								.attr("type", "submit")
								.child("Submit")
						)
						.on(
							#pages_crate::event::KnownEvent::Submit,
							#pages_crate::typed_event_handler::<
								#pages_crate::event::SubmitEvent,
								_,
							>(move |event: #pages_crate::event::SubmitEvent| {
								// This is mutated only by the WASM submit snapshot below.
								#[allow(unused_mut)]
								let mut snapshot_valid = true;
								#[cfg(all(target_family = "wasm", target_os = "unknown"))]
								let submit_target = {
									use #pages_crate::__private::wasm_bindgen::JsCast;
									event.raw().current_target().and_then(|target| {
										target
											.dyn_into::<#pages_crate::__private::web_sys::HtmlFormElement>()
											.ok()
									})
								};
								#[cfg(all(target_family = "wasm", target_os = "unknown"))]
								{
									if let Some(form) = submit_target.as_ref()
										&& let Ok(values) = #pages_crate::__private::web_sys::FormData::new_with_form(form)
									{
										let fields = submit_form
											.__model_state
											.borrow()
											.selected_descriptors()
											.iter()
											.filter(|descriptor| {
											<#policy_ident as #pages_crate::form::ModelFormPolicy>::allows(
													descriptor.name,
												)
											})
											.map(|descriptor| (
												descriptor.name,
												matches!(
													descriptor.kind,
													#pages_crate::form::ModelFormFieldKind::Boolean
												) && !(descriptor.nullable
													&& !<#schema_path as #pages_crate::form::ModelFormSchema>::default_boolean_is_true(
														descriptor.name,
													)),
													descriptor.nullable,
													descriptor.required,
													descriptor.has_default,
													#is_range_override,
													#is_color_override,
											))
											.collect::<::std::vec::Vec<_>>();
										let mut state = submit_form.__model_state.borrow_mut();
										for (field, is_checkbox, nullable, required, has_default, is_range, is_color) in fields {
											let default_clear_sentinel = format!("__reinhardt_defaulted_{field}");
											let clears_default = nullable
												&& has_default
												&& values
													.get(&default_clear_sentinel)
													.as_string()
													.as_deref()
													== ::core::option::Option::Some("true");
											if clears_default {
												let _ = state.set_value(
													field,
													#pages_crate::__private::serde_json::Value::Null,
												);
												continue;
											}
											if (is_range || is_color) && !required && state.value(field).is_none() {
												continue;
											}
											if let Some(value) = values.get(field).as_string() {
												let value = if is_checkbox {
													#pages_crate::__private::serde_json::Value::Bool(true)
												} else {
													#pages_crate::__private::serde_json::Value::String(value)
												};
												if let ::core::result::Result::Err(error) = state.set_value(
													field,
													value,
												) {
													snapshot_valid = false;
													submit_form.error.set(::core::option::Option::Some(error.to_string()));
												}
											} else if is_checkbox
												&& !nullable
												&& !(has_default && state.value(field).is_none())
											{
												let _ = state.set_value(field, #pages_crate::__private::serde_json::Value::Bool(false));
											}
										}
									}
								}
								let submitted_state = submit_form.__model_state.borrow().clone();
								event.prevent_default();
								if !snapshot_valid {
									return;
								}
								#[cfg(all(target_family = "wasm", target_os = "unknown"))]
								{
									let form = submit_form.clone();
									#pages_crate::platform::spawn_task(async move {
										let _ = form
											.submit_state(submitted_state, submit_target.as_ref())
											.await;
									});
								}
								#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
								{
									let _ = &submit_form;
								}
							}),
						)
						.on(
							#pages_crate::event::KnownEvent::Reset,
							#pages_crate::typed_event_handler::<
								#pages_crate::event::ResetEvent,
								_,
							>(move |_event: #pages_crate::event::ResetEvent| {
								#[cfg(all(target_family = "wasm", target_os = "unknown"))]
								reset_form.clear_selected_files();
								#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
								let _ = &reset_form;
							}),
						)
						}
					)
				}
			}

			impl #pages_crate::FormRuntimeSource for #form_ident {
				type Values = __ReinhardtModelFormValues;
				type Field = __ReinhardtModelFormField;

				fn runtime_initial_values(&self) -> Self::Values {
					self.runtime_current_values()
				}

				fn runtime_field_by_name(&self, name: &str) -> ::core::option::Option<Self::Field> {
					<#schema_path as #pages_crate::form::ModelFormSchema>::fields()
						.iter()
						.find(|descriptor| {
							descriptor.editable
								&& descriptor.name == name
								&& <#policy_ident as #pages_crate::form::ModelFormPolicy>::allows(
									descriptor.name,
								)
						})
						.map(|descriptor| __ReinhardtModelFormField(descriptor.name))
				}

				fn runtime_current_values(&self) -> Self::Values {
					let _ = self.__state_version.get();
					let state = self.__model_state.borrow();
					let mut values =
						state
							.selected_descriptors()
							.iter()
							.filter_map(|descriptor| {
								if !<#policy_ident as #pages_crate::form::ModelFormPolicy>::allows(descriptor.name) {
									return ::core::option::Option::None;
								}
								state.value(descriptor.name).cloned().map(|value| {
									(descriptor.name.to_owned(), value)
								})
							})
							.collect::<::std::collections::HashMap<_, _>>();
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					for descriptor in state.selected_descriptors() {
						if let ::core::option::Option::Some(file) = state.file(descriptor.name) {
							values.insert(
								format!("__reinhardt_file_{}", descriptor.name),
								#pages_crate::__private::serde_json::Value::String(format!(
									"{}:{}:{}:{}",
									file.name(),
									file.size(),
									file.last_modified(),
									file.type_(),
								)),
							);
						}
					}
					__ReinhardtModelFormValues(values)
				}

				fn runtime_server_error(
					&self,
				) -> ::core::option::Option<#pages_crate::ServerFnError> {
					self.__server_error.get()
				}

				fn runtime_clear_server_error(&self) {
					self.set_server_error(::core::option::Option::None);
				}

				fn runtime_register_server_error_handler(
					&self,
					handler: ::std::rc::Weak<
						dyn Fn(::core::option::Option<#pages_crate::ServerFnError>)
					>,
				) {
					let mut handlers = self.__server_error_handlers.borrow_mut();
					handlers.retain(|handler| handler.upgrade().is_some());
					handlers.push(handler);
				}

				fn runtime_apply_values(&self, values: &Self::Values) {
					let mut state = self.__model_state.borrow_mut();
					state.clear_selected_values();
					for (field, value) in &values.0 {
						let _ = state.set_value(field, value.clone());
					}
					drop(state);
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					for descriptor in <#schema_path as #pages_crate::form::ModelFormSchema>::fields() {
						if !descriptor.editable
							|| !<#policy_ident as #pages_crate::form::ModelFormPolicy>::allows(descriptor.name)
						{
							continue;
						}
						let value = self.__model_state.borrow().value(descriptor.name).cloned();
						self.sync_mounted_field(descriptor.name, value.as_ref());
					}
					self.__state_version.update(|version| *version = version.wrapping_add(1));
				}

				fn runtime_set_field_value<T>(&self, field: Self::Field, value: T)
				where
					T: ::core::any::Any + 'static,
				{
					let mut state = self.__model_state.borrow_mut();
					let previous = state.value(field.0).cloned();
					let result = state.set_any_value(field.0, value);
					let changed = previous != state.value(field.0).cloned();
					let current = state.value(field.0).cloned();
					drop(state);
					if changed {
						self.__state_version.update(|version| *version = version.wrapping_add(1));
					}
					if let ::core::result::Result::Err(error) = result {
						panic!("model form field {:?} rejected value: {}", field.0, error);
					}
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					self.sync_mounted_field(field.0, current.as_ref());
				}

				fn runtime_values_are_dirty(
					&self,
					current: &Self::Values,
					defaults: &Self::Values,
				) -> bool {
					current != defaults
				}

				fn runtime_apply_field_value(&self, field: Self::Field, values: &Self::Values) {
					let mut state = self.__model_state.borrow_mut();
					if let ::core::option::Option::Some(value) = values.0.get(field.0) {
						let _ = state.set_value(field.0, value.clone());
					} else if values
						.0
						.get(&format!("__reinhardt_file_{}", field.0))
						.is_none()
					{
						let _ = state.clear_value(field.0);
					}
					let current = state.value(field.0).cloned();
					drop(state);
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					self.sync_mounted_field(field.0, current.as_ref());
					self.__state_version.update(|version| *version = version.wrapping_add(1));
				}

				fn runtime_field_is_dirty(
					&self,
					field: Self::Field,
					current: &Self::Values,
					defaults: &Self::Values,
				) -> bool {
					if field.0.is_empty() {
						return current != defaults;
					}
					if <#schema_path as #pages_crate::form::ModelFormSchema>::fields()
						.iter()
						.find(|descriptor| descriptor.name == field.0)
						.is_some_and(|descriptor| {
							matches!(
								descriptor.kind,
								#pages_crate::form::ModelFormFieldKind::File
									| #pages_crate::form::ModelFormFieldKind::Image
							)
						})
					{
						return current.0.get(&format!("__reinhardt_file_{}", field.0))
							!= defaults.0.get(&format!("__reinhardt_file_{}", field.0));
					}
					current.0.get(field.0) != defaults.0.get(field.0)
				}

				fn runtime_watch_field<T>(
					&self,
					field: Self::Field,
				) -> ::core::option::Option<#pages_crate::Signal<T>>
				where
					T: Clone + 'static,
				{
					let _ = field;
					::core::option::Option::None
				}

				fn runtime_fields(&self) -> &'static [Self::Field] {
					#model_form_runtime_fields_ref
				}
			}

			let __reinhardt_form = #form_ident::new();
			__reinhardt_form
		}
	}
}

/// Generates the complete code for a form! macro invocation.
///
/// This function generates conditional code that works for both WASM and server builds.
pub(super) fn generate(
	macro_ast: &TypedFormMacro,
	ambient_arguments_source: Option<AmbientArgumentsSource>,
) -> TokenStream {
	let crate_info = get_reinhardt_pages_crate_info();
	let use_statement = &crate_info.use_statement;
	let pages_crate = &crate_info.ident;
	if let Some(model_source) = &macro_ast.model_source {
		return generate_model_form(macro_ast, model_source, pages_crate, use_statement);
	}

	let struct_name = &macro_ast.name;
	let ambient_argument_deprecation_markers =
		generate_ambient_argument_deprecation_markers(macro_ast, ambient_arguments_source);
	let form_runtime_contract = generate_form_runtime_contract(macro_ast, pages_crate);
	let runtime_contract_supported = supports_form_runtime_contract(&macro_ast.fields);
	let values_ident = format_ident!("{}Values", macro_ast.name);
	let runtime_initial_values_field_decl = if runtime_contract_supported {
		quote! {
			__initial_values: ::std::rc::Rc<::std::cell::RefCell<#values_ident>>,
		}
	} else {
		quote! {}
	};
	let runtime_initial_values_field_default_init =
		if runtime_contract_supported {
			let mut initial_fields: Vec<TokenStream> = collect_scalar_fields(&macro_ast.fields)
				.iter()
				.map(|field| {
					let name = &field.name;
					let init = field_type_default_value(&field.field_type);
					quote! { #name: #init, }
				})
				.collect();
			initial_fields.extend(collect_collections(&macro_ast.fields).iter().map(
				|collection| {
					let name = &collection.name;
					quote! { #name: ::std::vec::Vec::new(), }
				},
			));
			quote! {
				__initial_values: ::std::rc::Rc::new(
					::std::cell::RefCell::new(#values_ident {
						#(#initial_fields)*
					}),
				),
			}
		} else {
			quote! {}
		};
	let runtime_initial_values_outer_refresh = if runtime_contract_supported {
		quote! {
			*__reinhardt_form.__initial_values.borrow_mut() =
				#pages_crate::FormRuntimeSource::runtime_current_values(&__reinhardt_form);
		}
	} else {
		quote! {}
	};

	let effective_state = Some(TypedFormState {
		loading: true,
		error: true,
		success: true,
		span: macro_ast.span,
	});

	// Generate field declarations
	let field_decls = generate_field_declarations(&macro_ast.name, &macro_ast.fields, pages_crate);

	// Generate state field declarations
	let state_decls = generate_state_declarations(&effective_state, pages_crate);

	// Generate field initializers
	let field_inits = generate_field_initializers(&macro_ast.fields, pages_crate);

	// Generate outer-scope assignments for fields with `initial: <expr>` so the
	// expression is evaluated where its captured locals are visible (#4420).
	let initial_outer_setup = generate_initial_outer_setup(&macro_ast.fields, pages_crate);

	// Generate state field initializers
	let state_inits = generate_state_initializers(&effective_state, pages_crate);

	// Generate field accessor methods
	let field_accessors = generate_field_accessors(&macro_ast.name, &macro_ast.fields, pages_crate);

	// Generate state accessor methods
	let state_accessors = generate_state_accessors(&effective_state, pages_crate);
	let navigation_error_handling =
		generate_on_error_callback(&macro_ast.callbacks, &effective_state);

	// Generate watch methods + supporting struct fields, default initializers,
	// setters, and outer-scope capture code.
	//
	// Watch handlers must be able to capture locals from the surrounding scope
	// (issue #4384 / #4414). Because the watch methods are emitted on the form
	// struct's impl block, a closure written inline inside the method body is
	// lexically *outside* the user's enclosing function, so it cannot reference
	// outer locals — that is what produces `E0434`. To fix this, we capture the
	// user's closure at the macro call site (where outer locals are visible),
	// type-erase it into an `Arc<dyn Fn(&FormName) -> Page>`, and store it on
	// the form struct. The watch method then simply calls through the stored
	// handler, preserving reactivity via `Page::reactive`.
	let WatchArtifacts {
		methods: watch_methods,
		field_decls: watch_field_decls,
		field_default_inits: watch_field_default_inits,
		setter_methods: watch_setter_methods,
		outer_setup: watch_outer_setup,
	} = generate_watch_artifacts(&macro_ast.watch, pages_crate, struct_name);

	// Lift `success_url:` (when present) to the outer block so its closure
	// literal is expanded where enclosing-scope locals are visible. The
	// artifacts are spliced into the struct, the `new()` initializer, the
	// outer setup block, and the submit method body. Refs #4605 / #4612 /
	// #4610.
	let SuccessUrlArtifacts {
		field_decl: success_url_field_decl,
		field_default_init: success_url_field_default_init,
		setter_method: success_url_setter_method,
		outer_setup: success_url_outer_setup,
		submit_invocation: success_url_submit_invocation,
	} = build_success_url_artifacts(
		&macro_ast.success_url,
		pages_crate,
		struct_name,
		&navigation_error_handling,
	);

	// Lift `on_success:` (when the user closure carries an explicit parameter
	// type annotation) to the outer block so it can capture enclosing-scope
	// locals like `qid`. Closures without annotations stay inline in the
	// generated `fn submit()` body (which is what the historical contract
	// guarantees for `|_x|` / `|y|` shapes whose `T` cannot be inferred at
	// outer scope). Issue #4624.
	let OnSuccessArtifacts {
		field_decl: on_success_field_decl,
		field_default_init: on_success_field_default_init,
		setter_method: on_success_setter_method,
		outer_setup: on_success_outer_setup,
		submit_invocation: on_success_submit_invocation,
		lifted: on_success_lifted,
	} = build_on_success_artifacts(&macro_ast.callbacks.on_success);

	// Generate on_success_ref artifacts (#4624). When the user has not
	// supplied `on_success_ref:`, all four TokenStreams are empty, so the
	// macro output is unchanged for forms that don't use this feature.
	// Takes the full `macro_ast` so the outer-scope wrapper can emit a
	// dead-code type-safety guard against the server_fn signature.
	let OnSuccessRefArtifacts {
		field_decl: on_success_ref_field_decl,
		field_default_init: on_success_ref_field_default_init,
		setter_method: on_success_ref_setter_method,
		outer_setup: on_success_ref_outer_setup,
	} = generate_on_success_ref_artifacts(macro_ast, pages_crate);

	// Generate derived methods
	let derived_methods = generate_derived_methods(&macro_ast.derived, pages_crate);

	// Generate metadata for SSR
	let metadata_fn = generate_metadata_function(macro_ast, pages_crate);

	// Generate into_view implementation
	let into_view_impl = generate_into_page(macro_ast, pages_crate);

	// Generate submit method if action is specified
	let submit_method = generate_submit_method(
		macro_ast,
		pages_crate,
		&success_url_submit_invocation,
		&on_success_submit_invocation,
		on_success_lifted,
	);

	// Generate validation method
	let validate_method = generate_validate_method(macro_ast, pages_crate);

	// Generate load_initial_values method if initial_loader is specified
	let load_initial_method =
		generate_load_initial_values(macro_ast, pages_crate, runtime_contract_supported);

	// Generate load_choices method if choices_loader is specified
	let load_choices_method = generate_load_choices(macro_ast, pages_crate);
	let dynamic_choice_item_type = generate_dynamic_choice_item_type(macro_ast);

	// Collect every field (including group children) and compute the
	// struct-level where clause for generic-capable inner types. The clause
	// is spliced into BOTH the struct definition and every `impl #struct_name`
	// block — Rust requires consistency between the two. For forms without
	// any generic-capable field (`HiddenField<T>` / `ChoiceField<T>` /
	// `MultipleChoiceField<T>` / `JsonField<T>`), the helper returns an empty
	// TokenStream, so the macro output is unchanged for such forms. See
	// `build_typed_field_where_clause` below for the bound list and the
	// rationale (issue #4397, Task 9).
	let mut all_fields_for_bounds = collect_scalar_fields(&macro_ast.fields);
	for collection in collect_collections(&macro_ast.fields) {
		all_fields_for_bounds.extend(collect_scalar_fields(&collection.fields));
	}
	let where_clause = build_typed_field_where_clause(&all_fields_for_bounds);

	quote! {
		{
			#use_statement
			#ambient_argument_deprecation_markers
			#form_runtime_contract
			#dynamic_choice_item_type

			#[derive(Clone)]
			struct #struct_name
			#where_clause
				{
					__reinhardt_reactive_scope: ::core::option::Option<
						::std::rc::Rc<#pages_crate::reactive::ReactiveScope>,
					>,
					#field_decls
					#runtime_initial_values_field_decl
					#state_decls
					#watch_field_decls
				#success_url_field_decl
				#on_success_field_decl
				#on_success_ref_field_decl
			}

			impl #struct_name
			#where_clause
			{
				fn new(
					__reinhardt_reactive_scope: ::core::option::Option<
						::std::rc::Rc<#pages_crate::reactive::ReactiveScope>,
					>,
				) -> Self {
						Self {
							__reinhardt_reactive_scope,
							#field_inits
							#runtime_initial_values_field_default_init
							#state_inits
						#watch_field_default_inits
						#success_url_field_default_init
						#on_success_field_default_init
						#on_success_ref_field_default_init
					}
				}

				#field_accessors
				#state_accessors
				#watch_methods
				#watch_setter_methods
				#success_url_setter_method
				#on_success_setter_method
				#on_success_ref_setter_method
				#derived_methods
				#metadata_fn
				#validate_method
				#submit_method
				#load_initial_method
				#load_choices_method
				#into_view_impl
			}

			// Build the form instance and bind user-supplied watch handlers in
			// the outer scope, where enclosing-scope locals are visible. This is
			// what makes `watch:` handlers behave as real closures (issue #4414),
			// what makes `initial: <expr>` capture outer locals (issue #4420),
			// what makes `success_url:` capture outer locals like `qid`
			// (issue #4605 / #4612), what makes `on_success:` capture outer
			// locals when the user closure is annotated (issue #4624), and
			// what makes `on_success_ref:` capture outer locals (issue #4624).
				let __reinhardt_reactive_scope = if #pages_crate::reactive::current_scope_id().is_some() {
					::core::option::Option::None
				} else {
					::core::option::Option::Some(::std::rc::Rc::new(
						#pages_crate::reactive::ReactiveScope::new(),
					))
				};
				let mut __reinhardt_form = match __reinhardt_reactive_scope.as_ref() {
					::core::option::Option::Some(scope) => scope.enter(|| {
						#struct_name::new(::core::option::Option::Some(::std::rc::Rc::clone(scope)))
					}),
					::core::option::Option::None => #struct_name::new(::core::option::Option::None),
				};
				if let ::core::option::Option::Some(scope) = __reinhardt_reactive_scope.as_ref() {
					scope.enter(|| {
						#initial_outer_setup
						#runtime_initial_values_outer_refresh
						#watch_outer_setup
						#success_url_outer_setup
						#on_success_outer_setup
						#on_success_ref_outer_setup
					});
				} else {
					#initial_outer_setup
					#runtime_initial_values_outer_refresh
					#watch_outer_setup
					#success_url_outer_setup
					#on_success_outer_setup
					#on_success_ref_outer_setup
				}
				let _ = &__reinhardt_form.__reinhardt_reactive_scope;
			__reinhardt_form
		}
	}
}

fn generate_dynamic_choice_item_type(macro_ast: &TypedFormMacro) -> TokenStream {
	let has_dynamic_field_choices = collect_scalar_fields(&macro_ast.fields)
		.iter()
		.any(|field| field.choices_config.is_some());
	let has_dynamic_datalist_choices = collect_all_datalists(&macro_ast.fields)
		.iter()
		.any(|datalist| datalist.choices_config.is_some());

	if !has_dynamic_field_choices && !has_dynamic_datalist_choices {
		return TokenStream::new();
	}

	quote! {
		#[derive(Clone, Debug)]
		struct __ReinhardtChoiceItem<T> {
			value: T,
			label: String,
			disabled: bool,
			group: ::core::option::Option<String>,
			group_disabled: bool,
		}
	}
}

/// Generates field declarations for the form struct.
fn generate_field_declarations(
	form_name: &syn::Ident,
	entries: &[TypedFormFieldEntry],
	pages_crate: &TokenStream,
) -> TokenStream {
	let fields = collect_scalar_fields(entries);
	let collections = collect_collections(entries);
	let mut decls: Vec<TokenStream> = fields
		.iter()
		.map(|field| {
			let name = &field.name;
			let signal_type = field_type_to_signal_type(&field.field_type, pages_crate);
			quote! {
				#name: #signal_type,
			}
		})
		.collect();

	decls.extend(collections.iter().map(|collection| {
		let name = &collection.name;
		let item_ident = collection_item_values_ident(form_name, &collection.name);
		quote! {
			#name: #pages_crate::reactive::Signal<::std::vec::Vec<#pages_crate::CollectionItem<#item_ident>>>,
		}
	}));
	if !collections.is_empty() {
		decls.push(quote! {
			__path_signals: ::std::rc::Rc<
				::std::cell::RefCell<
					::std::collections::HashMap<
						::std::string::String,
						::std::boxed::Box<dyn ::core::any::Any>,
					>,
				>,
			>,
		});
	}

	// Add public tuple choices and internal metadata signals for dynamic choice fields.
	let choices_decls: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			if field.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				let choice_items_name =
					syn::Ident::new(&format!("{}_choice_items", field.name), field.name.span());
				let choice_value_ty = choice_value_type(&field.field_type);
				Some(quote! {
					#choices_name: #pages_crate::reactive::Signal<Vec<(#choice_value_ty, String)>>,
					#choice_items_name: #pages_crate::reactive::Signal<Vec<__ReinhardtChoiceItem<#choice_value_ty>>>,
				})
			} else {
				None
			}
		})
		.collect();

	decls.extend(choices_decls);

	let datalist_choices_decls: Vec<TokenStream> = collect_all_datalists(entries)
		.iter()
		.filter_map(|datalist| {
			if datalist.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", datalist.name), datalist.name.span());
				Some(quote! {
					#choices_name: #pages_crate::reactive::Signal<Vec<__ReinhardtChoiceItem<::std::string::String>>>,
				})
			} else {
				None
			}
		})
		.collect();

	decls.extend(datalist_choices_decls);

	let custom_widget_error_decls: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			if matches!(field.widget, TypedWidget::CustomExperimental(_)) {
				let error_name = format_ident!(
					"__{}_custom_widget_error",
					field.name,
					span = field.name.span()
				);
				Some(quote! {
					#error_name: #pages_crate::reactive::Signal<::core::option::Option<#pages_crate::FieldError>>,
				})
			} else {
				None
			}
		})
		.collect();

	decls.extend(custom_widget_error_decls);

	let custom_widget_touched_decls: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			custom_widget_touched_ident(field).map(|touched_name| {
				quote! {
					#touched_name: #pages_crate::reactive::Signal<bool>,
				}
			})
		})
		.collect();

	decls.extend(custom_widget_touched_decls);
	quote! { #(#decls)* }
}

/// Generates field initializers for the new() function.
fn generate_field_initializers(
	entries: &[TypedFormFieldEntry],
	pages_crate: &TokenStream,
) -> TokenStream {
	let fields = collect_scalar_fields(entries);
	let collections = collect_collections(entries);
	// `new()` is a `fn` item, so any `initial: <expr>` referencing enclosing-scope
	// locals fails to compile with `E0434: can't capture dynamic environment in
	// a fn item` (issue #4420). To preserve closure-like capture semantics for
	// `initial:` expressions, `new()` always seeds fields with the field type's
	// default value here; the user-supplied `initial:` expression is evaluated
	// later in the outer block (where its locals are visible) via
	// `generate_initial_outer_setup`, which re-assigns the corresponding Signal.
	let mut inits: Vec<TokenStream> = fields
		.iter()
		.map(|field| {
			let name = &field.name;
			let init = field_type_default_value(&field.field_type);
			quote! {
				#name: #pages_crate::reactive::Signal::new(#init),
			}
		})
		.collect();

	inits.extend(collections.iter().map(|collection| {
		let name = &collection.name;
		quote! {
			#name: #pages_crate::reactive::Signal::new(::std::vec::Vec::new()),
		}
	}));
	if !collections.is_empty() {
		inits.push(quote! {
			__path_signals: ::std::rc::Rc::new(
				::std::cell::RefCell::new(::std::collections::HashMap::new()),
			),
		});
	}

	// Add public tuple choices and internal metadata signal initializers.
	let choices_inits: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			if field.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				let choice_items_name =
					syn::Ident::new(&format!("{}_choice_items", field.name), field.name.span());
				Some(quote! {
					#choices_name: #pages_crate::reactive::Signal::new(Vec::new()),
					#choice_items_name: #pages_crate::reactive::Signal::new(Vec::new()),
				})
			} else {
				None
			}
		})
		.collect();

	inits.extend(choices_inits);

	let datalist_choices_inits: Vec<TokenStream> = collect_all_datalists(entries)
		.iter()
		.filter_map(|datalist| {
			if datalist.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", datalist.name), datalist.name.span());
				Some(quote! {
					#choices_name: #pages_crate::reactive::Signal::new(Vec::new()),
				})
			} else {
				None
			}
		})
		.collect();

	inits.extend(datalist_choices_inits);

	let custom_widget_error_inits: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			if matches!(field.widget, TypedWidget::CustomExperimental(_)) {
				let error_name = format_ident!(
					"__{}_custom_widget_error",
					field.name,
					span = field.name.span()
				);
				Some(quote! {
					#error_name: #pages_crate::reactive::Signal::new(::core::option::Option::None),
				})
			} else {
				None
			}
		})
		.collect();

	inits.extend(custom_widget_error_inits);

	let custom_widget_touched_inits: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			custom_widget_touched_ident(field).map(|touched_name| {
				quote! {
					#touched_name: #pages_crate::reactive::Signal::new(false),
				}
			})
		})
		.collect();

	inits.extend(custom_widget_touched_inits);
	quote! { #(#inits)* }
}

/// Generates outer-scope assignments for fields that declared `initial: <expr>`.
///
/// The user's `<expr>` is emitted lexically inside the same block as the
/// `form!` invocation (after `let mut __reinhardt_form = FormName::new();`),
/// so it can capture enclosing-function locals like any normal closure body.
/// Each assignment replaces the field's default-seeded Signal with one that
/// holds the user-supplied value. Because the form was just constructed and
/// has no subscribers yet, replacing the Signal is observably identical to
/// constructing it with that value in `new()` — only the scope at which the
/// expression is evaluated changes (issue #4420).
fn generate_initial_outer_setup(
	entries: &[TypedFormFieldEntry],
	pages_crate: &TokenStream,
) -> TokenStream {
	let fields = collect_scalar_fields(entries);
	let assigns: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			let expr = field.initial_expr.as_ref()?;
			let name = &field.name;
			Some(quote! {
				__reinhardt_form.#name = #pages_crate::reactive::Signal::new(
					::std::convert::Into::into(#expr),
				);
			})
		})
		.collect();
	quote! { #(#assigns)* }
}

/// Generates field accessor methods.
fn generate_field_accessors(
	form_name: &syn::Ident,
	entries: &[TypedFormFieldEntry],
	pages_crate: &TokenStream,
) -> TokenStream {
	let fields = collect_scalar_fields(entries);
	let collections = collect_collections(entries);
	let mut accessors: Vec<TokenStream> = fields
		.iter()
		.map(|field| {
			let name = &field.name;
			let signal_type = field_type_to_signal_type(&field.field_type, pages_crate);
			quote! {
				pub fn #name(&self) -> &#signal_type {
					&self.#name
				}
			}
		})
		.collect();

	for collection in collections {
		let name = &collection.name;
		let item_ident = collection_item_values_ident(form_name, &collection.name);
		let new_item_method = format_ident!("new_{}_item", name);
		accessors.push(quote! {
				pub fn #name(&self) -> &#pages_crate::reactive::Signal<::std::vec::Vec<#pages_crate::CollectionItem<#item_ident>>> {
					&self.#name
				}
			});
		accessors.push(quote! {
			pub fn #new_item_method(&self) -> #item_ident {
				#item_ident::default()
			}
		});
	}

	// Add public tuple choices accessors for dynamic choice fields.
	let choices_accessors: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			if field.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				let choice_value_ty = choice_value_type(&field.field_type);
				Some(quote! {
					/// Returns the choices signal for dynamic choice options.
					pub fn #choices_name(&self) -> &#pages_crate::reactive::Signal<Vec<(#choice_value_ty, String)>> {
						&self.#choices_name
					}
				})
			} else {
				None
			}
		})
		.collect();

	accessors.extend(choices_accessors);
	quote! { #(#accessors)* }
}

/// Generates state field declarations for the form struct.
///
/// State fields are prefixed with `__` to avoid collisions with user-defined fields.
/// Generated fields:
/// - `__loading: Signal<bool>` - True during form submission
/// - `__error: Signal<Option<String>>` - Contains error message if submission failed
/// - `__success: Signal<bool>` - True after successful submission
fn generate_state_declarations(
	_state: &Option<TypedFormState>,
	pages_crate: &TokenStream,
) -> TokenStream {
	quote! {
		__loading: #pages_crate::reactive::Signal<bool>,
		__error: #pages_crate::reactive::Signal<Option<String>>,
		__success: #pages_crate::reactive::Signal<bool>,
	}
}

/// Generates state field initializers for the new() function.
fn generate_state_initializers(
	_state: &Option<TypedFormState>,
	pages_crate: &TokenStream,
) -> TokenStream {
	quote! {
		__loading: #pages_crate::reactive::Signal::new(false),
		__error: #pages_crate::reactive::Signal::new(None),
		__success: #pages_crate::reactive::Signal::new(false),
	}
}

/// Generates state accessor methods.
///
/// Accessor methods provide a clean public API for accessing state signals:
/// - `loading()` returns `&Signal<bool>`
/// - `error()` returns `&Signal<Option<String>>`
/// - `success()` returns `&Signal<bool>`
fn generate_state_accessors(
	_state: &Option<TypedFormState>,
	pages_crate: &TokenStream,
) -> TokenStream {
	quote! {
		/// Returns the generated loading signal.
		///
		/// This compatibility signal is `true` while the generated submit
		/// handler is in progress. New runtime orchestration should use
		/// `use_form(&form).build().form_state()`.
		pub fn loading(&self) -> &#pages_crate::reactive::Signal<bool> {
			&self.__loading
		}

		/// Returns the generated submit error signal.
		///
		/// This compatibility signal contains the error message if the last
		/// generated submit failed. New runtime orchestration should use
		/// `use_form(&form).build().form_state()`.
		pub fn error(&self) -> &#pages_crate::reactive::Signal<Option<String>> {
			&self.__error
		}

		/// Returns the generated success signal.
		///
		/// This compatibility signal is `true` after the generated submit
		/// succeeds. New runtime orchestration should use
		/// `use_form(&form).build().form_state()`.
		pub fn success(&self) -> &#pages_crate::reactive::Signal<bool> {
			&self.__success
		}
	}
}

/// Bundle of token streams produced for the watch block.
///
/// The watch block needs to plug into several positions in the generated
/// form expansion — struct fields, struct field initializers in `new()`,
/// impl methods, internal setters, and a piece of code that runs in the
/// outer (macro call-site) scope. Returning them together keeps the call
/// site explicit and avoids re-walking the watch list multiple times.
struct WatchArtifacts {
	/// Methods to splice into `impl FormName { ... }`.
	methods: TokenStream,
	/// Field declarations to splice into `struct FormName { ... }`.
	field_decls: TokenStream,
	/// Default initializers used in `FormName::new()` — one per watch field,
	/// each producing an `Arc` that returns an empty `Page` until the real
	/// handler is bound from the outer scope.
	field_default_inits: TokenStream,
	/// Private setter methods on `FormName` used to bind the real handlers
	/// after `new()` returns.
	setter_methods: TokenStream,
	/// Code emitted in the outer block, after `let mut __reinhardt_form = FormName::new();`.
	/// This is where the user's closure literals appear lexically, so they can
	/// capture enclosing-scope locals (`outer_local` in the issue #4414 test).
	outer_setup: TokenStream,
}

/// Generates everything needed to wire up watch handlers.
///
/// Each watch item produces:
///
/// 1. A private struct field `__watch_<name>: Arc<dyn Fn(&FormName) -> Page>`
///    storing a type-erased handler.
/// 2. A default initializer in `FormName::new()` returning an empty `Page`,
///    so `new()` keeps its zero-argument signature.
/// 3. A public method `#name(&self) -> impl IntoPage` that clones the stored
///    handler and wraps it in `Page::reactive` for automatic re-rendering.
/// 4. A private setter `__set_watch_<name>` used internally.
/// 5. Outer-scope setup code that captures the user's closure literal
///    (where enclosing-scope locals are visible), wraps it in an Arc, and
///    calls the setter on the form instance.
///
/// The outer-scope step is the key part of the fix for issue #4414: a
/// previous attempt placed the user closure inline inside the watch method
/// body, but a method body is lexically outside the enclosing function and
/// cannot reach its locals, producing `E0434`. By capturing the closure at
/// the macro call site instead, the handler behaves like any other Rust
/// closure that captures from its surrounding function.
fn generate_watch_artifacts(
	watch: &Option<TypedFormWatch>,
	pages_crate: &TokenStream,
	struct_name: &syn::Ident,
) -> WatchArtifacts {
	let empty = WatchArtifacts {
		methods: quote! {},
		field_decls: quote! {},
		field_default_inits: quote! {},
		setter_methods: quote! {},
		outer_setup: quote! {},
	};

	let Some(watch) = watch else {
		return empty;
	};

	if watch.items.is_empty() {
		return empty;
	}

	let mut methods = Vec::new();
	let mut field_decls = Vec::new();
	let mut field_default_inits = Vec::new();
	let mut setter_methods = Vec::new();
	let mut outer_setup = Vec::new();

	for item in &watch.items {
		let method_name = &item.name;
		let closure = &item.closure;
		let field_ident = quote::format_ident!("__watch_{}", method_name);
		let setter_ident = quote::format_ident!("__set_watch_{}", method_name);

		// Stored handler type — type-erased so the form struct stays nameable.
		let handler_ty = quote! {
			::std::sync::Arc<
				dyn ::core::ops::Fn(&#struct_name) -> #pages_crate::component::Page,
			>
		};

		field_decls.push(quote! {
			#field_ident: #handler_ty,
		});

		// Default no-op handler used before the real one is bound. Returns an
		// empty fragment so an un-bound watch is harmless if ever called.
		field_default_inits.push(quote! {
			#field_ident: ::std::sync::Arc::new(
				|_: &#struct_name| -> #pages_crate::component::Page {
					#pages_crate::component::Page::Fragment(::std::vec::Vec::new())
				},
			),
		});

		setter_methods.push(quote! {
			#[doc(hidden)]
			fn #setter_ident(&mut self, __handler: #handler_ty) {
				self.#field_ident = __handler;
			}
		});

		methods.push(quote! {
			/// Returns a reactive view that automatically re-renders when its Signal dependencies change.
			///
			/// This method wraps the watch closure (bound from the form! macro
			/// call site) in `Page::reactive` for automatic re-rendering.
			pub fn #method_name(&self) -> impl #pages_crate::component::IntoPage {
				let __form = self.clone();
				let __handler = self.#field_ident.clone();
				#pages_crate::component::Page::reactive(move || __handler(&__form))
			}
		});

		// Outer-scope capture. The user's `#closure` is expanded *here*, which
		// is the same lexical scope as the `form!` macro invocation, so it can
		// capture locals from the enclosing function (issue #4414).
		//
		// The user's closure is invoked *inside* an outer `move` closure that
		// also captures the same enclosing-scope locals. This way the `Arc`'s
		// `'static` bound is satisfied: any outer locals referenced by the
		// user closure are moved/copied into the outer `move` closure's
		// environment, and the user closure then borrows from those owned
		// captures rather than from the surrounding stack frame.
		//
		// The user closure is funneled through a local `__coerce` identity
		// function whose `where`-bound forces its parameter type to
		// `&#struct_name`. Without this, the user's `|form| { form.method() }`
		// fails with `E0282: type annotations needed` because Rust type-checks
		// the closure body before learning the eventual call site's argument
		// type. The local fn item can name `#struct_name` (types from the
		// enclosing block are visible to fn items) but does not capture
		// values, so it does not reintroduce the original `E0434` (#4420).
		outer_setup.push(quote! {
			{
				let __wrapped: #handler_ty = ::std::sync::Arc::new(
					move |__f: &#struct_name| -> #pages_crate::component::Page {
						fn __coerce<__F, __R>(__f: __F) -> __F
						where
							__F: ::core::ops::Fn(&#struct_name) -> __R,
						{
							__f
						}
						let __user_watch_handler = __coerce(#closure);
						#pages_crate::component::IntoPage::into_page(
							__user_watch_handler(__f),
						)
					},
				);
				__reinhardt_form.#setter_ident(__wrapped);
			}
		});
	}

	WatchArtifacts {
		methods: quote! { #(#methods)* },
		field_decls: quote! { #(#field_decls)* },
		field_default_inits: quote! { #(#field_default_inits)* },
		setter_methods: quote! { #(#setter_methods)* },
		outer_setup: quote! { #(#outer_setup)* },
	}
}

/// Artifacts produced for the `on_success_ref:` callback (issue #4624).
///
/// `on_success_ref:` is the "lifted" sibling of `on_success:` — its user
/// closure is expanded at the outer construction block (same lexical scope
/// as the `form!` macro invocation), so it can capture enclosing-scope
/// locals (e.g. a `user_id` bound earlier in the surrounding function).
/// `on_success:` itself stays inline in `fn submit()` because it consumes
/// the server_fn return value `T` by move, and three in-tree callers rely
/// on that — flipping the signature to `&T` is a SemVer break deferred to
/// `develop/0.2.0`.
///
/// The trick that makes this work without naming `T` anywhere in the
/// macro output is the same `__coerce`-style identity-fn pattern used by
/// `WatchArtifacts`, extended to three generic parameters so the user
/// closure's `&T` value parameter can be inferred without the user having
/// to annotate it. At the `Arc<dyn Fn(&Form, &dyn Any)>` storage boundary
/// the inferred `T` is then type-erased; the submit-site downcast is
/// infallible by macro construction because both sides of the type
/// identity are produced from the same closure literal.
struct OnSuccessRefArtifacts {
	/// Field declaration to splice into `struct FormName { ... }`. Empty
	/// when the user did not supply `on_success_ref:`.
	field_decl: TokenStream,
	/// Default initializer used in `FormName::new()`. Empty when the user
	/// did not supply `on_success_ref:`.
	field_default_init: TokenStream,
	/// Private setter method on `FormName` used to bind the real handler
	/// after `new()` returns. Empty when the user did not supply
	/// `on_success_ref:`.
	setter_method: TokenStream,
	/// Code emitted in the outer block, after `let mut __reinhardt_form = FormName::new();`.
	/// This is where the user's closure literal appears lexically, so it
	/// can capture enclosing-scope locals.
	outer_setup: TokenStream,
}

/// Generates the artifacts needed to wire up `on_success_ref:`.
///
/// When the user has not supplied `on_success_ref:`, every TokenStream in
/// the returned artifacts is empty — the resulting macro output is
/// indistinguishable from a form that doesn't use the feature.
///
/// When supplied, this produces:
///
/// 1. A private struct field
///    `__on_success_ref_handler: Arc<dyn Fn(&FormName, &dyn Any)>` storing
///    a type-erased handler.
/// 2. A default initializer in `FormName::new()` returning a no-op closure
///    so `new()` keeps its zero-argument signature.
/// 3. A private setter `__set_on_success_ref_handler` used internally.
/// 4. Outer-scope setup code that captures the user's closure literal
///    (where enclosing-scope locals are visible), funnels it through a
///    `__coerce_form<__F, __T, __R>` identity fn to fix the parameter
///    shape `(&FormName, &__T) -> __R`, wraps it in an `Arc`, and calls
///    the setter on the form instance.
fn generate_on_success_ref_artifacts(
	macro_ast: &TypedFormMacro,
	pages_crate: &TokenStream,
) -> OnSuccessRefArtifacts {
	let _ = pages_crate; // reserved for future cross-crate refs; quiet warnings now.
	let callbacks = &macro_ast.callbacks;
	let struct_name = &macro_ast.name;

	let empty = OnSuccessRefArtifacts {
		field_decl: quote! {},
		field_default_init: quote! {},
		setter_method: quote! {},
		outer_setup: quote! {},
	};

	let Some(closure) = &callbacks.on_success_ref else {
		return empty;
	};

	// Compile-time type-safety guard (#4624). Forces the user closure's
	// inferred `__T` (the second-parameter type, e.g. `&Wrong` in
	// `on_success_ref: |_, _v: &Wrong| { ... }`) to equal the server_fn's
	// Ok arm. Implemented as a dead-code branch that calls the server_fn
	// (via `::core::unreachable!()` for every argument, which never-coerces
	// to any required type) and feeds the extracted Ok value into the
	// user closure. If the types disagree, compilation fails AT the user
	// closure's parameter type — not at runtime in `downcast_ref().expect()`.
	//
	// Skipped for non-ServerFn actions: a URL form has no Future to extract
	// `T` from, and `on_success_ref` would never actually fire there.
	let type_check_dead_code = match &macro_ast.action {
		TypedFormAction::ServerFn(server_fn_ident) => {
			// Replicate `generate_submit_method`'s argument count
			// computation (codegen.rs ~L1698-1725) so that the dead-code
			// call below matches whatever signature the runtime call uses.
			let all_fields = collect_scalar_fields(&macro_ast.fields);
			let field_count = all_fields.len();
			let strip_arg_count = macro_ast.strip_arguments.len();
			let extra_count = strip_arg_count;
			let total_arg_count = field_count + extra_count;
			let probe_args = (0..total_arg_count)
				.map(|_| quote! { ::core::unreachable!() })
				.collect::<Vec<_>>();

			quote! {
				#[allow(unreachable_code, dead_code, unused_variables, clippy::diverging_sub_expression)]
				if false {
					// Extract the `Ok` type `T` from the server_fn's
					// `Future<Output = Result<T, _>>`. Generic over the
					// future / Ok / Err types so it works regardless of
					// the user's concrete server_fn signature.
					fn __on_success_ref_extract_ok<__Fut, __T, __E>(_: __Fut) -> __T
					where
						__Fut: ::core::future::Future<
							Output = ::core::result::Result<__T, __E>,
						>,
					{
						::core::unreachable!()
					}
					let __probe_future = #server_fn_ident(#(#probe_args),*);
					let __probe_ok = __on_success_ref_extract_ok(__probe_future);
					// This call forces `__user_on_success_ref`'s second
					// parameter type to equal `__probe_ok`'s type, which
					// equals the server_fn's `Ok` arm. A mismatch with
					// the user closure's annotated/inferred `__T` fails
					// here at compile time.
					let _ = __user_on_success_ref(__form, &__probe_ok);
				}
			}
		}
		_ => quote! {},
	};

	// Stored handler type — type-erased so the form struct stays nameable.
	// The `&dyn Any` parameter is what lets the macro avoid spelling `T`:
	// `T` is inferred from the user closure's second parameter via the
	// `__coerce_form` identity fn below, and immediately erased here.
	let handler_ty = quote! {
		::std::sync::Arc<
			dyn ::core::ops::Fn(&#struct_name, &dyn ::core::any::Any),
		>
	};

	let field_decl = quote! {
		__on_success_ref_handler: #handler_ty,
	};

	// Default no-op handler used before the real one is bound. Calling it
	// is harmless: it ignores both the form ref and the erased value.
	let field_default_init = quote! {
		__on_success_ref_handler: ::std::sync::Arc::new(
			|_: &#struct_name, _: &dyn ::core::any::Any| {},
		),
	};

	let setter_method = quote! {
		#[doc(hidden)]
		fn __set_on_success_ref_handler(&mut self, __handler: #handler_ty) {
			self.__on_success_ref_handler = __handler;
		}
	};

	// Outer-scope capture. The user's `#closure` is expanded *here*,
	// which is the same lexical scope as the `form!` macro invocation,
	// so it can capture locals from the enclosing function (issue #4624).
	//
	// The closure is funneled through `__coerce_form` whose `where`-bound
	// pins its parameter shape to `(&#struct_name, &__T) -> __R`. This
	// gives the user's `|form, value| { ... }` enough type information
	// to infer the value type from the body, without the user having to
	// write `|form: &MyForm, value: &MyT|` annotations. `__T: 'static`
	// is required because `Any::downcast_ref` only works on `'static`
	// types — server_fn return types are owned types and naturally
	// satisfy this.
	//
	// Type safety is enforced by `#type_check_dead_code`: a dead-code
	// branch that calls the server_fn (via `unreachable!()` args) and
	// feeds the extracted Ok value into the user closure, forcing
	// Rust's type-checker to unify the closure's `__T` with the
	// server_fn's Ok arm at COMPILE TIME. With that guard in place,
	// the `downcast_ref().expect(...)` below is provably unreachable
	// in user-facing builds — it remains as a defensive runtime
	// invariant check in case the macro's own scaffolding ever
	// regresses, not as a fallibility ever surfaced to users.
	let outer_setup = quote! {
		{
			let __wrapped: #handler_ty = ::std::sync::Arc::new(
				move |__form: &#struct_name, __erased: &dyn ::core::any::Any| {
					fn __coerce_form<__F, __T, __R>(__f: __F) -> __F
					where
						__F: ::core::ops::Fn(&#struct_name, &__T) -> __R,
						__T: 'static,
					{
						__f
					}
					let __user_on_success_ref = __coerce_form(#closure);

					// Compile-time type assertion (#4624). The dead-code
					// branch emitted by `#type_check_dead_code` calls the
					// server_fn and feeds its Ok value into the user
					// closure, so the compiler unifies `__T` with the
					// server_fn's Ok type. If the user annotates the
					// closure parameter with a wrong type, compilation
					// fails AT the annotation site — never here at the
					// runtime downcast. See the doc comment on
					// `generate_on_success_ref_artifacts` for the full
					// soundness argument.
					#type_check_dead_code

					let __value_ref = __erased.downcast_ref().expect(
						"on_success_ref: internal invariant violated — \
						 server_fn Ok value did not downcast to the user \
						 closure's parameter type. This branch should be \
						 unreachable because the macro emits a compile-time \
						 type assertion that forces them to match; \
						 reaching it indicates a macro implementation bug, \
						 not a user error",
					);
					let _ = __user_on_success_ref(__form, __value_ref);
				},
			);
			__reinhardt_form.__set_on_success_ref_handler(__wrapped);
		}
	};

	OnSuccessRefArtifacts {
		field_decl,
		field_default_init,
		setter_method,
		outer_setup,
	}
}

/// Generates derived methods that compute derived values.
///
/// Each derived item becomes a method on the form struct that evaluates the closure
/// and returns the computed value directly. This provides a simple API for accessing
/// computed values that depend on form field signals.
///
/// ## Generated Pattern
///
/// For a derived item like:
/// ```text
/// char_count: |form| form.content().get().len()
/// ```
///
/// Generates:
/// ```text
/// pub fn char_count(&self) -> usize {
///     let __derived_closure = |form: &Self| form.content().get().len();
///     __derived_closure(self)
/// }
/// ```
///
/// The closure is evaluated each time the method is called, reading the current
/// signal values. For reactive memoization, users can wrap the call in `Memo::new`:
///
/// ```text
/// let form = form.clone();
/// let count_memo = Memo::new(move || form.char_count());
/// ```
// Parameter reserved for future crate path customization
fn generate_derived_methods(
	derived: &Option<TypedFormDerived>,
	_pages_crate: &TokenStream,
) -> TokenStream {
	let Some(derived) = derived else {
		return quote! {};
	};

	if derived.items.is_empty() {
		return quote! {};
	}

	let methods: Vec<TokenStream> = derived
		.items
		.iter()
		.map(|item| {
			let method_name = &item.name;
			let closure = &item.closure;

			// Extract the closure body for direct use
			// The closure has form: |param| body
			// We'll call it with &self to get the result
			quote! {
				/// Returns a computed value derived from form fields.
				///
				/// This method evaluates the derived expression with the current form state.
				/// The value is computed fresh on each call, reading current signal values.
				///
				/// For memoization, wrap in a `Memo`:
				/// ```no_run
				/// # use reinhardt_pages::reactive::Memo;
				/// let form = form.clone();
				/// let memo = Memo::new(move || form.char_count());
				/// ```
				#[allow(clippy::unused_self)]
				pub fn #method_name(&self) -> impl ::core::clone::Clone {
					let __derived_fn: fn(&Self) -> _ = #closure;
					__derived_fn(self)
				}
			}
		})
		.collect();

	quote! { #(#methods)* }
}

/// Converts a struct name to kebab-case for use as form ID.
///
/// Example: `RegisterForm` -> `register-form`, `LoginForm` -> `login-form`
fn form_id_kebab_case(name: &syn::Ident) -> String {
	name.to_string()
		.chars()
		.enumerate()
		.flat_map(|(i, c)| {
			if c.is_uppercase() && i > 0 {
				vec!['-', c.to_ascii_lowercase()]
			} else {
				vec![c.to_ascii_lowercase()]
			}
		})
		.collect::<String>()
		.replace('_', "-")
}

/// Resolves the form action URL from the typed action.
fn action_string(action: &TypedFormAction) -> String {
	match action {
		TypedFormAction::Url(url) => url.clone(),
		TypedFormAction::ServerFn(path) => format!("/api/{}", path.to_token_stream()),
		TypedFormAction::None => String::new(),
	}
}

/// Generates the metadata function for SSR.
fn generate_metadata_function(
	macro_ast: &TypedFormMacro,
	pages_crate: &TokenStream,
) -> TokenStream {
	let form_id_str = form_id_kebab_case(&macro_ast.name);
	let action_str = action_string(&macro_ast.action);

	let method_str = match macro_ast.method {
		FormMethod::Get => "GET",
		FormMethod::Post => "POST",
		FormMethod::Put => "PUT",
		FormMethod::Patch => "PATCH",
		FormMethod::Delete => "DELETE",
	};

	let form_class = macro_ast
		.styling
		.class
		.as_deref()
		.unwrap_or("reinhardt-form");

	// Collect all fields (including those in groups) for metadata
	let all_fields = collect_scalar_fields(&macro_ast.fields);
	let field_metadata: Vec<TokenStream> = all_fields
		.iter()
		.map(|field| {
			let name = field.name.to_string();
			let field_type = field_type_to_string(&field.field_type);
			let widget = widget_to_string(&field.widget);
			let required = field.validation.required;
			// Use name variable instead of creating new temporary to avoid E0716
			let label = field.display.label.as_deref().unwrap_or(&name);
			let placeholder = field.display.placeholder.as_deref().unwrap_or("");
			let input_class = field.styling.input_class();
			let wrapper_class = field.styling.wrapper_class();
			let label_class = field.styling.label_class();
			let error_class = field.styling.error_class();

			quote! {
				#pages_crate::form_generated::StaticFieldMetadata {
					name: #name.to_string(),
					field_type: #field_type.to_string(),
					widget: #widget.to_string(),
					required: #required,
					label: #label.to_string(),
					placeholder: #placeholder.to_string(),
					input_class: #input_class.to_string(),
					wrapper_class: #wrapper_class.to_string(),
					label_class: #label_class.to_string(),
					error_class: #error_class.to_string(),
				}
			}
		})
		.collect();

	quote! {
		pub fn metadata(&self) -> #pages_crate::form_generated::StaticFormMetadata {
			#pages_crate::form_generated::StaticFormMetadata {
				id: #form_id_str.to_string(),
				action: #action_str.to_string(),
				method: #method_str.to_string(),
				class: #form_class.to_string(),
				fields: vec![#(#field_metadata),*],
			}
		}
	}
}

/// Generates the into_view implementation.
fn generate_into_page(macro_ast: &TypedFormMacro, pages_crate: &TokenStream) -> TokenStream {
	// Collect all fields for signal bindings
	let all_fields = collect_scalar_fields(&macro_ast.fields);

	// Generate signal bindings for fields with bind: true
	let signal_bindings: Vec<TokenStream> = all_fields
		.iter()
		.filter(|field| field.bind && !matches!(field.widget, TypedWidget::CustomExperimental(_)))
		.map(|field| {
			let field_name = &field.name;
			let signal_ident = quote::format_ident!("{}_signal", field_name);
			quote! {
				let #signal_ident = self.#field_name.clone();
			}
		})
		.collect();

	// Generate onsubmit handler for server_fn forms
	let onsubmit_handler = generate_onsubmit_handler(macro_ast, pages_crate);

	quote! {
		pub fn into_page(self) -> #pages_crate::component::Page {
			use #pages_crate::component::{PageElement, IntoPage};

			#(#signal_bindings)*

			#onsubmit_handler

			form_element.into_page()
		}
	}
}

/// Generates the onsubmit handler for server_fn forms.
///
/// When a form has a server_fn action, this generates an onsubmit event handler
/// that prevents default form submission and calls the server function instead.
fn generate_onsubmit_handler(macro_ast: &TypedFormMacro, pages_crate: &TokenStream) -> TokenStream {
	let form_id_str = form_id_kebab_case(&macro_ast.name);
	let action_str = action_string(&macro_ast.action);

	let method_str = match macro_ast.method {
		FormMethod::Get => "get",
		FormMethod::Post => "post",
		FormMethod::Put => "put",
		FormMethod::Patch => "patch",
		FormMethod::Delete => "delete",
	};

	let form_class = macro_ast
		.styling
		.class
		.as_deref()
		.unwrap_or("reinhardt-form");

	// Collect all fields for use in onsubmit handler
	let all_fields = collect_scalar_fields(&macro_ast.fields);

	// Generate before_fields slot if present
	let before_fields_slot = generate_before_fields_slot(&macro_ast.slots);

	// Generate after_fields slot if present
	let after_fields_slot = generate_after_fields_slot(&macro_ast.slots);

	// Determine if CSRF protection is needed (non-GET methods)
	let needs_csrf = !matches!(macro_ast.method, FormMethod::Get);

	// Check if CSRF token was explicitly supplied as a server_fn business argument.
	let has_explicit_csrf_field = all_fields.iter().any(|f| f.name == "csrf_token");
	let csrf_business_arg = needs_csrf
		&& matches!(macro_ast.action, TypedFormAction::ServerFn(_))
		&& !has_explicit_csrf_field
		&& should_pass_csrf_as_business_argument(macro_ast);

	// Generate CSRF token injection for non-GET methods
	let csrf_injection = if needs_csrf {
		quote! {
			.child({
				let csrf_token = #pages_crate::csrf::get_csrf_token()
					.unwrap_or_default();
				PageElement::new("input")
					.attr("type", "hidden")
					.attr("name", #pages_crate::csrf::CSRF_FORM_FIELD)
					.attr("value", csrf_token)
			})
		}
	} else {
		quote! {}
	};

	// Generate watch component calls if watch block exists
	let watch_components = if let Some(watch) = &macro_ast.watch {
		let method_calls: Vec<TokenStream> = watch
			.items
			.iter()
			.map(|item| {
				let method_name = &item.name;
				quote! { .child(self.#method_name()) }
			})
			.collect();
		quote! { #(#method_calls)* }
	} else {
		quote! {}
	};

	// Generate field/group views
	let field_views: Vec<TokenStream> = macro_ast
		.fields
		.iter()
		.map(|entry| generate_field_entry_view(entry, pages_crate, &all_fields))
		.collect();

	match &macro_ast.action {
		TypedFormAction::ServerFn(server_fn_ident) => {
			// Generate field signal clones for onsubmit handler
			let field_names: Vec<&syn::Ident> = all_fields.iter().map(|f| &f.name).collect();
			let field_signal_clones: Vec<TokenStream> = field_names
				.iter()
				.map(|name| {
					// Sanitize variable name to avoid double underscores (submit__field -> submit_field)
					let signal_name_str = format!("submit_{}", name.unraw());
					let sanitized = signal_name_str.replace("__", "_");
					let signal_name = quote::format_ident!("{}", sanitized);
					quote! { let #signal_name = self.#name.clone(); }
				})
				.collect();

			// Generate field value getters for server_fn call
			let field_value_getters: Vec<TokenStream> = field_names
				.iter()
				.map(|name| {
					// Sanitize variable name to avoid double underscores (submit__field -> submit_field)
					let signal_name_str = format!("submit_{}", name.unraw());
					let sanitized = signal_name_str.replace("__", "_");
					let signal_name = quote::format_ident!("{}", sanitized);
					quote! { #signal_name.get() }
				})
				.collect();

			let strip_arg_exprs: Vec<&syn::Expr> = macro_ast
				.strip_arguments
				.iter()
				.map(|arg| &arg.value)
				.collect();

			// Legacy compatibility: old form! server_fn bindings could append
			// CSRF as a business argument through strip_arguments /
			// ambient_arguments. New use_form-based lowering should not use this
			// path; CSRF belongs in the X-CSRFToken header injected by
			// #[server_fn] client stubs.
			let server_fn_call = if csrf_business_arg || !strip_arg_exprs.is_empty() {
				quote! { #server_fn_ident(#(#field_names,)* #(#strip_arg_exprs),*).await }
			} else {
				quote! { #server_fn_ident(#(#field_names),*).await }
			};

			// Generate callbacks
			let callbacks = &macro_ast.callbacks;
			let redirect = &macro_ast.redirect_on_success;

			// Generate loading state management
			let loading_start = quote! {
				submit_loading.set(true);
				submit_error.set(None);
				submit_success.set(false);
			};

			// Clone loading/error signals if they exist
			let state_signal_clones = quote! {
				let submit_loading = self.loading().clone();
				let submit_error = self.error().clone();
				let submit_success = self.success().clone();
			};

			// Navigation failures use the same error callback and submit-error
			// signal as server-function failures.
			let on_error_code = if let Some(callback) = &callbacks.on_error {
				quote! { (#callback)(e.clone()); }
			} else {
				quote! {}
			};
			let async_signal_clones = quote! {
				let async_loading = submit_loading.clone();
				let async_error = submit_error.clone();
				let async_success = submit_success.clone();
			};
			let async_error_handling = quote! {
				async_error.set(Some(e.to_string()));
				async_success.set(false);
			};

			// `navigate_or_reload()` owns the exact RouterNotInstalled fallback
			// policy and direct browser dispatch for safe external redirects.
			let redirect_code = if let Some(url) = redirect {
				quote! {
					match #pages_crate::navigate_or_reload(
						::std::string::ToString::to_string(#url),
						#pages_crate::NavigationType::Push,
					) {
						Ok(()) => async_success.set(true),
						Err(__navigation_error) => {
							let e = #pages_crate::ServerFnError::application(
								::std::string::ToString::to_string(&__navigation_error),
							);
							#on_error_code
							#async_error_handling
						}
					}
				}
			} else if macro_ast.success_url.is_none() {
				quote! { async_success.set(true); }
			} else {
				quote! {}
			};

			// Generate on_success callback if present
			let on_success_code = if let Some(callback) = &callbacks.on_success {
				quote! { (#callback)(_value); }
			} else {
				quote! {}
			};

			// Generate on_success_ref clones + invocation if present (#4624).
			// The `spawn_task` async block captures by move, so the lifted
			// `Arc<dyn Fn(&Self, &dyn Any)>` handler and a form-instance
			// clone must be hoisted into the surrounding scope first.
			// `Form::clone()` is cheap — every field is an `Arc` or `Signal`.
			let on_success_ref_outer_clones = if callbacks.on_success_ref.is_some() {
				quote! {
					let __on_success_ref_handler_clone = self.__on_success_ref_handler.clone();
					let __form_clone_for_on_success_ref = self.clone();
				}
			} else {
				quote! {}
			};
			let on_success_ref_invocation = if callbacks.on_success_ref.is_some() {
				quote! {
					__on_success_ref_handler_clone(
						&__form_clone_for_on_success_ref,
						&_value as &dyn ::core::any::Any,
					);
				}
			} else {
				quote! {}
			};

			let success_url_outer_clones = if macro_ast.success_url.is_some() {
				quote! {
					let __success_url_handler_clone = self.__success_url_handler.clone();
					let __form_clone_for_success_url = self.clone();
				}
			} else {
				quote! {}
			};
			let success_url_event_clones = if macro_ast.success_url.is_some() {
				quote! {
					let __success_url_handler_for_submit =
						__success_url_handler_clone.clone();
					let __form_clone_for_success_url =
						__form_clone_for_success_url.clone();
				}
			} else {
				quote! {}
			};
			let success_url_invocation = if macro_ast.success_url.is_some() {
				quote! {
					if let ::core::option::Option::Some(__handler) =
						__success_url_handler_for_submit.as_ref()
					{
						let __url = __handler(&__form_clone_for_success_url);
						match #pages_crate::navigate_or_reload(
							__url,
							#pages_crate::NavigationType::Push,
						) {
							Ok(()) => async_success.set(true),
							Err(__navigation_error) => {
								let e = #pages_crate::ServerFnError::application(
									::std::string::ToString::to_string(&__navigation_error),
								);
								#on_error_code
								#async_error_handling
							}
						}
					} else {
						async_success.set(true);
					}
				}
			} else {
				quote! {}
			};

			// Generate loading end with async signal
			let async_loading_end = quote! { async_loading.set(false); };

			quote! {
				// Clone field signals for onsubmit handler
				#(#field_signal_clones)*

				// Clone state signals for onsubmit handler
				#state_signal_clones

				// Clone the lifted on_success_ref Arc + a form-instance ref
				// for the async block (#4624). Both must live in the outer
				// scope so `async move` can capture them.
				#on_success_ref_outer_clones
				#success_url_outer_clones

				let form_element = PageElement::new("form")
					.attr("id", #form_id_str)
					.attr("action", #action_str)
					.attr("method", #method_str)
					.attr("class", #form_class)
					#before_fields_slot
					#csrf_injection
					#(.child(#field_views))*
					#after_fields_slot
					#watch_components
					.on(
					#pages_crate::dom::event::EventType::Submit,
					{
						#[cfg(all(target_family = "wasm", target_os = "unknown"))]
						{
							#pages_crate::typed_event_handler::<#pages_crate::event::SubmitEvent, _>(
							move |event: #pages_crate::event::SubmitEvent| {
								// Prevent default form submission by handling it ourselves
								event.prevent_default();

								// Get field values from cloned signals
								#loading_start

								// Get field values from cloned signals - allow non_snake_case for generated variable names
								#(
									#[allow(non_snake_case, unused_variables)]
									let #field_names = #field_value_getters;
								)*

								// Clone loading/error signals for async block if they exist
								#async_signal_clones
								#success_url_event_clones

								#pages_crate::platform::spawn_task(async move {
									match #server_fn_call {
										Ok(_value) => {
											// Order matters: on_success_ref
											// (borrows) runs before on_success
											// (may consume by move). #4624.
											#on_success_ref_invocation
											#on_success_code
											#redirect_code
											#success_url_invocation
										}
										Err(e) => {
											#on_error_code
											#async_error_handling
										}
									}
									#async_loading_end
								});
							})
						}
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						{
							#pages_crate::typed_event_handler::<#pages_crate::event::SubmitEvent, _>(
							move |event: #pages_crate::event::SubmitEvent| {
								// Non-WASM: form submission is handled via HTTP, not JavaScript
								event.prevent_default();
							})
						}
					}
				);
			}
		}
		_ => {
			// For URL action or no action, just generate the form without onsubmit handler
			quote! {
				let form_element = PageElement::new("form")
					.attr("id", #form_id_str)
					.attr("action", #action_str)
					.attr("method", #method_str)
					.attr("class", #form_class)
					#before_fields_slot
					#csrf_injection
					#(.child(#field_views))*
					#after_fields_slot
					#watch_components;
			}
		}
	}
}

/// Generates code for the before_fields slot.
///
/// If a before_fields closure is defined in slots, generates code to call it
/// and add its result as a child before the field views.
fn generate_before_fields_slot(slots: &Option<TypedFormSlots>) -> TokenStream {
	match slots {
		Some(s) if s.before_fields.is_some() => {
			let closure = s.before_fields.as_ref().unwrap();
			quote! {
				.child((#closure)())
			}
		}
		_ => TokenStream::new(),
	}
}

/// Generates code for the after_fields slot.
///
/// If an after_fields closure is defined in slots, generates code to call it
/// and add its result as a child after the field views.
fn generate_after_fields_slot(slots: &Option<TypedFormSlots>) -> TokenStream {
	match slots {
		Some(s) if s.after_fields.is_some() => {
			let closure = s.after_fields.as_ref().unwrap();
			quote! {
				.child((#closure)())
			}
		}
		_ => TokenStream::new(),
	}
}

/// Generates view code for a field entry (either a regular field or a field group).
///
/// For field groups, generates a wrapper div with optional label and the fields inside.
fn generate_field_entry_view(
	entry: &TypedFormFieldEntry,
	pages_crate: &TokenStream,
	all_fields: &[&TypedFormFieldDef],
) -> TokenStream {
	match entry {
		TypedFormFieldEntry::Field(field) => {
			let field = field.as_ref();
			// Get the signal identifier for this field if it has bind: true
			let signal_ident = if field.bind {
				Some(quote::format_ident!("{}_signal", field.name))
			} else {
				None
			};
			generate_field_view(field, pages_crate, signal_ident.as_ref())
		}
		TypedFormFieldEntry::Group(group) => {
			generate_field_group_view(group, pages_crate, all_fields)
		}
		TypedFormFieldEntry::Collection(collection) => {
			generate_collection_view(collection, pages_crate)
		}
		TypedFormFieldEntry::SubmitButton(btn) => generate_submit_button_view(btn),
		TypedFormFieldEntry::ResetButton(btn) | TypedFormFieldEntry::Button(btn) => {
			generate_button_control_view(btn)
		}
		TypedFormFieldEntry::ImageInput(input) => generate_image_input_view(input),
		TypedFormFieldEntry::Output(output) => generate_output_view(output),
		TypedFormFieldEntry::Meter(meter) => generate_meter_view(meter),
		TypedFormFieldEntry::Progress(progress) => generate_progress_view(progress),
		TypedFormFieldEntry::Datalist(datalist) => generate_datalist_view(datalist, pages_crate),
	}
}

/// Generates view code for a submit button.
///
/// Produces a `<button type="submit">` element with optional class, id, and disabled attributes.
/// No wrapper div or label — just the bare button element.
fn generate_submit_button_view(btn: &TypedSubmitButtonDef) -> TokenStream {
	let label = &btn.label;
	let disabled = btn.disabled;

	let class_attr = btn.class.as_deref().map(|c| {
		quote! { .attr("class", #c) }
	});

	let id_attr = btn.id.as_deref().map(|id| {
		quote! { .attr("id", #id) }
	});

	quote! {
		PageElement::new("button")
			.attr("type", "submit")
			#class_attr
			#id_attr
			.bool_attr("disabled", #disabled)
			.child(#label)
	}
}

fn generate_button_control_view(btn: &TypedButtonControlDef) -> TokenStream {
	let label = &btn.label;
	let button_type = match btn.kind {
		TypedButtonKind::Reset => "reset",
		TypedButtonKind::Button => "button",
	};
	let disabled = btn.disabled;
	let class_attr = btn.class.as_deref().map(|c| quote! { .attr("class", #c) });
	let id_attr = btn.id.as_deref().map(|id| quote! { .attr("id", #id) });

	quote! {
		PageElement::new("button")
			.attr("type", #button_type)
			#class_attr
			#id_attr
			.bool_attr("disabled", #disabled)
			.child(#label)
	}
}

fn generate_image_input_view(input: &TypedImageInputDef) -> TokenStream {
	let name = input.name.to_string();
	let src = &input.src;
	let alt = &input.alt;
	let disabled = input.disabled;
	let class_attr = input
		.class
		.as_deref()
		.map(|c| quote! { .attr("class", #c) });
	let id_attr = input.id.as_deref().map(|id| quote! { .attr("id", #id) });
	let width_attr = input.width.map(|width| {
		let width = width.to_string();
		quote! { .attr("width", #width) }
	});
	let height_attr = input.height.map(|height| {
		let height = height.to_string();
		quote! { .attr("height", #height) }
	});

	quote! {
		PageElement::new("input")
			.attr("type", "image")
			.attr("name", #name)
			.attr("src", #src)
			.attr("alt", #alt)
			.attr("aria-label", #alt)
			#class_attr
			#id_attr
			.bool_attr("disabled", #disabled)
			#width_attr
			#height_attr
	}
}

fn generate_output_view(output: &TypedOutputDef) -> TokenStream {
	let name = output.name.to_string();
	let for_attr = if output.for_fields.is_empty() {
		None
	} else {
		let for_value = output
			.for_fields
			.iter()
			.map(ToString::to_string)
			.collect::<Vec<_>>()
			.join(" ");
		Some(quote! { .attr("for", #for_value) })
	};
	let class_attr = output
		.class
		.as_deref()
		.map(|c| quote! { .attr("class", #c) });
	let id_attr = output.id.as_deref().map(|id| quote! { .attr("id", #id) });
	let label_child = output
		.label
		.as_deref()
		.map(|label| quote! { .child(#label) });

	quote! {
		PageElement::new("output")
			.attr("name", #name)
			#for_attr
			#class_attr
			#id_attr
			#label_child
	}
}

fn generate_meter_view(meter: &TypedMeterDef) -> TokenStream {
	let name = meter.name.to_string();
	let value = &meter.value;
	let min_attr = meter
		.min
		.as_ref()
		.map(|expr| quote! { .attr("min", (#expr).to_string()) });
	let max_attr = meter
		.max
		.as_ref()
		.map(|expr| quote! { .attr("max", (#expr).to_string()) });
	let low_attr = meter
		.low
		.as_ref()
		.map(|expr| quote! { .attr("low", (#expr).to_string()) });
	let high_attr = meter
		.high
		.as_ref()
		.map(|expr| quote! { .attr("high", (#expr).to_string()) });
	let optimum_attr = meter
		.optimum
		.as_ref()
		.map(|expr| quote! { .attr("optimum", (#expr).to_string()) });
	let class_attr = meter
		.class
		.as_deref()
		.map(|c| quote! { .attr("class", #c) });
	let id_attr = meter.id.as_deref().map(|id| quote! { .attr("id", #id) });
	let label_child = meter
		.label
		.as_deref()
		.map(|label| quote! { .child(#label) });

	quote! {
		PageElement::new("meter")
			.attr("name", #name)
			.attr("value", (#value).to_string())
			#min_attr
			#max_attr
			#low_attr
			#high_attr
			#optimum_attr
			#class_attr
			#id_attr
			#label_child
	}
}

fn generate_progress_view(progress: &TypedProgressDef) -> TokenStream {
	let name = progress.name.to_string();
	let value_attr = progress
		.value
		.as_ref()
		.map(|expr| quote! { .attr("value", (#expr).to_string()) });
	let max_attr = progress
		.max
		.as_ref()
		.map(|expr| quote! { .attr("max", (#expr).to_string()) });
	let class_attr = progress
		.class
		.as_deref()
		.map(|c| quote! { .attr("class", #c) });
	let id_attr = progress.id.as_deref().map(|id| quote! { .attr("id", #id) });
	let label_child = progress
		.label
		.as_deref()
		.map(|label| quote! { .child(#label) });

	quote! {
		PageElement::new("progress")
			.attr("name", #name)
			#value_attr
			#max_attr
			#class_attr
			#id_attr
			#label_child
	}
}

fn generate_datalist_view(datalist: &TypedDatalistDef, pages_crate: &TokenStream) -> TokenStream {
	let id = datalist.name.to_string();
	if datalist.choices_config.is_some() {
		let choices_name =
			syn::Ident::new(&format!("{}_choices", datalist.name), datalist.name.span());
		return quote! {
			{
				let __choices_signal = self.#choices_name.clone();
				PageElement::new("datalist")
					.attr("id", #id)
					.child(#pages_crate::component::Page::reactive(move || {
						let __choices = __choices_signal.get();
						let mut __children = ::std::vec::Vec::new();
						for choice in __choices.into_iter() {
							__children.push(
								PageElement::new("option")
									.attr("value", choice.value)
									.bool_attr("disabled", choice.disabled)
									.child(choice.label)
									.into_page(),
							);
						}
						#pages_crate::component::Page::Fragment(__children)
					}))
			}
		};
	}

	let option_children: Vec<TokenStream> = datalist
		.static_choices
		.iter()
		.map(generate_static_choice_item_view)
		.collect();

	quote! {
		PageElement::new("datalist")
			.attr("id", #id)
			#(.child(#option_children))*
	}
}

fn generate_static_choice_item_view(choice: &TypedChoiceItem) -> TokenStream {
	match choice {
		TypedChoiceItem::Option(option) => generate_static_choice_option_view(option),
		TypedChoiceItem::Group(group) => {
			let label = &group.label;
			let disabled = group.disabled;
			let option_children: Vec<TokenStream> = group
				.options
				.iter()
				.map(generate_static_choice_option_view)
				.collect();

			quote! {
				PageElement::new("optgroup")
					.attr("label", #label)
					.bool_attr("disabled", #disabled)
					#(.child(#option_children))*
			}
		}
	}
}

fn generate_static_choice_option_view(option: &TypedChoiceOption) -> TokenStream {
	let value = &option.value;
	let label = &option.label;
	let disabled = option.disabled;

	quote! {
		PageElement::new("option")
			.attr("value", (#value).to_string())
			.bool_attr("disabled", #disabled)
			.child((#label).to_string())
	}
}

/// Generates view code for a field group.
///
/// Wraps the group's fields in a container div with optional label and CSS class.
fn generate_field_group_view(
	group: &TypedFormFieldGroup,
	pages_crate: &TokenStream,
	all_fields: &[&TypedFormFieldDef],
) -> TokenStream {
	let group_class = group.class.as_deref().unwrap_or("reinhardt-field-group");

	// Generate field views for the group
	let field_views: Vec<TokenStream> = group
		.fields
		.iter()
		.map(|entry| generate_field_entry_view(entry, pages_crate, all_fields))
		.collect();

	// Generate optional label
	let label_element = if let Some(label) = &group.label {
		quote! {
			.child(PageElement::new("legend")
				.attr("class", "reinhardt-field-group-label")
				.child(#label))
		}
	} else {
		TokenStream::new()
	};

	quote! {
	PageElement::new("fieldset")
		.attr("class", #group_class)
		#label_element
			#(.child(#field_views))*
	}
}

fn generate_collection_view(
	collection: &TypedFormFieldCollection,
	pages_crate: &TokenStream,
) -> TokenStream {
	let collection_name = &collection.name;
	let collection_name_str = collection.name.to_string();
	let collection_class = collection
		.class
		.as_deref()
		.unwrap_or("reinhardt-field-array");
	let item_fields = collect_scalar_fields(&collection.fields);
	let item_field_views: Vec<TokenStream> = item_fields
		.iter()
		.map(|field| generate_collection_field_view(field, pages_crate, &collection_name_str))
		.collect();
	let legend = collection.label.as_deref().unwrap_or(&collection_name_str);
	let min_items_attr = collection.min_items.map(|min| {
		let value = min.to_string();
		quote! { .attr("data-reinhardt-min-items", #value) }
	});
	let max_items_attr = collection.max_items.map(|max| {
		let value = max.to_string();
		quote! { .attr("data-reinhardt-max-items", #value) }
	});

	quote! {
		{
			let __collection_signal = self.#collection_name.clone();
			let __path_signals = self.__path_signals.clone();
			#pages_crate::component::Page::reactive(move || {
				let _ = &__path_signals;
				let __items = __collection_signal.get();
				let mut __item_pages = ::std::vec::Vec::new();
				for item in __items {
					let item_value = item.value().clone();
					let __item_key = ::std::format!("{:?}", item.key());
					__item_pages.push((
						__item_key.clone(),
						PageElement::new("div")
							.attr("class", "reinhardt-field-array-item")
							.attr("data-reinhardt-field-array-item", #collection_name_str)
							.attr("data-reinhardt-item-index", item.index().to_string())
							.attr("data-reinhardt-item-key", __item_key)
							#(.child(#item_field_views))*
							.into_page(),
					));
				}

				PageElement::new("fieldset")
					.attr("class", #collection_class)
					.attr("data-reinhardt-field-array", #collection_name_str)
					#min_items_attr
					#max_items_attr
					.child(
						PageElement::new("legend")
							.attr("class", "reinhardt-field-array-label")
							.child(#legend)
					)
					.child(#pages_crate::component::Page::keyed_fragment(__item_pages))
					.into_page()
			})
		}
	}
}

fn generate_collection_field_view(
	field: &TypedFormFieldDef,
	pages_crate: &TokenStream,
	collection_name: &str,
) -> TokenStream {
	let field_name_str = field.name.to_string();
	let input_type = widget_to_input_type(&field.widget);
	let label_text = field.display.label.as_deref().unwrap_or(&field_name_str);
	let placeholder = field.display.placeholder.as_deref().unwrap_or("");
	let required = field.validation.required;
	let autocomplete_attr = field.display.autocomplete.as_deref().map(|val| {
		quote! { .attr("autocomplete", #val) }
	});
	let wrapper_class = field.styling.wrapper_class();
	let label_class = field.styling.label_class();
	let input_class = field.styling.input_class();
	let custom_attrs = generate_custom_attrs(&field.custom_attrs);
	let listener = generate_collection_bind_listener(field, pages_crate, collection_name);
	let field_value = collection_field_value_expr(field);
	let value_attr = if matches!(
		field.field_type,
		TypedFieldType::FileField | TypedFieldType::ImageField
	) {
		quote! {}
	} else {
		quote! { .attr("value", #field_value) }
	};
	let checked_attr = if matches!(field.widget, TypedWidget::CheckboxInput) {
		let checked = collection_field_checked_expr(field);
		quote! { .bool_attr("checked", #checked) }
	} else {
		quote! {}
	};

	let input_element = match &field.widget {
		TypedWidget::Textarea => {
			quote! {
				PageElement::new("textarea")
					.attr("name", __field_name.clone())
					.attr("id", __field_id.clone())
					.attr("class", #input_class)
					.attr("placeholder", #placeholder)
					.bool_attr("required", #required)
					#autocomplete_attr
					#custom_attrs
					#listener
					.child(#field_value)
			}
		}
		TypedWidget::Select | TypedWidget::SelectMultiple => {
			let multiple = matches!(field.widget, TypedWidget::SelectMultiple);
			quote! {
				PageElement::new("select")
					.attr("name", __field_name.clone())
					.attr("id", __field_id.clone())
					.attr("class", #input_class)
					.bool_attr("required", #required)
					.bool_attr("multiple", #multiple)
					#autocomplete_attr
					#custom_attrs
					#listener
			}
		}
		TypedWidget::CheckboxInput => {
			quote! {
				PageElement::new("input")
					.attr("type", "checkbox")
					.attr("name", __field_name.clone())
					.attr("id", __field_id.clone())
					.attr("class", #input_class)
					.attr("value", "true")
					.bool_attr("required", #required)
					#checked_attr
					#custom_attrs
					#listener
			}
		}
		_ => {
			quote! {
				PageElement::new("input")
					.attr("type", #input_type)
					.attr("name", __field_name.clone())
					.attr("id", __field_id.clone())
					.attr("class", #input_class)
					.attr("placeholder", #placeholder)
					.bool_attr("required", #required)
					#value_attr
					#autocomplete_attr
					#custom_attrs
					#listener
			}
		}
	};

	let label_element = if matches!(field.widget, TypedWidget::HiddenInput) {
		quote! {}
	} else {
		quote! {
			.child(
				PageElement::new("label")
					.attr("for", __field_id.clone())
					.attr("class", #label_class)
					.child(#label_text)
			)
		}
	};

	if matches!(field.widget, TypedWidget::HiddenInput) {
		quote! {
			{
				let __field_name = ::std::format!(
					"{}[{}][{}]",
					#collection_name,
					item.index(),
					#field_name_str,
				);
				let __field_id = ::std::format!(
					"{}_{}_{}",
					#collection_name,
					item.index(),
					#field_name_str,
				);
				#input_element
			}
		}
	} else {
		let wrapper_attrs = generate_wrapper_attrs(&field.wrapper, wrapper_class);
		let wrapper_tag = field
			.wrapper
			.as_ref()
			.map(|w| w.tag.as_str())
			.unwrap_or("div");
		quote! {
			{
				let __field_name = ::std::format!(
					"{}[{}][{}]",
					#collection_name,
					item.index(),
					#field_name_str,
				);
				let __field_id = ::std::format!(
					"{}_{}_{}",
					#collection_name,
					item.index(),
					#field_name_str,
				);
				PageElement::new(#wrapper_tag)
					#wrapper_attrs
					#label_element
					.child(#input_element)
			}
		}
	}
}

fn collection_field_value_expr(field: &TypedFormFieldDef) -> TokenStream {
	let field_name = &field.name;
	match &field.field_type {
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField => quote! { item_value.#field_name.clone() },
		TypedFieldType::DateField
		| TypedFieldType::TimeField
		| TypedFieldType::UuidField
		| TypedFieldType::IpAddressField => quote! {
			item_value
				.#field_name
				.as_ref()
				.map(::std::string::ToString::to_string)
				.unwrap_or_default()
		},
		TypedFieldType::DateTimeField => quote! {
			item_value
				.#field_name
				.as_ref()
				.map(|value| value.format("%Y-%m-%dT%H:%M:%S").to_string())
				.unwrap_or_default()
		},
		TypedFieldType::MultipleChoiceField { .. } => quote! {
			item_value
				.#field_name
				.iter()
				.map(::std::string::ToString::to_string)
				.collect::<::std::vec::Vec<_>>()
				.join(",")
		},
		TypedFieldType::JsonField { .. } => quote! {
			::serde_json::to_string(&item_value.#field_name).unwrap_or_default()
		},
		TypedFieldType::FileField | TypedFieldType::ImageField => {
			quote! { ::std::string::String::new() }
		}
		TypedFieldType::IntegerField
		| TypedFieldType::FloatField
		| TypedFieldType::DecimalField
		| TypedFieldType::BooleanField
		| TypedFieldType::HiddenField { .. }
		| TypedFieldType::ChoiceField { .. } => {
			quote! { item_value.#field_name.to_string() }
		}
	}
}

fn collection_field_checked_expr(field: &TypedFormFieldDef) -> TokenStream {
	let field_name = &field.name;
	match &field.field_type {
		TypedFieldType::BooleanField => quote! { item_value.#field_name },
		_ => quote! { false },
	}
}

fn generate_collection_bind_listener(
	field: &TypedFormFieldDef,
	pages_crate: &TokenStream,
	collection_name: &str,
) -> TokenStream {
	let field_name = &field.name;
	let field_name_str = field.name.to_string();
	let field_type = field_type_to_value_type(&field.field_type);
	let collection_name_str = collection_name.to_string();
	let (event_type, payload_type) = match field.widget {
		TypedWidget::Textarea => (
			quote! { #pages_crate::event::KnownEvent::Input },
			quote! { #pages_crate::event::InputEvent },
		),
		TypedWidget::Select
		| TypedWidget::SelectMultiple
		| TypedWidget::CheckboxInput
		| TypedWidget::RadioSelect => (
			quote! { #pages_crate::event::KnownEvent::Change },
			quote! { #pages_crate::event::ChangeEvent },
		),
		_ => (
			quote! { #pages_crate::event::KnownEvent::Input },
			quote! { #pages_crate::event::InputEvent },
		),
	};
	let assignment = quote! {
		let mut __items = __field_collection_signal.get();
		if let ::core::option::Option::Some(__position) =
			__items.iter().position(|entry| entry.key() == __item_key)
		{
			let __item_index = __items[__position].index();
			let mut __item_value = __items[__position].value().clone();
			let __path_value = __new_value.clone();
			__item_value.#field_name = __new_value;
			__items[__position] = #pages_crate::CollectionItem::new(
				__item_key,
				__item_index,
				__item_value,
			);
			__field_collection_signal.set(__items);
			let __path_key = ::std::format!(
				"{}:{:?}:{}",
				#collection_name_str,
				__item_key,
				#field_name_str,
			);
			let __path_signal = __field_path_signals
				.borrow()
				.get(&__path_key)
				.and_then(|signal| {
					signal
						.downcast_ref::<#pages_crate::reactive::Signal<#field_type>>()
						.cloned()
				});
			if let ::core::option::Option::Some(__path_signal) = __path_signal {
				__path_signal.set(__path_value);
			}
		}
	};
	let typed_update = collection_field_typed_update(
		&field.field_type,
		&field.widget,
		pages_crate,
		&field_name_str,
		assignment,
	);

	quote! {
		.on(#event_type, {
			let __field_collection_signal = __collection_signal.clone();
			let __field_path_signals = __path_signals.clone();
			let __item_key = item.key();
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			{
				#pages_crate::typed_event_handler::<#payload_type, _>(
					move |event: #payload_type| {
						#typed_update
					},
				)
			}
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			{
				::std::sync::Arc::new(move |_event: #pages_crate::component::NativeEvent| {
					let _ = &__field_collection_signal;
					let _ = &__field_path_signals;
					let _ = __item_key;
				})
			}
		})
	}
}

fn collection_field_typed_update(
	field_type: &TypedFieldType,
	widget: &TypedWidget,
	pages_crate: &TokenStream,
	field_name: &str,
	assignment: TokenStream,
) -> TokenStream {
	if matches!(field_type, TypedFieldType::BooleanField)
		&& matches!(widget, TypedWidget::CheckboxInput)
	{
		return quote! {
			match event.checked() {
				::core::result::Result::Ok(__new_value) => { #assignment }
				::core::result::Result::Err(__error) => {
					#pages_crate::warn_log!(
						"form collection field `{}` failed to extract `checked`: {}",
						#field_name,
						__error,
					);
				}
			}
		};
	}

	if let TypedFieldType::MultipleChoiceField { inner } = field_type {
		return quote! {
			match event.selected_values() {
				::core::result::Result::Ok(__selected_values) => {
					let __new_value = __selected_values
						.into_iter()
						.filter_map(|value| value.parse::<#inner>().ok())
						.collect::<::std::vec::Vec<_>>();
					#assignment
				}
				::core::result::Result::Err(__error) => {
					#pages_crate::warn_log!(
						"form collection field `{}` failed to extract `selected_values`: {}",
						#field_name,
						__error,
					);
				}
			}
		};
	}

	if matches!(
		field_type,
		TypedFieldType::FileField | TypedFieldType::ImageField
	) {
		return quote! {
			match event.files() {
				::core::result::Result::Ok(__files) => {
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					{
						let __new_value = __files.first().map(|file| file.raw().clone());
						#assignment
					}
					#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
					{
						let _ = (
							&__field_collection_signal,
							&__field_path_signals,
							__item_key,
							__files,
						);
					}
				}
				::core::result::Result::Err(__error) => {
					#pages_crate::warn_log!(
						"form collection field `{}` failed to extract `files`: {}",
						#field_name,
						__error,
					);
				}
			}
		};
	}

	let raw_value = quote::format_ident!("__raw_value");
	let value_update = collection_field_value_update(field_type, &raw_value, assignment);
	quote! {
		match event.value() {
			::core::result::Result::Ok(#raw_value) => { #value_update }
			::core::result::Result::Err(__error) => {
				#pages_crate::warn_log!(
					"form collection field `{}` failed to extract `value`: {}",
					#field_name,
					__error,
				);
			}
		}
	}
}

fn collection_field_value_update(
	field_type: &TypedFieldType,
	raw_value: &syn::Ident,
	assignment: TokenStream,
) -> TokenStream {
	match field_type {
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField => quote! {
			let __new_value = #raw_value;
			#assignment
		},
		TypedFieldType::BooleanField => quote! {
			if let ::core::result::Result::Ok(__new_value) = #raw_value.parse::<bool>()
			{
				#assignment
			}
		},
		TypedFieldType::IntegerField => quote! {
			if let ::core::result::Result::Ok(__new_value) =
				#raw_value.parse::<i64>()
			{
				#assignment
			}
		},
		TypedFieldType::FloatField | TypedFieldType::DecimalField => quote! {
			if let ::core::result::Result::Ok(__new_value) =
				#raw_value.parse::<f64>()
			{
				#assignment
			}
		},
		TypedFieldType::DateField => {
			collection_option_parse_update(raw_value, quote! { chrono::NaiveDate }, assignment)
		}
		TypedFieldType::TimeField => {
			collection_option_parse_update(raw_value, quote! { chrono::NaiveTime }, assignment)
		}
		TypedFieldType::DateTimeField => collection_datetime_parse_update(raw_value, assignment),
		TypedFieldType::UuidField => {
			collection_option_parse_update(raw_value, quote! { uuid::Uuid }, assignment)
		}
		TypedFieldType::IpAddressField => {
			collection_option_parse_update(raw_value, quote! { ::std::net::IpAddr }, assignment)
		}
		TypedFieldType::HiddenField { inner } | TypedFieldType::ChoiceField { inner } => quote! {
			if let ::core::result::Result::Ok(__new_value) =
				#raw_value.parse::<#inner>()
			{
				#assignment
			}
		},
		TypedFieldType::JsonField { inner } => quote! {
			if let ::core::result::Result::Ok(__new_value) =
				::serde_json::from_str::<#inner>(&#raw_value)
			{
				#assignment
			}
		},
		TypedFieldType::MultipleChoiceField { .. }
		| TypedFieldType::FileField
		| TypedFieldType::ImageField => quote! {},
	}
}

fn collection_option_parse_update(
	raw_value: &syn::Ident,
	value_type: TokenStream,
	assignment: TokenStream,
) -> TokenStream {
	quote! {
		let __raw = #raw_value;
		if __raw.is_empty() {
			let __new_value = ::core::option::Option::None;
			#assignment
		} else if let ::core::result::Result::Ok(__parsed) =
			__raw.parse::<#value_type>()
		{
			let __new_value = ::core::option::Option::Some(__parsed);
			#assignment
		}
	}
}

fn collection_datetime_parse_update(
	raw_value: &syn::Ident,
	assignment: TokenStream,
) -> TokenStream {
	quote! {
		let __raw = #raw_value;
		if __raw.is_empty() {
			let __new_value = ::core::option::Option::None;
			#assignment
		} else if let ::core::result::Result::Ok(__parsed) =
			chrono::NaiveDateTime::parse_from_str(&__raw, "%Y-%m-%dT%H:%M:%S")
				.or_else(|_| chrono::NaiveDateTime::parse_from_str(&__raw, "%Y-%m-%dT%H:%M"))
		{
			let __new_value = ::core::option::Option::Some(__parsed);
			#assignment
		}
	}
}

/// Generates view code for a single field.
///
/// # Arguments
///
/// - `field`: The typed field definition
/// - `pages_crate`: The reinhardt-pages crate path
/// - `signal_ident`: Optional signal identifier for two-way binding. If provided,
///   generates an input event listener that updates the signal with the input value.
fn generate_field_view(
	field: &TypedFormFieldDef,
	pages_crate: &TokenStream,
	signal_ident: Option<&syn::Ident>,
) -> TokenStream {
	let field_name = &field.name;
	let field_name_str = field_name.to_string();
	let input_type = widget_to_input_type(&field.widget);
	let label_text = field.display.label.as_deref().unwrap_or(&field_name_str);
	let placeholder = field.display.placeholder.as_deref().unwrap_or("");
	let required = field.validation.required;

	let autocomplete_attr = field.display.autocomplete.as_deref().map(|val| {
		quote! { .attr("autocomplete", #val) }
	});

	let wrapper_class = field.styling.wrapper_class();
	let label_class = field.styling.label_class();
	let input_class = field.styling.input_class();

	// Generate custom attributes (aria-*, data-*)
	let custom_attrs = generate_custom_attrs(&field.custom_attrs);
	let native_attrs = generate_native_attrs(&field.native_attrs);

	// Generate event listener for two-way binding
	let event_listener = generate_bind_listener(
		signal_ident,
		&field.widget,
		&field.field_type,
		pages_crate,
		&field_name_str,
	);

	// Generate input element based on widget type
	let input_element = match &field.widget {
		TypedWidget::CustomExperimental(custom) => {
			let component = &custom.component;
			let adapter = &custom.adapter;
			let value_type = field_type_to_value_type(&field.field_type);
			let disabled = field.display.disabled;
			let custom_widget_error_name = format_ident!(
				"__{}_custom_widget_error",
				field_name,
				span = field_name.span()
			);
			let custom_widget_touched_name =
				custom_widget_touched_ident(field).expect("custom widget touched signal");
			quote! {
					{
						let __field_signal = self.#field_name.clone();
						let __custom_widget_error_signal = self.#custom_widget_error_name.clone();
						let __custom_widget_touched_signal =
							self.#custom_widget_touched_name.clone();
						let mut __ctx: #pages_crate::CustomWidgetContext<#value_type> =
							#pages_crate::CustomWidgetContext::new(
								__field_signal.get(),
								::std::rc::Rc::new({
									let __field_signal = __field_signal.clone();
									let __custom_widget_error_signal =
										__custom_widget_error_signal.clone();
									let __custom_widget_touched_signal =
										__custom_widget_touched_signal.clone();
									move |__raw: #pages_crate::CustomWidgetRawValue| {
										__custom_widget_touched_signal.set(true);
										match <#adapter as #pages_crate::FormWidgetAdapter<#value_type>>::parse(__raw) {
											::core::result::Result::Ok(__value) => {
												__custom_widget_error_signal
												.set(::core::option::Option::None);
											__field_signal.set(__value);
											::core::result::Result::Ok(())
										}
										::core::result::Result::Err(__error) => {
											__custom_widget_error_signal.set(
												::core::option::Option::Some(
													#pages_crate::FieldError::new(__error.message()),
												),
											);
											::core::result::Result::Err(__error)
										}
									}
									}
								}),
							);
						__ctx.disabled = #disabled;
						__ctx.required = #required;
						__ctx.touched = __custom_widget_touched_signal.get();
						__ctx.error = __custom_widget_error_signal.get();
						__ctx.name = #field_name_str.to_string();
					__ctx.id = #field_name_str.to_string();
					let __props =
						<#adapter as #pages_crate::FormWidgetAdapter<#value_type>>::props(__ctx);
					#component(__props)
				}
			}
		}
		TypedWidget::Textarea => {
			quote! {
				PageElement::new("textarea")
					.attr("name", #field_name_str)
					.attr("id", #field_name_str)
					.attr("class", #input_class)
					.attr("placeholder", #placeholder)
						.bool_attr("required", #required)
						#autocomplete_attr
						#native_attrs
						#custom_attrs
						#event_listener
			}
		}
		TypedWidget::Select | TypedWidget::SelectMultiple => {
			let multiple = matches!(field.widget, TypedWidget::SelectMultiple);
			let choice_children = if field.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				let choice_items_name =
					syn::Ident::new(&format!("{}_choice_items", field.name), field.name.span());
				quote! {
						.child({
							let __choices_signal = self.#choices_name.clone();
							let __choice_items_signal = self.#choice_items_name.clone();
							#pages_crate::component::Page::reactive(move || {
								let __choice_items = __choice_items_signal.get();
								let __choices: ::std::vec::Vec<_> = __choices_signal
									.get()
									.into_iter()
									.enumerate()
									.map(|(i, (value, label))| {
										match __choice_items.get(i) {
											::core::option::Option::Some(__metadata)
												if __metadata.value.to_string() == value.to_string()
													&& __metadata.label.as_str() == label.as_str() =>
											{
												__ReinhardtChoiceItem {
													value,
													label,
													disabled: __metadata.disabled,
													group: __metadata.group.clone(),
													group_disabled: __metadata.group_disabled,
												}
											}
											_ => __ReinhardtChoiceItem {
												value,
												label,
												disabled: false,
												group: ::core::option::Option::None,
												group_disabled: false,
											},
										}
									})
									.collect();
								let mut __children = ::std::vec::Vec::new();
								let mut __current_group: ::core::option::Option<(
									::std::string::String,
								bool,
								::std::vec::Vec<#pages_crate::component::Page>,
							)> = ::core::option::Option::None;

							for choice in __choices.into_iter() {
								let __option = PageElement::new("option")
									.attr("value", choice.value.to_string())
									.bool_attr("disabled", choice.disabled)
									.child(choice.label)
									.into_page();

								if let ::core::option::Option::Some(__group_label) = choice.group {
									let __same_group = __current_group
										.as_ref()
										.is_some_and(|(__label, __disabled, _)| {
											__label == &__group_label
												&& *__disabled == choice.group_disabled
										});

									if __same_group {
										if let ::core::option::Option::Some((_, _, __options)) =
											__current_group.as_mut()
										{
											__options.push(__option);
										}
									} else {
										if let ::core::option::Option::Some((
											__label,
											__disabled,
											__options,
										)) = __current_group.take()
										{
											__children.push(
												PageElement::new("optgroup")
													.attr("label", __label)
													.bool_attr("disabled", __disabled)
													.children(__options)
													.into_page(),
											);
										}
										__current_group = ::core::option::Option::Some((
											__group_label,
											choice.group_disabled,
											::std::vec![__option],
										));
									}
								} else {
									if let ::core::option::Option::Some((
										__label,
										__disabled,
										__options,
									)) = __current_group.take()
									{
										__children.push(
											PageElement::new("optgroup")
												.attr("label", __label)
												.bool_attr("disabled", __disabled)
												.children(__options)
												.into_page(),
										);
									}
									__children.push(__option);
								}
							}

							if let ::core::option::Option::Some((
								__label,
								__disabled,
								__options,
							)) = __current_group.take()
							{
								__children.push(
									PageElement::new("optgroup")
										.attr("label", __label)
										.bool_attr("disabled", __disabled)
										.children(__options)
										.into_page(),
								);
							}

							#pages_crate::component::Page::Fragment(__children)
						})
					})
				}
			} else {
				let choice_children: Vec<TokenStream> = field
					.static_choices
					.iter()
					.map(generate_static_choice_item_view)
					.collect();
				quote! {
					#(.child(#choice_children))*
				}
			};
			quote! {
				PageElement::new("select")
					.attr("name", #field_name_str)
					.attr("id", #field_name_str)
					.attr("class", #input_class)
					.bool_attr("required", #required)
					.bool_attr("multiple", #multiple)
					#autocomplete_attr
					#native_attrs
					#custom_attrs
					#event_listener
					#choice_children
			}
		}
		TypedWidget::CheckboxInput => {
			quote! {
				PageElement::new("input")
					.attr("type", "checkbox")
					.attr("name", #field_name_str)
					.attr("id", #field_name_str)
						.attr("class", #input_class)
						.bool_attr("required", #required)
						#native_attrs
						#custom_attrs
						#event_listener
			}
		}
		TypedWidget::RadioSelect if field.choices_config.is_some() => {
			let choices_name =
				syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
			let choice_items_name =
				syn::Ident::new(&format!("{}_choice_items", field.name), field.name.span());
			let checked_attr = signal_ident.map(|signal_ident| {
				quote! {
						.bool_attr(
						"checked",
						#signal_ident.get().to_string() == choice_value.to_string(),
					)
				}
			});
			quote! {
					{
						let __choices_signal = self.#choices_name.clone();
						let __choice_items_signal = self.#choice_items_name.clone();
						#pages_crate::component::Page::reactive(move || {
							let __choice_items = __choice_items_signal.get();
							let __choices: ::std::vec::Vec<_> = __choices_signal
								.get()
								.into_iter()
								.enumerate()
								.map(|(i, (value, label))| {
									match __choice_items.get(i) {
										::core::option::Option::Some(__metadata)
											if __metadata.value.to_string() == value.to_string()
												&& __metadata.label.as_str() == label.as_str() =>
										{
											__ReinhardtChoiceItem {
												value,
												label,
												disabled: __metadata.disabled,
												group: __metadata.group.clone(),
												group_disabled: __metadata.group_disabled,
											}
										}
										_ => __ReinhardtChoiceItem {
											value,
											label,
											disabled: false,
											group: ::core::option::Option::None,
											group_disabled: false,
										},
									}
								})
								.collect();
							let mut __children = ::std::vec::Vec::new();
							for (i, choice) in __choices.into_iter().enumerate() {
							let _ = (&choice.group, choice.group_disabled);
							let choice_value = choice.value;
							let __choice_id =
								::std::format!("{}_{}", #field_name_str, i);
							__children.push(
								PageElement::new("label")
									.attr("for", __choice_id.clone())
									.child(
										PageElement::new("input")
											.attr("type", "radio")
											.attr("name", #field_name_str)
											.attr("id", __choice_id)
											.attr("value", choice_value.to_string())
											.attr("class", #input_class)
											.bool_attr("required", #required)
											.bool_attr("disabled", choice.disabled)
											#checked_attr
											#native_attrs
											#custom_attrs
											#event_listener
									)
									.child(choice.label)
									.into_page(),
							);
						}
						#pages_crate::component::Page::Fragment(__children)
					})
				}
			}
		}
		TypedWidget::RadioSelect => {
			quote! {
				PageElement::new("input")
					.attr("type", "radio")
					.attr("name", #field_name_str)
					.attr("id", #field_name_str)
						.attr("class", #input_class)
						.bool_attr("required", #required)
						#native_attrs
						#custom_attrs
						#event_listener
			}
		}
		_ => {
			// Standard input element
			quote! {
				PageElement::new("input")
					.attr("type", #input_type)
					.attr("name", #field_name_str)
					.attr("id", #field_name_str)
					.attr("class", #input_class)
						.attr("placeholder", #placeholder)
						.bool_attr("required", #required)
						#autocomplete_attr
						#native_attrs
						#custom_attrs
						#event_listener
			}
		}
	};

	// Determine icon position and generate icon element if present
	let (icon_in_label, icon_left, icon_right) = if let Some(icon) = &field.icon {
		let icon_element = generate_icon_element(icon);
		match icon.position {
			TypedIconPosition::Label => (
				Some(quote! { .child(#icon_element) }),
				TokenStream::new(),
				TokenStream::new(),
			),
			TypedIconPosition::Left => (None, quote! { .child(#icon_element) }, TokenStream::new()),
			TypedIconPosition::Right => {
				(None, TokenStream::new(), quote! { .child(#icon_element) })
			}
		}
	} else {
		(None, TokenStream::new(), TokenStream::new())
	};

	// Generate label element (skip for hidden inputs)
	let label_element = if matches!(field.widget, TypedWidget::HiddenInput) {
		quote! {}
	} else {
		// If icon position is Label, include the icon inside the label
		let icon_child = icon_in_label.unwrap_or_default();
		quote! {
			.child(
				PageElement::new("label")
					.attr("for", #field_name_str)
					.attr("class", #label_class)
					#icon_child
					.child(#label_text)
			)
		}
	};

	// Wrapper element (skip for hidden inputs)
	if matches!(field.widget, TypedWidget::HiddenInput) {
		input_element
	} else {
		// Use custom wrapper if specified, otherwise default to div
		let wrapper_attrs = generate_wrapper_attrs(&field.wrapper, wrapper_class);
		let wrapper_tag = field
			.wrapper
			.as_ref()
			.map(|w| w.tag.as_str())
			.unwrap_or("div");

		quote! {
			PageElement::new(#wrapper_tag)
				#wrapper_attrs
				#label_element
				#icon_left
				.child(#input_element)
				#icon_right
		}
	}
}

/// Generates wrapper element attributes.
///
/// If a custom wrapper is specified, uses its attributes (merging with default class if needed).
/// Otherwise, uses the default wrapper_class.
fn generate_wrapper_attrs(wrapper: &Option<TypedWrapper>, default_class: &str) -> TokenStream {
	match wrapper {
		Some(w) => {
			// Generate attributes from custom wrapper
			let mut attrs = TokenStream::new();
			let mut has_class = false;

			for attr in &w.attrs {
				let name = &attr.name;
				let value = &attr.value;
				attrs.extend(quote! {
					.attr(#name, #value)
				});
				if name == "class" {
					has_class = true;
				}
			}

			// If no class was specified in custom wrapper, add default wrapper class
			if !has_class {
				attrs.extend(quote! {
					.attr("class", #default_class)
				});
			}

			attrs
		}
		None => {
			// Use default wrapper class
			quote! {
				.attr("class", #default_class)
			}
		}
	}
}

/// Generates SVG icon element code from a TypedIcon.
///
/// Creates an PageElement for the SVG element with all attributes and child elements.
fn generate_icon_element(icon: &TypedIcon) -> TokenStream {
	let attrs = generate_icon_attrs(&icon.attrs);
	let children = generate_icon_children(&icon.children);

	quote! {
		PageElement::new("svg")
			#attrs
			#children
	}
}

/// Generates attribute code for icon elements.
fn generate_icon_attrs(attrs: &[reinhardt_manouche::core::TypedIconAttr]) -> TokenStream {
	let mut result = TokenStream::new();
	for attr in attrs {
		let name = &attr.name;
		let value = &attr.value;
		result.extend(quote! {
			.attr(#name, #value)
		});
	}
	result
}

/// Generates child element code for icon elements (recursive for nested groups).
fn generate_icon_children(children: &[TypedIconChild]) -> TokenStream {
	let mut result = TokenStream::new();
	for child in children {
		let tag = &child.tag;
		let attrs = generate_icon_child_attrs(&child.attrs);
		let nested_children = generate_icon_children(&child.children);

		result.extend(quote! {
			.child(
				PageElement::new(#tag)
					#attrs
					#nested_children
			)
		});
	}
	result
}

/// Generates attribute code for icon child elements.
fn generate_icon_child_attrs(attrs: &[reinhardt_manouche::core::TypedIconAttr]) -> TokenStream {
	let mut result = TokenStream::new();
	for attr in attrs {
		let name = &attr.name;
		let value = &attr.value;
		result.extend(quote! {
			.attr(#name, #value)
		});
	}
	result
}

/// Generates custom attribute code (aria-*, data-*) for form field input elements.
///
/// Converts underscores in attribute names to hyphens for HTML output.
/// For example, `aria_label` becomes `aria-label`.
fn generate_custom_attrs(attrs: &[TypedCustomAttr]) -> TokenStream {
	let mut result = TokenStream::new();
	for attr in attrs {
		let html_name = attr.html_name(); // Convert underscores to hyphens
		let value = &attr.value;
		result.extend(quote! {
			.attr(#html_name, #value)
		});
	}
	result
}

/// Generates native HTML attribute code for form field input elements.
fn generate_native_attrs(attrs: &TypedFieldNativeAttrs) -> TokenStream {
	let mut result = TokenStream::new();

	if let Some(min) = &attrs.min {
		result.extend(quote! {
			.attr("min", (#min).to_string())
		});
	}
	if let Some(max) = &attrs.max {
		result.extend(quote! {
			.attr("max", (#max).to_string())
		});
	}
	if let Some(step) = &attrs.step {
		result.extend(quote! {
			.attr("step", (#step).to_string())
		});
	}
	if let Some(accept) = &attrs.accept {
		result.extend(quote! {
			.attr("accept", #accept)
		});
	}
	if let Some(capture) = &attrs.capture {
		result.extend(quote! {
			.attr("capture", #capture)
		});
	}
	if let Some(list) = &attrs.list {
		let list_id = list.to_string();
		result.extend(quote! {
			.attr("list", #list_id)
		});
	}
	if let Some(size) = attrs.size {
		let size_value = size.to_string();
		result.extend(quote! {
			.attr("size", #size_value)
		});
	}

	result
}

/// Generates an event listener for two-way binding.
///
/// When `signal_ident` is provided, generates a `.listener()` call that updates
/// the signal when the input value changes. The event type depends on the widget:
/// - Text inputs, textarea: "input" event
/// - Select, checkbox, radio: "change" event
///
/// The generated code handles both WASM and non-WASM environments using conditional
/// compilation. In WASM, it extracts the value from the DOM element and converts it
/// to the signal's inner Rust type. In non-WASM, the handler is a no-op for type
/// compatibility.
fn generate_bind_listener(
	signal_ident: Option<&syn::Ident>,
	widget: &TypedWidget,
	field_type: &TypedFieldType,
	pages_crate: &TokenStream,
	field_name: &str,
) -> TokenStream {
	let Some(signal_ident) = signal_ident else {
		return TokenStream::new();
	};

	if matches!(field_type, TypedFieldType::MultipleChoiceField { .. }) {
		return generate_multi_select_listener(signal_ident, field_type, pages_crate, field_name);
	}
	if matches!(
		field_type,
		TypedFieldType::FileField | TypedFieldType::ImageField
	) {
		return generate_file_input_listener(signal_ident, pages_crate, field_name);
	}

	let (event_type, payload_type) = match widget {
		TypedWidget::Textarea => (
			quote! { #pages_crate::event::KnownEvent::Input },
			quote! { #pages_crate::event::InputEvent },
		),
		TypedWidget::Select
		| TypedWidget::SelectMultiple
		| TypedWidget::CheckboxInput
		| TypedWidget::RadioSelect => (
			quote! { #pages_crate::event::KnownEvent::Change },
			quote! { #pages_crate::event::ChangeEvent },
		),
		_ => (
			quote! { #pages_crate::event::KnownEvent::Input },
			quote! { #pages_crate::event::InputEvent },
		),
	};
	let extraction = if matches!(field_type, TypedFieldType::BooleanField)
		&& matches!(widget, TypedWidget::CheckboxInput)
	{
		quote! {
			match event.checked() {
				::core::result::Result::Ok(__checked) => signal.set(__checked),
				::core::result::Result::Err(__error) => {
					#pages_crate::warn_log!(
						"form field `{}` failed to extract `checked`: {}",
						#field_name,
						__error,
					);
				}
			}
		}
	} else {
		let raw_value = quote::format_ident!("__raw_value");
		let conversion = generate_value_conversion(field_type, &raw_value);
		quote! {
			match event.value() {
				::core::result::Result::Ok(#raw_value) => {
					#conversion
				}
				::core::result::Result::Err(__error) => {
					#pages_crate::warn_log!(
						"form field `{}` failed to extract `value`: {}",
						#field_name,
						__error,
					);
				}
			}
		}
	};

	quote! {
		.on(#event_type, {
			let signal = #signal_ident.clone();
			#pages_crate::typed_event_handler::<#payload_type, _>(
				move |event: #payload_type| {
					#extraction
				},
			)
		})
	}
}

/// Generates the complete listener block for file-like fields.
fn generate_file_input_listener(
	signal_ident: &syn::Ident,
	pages_crate: &TokenStream,
	field_name: &str,
) -> TokenStream {
	let setup = quote! {
		let signal = #signal_ident.clone();
	};
	let selected_file = quote! {
		signal.set(files.first().map(|file| file.raw().clone()));
	};
	generate_file_change_listener(pages_crate, field_name, setup, selected_file)
}

fn generate_model_file_input_listener(pages_crate: &TokenStream) -> TokenStream {
	let setup = quote! {
		let form = self.clone();
	};
	let selected_file = quote! {
		match files.first() {
			::core::option::Option::Some(file) => {
				if let ::core::result::Result::Err(error) = form.set_file(field_name, file.raw().clone()) {
					#pages_crate::warn_log!(
						"model form field `{}` rejected file input: {}",
						field_name,
						error,
					);
				}
			}
			::core::option::Option::None => {
				if let ::core::result::Result::Err(error) = form.clear_file(field_name) {
					#pages_crate::warn_log!(
						"model form field `{}` could not clear file input: {}",
						field_name,
						error,
					);
				}
			}
		}
	};
	generate_file_change_listener(pages_crate, "model file", setup, selected_file)
}

fn generate_file_change_listener(
	pages_crate: &TokenStream,
	field_name: &str,
	setup: TokenStream,
	selected_file: TokenStream,
) -> TokenStream {
	quote! {
		.on(#pages_crate::event::KnownEvent::Change, {
			#setup
			#pages_crate::typed_event_handler::<#pages_crate::event::ChangeEvent, _>(
				move |event: #pages_crate::event::ChangeEvent| {
					match event.files() {
						::core::result::Result::Ok(files) => {
							#[cfg(all(target_family = "wasm", target_os = "unknown"))]
							{
								#selected_file
							}
							#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
							{
								let _ = files;
							}
						}
						::core::result::Result::Err(__error) => {
							#pages_crate::warn_log!(
								"form field `{}` failed to extract `files`: {}",
								#field_name,
								__error,
							);
						}
					}
				},
			)
		})
	}
}

/// Generates the value conversion code that transforms a DOM element value into
/// the appropriate Rust type for the signal. Parse failures are silently ignored,
/// preserving the previous signal value.
fn generate_value_conversion(field_type: &TypedFieldType, raw_value: &syn::Ident) -> TokenStream {
	match field_type {
		// String fields: direct assignment
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField => {
			quote! { signal.set(#raw_value); }
		}

		// BooleanField with other widgets (e.g. select "true"/"false")
		TypedFieldType::BooleanField => {
			quote! {
				if let Ok(v) = #raw_value.parse::<bool>() {
					signal.set(v);
				}
			}
		}

		// Numeric types
		TypedFieldType::IntegerField => {
			quote! {
				if let Ok(v) = #raw_value.parse::<i64>() {
					signal.set(v);
				}
			}
		}
		TypedFieldType::FloatField | TypedFieldType::DecimalField => {
			quote! {
				if let Ok(v) = #raw_value.parse::<f64>() {
					signal.set(v);
				}
			}
		}

		// Option-wrapped types: empty string → None, otherwise parse to Some
		TypedFieldType::DateField => {
			quote! {
				let raw = #raw_value;
				if raw.is_empty() {
					signal.set(::core::option::Option::None);
				} else if let Ok(v) = raw.parse::<chrono::NaiveDate>() {
					signal.set(::core::option::Option::Some(v));
				}
			}
		}
		TypedFieldType::TimeField => {
			quote! {
				let raw = #raw_value;
				if raw.is_empty() {
					signal.set(::core::option::Option::None);
				} else if let Ok(v) = raw.parse::<chrono::NaiveTime>() {
					signal.set(::core::option::Option::Some(v));
				}
			}
		}
		TypedFieldType::DateTimeField => {
			quote! {
				let raw = #raw_value;
				if raw.is_empty() {
					signal.set(::core::option::Option::None);
				} else if let Ok(v) =
					chrono::NaiveDateTime::parse_from_str(&raw, "%Y-%m-%dT%H:%M:%S")
						.or_else(|_| chrono::NaiveDateTime::parse_from_str(&raw, "%Y-%m-%dT%H:%M"))
				{
					signal.set(::core::option::Option::Some(v));
				}
			}
		}
		TypedFieldType::UuidField => {
			quote! {
				let raw = #raw_value;
				if raw.is_empty() {
					signal.set(::core::option::Option::None);
				} else if let Ok(v) = raw.parse::<uuid::Uuid>() {
					signal.set(::core::option::Option::Some(v));
				}
			}
		}
		TypedFieldType::IpAddressField => {
			quote! {
				let raw = #raw_value;
				if raw.is_empty() {
					signal.set(::core::option::Option::None);
				} else if let Ok(v) = raw.parse::<::std::net::IpAddr>() {
					signal.set(::core::option::Option::Some(v));
				}
			}
		}

		// Generic-capable fields using FromStr
		TypedFieldType::HiddenField { inner } | TypedFieldType::ChoiceField { inner } => {
			quote! {
				if let Ok(v) = #raw_value.parse::<#inner>() {
					signal.set(v);
				}
			}
		}

		// JsonField: use serde_json (FromStr not required)
		TypedFieldType::JsonField { inner } => {
			quote! {
				if let Ok(v) = ::serde_json::from_str::<#inner>(&#raw_value) {
					signal.set(v);
				}
			}
		}

		// MultipleChoiceField handled separately in generate_multi_select_listener
		TypedFieldType::MultipleChoiceField { .. } => {
			unreachable!("MultipleChoiceField is handled by generate_multi_select_listener")
		}

		TypedFieldType::FileField | TypedFieldType::ImageField => {
			unreachable!("FileField and ImageField are handled by generate_file_input_listener")
		}
	}
}

/// Generates the complete listener block for `MultipleChoiceField<T>`, which
/// iterates over `selectedOptions` and parses each value via `FromStr`.
fn generate_multi_select_listener(
	signal_ident: &syn::Ident,
	field_type: &TypedFieldType,
	pages_crate: &TokenStream,
	field_name: &str,
) -> TokenStream {
	let TypedFieldType::MultipleChoiceField { inner } = field_type else {
		unreachable!("generate_multi_select_listener called with non-MultipleChoiceField");
	};

	quote! {
		.on(#pages_crate::event::KnownEvent::Change, {
			let signal = #signal_ident.clone();
			#pages_crate::typed_event_handler::<#pages_crate::event::ChangeEvent, _>(
				move |event: #pages_crate::event::ChangeEvent| {
					match event.selected_values() {
						::core::result::Result::Ok(__selected_values) => {
							let values = __selected_values
								.into_iter()
								.filter_map(|value| value.parse::<#inner>().ok())
								.collect::<::std::vec::Vec<_>>();
							signal.set(values);
						}
						::core::result::Result::Err(__error) => {
							#pages_crate::warn_log!(
								"form field `{}` failed to extract `selected_values`: {}",
								#field_name,
								__error,
							);
						}
					}
				},
			)
		})
	}
}

/// Generates the validate method (server-side).
///
/// Only rules whose scope includes server-side execution are emitted; rules
/// scoped strictly to the client (`#[client(on = ...)]`) are excluded.
fn generate_validate_method(macro_ast: &TypedFormMacro, _pages_crate: &TokenStream) -> TokenStream {
	let validators: Vec<TokenStream> = macro_ast
		.validators
		.iter()
		.flat_map(|v| generate_server_validator_rules(&v.field_name, &v.rules))
		.collect();

	if validators.is_empty() {
		quote! {
			pub fn validate(&self) -> Result<(), Vec<(&'static str, String)>> {
				Ok(())
			}
		}
	} else {
		quote! {
				pub fn validate(&self) -> Result<(), Vec<(&'static str, String)>> {
					let mut errors = Vec::new();
					#(#validators)*
					if errors.is_empty() {
					Ok(())
				} else {
					Err(errors)
				}
			}
		}
	}
}

/// Generates server-side validation code for rules that include server scope.
///
/// Rules with scope `Both`, `Server`, or `ServerAndClient { .. }` are included;
/// rules with scope `Client { .. }` are excluded (they fire only on the client).
fn generate_server_validator_rules(
	field_name: &syn::Ident,
	rules: &[TypedValidatorRule],
) -> Vec<TokenStream> {
	rules
		.iter()
		.filter(|rule| rule.scope.includes_server())
		.map(|rule| {
			let condition = &rule.condition;
			let message = &rule.message;
			let field_variant = field_variant_ident(field_name);
			quote! {
				{
					let v = self.#field_name.get();
					if !(#condition) {
						errors.push((stringify!(#field_variant), (#message).to_string()));
					}
				}
			}
		})
		.collect()
}

/// Generates the submit method if action is specified.
///
/// When callbacks are defined, the submit method integrates them at appropriate points:
/// - `on_submit`: Called before submission starts
/// - `on_loading(true/false)`: Called when loading state changes
/// - `on_success(result)`: Called when submission succeeds
/// - `on_error(e)`: Called when submission fails
fn generate_submit_method(
	macro_ast: &TypedFormMacro,
	pages_crate: &TokenStream,
	success_url_submit_invocation: &TokenStream,
	on_success_submit_invocation: &TokenStream,
	on_success_lifted: bool,
) -> TokenStream {
	let callbacks = &macro_ast.callbacks;
	let state = &macro_ast.state;
	let redirect = &macro_ast.redirect_on_success;

	match &macro_ast.action {
		TypedFormAction::ServerFn(server_fn_ident) => {
			// Generate submit that calls the server_fn with callbacks
			let all_fields = collect_scalar_fields(&macro_ast.fields);
			let file_server_fn_contract = generate_file_server_fn_contract(
				macro_ast,
				server_fn_ident,
				&all_fields,
				pages_crate,
			);

			let strip_arg_exprs: Vec<&syn::Expr> = macro_ast
				.strip_arguments
				.iter()
				.map(|arg| &arg.value)
				.collect();

			let submit_field_arguments: Vec<TokenStream> = all_fields
				.iter()
				.map(|field| {
					let field_name = &field.name;
					if field.validation.required
						&& matches!(
							field.field_type,
							TypedFieldType::FileField | TypedFieldType::ImageField
						) {
						quote! {
							match self.#field_name.get() {
								::core::option::Option::Some(file) => file,
								::core::option::Option::None => {
									return ::core::result::Result::Err(
										#pages_crate::ServerFnError::validation([
											(stringify!(#field_name), "This field is required."),
										]),
									);
								}
							}
						}
					} else {
						quote! { self.#field_name.get() }
					}
				})
				.collect();
			let required_file_validation: Vec<TokenStream> = all_fields
				.iter()
				.filter(|field| {
					field.validation.required
						&& matches!(
							field.field_type,
							TypedFieldType::FileField | TypedFieldType::ImageField
						)
				})
				.map(|field| {
					let field_name = &field.name;
					quote! {
						if self.#field_name.get().is_none() {
							return ::core::result::Result::Err(
								#pages_crate::ServerFnError::validation([
									(stringify!(#field_name), "This field is required."),
								]),
							);
						}
					}
				})
				.collect();

			let submit_server_fn_call = if !strip_arg_exprs.is_empty() {
				// Explicit ambient_arguments path: append exactly the user-supplied
				// expressions positionally after the form-field arguments.
				quote! {
					{
						#server_fn_ident(#(#submit_field_arguments,)* #(#strip_arg_exprs),*).await
					}
				}
			} else {
				quote! { #server_fn_ident(#(#submit_field_arguments),*).await }
			};

			// Generate callback invocations
			let on_submit_code = generate_on_submit_callback(callbacks);
			let on_loading_start_code = generate_on_loading_callback(callbacks, state, true);
			let on_loading_end_code = generate_on_loading_callback(callbacks, state, false);
			let on_success_code = generate_on_success_callback(
				callbacks,
				state,
				&macro_ast.success_url,
				on_success_lifted,
			);
			let on_error_code = generate_on_error_callback(callbacks, state);
			let redirect_code = generate_redirect_code(redirect, pages_crate, &on_error_code);

			// Lifted `on_success_ref:` invocation (issue #4624). Pulls the
			// stored `Arc<dyn Fn(&Self, &dyn Any)>` handler installed by
			// the outer-scope setup and invokes it with `&value as &dyn Any`.
			// Emitted only when the user supplied `on_success_ref:` — when
			// they did not, the field itself is absent from the form
			// struct (gated symmetrically in `generate_on_success_ref_artifacts`)
			// so the invocation must be gated here too. Ordering: this
			// runs *before* `#on_success_code` so that the by-ref handler
			// observes `value` first and the by-move `on_success` consumes
			// it second (ref-first, move-second contract documented on
			// `TypedFormCallbacks::on_success_ref`).
			let on_success_ref_invocation = if callbacks.on_success_ref.is_some() {
				quote! {
					{
						let __handler = self.__on_success_ref_handler.clone();
						__handler(self, &value as &dyn ::core::any::Any);
					}
				}
			} else {
				quote! {}
			};

			quote! {
				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				pub async fn submit(&self) -> Result<(), #pages_crate::ServerFnError> {
					#file_server_fn_contract
					#(#required_file_validation)*

					// Call on_submit callback before submission
					#on_submit_code

					// Set loading state and call on_loading callback
					#on_loading_start_code

					// Call the server function with individual field values as arguments
					let result = #submit_server_fn_call;

					// Clear loading state
					#on_loading_end_code

					// Handle result with callbacks and redirect
					match result {
						Ok(value) => {
							// Reference `value` so the binding is always considered
							// used even when no `on_success` / `success_url`
							// callback consumes it. Avoids `unused_variables`
							// under `-D warnings`. Must precede every consumer
							// below because the user's `on_success:` closure takes
							// `value` by move (`callback(value)`), which would
							// invalidate a trailing borrow for non-`Copy`
							// server-fn outputs.
							let _ = &value;
							// Order matters: `on_success_ref` (lifted,
							// borrows `value` via `&dyn Any`) runs before
							// `on_success` (inline, may consume `value`
							// by move). See #4624 ordering contract on
							// `TypedFormCallbacks::on_success_ref`.
							#on_success_ref_invocation
							#on_success_code
							// Issue #4624: when the user annotated their
							// `on_success:` closure (and the lift is therefore
							// active), this splice forwards `value` to the
							// outer-scope-lifted handler stored on the form
							// struct. When the lift is not active this is the
							// empty token stream and `value` is consumed by the
							// inline #on_success_code above instead.
							//
							// Must run in the same slot as `#on_success_code`
							// — i.e. before `#redirect_code` and
							// `#success_url_submit_invocation` — so the
							// annotated callback is not skipped when those
							// dispatchers choose their navigation policy.
							#on_success_submit_invocation
							#redirect_code
							#success_url_submit_invocation
							Ok(())
						}
						Err(e) => {
							#on_error_code
							Err(e)
						}
					}
				}

				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				pub async fn submit(&self) -> Result<(), #pages_crate::ServerFnError> {
					#file_server_fn_contract

					// Reference server_fn on native so the user's `use` statement
					// stays live and `unused_imports` does not fire under
					// `-D warnings` (reinhardt-web#4070). The wasm branch is the
					// only path that actually invokes it; on native the form is
					// submitted over HTTP, so this is a pure compile-time sink.
					let _ = &#server_fn_ident;
					Ok(())
				}
			}
		}
		TypedFormAction::Url(_) => {
			// For URL action, submit triggers form submission
			// Callbacks are less relevant here since the browser handles the submission
			let on_submit_code = generate_on_submit_callback(callbacks);

			quote! {
				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				pub fn submit(&self) {
					// Call on_submit callback before submission
					#on_submit_code

					// Trigger native form submission via JavaScript
					// This will be handled by the browser
					#pages_crate::dom::submit_form(&self.metadata());
				}

				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				pub fn submit(&self) {
					// On server, submit is a no-op
				}
			}
		}
		TypedFormAction::None => {
			// No action means no submit method
			quote! {}
		}
	}
}

fn generate_file_server_fn_contract(
	macro_ast: &TypedFormMacro,
	server_fn_ident: &syn::Path,
	fields: &[&TypedFormFieldDef],
	pages_crate: &TokenStream,
) -> TokenStream {
	if !fields.iter().any(|field| {
		matches!(
			field.field_type,
			TypedFieldType::FileField | TypedFieldType::ImageField
		)
	}) {
		return quote! {};
	}

	let argument_names: Vec<String> = fields
		.iter()
		.map(|field| field.name.to_string().trim_start_matches("r#").to_owned())
		.chain(macro_ast.strip_arguments.iter().map(|argument| {
			argument
				.name
				.to_string()
				.trim_start_matches("r#")
				.to_owned()
		}))
		.collect();
	let argument_count = argument_names.len();
	let argument_marker_types: Vec<&syn::Ident> = fields
		.iter()
		.map(|field| &field.name)
		.chain(
			macro_ast
				.strip_arguments
				.iter()
				.map(|argument| &argument.name),
		)
		.collect();
	let argument_bounds: Vec<TokenStream> = argument_names
		.iter()
		.enumerate()
		.map(|(index, _)| {
			let marker = argument_marker_types[index];
			quote! {
				#pages_crate::server_fn::ServerFnArgument<#index, Name = #server_fn_ident::__args::#marker>
			}
		})
		.collect();
	let name_assertions: Vec<TokenStream> = argument_names
		.iter()
		.enumerate()
		.map(|(index, name)| {
			quote! {
				const _: () = assert!(
					__reinhardt_server_fn_argument_name_matches(
						<#server_fn_ident::marker as #pages_crate::server_fn::ServerFnArgument<#index>>::METADATA.name,
						#name,
					)
				);
			}
		})
		.collect();

	quote! {
		fn __reinhardt_assert_file_server_fn_contract()
		where
			#server_fn_ident::marker:
				#pages_crate::server_fn::ServerFnArgumentCount<#argument_count>
				#(+ #argument_bounds)*,
		{
		}

		__reinhardt_assert_file_server_fn_contract();
		const fn __reinhardt_server_fn_argument_name_matches(actual: &str, expected: &str) -> bool {
			let actual = actual.as_bytes();
			let expected = expected.as_bytes();
			if actual.len() != expected.len() {
				return false;
			}
			let mut index = 0;
			while index < actual.len() {
				if actual[index] != expected[index] {
					return false;
				}
				index += 1;
			}
			true
		}
		#(#name_assertions)*
	}
}

/// Generates the load_initial_values method if initial_loader is specified.
///
/// This method calls the initial_loader server_fn and populates fields
/// that have `initial_from` specified with values from the loader result.
///
/// The generated method:
/// - Is async and returns `Result<(), ServerFnError>`
/// - Calls the initial_loader server_fn to fetch initial data
/// - Uses field access syntax to populate fields based on `initial_from` mapping
///
/// # Example
///
/// For a form with:
/// ```text
/// initial_loader: get_profile,
/// fields: {
///     username: CharField { initial_from: "name" },
///     email: EmailField { initial_from: "email_address" },
/// }
/// ```
///
/// Generates:
/// ```text
/// pub async fn load_initial_values(&self) -> Result<(), ServerFnError> {
///     let data = get_profile().await?;
///     self.username.set(data.name.clone());
///     self.email.set(data.email_address.clone());
///     Ok(())
/// }
/// ```
fn generate_load_initial_values(
	macro_ast: &TypedFormMacro,
	pages_crate: &TokenStream,
	runtime_contract_supported: bool,
) -> TokenStream {
	// Return empty if no initial_loader is specified
	let Some(initial_loader) = &macro_ast.initial_loader else {
		return quote! {};
	};

	// Collect fields that have initial_from specified (including from groups)
	let all_fields = collect_scalar_fields(&macro_ast.fields);
	let field_setters: Vec<TokenStream> = all_fields
		.iter()
		.filter_map(|field| {
			field.initial_from.as_ref().map(|from_field| {
				let field_name = &field.name;
				let from_ident = syn::Ident::new(from_field, field.name.span());
				quote! {
					self.#field_name.set(data.#from_ident.clone());
				}
			})
		})
		.collect();
	let runtime_initial_values_refresh = if runtime_contract_supported {
		quote! {
			*self.__initial_values.borrow_mut() =
				#pages_crate::FormRuntimeSource::runtime_current_values(self);
		}
	} else {
		quote! {}
	};

	// If no fields have initial_from, just call the loader but don't set anything
	if field_setters.is_empty() {
		quote! {
			/// Loads initial values from the initial_loader server function.
			///
			/// Note: No fields have `initial_from` specified, so this method
			/// only calls the loader without populating any fields.
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			pub async fn load_initial_values(&self) -> Result<(), #pages_crate::ServerFnError> {
				let _data = #initial_loader().await?;
				Ok(())
			}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub async fn load_initial_values(&self) -> Result<(), #pages_crate::ServerFnError> {
				// On server, this is a no-op since initial values are typically
				// loaded differently in SSR context
				Ok(())
			}
		}
	} else {
		quote! {
			/// Loads initial values from the initial_loader server function.
			///
			/// Calls the configured initial_loader and populates fields
			/// that have `initial_from` specified with values from the result.
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				pub async fn load_initial_values(&self) -> Result<(), #pages_crate::ServerFnError> {
					let data = #initial_loader().await?;
					#(#field_setters)*
					#runtime_initial_values_refresh
					Ok(())
				}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub async fn load_initial_values(&self) -> Result<(), #pages_crate::ServerFnError> {
				// On server, this is a no-op since initial values are typically
				// loaded differently in SSR context
				Ok(())
			}
		}
	}
}

/// Generates the load_choices method if choices_loader is specified.
///
/// This method calls the choices_loader server_fn and populates the choices signals
/// for fields that have `choices_config` specified.
///
/// The generated method:
/// - Is async and returns `Result<(), ServerFnError>`
/// - Calls the choices_loader server_fn to fetch choice data
/// - Uses field access syntax to extract choices based on `choices_from` mapping
/// - Transforms each choice item to the public (value, label) tuple based on `choice_value` and `choice_label`
/// - Stores disabled and group metadata in internal choice-item signals for rendering
///
/// # Example
///
/// For a form with:
/// ```text
/// choices_loader: get_poll_detail,
/// fields: {
///     choice: ChoiceField {
///         choices_from: "choices",
///         choice_value: "id",
///         choice_label: "choice_text",
///     },
/// }
/// ```
///
/// Generates:
/// ```text
/// pub async fn load_choices(&self) -> Result<(), ServerFnError> {
///     let data = get_poll_detail().await?;
///     self.choice_choices.set(
///         data.choices.iter().map(|item| {
///             (item.id.to_string(), item.choice_text.clone())
///         }).collect()
///     );
///     self.choice_choice_items.set(...);
///     Ok(())
/// }
/// ```
fn generate_load_choices(macro_ast: &TypedFormMacro, pages_crate: &TokenStream) -> TokenStream {
	// Return empty if no choices_loader is specified
	let Some(choices_loader) = &macro_ast.choices_loader else {
		return quote! {};
	};

	// Collect fields that have choices_config specified (including from groups)
	let all_fields = collect_scalar_fields(&macro_ast.fields);
	let mut choice_setters: Vec<TokenStream> = all_fields
		.iter()
		.filter_map(|field| {
			field.choices_config.as_ref().map(|config| {
				let choices_signal_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				let choice_items_signal_name =
					syn::Ident::new(&format!("{}_choice_items", field.name), field.name.span());
				let from_ident = syn::Ident::new(&config.choices_from, field.name.span());
				let value_ident = syn::Ident::new(&config.choice_value, field.name.span());
				let label_ident = syn::Ident::new(&config.choice_label, field.name.span());
				let disabled_expr = config.choice_disabled.as_ref().map_or_else(
					|| quote!(false),
					|path| {
						let ident = syn::Ident::new(path, field.name.span());
						quote!(item.#ident)
					},
				);
				let group_expr = config.choice_group.as_ref().map_or_else(
					|| quote!(::core::option::Option::None),
					|path| {
						let ident = syn::Ident::new(path, field.name.span());
						quote!(::core::option::Option::Some(item.#ident.clone()))
					},
				);
				let group_disabled_expr = config.choice_group_disabled.as_ref().map_or_else(
					|| quote!(false),
					|path| {
						let ident = syn::Ident::new(path, field.name.span());
						quote!(item.#ident)
					},
				);
				// When the field carries a generic type parameter (e.g.
				// ChoiceField<i64>), clone the value rather than
				// stringifying so the tuple type matches the choices
				// Signal declaration `Signal<Vec<(T, String)>>`.
				let value_expr = if choice_inner_type_is_string(&field.field_type) {
					quote!(item.#value_ident.to_string())
				} else {
					quote!(item.#value_ident.clone())
				};
				let tuple_value_expr = value_expr.clone();
				let item_value_expr = value_expr;
				quote! {
						self.#choices_signal_name.set(
							data.#from_ident.iter().map(|item| {
								(#tuple_value_expr, item.#label_ident.clone())
							}).collect()
						);
						self.#choice_items_signal_name.set(
							data.#from_ident.iter().map(|item| {
								__ReinhardtChoiceItem {
									value: #item_value_expr,
									label: item.#label_ident.clone(),
									disabled: #disabled_expr,
									group: #group_expr,
								group_disabled: #group_disabled_expr,
							}
						}).collect()
					);
				}
			})
		})
		.collect();

	let datalist_setters: Vec<TokenStream> = collect_all_datalists(&macro_ast.fields)
		.iter()
		.filter_map(|datalist| {
			datalist.choices_config.as_ref().map(|config| {
				let choices_signal_name =
					syn::Ident::new(&format!("{}_choices", datalist.name), datalist.name.span());
				let from_ident = syn::Ident::new(&config.choices_from, datalist.name.span());
				let value_ident = syn::Ident::new(&config.choice_value, datalist.name.span());
				let label_ident = syn::Ident::new(&config.choice_label, datalist.name.span());
				let disabled_expr = config.choice_disabled.as_ref().map_or_else(
					|| quote!(false),
					|path| {
						let ident = syn::Ident::new(path, datalist.name.span());
						quote!(item.#ident)
					},
				);
				quote! {
					self.#choices_signal_name.set(
						data.#from_ident.iter().map(|item| {
							__ReinhardtChoiceItem {
								value: item.#value_ident.to_string(),
								label: item.#label_ident.clone(),
								disabled: #disabled_expr,
								group: ::core::option::Option::None,
								group_disabled: false,
							}
						}).collect()
					);
				}
			})
		})
		.collect();

	choice_setters.extend(datalist_setters);

	// If no fields have choices_config, just call the loader but don't set anything
	if choice_setters.is_empty() {
		quote! {
			/// Loads choices from the choices_loader server function.
			///
			/// Note: No fields have `choices_from` specified, so this method
			/// only calls the loader without populating any choices.
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			pub async fn load_choices(&self) -> Result<(), #pages_crate::ServerFnError> {
				let _data = #choices_loader().await?;
				Ok(())
			}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub async fn load_choices(&self) -> Result<(), #pages_crate::ServerFnError> {
				// On server, this is a no-op since choices are typically
				// loaded differently in SSR context
				Ok(())
			}
		}
	} else {
		quote! {
				/// Loads choices from the choices_loader server function.
				///
				/// Calls the configured choices_loader and populates the public choices
				/// signals plus internal rendering metadata for fields that have
				/// `choices_from` specified.
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			pub async fn load_choices(&self) -> Result<(), #pages_crate::ServerFnError> {
				let data = #choices_loader().await?;
				#(#choice_setters)*
				Ok(())
			}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub async fn load_choices(&self) -> Result<(), #pages_crate::ServerFnError> {
				// On server, this is a no-op since choices are typically
				// loaded differently in SSR context
				Ok(())
			}
		}
	}
}

/// Generates the on_submit callback invocation.
fn generate_on_submit_callback(callbacks: &TypedFormCallbacks) -> TokenStream {
	if let Some(on_submit) = &callbacks.on_submit {
		quote! {
			{
				let callback = #on_submit;
				callback(self);
			}
		}
	} else {
		quote! {}
	}
}

/// Generates the on_loading callback invocation and state update.
fn generate_on_loading_callback(
	callbacks: &TypedFormCallbacks,
	_state: &Option<TypedFormState>,
	is_loading: bool,
) -> TokenStream {
	let mut code = Vec::new();

	code.push(quote! {
		self.__loading.set(#is_loading);
	});

	// Clear success/error states when starting loading
	if is_loading {
		code.push(quote! {
			self.__success.set(false);
			self.__error.set(None);
		});
	}

	// Call on_loading callback if defined
	if let Some(on_loading) = &callbacks.on_loading {
		code.push(quote! {
			{
				let callback = #on_loading;
				callback(#is_loading);
			}
		});
	}

	quote! { #(#code)* }
}

/// Aggregated token-stream products of a `success_url:` lift.
///
/// Issue #4605 / #4612 / #4610: the original `generate_on_success_callback`
/// emitted the user's `success_url:` closure literal *inside* the generated
/// `fn submit()` method. Closures inside a `fn` item cannot capture
/// enclosing-scope locals (Rust E0434), so a user closure that referenced
/// outer locals like `let qid = ...;` failed to compile. This struct mirrors
/// the [`WatchArtifacts`] shape: a single helper computes the field
/// declaration, the outer-scope setup (where the user's closure literal is
/// expanded — and where outer locals are still visible), and the
/// per-submit invocation that pulls the stored handler off the form and
/// runs it.
///
/// Empty defaults are produced when the user did not write `success_url:`
/// (every splice point becomes `quote! {}`).
pub(crate) struct SuccessUrlArtifacts {
	/// Field declaration appended to `struct #struct_name { ... }`. Empty
	/// when no `success_url` was supplied.
	field_decl: TokenStream,
	/// Default initializer appended to `Self { ... }` inside `fn new()`.
	/// Stores `None` so the form is usable before the outer-scope setup
	/// binds the real handler.
	field_default_init: TokenStream,
	/// Private setter method spliced into `impl #struct_name`. Used by the
	/// outer-scope setup to install the type-erased handler.
	setter_method: TokenStream,
	/// Code spliced into the outer block (after `let mut __reinhardt_form
	/// = ...::new();`) where the user closure literal is expanded with full
	/// access to enclosing-scope locals like `qid`.
	outer_setup: TokenStream,
	/// Code spliced into the WASM-only `submit()` body after the
	/// server_fn's `Ok(value)` arm. Pulls the stored handler off the form
	/// and dispatches navigation through the new
	/// [`crate::router::navigate`] API.
	submit_invocation: TokenStream,
}

/// Generates the on_success callback invocation and state update spliced
/// inline inside the generated `submit()` body.
///
/// Issue #4624: when the user's `on_success:` closure carries explicit
/// parameter type annotations (e.g. `|value: LoginResponse|`), the lift
/// performed by [`build_on_success_artifacts`] is active and the user
/// closure is invoked from the outer-scope-bound handler — not inline
/// here. In that case `on_success_lifted` is `true` and we skip emitting
/// the inline `let callback = #on_success; callback(value);`. The
/// `__success.set(true)` state update is still emitted because state
/// signals live on the form struct itself and are unaffected by the lift.
///
/// When `on_success_lifted` is `false` (unannotated closure, or no
/// closure at all) we preserve the historical inline emit — the closure
/// body executes inside `fn submit()` exactly as before. This branch
/// matches every existing in-tree caller that did not need outer-scope
/// capture, so it remains the zero-friction default.
///
/// `success_url` is handled separately by [`build_success_url_artifacts`]:
/// it IS lifted to outer scope because its target use case (URL builders
/// like `links::poll_results(qid)`) is *intrinsically* outer-scope-capturing.
/// The lift changes the closure signature from `|form, value|` to `|form|`
/// (the value parameter is dropped); the form's state signals are
/// accessible through the `form` parameter, and outer-scope locals like
/// `qid` are now captured normally. Refs #4605 / #4612.
fn generate_on_success_callback(
	callbacks: &TypedFormCallbacks,
	_state: &Option<TypedFormState>,
	success_url: &Option<syn::Expr>,
	on_success_lifted: bool,
) -> TokenStream {
	let mut code = Vec::new();

	let _ = success_url;
	code.push(quote! {
		self.__success.set(true);
	});

	if !on_success_lifted && let Some(on_success) = &callbacks.on_success {
		code.push(quote! {
			{
				let callback = #on_success;
				callback(value);
			}
		});
	}

	quote! { #(#code)* }
}

/// Builds the [`SuccessUrlArtifacts`] for the optional `success_url:`
/// attribute.
///
/// When the user supplies a closure (`success_url: |form| <expr>`), the
/// closure literal is expanded at the outer block — capturing
/// enclosing-scope locals normally — and stored on the form struct via the
/// generated setter. `submit()` then invokes the stored handler and
/// dispatches the resulting URL through the SPA-aware navigation API.
///
/// When the user supplies a string-literal or other non-closure expression,
/// the macro evaluates it once at outer scope and dispatches the captured
/// value the same way.
fn build_success_url_artifacts(
	success_url: &Option<syn::Expr>,
	pages_crate: &TokenStream,
	struct_name: &syn::Ident,
	navigation_error_handling: &TokenStream,
) -> SuccessUrlArtifacts {
	let empty = SuccessUrlArtifacts {
		field_decl: quote! {},
		field_default_init: quote! {},
		setter_method: quote! {},
		outer_setup: quote! {},
		submit_invocation: quote! {},
	};

	let Some(url_expr) = success_url else {
		return empty;
	};

	// Handler type — type-erased so the form struct stays nameable. The
	// `Send + Sync` bounds mirror `WatchArtifacts` so callbacks can be
	// stored uniformly across SSR / CSR.
	let handler_ty = quote! {
		::std::sync::Arc<
			dyn ::core::ops::Fn(&#struct_name) -> ::std::string::String
				+ ::core::marker::Send
				+ ::core::marker::Sync,
		>
	};

	let field_decl = quote! {
		__success_url_handler: ::core::option::Option<#handler_ty>,
	};

	let field_default_init = quote! {
		__success_url_handler: ::core::option::Option::None,
	};

	let setter_method = quote! {
		#[doc(hidden)]
		fn __set_success_url_handler(&mut self, __handler: #handler_ty) {
			self.__success_url_handler = ::core::option::Option::Some(__handler);
		}
	};

	// Outer-scope wrap. Mirrors the WatchArtifacts pattern: the user
	// closure literal is expanded here, where enclosing-scope locals are
	// visible (issues #4605 / #4612). For closure expressions we use the
	// same `__coerce` identity-function trick that watch artifacts use so
	// the user's `|form| ...` closure infers its parameter type to
	// `&#struct_name` without needing an explicit annotation. For
	// non-closure expressions (string literals, expressions), the value is
	// evaluated once and reused on every call.
	let outer_setup = if matches!(url_expr, syn::Expr::Closure(_)) {
		quote! {
			{
				let __wrapped: #handler_ty = ::std::sync::Arc::new(
					move |__form: &#struct_name| -> ::std::string::String {
						fn __coerce<__F, __R>(__f: __F) -> __F
						where
							__F: ::core::ops::Fn(&#struct_name) -> __R,
						{
							__f
						}
						let __user_success_url = __coerce(#url_expr);
						::std::string::ToString::to_string(&__user_success_url(__form))
					},
				);
				__reinhardt_form.__set_success_url_handler(__wrapped);
			}
		}
	} else {
		quote! {
			{
				let __captured_url: ::std::string::String =
					::std::string::ToString::to_string(&{ #url_expr });
				let __wrapped: #handler_ty = ::std::sync::Arc::new(
					move |_form: &#struct_name| -> ::std::string::String {
						__captured_url.clone()
					},
				);
				__reinhardt_form.__set_success_url_handler(__wrapped);
			}
		}
	};

	// Submit-time invocation: pull the handler off `self`, compute the
	// URL, and dispatch through `navigate_or_reload()`. Wrapped in
	// `#[cfg(all(target_family = "wasm", target_os = "unknown"))]`
	// because native/SSR has no browser history or hard-navigation fallback to
	// drive, even though the navigation API itself remains available there.
	let submit_invocation = quote! {
		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		{
			if let ::core::option::Option::Some(__handler) =
				self.__success_url_handler.as_ref()
			{
				let __url = __handler(self);
				// The shared helper retries only RouterNotInstalled and dispatches
				// safe external URLs directly through browser navigation.
				if let Err(__navigation_error) = #pages_crate::navigate_or_reload(
					__url,
					#pages_crate::NavigationType::Push,
				) {
					let e = #pages_crate::ServerFnError::application(
						::std::string::ToString::to_string(&__navigation_error),
					);
					#navigation_error_handling
					return Err(e);
				}
			}
		}
	};

	SuccessUrlArtifacts {
		field_decl,
		field_default_init,
		setter_method,
		outer_setup,
		submit_invocation,
	}
}

/// Aggregated token-stream products of an `on_success:` lift.
///
/// Issue #4624: companion to [`SuccessUrlArtifacts`]. When the user's
/// `on_success:` closure carries explicit parameter type annotations
/// (e.g. `|value: LoginResponse|`), this struct describes a lift that
/// stores the closure in an outer-scope-bound dispatcher on the form
/// struct, so the closure body can capture enclosing-scope locals like a
/// `qid` route parameter. When the closure is unannotated, every field
/// here is empty and the closure stays inline in `fn submit()` (the
/// historical contract).
///
/// The dispatcher's value parameter is type-erased through
/// `Box<dyn Any + Send>` so the form struct does not need a generic
/// parameter for the server_fn's return type `T`. `T` is recovered from
/// the user's annotation via a monomorphized `__coerce<T, F>` helper
/// (the same pattern used by [`build_success_url_artifacts`] and by
/// `WatchArtifacts`). The runtime downcast in the dispatcher is
/// statically guaranteed to succeed because the only producer of the
/// boxed value is `submit()`, which boxes the server_fn's `Output` (the
/// same type the annotated closure expects).
pub(crate) struct OnSuccessArtifacts {
	/// Field declaration appended to `struct #struct_name { ... }`. Empty
	/// when the closure is not lifted.
	field_decl: TokenStream,
	/// Default initializer appended to `Self { ... }` inside `fn new()`.
	/// Stores `None` so the form is usable before the outer-scope setup
	/// binds the real dispatcher.
	field_default_init: TokenStream,
	/// Private setter method spliced into `impl #struct_name`. Used by
	/// the outer-scope setup to install the type-erased dispatcher.
	setter_method: TokenStream,
	/// Code spliced into the outer block (after `let mut __reinhardt_form
	/// = ...::new();`) where the user closure literal is expanded with
	/// full access to enclosing-scope locals.
	outer_setup: TokenStream,
	/// Code spliced into the WASM-only `submit()` body after the
	/// server_fn's `Ok(value)` arm. Boxes `value` and forwards it to the
	/// stored dispatcher.
	submit_invocation: TokenStream,
	/// `true` when the user's `on_success:` closure was eligible for the
	/// lift (every parameter carries an explicit type annotation) and the
	/// other fields are populated. `false` when the closure stays inline
	/// in `fn submit()` and the other fields are empty token streams.
	lifted: bool,
}

/// Builds the [`OnSuccessArtifacts`] for the optional `on_success:` callback.
///
/// The lift only activates when the user closure carries explicit type
/// annotations on every parameter — `|value: LoginResponse|` lifts;
/// `|value|` or `|_value|` does not. The annotation requirement exists
/// because the macro itself never sees the server_fn's return type as a
/// token; instead the monomorphized `__coerce<T, F>` helper recovers `T`
/// from the closure's annotated parameter. Closures with elided
/// parameter types leave `T` unconstrained and would fail to compile
/// inside `__coerce`, so we keep them on the historical inline path.
///
/// The `TypedFormCallbacks` parser narrows `on_success` to `ExprClosure`
/// already (see `reinhardt-manouche`), so this helper only inspects the
/// closure's parameter list and never sees a non-closure `Expr`.
fn build_on_success_artifacts(on_success: &Option<syn::ExprClosure>) -> OnSuccessArtifacts {
	let empty = OnSuccessArtifacts {
		field_decl: quote! {},
		field_default_init: quote! {},
		setter_method: quote! {},
		outer_setup: quote! {},
		submit_invocation: quote! {},
		lifted: false,
	};

	let Some(closure) = on_success else {
		return empty;
	};

	// Eligibility gate: only closures whose every parameter has an
	// explicit type annotation are lifted. This guards `__coerce<T, F>`
	// against type-inference dead ends for `|_x|` / `|y|` shapes.
	if closure.inputs.is_empty() {
		return empty;
	}
	let all_annotated = closure
		.inputs
		.iter()
		.all(|pat| matches!(pat, syn::Pat::Type(_)));
	if !all_annotated {
		return empty;
	}

	// Type-erased dispatcher. The form struct never names `T`; `T` is
	// recovered from the user closure's annotation via `__coerce<T, F>`
	// at outer setup time, and re-materialized at submit time via an
	// `Any` downcast. The `Send + Sync` bounds mirror the precedent set
	// by [`build_success_url_artifacts`] and `WatchArtifacts` so the
	// form struct can stay `Clone` + cross-thread.
	let dispatcher_ty = quote! {
		::std::sync::Arc<
			dyn ::core::ops::Fn(::std::boxed::Box<
				dyn ::core::any::Any + ::core::marker::Send,
			>)
				+ ::core::marker::Send
				+ ::core::marker::Sync,
		>
	};

	let field_decl = quote! {
		__on_success_dispatcher: ::core::option::Option<#dispatcher_ty>,
	};

	let field_default_init = quote! {
		__on_success_dispatcher: ::core::option::Option::None,
	};

	// Setter is wasm-only because [`outer_setup`] is wasm-only (see the
	// rationale below). Keeping the setter ungated would mean dead code on
	// server builds where the only caller (`outer_setup`) is `cfg`'d out.
	let setter_method = quote! {
		#[doc(hidden)]
		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		fn __set_on_success_dispatcher(&mut self, __dispatcher: #dispatcher_ty) {
			self.__on_success_dispatcher = ::core::option::Option::Some(__dispatcher);
		}
	};

	// Force the user closure to capture by value. Without an explicit
	// `move`, Rust captures enclosing-scope locals by reference, which
	// leaves the resulting closure with a non-`'static` lifetime — and the
	// `Arc<dyn Fn(...) + Send + Sync>` dispatcher field is implicitly
	// `'static` (the trait object's default object lifetime in a struct
	// field). Injecting `move` is strictly an improvement: outer-scope
	// captures that wouldn't have compiled inside the historical
	// `fn submit()` body (E0434) now succeed; captures that worked before
	// (function paths, `'static` values) are unaffected; non-trivial
	// captures are moved into the closure, matching the typical Rust
	// idiom for "store this closure in a 'static container".
	let mut moved_closure = closure.clone();
	if moved_closure.capture.is_none() {
		moved_closure.capture = Some(syn::Token![move](proc_macro2::Span::call_site()));
	}

	// Outer-scope wrap. The closure literal is expanded here where
	// enclosing-scope locals are visible (issue #4624). The
	// monomorphized `__coerce` is what makes the lift work without the
	// macro knowing the server_fn's return type — `T` and `F` are
	// inferred from the user's closure expression.
	//
	// The `#[cfg(all(target_family = "wasm", target_os = "unknown"))]`
	// gate matches the historical inline path: pre-lift, the user
	// closure was spliced inside the wasm-only `fn submit()` body, so
	// any types referenced by the closure (often `cfg(client)`-gated
	// types like `LoginResponse` in `reinhardt-admin`) only needed to
	// be in scope on wasm builds. Lifting the closure to the outer
	// block at the form! call site unconditionally would resurface
	// those imports on server builds — which is exactly the bug that
	// caused `reinhardt-admin`'s login form to stop compiling. Keep the
	// expansion wasm-only so the historical scope contract holds. On
	// server builds, `submit()` is a stub that does not invoke
	// `on_success` anyway, so skipping the lift there is a true
	// no-op.
	let outer_setup = quote! {
		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		{
			fn __coerce<__T, __F>(__cb: __F) -> #dispatcher_ty
			where
				__T: 'static + ::core::marker::Send,
				__F: ::core::ops::Fn(__T)
					+ ::core::marker::Send
					+ ::core::marker::Sync
					+ 'static,
			{
				::std::sync::Arc::new(
					move |__erased: ::std::boxed::Box<
						dyn ::core::any::Any + ::core::marker::Send,
					>| {
						// The only producer of `__erased` is the
						// generated `submit()` body below, which always
						// boxes the server_fn's `Output`. The downcast
						// therefore cannot fail at runtime; the `expect`
						// is purely defensive and would only fire if a
						// future codegen change broke the type-erasure
						// invariant.
						let __value = *__erased
							.downcast::<__T>()
							.expect("form! on_success: dispatcher received a value of unexpected type");
						__cb(__value);
					},
				)
			}
			let __wrapped = __coerce(#moved_closure);
			__reinhardt_form.__set_on_success_dispatcher(__wrapped);
		}
	};

	// Submit-time invocation. Runs only on browser wasm because the
	// inline `fn submit()` body is itself gated by the same `cfg`.
	let submit_invocation = quote! {
		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		{
			if let ::core::option::Option::Some(__dispatcher) =
				self.__on_success_dispatcher.as_ref()
			{
				let __erased: ::std::boxed::Box<
					dyn ::core::any::Any + ::core::marker::Send,
				> = ::std::boxed::Box::new(value);
				__dispatcher(__erased);
			}
		}
	};

	OnSuccessArtifacts {
		field_decl,
		field_default_init,
		setter_method,
		outer_setup,
		submit_invocation,
		lifted: true,
	}
}

/// Generates the on_error callback invocation and state update.
fn generate_on_error_callback(
	callbacks: &TypedFormCallbacks,
	_state: &Option<TypedFormState>,
) -> TokenStream {
	let mut code = Vec::new();

	code.push(quote! {
		self.__error.set(Some(e.to_string()));
		self.__success.set(false);
	});

	// Call on_error callback if defined
	if let Some(on_error) = &callbacks.on_error {
		code.push(quote! {
			{
				let callback = #on_error;
				callback(e.clone());
			}
		});
	}

	quote! { #(#code)* }
}

/// Generates the redirect code if `redirect_on_success` is specified.
///
/// Issue #4610: `navigate_or_reload()` owns the exact RouterNotInstalled
/// fallback policy and dispatches safe external destinations directly.
fn generate_redirect_code(
	redirect: &Option<String>,
	pages_crate: &TokenStream,
	navigation_error_handling: &TokenStream,
) -> TokenStream {
	let Some(url) = redirect else {
		return quote! {};
	};

	quote! {
		// Redirect through the shared helper so RouterRejected remains an
		// application error while safe external URLs hard-navigate.
		if let Err(__navigation_error) = #pages_crate::navigate_or_reload(
			::std::string::ToString::to_string(#url),
			#pages_crate::NavigationType::Push,
		) {
			let e = #pages_crate::ServerFnError::application(
				::std::string::ToString::to_string(&__navigation_error),
			);
			#navigation_error_handling
			return Err(e);
		}
	}
}

/// Converts field type to its value type.
fn field_type_to_value_type(field_type: &TypedFieldType) -> TokenStream {
	match field_type {
		// Inherently String-shaped (always String, no generic accepted)
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField => quote!(::std::string::String),

		// Primitive typed
		TypedFieldType::IntegerField => quote!(i64),
		TypedFieldType::FloatField | TypedFieldType::DecimalField => quote!(f64),
		TypedFieldType::BooleanField => quote!(bool),

		// Date / Time
		TypedFieldType::DateField => quote!(::core::option::Option<chrono::NaiveDate>),
		TypedFieldType::TimeField => quote!(::core::option::Option<chrono::NaiveTime>),
		TypedFieldType::DateTimeField => {
			quote!(::core::option::Option<chrono::NaiveDateTime>)
		}

		// File-like
		TypedFieldType::FileField | TypedFieldType::ImageField => {
			quote!(::core::option::Option<web_sys::File>)
		}

		// Misc
		TypedFieldType::UuidField => quote!(::core::option::Option<uuid::Uuid>),

		// Generic-capable: substitute the inner type recorded by the validator
		TypedFieldType::HiddenField { inner } => quote!(#inner),
		TypedFieldType::ChoiceField { inner } => quote!(#inner),
		TypedFieldType::MultipleChoiceField { inner } => quote!(::std::vec::Vec<#inner>),
		TypedFieldType::JsonField { inner } => quote!(#inner),

		// Specialized
		TypedFieldType::IpAddressField => quote!(::core::option::Option<::std::net::IpAddr>),
	}
}

/// Converts field type to Signal type.
fn field_type_to_signal_type(
	field_type: &TypedFieldType,
	pages_crate: &TokenStream,
) -> TokenStream {
	let inner_type = match field_type {
		// Inherently String-shaped (always String, no generic accepted)
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField => quote!(::std::string::String),

		// Primitive typed
		TypedFieldType::IntegerField => quote!(i64),
		TypedFieldType::FloatField | TypedFieldType::DecimalField => quote!(f64),
		TypedFieldType::BooleanField => quote!(bool),

		// Date / Time
		TypedFieldType::DateField => quote!(::core::option::Option<chrono::NaiveDate>),
		TypedFieldType::TimeField => quote!(::core::option::Option<chrono::NaiveTime>),
		TypedFieldType::DateTimeField => quote!(::core::option::Option<chrono::NaiveDateTime>),

		// File-like
		TypedFieldType::FileField | TypedFieldType::ImageField => {
			quote!(::core::option::Option<web_sys::File>)
		}

		// Misc
		TypedFieldType::UuidField => quote!(::core::option::Option<uuid::Uuid>),

		// Generic-capable: substitute the inner type recorded by the validator
		TypedFieldType::HiddenField { inner } => quote!(#inner),
		TypedFieldType::ChoiceField { inner } => quote!(#inner),
		TypedFieldType::MultipleChoiceField { inner } => quote!(::std::vec::Vec<#inner>),
		TypedFieldType::JsonField { inner } => quote!(#inner),

		// Specialized
		TypedFieldType::IpAddressField => {
			quote!(::core::option::Option<::std::net::IpAddr>)
		}
	};

	quote!(#pages_crate::reactive::Signal<#inner_type>)
}

/// Returns the default value for a field type.
fn field_type_default_value(field_type: &TypedFieldType) -> TokenStream {
	match field_type {
		// Inherently String-shaped — empty string default
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField => quote!(::std::string::String::new()),

		// Primitive
		TypedFieldType::IntegerField => quote!(0i64),
		TypedFieldType::FloatField | TypedFieldType::DecimalField => quote!(0.0f64),
		TypedFieldType::BooleanField => quote!(false),

		// Optional-shaped — None default
		TypedFieldType::DateField
		| TypedFieldType::TimeField
		| TypedFieldType::DateTimeField
		| TypedFieldType::FileField
		| TypedFieldType::ImageField
		| TypedFieldType::UuidField => quote!(::core::option::Option::None),

		// Generic-capable: rely on T: Default (enforced by the
		// struct-level where clause emitted in Task 9)
		TypedFieldType::HiddenField { inner }
		| TypedFieldType::ChoiceField { inner }
		| TypedFieldType::JsonField { inner } => {
			quote!(<#inner as ::core::default::Default>::default())
		}
		TypedFieldType::MultipleChoiceField { .. } => quote!(::std::vec::Vec::new()),

		// Specialized
		TypedFieldType::IpAddressField => quote!(::core::option::Option::None),
	}
}

/// Returns the inner type used as the *value* in a choices store for the
/// given field type.
///
/// For `ChoiceField<T>` the choices store is `Signal<Vec<(T, String)>>`
/// (value, label). For `MultipleChoiceField<T>` the per-item value type is
/// also `T`. This helper MUST only be called on those two field types —
/// it panics otherwise to surface mis-routed callers.
fn choice_value_type(field_type: &TypedFieldType) -> TokenStream {
	match field_type {
		TypedFieldType::ChoiceField { inner } | TypedFieldType::MultipleChoiceField { inner } => {
			quote!(#inner)
		}
		_ => panic!(
			"choice_value_type called on a non-choice TypedFieldType variant; \
			 this is a codegen bug",
		),
	}
}

/// Returns `true` when the inner type of a `ChoiceField` or
/// `MultipleChoiceField` is `String` (including the implicit default).
/// When `false`, the field carries a user-supplied non-String generic
/// and the codegen must not stringify the value in choice-loading code.
fn choice_inner_type_is_string(field_type: &TypedFieldType) -> bool {
	match field_type {
		TypedFieldType::ChoiceField { inner } | TypedFieldType::MultipleChoiceField { inner } => {
			type_is_string(inner)
		}
		_ => false,
	}
}

/// Returns `true` when a `syn::Type` is the path `String`
/// (including fully-qualified forms like `::std::string::String`).
fn type_is_string(ty: &syn::Type) -> bool {
	if let syn::Type::Path(type_path) = ty
		&& type_path.qself.is_none()
	{
		let segs = &type_path.path.segments;
		if segs.len() == 1 {
			return segs[0].ident == "String";
		}
		// std::string::String (or ::std::string::String)
		if segs.len() == 3
			&& segs[0].ident == "std"
			&& segs[1].ident == "string"
			&& segs[2].ident == "String"
		{
			return true;
		}
	}
	false
}

/// Builds the struct-level `where` clause that asserts trait-bound
/// satisfaction for every concrete type substituted into a generic-capable
/// field. For monomorphic forms (where the inner type is fully resolved at
/// macro-expansion time) this clause acts as a compile-time assertion: if
/// the user picked a type that lacks the required impls, the error fires
/// once at the struct definition site with a precise diagnostic.
///
/// Returns an empty token stream when the form has no generic-capable
/// fields (so no where clause is emitted at all).
///
/// Each unique inner type is asserted once. Duplicate inner types across
/// multiple fields contribute only one predicate (deduplicated by the
/// stringified token sequence of the inner type).
fn build_typed_field_where_clause(all_fields: &[&TypedFormFieldDef]) -> TokenStream {
	let mut bounds: Vec<TokenStream> = Vec::new();
	let mut seen: ::std::collections::HashSet<String> = ::std::collections::HashSet::new();
	let mut needs_from_str: ::std::collections::HashSet<String> =
		::std::collections::HashSet::new();

	// First pass: collect which types need FromStr (used by non-JsonField generics)
	for field in all_fields {
		let inner = match &field.field_type {
			TypedFieldType::HiddenField { inner }
			| TypedFieldType::ChoiceField { inner }
			| TypedFieldType::MultipleChoiceField { inner } => Some(inner),
			_ => None,
		};
		if let Some(ty) = inner {
			needs_from_str.insert(quote!(#ty).to_string());
		}
	}

	// Second pass: emit bounds for all generic-capable fields
	for field in all_fields {
		let inner = match &field.field_type {
			TypedFieldType::HiddenField { inner }
			| TypedFieldType::ChoiceField { inner }
			| TypedFieldType::JsonField { inner }
			| TypedFieldType::MultipleChoiceField { inner } => Some(inner.clone()),
			_ => None,
		};
		if let Some(ty) = inner {
			let key = quote!(#ty).to_string();
			if seen.insert(key.clone()) {
				if needs_from_str.contains(&key) {
					bounds.push(quote! {
							#ty: ::serde::Serialize
								+ ::serde::de::DeserializeOwned
								+ ::core::clone::Clone
								+ ::core::cmp::PartialEq
								+ ::core::fmt::Display
								+ ::core::str::FromStr
								+ ::core::default::Default
							+ 'static
					});
				} else {
					bounds.push(quote! {
							#ty: ::serde::Serialize
								+ ::serde::de::DeserializeOwned
								+ ::core::clone::Clone
								+ ::core::cmp::PartialEq
								+ ::core::fmt::Display
								+ ::core::default::Default
							+ 'static
					});
				}
			}
		}
	}

	if bounds.is_empty() {
		quote!()
	} else {
		quote!(where #(#bounds),*)
	}
}

/// Converts field type to string representation.
fn field_type_to_string(field_type: &TypedFieldType) -> &'static str {
	match field_type {
		TypedFieldType::CharField => "CharField",
		TypedFieldType::TextField => "TextField",
		TypedFieldType::EmailField => "EmailField",
		TypedFieldType::PasswordField => "PasswordField",
		TypedFieldType::IntegerField => "IntegerField",
		TypedFieldType::FloatField => "FloatField",
		TypedFieldType::DecimalField => "DecimalField",
		TypedFieldType::BooleanField => "BooleanField",
		TypedFieldType::DateField => "DateField",
		TypedFieldType::TimeField => "TimeField",
		TypedFieldType::DateTimeField => "DateTimeField",
		TypedFieldType::ChoiceField { .. } => "ChoiceField",
		TypedFieldType::MultipleChoiceField { .. } => "MultipleChoiceField",
		TypedFieldType::FileField => "FileField",
		TypedFieldType::ImageField => "ImageField",
		TypedFieldType::UrlField => "UrlField",
		TypedFieldType::SlugField => "SlugField",
		TypedFieldType::UuidField => "UuidField",
		TypedFieldType::IpAddressField => "IpAddressField",
		TypedFieldType::JsonField { .. } => "JsonField",
		TypedFieldType::HiddenField { .. } => "HiddenField",
	}
}

/// Converts widget type to string representation.
fn widget_to_string(widget: &TypedWidget) -> &'static str {
	match widget {
		TypedWidget::TextInput => "TextInput",
		TypedWidget::PasswordInput => "PasswordInput",
		TypedWidget::EmailInput => "EmailInput",
		TypedWidget::NumberInput => "NumberInput",
		TypedWidget::Textarea => "Textarea",
		TypedWidget::CheckboxInput => "CheckboxInput",
		TypedWidget::RadioInput => "RadioInput",
		TypedWidget::RadioSelect => "RadioSelect",
		TypedWidget::Select => "Select",
		TypedWidget::SelectMultiple => "SelectMultiple",
		TypedWidget::DateInput => "DateInput",
		TypedWidget::MonthInput => "MonthInput",
		TypedWidget::WeekInput => "WeekInput",
		TypedWidget::TimeInput => "TimeInput",
		TypedWidget::DateTimeInput => "DateTimeInput",
		TypedWidget::FileInput => "FileInput",
		TypedWidget::HiddenInput => "HiddenInput",
		TypedWidget::ColorInput => "ColorInput",
		TypedWidget::RangeInput => "RangeInput",
		TypedWidget::UrlInput => "UrlInput",
		TypedWidget::TelInput => "TelInput",
		TypedWidget::SearchInput => "SearchInput",
		TypedWidget::CustomExperimental(_) => "CustomWidget",
	}
}

/// Converts widget type to HTML input type attribute.
fn widget_to_input_type(widget: &TypedWidget) -> &'static str {
	match widget {
		TypedWidget::TextInput => "text",
		TypedWidget::PasswordInput => "password",
		TypedWidget::EmailInput => "email",
		TypedWidget::NumberInput => "number",
		TypedWidget::Textarea => "textarea", // Not used directly
		TypedWidget::CheckboxInput => "checkbox",
		TypedWidget::RadioInput => "radio",
		TypedWidget::RadioSelect => "radio",
		TypedWidget::Select => "select",         // Not used directly
		TypedWidget::SelectMultiple => "select", // Not used directly
		TypedWidget::DateInput => "date",
		TypedWidget::MonthInput => "month",
		TypedWidget::WeekInput => "week",
		TypedWidget::TimeInput => "time",
		TypedWidget::DateTimeInput => "datetime-local",
		TypedWidget::FileInput => "file",
		TypedWidget::HiddenInput => "hidden",
		TypedWidget::ColorInput => "color",
		TypedWidget::RangeInput => "range",
		TypedWidget::UrlInput => "url",
		TypedWidget::TelInput => "tel",
		TypedWidget::SearchInput => "search",
		TypedWidget::CustomExperimental(_) => "custom",
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use quote::quote;
	use syn::visit::Visit;

	struct TagInputMatchVisitor<'ast> {
		matches: Vec<&'ast syn::ExprMatch>,
	}

	impl<'ast> Visit<'ast> for TagInputMatchVisitor<'ast> {
		fn visit_local(&mut self, local: &'ast syn::Local) {
			let is_tag_input_pattern = matches!(
				&local.pat,
				syn::Pat::Tuple(pattern)
					if pattern.elems.len() == 2
						&& matches!(
							pattern.elems.first(),
							Some(syn::Pat::Ident(pattern)) if pattern.ident == "tag"
						)
						&& matches!(
							pattern.elems.iter().nth(1),
							Some(syn::Pat::Ident(pattern)) if pattern.ident == "input_type"
						)
			);
			if is_tag_input_pattern
				&& let Some(init) = &local.init
				&& let syn::Expr::Match(expression) = init.expr.as_ref()
			{
				self.matches.push(expression);
			}
			syn::visit::visit_local(self, local);
		}
	}

	fn pattern_variants(pattern: &syn::Pat, variants: &mut Vec<String>) {
		match pattern {
			syn::Pat::Or(pattern) => {
				for case in &pattern.cases {
					pattern_variants(case, variants);
				}
			}
			syn::Pat::Paren(pattern) => pattern_variants(&pattern.pat, variants),
			syn::Pat::Path(pattern) => {
				if let Some(segment) = pattern.path.segments.last() {
					variants.push(segment.ident.to_string());
				}
			}
			_ => {}
		}
	}

	fn string_tuple(expression: &syn::Expr) -> Option<Vec<String>> {
		let syn::Expr::Tuple(tuple) = expression else {
			return None;
		};
		tuple
			.elems
			.iter()
			.map(|expression| {
				let syn::Expr::Lit(expression) = expression else {
					return None;
				};
				let syn::Lit::Str(value) = &expression.lit else {
					return None;
				};
				Some(value.value())
			})
			.collect()
	}

	fn parse_validate_generate(input: proc_macro2::TokenStream) -> TokenStream {
		use reinhardt_manouche::core::FormMacro;

		let untyped_ast: FormMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::form::validator::validate(&untyped_ast, None).unwrap();
		generate(&typed_ast, None)
	}

	#[rstest::rstest]
	fn test_generate_simple_form() {
		let input = quote! {
			name: LoginForm,
			action: "/api/login",

			fields: {
				username: CharField { required },
				password: CharField { widget: PasswordInput },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check struct generation
		assert!(output_str.contains("struct LoginForm"));

		// Check field accessors
		assert!(output_str.contains("fn username"));
		assert!(output_str.contains("fn password"));

		// Check into_view method
		assert!(output_str.contains("fn into_page"));

		// Check action
		assert!(output_str.contains("/api/login"));
	}

	#[rstest::rstest]
	fn test_generate_model_temporal_controls_preserve_html_precision_and_local_values() {
		let input = quote! {
			name: TemporalControlsForm,
			model: TemporalDocument,
			policy: TemporalDocumentFields,
			fields: [aware_at, naive_at, starts_at],
			server_fn: save_temporal_document,
			class: "temporal-form",
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("ModelFormFieldKind :: Time"));
		assert!(output_str.contains("ModelFormFieldKind :: DateTime"));
		assert!(output_str.contains("ModelFormFieldKind :: NaiveDateTime"));
		assert!(output_str.contains("control = control . attr (\"step\" , \"any\")"));
		assert!(output_str.contains("value . strip_suffix ('Z')"));
		assert!(output_str.contains("TemporalDocumentFields"));
		assert!(output_str.contains("ServerFnMetadata"));
		assert!(output_str.contains(":: PATH"));
		assert!(output_str.contains("temporal-form"));
		assert!(output_str.contains("__reinhardt_checkbox_"));
		assert!(output_str.contains("__reinhardt_color_"));
		assert!(output_str.contains("__reinhardt_range_"));
		assert!(output_str.contains("let changed = previous != state . value (field) . cloned ()"));
		assert!(output_str.contains("let _ = self . __state_version . get ()"));
		assert!(output_str.contains("ModelFormPolicy"));
		assert!(output_str.contains("selected_descriptors () . iter () . filter (| descriptor |"));
		assert!(output_str.contains("struct __ReinhardtModelFormField"));
		assert!(output_str.contains("runtime_field_by_name"));
		assert!(output_str.contains("__ReinhardtModelFormField (\"aware_at\")"));
		assert!(output_str.contains("FormRuntimeSource for TemporalControlsForm"));
	}

	#[rstest::rstest]
	fn test_generate_model_color_snapshot_uses_descriptor_metadata() {
		let input = quote! {
			name: PaletteForm,
			model: Palette,
			policy: PaletteFields,
			fields: [accent],
			server_fn: save_palette,
			overrides: {
				accent: { widget: ColorInput },
			},
		};

		let output = parse_validate_generate(input).to_string();

		assert!(output.contains("let fields = submit_form"));
		assert!(output.contains("matches ! (descriptor . name , \"accent\")"));
		assert!(output.contains("input_type == \"color\" && stored_value . is_none ()"));
		assert!(output.contains("__reinhardt_defaulted_"));
		assert!(output.contains("using the generated default"));
	}

	#[rstest::rstest]
	fn test_generate_model_form_uses_endpoint_policy_and_safe_native_controls() {
		let input = quote! {
			name: QuestionForm,
			model: Question,
			policy: QuestionSubmissionPolicy,
			fields: [title, published, score],
			server_fn: save_question,
			overrides: {
				score: { widget: RangeInput },
			},
		};

		let output = parse_validate_generate(input).to_string();

		assert_eq!(
			output
				.matches(
					"pub type QuestionFormData = QuestionModelFormData < QuestionSubmissionPolicy >"
				)
				.count(),
			1,
			"the public data alias must remain assignable to the server endpoint payload",
		);
		assert!(output.contains("i128 :: from (min)"));
		assert!(output.contains("Value :: Null => :: std :: string :: String :: new ()"));
		assert!(output.contains("child (\"Unset\")"));
		assert!(output.contains("__reinhardt_checkbox_"));
		assert!(output.contains("\"unset\""));
		assert!(output.contains("Clear value"));
		assert!(!output.contains("checkbox_edit_script"));
	}

	#[rstest::rstest]
	fn test_generate_model_form_keeps_storage_controls_compilable() {
		let input = quote! {
			name: UploadForm,
			model: UploadDocument,
			policy: UploadDocumentPolicy,
			fields: [document, preview],
			server_fn: save_upload,
		};

		let output = parse_validate_generate(input);
		let block: syn::Block =
			syn::parse2(output).expect("generated model form expansion must parse as a Rust block");
		let mut visitor = TagInputMatchVisitor {
			matches: Vec::new(),
		};
		visitor.visit_block(&block);
		assert_eq!(visitor.matches.len(), 1);

		let storage_arms = visitor.matches[0]
			.arms
			.iter()
			.filter_map(|arm| match arm.body.as_ref() {
				syn::Expr::Match(expression) => Some(expression),
				_ => None,
			})
			.flat_map(|expression| {
				expression.arms.iter().filter_map(|arm| {
					(string_tuple(&arm.body) == Some(vec!["input".into(), "file".into()])).then(
						|| {
							let mut variants = Vec::new();
							pattern_variants(&arm.pat, &mut variants);
							variants
						},
					)
				})
			})
			.collect::<Vec<_>>();
		assert_eq!(storage_arms, vec![vec!["File", "Image"]]);
	}

	#[rstest::rstest]
	fn test_generate_model_form_dispatches_through_marker_contract() {
		let input = quote! {
			name: UploadForm,
			model: UploadDocument,
			policy: UploadDocumentPolicy,
			fields: [title, document],
			server_fn: save_upload,
		};

		let output = parse_validate_generate(input).to_string();

		assert!(output.contains("ModelFormServerFn"));
		assert!(output.contains(":: Response"));
		assert!(output.contains("ModelFormSelectionCount < 2usize >"));
		assert!(output.contains("ModelFormSelectionArgument < 0usize"));
		assert!(output.contains("ModelFormSelectionArgument < 1usize"));
		assert!(output.contains("const NAME : & 'static str = \"title\""));
		assert!(!output.contains("save_upload :: __args :: title"));
		assert!(output.contains("not permitted by its policy"));
	}

	#[rstest::rstest]
	#[case::fields(quote! { fields: [name], })]
	#[case::exclude(quote! { exclude: [api_url], })]
	fn test_generate_model_form_validates_and_rehydrates_submission_snapshot(
		#[case] selection: TokenStream,
	) {
		let input = quote! {
			name: ClusterForm,
			model: Cluster,
			policy: ClusterPolicy,
			#selection
			server_fn: save_cluster,
		};

		let output = parse_validate_generate(input).to_string();
		assert_eq!(output.matches("build_validated_payload_for").count(), 2);
		assert_eq!(
			output
				.matches("build_validated_payload_for :: < ClusterModelFormData < ClusterFormSelectionPolicy > , ClusterFormSelectionPolicy , >")
				.count(),
			2,
			"native and WASM snapshots must validate only selected fields",
		);
		let validation = output
			.rfind("build_validated_payload_for")
			.expect("WASM submission must validate its snapshot");
		let rehydration = output[validation..]
			.find("ModelFormPayload < ClusterFormSelectionPolicy >> :: get_json")
			.map(|offset| validation + offset)
			.expect("generated submission must read normalized payload fields");
		let dispatch = output[validation..]
			.find("submit (& state)")
			.map(|offset| validation + offset)
			.expect("generated submission must dispatch through the marker contract");

		assert_eq!(
			(validation < rehydration, rehydration < dispatch),
			(true, true)
		);
		assert_eq!(
			output
				.matches("validated model-form payload must rehydrate its selected field")
				.count(),
			1
		);
		assert_eq!(output.matches("client_validation").count(), 0);
	}

	#[rstest::rstest]
	fn test_generate_model_form_validates_native_submit_with_structured_errors() {
		let input = quote! {
			name: UploadForm,
			model: UploadDocument,
			policy: UploadDocumentPolicy,
			fields: [title, document],
			server_fn: save_upload,
		};

		let output = parse_validate_generate(input).to_string();
		let (_, native_submit) = output
			.rsplit_once("pub async fn submit")
			.expect("generated model form must have a native submit method");
		let (native_submit, _) = native_submit
			.split_once("fn clear_mounted_file_inputs_matching")
			.expect("generated native submit method must precede WASM file cleanup");

		assert_eq!(
			native_submit
				.matches("state . build_validated_payload_for")
				.count(),
			1
		);
		assert_eq!(
			native_submit
				.matches("ServerFnError :: from (errors)")
				.count(),
			1
		);
		assert_eq!(native_submit.matches("self . data ()").count(), 0);
	}

	#[rstest::rstest]
	fn test_generate_form_with_styling() {
		let input = quote! {
			name: StyledForm,
			action: "/test",
			class: "my-form",

			fields: {
				email: EmailField {
					required,
					class: "email-input",
					wrapper_class: "field-wrapper",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("my-form"));
		assert!(output_str.contains("email-input"));
		assert!(output_str.contains("field-wrapper"));
	}

	#[rstest::rstest]
	fn test_generate_omits_on_success_ref_scaffold_when_unused() {
		// Arrange — a form without `on_success_ref:` must NOT pay the
		// scaffold cost: no field, no setter, no Arc allocation in `new()`.
		let input = quote! {
			name: NoRefForm,
			server_fn: update_profile,

			fields: {
				data: CharField {},
			},
		};

		// Act
		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Assert — the scaffold tokens are absent when the feature is unused.
		assert!(
			!output_str.contains("__on_success_ref_handler"),
			"unused on_success_ref must not allocate a struct field"
		);
		assert!(
			!output_str.contains("__set_on_success_ref_handler"),
			"unused on_success_ref must not generate a setter"
		);
	}

	#[rstest::rstest]
	fn test_generate_hidden_field() {
		let input = quote! {
			name: HiddenForm,
			action: "/test",

			fields: {
				csrf_token: HiddenField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Hidden fields should not have label
		assert!(output_str.contains("hidden"));
	}

	#[rstest::rstest]
	fn test_generate_form_without_state_clause_keeps_standard_signals() {
		let input = quote! {
			name: NoStateForm,
			action: "/test",

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// The legacy `state: { ... }` clause is rejected by validation, but
		// generated forms still expose standard submit status signals for
		// existing UI code and examples.
		assert!(output_str.contains("__loading"));
		assert!(output_str.contains("__error"));
		assert!(output_str.contains("__success"));
	}

	#[rstest::rstest]
	fn test_generate_form_without_callbacks() {
		let input = quote! {
			name: NoCallbackForm,
			server_fn: submit_form,

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Submit should still be generated
		assert!(output_str.contains("fn submit"));
		// But without callback-specific code (no explicit callback variable)
		// The basic structure should still work
		assert!(output_str.contains("submit_form"));
		assert!(output_str.contains("typed_event_handler :: <"));
		assert!(output_str.contains("SubmitEvent"));
		assert!(output_str.contains("event . prevent_default ()"));
	}

	#[rstest::rstest]
	fn test_generate_wrapper_basic() {
		let input = quote! {
			name: WrapperForm,
			action: "/test",

			fields: {
				username: CharField {
					wrapper: div { class: "relative" },
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should use custom wrapper tag (div)
		assert!(output_str.contains("PageElement :: new (\"div\")"));
		// Should have custom class
		assert!(output_str.contains("\"relative\""));
	}

	#[rstest::rstest]
	fn test_generate_wrapper_custom_tag() {
		let input = quote! {
			name: WrapperForm,
			action: "/test",

			fields: {
				email: EmailField {
					wrapper: span { class: "inline-block" },
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should use custom wrapper tag (span)
		assert!(output_str.contains("PageElement :: new (\"span\")"));
		assert!(output_str.contains("\"inline-block\""));
	}

	#[rstest::rstest]
	fn test_generate_wrapper_multiple_attrs() {
		let input = quote! {
			name: WrapperForm,
			action: "/test",

			fields: {
				password: CharField {
					widget: PasswordInput,
					wrapper: div {
						class: "form-field",
						id: "password-wrapper",
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should have both class and id attributes
		assert!(output_str.contains("\"form-field\""));
		assert!(output_str.contains("\"password-wrapper\""));
	}

	#[rstest::rstest]
	fn test_generate_wrapper_no_attrs_uses_default_class() {
		let input = quote! {
			name: WrapperForm,
			action: "/test",

			fields: {
				username: CharField {
					wrapper: section,
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should use custom tag but default class
		assert!(output_str.contains("PageElement :: new (\"section\")"));
		// Should have default reinhardt-field class
		assert!(output_str.contains("reinhardt-field"));
	}

	#[rstest::rstest]
	fn test_generate_field_without_wrapper_uses_default_div() {
		let input = quote! {
			name: DefaultForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should use default div wrapper
		assert!(output_str.contains("PageElement :: new (\"div\")"));
		// Should have default reinhardt-field class
		assert!(output_str.contains("reinhardt-field"));
	}

	// =========================================================================
	// Icon Code Generation Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_generate_icon_basic() {
		let input = quote! {
			name: IconForm,
			action: "/test",

			fields: {
				username: CharField {
					icon: svg {
						class: "w-5 h-5",
						viewBox: "0 0 24 24",
						path { d: "M12 12c2.21 0 4-1.79 4-4" }
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate SVG element
		assert!(output_str.contains("PageElement :: new (\"svg\")"));
		// Should have class attribute
		assert!(output_str.contains("\"w-5 h-5\""));
		// Should have viewBox attribute
		assert!(output_str.contains("\"0 0 24 24\""));
		// Should generate path child
		assert!(output_str.contains("PageElement :: new (\"path\")"));
	}

	#[rstest::rstest]
	fn test_generate_icon_left_position() {
		let input = quote! {
			name: IconLeftForm,
			action: "/test",

			fields: {
				email: EmailField {
					icon: svg {
						viewBox: "0 0 24 24",
						path { d: "M0 0h24v24H0z" }
					},
					icon_position: "left",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate SVG element
		assert!(output_str.contains("PageElement :: new (\"svg\")"));
		// Should generate path child
		assert!(output_str.contains("PageElement :: new (\"path\")"));
		// Icon should appear in the wrapper (before input)
		assert!(output_str.contains("\"0 0 24 24\""));
	}

	#[rstest::rstest]
	fn test_generate_icon_right_position() {
		let input = quote! {
			name: IconRightForm,
			action: "/test",

			fields: {
				search: CharField {
					icon: svg {
						viewBox: "0 0 24 24",
						circle { cx: "11", cy: "11", r: "8" }
					},
					icon_position: "right",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate SVG element
		assert!(output_str.contains("PageElement :: new (\"svg\")"));
		// Should generate circle child
		assert!(output_str.contains("PageElement :: new (\"circle\")"));
		// Should have circle attributes
		assert!(output_str.contains("\"cx\""));
		assert!(output_str.contains("\"11\""));
	}

	#[rstest::rstest]
	fn test_generate_icon_label_position() {
		let input = quote! {
			name: IconLabelForm,
			action: "/test",

			fields: {
				password: CharField {
					widget: PasswordInput,
					icon: svg {
						viewBox: "0 0 24 24",
						path { d: "M12 15v2m0 0v2" }
					},
					icon_position: "label",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate SVG element
		assert!(output_str.contains("PageElement :: new (\"svg\")"));
		// Should have label element
		assert!(output_str.contains("PageElement :: new (\"label\")"));
		// Icon should be child of label (icon appears near label)
		assert!(output_str.contains("\"0 0 24 24\""));
	}

	#[rstest::rstest]
	fn test_generate_icon_with_nested_group() {
		let input = quote! {
			name: NestedIconForm,
			action: "/test",

			fields: {
				status: CharField {
					icon: svg {
						viewBox: "0 0 24 24",
						g {
							fill: "none",
							stroke: "currentColor",
							path { d: "M5 13l4 4L19 7" }
						}
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate SVG element
		assert!(output_str.contains("PageElement :: new (\"svg\")"));
		// Should generate g element
		assert!(output_str.contains("PageElement :: new (\"g\")"));
		// Should have fill attribute
		assert!(output_str.contains("\"fill\""));
		assert!(output_str.contains("\"none\""));
		// Should have nested path
		assert!(output_str.contains("PageElement :: new (\"path\")"));
	}

	#[rstest::rstest]
	fn test_generate_field_without_icon() {
		let input = quote! {
			name: NoIconForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should NOT contain svg element for this field
		// Note: This checks the into_view generated code
		assert!(output_str.contains("PageElement :: new (\"input\")"));
		// The field wrapper should still be present
		assert!(output_str.contains("PageElement :: new (\"div\")"));
	}

	// =========================================================================
	// Custom Attrs Code Generation Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_generate_custom_attrs_aria() {
		let input = quote! {
			name: AriaForm,
			action: "/test",

			fields: {
				email: EmailField {
					attrs: {
						aria_label: "Email address",
						aria_required: "true",
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate aria-label attribute (underscore converted to hyphen)
		assert!(output_str.contains("\"aria-label\""));
		assert!(output_str.contains("\"Email address\""));
		// Should generate aria-required attribute
		assert!(output_str.contains("\"aria-required\""));
		assert!(output_str.contains("\"true\""));
	}

	#[rstest::rstest]
	fn test_generate_custom_attrs_data() {
		let input = quote! {
			name: DataForm,
			action: "/test",

			fields: {
				username: CharField {
					attrs: {
						data_testid: "username-input",
						data_analytics: "signup-username",
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate data-testid attribute
		assert!(output_str.contains("\"data-testid\""));
		assert!(output_str.contains("\"username-input\""));
		// Should generate data-analytics attribute
		assert!(output_str.contains("\"data-analytics\""));
		assert!(output_str.contains("\"signup-username\""));
	}

	#[rstest::rstest]
	fn test_generate_custom_attrs_mixed() {
		let input = quote! {
			name: MixedAttrsForm,
			action: "/test",

			fields: {
				password: CharField {
					widget: PasswordInput,
					attrs: {
						aria_label: "Password",
						data_testid: "password-field",
						aria_describedby: "password-help",
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate all custom attributes
		assert!(output_str.contains("\"aria-label\""));
		assert!(output_str.contains("\"data-testid\""));
		assert!(output_str.contains("\"aria-describedby\""));
	}

	#[rstest::rstest]
	fn test_generate_custom_attrs_with_other_properties() {
		let input = quote! {
			name: CombinedForm,
			action: "/test",

			fields: {
				search: CharField {
					required,
					label: "Search",
					placeholder: "Enter search term",
					class: "search-input",
					attrs: {
						aria_label: "Search field",
						data_cy: "search-input",
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should have standard attributes
		assert!(output_str.contains("\"Search\""));
		assert!(output_str.contains("\"Enter search term\""));
		assert!(output_str.contains("\"search-input\""));
		// Should also have custom attributes
		assert!(output_str.contains("\"aria-label\""));
		assert!(output_str.contains("\"Search field\""));
		assert!(output_str.contains("\"data-cy\""));
	}

	#[rstest::rstest]
	fn test_generate_field_without_custom_attrs() {
		let input = quote! {
			name: NoAttrsForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate normal input without aria-/data- attributes
		assert!(output_str.contains("PageElement :: new (\"input\")"));
		// Should not contain aria- or data- prefixed attributes
		// (These are the custom attrs, not standard HTML attrs)
		assert!(!output_str.contains("\"aria-label\""));
		assert!(!output_str.contains("\"data-testid\""));
	}

	#[rstest::rstest]
	fn test_generate_custom_attrs_on_textarea() {
		let input = quote! {
			name: TextareaAttrsForm,
			action: "/test",

			fields: {
				description: TextField {
					attrs: {
						aria_multiline: "true",
						data_autoresize: "enabled",
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate textarea element
		assert!(output_str.contains("PageElement :: new (\"textarea\")"));
		// Should have custom attrs on textarea
		assert!(output_str.contains("\"aria-multiline\""));
		assert!(output_str.contains("\"data-autoresize\""));
	}

	#[rstest::rstest]
	fn test_generate_native_attrs() {
		let input = quote! {
		name: NativeAttrsForm,
		action: "/test",

		fields: {
			billing_month: CharField {
				widget: MonthInput,
				min: "2026-01",
				max: "2026-12",
				step: 1,
			}
			search: CharField {
				widget: SearchInput,
				list: suggestions,
				size: 32,
			}
			suggestions: Datalist {
				options: [("term", "Term")],
			}
				avatar: FileField {
					accept: "image/png",
					capture: "environment",
				}
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains(". attr (\"type\" , \"month\")"));
		assert!(output_str.contains(". attr (\"min\" , (\"2026-01\") . to_string ())"));
		assert!(output_str.contains(". attr (\"max\" , (\"2026-12\") . to_string ())"));
		assert!(output_str.contains(". attr (\"step\" , (1) . to_string ())"));
		assert!(output_str.contains(". attr (\"type\" , \"search\")"));
		assert!(output_str.contains(". attr (\"list\" , \"suggestions\")"));
		assert!(output_str.contains(". attr (\"size\" , \"32\")"));
		assert!(output_str.contains(". attr (\"accept\" , \"image/png\")"));
		assert!(output_str.contains(". attr (\"capture\" , \"environment\")"));
	}

	#[rstest::rstest]
	fn test_generate_month_and_week_native_input_attrs() {
		let input = quote! {
			name: MonthWeekAttrsForm,
			action: "/test",

			fields: {
				billing_month: CharField {
					widget: MonthInput,
					min: "2026-01",
				}
				sprint_week: CharField {
					widget: WeekInput,
					max: "2026-W52",
				}
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains(". attr (\"type\" , \"month\")"));
		assert!(output_str.contains(". attr (\"min\" , (\"2026-01\") . to_string ())"));
		assert!(output_str.contains(". attr (\"type\" , \"week\")"));
		assert!(output_str.contains(". attr (\"max\" , (\"2026-W52\") . to_string ())"));
	}

	#[rstest::rstest]
	fn test_generate_control_entries_without_value_accessors() {
		let input = quote! {
			name: ControlEntryCodegenForm,
			action: "/test",

			fields: {
				title: CharField {}
				reset: ResetButton {
					label: "Reset",
				}
				preview: Button {
					label: "Preview",
				}
				progress: Progress {
					value: 3,
					max: 10,
				}
				image_submit: ImageInput {
					src: "/submit.png",
					alt: "Submit image",
				}
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("PageElement :: new (\"button\")"));
		assert!(output_str.contains(". attr (\"type\" , \"reset\")"));
		assert!(output_str.contains(". attr (\"type\" , \"button\")"));
		assert!(output_str.contains("PageElement :: new (\"progress\")"));
		assert!(output_str.contains(". attr (\"name\" , \"progress\")"));
		assert!(output_str.contains(". attr (\"type\" , \"image\")"));
		assert!(output_str.contains(". attr (\"aria-label\" , \"Submit image\")"));
		assert!(output_str.contains("pub fn title"));
		assert!(output_str.contains("title :"));
		assert!(output_str.contains("& self . title"));
		assert!(!output_str.contains("pub fn reset"));
		assert!(!output_str.contains("pub fn preview"));
		assert!(!output_str.contains("pub fn progress"));
		assert!(!output_str.contains("reset : :: reinhardt_pages :: reactive :: Signal"));
		assert!(!output_str.contains("preview : :: reinhardt_pages :: reactive :: Signal"));
		assert!(!output_str.contains("progress : :: reinhardt_pages :: reactive :: Signal"));
	}

	#[rstest::rstest]
	fn test_generate_datalist_and_optgroup_entries() {
		let input = quote! {
			name: DatalistOptGroupCodegenForm,
			action: "/test",

			fields: {
				query: CharField {
					widget: SearchInput,
					list: suggestions,
				}
				suggestions: Datalist {
					options: [("a", "A"), ("b", "B") { disabled }],
				}
				status: ChoiceField<String> {
					widget: Select,
					choices: [OptGroup("Active") {
						("open", "Open"),
					}],
				}
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains(". attr (\"type\" , \"search\")"));
		assert!(output_str.contains(". attr (\"list\" , \"suggestions\")"));
		assert!(output_str.contains("PageElement :: new (\"datalist\")"));
		assert!(output_str.contains(". attr (\"id\" , \"suggestions\")"));
		assert!(output_str.contains("PageElement :: new (\"option\")"));
		assert!(output_str.contains(". attr (\"value\" , (\"a\") . to_string ())"));
		assert!(output_str.contains(". attr (\"value\" , (\"b\") . to_string ())"));
		assert!(output_str.contains(". bool_attr (\"disabled\" , true)"));
		assert!(output_str.contains("PageElement :: new (\"select\")"));
		assert!(output_str.contains("PageElement :: new (\"optgroup\")"));
		assert!(output_str.contains(". attr (\"label\" , \"Active\")"));
		assert!(output_str.contains(". attr (\"value\" , (\"open\") . to_string ())"));
	}

	// ========================================
	// Two-way binding (bind) tests
	// ========================================

	#[rstest::rstest]
	fn test_generate_bind_default_true() {
		let input = quote! {
			name: BindDefaultForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Default bind is true, so should generate signal binding and listener
		assert!(output_str.contains("let username_signal = self . username . clone ()"));
		assert!(output_str.contains("KnownEvent :: Input"));
		assert!(output_str.contains("typed_event_handler :: <"));
		assert!(output_str.contains("signal . set"));
	}

	#[rstest::rstest]
	fn test_generate_bind_explicit_true() {
		let input = quote! {
			name: BindTrueForm,
			action: "/test",

			fields: {
				email: EmailField {
					bind: true,
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// bind: true should generate signal binding and listener
		assert!(output_str.contains("let email_signal = self . email . clone ()"));
		assert!(output_str.contains("KnownEvent :: Input"));
		assert!(output_str.contains("InputEvent"));
	}

	#[rstest::rstest]
	fn test_generate_bind_explicit_false() {
		let input = quote! {
			name: BindFalseForm,
			action: "/test",

			fields: {
				password: CharField {
					widget: PasswordInput,
					bind: false,
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// bind: false should NOT generate signal binding or listener
		assert!(!output_str.contains("let password_signal"));
		// The input element should still be generated
		assert!(output_str.contains("PageElement :: new (\"input\")"));
		// But no listener for password
		assert!(!output_str.contains("password_signal"));
	}

	#[rstest::rstest]
	fn test_generate_bind_textarea_uses_input_event() {
		let input = quote! {
			name: TextareaBindForm,
			action: "/test",

			fields: {
				description: TextField {
					bind: true,
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("typed_event_handler :: <"));
		assert!(output_str.contains("InputEvent"));
		assert!(output_str.contains("event . value ()"));
		assert!(output_str.contains("failed to extract `value`"));
		assert!(output_str.contains("\"description\""));
		assert!(!output_str.contains("HtmlTextAreaElement"));
	}

	#[rstest::rstest]
	fn test_generate_bind_select_uses_change_event() {
		let input = quote! {
			name: SelectBindForm,
			action: "/test",

			fields: {
				country: ChoiceField {
					bind: true,
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("typed_event_handler :: <"));
		assert!(output_str.contains("ChangeEvent"));
		assert!(output_str.contains("event . value ()"));
		assert!(output_str.contains("failed to extract `value`"));
		assert!(output_str.contains("\"country\""));
		assert!(!output_str.contains("HtmlSelectElement"));
	}

	#[rstest::rstest]
	fn test_generate_dynamic_radio_select_renders_choice_inputs() {
		let input = quote! {
			name: DynamicRadioForm,
			server_fn: submit_vote,

			fields: {
				choice_id: ChoiceField {
					widget: RadioSelect,
					required,
					label: "Select your choice",
					class: "form-check",
					choices_from: "choices",
					choice_value: "id",
					choice_label: "choice_text",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("Page :: reactive"));
		assert!(output_str.contains("let __choices_signal = self . choice_id_choices . clone ()"));
		assert!(output_str.contains("__choices_signal . get ()"));
		assert!(output_str.contains("for (i , choice) in __choices . into_iter () . enumerate ()"));
		assert!(output_str.contains("let choice_value = choice . value"));
		assert!(output_str.contains(". attr (\"type\" , \"radio\")"));
		assert!(output_str.contains(". attr (\"value\" , choice_value . to_string ())"));
		assert!(output_str.contains(". bool_attr (\"disabled\" , choice . disabled)"));
		assert!(output_str.contains(". child (choice . label)"));
		assert!(output_str.contains("KnownEvent :: Change"));
		assert!(output_str.contains("ChangeEvent"));
		assert!(output_str.contains("event . value ()"));
	}

	#[rstest::rstest]
	fn test_generate_load_choices_non_string_value_sets_tuple_and_metadata() {
		let input = quote! {
			name: NonStringChoiceLoaderForm,
			server_fn: submit_choice,
			choices_loader: load_options,

			fields: {
				choice_id: ChoiceField<i64> {
					choices_from: "choices",
					choice_value: "id",
					choice_label: "name",
					choice_disabled: "disabled",
				},
				selected_ids: MultipleChoiceField<i64> {
					widget: SelectMultiple,
					choices_from: "choices",
					choice_value: "id",
					choice_label: "name",
					choice_group: "group",
					choice_group_disabled: "group_disabled",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("self . choice_id_choices . set"));
		assert!(output_str.contains("self . choice_id_choice_items . set"));
		assert!(output_str.contains("self . selected_ids_choices . set"));
		assert!(output_str.contains("self . selected_ids_choice_items . set"));
		assert!(output_str.contains("item . id . clone ()"));
		assert!(!output_str.contains("item . id . to_string ()"));
		assert!(output_str.contains("__ReinhardtChoiceItem"));
		assert!(output_str.contains("value : item . id . clone ()"));
		assert!(
			output_str
				.contains("group : :: core :: option :: Option :: Some (item . group . clone ())")
		);
		assert!(output_str.contains("group_disabled : item . group_disabled"));
	}

	#[rstest::rstest]
	fn test_generate_bind_checkbox_uses_change_event() {
		let input = quote! {
			name: CheckboxBindForm,
			action: "/test",

			fields: {
				agree: BooleanField {
					bind: true,
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("typed_event_handler :: <"));
		assert!(output_str.contains("ChangeEvent"));
		assert!(output_str.contains("event . checked ()"));
		assert!(output_str.contains("failed to extract `checked`"));
		assert!(output_str.contains("\"agree\""));
		assert!(!output_str.contains("HtmlInputElement"));
	}

	#[rstest::rstest]
	fn test_generate_bind_file_field_uses_change_event_and_files() {
		let input = quote! {
			name: FileBindForm,
			action: "/test",

			fields: {
				document: FileField {
					bind: true,
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("typed_event_handler :: <"));
		assert!(output_str.contains("ChangeEvent"));
		assert!(output_str.contains("event . files ()"));
		assert!(output_str.contains("file . raw () . clone ()"));
		assert!(output_str.contains("failed to extract `files`"));
		assert!(output_str.contains("\"document\""));
	}

	#[rstest::rstest]
	fn test_generate_bind_multiple_fields() {
		let input = quote! {
			name: MultiBindForm,
			action: "/test",

			fields: {
				username: CharField { bind: true },
				email: EmailField { bind: true },
				password: CharField { widget: PasswordInput, bind: false },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should generate signal bindings for username and email
		assert!(output_str.contains("let username_signal = self . username . clone ()"));
		assert!(output_str.contains("let email_signal = self . email . clone ()"));

		// Should NOT generate signal binding for password
		assert!(!output_str.contains("let password_signal"));

		// Should have listeners for username and email
		assert!(output_str.contains("username_signal"));
		assert!(output_str.contains("email_signal"));
	}

	#[rstest::rstest]
	fn test_generate_bind_with_cfg_attributes() {
		let input = quote! {
			name: CfgBindForm,
			action: "/test",

			fields: {
				name: CharField { bind: true },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Generated code should have cfg attributes for platform-specific code
		assert!(
			output_str
				.contains("# [cfg (all (target_family = \"wasm\" , target_os = \"unknown\"))]")
		);
		assert!(
			output_str.contains(
				"# [cfg (not (all (target_family = \"wasm\" , target_os = \"unknown\")))]"
			)
		);
	}

	// ===========================================
	// Watch Code Generation Tests
	// ===========================================

	#[rstest::rstest]
	fn test_generate_watch_single_item() {
		let input = quote! {
			name: WatchForm,
			action: "/test",

			watch: {
				error_display: |form| {
					form.error()
				},
			},

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that the watch method is generated
		assert!(output_str.contains("fn error_display"));
		assert!(output_str.contains("IntoPage"));
		// Watch methods use Page::reactive for automatic re-rendering
		assert!(output_str.contains("Page :: reactive"));
		assert!(output_str.contains("form . error ()"));
	}

	#[rstest::rstest]
	fn test_generate_watch_multiple_items() {
		let input = quote! {
			name: MultiWatchForm,
			action: "/test",

			watch: {
				email_view: |form| { form.email().get() },
				length_view: |form| { form.email().get().len() },
			},

			fields: {
				email: EmailField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check all three watch methods are generated
		assert!(output_str.contains("fn email_view"));
		assert!(output_str.contains("fn length_view"));
	}

	#[rstest::rstest]
	fn test_generate_watch_complex_closure() {
		let input = quote! {
			name: ComplexWatchForm,
			action: "/test",

			watch: {
				conditional_view: |form| {
					if form.data().get().trim().is_empty() {
						"Missing"
					} else {
						"Ready"
					}
				},
			},

			fields: {
				data: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that the watch method is generated with the complex closure
		assert!(output_str.contains("fn conditional_view"));
		assert!(output_str.contains("form . data () . get () . trim () . is_empty ()"));
	}

	#[rstest::rstest]
	fn test_generate_watch_with_form_clone() {
		let input = quote! {
			name: CloneWatchForm,
			action: "/test",

			watch: {
				preview: |form| { form.username() },
			},

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that the form is cloned inside the watch method and that the
		// user's handler is captured at the macro call site as a stored
		// `Arc<dyn Fn(&Self) -> Page>` field (issue #4414). The previous
		// `fn __call_watch` indirection and the inline `__watch_handler` local
		// have both been removed; the user closure is now bound from the outer
		// scope so it can capture enclosing-scope locals.
		assert!(output_str.contains("let __form = self . clone ()"));
		assert!(output_str.contains("__watch_preview"));
		assert!(output_str.contains("__user_watch_handler"));
		assert!(!output_str.contains("__call_watch"));
	}

	#[rstest::rstest]
	fn test_generate_no_watch_block() {
		let input = quote! {
			name: NoWatchForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// No watch methods should be generated
		assert!(!output_str.contains("__watch_closure"));
	}

	#[rstest::rstest]
	fn test_generate_watch_empty_block() {
		let input = quote! {
			name: EmptyWatchForm,
			action: "/test",

			watch: {},

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// No watch methods should be generated for empty block
		assert!(!output_str.contains("__watch_closure"));
	}

	#[rstest::rstest]
	fn test_generate_empty_fields_focus_omits_unreachable_dom_path() {
		let input = quote! {
			name: EmptyFieldsForm,
			action: "/test",

			fields: {},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("fn runtime_set_focus"));
		assert!(output_str.contains("match field { }"));
		assert!(!output_str.contains("let field_id = match field { }"));
	}

	// ===== Redirect Code Generation Tests =====

	#[rstest::rstest]
	fn test_generate_redirect_static_path() {
		let input = quote! {
			name: RedirectForm,
			server_fn: submit_form,
			redirect_on_success: "/dashboard",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("navigate_or_reload"));
		assert!(output_str.contains("NavigationType :: Push"));
		assert!(output_str.contains("ServerFnError :: application"));
		assert!(output_str.contains("return Err"));
		assert!(output_str.contains("self . __error . set"));
		assert!(output_str.contains("self . __success . set (false)"));
		assert!(!output_str.contains("set_href"));
		assert!(output_str.contains("/dashboard"));
	}

	#[rstest::rstest]
	fn test_generate_redirect_full_url() {
		let input = quote! {
			name: RedirectUrlForm,
			server_fn: submit_form,
			redirect_on_success: "https://example.com/success",

			fields: {
				email: EmailField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("https://example.com/success"));
		assert!(output_str.contains("navigate_or_reload"));
		assert!(output_str.contains("NavigationType :: Push"));
		assert!(output_str.contains("ServerFnError :: application"));
		assert!(output_str.contains("return Err"));
		assert!(!output_str.contains("set_href"));
	}

	#[rstest::rstest]
	fn test_generate_no_redirect() {
		let input = quote! {
			name: NoRedirectForm,
			server_fn: submit_form,

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("submit"));
		assert!(!output_str.contains("navigate_or_reload"));
		assert!(!output_str.contains("set_href"));
	}

	#[rstest::rstest]
	fn test_generate_success_url_runs_from_onsubmit_path() {
		let input = quote! {
			name: SuccessUrlSubmitForm,
			server_fn: submit_form,
			success_url: |_form| "/done",

			fields: {
				name: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		assert!(
			output_str.contains(
				"let __success_url_handler_clone = self . __success_url_handler . clone ()"
			)
		);
		assert!(output_str.contains("let __url = __handler (& __form_clone_for_success_url)"));
		assert!(output_str.contains("navigate_or_reload"));
		assert!(output_str.contains("NavigationType :: Push"));
		assert!(output_str.contains("ServerFnError :: application"));
		assert!(output_str.contains("return Err"));
		assert!(!output_str.contains("set_href"));
	}

	#[rstest::rstest]
	fn redirect_builder_uses_shared_navigation_fallback() {
		let pages_crate = quote!(::reinhardt_pages);
		let navigation_error_handling = quote! {
			self.__error.set(Some(e.to_string()));
			self.__success.set(false);
		};
		let output = generate_redirect_code(
			&Some("/done".to_owned()),
			&pages_crate,
			&navigation_error_handling,
		);
		let output_str = output.to_string();

		assert!(output_str.contains("navigate_or_reload"));
		assert!(output_str.contains("NavigationType :: Push"));
		assert!(output_str.contains("ServerFnError :: application"));
		assert!(output_str.contains("return Err"));
		assert!(!output_str.contains("web_sys :: window"));
		assert!(!output_str.contains("set_href"));
	}

	#[rstest::rstest]
	fn success_url_submit_builder_uses_shared_navigation_fallback() {
		let pages_crate = quote!(::reinhardt_pages);
		let struct_name = quote::format_ident!("SuccessUrlForm");
		let success_url = Some(syn::parse_quote!(|_form| "/done"));
		let navigation_error_handling = quote! {
			self.__error.set(Some(e.to_string()));
			self.__success.set(false);
		};
		let artifacts = build_success_url_artifacts(
			&success_url,
			&pages_crate,
			&struct_name,
			&navigation_error_handling,
		);
		let output_str = artifacts.submit_invocation.to_string();

		assert!(output_str.contains("navigate_or_reload"));
		assert!(output_str.contains("NavigationType :: Push"));
		assert!(output_str.contains("ServerFnError :: application"));
		assert!(output_str.contains("return Err"));
		assert!(!output_str.contains("web_sys :: window"));
		assert!(!output_str.contains("set_href"));
	}

	#[rstest::rstest]
	fn test_generate_redirect_url_action_ignored() {
		let input = quote! {
			name: UrlRedirectForm,
			action: "/submit",
			redirect_on_success: "/redirect-target",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// For URL action, redirect_on_success is ignored
		// because the browser handles the form submission
		// The redirect code should not appear in the URL action submit method
		assert!(output_str.contains("/submit"));
		// The URL action submit uses dom::submit_form, not our redirect logic
		assert!(output_str.contains("submit_form"));
	}

	// =========================================================================
	// Slots Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_generate_slots_before_fields() {
		let input = quote! {
			name: SlotsBeforeForm,
			action: "/test",

			slots: {
				before_fields: || {
					view! { <div class="header">"Header"</div> }
				},
			},

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that into_view method exists
		assert!(output_str.contains("fn into_page"));

		// Check that the form element is generated
		assert!(output_str.contains("PageElement :: new (\"form\")"));

		// The slot closure should be called with .child()
		assert!(output_str.contains(". child"));
	}

	#[rstest::rstest]
	fn test_generate_slots_after_fields() {
		let input = quote! {
			name: SlotsAfterForm,
			action: "/test",

			slots: {
				after_fields: || {
					view! { <button type="submit">"Submit"</button> }
				},
			},

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that into_view method exists
		assert!(output_str.contains("fn into_page"));

		// The slot closure should be called
		assert!(output_str.contains(". child"));
	}

	#[rstest::rstest]
	fn test_generate_slots_both() {
		let input = quote! {
			name: SlotsBothForm,
			action: "/test",

			slots: {
				before_fields: || {
					view! { <div class="header">"Header"</div> }
				},
				after_fields: || {
					view! { <div class="footer">"Footer"</div> }
				},
			},

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that both slots are rendered as children
		// The form should have multiple .child() calls
		let child_count = output_str.matches(". child").count();
		// At least 3: before_fields slot, field wrapper, after_fields slot
		assert!(
			child_count >= 3,
			"Expected at least 3 child calls, got {child_count}"
		);
	}

	#[rstest::rstest]
	fn test_generate_no_slots() {
		let input = quote! {
			name: NoSlotsForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Form should still be generated properly without slots
		assert!(output_str.contains("fn into_page"));
		assert!(output_str.contains("PageElement :: new (\"form\")"));
	}

	// =========================================================================
	// Initial Loader Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_generate_initial_loader_basic() {
		let input = quote! {
			name: LoaderForm,
			server_fn: update_profile,
			initial_loader: get_profile,

			fields: {
				username: CharField {
					required,
					initial_from: "username",
				},
				email: EmailField {
					initial_from: "email",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that load_initial_values method is generated
		assert!(output_str.contains("fn load_initial_values"));

		// Check it's async
		assert!(output_str.contains("async fn load_initial_values"));

		// Check it calls the initial_loader
		assert!(output_str.contains("get_profile"));

		// Check it sets the fields from the data
		assert!(output_str.contains("self . username . set"));
		assert!(output_str.contains("self . email . set"));

		// Check it accesses the correct data fields
		assert!(output_str.contains("data . username"));
		assert!(output_str.contains("data . email"));
	}

	#[rstest::rstest]
	fn test_generate_initial_loader_with_path() {
		let input = quote! {
			name: PathLoaderForm,
			server_fn: api::update,
			initial_loader: api::loader::get_data,

			fields: {
				name: CharField {
					initial_from: "name",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that the full path is used
		assert!(output_str.contains("api :: loader :: get_data"));
	}

	#[rstest::rstest]
	fn test_generate_initial_loader_no_initial_from() {
		let input = quote! {
			name: NoMappingForm,
			server_fn: update_data,
			initial_loader: get_data,

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// load_initial_values should still be generated
		assert!(output_str.contains("fn load_initial_values"));

		// But it should use _data since no fields use initial_from
		assert!(output_str.contains("let _data = get_data ()"));
	}

	#[rstest::rstest]
	fn test_generate_no_initial_loader() {
		let input = quote! {
			name: NoLoaderForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// load_initial_values should NOT be generated
		assert!(!output_str.contains("fn load_initial_values"));
	}

	#[rstest::rstest]
	fn test_generate_initial_loader_partial_mapping() {
		let input = quote! {
			name: PartialMappingForm,
			server_fn: update_profile,
			initial_loader: get_profile,

			fields: {
				username: CharField {
					initial_from: "username",
				},
				bio: TextField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Only username should have a setter
		assert!(output_str.contains("self . username . set"));

		// bio should NOT have a setter from initial data
		assert!(!output_str.contains("self . bio . set (data"));
	}

	#[rstest::rstest]
	fn test_generate_initial_loader_different_field_name() {
		let input = quote! {
			name: DifferentNameForm,
			server_fn: update_profile,
			initial_loader: get_profile,

			fields: {
				display_name: CharField {
					initial_from: "user_display_name",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should set display_name field from user_display_name in data
		assert!(output_str.contains("self . display_name . set"));
		assert!(output_str.contains("data . user_display_name"));
	}

	// =========================================================================
	// Combined Features Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_generate_full_step9_features() {
		let input = quote! {
			name: CompleteForm,
			server_fn: update_profile,
			initial_loader: get_profile,

			slots: {
				before_fields: || {
					view! { <h2>"Edit Profile"</h2> }
				},
				after_fields: || {
					view! { <button type="submit">"Save"</button> }
				},
			},

			fields: {
				username: CharField {
					required,
					label: "Username",
					initial_from: "username",
				},
				email: EmailField {
					required,
					label: "Email",
					initial_from: "email",
				},
				bio: TextField {
					label: "Biography",
					placeholder: "Tell us about yourself",
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check all Step 9 features are present
		// 1. initial_loader method
		assert!(output_str.contains("fn load_initial_values"));
		assert!(output_str.contains("get_profile"));

		// 2. initial_from mappings
		assert!(output_str.contains("data . username"));
		assert!(output_str.contains("data . email"));

		// 3. slots
		assert!(output_str.contains("fn into_page"));

		// 4. submit method
		assert!(output_str.contains("fn submit"));
	}

	// =========================================================================
	// Field Group Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_generate_field_group_basic() {
		let input = quote! {
			name: AddressForm,
			action: "/test",

			fields: {
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField { label: "Street" },
						city: CharField { label: "City" },
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check fieldset is generated
		assert!(
			output_str.contains("\"fieldset\""),
			"Expected fieldset element to be generated"
		);

		// Check legend is generated with label
		assert!(
			output_str.contains("\"legend\""),
			"Expected legend element to be generated"
		);
		assert!(
			output_str.contains("\"Address\""),
			"Expected legend text 'Address'"
		);

		// Check fields inside the group are generated
		assert!(output_str.contains("street"));
		assert!(output_str.contains("city"));
	}

	#[rstest::rstest]
	fn test_generate_field_group_with_class() {
		let input = quote! {
			name: StyledGroupForm,
			action: "/test",

			fields: {
				info_group: FieldGroup {
					label: "Information",
					class: "form-section border-gray-200",
					fields: {
						name: CharField {},
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check class is applied to fieldset
		assert!(
			output_str.contains("form-section border-gray-200"),
			"Expected class to be applied to fieldset"
		);
	}

	#[rstest::rstest]
	fn test_generate_field_group_without_label() {
		let input = quote! {
			name: NoLabelGroupForm,
			action: "/test",

			fields: {
				hidden_group: FieldGroup {
					class: "hidden-section",
					fields: {
						data: CharField {},
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Fieldset should exist
		assert!(output_str.contains("\"fieldset\""));

		// Legend should NOT exist when no label
		// The legend text should not appear
		// But the fieldset should have the class
		assert!(output_str.contains("hidden-section"));
	}

	#[rstest::rstest]
	fn test_generate_field_group_mixed_with_fields() {
		let input = quote! {
			name: MixedForm,
			action: "/test",

			fields: {
				email: EmailField { label: "Email" },
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField { label: "Street" },
						zip: CharField { label: "ZIP" },
					},
				},
				notes: TextField { label: "Notes" },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check all fields/groups are generated
		assert!(output_str.contains("email"));
		assert!(output_str.contains("\"fieldset\""));
		assert!(output_str.contains("street"));
		assert!(output_str.contains("zip"));
		assert!(output_str.contains("notes"));
	}

	#[rstest::rstest]
	fn test_generate_multiple_field_groups() {
		let input = quote! {
			name: MultiGroupForm,
			action: "/test",

			fields: {
				personal_group: FieldGroup {
					label: "Personal",
					class: "personal-section",
					fields: {
						name: CharField { label: "Name" },
						age: IntegerField { label: "Age" },
					},
				},
				contact_group: FieldGroup {
					label: "Contact",
					class: "contact-section",
					fields: {
						email: EmailField { label: "Email" },
						phone: CharField { label: "Phone" },
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check both groups are generated
		assert!(output_str.contains("personal-section"));
		assert!(output_str.contains("contact-section"));

		// Check both legends
		assert!(output_str.contains("\"Personal\""));
		assert!(output_str.contains("\"Contact\""));

		// Check all fields
		assert!(output_str.contains("name"));
		assert!(output_str.contains("age"));
		assert!(output_str.contains("email"));
		assert!(output_str.contains("phone"));
	}

	#[rstest::rstest]
	fn test_generate_field_group_with_initial_from() {
		let input = quote! {
			name: InitialGroupForm,
			server_fn: update_data,
			initial_loader: get_data,

			fields: {
				profile_group: FieldGroup {
					label: "Profile",
					fields: {
						name: CharField { initial_from: "name" },
						bio: TextField { initial_from: "biography" },
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check initial loader method is generated
		assert!(output_str.contains("fn load_initial_values"));

		// Check fields from group are mapped
		assert!(output_str.contains("data . name"));
		assert!(output_str.contains("data . biography"));
	}

	#[rstest::rstest]
	fn test_generate_field_group_accessors() {
		let input = quote! {
			name: AccessorGroupForm,
			action: "/test",

			fields: {
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField {},
						city: CharField {},
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check field accessors are generated for fields within groups
		assert!(
			output_str.contains("fn street"),
			"Expected street accessor to be generated"
		);
		assert!(
			output_str.contains("fn city"),
			"Expected city accessor to be generated"
		);

		// Fields in groups should have their Signals (checking struct fields)
		assert!(
			output_str.contains("street") && output_str.contains("Signal"),
			"Expected street Signal field to be generated"
		);
		assert!(
			output_str.contains("city") && output_str.contains("Signal"),
			"Expected city Signal field to be generated"
		);
	}

	#[rstest::rstest]
	fn test_generate_field_group_with_bind() {
		let input = quote! {
			name: BindGroupForm,
			action: "/test",

			fields: {
				credentials_group: FieldGroup {
					label: "Credentials",
					fields: {
						username: CharField { bind: true },
						password: CharField {
							widget: PasswordInput,
							bind: false,
						},
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that username has binding (on_input handler)
		assert!(
			output_str.contains("username"),
			"Expected username field to be generated"
		);

		// Check that password field exists
		assert!(
			output_str.contains("password"),
			"Expected password field to be generated"
		);
	}

	#[rstest::rstest]
	fn test_generate_field_group_with_wrapper() {
		let input = quote! {
			name: WrapperGroupForm,
			action: "/test",

			fields: {
				styled_group: FieldGroup {
					label: "Styled Fields",
					fields: {
						email: EmailField {
							label: "Email",
							wrapper: div { class: "relative" },
						},
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check wrapper is generated for field inside group
		assert!(
			output_str.contains("relative"),
			"Expected wrapper class to be applied"
		);
	}

	#[rstest::rstest]
	fn test_generate_field_group_with_icon() {
		let input = quote! {
			name: IconGroupForm,
			action: "/test",

			fields: {
				search_group: FieldGroup {
					label: "Search",
					fields: {
						query: CharField {
							icon: svg {
								viewBox: "0 0 24 24",
								path { d: "M21 21l-6-6" }
							},
							icon_position: "left",
						},
					},
				},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check SVG icon is generated for field inside group
		assert!(
			output_str.contains("\"svg\""),
			"Expected svg element to be generated"
		);
		assert!(
			output_str.contains("\"path\""),
			"Expected path element to be generated"
		);
	}

	// --- SubmitButton rendering regression tests (Fixes #3334) ---

	#[rstest::rstest]
	fn test_submit_button_rendered_in_url_action_form() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			action: "/api/login",

			fields: {
				username: CharField { required, label: "Username" },
				password: CharField { required, widget: PasswordInput, label: "Password" },
				submit: SubmitButton { label: "Sign in", class: "btn-primary" },
			},
		};

		// Act
		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Assert
		assert!(
			output_str.contains("\"button\""),
			"URL action form: codegen must emit a <button> element for SubmitButton"
		);
		assert!(
			output_str.contains("\"submit\""),
			"URL action form: codegen must set type=\"submit\" on the button"
		);
		assert!(
			output_str.contains("Sign in"),
			"URL action form: codegen must include the button label"
		);
		assert!(
			output_str.contains("btn-primary"),
			"URL action form: codegen must include the button class"
		);
	}

	#[rstest::rstest]
	fn test_submit_button_rendered_in_server_fn_form() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: login,

			fields: {
				username: CharField { required, label: "Username" },
				password: CharField { required, widget: PasswordInput, label: "Password" },
				submit: SubmitButton { label: "Sign in", class: "btn-primary" },
			},
		};

		// Act
		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Assert
		assert!(
			output_str.contains("\"button\""),
			"server_fn form: codegen must emit a <button> element for SubmitButton"
		);
		assert!(
			output_str.contains("\"submit\""),
			"server_fn form: codegen must set type=\"submit\" on the button"
		);
		assert!(
			output_str.contains("Sign in"),
			"server_fn form: codegen must include the button label"
		);
		assert!(
			output_str.contains("btn-primary"),
			"server_fn form: codegen must include the button class"
		);
	}

	#[rstest::rstest]
	fn test_submit_button_with_id_and_disabled() {
		// Arrange
		let input = quote! {
			name: TestForm,
			action: "/test",

			fields: {
				name: CharField { required },
				submit: SubmitButton { label: "Send", id: "submit-btn", disabled },
			},
		};

		// Act
		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Assert
		assert!(
			output_str.contains("\"button\""),
			"codegen must emit a <button> element"
		);
		assert!(
			output_str.contains("submit-btn"),
			"codegen must include the button id"
		);
		assert!(
			output_str.contains("\"disabled\""),
			"codegen must include the disabled attribute"
		);
	}

	#[rstest::rstest]
	fn test_submit_button_with_default_label() {
		// Arrange
		let input = quote! {
			name: MinimalForm,
			action: "/test",

			fields: {
				name: CharField { required },
				submit: SubmitButton {},
			},
		};

		// Act
		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Assert
		assert!(
			output_str.contains("\"button\""),
			"codegen must emit a <button> element even with no explicit label"
		);
		assert!(
			output_str.contains("Submit"),
			"codegen must use default label \"Submit\" when none specified"
		);
	}

	#[rstest::rstest]
	fn test_no_submit_button_when_not_specified() {
		// Arrange
		let input = quote! {
			name: NoButtonForm,
			action: "/test",

			fields: {
				name: CharField { required },
			},
		};

		// Act
		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Assert - no button element should be generated
		assert!(
			!output_str.contains("\"button\""),
			"form without SubmitButton must not generate a <button> element"
		);
	}

	#[test]
	fn hidden_field_with_generic_emits_typed_signal() {
		// Arrange
		let ft = TypedFieldType::HiddenField {
			inner: syn::parse_quote!(i64),
		};
		let pages_crate = quote::quote!(::reinhardt_pages);

		// Act
		let actual = field_type_to_signal_type(&ft, &pages_crate).to_string();

		// Assert
		let expected = quote::quote!(::reinhardt_pages::reactive::Signal<i64>).to_string();
		assert_eq!(actual, expected);
	}

	#[test]
	fn hidden_field_default_string_when_no_generic_specified() {
		// Arrange — validator substitutes String when the user omits generics
		let ft = TypedFieldType::HiddenField {
			inner: syn::parse_quote!(::std::string::String),
		};
		let pages_crate = quote::quote!(::reinhardt_pages);

		// Act
		let actual = field_type_to_signal_type(&ft, &pages_crate).to_string();

		// Assert
		let expected =
			quote::quote!(::reinhardt_pages::reactive::Signal<::std::string::String>).to_string();
		assert_eq!(actual, expected);
	}

	#[test]
	fn multiple_choice_emits_vec_of_inner() {
		// Arrange
		let ft = TypedFieldType::MultipleChoiceField {
			inner: syn::parse_quote!(i64),
		};
		let pages_crate = quote::quote!(::reinhardt_pages);

		// Act
		let actual = field_type_to_signal_type(&ft, &pages_crate).to_string();

		// Assert
		// Proc-macro token streams render nested angle brackets with a
		// space between `>` and `>` (e.g. `Vec < i64 > >`), matching
		// how `field_type_to_signal_type` constructs the type. The
		// expected string uses a literal to avoid `quote!` collapsing.
		let expected = ":: reinhardt_pages :: reactive :: Signal < :: std :: vec :: Vec < i64 > >";
		assert_eq!(actual, expected);
	}

	#[test]
	fn ip_address_field_is_specialized_to_option_ipaddr() {
		// Arrange
		let ft = TypedFieldType::IpAddressField;
		let pages_crate = quote::quote!(::reinhardt_pages);

		// Act
		let actual = field_type_to_signal_type(&ft, &pages_crate).to_string();

		// Assert
		let expected = ":: reinhardt_pages :: reactive :: Signal < :: core :: option :: Option < :: std :: net :: IpAddr > >";
		assert_eq!(actual, expected);
	}

	#[test]
	fn file_fields_emit_option_file_signals() {
		let pages_crate = quote::quote!(::reinhardt_pages);

		let file_actual =
			field_type_to_signal_type(&TypedFieldType::FileField, &pages_crate).to_string();
		let image_actual =
			field_type_to_signal_type(&TypedFieldType::ImageField, &pages_crate).to_string();

		let expected = ":: reinhardt_pages :: reactive :: Signal < :: core :: option :: Option < web_sys :: File > >";
		assert_eq!(file_actual, expected);
		assert_eq!(image_actual, expected);
		assert!(supports_generated_form_values(&TypedFieldType::FileField));
		assert!(supports_generated_form_values(&TypedFieldType::ImageField));
	}

	#[test]
	fn json_field_explicit_serde_json_value() {
		// Arrange
		let ft = TypedFieldType::JsonField {
			inner: syn::parse_quote!(::serde_json::Value),
		};
		let pages_crate = quote::quote!(::reinhardt_pages);

		// Act
		let actual = field_type_to_signal_type(&ft, &pages_crate).to_string();

		// Assert
		let expected =
			quote::quote!(::reinhardt_pages::reactive::Signal<::serde_json::Value>).to_string();
		assert_eq!(actual, expected);
	}

	#[test]
	fn hidden_field_default_value_is_t_default() {
		// Arrange
		let ft = TypedFieldType::HiddenField {
			inner: syn::parse_quote!(i64),
		};

		// Act
		let actual = field_type_default_value(&ft).to_string();

		// Assert
		let expected = quote::quote!(<i64 as ::core::default::Default>::default()).to_string();
		assert_eq!(actual, expected);
	}

	#[test]
	fn multiple_choice_default_value_is_vec_new() {
		// Arrange
		let ft = TypedFieldType::MultipleChoiceField {
			inner: syn::parse_quote!(i64),
		};

		// Act
		let actual = field_type_default_value(&ft).to_string();

		// Assert
		let expected = quote::quote!(::std::vec::Vec::new()).to_string();
		assert_eq!(actual, expected);
	}
}
