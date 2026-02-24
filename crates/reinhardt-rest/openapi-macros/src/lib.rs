//! Procedural macros for OpenAPI schema generation in Reinhardt.
//!
//! This crate provides derive macros and attribute macros for automatic
//! OpenAPI schema generation from Rust types.
//!

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

mod crate_paths;
mod schema;
mod serde_attrs;

use crate::crate_paths::get_reinhardt_openapi_crate;
use schema::{FieldAttributes, extract_field_attributes};
use serde_attrs::{
	TaggingStrategy, extract_serde_enum_attrs, extract_serde_rename_all,
	extract_serde_variant_attrs,
};

/// Derive macro for automatic OpenAPI schema generation.
///
/// This macro implements the `ToSchema` trait for your struct or enum,
/// generating an OpenAPI schema based on the type's fields/variants and attributes.
///
/// # Attributes
///
/// ## Container Attributes
///
/// - `#[schema(title = "...")]` - Override the schema title (default: type name)
/// - `#[schema(description = "...")]` - Schema description
/// - `#[schema(example = "...")]` - Example value for the entire type
///
/// ## Field Attributes (for structs)
///
/// - `#[schema(description = "...")]` - Field description (also reads doc comments)
/// - `#[schema(example = "...")]` - Example value for this field
/// - `#[schema(default)]` - Mark field as having a default value
/// - `#[schema(deprecated)]` - Mark field as deprecated
/// - `#[schema(read_only)]` - Field is read-only (GET responses only)
/// - `#[schema(write_only)]` - Field is write-only (POST/PUT requests only)
/// - `#[schema(format = "...")]` - OpenAPI format (e.g., "email", "uri", "date-time")
/// - `#[schema(minimum = N)]` - Minimum value for numbers
/// - `#[schema(maximum = N)]` - Maximum value for numbers
/// - `#[schema(min_length = N)]` - Minimum length for strings/arrays
/// - `#[schema(max_length = N)]` - Maximum length for strings/arrays
/// - `#[schema(pattern = "...")]` - Regex pattern for string validation
///
/// # Enum Support
///
/// The macro supports serde's enum tagging strategies:
///
/// - **External** (default): `{"VariantName": {...}}`
/// - **Internal**: `#[serde(tag = "type")]` -> `{"type": "VariantName", ...}`
/// - **Adjacent**: `#[serde(tag = "t", content = "c")]` -> `{"t": "VariantName", "c": {...}}`
/// - **Untagged**: `#[serde(untagged)]` -> `{...}` (no discriminator)
///
/// ## Variant Types
///
/// - **Unit variants**: Become string enum values
/// - **Newtype variants**: Use the inner type's schema
/// - **Tuple variants**: Generate array schema
/// - **Struct variants**: Generate object schema with properties
///
#[proc_macro_derive(Schema, attributes(schema))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	match &input.data {
		Data::Struct(data) => derive_struct_schema(&input, data),
		Data::Enum(data) => derive_enum_schema(&input, data),
		Data::Union(_) => syn::Error::new_spanned(&input, "Schema cannot be derived for unions")
			.to_compile_error()
			.into(),
	}
}

/// Generate schema for struct types
fn derive_struct_schema(input: &DeriveInput, data: &syn::DataStruct) -> TokenStream {
	let name = &input.ident;
	let generics = &input.generics;
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	let fields = match &data.fields {
		Fields::Named(fields) => &fields.named,
		_ => {
			return syn::Error::new_spanned(
				input,
				"Schema can only be derived for structs with named fields",
			)
			.to_compile_error()
			.into();
		}
	};

	// Extract container-level attributes
	let struct_name = name.to_string();

	// Extract container-level rename_all for field name transformation
	// Fixes #835
	let rename_all = extract_serde_rename_all(&input.attrs);

	// Generate schema for each field
	let mut field_schemas = Vec::new();
	let mut required_fields = Vec::new();
	// Fixes #839: Track flattened fields for allOf generation
	let mut flatten_schemas = Vec::new();

	for field in fields {
		let field_name = field.ident.as_ref().unwrap();
		let field_name_str = field_name.to_string();
		let field_type = &field.ty;

		// Extract field attributes
		let attrs = match extract_field_attributes(&field.attrs) {
			Ok(attrs) => attrs,
			Err(err) => return err.to_compile_error().into(),
		};

		// Fixes #837: Validate mutual exclusion of read_only and write_only
		if attrs.read_only && attrs.write_only {
			return syn::Error::new_spanned(
				field,
				"A field cannot be both read_only and write_only",
			)
			.to_compile_error()
			.into();
		}

		// Fixes #836: Skip fields with serde skip attributes
		if attrs.skip || attrs.skip_serializing || attrs.skip_deserializing {
			continue;
		}

		// Fixes #839: Handle flattened fields separately
		if attrs.flatten {
			let schema_builder = build_field_schema(field_type, &attrs);
			flatten_schemas.push(schema_builder);
			continue;
		}

		// Use renamed property name if available (from #[serde(rename)] or #[schema(rename)])
		// Fixes #835: Apply rename_all if no explicit rename is set
		let property_name = attrs
			.rename
			.clone()
			.unwrap_or_else(|| apply_rename_all(&field_name_str, rename_all.as_deref()));

		// Check if field is Option<T> (makes it optional)
		// Fixes #838: Also consider default attribute for required fields
		let is_option = is_option_type(field_type);
		if !is_option && !attrs.default {
			required_fields.push(property_name.clone());
		}

		// Build field schema with attributes
		let schema_builder = build_field_schema(field_type, &attrs);

		field_schemas.push(quote! {
			builder = builder.property(#property_name, #schema_builder);
		});
	}

	// Add required fields
	let required_builder = if !required_fields.is_empty() {
		quote! {
			#(builder = builder.required(#required_fields);)*
		}
	} else {
		quote! {}
	};

	// Get dynamic crate path
	let openapi_crate = get_reinhardt_openapi_crate();

	// Fixes #839: Generate allOf if there are flattened fields
	let schema_body = if !flatten_schemas.is_empty() {
		quote! {
			use #openapi_crate::Schema;
			use #openapi_crate::utoipa::openapi::schema::{AllOfBuilder, ObjectBuilder, SchemaType, Type};

			// Build the main object schema with regular properties
			let mut builder = ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Object));

			#(#field_schemas)*
			#required_builder

			let main_schema = Schema::Object(builder.build());

			// Combine with flattened schemas using allOf
			let mut all_of_builder = AllOfBuilder::new();
			all_of_builder = all_of_builder.item(#openapi_crate::RefOr::T(main_schema));
			#(all_of_builder = all_of_builder.item(#openapi_crate::RefOr::T(#flatten_schemas));)*

			Schema::AllOf(all_of_builder.build())
		}
	} else {
		quote! {
			use #openapi_crate::Schema;
			use #openapi_crate::utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};

			let mut builder = ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Object));

			#(#field_schemas)*
			#required_builder

			Schema::Object(builder.build())
		}
	};

	// Generate inventory registration only for non-generic types
	// Generic types cannot be registered at compile time since they don't have a concrete type
	let inventory_registration = if generics.params.is_empty() {
		quote! {
			// Automatic schema registration via inventory
			// This allows the framework to discover all schemas at compile time
			::inventory::submit! {
				#openapi_crate::SchemaRegistration::new(
					#struct_name,
					#name::schema
				)
			}
		}
	} else {
		quote! {}
	};

	let expanded = quote! {
		impl #impl_generics #openapi_crate::ToSchema for #name #ty_generics #where_clause {
			fn schema() -> #openapi_crate::Schema {
				#schema_body
			}

			fn schema_name() -> Option<String> {
				Some(#struct_name.to_string())
			}
		}

		#inventory_registration
	};

	TokenStream::from(expanded)
}

/// Generate schema for enum types
fn derive_enum_schema(input: &DeriveInput, data: &syn::DataEnum) -> TokenStream {
	let name = &input.ident;
	let generics = &input.generics;
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
	let enum_name = name.to_string();

	// Extract serde enum attributes for tagging strategy
	let serde_attrs = extract_serde_enum_attrs(&input.attrs);
	let tagging = serde_attrs.tagging_strategy();

	// Get dynamic crate path
	let openapi_crate = get_reinhardt_openapi_crate();

	// Check if all variants are unit variants (simple string enum)
	let all_unit_variants = data
		.variants
		.iter()
		.all(|v| matches!(v.fields, Fields::Unit));

	let schema_body = if all_unit_variants && matches!(tagging, TaggingStrategy::External) {
		// Simple string enum: generate string schema with enum values
		generate_simple_enum_schema(data, &openapi_crate, &serde_attrs)
	} else {
		// Complex enum: use EnumSchemaBuilder
		generate_complex_enum_schema(data, &openapi_crate, &enum_name, &tagging)
	};

	// Generate inventory registration only for non-generic types
	let inventory_registration = if generics.params.is_empty() {
		quote! {
			::inventory::submit! {
				#openapi_crate::SchemaRegistration::new(
					#enum_name,
					#name::schema
				)
			}
		}
	} else {
		quote! {}
	};

	let expanded = quote! {
		impl #impl_generics #openapi_crate::ToSchema for #name #ty_generics #where_clause {
			fn schema() -> #openapi_crate::Schema {
				#schema_body
			}

			fn schema_name() -> Option<String> {
				Some(#enum_name.to_string())
			}
		}

		#inventory_registration
	};

	TokenStream::from(expanded)
}

/// Generate schema for simple unit-variant enums (string enum)
fn generate_simple_enum_schema(
	data: &syn::DataEnum,
	openapi_crate: &proc_macro2::TokenStream,
	serde_attrs: &serde_attrs::SerdeEnumAttrs,
) -> proc_macro2::TokenStream {
	let variant_names: Vec<String> = data
		.variants
		.iter()
		.filter_map(|v| {
			let variant_attrs = extract_serde_variant_attrs(&v.attrs);
			if variant_attrs.skip {
				return None;
			}
			// Apply rename if present, considering rename_all strategy
			let name = variant_attrs.rename.unwrap_or_else(|| {
				apply_rename_all(&v.ident.to_string(), serde_attrs.rename_all.as_deref())
			});
			Some(name)
		})
		.collect();

	quote! {
		use #openapi_crate::Schema;
		use #openapi_crate::utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};

		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::String))
				.enum_values(Some(vec![#(serde_json::Value::String(#variant_names.to_string())),*]))
				.build()
		)
	}
}

/// Generate schema for complex enums using EnumSchemaBuilder
fn generate_complex_enum_schema(
	data: &syn::DataEnum,
	openapi_crate: &proc_macro2::TokenStream,
	enum_name: &str,
	tagging: &TaggingStrategy,
) -> proc_macro2::TokenStream {
	// Generate tagging strategy expression
	let tagging_expr = match tagging {
		TaggingStrategy::External => quote! {
			#openapi_crate::EnumTagging::External
		},
		TaggingStrategy::Internal { tag } => quote! {
			#openapi_crate::EnumTagging::Internal { tag: #tag.to_string() }
		},
		TaggingStrategy::Adjacent { tag, content } => quote! {
			#openapi_crate::EnumTagging::Adjacent {
				tag: #tag.to_string(),
				content: #content.to_string(),
			}
		},
		TaggingStrategy::Untagged => quote! {
			#openapi_crate::EnumTagging::Untagged
		},
	};

	// Generate variant schemas
	let variant_additions: Vec<proc_macro2::TokenStream> = data
		.variants
		.iter()
		.filter_map(|variant| {
			let variant_attrs = extract_serde_variant_attrs(&variant.attrs);
			if variant_attrs.skip {
				return None;
			}

			let variant_name = variant_attrs
				.rename
				.clone()
				.unwrap_or_else(|| variant.ident.to_string());

			let variant_schema = generate_variant_schema(&variant.fields, openapi_crate);

			Some(quote! {
				builder = builder.variant(#variant_name, #variant_schema);
			})
		})
		.collect();

	quote! {
		use #openapi_crate::{EnumSchemaBuilder, Schema, SchemaExt};

		let mut builder = EnumSchemaBuilder::new(#enum_name)
			.tagging(#tagging_expr);

		#(#variant_additions)*

		builder.build()
	}
}

/// Generate schema for a single variant's fields
fn generate_variant_schema(
	fields: &Fields,
	openapi_crate: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
	match fields {
		Fields::Unit => {
			// Unit variant: empty object or null
			quote! {
				#openapi_crate::Schema::object()
			}
		}
		Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
			// Newtype variant: use inner type's schema
			let inner_type = &fields.unnamed.first().unwrap().ty;
			quote! {
				<#inner_type as #openapi_crate::ToSchema>::schema()
			}
		}
		Fields::Unnamed(fields) => {
			// Tuple variant: array of inner types
			let type_schemas: Vec<proc_macro2::TokenStream> = fields
				.unnamed
				.iter()
				.map(|f| {
					let ty = &f.ty;
					quote! {
						#openapi_crate::RefOr::T(<#ty as #openapi_crate::ToSchema>::schema())
					}
				})
				.collect();

			quote! {
				{
					use #openapi_crate::utoipa::openapi::schema::{ArrayBuilder, SchemaType, Type};
					#openapi_crate::Schema::Array(
						ArrayBuilder::new()
							.schema_type(SchemaType::Type(Type::Array))
							.prefix_items(vec![#(#type_schemas),*])
							.build()
					)
				}
			}
		}
		Fields::Named(fields) => {
			// Struct variant: object with properties
			let mut property_additions = Vec::new();
			let mut required_additions = Vec::new();

			for field in &fields.named {
				let field_name = field.ident.as_ref().unwrap();
				let field_name_str = field_name.to_string();
				let field_type = &field.ty;

				// Check for serde rename on field
				let field_attrs = extract_field_attributes(&field.attrs).unwrap_or_default();
				let property_name = field_attrs.rename.unwrap_or(field_name_str);

				// Check if required
				let is_option = is_option_type(field_type);
				if !is_option {
					required_additions.push(quote! {
						builder = builder.required(#property_name);
					});
				}

				property_additions.push(quote! {
					builder = builder.property(
						#property_name,
						<#field_type as #openapi_crate::ToSchema>::schema()
					);
				});
			}

			quote! {
				{
					use #openapi_crate::utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};
					let mut builder = ObjectBuilder::new()
						.schema_type(SchemaType::Type(Type::Object));
					#(#property_additions)*
					#(#required_additions)*
					#openapi_crate::Schema::Object(builder.build())
				}
			}
		}
	}
}

/// Apply rename_all transformation to a variant name
fn apply_rename_all(name: &str, rename_all: Option<&str>) -> String {
	match rename_all {
		Some("lowercase") => name.to_lowercase(),
		Some("UPPERCASE") => name.to_uppercase(),
		Some("camelCase") => to_camel_case(name),
		Some("snake_case") => to_snake_case(name),
		Some("SCREAMING_SNAKE_CASE") => to_snake_case(name).to_uppercase(),
		Some("kebab-case") => to_snake_case(name).replace('_', "-"),
		Some("SCREAMING-KEBAB-CASE") => to_snake_case(name).to_uppercase().replace('_', "-"),
		Some("PascalCase") | None => name.to_string(),
		Some(_) => name.to_string(),
	}
}

/// Convert PascalCase to camelCase
fn to_camel_case(s: &str) -> String {
	let mut result = String::new();
	for (i, c) in s.chars().enumerate() {
		if i == 0 {
			result.extend(c.to_lowercase());
		} else {
			result.push(c);
		}
	}
	result
}

/// Convert PascalCase to snake_case
/// Fixes #833: Handle consecutive uppercase correctly (e.g., "XMLParser" -> "xmlparser")
///
/// This follows serde's behavior where consecutive uppercase letters are treated
/// as a single word (e.g., "XMLParser" -> "xmlparser", not "x_m_l_parser").
fn to_snake_case(s: &str) -> String {
	let mut result = String::new();
	let chars: Vec<char> = s.chars().collect();

	for (i, c) in chars.iter().enumerate() {
		if c.is_uppercase() {
			// Only insert underscore before uppercase if:
			// 1. Not the first character AND
			// 2. Previous character is lowercase OR
			// 3. (Not the last character AND next character is lowercase)
			// This handles: "HttpRequest" -> "http_request"
			// But not: "XMLParser" -> "x_m_l_parser" (becomes "xmlparser")
			if i > 0 {
				let prev_is_lowercase = chars[i - 1].is_lowercase();
				let next_is_lowercase = i + 1 < chars.len() && chars[i + 1].is_lowercase();

				if prev_is_lowercase || next_is_lowercase {
					result.push('_');
				}
			}
			result.extend(c.to_lowercase());
		} else {
			result.push(*c);
		}
	}
	result
}

/// Helper function to check if a type is `Option<T>`
fn is_option_type(ty: &syn::Type) -> bool {
	if let syn::Type::Path(type_path) = ty
		&& let Some(segment) = type_path.path.segments.last()
	{
		return segment.ident == "Option";
	}
	false
}

/// Build schema for a field type with attributes
fn build_field_schema(field_type: &syn::Type, attrs: &FieldAttributes) -> proc_macro2::TokenStream {
	let openapi_crate = get_reinhardt_openapi_crate();
	let base_schema = quote! {
		<#field_type as #openapi_crate::ToSchema>::schema()
	};

	// If no attributes, return base schema
	if attrs.is_empty() {
		return base_schema;
	}

	// Build schema with attributes applied
	let mut modifications = Vec::new();

	if let Some(ref description) = attrs.description {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.description = Some(#description.to_string());
				schema = Schema::Object(obj);
			}
		});
	}

	if let Some(ref example) = attrs.example {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.example = Some(serde_json::json!(#example));
				schema = Schema::Object(obj);
			}
		});
	}

	if let Some(ref format) = attrs.format {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.format = Some(#openapi_crate::utoipa::openapi::schema::SchemaFormat::Custom(#format.to_string()));
				schema = Schema::Object(obj);
			}
		});
	}

	if attrs.read_only {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.read_only = Some(true);
				schema = Schema::Object(obj);
			}
		});
	}

	if attrs.write_only {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.write_only = Some(true);
				schema = Schema::Object(obj);
			}
		});
	}

	if attrs.deprecated {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.deprecated = Some(#openapi_crate::utoipa::openapi::Deprecated::True);
				schema = Schema::Object(obj);
			}
		});
	}

	if let Some(min) = attrs.minimum {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.minimum = Some(#openapi_crate::utoipa::Number::from(#min as f64));
				schema = Schema::Object(obj);
			}
		});
	}

	if let Some(max) = attrs.maximum {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.maximum = Some(#openapi_crate::utoipa::Number::from(#max as f64));
				schema = Schema::Object(obj);
			}
		});
	}

	if let Some(min_len) = attrs.min_length {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.min_length = Some(#min_len);
				schema = Schema::Object(obj);
			}
		});
	}

	if let Some(max_len) = attrs.max_length {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.max_length = Some(#max_len);
				schema = Schema::Object(obj);
			}
		});
	}

	if let Some(ref pattern) = attrs.pattern {
		modifications.push(quote! {
			if let Schema::Object(mut obj) = schema {
				obj.pattern = Some(#pattern.to_string());
				schema = Schema::Object(obj);
			}
		});
	}

	if modifications.is_empty() {
		base_schema
	} else {
		quote! {
			{
				let mut schema = #base_schema;
				#(#modifications)*
				schema
			}
		}
	}
}
