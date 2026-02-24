//! Implementation of the `#[document(...)]` attribute macro.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
	Data, DeriveInput, Fields, GenericArgument, Lit, PathArguments, Type, parse_macro_input,
};

pub(crate) mod attr_parser;

use attr_parser::DocumentAttrs;

/// Collected information about a single struct field.
struct FieldInfo {
	ident: syn::Ident,
	ty: syn::Type,
	attrs: crate::field::attr_parser::FieldAttrs,
	is_option: bool,
}

/// Implementation of the `#[document(...)]` attribute macro.
pub(crate) fn document_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
	// Parse attributes
	let attrs = parse_macro_input!(attr as DocumentAttrs);

	// Parse the struct
	let mut input = parse_macro_input!(item as DeriveInput);

	// Extract struct information
	let struct_name = &input.ident;
	let collection = &attrs.collection;
	let database = attrs.database.as_deref().unwrap_or("default");

	// Extract fields and find primary key
	let fields = match &input.data {
		Data::Struct(data) => match &data.fields {
			Fields::Named(fields) => &fields.named,
			_ => {
				return syn::Error::new_spanned(
					&input,
					"#[document(...)] only supports structs with named fields",
				)
				.to_compile_error()
				.into();
			}
		},
		_ => {
			return syn::Error::new_spanned(&input, "#[document(...)] only supports structs")
				.to_compile_error()
				.into();
		}
	};

	// Step 2.1: Collect all field information
	let mut field_infos = Vec::new();
	let mut id_type = None;
	let mut id_field_name = None;

	for field in fields {
		let ident = field.ident.as_ref().unwrap().clone();
		let ty = field.ty.clone();
		let is_option = extract_option_inner_type(&ty).is_some();

		let mut field_attrs = crate::field::attr_parser::FieldAttrs::default();
		for attr in &field.attrs {
			if attr.path().is_ident("field") {
				if let Ok(parsed) = attr.parse_args::<crate::field::attr_parser::FieldAttrs>() {
					field_attrs = parsed;
				} else if let Ok(meta) = attr.parse_args::<syn::Ident>()
					&& meta == "primary_key"
				{
					field_attrs.primary_key = true;
				}
			}
		}

		if field_attrs.primary_key {
			let inner_type = extract_option_inner_type(&ty).unwrap_or_else(|| ty.clone());
			id_type = Some(quote! { #inner_type });
			id_field_name = Some(quote! { #ident });
		}

		field_infos.push(FieldInfo {
			ident,
			ty,
			attrs: field_attrs,
			is_option,
		});
	}

	let id_type = match id_type {
		Some(t) => t,
		None => {
			return syn::Error::new_spanned(
				&input,
				"No primary key field found. Add #[field(primary_key)] to one field.",
			)
			.to_compile_error()
			.into();
		}
	};
	let id_field_name = id_field_name.unwrap();

	// Step 2.2: Inject serde attributes and strip #[field(...)]
	let mut default_fns = Vec::new();
	if let Data::Struct(ref mut data) = input.data
		&& let Fields::Named(ref mut fields) = data.fields
	{
		for (field, info) in fields.named.iter_mut().zip(field_infos.iter()) {
			inject_serde_attrs(field, info, struct_name, &mut default_fns);
			field.attrs.retain(|attr| !attr.path().is_ident("field"));
		}
	}

	// Step 2.3: Generate backend_type()
	let backend_type_fn = gen_backend_type(&attrs.backend);

	// Step 2.4: Generate indexes()
	let indexes_fn = gen_indexes(&field_infos);

	// Step 2.5: Generate validate()
	let validate_fn = gen_validate(&field_infos);

	// Step 2.6: Generate validation_schema()
	let validation_schema_fn = gen_validation_schema(&field_infos);

	// Step 2.7: Generate references()
	let references_fn = gen_references(&field_infos);

	// Combine default function impls
	let default_fns_tokens: TokenStream2 = if default_fns.is_empty() {
		quote! {}
	} else {
		let fns = &default_fns;
		quote! {
			#[automatically_derived]
			impl #struct_name {
				#(#fns)*
			}
		}
	};

	// Generate Document trait implementation
	let expanded = quote! {
		#input

		#default_fns_tokens

		#[automatically_derived]
		impl reinhardt_db::nosql::document::Document for #struct_name {
			type Id = #id_type;

			const COLLECTION_NAME: &'static str = #collection;
			const DATABASE_NAME: &'static str = #database;

			fn id(&self) -> Option<&Self::Id> {
				self.#id_field_name.as_ref()
			}

			fn set_id(&mut self, id: Self::Id) {
				self.#id_field_name = Some(id);
			}

			#backend_type_fn

			#indexes_fn

			#validate_fn

			#validation_schema_fn

			#references_fn
		}
	};

	TokenStream::from(expanded)
}

/// Extracts the inner type from `Option<T>`, returning `Some(T)` if successful.
/// Returns `None` if the type is not an `Option`.
fn extract_option_inner_type(ty: &Type) -> Option<Type> {
	let Type::Path(type_path) = ty else {
		return None;
	};

	let last_segment = type_path.path.segments.last()?;

	// Check if this is Option (or std::option::Option, core::option::Option)
	if last_segment.ident != "Option" {
		return None;
	}

	let PathArguments::AngleBracketed(args) = &last_segment.arguments else {
		return None;
	};

	if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
		return Some(inner_ty.clone());
	}

	None
}

/// Inject serde attributes based on field attributes and collect default function definitions.
fn inject_serde_attrs(
	field: &mut syn::Field,
	info: &FieldInfo,
	struct_name: &syn::Ident,
	default_fns: &mut Vec<TokenStream2>,
) {
	if info.attrs.primary_key {
		if info.is_option {
			field.attrs.push(syn::parse_quote! {
				#[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
			});
		} else {
			field.attrs.push(syn::parse_quote! {
				#[serde(rename = "_id")]
			});
		}
	} else if let Some(ref rename) = info.attrs.rename {
		field.attrs.push(syn::parse_quote! {
			#[serde(rename = #rename)]
		});
	}

	if let Some(ref default_val) = info.attrs.default {
		let fn_name = format_ident!("__default_{}", info.ident);
		let ty = &info.ty;
		let default_expr = parse_default_value(default_val, ty);

		let fn_name_str = format!("{}::{}", struct_name, fn_name);
		field.attrs.push(syn::parse_quote! {
			#[serde(default = #fn_name_str)]
		});

		default_fns.push(quote! {
			fn #fn_name() -> #ty {
				#default_expr
			}
		});
	}
}

/// Parse a default value string into a token stream based on the field type.
fn parse_default_value(value: &str, ty: &Type) -> TokenStream2 {
	let type_str = quote!(#ty).to_string();

	if type_str.contains("String") {
		quote! { String::from(#value) }
	} else if type_str.contains("bool") {
		if value == "true" {
			quote! { true }
		} else if value == "false" {
			quote! { false }
		} else {
			let msg = format!(
				"Invalid boolean default value: '{}'. Expected 'true' or 'false'.",
				value
			);
			quote! { compile_error!(#msg) }
		}
	} else if type_str.contains("f32") || type_str.contains("f64") {
		match value.parse::<f64>() {
			Ok(val) => {
				if type_str.contains("f32") {
					let v = val as f32;
					quote! { #v }
				} else {
					quote! { #val }
				}
			}
			Err(_) => {
				let msg = format!(
					"Invalid float default value: '{}'. Expected a valid floating-point number.",
					value
				);
				quote! { compile_error!(#msg) }
			}
		}
	} else {
		// Integer types
		match value.parse::<i64>() {
			Ok(val) => {
				if type_str.contains("i32") {
					let v = val as i32;
					quote! { #v }
				} else if type_str.contains("i64") {
					quote! { #val }
				} else if type_str.contains("u32") {
					let v = val as u32;
					quote! { #v }
				} else if type_str.contains("u64") {
					let v = val as u64;
					quote! { #v }
				} else {
					// Fallback: try as string
					quote! { String::from(#value) }
				}
			}
			Err(_) => {
				let msg = format!(
					"Invalid integer default value: '{}'. Expected a valid integer.",
					value
				);
				quote! { compile_error!(#msg) }
			}
		}
	}
}

/// Generate `backend_type()` method implementation.
fn gen_backend_type(backend: &str) -> TokenStream2 {
	match backend {
		"mongodb" => quote! {
			fn backend_type() -> reinhardt_db::nosql::types::NoSQLBackendType {
				reinhardt_db::nosql::types::NoSQLBackendType::MongoDB
			}
		},
		_ => quote! {},
	}
}

/// Determine the BSON field name for a field.
fn bson_field_name(info: &FieldInfo) -> String {
	if info.attrs.primary_key {
		"_id".to_string()
	} else if let Some(ref rename) = info.attrs.rename {
		rename.clone()
	} else {
		info.ident.to_string()
	}
}

/// Generate `indexes()` method implementation.
fn gen_indexes(field_infos: &[FieldInfo]) -> TokenStream2 {
	let mut index_entries = Vec::new();

	for info in field_infos {
		if !info.attrs.index && !info.attrs.unique {
			continue;
		}

		let bson_name = bson_field_name(info);
		let is_unique = info.attrs.unique;

		if is_unique {
			index_entries.push(quote! {
				reinhardt_db::nosql::document::IndexModel::builder()
					.key(#bson_name, reinhardt_db::nosql::document::IndexOrder::Ascending)
					.unique(true)
					.build()
			});
		} else {
			index_entries.push(quote! {
				reinhardt_db::nosql::document::IndexModel::builder()
					.key(#bson_name, reinhardt_db::nosql::document::IndexOrder::Ascending)
					.build()
			});
		}
	}

	if index_entries.is_empty() {
		return quote! {};
	}

	quote! {
		fn indexes() -> Vec<reinhardt_db::nosql::document::IndexModel> {
			vec![
				#(#index_entries),*
			]
		}
	}
}

/// Generate `validate()` method implementation.
fn gen_validate(field_infos: &[FieldInfo]) -> TokenStream2 {
	let mut checks = Vec::new();

	for info in field_infos {
		// Skip primary key fields from validation
		if info.attrs.primary_key {
			continue;
		}

		let field_ident = &info.ident;
		let field_name = info.ident.to_string();

		// Required validation
		if info.attrs.required {
			if info.is_option {
				checks.push(quote! {
					if self.#field_ident.is_none() {
						return Err(reinhardt_db::nosql::error::OdmError::Validation(
							reinhardt_db::nosql::error::ValidationError::Required(#field_name)
						));
					}
				});
			} else {
				let ty = &info.ty;
				let type_str = quote!(#ty).to_string();
				if type_str.contains("String") {
					checks.push(quote! {
						if self.#field_ident.is_empty() {
							return Err(reinhardt_db::nosql::error::OdmError::Validation(
								reinhardt_db::nosql::error::ValidationError::Required(#field_name)
							));
						}
					});
				}
			}
		}

		// Min/Max validation
		if info.attrs.min.is_some() || info.attrs.max.is_some() {
			let min_val: i64 = info.attrs.min.as_ref().map(lit_to_i64).unwrap_or(i64::MIN);
			let max_val: i64 = info.attrs.max.as_ref().map(lit_to_i64).unwrap_or(i64::MAX);

			checks.push(quote! {
				if (self.#field_ident as i64) < #min_val || (self.#field_ident as i64) > #max_val {
					return Err(reinhardt_db::nosql::error::OdmError::Validation(
						reinhardt_db::nosql::error::ValidationError::OutOfRange {
							field: #field_name,
							min: #min_val,
							max: #max_val,
						}
					));
				}
			});
		}

		// Built-in validate patterns
		if let Some(ref validate) = info.attrs.validate {
			match validate.as_str() {
				"email" => {
					checks.push(quote! {
						if !self.#field_ident.contains('@') {
							return Err(reinhardt_db::nosql::error::OdmError::Validation(
								reinhardt_db::nosql::error::ValidationError::InvalidEmail
							));
						}
					});
				}
				"url" => {
					checks.push(quote! {
						if !self.#field_ident.starts_with("http://") && !self.#field_ident.starts_with("https://") {
							return Err(reinhardt_db::nosql::error::OdmError::Validation(
								reinhardt_db::nosql::error::ValidationError::InvalidUrl
							));
						}
					});
				}
				custom_fn => {
					// Custom validation function
					let fn_path: syn::Path = match syn::parse_str(custom_fn) {
						Ok(path) => path,
						Err(_) => {
							let msg = format!("invalid validation function path: '{}'", custom_fn);
							return quote! {
								fn validate(&self) -> reinhardt_db::nosql::error::OdmResult<()> {
									compile_error!(#msg);
								}
							};
						}
					};
					checks.push(quote! {
						if let Err(e) = #fn_path(&self.#field_ident) {
							return Err(reinhardt_db::nosql::error::OdmError::Validation(
								reinhardt_db::nosql::error::ValidationError::Custom(e.to_string())
							));
						}
					});
				}
			}
		}
	}

	if checks.is_empty() {
		return quote! {};
	}

	quote! {
		fn validate(&self) -> reinhardt_db::nosql::error::OdmResult<()> {
			#(#checks)*
			Ok(())
		}
	}
}

/// Convert a `syn::Lit` to `i64`.
///
/// Panics with a descriptive message for non-numeric literals,
/// which produces a compile-time error in proc macro context.
fn lit_to_i64(lit: &Lit) -> i64 {
	match lit {
		Lit::Int(i) => i
			.base10_parse::<i64>()
			.expect("min/max attribute: failed to parse integer literal"),
		Lit::Float(f) => f
			.base10_parse::<f64>()
			.expect("min/max attribute: failed to parse float literal") as i64,
		_ => panic!("min/max attributes require numeric literals (integer or float)"),
	}
}

/// Map a Rust type string to a BSON type string for validation schema.
fn rust_type_to_bson_type(ty: &Type) -> Option<&'static str> {
	let type_str = quote!(#ty).to_string();
	// Strip Option wrapper
	let inner = if type_str.contains("Option") {
		// Get the inner type text roughly
		type_str
			.replace("Option", "")
			.replace(['<', '>'], "")
			.trim()
			.to_string()
	} else {
		type_str
	};
	let inner = inner.trim();

	if inner.contains("String") || inner == "& str" {
		Some("string")
	} else if inner.contains("i32") {
		Some("int")
	} else if inner.contains("i64") {
		Some("long")
	} else if inner.contains("f32") || inner.contains("f64") {
		Some("double")
	} else if inner.contains("bool") {
		Some("bool")
	} else if inner.contains("ObjectId") {
		Some("objectId")
	} else {
		None
	}
}

/// Generate `validation_schema()` method implementation.
fn gen_validation_schema(field_infos: &[FieldInfo]) -> TokenStream2 {
	let mut property_inserts = Vec::new();
	let mut required_inserts = Vec::new();

	for info in field_infos {
		if info.attrs.primary_key {
			continue;
		}

		let bson_name = bson_field_name(info);
		let bson_type = match rust_type_to_bson_type(&info.ty) {
			Some(t) => t,
			None => continue,
		};

		let mut prop_fields = vec![quote! {
			prop.insert("bsonType", #bson_type);
		}];

		// Add min/max constraints
		if let Some(ref min_lit) = info.attrs.min {
			let min_val = lit_to_i64(min_lit);
			prop_fields.push(quote! {
				prop.insert("minimum", #min_val);
			});
		}
		if let Some(ref max_lit) = info.attrs.max {
			let max_val = lit_to_i64(max_lit);
			prop_fields.push(quote! {
				prop.insert("maximum", #max_val);
			});
		}

		property_inserts.push(quote! {
			{
				let mut prop = bson::Document::new();
				#(#prop_fields)*
				properties.insert(#bson_name, prop);
			}
		});

		if info.attrs.required {
			required_inserts.push(quote! {
				required_fields.push(#bson_name);
			});
		}
	}

	if property_inserts.is_empty() {
		return quote! {};
	}

	quote! {
		fn validation_schema() -> Option<bson::Document> {
			let mut properties = bson::Document::new();
			let mut required_fields: Vec<&str> = Vec::new();

			#(#property_inserts)*
			#(#required_inserts)*

			if properties.is_empty() {
				return None;
			}

			let mut schema = bson::doc! {
				"bsonType": "object",
				"properties": properties,
			};
			if !required_fields.is_empty() {
				let req_arr: Vec<bson::Bson> = required_fields
					.into_iter()
					.map(|s| bson::Bson::String(s.to_string()))
					.collect();
				schema.insert("required", req_arr);
			}
			Some(schema)
		}
	}
}

/// Generate `references()` method implementation.
fn gen_references(field_infos: &[FieldInfo]) -> TokenStream2 {
	let mut ref_entries = Vec::new();

	for info in field_infos {
		if let Some(ref collection) = info.attrs.references {
			let field_name = info.ident.to_string();
			ref_entries.push(quote! {
				(#field_name, #collection)
			});
		}
	}

	if ref_entries.is_empty() {
		return quote! {};
	}

	quote! {
		fn references() -> Vec<(&'static str, &'static str)> {
			vec![
				#(#ref_entries),*
			]
		}
	}
}
