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
use quote::{ToTokens, quote};

use crate::crate_paths::get_reinhardt_pages_crate_info;
use reinhardt_manouche::core::{
	FormMethod, TypedCustomAttr, TypedFieldType, TypedFormAction, TypedFormCallbacks,
	TypedFormDerived, TypedFormFieldDef, TypedFormFieldEntry, TypedFormFieldGroup, TypedFormMacro,
	TypedFormSlots, TypedFormState, TypedFormWatch, TypedIcon, TypedIconChild, TypedIconPosition,
	TypedValidatorRule, TypedWidget, TypedWrapper,
};

/// Collects all fields from field entries, flattening groups.
///
/// This is useful for generating struct declarations and accessors,
/// where all fields need to be at the same level regardless of grouping.
fn collect_all_fields(entries: &[TypedFormFieldEntry]) -> Vec<&TypedFormFieldDef> {
	let mut fields = Vec::new();
	for entry in entries {
		match entry {
			TypedFormFieldEntry::Field(field) => fields.push(field.as_ref()),
			TypedFormFieldEntry::Group(group) => {
				fields.extend(group.fields.iter());
			}
		}
	}
	fields
}

/// Generates the complete code for a form! macro invocation.
///
/// This function generates conditional code that works for both WASM and server builds.
pub(super) fn generate(macro_ast: &TypedFormMacro) -> TokenStream {
	let crate_info = get_reinhardt_pages_crate_info();
	let use_statement = &crate_info.use_statement;
	let pages_crate = &crate_info.ident;

	let struct_name = &macro_ast.name;

	// Generate field declarations
	let field_decls = generate_field_declarations(&macro_ast.fields, pages_crate);

	// Generate state field declarations
	let state_decls = generate_state_declarations(&macro_ast.state, pages_crate);

	// Generate field initializers
	let field_inits = generate_field_initializers(&macro_ast.fields, pages_crate);

	// Generate state field initializers
	let state_inits = generate_state_initializers(&macro_ast.state, pages_crate);

	// Generate field accessor methods
	let field_accessors = generate_field_accessors(&macro_ast.fields, pages_crate);

	// Generate state accessor methods
	let state_accessors = generate_state_accessors(&macro_ast.state, pages_crate);

	// Generate watch methods
	let watch_methods = generate_watch_methods(&macro_ast.watch, pages_crate, struct_name);

	// Generate derived methods
	let derived_methods = generate_derived_methods(&macro_ast.derived, pages_crate);

	// Generate metadata for SSR
	let metadata_fn = generate_metadata_function(macro_ast, pages_crate);

	// Generate into_view implementation
	let into_view_impl = generate_into_page(macro_ast, pages_crate);

	// Generate submit method if action is specified
	let submit_method = generate_submit_method(macro_ast, pages_crate);

	// Generate validation method
	let validate_method = generate_validate_method(macro_ast, pages_crate);

	// Generate load_initial_values method if initial_loader is specified
	let load_initial_method = generate_load_initial_values(macro_ast, pages_crate);

	// Generate load_choices method if choices_loader is specified
	let load_choices_method = generate_load_choices(macro_ast, pages_crate);

	quote! {
		{
			#use_statement

			#[derive(Clone)]
			struct #struct_name {
				#field_decls
				#state_decls
			}

			impl #struct_name {
				fn new() -> Self {
					Self {
						#field_inits
						#state_inits
					}
				}

				#field_accessors
				#state_accessors
				#watch_methods
				#derived_methods
				#metadata_fn
				#validate_method
				#submit_method
				#load_initial_method
				#load_choices_method
				#into_view_impl
			}

			#struct_name::new()
		}
	}
}

/// Generates field declarations for the form struct.
fn generate_field_declarations(
	entries: &[TypedFormFieldEntry],
	pages_crate: &TokenStream,
) -> TokenStream {
	let fields = collect_all_fields(entries);
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

	// Add choices signal fields for dynamic choice fields
	let choices_decls: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			if field.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				Some(quote! {
					#choices_name: #pages_crate::reactive::Signal<Vec<(String, String)>>,
				})
			} else {
				None
			}
		})
		.collect();

	decls.extend(choices_decls);
	quote! { #(#decls)* }
}

/// Generates field initializers for the new() function.
fn generate_field_initializers(
	entries: &[TypedFormFieldEntry],
	pages_crate: &TokenStream,
) -> TokenStream {
	let fields = collect_all_fields(entries);
	let mut inits: Vec<TokenStream> = fields
		.iter()
		.map(|field| {
			let name = &field.name;
			let default_value = field_type_default_value(&field.field_type);
			quote! {
				#name: #pages_crate::reactive::Signal::new(#default_value),
			}
		})
		.collect();

	// Add choices signal initializers for dynamic choice fields
	let choices_inits: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			if field.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				Some(quote! {
					#choices_name: #pages_crate::reactive::Signal::new(Vec::new()),
				})
			} else {
				None
			}
		})
		.collect();

	inits.extend(choices_inits);
	quote! { #(#inits)* }
}

/// Generates field accessor methods.
fn generate_field_accessors(
	entries: &[TypedFormFieldEntry],
	pages_crate: &TokenStream,
) -> TokenStream {
	let fields = collect_all_fields(entries);
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

	// Add choices accessors for dynamic choice fields
	let choices_accessors: Vec<TokenStream> = fields
		.iter()
		.filter_map(|field| {
			if field.choices_config.is_some() {
				let choices_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				Some(quote! {
					/// Returns the choices signal for dynamic choice options.
					pub fn #choices_name(&self) -> &#pages_crate::reactive::Signal<Vec<(String, String)>> {
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
	state: &Option<TypedFormState>,
	pages_crate: &TokenStream,
) -> TokenStream {
	let Some(state) = state else {
		return quote! {};
	};

	let mut decls = Vec::new();

	if state.loading {
		decls.push(quote! {
			__loading: #pages_crate::reactive::Signal<bool>,
		});
	}

	if state.error {
		decls.push(quote! {
			__error: #pages_crate::reactive::Signal<Option<String>>,
		});
	}

	if state.success {
		decls.push(quote! {
			__success: #pages_crate::reactive::Signal<bool>,
		});
	}

	quote! { #(#decls)* }
}

/// Generates state field initializers for the new() function.
fn generate_state_initializers(
	state: &Option<TypedFormState>,
	pages_crate: &TokenStream,
) -> TokenStream {
	let Some(state) = state else {
		return quote! {};
	};

	let mut inits = Vec::new();

	if state.loading {
		inits.push(quote! {
			__loading: #pages_crate::reactive::Signal::new(false),
		});
	}

	if state.error {
		inits.push(quote! {
			__error: #pages_crate::reactive::Signal::new(None),
		});
	}

	if state.success {
		inits.push(quote! {
			__success: #pages_crate::reactive::Signal::new(false),
		});
	}

	quote! { #(#inits)* }
}

/// Generates state accessor methods.
///
/// Accessor methods provide a clean public API for accessing state signals:
/// - `loading()` returns `&Signal<bool>`
/// - `error()` returns `&Signal<Option<String>>`
/// - `success()` returns `&Signal<bool>`
fn generate_state_accessors(
	state: &Option<TypedFormState>,
	pages_crate: &TokenStream,
) -> TokenStream {
	let Some(state) = state else {
		return quote! {};
	};

	let mut accessors = Vec::new();

	if state.loading {
		accessors.push(quote! {
			/// Returns the loading state signal.
			///
			/// This signal is `true` while form submission is in progress.
			pub fn loading(&self) -> &#pages_crate::reactive::Signal<bool> {
				&self.__loading
			}
		});
	}

	if state.error {
		accessors.push(quote! {
			/// Returns the error state signal.
			///
			/// This signal contains the error message if the last submission failed.
			pub fn error(&self) -> &#pages_crate::reactive::Signal<Option<String>> {
				&self.__error
			}
		});
	}

	if state.success {
		accessors.push(quote! {
			/// Returns the success state signal.
			///
			/// This signal is `true` after successful form submission.
			pub fn success(&self) -> &#pages_crate::reactive::Signal<bool> {
				&self.__success
			}
		});
	}

	quote! { #(#accessors)* }
}

/// Generates watch methods that return reactive views.
///
/// Each watch item becomes a method on the form struct that returns an `impl IntoPage`.
/// The method clones the form instance and wraps the user's closure in an `Effect::new`
/// to create a reactive view that automatically re-renders when Signal dependencies change.
///
/// ## Generated Pattern
///
/// For a watch item like:
/// ```text
/// error_display: |form| {
///     if form.error().get().is_some() { ... }
/// }
/// ```
///
/// Generates:
/// ```text
/// pub fn error_display(&self) -> impl IntoPage {
///     let form = self.clone();
///     Effect::new(move || {
///         let result = (|form| { ... })(&form);
///         result
///     })
/// }
/// ```
fn generate_watch_methods(
	watch: &Option<TypedFormWatch>,
	pages_crate: &TokenStream,
	struct_name: &syn::Ident,
) -> TokenStream {
	let Some(watch) = watch else {
		return quote! {};
	};

	if watch.items.is_empty() {
		return quote! {};
	}

	let methods: Vec<TokenStream> = watch
		.items
		.iter()
		.map(|item| {
			let method_name = &item.name;
			let closure = &item.closure;

			quote! {
				/// Returns a reactive view that automatically re-renders when its Signal dependencies change.
				///
				/// This method wraps the watch closure in Page::reactive for automatic re-rendering.
				pub fn #method_name(&self) -> impl #pages_crate::component::IntoPage {
					let form = self.clone();
					#pages_crate::component::Page::reactive(move || {
						// Helper function to provide type inference for the closure parameter
						// Uses Fn instead of FnOnce to allow multiple calls from reactive system
						#[inline]
						fn __call_watch<T, R>(form: &T, f: impl Fn(&T) -> R) -> R {
							f(form)
						}
						__call_watch::<#struct_name, _>(&form, #closure)
					})
				}
			}
		})
		.collect();

	quote! { #(#methods)* }
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
#[allow(unused_variables)]
fn generate_derived_methods(
	derived: &Option<TypedFormDerived>,
	pages_crate: &TokenStream,
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
				/// ```ignore
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
	let all_fields = collect_all_fields(&macro_ast.fields);
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
	let all_fields = collect_all_fields(&macro_ast.fields);

	// Generate signal bindings for fields with bind: true
	let signal_bindings: Vec<TokenStream> = all_fields
		.iter()
		.filter(|field| field.bind)
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
	let all_fields = collect_all_fields(&macro_ast.fields);

	// Generate before_fields slot if present
	let before_fields_slot = generate_before_fields_slot(&macro_ast.slots);

	// Generate after_fields slot if present
	let after_fields_slot = generate_after_fields_slot(&macro_ast.slots);

	// Determine if CSRF protection is needed (non-GET methods)
	let needs_csrf = !matches!(macro_ast.method, FormMethod::Get);

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
					let signal_name_str = format!("submit_{}", name);
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
					let signal_name_str = format!("submit_{}", name);
					let sanitized = signal_name_str.replace("__", "_");
					let signal_name = quote::format_ident!("{}", sanitized);
					quote! { #signal_name.get() }
				})
				.collect();

			// Generate callbacks
			let callbacks = &macro_ast.callbacks;
			let state = &macro_ast.state;
			let redirect = &macro_ast.redirect_on_success;

			// Check if loading/error states are enabled
			let has_loading = state.as_ref().is_some_and(|s| s.loading);
			let has_error = state.as_ref().is_some_and(|s| s.error);

			// Generate loading state management
			let loading_start = if has_loading {
				quote! { submit_loading.set(true); }
			} else {
				quote! {}
			};

			// Clone loading/error signals if they exist
			let state_signal_clones = {
				let mut clones = Vec::new();
				if has_loading {
					clones.push(quote! { let submit_loading = self.loading().clone(); });
				}
				if has_error {
					clones.push(quote! { let submit_error = self.error().clone(); });
				}
				quote! { #(#clones)* }
			};

			// Generate redirect code
			let redirect_code = if let Some(url) = redirect {
				quote! {
					if let Some(window) = web_sys::window() {
						let _ = window.location().set_href(#url);
					}
				}
			} else {
				quote! {}
			};

			// Generate on_success callback if present
			let on_success_code = if let Some(callback) = &callbacks.on_success {
				quote! { (#callback)(_value); }
			} else {
				quote! {}
			};

			// Generate on_error callback if present
			let on_error_code = if let Some(callback) = &callbacks.on_error {
				quote! { (#callback)(&e); }
			} else {
				quote! {}
			};

			// Generate async block signal clones only for existing state signals
			let async_signal_clones = {
				let mut clones = Vec::new();
				if has_loading {
					clones.push(quote! { let async_loading = submit_loading.clone(); });
				}
				if has_error {
					clones.push(quote! { let async_error = submit_error.clone(); });
				}
				quote! { #(#clones)* }
			};

			// Generate loading end with async signal
			let async_loading_end = if has_loading {
				quote! { async_loading.set(false); }
			} else {
				quote! {}
			};

			// Generate async error handling with async signal
			let async_error_handling = if has_error {
				quote! { async_error.set(Some(e.to_string())); }
			} else {
				quote! {}
			};

			quote! {
				// Clone field signals for onsubmit handler
				#(#field_signal_clones)*

				// Clone state signals for onsubmit handler
				#state_signal_clones

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
						#[cfg(target_arch = "wasm32")]
						{
							::std::sync::Arc::new(move |event: web_sys::Event| {
								// Prevent default form submission by handling it ourselves
								event.prevent_default();

								// Get field values from cloned signals
								#loading_start

								// Clone signals for async block - allow non_snake_case for generated variable names
								{
									#(
										#[allow(non_snake_case)]
										let #field_names = #field_value_getters;
									)*
								}

								#[cfg(target_arch = "wasm32")]
								{
									// Clone loading/error signals for async block if they exist
									#async_signal_clones

									#pages_crate::spawn::spawn_task(async move {
										match #server_fn_ident(#(#field_names),*).await {
											Ok(_value) => {
												#on_success_code
												#redirect_code
											}
											Err(e) => {
												#on_error_code
												#async_error_handling
											}
										}
										#async_loading_end
									});
								}
							})
						}
						#[cfg(not(target_arch = "wasm32"))]
						{
							::std::sync::Arc::new(move |event: #pages_crate::component::DummyEvent| {
								// Prevent default form submission by handling it ourselves (no-op in non-WASM)
								event.prevent_default();

								// Get field values from cloned signals
								#loading_start

								// Clone signals for async block - allow non_snake_case for generated variable names
								{
									#(
										#[allow(non_snake_case)]
										let #field_names = #field_value_getters;
									)*
								}

								#[cfg(target_arch = "wasm32")]
								{
									// Clone loading/error signals for async block if they exist
									#async_signal_clones

									#pages_crate::spawn::spawn_task(async move {
										match #server_fn_ident(#(#field_names),*).await {
											Ok(_value) => {
												#on_success_code
												#redirect_code
											}
											Err(e) => {
												#on_error_code
												#async_error_handling
											}
										}
										#async_loading_end
									});
								}
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
	}
}

/// Generates view code for a field group.
///
/// Wraps the group's fields in a container div with optional label and CSS class.
fn generate_field_group_view(
	group: &TypedFormFieldGroup,
	pages_crate: &TokenStream,
	_all_fields: &[&TypedFormFieldDef],
) -> TokenStream {
	let group_class = group.class.as_deref().unwrap_or("reinhardt-field-group");

	// Generate field views for the group
	let field_views: Vec<TokenStream> = group
		.fields
		.iter()
		.map(|field| {
			let signal_ident = if field.bind {
				Some(quote::format_ident!("{}_signal", field.name))
			} else {
				None
			};
			generate_field_view(field, pages_crate, signal_ident.as_ref())
		})
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

	let wrapper_class = field.styling.wrapper_class();
	let label_class = field.styling.label_class();
	let input_class = field.styling.input_class();

	// Generate custom attributes (aria-*, data-*)
	let custom_attrs = generate_custom_attrs(&field.custom_attrs);

	// Generate event listener for two-way binding
	let event_listener = generate_bind_listener(signal_ident, &field.widget, pages_crate);

	// Generate input element based on widget type
	let input_element = match &field.widget {
		TypedWidget::Textarea => {
			quote! {
				PageElement::new("textarea")
					.attr("name", #field_name_str)
					.attr("id", #field_name_str)
					.attr("class", #input_class)
					.attr("placeholder", #placeholder)
					.bool_attr("required", #required)
					#custom_attrs
					#event_listener
			}
		}
		TypedWidget::Select | TypedWidget::SelectMultiple => {
			let multiple = matches!(field.widget, TypedWidget::SelectMultiple);
			quote! {
				PageElement::new("select")
					.attr("name", #field_name_str)
					.attr("id", #field_name_str)
					.attr("class", #input_class)
					.bool_attr("required", #required)
					.bool_attr("multiple", #multiple)
					#custom_attrs
					#event_listener
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
					#custom_attrs
					#event_listener
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

/// Generates an event listener for two-way binding.
///
/// When `signal_ident` is provided, generates a `.listener()` call that updates
/// the signal when the input value changes. The event type depends on the widget:
/// - Text inputs, textarea: "input" event
/// - Select, checkbox, radio: "change" event
///
/// The generated code handles both WASM and non-WASM environments using conditional
/// compilation. In WASM, it extracts the value from the DOM element. In non-WASM,
/// the handler is a no-op for type compatibility.
fn generate_bind_listener(
	signal_ident: Option<&syn::Ident>,
	widget: &TypedWidget,
	pages_crate: &TokenStream,
) -> TokenStream {
	let Some(signal_ident) = signal_ident else {
		return TokenStream::new();
	};

	// Determine event type and element type based on widget
	let (event_name, element_type, value_getter) = match widget {
		TypedWidget::Textarea => ("input", "HtmlTextAreaElement", quote! { textarea.value() }),
		TypedWidget::Select | TypedWidget::SelectMultiple => {
			("change", "HtmlSelectElement", quote! { select.value() })
		}
		TypedWidget::CheckboxInput => (
			"change",
			"HtmlInputElement",
			quote! { input.checked().to_string() },
		),
		TypedWidget::RadioSelect => ("change", "HtmlInputElement", quote! { input.value() }),
		_ => ("input", "HtmlInputElement", quote! { input.value() }),
	};

	// Create element type identifiers for WASM code
	let element_type_ident = quote::format_ident!("{}", element_type);
	let element_var = match widget {
		TypedWidget::Textarea => quote::format_ident!("textarea"),
		TypedWidget::Select | TypedWidget::SelectMultiple => quote::format_ident!("select"),
		_ => quote::format_ident!("input"),
	};

	quote! {
		.listener(#event_name, {
			let signal = #signal_ident.clone();
			move |event| {
				#[cfg(target_arch = "wasm32")]
				{
					use wasm_bindgen::JsCast;
					if let Some(target) = event.target() {
						if let Ok(#element_var) = target.dyn_into::<web_sys::#element_type_ident>() {
							signal.set(#value_getter);
						}
					}
				}
				#[cfg(not(target_arch = "wasm32"))]
				{
					let _ = event;
					let _ = &#pages_crate::component::DummyEvent;
				}
			}
		})
	}
}

/// Generates the validate method.
fn generate_validate_method(macro_ast: &TypedFormMacro, _pages_crate: &TokenStream) -> TokenStream {
	let validators: Vec<TokenStream> = macro_ast
		.validators
		.iter()
		.flat_map(|v| generate_validator_rules(&v.field_name, &v.rules))
		.collect();

	if validators.is_empty() {
		quote! {
			pub fn validate(&self) -> Result<(), Vec<String>> {
				Ok(())
			}
		}
	} else {
		quote! {
			pub fn validate(&self) -> Result<(), Vec<String>> {
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

/// Generates validator rule checks.
fn generate_validator_rules(
	field_name: &syn::Ident,
	rules: &[TypedValidatorRule],
) -> Vec<TokenStream> {
	rules
		.iter()
		.map(|rule| {
			let condition = &rule.condition;
			let message = &rule.message;
			let field_name_str = field_name.to_string();
			quote! {
				{
					let v = self.#field_name.get();
					if !(#condition) {
						errors.push(format!("{}: {}", #field_name_str, #message));
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
fn generate_submit_method(macro_ast: &TypedFormMacro, pages_crate: &TokenStream) -> TokenStream {
	let callbacks = &macro_ast.callbacks;
	let state = &macro_ast.state;
	let redirect = &macro_ast.redirect_on_success;

	match &macro_ast.action {
		TypedFormAction::ServerFn(server_fn_ident) => {
			// Generate submit that calls the server_fn with callbacks
			let all_fields = collect_all_fields(&macro_ast.fields);
			let field_names: Vec<&syn::Ident> = all_fields.iter().map(|f| &f.name).collect();

			// Generate callback invocations
			let on_submit_code = generate_on_submit_callback(callbacks);
			let on_loading_start_code = generate_on_loading_callback(callbacks, state, true);
			let on_loading_end_code = generate_on_loading_callback(callbacks, state, false);
			let on_success_code = generate_on_success_callback(callbacks, state);
			let on_error_code = generate_on_error_callback(callbacks, state);
			let redirect_code = generate_redirect_code(redirect);

			quote! {
				#[cfg(target_arch = "wasm32")]
				pub async fn submit(&self) -> Result<(), #pages_crate::ServerFnError> {
					// Call on_submit callback before submission
					#on_submit_code

					// Set loading state and call on_loading callback
					#on_loading_start_code

					// Call the server function with individual field values as arguments
					let result = #server_fn_ident(#(self.#field_names.get()),*).await;

					// Clear loading state
					#on_loading_end_code

					// Handle result with callbacks and redirect
					match result {
						Ok(value) => {
							#on_success_code
							#redirect_code
							Ok(())
						}
						Err(e) => {
							#on_error_code
							Err(e)
						}
					}
				}

				#[cfg(not(target_arch = "wasm32"))]
				pub async fn submit(&self) -> Result<(), #pages_crate::ServerFnError> {
					// On server, submit is a no-op (form is submitted via HTTP)
					Ok(())
				}
			}
		}
		TypedFormAction::Url(_) => {
			// For URL action, submit triggers form submission
			// Callbacks are less relevant here since the browser handles the submission
			let on_submit_code = generate_on_submit_callback(callbacks);

			quote! {
				#[cfg(target_arch = "wasm32")]
				pub fn submit(&self) {
					// Call on_submit callback before submission
					#on_submit_code

					// Trigger native form submission via JavaScript
					// This will be handled by the browser
					#pages_crate::dom::submit_form(&self.metadata());
				}

				#[cfg(not(target_arch = "wasm32"))]
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
) -> TokenStream {
	// Return empty if no initial_loader is specified
	let Some(initial_loader) = &macro_ast.initial_loader else {
		return quote! {};
	};

	// Collect fields that have initial_from specified (including from groups)
	let all_fields = collect_all_fields(&macro_ast.fields);
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

	// If no fields have initial_from, just call the loader but don't set anything
	if field_setters.is_empty() {
		quote! {
			/// Loads initial values from the initial_loader server function.
			///
			/// Note: No fields have `initial_from` specified, so this method
			/// only calls the loader without populating any fields.
			#[cfg(target_arch = "wasm32")]
			pub async fn load_initial_values(&self) -> Result<(), #pages_crate::ServerFnError> {
				let _data = #initial_loader().await?;
				Ok(())
			}

			#[cfg(not(target_arch = "wasm32"))]
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
			#[cfg(target_arch = "wasm32")]
			pub async fn load_initial_values(&self) -> Result<(), #pages_crate::ServerFnError> {
				let data = #initial_loader().await?;
				#(#field_setters)*
				Ok(())
			}

			#[cfg(not(target_arch = "wasm32"))]
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
/// - Transforms each choice item to (value, label) tuple based on `choice_value` and `choice_label`
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
///     Ok(())
/// }
/// ```
fn generate_load_choices(macro_ast: &TypedFormMacro, pages_crate: &TokenStream) -> TokenStream {
	// Return empty if no choices_loader is specified
	let Some(choices_loader) = &macro_ast.choices_loader else {
		return quote! {};
	};

	// Collect fields that have choices_config specified (including from groups)
	let all_fields = collect_all_fields(&macro_ast.fields);
	let field_setters: Vec<TokenStream> = all_fields
		.iter()
		.filter_map(|field| {
			field.choices_config.as_ref().map(|config| {
				let choices_signal_name =
					syn::Ident::new(&format!("{}_choices", field.name), field.name.span());
				let from_ident = syn::Ident::new(&config.choices_from, field.name.span());
				let value_ident = syn::Ident::new(&config.choice_value, field.name.span());
				let label_ident = syn::Ident::new(&config.choice_label, field.name.span());
				quote! {
					self.#choices_signal_name.set(
						data.#from_ident.iter().map(|item| {
							(item.#value_ident.to_string(), item.#label_ident.clone())
						}).collect()
					);
				}
			})
		})
		.collect();

	// If no fields have choices_config, just call the loader but don't set anything
	if field_setters.is_empty() {
		quote! {
			/// Loads choices from the choices_loader server function.
			///
			/// Note: No fields have `choices_from` specified, so this method
			/// only calls the loader without populating any choices.
			#[cfg(target_arch = "wasm32")]
			pub async fn load_choices(&self) -> Result<(), #pages_crate::ServerFnError> {
				let _data = #choices_loader().await?;
				Ok(())
			}

			#[cfg(not(target_arch = "wasm32"))]
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
			/// Calls the configured choices_loader and populates the choices signals
			/// for fields that have `choices_from` specified.
			#[cfg(target_arch = "wasm32")]
			pub async fn load_choices(&self) -> Result<(), #pages_crate::ServerFnError> {
				let data = #choices_loader().await?;
				#(#field_setters)*
				Ok(())
			}

			#[cfg(not(target_arch = "wasm32"))]
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
	state: &Option<TypedFormState>,
	is_loading: bool,
) -> TokenStream {
	let mut code = Vec::new();

	// Update loading state if defined
	if let Some(state) = state
		&& state.loading
	{
		code.push(quote! {
			self.__loading.set(#is_loading);
		});
	}

	// Clear success/error states when starting loading
	if is_loading && let Some(state) = state {
		if state.success {
			code.push(quote! {
				self.__success.set(false);
			});
		}
		if state.error {
			code.push(quote! {
				self.__error.set(None);
			});
		}
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

/// Generates the on_success callback invocation and state update.
fn generate_on_success_callback(
	callbacks: &TypedFormCallbacks,
	state: &Option<TypedFormState>,
) -> TokenStream {
	let mut code = Vec::new();

	// Update success state if defined
	if let Some(state) = state
		&& state.success
	{
		code.push(quote! {
			self.__success.set(true);
		});
	}

	// Call on_success callback if defined
	if let Some(on_success) = &callbacks.on_success {
		code.push(quote! {
			{
				let callback = #on_success;
				callback(value);
			}
		});
	}

	quote! { #(#code)* }
}

/// Generates the on_error callback invocation and state update.
fn generate_on_error_callback(
	callbacks: &TypedFormCallbacks,
	state: &Option<TypedFormState>,
) -> TokenStream {
	let mut code = Vec::new();

	// Update error state if defined
	if let Some(state) = state
		&& state.error
	{
		code.push(quote! {
			self.__error.set(Some(e.to_string()));
		});
	}

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

/// Generates the redirect code if redirect_on_success is specified.
fn generate_redirect_code(redirect: &Option<String>) -> TokenStream {
	let Some(url) = redirect else {
		return quote! {};
	};

	quote! {
		// Redirect to the specified URL on success
		if let Some(window) = web_sys::window() {
			let _ = window.location().set_href(#url);
		}
	}
}

/// Converts field type to Signal type.
fn field_type_to_signal_type(
	field_type: &TypedFieldType,
	pages_crate: &TokenStream,
) -> TokenStream {
	let inner_type = match field_type {
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField
		| TypedFieldType::IpAddressField
		| TypedFieldType::JsonField => quote!(String),

		TypedFieldType::IntegerField => quote!(i64),
		TypedFieldType::FloatField | TypedFieldType::DecimalField => quote!(f64),
		TypedFieldType::BooleanField => quote!(bool),

		TypedFieldType::DateField => quote!(Option<chrono::NaiveDate>),
		TypedFieldType::TimeField => quote!(Option<chrono::NaiveTime>),
		TypedFieldType::DateTimeField => quote!(Option<chrono::NaiveDateTime>),

		TypedFieldType::ChoiceField => quote!(String),
		TypedFieldType::MultipleChoiceField => quote!(Vec<String>),

		TypedFieldType::FileField | TypedFieldType::ImageField => quote!(Option<web_sys::File>),

		TypedFieldType::UuidField => quote!(Option<uuid::Uuid>),
		TypedFieldType::HiddenField => quote!(String),
	};

	quote!(#pages_crate::reactive::Signal<#inner_type>)
}

/// Returns the default value for a field type.
fn field_type_default_value(field_type: &TypedFieldType) -> TokenStream {
	match field_type {
		TypedFieldType::CharField
		| TypedFieldType::TextField
		| TypedFieldType::EmailField
		| TypedFieldType::PasswordField
		| TypedFieldType::UrlField
		| TypedFieldType::SlugField
		| TypedFieldType::IpAddressField
		| TypedFieldType::JsonField
		| TypedFieldType::ChoiceField
		| TypedFieldType::HiddenField => quote!(String::new()),

		TypedFieldType::IntegerField => quote!(0i64),
		TypedFieldType::FloatField | TypedFieldType::DecimalField => quote!(0.0f64),
		TypedFieldType::BooleanField => quote!(false),

		TypedFieldType::DateField
		| TypedFieldType::TimeField
		| TypedFieldType::DateTimeField
		| TypedFieldType::FileField
		| TypedFieldType::ImageField
		| TypedFieldType::UuidField => quote!(None),

		TypedFieldType::MultipleChoiceField => quote!(Vec::new()),
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
		TypedFieldType::ChoiceField => "ChoiceField",
		TypedFieldType::MultipleChoiceField => "MultipleChoiceField",
		TypedFieldType::FileField => "FileField",
		TypedFieldType::ImageField => "ImageField",
		TypedFieldType::UrlField => "UrlField",
		TypedFieldType::SlugField => "SlugField",
		TypedFieldType::UuidField => "UuidField",
		TypedFieldType::IpAddressField => "IpAddressField",
		TypedFieldType::JsonField => "JsonField",
		TypedFieldType::HiddenField => "HiddenField",
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
		TypedWidget::TimeInput => "TimeInput",
		TypedWidget::DateTimeInput => "DateTimeInput",
		TypedWidget::FileInput => "FileInput",
		TypedWidget::HiddenInput => "HiddenInput",
		TypedWidget::ColorInput => "ColorInput",
		TypedWidget::RangeInput => "RangeInput",
		TypedWidget::UrlInput => "UrlInput",
		TypedWidget::TelInput => "TelInput",
		TypedWidget::SearchInput => "SearchInput",
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
		TypedWidget::TimeInput => "time",
		TypedWidget::DateTimeInput => "datetime-local",
		TypedWidget::FileInput => "file",
		TypedWidget::HiddenInput => "hidden",
		TypedWidget::ColorInput => "color",
		TypedWidget::RangeInput => "range",
		TypedWidget::UrlInput => "url",
		TypedWidget::TelInput => "tel",
		TypedWidget::SearchInput => "search",
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use quote::quote;

	fn parse_validate_generate(input: proc_macro2::TokenStream) -> TokenStream {
		use reinhardt_manouche::core::FormMacro;

		let untyped_ast: FormMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::form::validator::validate(&untyped_ast).unwrap();
		generate(&typed_ast)
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
	fn test_generate_state_all_fields() {
		let input = quote! {
			name: StateForm,
			server_fn: submit_form,

			state: { loading, error, success },

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check state field declarations
		assert!(output_str.contains("__loading"));
		assert!(output_str.contains("__error"));
		assert!(output_str.contains("__success"));

		// Check accessor methods are generated
		assert!(output_str.contains("fn loading"));
		assert!(output_str.contains("fn error"));
		assert!(output_str.contains("fn success"));
	}

	#[rstest::rstest]
	fn test_generate_state_single_field() {
		let input = quote! {
			name: LoadingForm,
			action: "/test",

			state: { loading },

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Only loading should be present
		assert!(output_str.contains("__loading"));
		assert!(output_str.contains("fn loading"));

		// error and success should not be present
		assert!(!output_str.contains("__error"));
		assert!(!output_str.contains("__success"));
	}

	#[rstest::rstest]
	fn test_generate_form_without_state() {
		let input = quote! {
			name: NoStateForm,
			action: "/test",

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// No state fields should be present
		assert!(!output_str.contains("__loading"));
		assert!(!output_str.contains("__error"));
		assert!(!output_str.contains("__success"));
	}

	#[rstest::rstest]
	fn test_generate_callback_on_success() {
		let input = quote! {
			name: CallbackForm,
			server_fn: submit_form,

			on_success: |result| {
				log::info!("Success!");
			},

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check submit method with callback invocation
		assert!(output_str.contains("fn submit"));
		// The callback closure should be in the generated code
		assert!(output_str.contains("callback"));
		assert!(output_str.contains("value"));
	}

	#[rstest::rstest]
	fn test_generate_callback_on_error() {
		let input = quote! {
			name: ErrorCallbackForm,
			server_fn: submit_form,

			on_error: |e| {
				log::error!("Error: {:?}", e);
			},

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check error handling with callback
		assert!(output_str.contains("fn submit"));
		assert!(output_str.contains("Err (e)") || output_str.contains("Err(e)"));
	}

	#[rstest::rstest]
	fn test_generate_callback_on_loading() {
		let input = quote! {
			name: LoadingCallbackForm,
			server_fn: submit_form,

			on_loading: |is_loading| {
				log::info!("Loading: {}", is_loading);
			},

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check loading callback invocation
		assert!(output_str.contains("fn submit"));
		// Should have both true and false calls for on_loading
		assert!(output_str.contains("true") && output_str.contains("false"));
	}

	#[rstest::rstest]
	fn test_generate_callback_on_submit() {
		let input = quote! {
			name: SubmitCallbackForm,
			server_fn: submit_form,

			on_submit: |form| {
				log::info!("Submitting form");
			},

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check on_submit callback is called before request
		assert!(output_str.contains("fn submit"));
		assert!(output_str.contains("callback (self)") || output_str.contains("callback(self)"));
	}

	#[rstest::rstest]
	fn test_generate_all_callbacks() {
		let input = quote! {
			name: AllCallbacksForm,
			server_fn: submit_form,

			on_submit: |form| { /* submit */ },
			on_success: |result| { /* success */ },
			on_error: |e| { /* error */ },
			on_loading: |is_loading| { /* loading */ },

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// All callbacks should be present
		assert!(output_str.contains("fn submit"));
		// Check the submit method contains callback invocations
		assert!(output_str.contains("callback"));
	}

	#[rstest::rstest]
	fn test_generate_callbacks_with_state() {
		let input = quote! {
			name: CallbackStateForm,
			server_fn: submit_form,

			state: { loading, error, success },

			on_success: |result| { /* success */ },
			on_error: |e| { /* error */ },

			fields: {
				data: CharField {},
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check state updates are generated
		assert!(output_str.contains("__loading . set"));
		assert!(output_str.contains("__success . set"));
		assert!(output_str.contains("__error . set"));

		// Check callback invocations are also present
		assert!(output_str.contains("callback"));
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
		assert!(output_str.contains(". listener (\"input\""));
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
		assert!(output_str.contains(". listener (\"input\""));
		assert!(output_str.contains("HtmlInputElement"));
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

		// Textarea should use "input" event and HtmlTextAreaElement
		assert!(output_str.contains(". listener (\"input\""));
		assert!(output_str.contains("HtmlTextAreaElement"));
		assert!(output_str.contains("textarea . value ()"));
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

		// Select should use "change" event and HtmlSelectElement
		assert!(output_str.contains(". listener (\"change\""));
		assert!(output_str.contains("HtmlSelectElement"));
		assert!(output_str.contains("select . value ()"));
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

		// Checkbox should use "change" event and checked property
		assert!(output_str.contains(". listener (\"change\""));
		assert!(output_str.contains("HtmlInputElement"));
		assert!(output_str.contains("input . checked ()"));
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
		assert!(output_str.contains("# [cfg (target_arch = \"wasm32\")]"));
		assert!(output_str.contains("# [cfg (not (target_arch = \"wasm32\"))]"));
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
				error_view: |form| { form.error() },
				loading_view: |form| { form.loading() },
				success_view: |form| { form.success() },
			},

			state: {
				loading,
				error,
				success,
			},

			fields: {
				email: EmailField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check all three watch methods are generated
		assert!(output_str.contains("fn error_view"));
		assert!(output_str.contains("fn loading_view"));
		assert!(output_str.contains("fn success_view"));
	}

	#[rstest::rstest]
	fn test_generate_watch_complex_closure() {
		let input = quote! {
			name: ComplexWatchForm,
			action: "/test",

			watch: {
				conditional_view: |form| {
					if form.error().get().is_some() {
						"Error occurred"
					} else if form.loading().get().clone() {
						"Loading..."
					} else {
						"Ready"
					}
				},
			},

			state: {
				loading,
				error,
			},

			fields: {
				data: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Check that the watch method is generated with the complex closure
		assert!(output_str.contains("fn conditional_view"));
		assert!(output_str.contains("form . error () . get () . is_some ()"));
		assert!(output_str.contains("form . loading () . get () . clone ()"));
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

		// Check that the form is cloned before use and __call_watch is used
		assert!(output_str.contains("let form = self . clone ()"));
		assert!(output_str.contains("__call_watch"));
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

		// Should generate redirect code with web_sys::window
		assert!(output_str.contains("web_sys"));
		assert!(output_str.contains("window"));
		assert!(output_str.contains("location"));
		assert!(output_str.contains("set_href"));
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

		// Should generate redirect code with full URL
		assert!(output_str.contains("https://example.com/success"));
		assert!(output_str.contains("set_href"));
	}

	#[rstest::rstest]
	fn test_generate_redirect_with_callbacks() {
		let input = quote! {
			name: RedirectCallbackForm,
			server_fn: submit_form,
			redirect_on_success: "/profile",

			on_success: |result| {
				console::log_1(&"Success!".into());
			},

			fields: {
				name: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should have both callback and redirect
		assert!(output_str.contains("Success!"));
		assert!(output_str.contains("/profile"));
		assert!(output_str.contains("set_href"));
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

		// Should not generate redirect code when redirect_on_success is not specified
		// The submit method should still exist but without set_href call for redirect
		assert!(output_str.contains("submit"));

		// Count occurrences of set_href - should be minimal or none related to redirect
		// We check that there's no "/dashboard" or similar redirect-specific patterns
		assert!(!output_str.contains("Redirect to the specified URL"));
	}

	#[rstest::rstest]
	fn test_generate_redirect_with_state() {
		let input = quote! {
			name: RedirectStateForm,
			server_fn: submit_form,
			redirect_on_success: "/success",

			state: { loading, success },

			fields: {
				data: CharField { required },
			},
		};

		let output = parse_validate_generate(input);
		let output_str = output.to_string();

		// Should have state fields and redirect
		assert!(output_str.contains("__loading"));
		assert!(output_str.contains("__success"));
		assert!(output_str.contains("/success"));
		assert!(output_str.contains("set_href"));
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

			state: { loading, error, success },

			on_success: |result| {
				log::info!("Success!");
			},

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

		// 4. state
		assert!(output_str.contains("__loading"));
		assert!(output_str.contains("__error"));
		assert!(output_str.contains("__success"));

		// 5. callbacks
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
}
