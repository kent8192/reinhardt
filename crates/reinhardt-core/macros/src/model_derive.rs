//! Model derive macro for automatic ORM model registration
//!
//! Provides automatic `Model` trait implementation and registration to the global ModelRegistry.
//!
//! # Automatic Relationship Discovery
//!
//! The `#[model(...)]` attribute macro automatically detects relationship fields and registers
//! them in the global `RELATIONSHIPS` registry for reverse relation construction:
//!
//! - **`ForeignKeyField<T>`** → Registered as `RelationshipType::ForeignKey`
//! - **`OneToOneField<T>`** → Registered as `RelationshipType::OneToOne`
//! - **`ManyToManyField<T, U>`** → Registered as `RelationshipType::ManyToMany`
//!
//! # Type-Safe ManyToMany Accessor Methods
//!
//! The `#[model(...)]` macro automatically generates type-safe accessor methods for each `ManyToManyField`.
//!
//! **Benefits:**
//! - Compile-time field name validation (no typos)
//! - Type inference for Source and Target models
//! - IDE auto-completion support
//! - Cleaner, more idiomatic API
//!
//! The macro generates linkme distributed_slice registrations for each relationship,
//! enabling `build_reverse_relations()` to construct reverse accessors at runtime.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Result, Type, parse_quote};
use syn::{Ident, LitBool, LitStr, bracketed, parenthesized};

use crate::crate_paths::{
	get_linkme_crate, get_reinhardt_core_crate, get_reinhardt_crate,
	get_reinhardt_migrations_crate, get_reinhardt_orm_crate,
};
use crate::rel::RelAttribute;

/// Constraint specification from `#[model(constraints = [...])]`
#[derive(Debug, Clone)]
enum ConstraintSpec {
	/// unique(fields = [...], name = "...", condition = "...")
	Unique {
		fields: Vec<String>,
		name: Option<String>,
		condition: Option<String>,
	},
}

/// Parsed model attributes (intermediate representation)
struct ModelAttributesParsed {
	app_label: Option<String>,
	table_name: Option<String>,
	constraints: Option<Vec<ConstraintSpec>>,
	unique_together: Vec<Vec<String>>, // Multiple Django-style unique_together constraints
	/// Optional custom manager path: `manager = MyManager` (Issue #3980).
	manager: Option<syn::Path>,
	/// Whether to generate Info companion struct (Issue #4194).
	/// `None` means not specified (defaults to `true` in `ModelConfig`).
	info: Option<bool>,
	/// Whether this model is only available on the native/server side.
	server_only: bool,
	/// Whether the original model has `#[derive(serde::Serialize)]`.
	/// Passed from the attribute macro since derive macros cannot see `#[derive()]`.
	serde_serialize: bool,
	/// Whether the original model has `#[derive(serde::Deserialize)]`.
	serde_deserialize: bool,
}

/// Validate a raw SQL expression to reject dangerous patterns.
///
/// This is a basic compile-time check that rejects obviously dangerous SQL
/// keywords and patterns that should never appear in `check`, `generated`,
/// or `condition` constraint attributes. It does not replace parameterized
/// queries, but prevents accidental or malicious injection of DDL/DML
/// statements in model attribute strings.
fn validate_sql_expression(sql: &str, attr_name: &str) -> Result<()> {
	let upper = sql.to_uppercase();

	// Reject statement terminators that could allow statement chaining
	if sql.contains(';') {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			format!(
				"Semicolons are not allowed in {} expressions: {:?}",
				attr_name, sql
			),
		));
	}

	// Reject DDL/DML keywords that should never appear in check/generated/condition
	const BLOCKED_KEYWORDS: &[&str] = &[
		"DROP ",
		"DELETE ",
		"INSERT ",
		"UPDATE ",
		"ALTER ",
		"TRUNCATE ",
		"EXEC ",
		"EXECUTE ",
		"CREATE ",
		"GRANT ",
		"REVOKE ",
	];
	for keyword in BLOCKED_KEYWORDS {
		if upper.contains(keyword) {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				format!(
					"Dangerous SQL keyword {:?} detected in {} expression: {:?}",
					keyword.trim(),
					attr_name,
					sql
				),
			));
		}
	}

	// Reject comment sequences that could hide injected SQL
	if sql.contains("--") || sql.contains("/*") {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			format!(
				"SQL comments are not allowed in {} expressions: {:?}",
				attr_name, sql
			),
		));
	}

	Ok(())
}

/// Model configuration from `#[model(...)]` attribute
#[derive(Debug, Clone)]
struct ModelConfig {
	app_label: String,
	table_name: String,
	constraints: Vec<ConstraintSpec>,
	/// Custom manager type path from `manager = MyManager` (Issue #3980, #3984).
	///
	/// When `Some`, the macro sets `type Objects = MyManager` in the generated
	/// `Model` impl so that `objects()` returns the custom manager directly.
	manager: Option<syn::Path>,
	/// Whether to generate an `{Model}Info` companion struct (Issue #4194).
	/// Defaults to `true`. Set `#[model(info = false)]` to opt out.
	info: bool,
	/// Whether this model should skip shared data/info output.
	server_only: bool,
	/// Whether the original model derives `serde::Serialize`.
	serde_serialize: bool,
	/// Whether the original model derives `serde::Deserialize`.
	serde_deserialize: bool,
}

impl ModelConfig {
	/// Parse `#[model(...)]` attribute
	fn from_attrs(attrs: &[syn::Attribute], struct_name: &syn::Ident) -> Result<Self> {
		let mut app_label = None;
		let mut table_name = None;
		let mut constraints = Vec::new();
		let mut manager: Option<syn::Path> = None;
		let mut info: Option<bool> = None;
		let mut server_only = false;
		let mut serde_serialize = false;
		let mut serde_deserialize = false;

		for attr in attrs {
			// Accept both #[model(...)] and #[model_config(...)] helper attributes
			if !attr.path().is_ident("model") && !attr.path().is_ident("model_config") {
				continue;
			}

			// Use custom parser for all model attributes
			let model_attr = attr
				.parse_args_with(|input: syn::parse::ParseStream| {
					Self::parse_model_attributes(input)
				})
				.map_err(|e| {
					syn::Error::new_spanned(attr, format!("parse_args_with failed: {}", e))
				})?;

			if let Some(c) = model_attr.constraints {
				constraints = c;
			}
			// Convert each unique_together to ConstraintSpec::Unique
			for fields in model_attr.unique_together {
				constraints.push(ConstraintSpec::Unique {
					fields,
					name: None, // Auto-generate name
					condition: None,
				});
			}
			if let Some(al) = model_attr.app_label {
				app_label = Some(al);
			}
			if let Some(tn) = model_attr.table_name {
				table_name = Some(tn);
			}
			if let Some(m) = model_attr.manager {
				if manager.is_some() {
					return Err(syn::Error::new_spanned(
						struct_name,
						"#[model(manager = ...)] specified more than once",
					));
				}
				manager = Some(m);
			}
			if let Some(i) = model_attr.info {
				info = Some(i);
			}
			if model_attr.server_only {
				server_only = true;
			}
			if model_attr.serde_serialize {
				serde_serialize = true;
			}
			if model_attr.serde_deserialize {
				serde_deserialize = true;
			}
		}

		let table_name = table_name.ok_or_else(|| {
			syn::Error::new_spanned(
				struct_name,
				"table_name attribute is required in #[model(...)]",
			)
		})?;

		Ok(Self {
			app_label: app_label.unwrap_or_else(|| "default".to_string()),
			table_name,
			constraints,
			manager,
			info: info.unwrap_or(true),
			server_only,
			serde_serialize,
			serde_deserialize,
		})
	}

	/// Parse all model attributes using custom parser
	fn parse_model_attributes(input: syn::parse::ParseStream) -> Result<ModelAttributesParsed> {
		use syn::Token;

		let mut app_label = None;
		let mut table_name = None;
		let mut constraints = None;
		let mut unique_together = Vec::new();
		let mut manager: Option<syn::Path> = None;
		let mut info: Option<bool> = None;
		let mut server_only = false;
		let mut serde_serialize = false;
		let mut serde_deserialize = false;

		while !input.is_empty() {
			let ident: Ident = input.parse()?;

			// Bare flags (no `= value`)
			if ident == "serde_serialize" {
				serde_serialize = true;
				if input.peek(Token![,]) {
					input.parse::<Token![,]>()?;
				} else {
					break;
				}
				continue;
			} else if ident == "serde_deserialize" {
				serde_deserialize = true;
				if input.peek(Token![,]) {
					input.parse::<Token![,]>()?;
				} else {
					break;
				}
				continue;
			} else if ident == "server_only" {
				server_only = true;
				if input.peek(Token![,]) {
					input.parse::<Token![,]>()?;
				} else {
					break;
				}
				continue;
			}

			input.parse::<Token![=]>()?;

			if ident == "app_label" {
				let value: LitStr = input.parse()?;
				app_label = Some(value.value());
			} else if ident == "table_name" {
				let value: LitStr = input.parse()?;
				table_name = Some(value.value());
			} else if ident == "manager" {
				// Custom object manager type: `manager = MyManager` (Issue #3980).
				let path: syn::Path = input.parse()?;
				manager = Some(path);
			} else if ident == "info" {
				let value: LitBool = input.parse()?;
				info = Some(value.value());
			} else if ident == "unique_together" {
				// Tuple syntax: unique_together = ("field1", "field2")
				use syn::punctuated::Punctuated;
				let content;
				parenthesized!(content in input);
				let fields: Punctuated<LitStr, Token![,]> =
					content.call(Punctuated::parse_terminated)?;
				unique_together.push(fields.iter().map(|lit| lit.value()).collect());
			} else if ident == "constraints" {
				// Parse array: [unique(...), ...]
				let array_content;
				bracketed!(array_content in input);

				let mut specs = Vec::new();
				while !array_content.is_empty() {
					specs.push(Self::parse_constraint(&array_content)?);

					if array_content.peek(Token![,]) {
						array_content.parse::<Token![,]>()?;
					} else {
						break;
					}
				}
				constraints = Some(specs);
			} else {
				return Err(syn::Error::new_spanned(
					&ident,
					format!("Unknown model attribute: {}", ident),
				));
			}

			// Parse optional comma
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			} else {
				break;
			}
		}

		Ok(ModelAttributesParsed {
			app_label,
			table_name,
			constraints,
			unique_together,
			manager,
			info,
			server_only,
			serde_serialize,
			serde_deserialize,
		})
	}

	/// Parse constraint specification: unique(fields = [...], name = "...", condition = "...")
	fn parse_constraint(input: syn::parse::ParseStream) -> Result<ConstraintSpec> {
		use syn::Token;
		use syn::punctuated::Punctuated;

		// Define custom keyword for "unique"
		mod kw {
			syn::custom_keyword!(unique);
		}

		// Parse constraint type using custom keyword
		let _unique_keyword = input.parse::<kw::unique>()?;

		// Parse parentheses with parameters
		let content;
		parenthesized!(content in input);

		let mut fields = None;
		let mut name = None;
		let mut condition = None;

		// Parse named parameters (fields = [...], name = "...", condition = "...")
		loop {
			if content.is_empty() {
				break;
			}

			let param_name: Ident = content.parse()?;
			content.parse::<Token![=]>()?;

			if param_name == "fields" {
				// Parse array using Punctuated for proper comma handling
				let array_content;
				bracketed!(array_content in content);

				// Use Punctuated::parse_terminated for robust comma-separated parsing
				let field_literals: Punctuated<LitStr, Token![,]> =
					array_content.call(Punctuated::parse_terminated)?;

				fields = Some(field_literals.iter().map(|lit| lit.value()).collect());
			} else if param_name == "name" {
				// Parse string: "constraint_name"
				let value: LitStr = content.parse()?;
				name = Some(value.value());
			} else if param_name == "condition" {
				// Parse string: "WHERE clause"
				let value: LitStr = content.parse()?;
				let condition_str = value.value();
				validate_sql_expression(&condition_str, "condition")?;
				condition = Some(condition_str);
			} else {
				return Err(syn::Error::new_spanned(
					param_name,
					"Unknown parameter. Supported: fields, name, condition",
				));
			}

			// Parse optional comma between parameters
			if content.peek(Token![,]) {
				content.parse::<Token![,]>()?;
			} else {
				break;
			}
		}

		// fields is required
		let fields = fields.ok_or_else(|| {
			syn::Error::new(
				proc_macro2::Span::call_site(),
				"unique constraint requires 'fields' parameter",
			)
		})?;

		Ok(ConstraintSpec::Unique {
			fields,
			name,
			condition,
		})
	}
}

/// Foreign key specification
#[derive(Debug, Clone)]
enum ForeignKeySpec {
	/// Type directly: `#[field(foreign_key = User)]`
	Type(syn::Type),
	/// app_label.model_name format: `#[field(foreign_key = "users.User")]`
	AppModel {
		app_label: String,
		model_name: String,
	},
}

/// Storage strategy for PostgreSQL columns
#[cfg(feature = "db-postgres")]
#[derive(Debug, Clone)]
enum StorageStrategy {
	Plain,
	Extended,
	External,
	Main,
}

/// Compression method for PostgreSQL columns
#[cfg(feature = "db-postgres")]
#[derive(Debug, Clone)]
enum CompressionMethod {
	Pglz,
	Lz4,
}

/// Field configuration from `#[field(...)]` attribute
#[derive(Debug, Clone, Default)]
struct FieldConfig {
	primary_key: bool,
	max_length: Option<u64>,
	null: Option<bool>,
	blank: Option<bool>,
	unique: Option<bool>,
	default: Option<syn::Expr>, // Changed from String to Expr to support bool, int, etc.
	db_column: Option<String>,
	editable: Option<bool>,
	index: Option<bool>,
	check: Option<String>,
	// Validator flags
	email: Option<bool>,
	url: Option<bool>,
	min_length: Option<u64>,
	min_value: Option<i64>,
	max_value: Option<i64>,
	// Time-related fields
	auto_now_add: Option<bool>,
	auto_now: Option<bool>,
	// Relationship fields
	foreign_key: Option<ForeignKeySpec>,

	// Generated Columns (all DBMS)
	generated: Option<String>,
	generated_stored: Option<bool>,
	#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]
	generated_virtual: Option<bool>,

	// Identity/Auto-increment
	#[cfg(feature = "db-postgres")]
	identity_always: Option<bool>,
	#[cfg(feature = "db-postgres")]
	identity_by_default: Option<bool>,
	/// Auto-increment for integer primary keys.
	/// Available for all databases. When set to true on an integer primary key,
	/// the field is excluded from required builder inputs and uses 0 as default value.
	/// Integer primary keys are treated as auto_increment by default unless
	/// explicitly set to false.
	auto_increment: Option<bool>,
	#[cfg(feature = "db-sqlite")]
	autoincrement: Option<bool>,

	// Character Set & Collation
	collate: Option<String>,
	#[cfg(feature = "db-mysql")]
	character_set: Option<String>,

	// Comment
	#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
	comment: Option<String>,

	// Storage Optimization (PostgreSQL)
	#[cfg(feature = "db-postgres")]
	storage: Option<StorageStrategy>,
	#[cfg(feature = "db-postgres")]
	compression: Option<CompressionMethod>,

	// ON UPDATE Trigger (MySQL)
	#[cfg(feature = "db-mysql")]
	on_update_current_timestamp: Option<bool>,

	// Invisible Columns (MySQL)
	#[cfg(feature = "db-mysql")]
	invisible: Option<bool>,

	// Full-Text Index (PostgreSQL, MySQL)
	#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
	fulltext: Option<bool>,

	// Numeric Attributes (MySQL, deprecated)
	#[cfg(feature = "db-mysql")]
	unsigned: Option<bool>,
	#[cfg(feature = "db-mysql")]
	zerofill: Option<bool>,

	// Getter/setter generation control
	/// Skip getter and setter generation for this field.
	/// Used by `#[user]` macro to avoid conflicts with trait method signatures.
	skip_getter: bool,

	/// Completely skip this field from model processing.
	/// Excluded from type validation, getter/setter, constructor, metadata, and registration.
	/// Initialized with `Default::default()` in constructor. Implies `skip_getter = true`.
	/// Used by `#[user]` macro for non-DB cache fields (e.g., `Vec<String>` permissions).
	skip: bool,

	/// Exclude this field from the generated Info companion struct (Issue #4194).
	/// The field remains in the model but does not appear in `{Model}Info`.
	/// In `From<{Model}Info> for {Model}`, the excluded field uses `Default::default()`.
	skip_info: bool,

	// Constructor input generation control
	/// Whether to include this field in required builder inputs.
	/// When true, field is included even if it would normally be auto-generated
	/// When false, field is excluded and uses default value
	include_in_new: Option<bool>,

	// PostgreSQL-specific type attributes
	/// Explicit field type specification (e.g., "jsonb", "hstore", "citext")
	/// Takes priority over automatic type inference
	#[cfg(feature = "db-postgres")]
	field_type: Option<String>,
	/// Base type for array elements (e.g., "VARCHAR(50)", "INTEGER")
	/// Used when the Rust type is `Vec<T>` but the element type cannot be inferred
	#[cfg(feature = "db-postgres")]
	array_base_type: Option<String>,
}

impl FieldConfig {
	/// Parse `#[field(...)]` attribute
	fn from_attrs(attrs: &[syn::Attribute]) -> Result<Self> {
		let mut config = Self::default();

		for attr in attrs {
			if !attr.path().is_ident("field") {
				continue;
			}

			// Support empty #[field] attribute
			if matches!(attr.meta, syn::Meta::Path(_)) {
				continue;
			}

			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("primary_key") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.primary_key = value.value;
					Ok(())
				} else if meta.path.is_ident("max_length") {
					let value: syn::LitInt = meta.value()?.parse()?;
					config.max_length = Some(value.base10_parse()?);
					Ok(())
				} else if meta.path.is_ident("null") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.null = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("blank") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.blank = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("unique") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.unique = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("default") {
					// Parse as Expr to support bool, int, string, etc.
					let value: syn::Expr = meta.value()?.parse()?;
					config.default = Some(value);
					Ok(())
				} else if meta.path.is_ident("db_column") {
					let value: syn::LitStr = meta.value()?.parse()?;
					config.db_column = Some(value.value());
					Ok(())
				} else if meta.path.is_ident("editable") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.editable = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("index") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.index = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("check") {
					let value: syn::LitStr = meta.value()?.parse()?;
					let check_str = value.value();
					validate_sql_expression(&check_str, "check")?;
					config.check = Some(check_str);
					Ok(())
				} else if meta.path.is_ident("email") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.email = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("url") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.url = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("min_length") {
					let value: syn::LitInt = meta.value()?.parse()?;
					config.min_length = Some(value.base10_parse()?);
					Ok(())
				} else if meta.path.is_ident("min_value") {
					let value: syn::LitInt = meta.value()?.parse()?;
					config.min_value = Some(value.base10_parse()?);
					Ok(())
				} else if meta.path.is_ident("max_value") {
					let value: syn::LitInt = meta.value()?.parse()?;
					config.max_value = Some(value.base10_parse()?);
					Ok(())
				} else if meta.path.is_ident("auto_now_add") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.auto_now_add = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("auto_now") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.auto_now = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("foreign_key") {
					// Try parsing as Type first (direct type specification)
					if let Ok(ty) = meta.value()?.parse::<syn::Type>() {
						config.foreign_key = Some(ForeignKeySpec::Type(ty));
						return Ok(());
					}

					// Fall back to string specification
					if let Ok(value) = meta.value()?.parse::<syn::LitStr>() {
						let spec_str = value.value();

						if spec_str.contains('.') {
							// app_label.model_name format
							let parts: Vec<&str> = spec_str.split('.').collect();
							if parts.len() == 2 {
								config.foreign_key = Some(ForeignKeySpec::AppModel {
									app_label: parts[0].to_string(),
									model_name: parts[1].to_string(),
								});
								return Ok(());
							} else {
								return Err(meta.error(
									"foreign_key must be in 'app_label.model_name' format",
								));
							}
						} else {
							// Type name only (for backward compatibility)
							if let Ok(ty) = syn::parse_str::<syn::Type>(&spec_str) {
								config.foreign_key = Some(ForeignKeySpec::Type(ty));
								return Ok(());
							} else {
								return Err(meta.error("Invalid foreign_key specification"));
							}
						}
					}

					Err(meta.error("foreign_key must be a type (User) or string (\"users.User\")"))
				}
				// Generated Columns
				else if meta.path.is_ident("generated") {
					let value: syn::LitStr = meta.value()?.parse()?;
					let gen_str = value.value();
					validate_sql_expression(&gen_str, "generated")?;
					config.generated = Some(gen_str);
					Ok(())
				} else if meta.path.is_ident("generated_stored") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.generated_stored = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("generated_virtual") {
					#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.generated_virtual = Some(value.value);
						Ok(())
					}
					#[cfg(not(any(feature = "db-mysql", feature = "db-sqlite")))]
					{
						Err(meta.error(
							"generated_virtual is only available with db-mysql or db-sqlite features",
						))
					}
				}
				// Identity/Auto-increment
				else if meta.path.is_ident("identity_always") {
					#[cfg(feature = "db-postgres")]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.identity_always = Some(value.value);
						Ok(())
					}
					#[cfg(not(feature = "db-postgres"))]
					{
						Err(meta
							.error("identity_always is only available with db-postgres feature"))
					}
				} else if meta.path.is_ident("identity_by_default") {
					#[cfg(feature = "db-postgres")]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.identity_by_default = Some(value.value);
						Ok(())
					}
					#[cfg(not(feature = "db-postgres"))]
					{
						Err(meta.error(
							"identity_by_default is only available with db-postgres feature",
						))
					}
				} else if meta.path.is_ident("auto_increment") {
					// auto_increment is available for all databases
					// Integer primary keys are treated as auto_increment by default
					let value: syn::LitBool = meta.value()?.parse()?;
					config.auto_increment = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("autoincrement") {
					#[cfg(feature = "db-sqlite")]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.autoincrement = Some(value.value);
						Ok(())
					}
					#[cfg(not(feature = "db-sqlite"))]
					{
						Err(meta.error("autoincrement is only available with db-sqlite feature"))
					}
				}
				// Character Set & Collation
				else if meta.path.is_ident("collate") {
					let value: syn::LitStr = meta.value()?.parse()?;
					config.collate = Some(value.value());
					Ok(())
				} else if meta.path.is_ident("character_set") {
					#[cfg(feature = "db-mysql")]
					{
						let value: syn::LitStr = meta.value()?.parse()?;
						config.character_set = Some(value.value());
						Ok(())
					}
					#[cfg(not(feature = "db-mysql"))]
					{
						Err(meta.error("character_set is only available with db-mysql feature"))
					}
				}
				// Comment
				else if meta.path.is_ident("comment") {
					#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
					{
						let value: syn::LitStr = meta.value()?.parse()?;
						config.comment = Some(value.value());
						Ok(())
					}
					#[cfg(not(any(feature = "db-postgres", feature = "db-mysql")))]
					{
						Err(meta.error(
							"comment is only available with db-postgres or db-mysql features",
						))
					}
				}
				// Storage Optimization
				else if meta.path.is_ident("storage") {
					#[cfg(feature = "db-postgres")]
					{
						let value: syn::LitStr = meta.value()?.parse()?;
						let storage_str = value.value();
						let storage = match storage_str.to_lowercase().as_str() {
							"plain" => StorageStrategy::Plain,
							"extended" => StorageStrategy::Extended,
							"external" => StorageStrategy::External,
							"main" => StorageStrategy::Main,
							_ => {
								return Err(meta.error(
									"storage must be one of: plain, extended, external, main",
								));
							}
						};
						config.storage = Some(storage);
						Ok(())
					}
					#[cfg(not(feature = "db-postgres"))]
					{
						Err(meta.error("storage is only available with db-postgres feature"))
					}
				} else if meta.path.is_ident("compression") {
					#[cfg(feature = "db-postgres")]
					{
						let value: syn::LitStr = meta.value()?.parse()?;
						let compression_str = value.value();
						let compression = match compression_str.to_lowercase().as_str() {
							"pglz" => CompressionMethod::Pglz,
							"lz4" => CompressionMethod::Lz4,
							_ => return Err(meta.error("compression must be one of: pglz, lz4")),
						};
						config.compression = Some(compression);
						Ok(())
					}
					#[cfg(not(feature = "db-postgres"))]
					{
						Err(meta.error("compression is only available with db-postgres feature"))
					}
				}
				// ON UPDATE Trigger
				else if meta.path.is_ident("on_update_current_timestamp") {
					#[cfg(feature = "db-mysql")]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.on_update_current_timestamp = Some(value.value);
						Ok(())
					}
					#[cfg(not(feature = "db-mysql"))]
					{
						Err(meta.error(
							"on_update_current_timestamp is only available with db-mysql feature",
						))
					}
				}
				// Invisible Columns
				else if meta.path.is_ident("invisible") {
					#[cfg(feature = "db-mysql")]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.invisible = Some(value.value);
						Ok(())
					}
					#[cfg(not(feature = "db-mysql"))]
					{
						Err(meta.error("invisible is only available with db-mysql feature"))
					}
				}
				// Full-Text Index
				else if meta.path.is_ident("fulltext") {
					#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.fulltext = Some(value.value);
						Ok(())
					}
					#[cfg(not(any(feature = "db-postgres", feature = "db-mysql")))]
					{
						Err(meta.error(
							"fulltext is only available with db-postgres or db-mysql features",
						))
					}
				}
				// Numeric Attributes (MySQL, deprecated)
				else if meta.path.is_ident("unsigned") {
					#[cfg(feature = "db-mysql")]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.unsigned = Some(value.value);
						Ok(())
					}
					#[cfg(not(feature = "db-mysql"))]
					{
						Err(meta.error("unsigned is only available with db-mysql feature"))
					}
				} else if meta.path.is_ident("zerofill") {
					#[cfg(feature = "db-mysql")]
					{
						let value: syn::LitBool = meta.value()?.parse()?;
						config.zerofill = Some(value.value);
						Ok(())
					}
					#[cfg(not(feature = "db-mysql"))]
					{
						Err(meta.error("zerofill is only available with db-mysql feature"))
					}
				}
				// Constructor generation control
				else if meta.path.is_ident("include_in_new") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.include_in_new = Some(value.value);
					Ok(())
				}
				// PostgreSQL-specific type attributes
				else if meta.path.is_ident("field_type") {
					#[cfg(feature = "db-postgres")]
					{
						let value: syn::LitStr = meta.value()?.parse()?;
						config.field_type = Some(value.value());
						Ok(())
					}
					#[cfg(not(feature = "db-postgres"))]
					{
						Err(meta.error("field_type is only available with db-postgres feature"))
					}
				} else if meta.path.is_ident("array_base_type") {
					#[cfg(feature = "db-postgres")]
					{
						let value: syn::LitStr = meta.value()?.parse()?;
						config.array_base_type = Some(value.value());
						Ok(())
					}
					#[cfg(not(feature = "db-postgres"))]
					{
						Err(meta
							.error("array_base_type is only available with db-postgres feature"))
					}
				} else if meta.path.is_ident("skip_getter") {
					config.skip_getter = meta.value()?.parse::<syn::LitBool>()?.value();
					Ok(())
				} else if meta.path.is_ident("skip") {
					config.skip = meta.value()?.parse::<syn::LitBool>()?.value();
					Ok(())
				} else if meta.path.is_ident("skip_info") {
					config.skip_info = meta.value()?.parse::<syn::LitBool>()?.value();
					Ok(())
				} else {
					Err(meta.error("unsupported field attribute"))
				}
			})?;
		}

		// skip implies skip_getter
		if config.skip {
			config.skip_getter = true;
		}

		Ok(config)
	}

	/// Validate field configuration for mutual exclusivity and logical consistency
	fn validate(&self) -> Result<()> {
		// Check mutual exclusivity of auto-increment attributes
		#[allow(unused_mut)]
		let mut auto_increment_count = 0;

		#[cfg(feature = "db-postgres")]
		{
			if self.identity_always.is_some() {
				auto_increment_count += 1;
			}
			if self.identity_by_default.is_some() {
				auto_increment_count += 1;
			}
		}

		#[cfg(feature = "db-mysql")]
		{
			if self.auto_increment.is_some() {
				auto_increment_count += 1;
			}
		}

		#[cfg(feature = "db-sqlite")]
		{
			if self.autoincrement.is_some() {
				auto_increment_count += 1;
			}
		}

		if auto_increment_count > 1 {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"Only one auto-increment attribute (identity_always, identity_by_default, auto_increment, autoincrement) can be specified per field",
			));
		}

		// Generated columns cannot have default values
		if self.generated.is_some() && self.default.is_some() {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"Generated columns cannot have default values",
			));
		}

		// Generated columns should have either generated_stored or generated_virtual
		if self.generated.is_some() {
			let has_stored = self.generated_stored.unwrap_or(false);

			#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]
			let has_virtual = self.generated_virtual.unwrap_or(false);
			#[cfg(not(any(feature = "db-mysql", feature = "db-sqlite")))]
			let has_virtual = false;

			if !has_stored && !has_virtual {
				return Err(syn::Error::new(
					proc_macro2::Span::call_site(),
					"Generated columns must specify either generated_stored=true or generated_virtual=true",
				));
			}

			if has_stored && has_virtual {
				return Err(syn::Error::new(
					proc_macro2::Span::call_site(),
					"Generated columns cannot be both STORED and VIRTUAL",
				));
			}
		}

		Ok(())
	}
}

/// Field information for processing
#[derive(Debug, Clone)]
struct FieldInfo {
	name: syn::Ident,
	ty: Type,
	config: FieldConfig,
	/// Field-level `#[serde(...)]` attributes copied to generated companion fields.
	serde_attrs: Vec<syn::Attribute>,
	/// Whether `#[model]` injected the relation-model `#[serde(skip)]` attribute.
	injected_relation_serde_skip: bool,
	/// Optional relationship attribute from `#[rel(...)]`
	///
	/// This field is reserved for future accessor generation support.
	/// Currently, relationship fields (ForeignKeyField, ManyToManyField) are processed
	/// at runtime through their types, but this field will enable compile-time accessor
	/// generation for relationship traversal methods.
	///
	/// Planned usage:
	/// - Generate type-safe accessor methods (e.g., user.get_profile(), user.get_posts())
	/// - Enable eager loading optimization hints
	/// - Support relationship-specific query methods
	///
	/// Implementation requires architectural decisions on:
	/// - Accessor naming conventions
	/// - Async/sync accessor variants
	/// - Relationship traversal API design
	#[allow(dead_code)]
	rel: Option<RelAttribute>,
	/// Whether this is an auto-generated FK _id field (marked with `#[fk_id_field]`)
	/// These fields should have getters but not setters
	is_fk_id_field: bool,
}

/// Foreign key / One-to-one field information for automatic ID field generation
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used for accessor generation in future
struct ForeignKeyFieldInfo {
	/// Original field name (e.g., "author")
	field_name: syn::Ident,
	/// Target model type (e.g., User)
	target_type: Type,
	/// Generated ID column name (e.g., "author_id" or custom via db_column)
	id_column_name: String,
	/// Related name for reverse accessor
	related_name: Option<String>,
	/// Whether this is a OneToOne field (requires UNIQUE constraint)
	is_one_to_one: bool,
	/// The full RelAttribute for additional options
	rel_attr: RelAttribute,
}

/// Generate field metadata string from Rust type
fn field_type_to_metadata_string(ty: &Type, _config: &FieldConfig) -> Result<String> {
	let (_is_option, inner_ty) = extract_option_type(ty);

	match inner_ty {
		Type::Path(type_path) => {
			let last_segment = type_path
				.path
				.segments
				.last()
				.ok_or_else(|| syn::Error::new_spanned(ty, "Invalid type path"))?;

			let type_name = match last_segment.ident.to_string().as_str() {
				"i32" => "IntegerField",
				"i64" => "BigIntegerField",
				"String" => "CharField",
				"bool" => "BooleanField",
				"f32" | "f64" => "FloatField",
				"DateTime" => "DateTimeField",
				"Date" => "DateField",
				"Time" => "TimeField",
				"Decimal" => "DecimalField",
				"Uuid" => "UuidField",
				// Extended types (SQL generation is gated per-DB in map_type_to_field_type)
				"Vec" => "ArrayField",
				"Value" => "JsonField",
				"HashMap" => "HStoreField",
				other => {
					return Err(syn::Error::new_spanned(
						ty,
						format!("Unsupported field type: {}", other),
					));
				}
			};

			Ok(format!("reinhardt.orm.models.{}", type_name))
		}
		_ => Err(syn::Error::new_spanned(ty, "Unsupported field type")),
	}
}

/// Serialize a `#[field(default = ...)]` expression into the dialect-neutral
/// SQL fragment stored in `FieldState.params["default"]`.
///
/// The autodetector reads this string verbatim into the generated migration's
/// `ColumnDefinition.default`, and the runner interpolates it as
/// `DEFAULT <fragment>` inside the generated DDL. The serialization therefore
/// has to:
///
/// * Produce SQL the three supported dialects (Postgres, MySQL, SQLite) all
///   accept. For booleans we emit lowercase `true` / `false`; Postgres and
///   MySQL accept these as literals and SQLite (≥ 3.23) treats them as
///   integer 1 / 0.
/// * Quote string literals so that `default = "active"` lands as `'active'`.
///   We use SQL single-quote escaping (double the inner quote) rather than
///   Rust escaping.
/// * Stay opt-in for anything we cannot prove is safe — unrecognised forms
///   (function calls, paths, complex expressions) return `None` so that the
///   macro keeps today's behaviour of silently omitting the default rather
///   than emitting something that would break parsing downstream. The runner
///   surfaces a clearer "missing default" failure when this matters; see
///   reinhardt-web#4447.
fn serialize_field_default(expr: &syn::Expr) -> Option<String> {
	// Allow a leading unary `-` so `default = -1` works.
	if let syn::Expr::Unary(unary) = expr
		&& matches!(unary.op, syn::UnOp::Neg(_))
		&& let Some(inner) = serialize_field_default(&unary.expr)
	{
		return Some(format!("-{}", inner));
	}

	let lit = match expr {
		syn::Expr::Lit(l) => &l.lit,
		_ => return None,
	};
	match lit {
		syn::Lit::Bool(b) => Some(if b.value {
			"true".into()
		} else {
			"false".into()
		}),
		syn::Lit::Int(i) => Some(i.base10_digits().to_string()),
		syn::Lit::Float(f) => Some(f.base10_digits().to_string()),
		syn::Lit::Str(s) => Some(format!("'{}'", s.value().replace('\'', "''"))),
		_ => None,
	}
}

/// Map Rust type to ORM field type
fn map_type_to_field_type(ty: &Type, config: &FieldConfig) -> Result<TokenStream> {
	let migrations_crate = get_reinhardt_migrations_crate();

	// PostgreSQL: Check for explicit field_type attribute first
	#[cfg(feature = "db-postgres")]
	if let Some(explicit_type) = &config.field_type {
		return map_explicit_field_type(explicit_type, &migrations_crate);
	}

	// Extract the inner type if it's Option<T>
	let (_is_option, inner_ty) = extract_option_type(ty);

	let field_type = match inner_ty {
		Type::Path(type_path) => {
			let last_segment = type_path
				.path
				.segments
				.last()
				.ok_or_else(|| syn::Error::new_spanned(ty, "Invalid type path"))?;

			match last_segment.ident.to_string().as_str() {
				"i32" => {
					quote! { #migrations_crate::FieldType::Integer }
				}
				"i64" => {
					quote! { #migrations_crate::FieldType::BigInteger }
				}
				"String" => {
					let max_length = config.max_length.ok_or_else(|| {
						syn::Error::new_spanned(ty, "String fields require max_length attribute")
					})? as u32;
					quote! { #migrations_crate::FieldType::VarChar(#max_length) }
				}
				"bool" => {
					quote! { #migrations_crate::FieldType::Boolean }
				}
				"DateTime" => {
					quote! { #migrations_crate::FieldType::TimestampTz }
				}
				"Date" => {
					quote! { #migrations_crate::FieldType::Date }
				}
				"Time" => {
					quote! { #migrations_crate::FieldType::Time }
				}
				"f32" => {
					quote! { #migrations_crate::FieldType::Float }
				}
				"f64" => {
					quote! { #migrations_crate::FieldType::Double }
				}
				"Uuid" => {
					quote! { #migrations_crate::FieldType::Uuid }
				}
				// PostgreSQL: Vec<T> -> Array type
				#[cfg(feature = "db-postgres")]
				"Vec" => {
					return map_vec_to_array_type(ty, last_segment, config, &migrations_crate);
				}
				// PostgreSQL: serde_json::Value -> JSONB
				#[cfg(feature = "db-postgres")]
				"Value" => {
					// Assume serde_json::Value for JSONB
					quote! { #migrations_crate::FieldType::Jsonb }
				}
				// PostgreSQL: HashMap<String, String> -> HStore
				#[cfg(feature = "db-postgres")]
				"HashMap" => {
					quote! { #migrations_crate::FieldType::HStore }
				}
				_ => {
					return Err(syn::Error::new_spanned(
						ty,
						format!("Unsupported field type: {}", last_segment.ident),
					));
				}
			}
		}
		_ => {
			return Err(syn::Error::new_spanned(ty, "Unsupported field type"));
		}
	};

	Ok(field_type)
}

/// Map explicit PostgreSQL field type string to FieldType
#[cfg(feature = "db-postgres")]
fn map_explicit_field_type(
	field_type_str: &str,
	migrations_crate: &proc_macro2::TokenStream,
) -> Result<TokenStream> {
	let field_type = match field_type_str.to_lowercase().as_str() {
		"jsonb" => quote! { #migrations_crate::FieldType::Jsonb },
		"json" => quote! { #migrations_crate::FieldType::Json },
		"hstore" => quote! { #migrations_crate::FieldType::HStore },
		"citext" => quote! { #migrations_crate::FieldType::CIText },
		"int4range" | "integer_range" => quote! { #migrations_crate::FieldType::Int4Range },
		"int8range" | "bigint_range" => quote! { #migrations_crate::FieldType::Int8Range },
		"numrange" | "decimal_range" => quote! { #migrations_crate::FieldType::NumRange },
		"daterange" | "date_range" => quote! { #migrations_crate::FieldType::DateRange },
		"tsrange" | "timestamp_range" => quote! { #migrations_crate::FieldType::TsRange },
		"tstzrange" | "timestamptz_range" => quote! { #migrations_crate::FieldType::TsTzRange },
		"tsvector" => quote! { #migrations_crate::FieldType::TsVector },
		"tsquery" => quote! { #migrations_crate::FieldType::TsQuery },
		"uuid" => quote! { #migrations_crate::FieldType::Uuid },
		"text" => quote! { #migrations_crate::FieldType::Text },
		other => {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				format!(
					"Unknown PostgreSQL field type: '{}'. Supported types: jsonb, json, hstore, \
					 citext, int4range, int8range, numrange, daterange, tsrange, tstzrange, \
					 tsvector, tsquery, uuid, text",
					other
				),
			));
		}
	};
	Ok(field_type)
}

/// Map `Vec<T>` to PostgreSQL Array type
#[cfg(feature = "db-postgres")]
fn map_vec_to_array_type(
	ty: &Type,
	segment: &syn::PathSegment,
	config: &FieldConfig,
	migrations_crate: &proc_macro2::TokenStream,
) -> Result<TokenStream> {
	// First check if array_base_type is explicitly specified
	if let Some(base_type) = &config.array_base_type {
		// Parse the base type string to FieldType
		let inner_field_type = parse_base_type_string(base_type, migrations_crate)?;
		return Ok(quote! {
			#migrations_crate::FieldType::Array(Box::new(#inner_field_type))
		});
	}

	// Try to infer the element type from Vec<T>
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(Type::Path(inner_path))) = args.args.first()
		&& let Some(inner_segment) = inner_path.path.segments.last()
	{
		let inner_type_name = inner_segment.ident.to_string();
		let inner_field_type = match inner_type_name.as_str() {
			"String" => {
				// For String arrays, check if max_length is provided
				if let Some(max_length) = config.max_length {
					let ml = max_length as u32;
					quote! { #migrations_crate::FieldType::VarChar(#ml) }
				} else {
					// Default to TEXT for string arrays without max_length
					quote! { #migrations_crate::FieldType::Text }
				}
			}
			"i32" => quote! { #migrations_crate::FieldType::Integer },
			"i64" => quote! { #migrations_crate::FieldType::BigInteger },
			"f32" => quote! { #migrations_crate::FieldType::Float },
			"f64" => quote! { #migrations_crate::FieldType::Double },
			"bool" => quote! { #migrations_crate::FieldType::Boolean },
			"Uuid" => quote! { #migrations_crate::FieldType::Uuid },
			_ => {
				return Err(syn::Error::new_spanned(
					ty,
					format!(
						"Cannot infer array element type for Vec<{}>. \
						 Use #[field(array_base_type = \"...\")] to specify explicitly.",
						inner_type_name
					),
				));
			}
		};

		return Ok(quote! {
			#migrations_crate::FieldType::Array(Box::new(#inner_field_type))
		});
	}

	Err(syn::Error::new_spanned(
		ty,
		"Cannot infer Vec element type. Use #[field(array_base_type = \"...\")] to specify explicitly.",
	))
}

/// Parse a base type string (e.g., "VARCHAR(50)", "INTEGER") to FieldType tokens
#[cfg(feature = "db-postgres")]
fn parse_base_type_string(
	base_type: &str,
	migrations_crate: &proc_macro2::TokenStream,
) -> Result<TokenStream> {
	let upper = base_type.to_uppercase();

	// Check for VARCHAR(n) pattern
	if upper.starts_with("VARCHAR(") && upper.ends_with(')') {
		let len_str = &upper[8..upper.len() - 1];
		if let Ok(length) = len_str.parse::<u32>() {
			return Ok(quote! { #migrations_crate::FieldType::VarChar(#length) });
		}
	}

	// Check for CHAR(n) pattern
	if upper.starts_with("CHAR(") && upper.ends_with(')') {
		let len_str = &upper[5..upper.len() - 1];
		if let Ok(length) = len_str.parse::<u32>() {
			return Ok(quote! { #migrations_crate::FieldType::Char(#length) });
		}
	}

	// Simple type mapping
	let field_type = match upper.as_str() {
		"INTEGER" | "INT" | "INT4" => quote! { #migrations_crate::FieldType::Integer },
		"BIGINT" | "INT8" => quote! { #migrations_crate::FieldType::BigInteger },
		"SMALLINT" | "INT2" => quote! { #migrations_crate::FieldType::SmallInteger },
		"TEXT" => quote! { #migrations_crate::FieldType::Text },
		"BOOLEAN" | "BOOL" => quote! { #migrations_crate::FieldType::Boolean },
		"REAL" | "FLOAT4" => quote! { #migrations_crate::FieldType::Float },
		"DOUBLE PRECISION" | "FLOAT8" => quote! { #migrations_crate::FieldType::Double },
		"UUID" => quote! { #migrations_crate::FieldType::Uuid },
		"DATE" => quote! { #migrations_crate::FieldType::Date },
		"TIME" => quote! { #migrations_crate::FieldType::Time },
		"TIMESTAMP" => quote! { #migrations_crate::FieldType::DateTime },
		"JSONB" => quote! { #migrations_crate::FieldType::Jsonb },
		"JSON" => quote! { #migrations_crate::FieldType::Json },
		_ => {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				format!(
					"Unknown base type for array: '{}'. Use standard SQL types like \
					 INTEGER, BIGINT, VARCHAR(n), TEXT, BOOLEAN, etc.",
					base_type
				),
			));
		}
	};

	Ok(field_type)
}

/// Extract `Option<T>` and return (is_option, inner_type)
fn extract_option_type(ty: &Type) -> (bool, &Type) {
	if let Type::Path(type_path) = ty
		&& let Some(last_segment) = type_path.path.segments.last()
		&& last_segment.ident == "Option"
		&& let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
	{
		return (true, inner_ty);
	}
	(false, ty)
}

/// Generate field accessor methods that return FieldRef<M, T>
///
/// Generates const methods like:
/// ```rust,ignore
/// use reinhardt_db::orm::expressions::FieldRef;
/// use reinhardt_macros::model;
///
/// #[model(app_label = "users", table_name = "users")]
/// struct User {
///     #[field(primary_key = true)]
///     id: i64,
///     name: String,
/// }
///
/// // The #[model] attribute macro automatically generates:
/// impl User {
///     pub const fn field_id() -> FieldRef<User, i64> { FieldRef::new("id") }
///     pub const fn field_name() -> FieldRef<User, String> { FieldRef::new("name") }
/// }
/// ```
fn generate_field_accessors(struct_name: &syn::Ident, field_infos: &[FieldInfo]) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();

	let accessor_methods: Vec<_> = field_infos
		.iter()
		.filter(|field| !field.config.skip)
		.map(|field| {
			let field_name = &field.name;
			let field_type = &field.ty;
			let method_name = syn::Ident::new(&format!("field_{}", field_name), field_name.span());
			let field_name_str = field_name.to_string();

			quote! {
				/// Field accessor for type-safe field references
				///
				/// Returns a `FieldRef<#struct_name, #field_type>` that provides compile-time
				/// type safety for field operations.
				pub const fn #method_name() -> #orm_crate::expressions::FieldRef<#struct_name, #field_type> {
					#orm_crate::expressions::FieldRef::new(#field_name_str)
				}
			}
		})
		.collect();

	quote! {
		impl #struct_name {
			#(#accessor_methods)*
		}
	}
}

/// Generate accessor methods for ManyToMany relationships.
///
/// The generated accessor method internally calls `ManyToManyAccessor::new()`
/// with the field name, providing compile-time field name validation and
/// improved IDE support.
///
///
/// # Generated Code Characteristics
///
/// - **Method naming**: `{field_name}_accessor()`
/// - **Visibility**: `pub` (same as model)
/// - **Type parameters**: Inferred from `ManyToManyField<Source, Target>`
/// - **Documentation**: Auto-generated with field name
fn generate_m2m_accessor_methods(
	struct_name: &syn::Ident,
	field_infos: &[FieldInfo],
) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();

	let accessor_methods: Vec<_> = field_infos
		.iter()
		// Filter only ManyToManyField types
		.filter(|field| is_many_to_many_field_type(&field.ty))
		.filter_map(|field| {
			let field_name = &field.name;
			let field_name_str = field_name.to_string();

			// Method name: {field_name}_accessor
			let method_name = syn::Ident::new(
				&format!("{}_accessor", field_name),
				field_name.span()
			);

			// Extract Target from ManyToManyField<Source, Target>
			let target_ty = extract_m2m_target_type(&field.ty)?;

			let doc_comment = format!(
				"Create a ManyToManyAccessor for the '{}' relationship",
				field_name_str
			);

			Some(quote! {
				#[doc = #doc_comment]
				pub fn #method_name(
					&self,
					db: #orm_crate::connection::DatabaseConnection
				) -> #orm_crate::ManyToManyAccessor<#struct_name, #target_ty> {
					#orm_crate::ManyToManyAccessor::new(
						self,
						#field_name_str,
						db
					)
				}
			})
		})
		.collect();

	if accessor_methods.is_empty() {
		quote! {}
	} else {
		quote! {
			impl #struct_name {
				#(#accessor_methods)*
			}
		}
	}
}

/// Generate accessor methods for ForeignKey and OneToOne relationships.
///
/// The generated accessor method loads the related instance from the database
/// using the FK _id field value.
///
/// # Generated Code Characteristics
///
/// - **Method naming**: `{field_name}()`
/// - **Visibility**: `pub` (same as model)
/// - **Return type**: `Option<Target>`
/// - **Documentation**: Auto-generated with field name
fn generate_fk_accessor_methods(
	struct_name: &syn::Ident,
	field_infos: &[FieldInfo],
) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();
	let core_crate = get_reinhardt_core_crate();

	let accessor_methods: Vec<_> = field_infos
		.iter()
		// Filter only ForeignKeyField and OneToOneField
		.filter(|field| {
			is_foreign_key_field_type(&field.ty) || is_one_to_one_field_type(&field.ty)
		})
		.map(|field| {
			let field_name = &field.name;
			let field_name_str = field_name.to_string();

			// FK _id field name (e.g., user → user_id)
			let fk_id_field_name = syn::Ident::new(
				&format!("{}_id", field_name),
				field_name.span()
			);

			// Method name: {field_name}
			let method_name = field_name;

			// Extract Target from ForeignKeyField<Target> or OneToOneField<Target>
			let target_ty = extract_foreign_key_target_type(&field.ty);

			let doc_comment = format!(
				"Load the related '{}' instance from the database",
				field_name_str
			);

			quote! {
				#[doc = #doc_comment]
				pub async fn #method_name(
					&self,
					db: &#orm_crate::connection::DatabaseConnection
				) -> #core_crate::exception::Result<Option<#target_ty>> {
					use #orm_crate::Model;

					// Get FK _id value (getter returns &PrimaryKey)
					let fk_id = self.#fk_id_field_name();

					// Query the target model using the FK _id via the typed
					// `FieldRef::eq` builder (Issue #4650).
					#target_ty::objects()
						.filter(#target_ty::field_id().eq(fk_id.to_string()))
						.first_with_db(db)
						.await
				}
			}
		})
		.collect();

	if accessor_methods.is_empty() {
		quote! {}
	} else {
		quote! {
			impl #struct_name {
				#(#accessor_methods)*
			}
		}
	}
}

/// Generate static accessor methods for ForeignKey relationships.
///
/// The generated accessor method returns a `ForeignKeyAccessor` that can be used
/// to access reverse relationships in a type-safe manner.
///
/// # Generated Code Characteristics
///
/// - **Method naming**: `{field_name}_accessor()`
/// - **Visibility**: `pub` (same as model)
/// - **Return type**: `ForeignKeyAccessor<Self, Target>`
/// - **Static method**: No `&self` parameter required
///
/// # Generated Method
///
/// ```ignore
/// impl Tweet {
///     /// Get the ForeignKey accessor for the 'user' relationship
///     pub fn user_accessor() -> ForeignKeyAccessor<Tweet, User> {
///         ForeignKeyAccessor::new("user_id")
///     }
/// }
/// ```
///
/// # Usage
///
/// ```ignore
/// // Get reverse accessor for User → Tweets relationship
/// let tweets_accessor = Tweet::user_accessor().reverse(&user, db);
/// let tweets = tweets_accessor.all().await?;
/// ```
fn generate_fk_static_accessor_methods(
	struct_name: &syn::Ident,
	field_infos: &[FieldInfo],
) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();

	let accessor_methods: Vec<_> = field_infos
		.iter()
		// Filter only ForeignKeyField and OneToOneField
		.filter(|field| {
			is_foreign_key_field_type(&field.ty) || is_one_to_one_field_type(&field.ty)
		})
		.map(|field| {
			let field_name = &field.name;
			let field_name_str = field_name.to_string();

			// FK _id field name (e.g., user → user_id)
			let db_column = format!("{}_id", field_name_str);

			// Method name: {field_name}_accessor
			let method_name =
				syn::Ident::new(&format!("{}_accessor", field_name), field_name.span());

			// Extract Target from ForeignKeyField<Target> or OneToOneField<Target>
			let target_ty = extract_foreign_key_target_type(&field.ty);

			let doc_comment = format!(
				"Get the ForeignKey accessor for the '{}' relationship",
				field_name_str
			);

			quote! {
				#[doc = #doc_comment]
				pub fn #method_name() -> #orm_crate::ForeignKeyAccessor<#struct_name, #target_ty> {
					#orm_crate::ForeignKeyAccessor::new(#db_column)
				}
			}
		})
		.collect();

	if accessor_methods.is_empty() {
		quote! {}
	} else {
		quote! {
			impl #struct_name {
				#(#accessor_methods)*
			}
		}
	}
}

/// Make all fields module-local (non-pub) in the struct definition
fn make_fields_private(input: &mut DeriveInput) {
	if let Data::Struct(data) = &mut input.data
		&& let Fields::Named(fields) = &mut data.fields
	{
		for field in fields.named.iter_mut() {
			field.vis = syn::Visibility::Inherited;
		}
	}
}

/// Check if a type is Copy (returns value instead of reference)
fn is_copy_type(ty: &Type) -> bool {
	// Determine if type is primitive or Copy-derivable
	matches!(
		ty,
		Type::Path(path) if matches!(
			path.path.segments.last().map(|s| s.ident.to_string()).as_deref(),
			Some("i8" | "i16" | "i32" | "i64" | "i128" |
				 "u8" | "u16" | "u32" | "u64" | "u128" |
				 "f32" | "f64" | "bool" | "char" | "Uuid")
		)
	) || matches!(
		ty,
		Type::Path(path) if path.path.segments.iter().any(|seg|
			seg.ident == "DateTime"
		)
	)
}

/// Generate getter methods for all fields
fn generate_getter_methods(struct_name: &syn::Ident, field_infos: &[FieldInfo]) -> TokenStream {
	let getter_methods: Vec<_> = field_infos
		.iter()
		// Exclude ForeignKey, OneToOne, and skip_getter fields
		.filter(|field| {
			!is_foreign_key_field_type(&field.ty)
				&& !is_one_to_one_field_type(&field.ty)
				&& !field.config.skip_getter
		})
		.map(|field| {
			let field_name = &field.name;
			let field_type = &field.ty;
			let method_name = field_name;

			// Copy types return value, others return reference
			if is_copy_type(field_type) {
				quote! {
					#[doc = concat!("Get ", stringify!(#field_name))]
					pub fn #method_name(&self) -> #field_type {
						self.#field_name
					}
				}
			} else {
				quote! {
					#[doc = concat!("Get reference to ", stringify!(#field_name))]
					pub fn #method_name(&self) -> &#field_type {
						&self.#field_name
					}
				}
			}
		})
		.collect();

	quote! {
		impl #struct_name {
			#(#getter_methods)*
		}
	}
}

/// Generate setter methods for user-defined fields (excluding auto-generated)
fn generate_setter_methods(struct_name: &syn::Ident, field_infos: &[FieldInfo]) -> TokenStream {
	let setter_methods: Vec<_> = field_infos
		.iter()
		.filter(|f| !is_auto_generated_field(f) && !f.config.skip_getter)
		.map(|field| {
			let field_name = &field.name;
			let field_type = &field.ty;
			let setter_name = syn::Ident::new(&format!("set_{}", field_name), field_name.span());

			quote! {
				#[doc = concat!("Set ", stringify!(#field_name))]
				pub fn #setter_name(&mut self, value: #field_type) {
					self.#field_name = value;
				}
			}
		})
		.collect();

	quote! {
		impl #struct_name {
			#(#setter_methods)*
		}
	}
}

/// Implementation of the `Model` derive macro
pub(crate) fn model_derive_impl(mut input: DeriveInput) -> Result<TokenStream> {
	// Get the dynamically resolved crate paths
	let reinhardt = get_reinhardt_crate();
	let orm_crate = get_reinhardt_orm_crate();

	// Make all fields module-local (non-pub)
	make_fields_private(&mut input);

	let struct_name = &input.ident;

	let generics = &input.generics;
	let where_clause = &generics.where_clause;

	// Parse model configuration
	let model_config = ModelConfig::from_attrs(&input.attrs, struct_name)?;
	let app_label = &model_config.app_label;
	let table_name = &model_config.table_name;

	// Only support structs
	let fields = match &input.data {
		Data::Struct(data_struct) => match &data_struct.fields {
			Fields::Named(fields) => &fields.named,
			_ => {
				return Err(syn::Error::new_spanned(
					struct_name,
					"Model can only be derived for structs with named fields",
				));
			}
		},
		_ => {
			return Err(syn::Error::new_spanned(
				struct_name,
				"Model can only be derived for structs",
			));
		}
	};

	// Process all fields
	let mut field_infos = Vec::new();
	let mut rel_fields = Vec::new();
	// Collect auto-generated FK _id field names for builder setter generation.
	let mut fk_id_field_names: Vec<syn::Ident> = Vec::new();

	for field in fields {
		// Check if this is auto-generated FK _id field
		// These are generated by #[model] attribute macro
		// Identified by: field name ends with "_id" AND type matches a generated primary-key projection
		let is_fk_id_field = if let Some(field_name) = &field.ident {
			let name_str = field_name.to_string();
			let field_ty = &field.ty;
			let type_str = quote!(#field_ty).to_string();

			// Check if field name ends with "_id" and type contains a primary-key projection.
			// This pattern identifies auto-generated FK _id fields created by #[model(...)] macro
			name_str.ends_with("_id")
				&& (type_str.contains("InfoModel") || type_str.contains("Model"))
				&& type_str.contains("PrimaryKey")
		} else {
			false
		};

		if is_fk_id_field {
			// Collect the field name for builder setter generation.
			if let Some(field_name) = &field.ident {
				fk_id_field_names.push(field_name.clone());
			}
			// FK _id fields need getters but not setters, so add them to field_infos
			// with a flag to indicate they are auto-generated
		}

		let name = field
			.ident
			.clone()
			.ok_or_else(|| syn::Error::new_spanned(field, "Field must have a name"))?;
		let ty = field.ty.clone();
		let config = FieldConfig::from_attrs(&field.attrs)?;
		config.validate()?;
		let injected_relation_serde_skip = field.attrs.iter().any(|attr| {
			attr.path()
				.is_ident("reinhardt_internal_relation_serde_skip")
		});
		let serde_attrs: Vec<syn::Attribute> = field
			.attrs
			.iter()
			.filter(|attr| attr.path().is_ident("serde"))
			.cloned()
			.collect();

		// Parse #[rel(...)] attribute if present
		let rel = field
			.attrs
			.iter()
			.find(|attr| attr.path().is_ident("rel"))
			.map(RelAttribute::from_attribute)
			.transpose()?;

		// Collect relationship fields for later processing
		if let Some(ref rel_attr) = rel {
			rel_fields.push((name.clone(), rel_attr.clone()));
		}

		field_infos.push(FieldInfo {
			name,
			ty,
			config,
			serde_attrs,
			injected_relation_serde_skip,
			rel,
			is_fk_id_field,
		});
	}

	// Extract ForeignKeyField and OneToOneField information
	let mut fk_field_infos: Vec<ForeignKeyFieldInfo> = Vec::new();
	for field_info in &field_infos {
		if let Some(ref rel_attr) = field_info.rel {
			// Check if this is a ForeignKeyField or OneToOneField type
			if let Some(target_type) = extract_fk_target_type(&field_info.ty) {
				let is_one_to_one = is_one_to_one_field_type(&field_info.ty);

				// Validate relationship type matches field type
				if is_one_to_one && rel_attr.rel_type != crate::rel::RelationType::OneToOne {
					return Err(syn::Error::new(
						rel_attr.span,
						"OneToOneField must use #[rel(one_to_one, ...)]",
					));
				}
				if is_foreign_key_field_type(&field_info.ty)
					&& rel_attr.rel_type != crate::rel::RelationType::ForeignKey
				{
					return Err(syn::Error::new(
						rel_attr.span,
						"ForeignKeyField must use #[rel(foreign_key, ...)]",
					));
				}

				// Generate ID column name: db_column or {field_name}_id
				let id_column_name = rel_attr
					.db_column
					.clone()
					.unwrap_or_else(|| format!("{}_id", field_info.name));

				fk_field_infos.push(ForeignKeyFieldInfo {
					field_name: field_info.name.clone(),
					target_type: target_type.clone(),
					id_column_name,
					related_name: rel_attr.related_name.clone(),
					is_one_to_one,
					rel_attr: rel_attr.clone(),
				});
			}
		}
	}

	// Find all primary key fields
	let pk_fields: Vec<_> = field_infos
		.iter()
		.filter(|f| f.config.primary_key)
		.collect();

	if pk_fields.is_empty() {
		return Err(syn::Error::new_spanned(
			struct_name,
			"Model must have at least one primary key field",
		));
	}

	// Determine if this is a composite primary key
	let is_composite_pk = pk_fields.len() > 1;

	// Find all indexed fields
	let indexed_fields: Vec<_> = field_infos
		.iter()
		.filter(|f| f.config.index.unwrap_or(false))
		.map(|f| f.name.to_string())
		.collect();

	// Find all check constraint fields
	let check_constraints: Vec<(String, String)> = field_infos
		.iter()
		.filter_map(|f| {
			f.config
				.check
				.as_ref()
				.map(|expr| (f.name.to_string(), expr.clone()))
		})
		.collect();

	// Extract check constraint names and expressions for code generation
	let check_constraint_names: Vec<String> = check_constraints
		.iter()
		.map(|(field_name, _)| format!("{}_check", field_name))
		.collect();
	let check_constraint_expressions: Vec<String> = check_constraints
		.iter()
		.map(|(_, expr)| expr.clone())
		.collect();

	// Process unique constraints from model config
	let unique_constraints: Vec<(Vec<String>, Option<String>, Option<String>)> = model_config
		.constraints
		.iter()
		.map(|c| match c {
			ConstraintSpec::Unique {
				fields,
				name,
				condition,
			} => (fields.clone(), name.clone(), condition.clone()),
		})
		.collect();

	// Generate unique constraint names and definitions for code generation
	let unique_constraint_names: Vec<String> = unique_constraints
		.iter()
		.map(|(fields, name, _)| {
			if let Some(n) = name {
				n.clone()
			} else {
				// Auto-generate name: {table_name}_{field1}_{field2}_uniq
				format!("{}_{}_uniq", table_name, fields.join("_"))
			}
		})
		.collect();

	let unique_constraint_definitions: Vec<String> = unique_constraints
		.iter()
		.map(|(fields, _, condition)| {
			let fields_str = fields.join(", ");
			if let Some(cond) = condition {
				format!("UNIQUE ({}) WHERE {}", fields_str, cond)
			} else {
				format!("UNIQUE ({})", fields_str)
			}
		})
		.collect();

	// Token streams that register each model-level UNIQUE constraint
	// (e.g., from `unique_together`) into ModelMetadata.constraints so the
	// migration autodetector can emit AddConstraint operations.
	// See reinhardt-web#4022.
	let unique_constraint_field_lists: Vec<Vec<String>> = unique_constraints
		.iter()
		.map(|(fields, _, _)| fields.clone())
		.collect();

	// Define composite_pk_type_def and holder for code generation
	let composite_pk_type_def: Option<TokenStream>;
	// Note: composite_pk_type_holder is only assigned in the composite PK branch,
	// but must be declared here to extend its lifetime beyond the if-else scope
	#[allow(unused_assignments)]
	let mut composite_pk_type_holder: Option<Type> = None;

	// For single PK, extract field info
	let (pk_name, _pk_ty, pk_is_option, pk_type) = if !is_composite_pk {
		composite_pk_type_def = None;
		let pk_field = pk_fields[0];
		let pk_name = &pk_field.name;
		let pk_ty = &pk_field.ty;
		let (pk_is_option, pk_inner_ty) = extract_option_type(pk_ty);
		let pk_type = if pk_is_option { pk_inner_ty } else { pk_ty };
		(pk_name, pk_ty, pk_is_option, pk_type)
	} else {
		// Composite primary key: generate dedicated composite PK type
		let composite_pk_name =
			syn::Ident::new(&format!("{}CompositePk", struct_name), struct_name.span());

		// Generate the composite PK type definition
		composite_pk_type_def = Some(generate_composite_pk_type(struct_name, &pk_fields));

		// Use the generated composite PK type and store in holder (avoid temporary variable)
		composite_pk_type_holder = Some(parse_quote! { #composite_pk_name });
		let composite_pk_type_ref = composite_pk_type_holder.as_ref().unwrap();

		// Use first field name for primary_key_field() (legacy API compatibility)
		let first_pk_name = &pk_fields[0].name;
		(
			first_pk_name,
			composite_pk_type_ref,
			false,
			composite_pk_type_ref,
		)
	};

	// Generate field_metadata implementation
	let field_metadata_items = generate_field_metadata(&field_infos, &fk_field_infos)?;

	// Generate auto-registration code
	let registration_code = generate_registration_code(
		struct_name,
		app_label,
		table_name,
		&field_infos,
		&fk_field_infos,
		&unique_constraint_names,
		&unique_constraint_field_lists,
	)?;

	// Generate relationship registration code for RELATIONSHIPS registry
	let relationship_registrations =
		generate_relationship_registrations(struct_name, app_label, &field_infos, &fk_field_infos);

	// Generate primary_key() and set_primary_key() implementations
	let (pk_impl, set_pk_impl, composite_pk_impl) = if is_composite_pk {
		// Composite primary key implementation
		let composite_impl = generate_composite_pk_impl(&pk_fields);

		// For composite PK, use the generated composite PK type
		let pk_field_names: Vec<_> = pk_fields.iter().map(|f| &f.name).collect();

		// Check if any field is Option
		let has_option_fields = pk_fields.iter().any(|f| {
			let (is_option, _) = extract_option_type(&f.ty);
			is_option
		});

		let pk_getter = if has_option_fields {
			// If any field is Option, check all fields have values
			quote! {
				fn primary_key(&self) -> Option<Self::PrimaryKey> {
					// Check if all fields have values
					if #(self.#pk_field_names.is_some())&&* {
						Some(Self::PrimaryKey::new(
							#(self.#pk_field_names.clone().unwrap()),*
						))
					} else {
						None
					}
				}
			}
		} else {
			// All fields are non-Option, construct composite PK directly
			quote! {
				fn primary_key(&self) -> Option<Self::PrimaryKey> {
					Some(Self::PrimaryKey::new(
						#(self.#pk_field_names.clone()),*
					))
				}
			}
		};

		let pk_setter = if has_option_fields {
			quote! {
				fn set_primary_key(&mut self, value: Self::PrimaryKey) {
					#(
						self.#pk_field_names = Some(value.#pk_field_names);
					)*
				}
			}
		} else {
			quote! {
				fn set_primary_key(&mut self, value: Self::PrimaryKey) {
					#(
						self.#pk_field_names = value.#pk_field_names;
					)*
				}
			}
		};

		(pk_getter, pk_setter, composite_impl)
	} else {
		// Single primary key implementation
		let (pk_getter, pk_setter) = if pk_is_option {
			// If primary key is Option<T>, extract the inner value
			(
				quote! {
					fn primary_key(&self) -> Option<Self::PrimaryKey> {
						self.#pk_name.clone()
					}
				},
				quote! {
					fn set_primary_key(&mut self, value: Self::PrimaryKey) {
						self.#pk_name = Some(value);
					}
				},
			)
		} else {
			// If primary key is not Option, wrap in Some
			(
				quote! {
					fn primary_key(&self) -> Option<Self::PrimaryKey> {
						Some(self.#pk_name.clone())
					}
				},
				quote! {
					fn set_primary_key(&mut self, value: Self::PrimaryKey) {
						self.#pk_name = value;
					}
				},
			)
		};

		(pk_getter, pk_setter, quote! {})
	};

	// Generate field accessor methods
	let field_accessors = generate_field_accessors(struct_name, &field_infos);

	// Generate ManyToMany accessor methods
	let m2m_accessor_methods = generate_m2m_accessor_methods(struct_name, &field_infos);

	// Generate ForeignKey and OneToOne accessor methods
	let fk_accessor_methods = generate_fk_accessor_methods(struct_name, &field_infos);

	// Generate relationship metadata
	let relationship_metadata = generate_relationship_metadata(&rel_fields, app_label, struct_name);

	// Generate new() as zero-arg alias of build()
	let new_fn_impl = generate_new_alias(struct_name, &field_infos, &fk_id_field_names);

	// Generate typestate build() builder. new() is a zero-arg alias of build().
	// See issues #4400 and #4401.
	let build_fn_impl = generate_build_function(struct_name, &field_infos, &fk_id_field_names);

	// Generate getter/setter methods
	let getters = generate_getter_methods(struct_name, &field_infos);
	let setters = generate_setter_methods(struct_name, &field_infos);

	// Generate static FK accessor methods for type-safe reverse relationship access
	let fk_static_accessor_methods = generate_fk_static_accessor_methods(struct_name, &field_infos);

	// Generate field selector struct for type-safe JOIN/GROUP BY/HAVING operations
	let field_selector_name =
		syn::Ident::new(&format!("{}Fields", struct_name), struct_name.span());
	let field_selector_struct = generate_field_selector_struct(struct_name, &field_infos);

	let (info_impl_generics, info_ty_generics, info_where_clause) = generics.split_for_impl();
	let info_model_impl = if model_config.server_only {
		quote! {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl #info_impl_generics #reinhardt::model_info::InfoModel for #struct_name #info_ty_generics #info_where_clause {
				type PrimaryKey = #pk_type;
			}
		}
	} else {
		quote! {
			impl #info_impl_generics #reinhardt::model_info::InfoModel for #struct_name #info_ty_generics #info_where_clause {
				type PrimaryKey = #pk_type;
			}
		}
	};

	// Server-only models still expose native primary-key metadata for FK id
	// generation, but skip shared Info companion output.
	let info_struct = if model_config.server_only {
		quote! {}
	} else if model_config.info {
		generate_info_struct(
			struct_name,
			generics,
			&field_infos,
			&fk_field_infos,
			model_config.serde_serialize,
			model_config.serde_deserialize,
		)?
	} else {
		quote! {}
	};
	let shared_info_output = quote! {
		#info_model_impl
		#info_struct
	};

	// Determine the `type Objects` associated type for the Model impl.
	// When `#[model(manager = MyManager)]` is specified, `objects()` returns
	// the custom manager; otherwise it returns the default `Manager<Self>`
	// (Issue #3984).
	let objects_type = match &model_config.manager {
		Some(path) => quote! { #path },
		None => quote! { #orm_crate::Manager<Self> },
	};

	// Generate the Model implementation
	let expanded = quote! {
			// Generate composite PK type definition if needed
			#composite_pk_type_def

			#shared_info_output

			// Generate new() as a zero-arg alias of build()
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#new_fn_impl

			// Generate typestate build() builder (see #4400)
			#build_fn_impl

			// Generate getter methods for all fields
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#getters

			// Generate setter methods for user-defined fields
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#setters

			// Generate field accessor methods for type-safe field references
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#field_accessors

			// Generate ManyToMany accessor methods
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#m2m_accessor_methods

			// Generate ForeignKey and OneToOne accessor methods
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#fk_accessor_methods

			// Generate static FK accessor methods for type-safe reverse relationship access
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#fk_static_accessor_methods

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl #generics #orm_crate::Model for #struct_name #generics #where_clause {
			type PrimaryKey = #pk_type;
			type Fields = #field_selector_name;
			type Objects = #objects_type;

			fn table_name() -> &'static str {
				#table_name
			}

			fn new_fields() -> Self::Fields {
				#field_selector_name::new()
			}

			fn app_label() -> &'static str {
				#app_label
			}

			fn primary_key_field() -> &'static str {
				stringify!(#pk_name)
			}

			#pk_impl

			#set_pk_impl

			#composite_pk_impl

			fn field_metadata() -> Vec<#orm_crate::inspection::FieldInfo> {
				vec![
					#(#field_metadata_items),*
				]
			}

			fn index_metadata() -> Vec<#orm_crate::inspection::IndexInfo> {
				vec![
					#(
						#orm_crate::inspection::IndexInfo {
							name: format!("{}_{}_idx", <Self as #orm_crate::Model>::table_name(), #indexed_fields),
							fields: vec![#indexed_fields.to_string()],
							unique: false,
							condition: None,
						}
					),*
				]
			}

			fn constraint_metadata() -> Vec<#orm_crate::inspection::ConstraintInfo> {
				let mut constraints = Vec::new();
				// Check constraints
				#(
					constraints.push(#orm_crate::inspection::ConstraintInfo {
						name: #check_constraint_names.to_string(),
						constraint_type: #orm_crate::inspection::ConstraintType::Check,
						definition: #check_constraint_expressions.to_string(),
					});
				)*
				// Unique constraints
				#(
					constraints.push(#orm_crate::inspection::ConstraintInfo {
						name: #unique_constraint_names.to_string(),
						constraint_type: #orm_crate::inspection::ConstraintType::Unique,
						definition: #unique_constraint_definitions.to_string(),
					});
				)*
				constraints
			}

			#relationship_metadata
			}


			#registration_code

			// Register relationships in RELATIONSHIPS distributed slice
			#relationship_registrations

			// Generate field selector struct for type-safe JOIN/GROUP BY/HAVING operations
			#field_selector_struct
	};

	Ok(expanded)
}

/// Generate FieldInfo construction for field_metadata()
fn generate_field_metadata(
	field_infos: &[FieldInfo],
	fk_field_infos: &[ForeignKeyFieldInfo],
) -> Result<Vec<TokenStream>> {
	let mut items = Vec::new();

	// Filter out skipped, ManyToMany, ForeignKeyField, OneToOneField, and FK _id fields
	let regular_fields: Vec<_> = field_infos
		.iter()
		.filter(|f| {
			// Exclude fields marked with #[field(skip = true)]
			if f.config.skip {
				return false;
			}
			// Exclude FK _id fields (auto-generated by #[model] attribute macro)
			if f.is_fk_id_field {
				return false;
			}
			// Exclude ManyToMany
			if f.rel
				.as_ref()
				.map(|r| matches!(r.rel_type, crate::rel::RelationType::ManyToMany))
				.unwrap_or(false)
			{
				return false;
			}
			// Exclude ForeignKeyField and OneToOneField (we generate _id fields instead)
			if is_relationship_field_type(&f.ty) {
				return false;
			}
			true
		})
		.collect();

	let orm_crate = get_reinhardt_orm_crate();

	// If there are no regular fields, return empty vec
	if regular_fields.is_empty() {
		let _ = &orm_crate; // Suppress unused warning
	}

	for field_info in regular_fields {
		let name = field_info.name.to_string();
		let field_type_path = field_type_to_metadata_string(&field_info.ty, &field_info.config)?;
		let _field_type = map_type_to_field_type(&field_info.ty, &field_info.config)?;
		let config = &field_info.config;

		let (is_option, _) = extract_option_type(&field_info.ty);
		let nullable = config.null.unwrap_or(is_option);
		let primary_key = config.primary_key;
		let unique = config.unique.unwrap_or(false);
		let blank = config.blank.unwrap_or(false);
		let editable = config.editable.unwrap_or(true);

		// Build attributes map
		let mut attrs = Vec::new();
		if let Some(max_length) = config.max_length {
			attrs.push(quote! {
				attributes.insert(
					"max_length".to_string(),
					#orm_crate::fields::FieldKwarg::Uint(#max_length)
				);
			});
		}

		// Add validator attributes
		if let Some(email) = config.email
			&& email
		{
			attrs.push(quote! {
				attributes.insert(
					"email".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(true)
				);
			});
		}
		if let Some(url) = config.url
			&& url
		{
			attrs.push(quote! {
				attributes.insert(
					"url".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(true)
				);
			});
		}
		if let Some(min_length) = config.min_length {
			attrs.push(quote! {
				attributes.insert(
					"min_length".to_string(),
					#orm_crate::fields::FieldKwarg::Uint(#min_length)
				);
			});
		}
		if let Some(min_value) = config.min_value {
			attrs.push(quote! {
				attributes.insert(
					"min_value".to_string(),
					#orm_crate::fields::FieldKwarg::Int(#min_value)
				);
			});
		}
		if let Some(max_value) = config.max_value {
			attrs.push(quote! {
				attributes.insert(
					"max_value".to_string(),
					#orm_crate::fields::FieldKwarg::Int(#max_value)
				);
			});
		}

		// Generated Columns
		if let Some(ref generated_expr) = config.generated {
			attrs.push(quote! {
				attributes.insert(
					"generated".to_string(),
					#orm_crate::fields::FieldKwarg::String(#generated_expr.to_string())
				);
			});
		}
		if let Some(generated_stored) = config.generated_stored {
			attrs.push(quote! {
				attributes.insert(
					"generated_stored".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#generated_stored)
				);
			});
		}
		#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]
		if let Some(generated_virtual) = config.generated_virtual {
			attrs.push(quote! {
				attributes.insert(
					"generated_virtual".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#generated_virtual)
				);
			});
		}

		// Identity/Auto-increment
		#[cfg(feature = "db-postgres")]
		if let Some(identity_always) = config.identity_always {
			attrs.push(quote! {
				attributes.insert(
					"identity_always".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#identity_always)
				);
			});
		}
		#[cfg(feature = "db-postgres")]
		if let Some(identity_by_default) = config.identity_by_default {
			attrs.push(quote! {
				attributes.insert(
					"identity_by_default".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#identity_by_default)
				);
			});
		}
		#[cfg(feature = "db-mysql")]
		if let Some(auto_increment) = config.auto_increment {
			attrs.push(quote! {
				attributes.insert(
					"auto_increment".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#auto_increment)
				);
			});
		}
		#[cfg(feature = "db-sqlite")]
		if let Some(autoincrement) = config.autoincrement {
			attrs.push(quote! {
				attributes.insert(
					"autoincrement".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#autoincrement)
				);
			});
		}

		// Character Set & Collation
		if let Some(ref collate) = config.collate {
			attrs.push(quote! {
				attributes.insert(
					"collate".to_string(),
					#orm_crate::fields::FieldKwarg::String(#collate.to_string())
				);
			});
		}
		#[cfg(feature = "db-mysql")]
		if let Some(ref character_set) = config.character_set {
			attrs.push(quote! {
				attributes.insert(
					"character_set".to_string(),
					#orm_crate::fields::FieldKwarg::String(#character_set.to_string())
				);
			});
		}

		// Comment
		#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
		if let Some(ref comment) = config.comment {
			attrs.push(quote! {
				attributes.insert(
					"comment".to_string(),
					#orm_crate::fields::FieldKwarg::String(#comment.to_string())
				);
			});
		}

		// Storage Optimization (PostgreSQL)
		#[cfg(feature = "db-postgres")]
		if let Some(ref storage) = config.storage {
			let storage_str = match storage {
				StorageStrategy::Plain => "plain",
				StorageStrategy::Extended => "extended",
				StorageStrategy::External => "external",
				StorageStrategy::Main => "main",
			};
			attrs.push(quote! {
				attributes.insert(
					"storage".to_string(),
					#orm_crate::fields::FieldKwarg::String(#storage_str.to_string())
				);
			});
		}
		#[cfg(feature = "db-postgres")]
		if let Some(ref compression) = config.compression {
			let compression_str = match compression {
				CompressionMethod::Pglz => "pglz",
				CompressionMethod::Lz4 => "lz4",
			};
			attrs.push(quote! {
				attributes.insert(
					"compression".to_string(),
					#orm_crate::fields::FieldKwarg::String(#compression_str.to_string())
				);
			});
		}

		// ON UPDATE Trigger (MySQL)
		#[cfg(feature = "db-mysql")]
		if let Some(on_update_current_timestamp) = config.on_update_current_timestamp {
			attrs.push(quote! {
				attributes.insert(
					"on_update_current_timestamp".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#on_update_current_timestamp)
				);
			});
		}

		// Invisible Columns (MySQL)
		#[cfg(feature = "db-mysql")]
		if let Some(invisible) = config.invisible {
			attrs.push(quote! {
				attributes.insert(
					"invisible".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#invisible)
				);
			});
		}

		// Full-Text Index (PostgreSQL, MySQL)
		#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
		if let Some(fulltext) = config.fulltext {
			attrs.push(quote! {
				attributes.insert(
					"fulltext".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#fulltext)
				);
			});
		}

		// Numeric Attributes (MySQL, deprecated)
		#[cfg(feature = "db-mysql")]
		if let Some(unsigned) = config.unsigned {
			attrs.push(quote! {
				attributes.insert(
					"unsigned".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#unsigned)
				);
			});
		}
		#[cfg(feature = "db-mysql")]
		if let Some(zerofill) = config.zerofill {
			attrs.push(quote! {
				attributes.insert(
					"zerofill".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#zerofill)
				);
			});
		}

		let db_column_value = match &config.db_column {
			Some(col) => quote! { Some(#col.to_string()) },
			None => quote! { None },
		};

		let item = quote! {
			{
				let mut attributes = ::std::collections::HashMap::new();
				#(#attrs)*

				#orm_crate::inspection::FieldInfo {
					name: #name.to_string(),
					field_type: #field_type_path.to_string(),
					nullable: #nullable,
					primary_key: #primary_key,
					unique: #unique,
					blank: #blank,
					editable: #editable,
					default: None,
					db_default: None,
					db_column: #db_column_value,
					choices: None,
					attributes,
				}
			}
		};

		items.push(item);
	}

	// Generate _id field metadata for ForeignKeyField and OneToOneField
	for fk_info in fk_field_infos {
		let name = &fk_info.id_column_name;
		let nullable = fk_info.rel_attr.null.unwrap_or(false);
		let unique = fk_info.is_one_to_one; // OneToOne fields have UNIQUE constraint
		let db_index = fk_info.rel_attr.db_index.unwrap_or(true); // FK fields are indexed by default

		// Generate the field type based on target model's primary key
		// We use IntegerField as a safe default; runtime will resolve the actual type
		let field_type_path = "IntegerField";

		let item = quote! {
			{
				let mut attributes = ::std::collections::HashMap::new();
				if #db_index {
					attributes.insert(
						"db_index".to_string(),
						#orm_crate::fields::FieldKwarg::Bool(true)
					);
				}

				#orm_crate::inspection::FieldInfo {
					name: #name.to_string(),
					field_type: #field_type_path.to_string(),
					nullable: #nullable,
					primary_key: false,
					unique: #unique,
					blank: false,
					editable: true,
					default: None,
					db_default: None,
					db_column: None,
					choices: None,
					attributes,
				}
			}
		};

		items.push(item);
	}

	Ok(items)
}

/// Generate automatic registration code using ctor
fn generate_registration_code(
	struct_name: &syn::Ident,
	app_label: &str,
	table_name: &str,
	field_infos: &[FieldInfo],
	fk_field_infos: &[ForeignKeyFieldInfo],
	unique_constraint_names: &[String],
	unique_constraint_field_lists: &[Vec<String>],
) -> Result<TokenStream> {
	let migrations_crate = get_reinhardt_migrations_crate();
	let orm_crate = get_reinhardt_orm_crate();
	let model_name = struct_name.to_string();
	let register_fn_name = syn::Ident::new(
		&format!(
			"__register_{}_model",
			struct_name.to_string().to_lowercase()
		),
		struct_name.span(),
	);

	// Separate ManyToMany fields from regular fields (also exclude ForeignKeyField/OneToOneField and FK _id fields)
	let (m2m_fields, regular_fields_with_fk_id): (Vec<_>, Vec<_>) =
		field_infos.iter().partition(|f| {
			// Exclude ManyToMany
			if f.rel
				.as_ref()
				.map(|r| matches!(r.rel_type, crate::rel::RelationType::ManyToMany))
				.unwrap_or(false)
			{
				return true;
			}
			// Exclude ForeignKeyField and OneToOneField (they are virtual, we generate _id fields instead)
			if is_relationship_field_type(&f.ty) {
				return true;
			}
			false
		});

	// Filter out FK _id fields and skip fields from regular_fields
	let regular_fields: Vec<_> = regular_fields_with_fk_id
		.into_iter()
		.filter(|f| !f.is_fk_id_field && !f.config.skip)
		.collect();

	// Generate field registration code for regular fields
	let mut field_registrations = Vec::new();
	for field_info in &regular_fields {
		let field_name = field_info.name.to_string();
		let field_type = map_type_to_field_type(&field_info.ty, &field_info.config)?;
		let config = &field_info.config;

		let mut params = Vec::new();
		if config.primary_key {
			params.push(quote! { .with_param("primary_key", "true") });
		}

		// auto_increment emission for PK and non-PK fields is handled below,
		// gated on `is_integer_primary_key_type` so non-integer PKs (Uuid,
		// String, custom types) do not accidentally inherit
		// `auto_increment = "true"`. See reinhardt-web#4378.

		// not_null: infer from Rust Option type
		let (is_option, _) = extract_option_type(&field_info.ty);
		let is_not_null = if let Some(null) = config.null {
			!null
		} else if config.primary_key {
			true
		} else {
			!is_option
		};
		if is_not_null {
			params.push(quote! { .with_param("not_null", "true") });
		}

		if let Some(max_length) = config.max_length {
			let ml_str = max_length.to_string();
			params.push(quote! { .with_param("max_length", #ml_str) });
		}
		if let Some(null) = config.null {
			params.push(quote! { .with_nullable(#null) });
		}
		if let Some(unique) = config.unique
			&& unique
		{
			params.push(quote! { .with_param("unique", "true") });
		}
		// Infer nullable from Rust type when not explicitly set.
		//
		// PK columns are always NOT NULL at the DB level. The Option<T>
		// wrapper for PKs is a Rust-side convention to allow `id = None`
		// before the DB assigns the auto-increment value, not a DB-level
		// nullability statement. Emitting `null = "true"` for `Option<T>`
		// PKs would diverge from `column_def_to_field_state`'s migration-
		// replay output (which derives nullability from `not_null`) and
		// surface as a spurious `AlterColumn` for the unchanged PK under
		// offline state reconstruction.
		//
		// See reinhardt-web#4052 for the residual regression.
		if config.null.is_none() {
			let (is_option, _) = extract_option_type(&field_info.ty);
			let nullable = !config.primary_key && is_option;
			params.push(quote! { .with_nullable(#nullable) });
		}
		// auto_increment: explicit value or default true for integer PKs
		if config.primary_key && is_integer_primary_key_type(&field_info.ty) {
			let auto_inc = config.auto_increment.unwrap_or(true);
			let auto_inc_str = auto_inc.to_string();
			params.push(quote! { .with_param("auto_increment", #auto_inc_str) });
		} else if let Some(auto_increment) = config.auto_increment {
			let auto_inc_str = auto_increment.to_string();
			params.push(quote! { .with_param("auto_increment", #auto_inc_str) });
		}
		// auto_now / auto_now_add params
		if config.auto_now == Some(true) {
			params.push(quote! { .with_param("auto_now", "true") });
		}
		if config.auto_now_add == Some(true) {
			params.push(quote! { .with_param("auto_now_add", "true") });
		}

		// Propagate `#[field(default = ...)]` into FieldState.params so the
		// autodetector emits `ColumnDefinition.default = Some(<sql>)`. Without
		// this, makemigrations dropped the default on the floor and the
		// runner produced `ADD COLUMN ... NOT NULL` with no DEFAULT — see
		// reinhardt-web#4447. Unrecognised expression forms are intentionally
		// skipped (today's behaviour) rather than emitted as garbage.
		if let Some(ref default_expr) = config.default
			&& let Some(serialized) = serialize_field_default(default_expr)
		{
			params.push(quote! { .with_param("default", #serialized) });
		}

		// Generate ForeignKey information if present
		let fk_registration = if let Some(fk_spec) = &config.foreign_key {
			match fk_spec {
				ForeignKeySpec::Type(ty) => {
					// For direct type reference, extract type name and convert to snake_case
					let type_name_str = quote! { #ty }.to_string();
					quote! {
						.with_foreign_key({
							// Extract last segment of type path and convert to snake_case
							let type_name = #type_name_str;
							let last_segment = type_name.split("::").last().unwrap_or(&type_name);
							let referenced_table = #migrations_crate::to_snake_case(last_segment);

							#migrations_crate::ForeignKeyInfo {
								referenced_table,
								referenced_column: "id".to_string(),
								on_delete: #migrations_crate::ForeignKeyAction::Cascade,
								on_update: #migrations_crate::ForeignKeyAction::Cascade,
							}
						})
					}
				}
				ForeignKeySpec::AppModel {
					app_label,
					model_name,
				} => {
					let table_name_str = format!("{}_{}", app_label, model_name.to_lowercase());
					quote! {
						.with_foreign_key(#migrations_crate::ForeignKeyInfo {
							referenced_table: #table_name_str.to_string(),
							referenced_column: "id".to_string(),
							on_delete: #migrations_crate::ForeignKeyAction::Cascade,
							on_update: #migrations_crate::ForeignKeyAction::Cascade,
						})
					}
				}
			}
		} else {
			quote! {}
		};

		field_registrations.push(quote! {
			metadata.add_field(
				#field_name.to_string(),
				#migrations_crate::model_registry::FieldMetadata::new(#field_type)
					#(#params)*
					#fk_registration
			);
		});
	}

	// Generate ManyToMany field registration code
	let mut m2m_registrations = Vec::new();
	for field_info in &m2m_fields {
		let field_name = field_info.name.to_string();

		// Get target model name: from #[rel(to = "...")] or infer from ManyToManyField<Source, Target>
		let to_model = if let Some(rel) = &field_info.rel
			&& let Some(to_type) = &rel.to
		{
			// Explicit 'to' parameter in #[rel(...)]
			quote! { #to_type }.to_string()
		} else if let Some(target_ty) = extract_m2m_target_type(&field_info.ty) {
			// Infer from ManyToManyField<Source, Target> - extract Target type name
			if let Type::Path(type_path) = target_ty
				&& let Some(last_segment) = type_path.path.segments.last()
			{
				last_segment.ident.to_string()
			} else {
				continue; // Skip if cannot extract target type
			}
		} else {
			continue; // Skip if no 'to' parameter and cannot infer from type
		};

		// Get relationship attributes (may be None if no #[rel(...)] attribute)
		let related_name = field_info
			.rel
			.as_ref()
			.and_then(|r| r.related_name.as_ref())
			.map(|r| quote! { Some(#r.to_string()) })
			.unwrap_or(quote! { None });
		let through = field_info
			.rel
			.as_ref()
			.and_then(|r| r.through.as_ref())
			.map(|t| quote! { Some(#t.to_string()) })
			.unwrap_or(quote! { None });
		let source_field = field_info
			.rel
			.as_ref()
			.and_then(|r| r.source_field.as_ref())
			.map(|s| quote! { Some(#s.to_string()) })
			.unwrap_or(quote! { None });
		let target_field = field_info
			.rel
			.as_ref()
			.and_then(|r| r.target_field.as_ref())
			.map(|t| quote! { Some(#t.to_string()) })
			.unwrap_or(quote! { None });

		m2m_registrations.push(quote! {
			metadata.add_many_to_many(
				#migrations_crate::model_registry::ManyToManyMetadata {
					field_name: #field_name.to_string(),
					to_model: #to_model.to_string(),
					related_name: #related_name,
					through: #through,
					source_field: #source_field,
					target_field: #target_field,
					db_constraint_prefix: None,
				}
			);
		});
	}

	// Generate FK _id field registration code
	let mut fk_id_registrations = Vec::new();
	for fk_info in fk_field_infos {
		let id_column_name = &fk_info.id_column_name;
		let nullable = fk_info.rel_attr.null.unwrap_or(false);
		let unique = fk_info.is_one_to_one; // OneToOne fields have UNIQUE constraint
		let db_index = fk_info.rel_attr.db_index.unwrap_or(true); // FK fields are indexed by default
		let not_null_str = (!nullable).to_string();
		let unique_str = unique.to_string();
		let db_index_str = db_index.to_string();

		// Extract "User" from ForeignKeyField<User>
		let target_model_name = if let Type::Path(type_path) = &fk_info.target_type {
			type_path
				.path
				.segments
				.last()
				.map(|seg| seg.ident.to_string())
				.unwrap_or_else(|| "Unknown".to_string())
		} else {
			"Unknown".to_string()
		};

		// `fk_target_app` is sourced from the FK target type itself via
		// `<TargetType as Model>::app_label()` — the model's
		// authoritative app label, which respects `#[app_label = "..."]`
		// overrides and any future remapping. The macro deliberately
		// does NOT try to guess the app label from the syntactic path
		// the user wrote: a path like `reinhardt_auth::User` is just a
		// crate / module name and can diverge from the registered app
		// label (e.g. crate `reinhardt_auth` registering its app as
		// `"auth"` via `#[app_label("auth")]`), and a bare ident
		// `User` can come from a `use`-import out of another crate.
		// Reading `app_label()` off the type sidesteps both pitfalls.
		//
		// The qualified lookup at FK resolution time uses this value,
		// so the qualifier always matches the registry key regardless
		// of whether the target is referenced by a bare ident, a
		// `use`-imported ident, or an absolute path. The user can
		// disambiguate same-name models across apps by writing a
		// path-typed FK target (`ForeignKeyField<reinhardt_auth::User>`)
		// or by relying on Rust's normal scoping — Rust resolves the
		// type and the macro reads the type's own app label.
		//
		// We only emit `fk_target_app` for `Type::Path` target types
		// (the common case for `ForeignKeyField<T>`). Other shapes
		// (`fn` types, trait objects, etc.) cannot be FK targets and
		// don't reach this branch in practice.
		//
		// See issue #4436 and PR #4440 review threads on
		// `model_derive.rs` line 2863 and `operations.rs` line 2836.
		let fk_target_app_chain = if let Type::Path(_) = &fk_info.target_type {
			let target_ty = &fk_info.target_type;
			quote! {
				.with_param(
					"fk_target_app",
					<#target_ty as #orm_crate::Model>::app_label(),
				)
			}
		} else {
			quote! {}
		};

		// The `FieldType::Uuid` value here is a placeholder. The real column
		// type is resolved at migration-generation time by looking up the
		// target model's primary key in the global `ModelRegistry`
		// (see `ColumnDefinition::from_field_state`). The placeholder is
		// required because the target model's PK type is not knowable at
		// macro-expansion time (the registry is populated at runtime via
		// `#[ctor::ctor]`).
		//
		// `nullable` is set on the structured `FieldMetadata.nullable`
		// field (single source of truth — `FieldMetadata::to_model_state`
		// reads it directly when constructing `FieldState`). `not_null`
		// is still emitted as a parameter because `ColumnDefinition::from_field_state`
		// reads `params["not_null"]` to set its boolean. Reflects the
		// non-`Option<_>` nullability of `ForeignKeyField<T>` (issue #4431).
		// Follow-up tracked in #4436 to migrate `from_field_state` to
		// derive `not_null` from `FieldState.nullable` and drop this param.
		fk_id_registrations.push(quote! {
			metadata.add_field(
				#id_column_name.to_string(),
				#migrations_crate::model_registry::FieldMetadata::new(
					#migrations_crate::FieldType::Uuid
				)
					.with_nullable(#nullable)
					.with_param("not_null", #not_null_str)
					.with_param("unique", #unique_str)
					.with_param("db_index", #db_index_str)
					.with_param("fk_target", #target_model_name)
					#fk_target_app_chain
			);
		});
	}

	// Generate type path for global model registry
	let type_path = quote! { #struct_name }.to_string();

	// Build per-constraint registration blocks for ModelMetadata.
	// We walk three parallel vectors (names + field lists) and emit one
	// `metadata.add_constraint(...)` call per declared `unique_together`.
	// See reinhardt-web#4022.
	let constraint_registrations: Vec<TokenStream> = unique_constraint_names
		.iter()
		.zip(unique_constraint_field_lists.iter())
		.map(|(name, fields)| {
			let field_lits = fields.iter().map(|f| quote! { #f.to_string() });
			quote! {
				metadata.add_constraint(
					#migrations_crate::ConstraintDefinition {
						name: #name.to_string(),
						constraint_type: "unique".to_string(),
						fields: vec![ #(#field_lits),* ],
						expression: None,
						foreign_key_info: None,
					}
				);
			}
		})
		.collect();

	let code = quote! {
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#[::ctor::ctor]
		fn #register_fn_name() {
			use #migrations_crate::model_registry::ModelMetadata;

			// Register in migration registry
			let mut metadata = ModelMetadata::new(
				#app_label,
				#model_name,
				#table_name,
			);

			#(#field_registrations)*
			#(#fk_id_registrations)*
			#(#m2m_registrations)*
			#(#constraint_registrations)*

			#migrations_crate::model_registry::global_registry().register_model(metadata);

			// Register in global model registry for foreign_key resolution
			#orm_crate::registry::global_model_registry().register(
				#orm_crate::registry::ModelInfo {
					app_label: #app_label.to_string(),
					model_name: #model_name.to_string(),
					type_path: #type_path.to_string(),
					table_name: #table_name.to_string(),
				}
			);
		}
	};

	Ok(code)
}

/// Generate relationship registration code for RELATIONSHIPS registry
///
/// This function scans all fields in the model and detects relationship fields
/// (ForeignKeyField, OneToOneField, ManyToManyField) automatically, then generates
/// linkme distributed_slice registration code for each relationship.
///
/// For ForeignKey and OneToOne fields with `related_name`, this also generates
/// reverse relationship registrations for building reverse accessors at runtime.
///
/// # Arguments
///
/// * `struct_name` - The name of the model struct
/// * `app_label` - The app label for the model
/// * `field_infos` - All field information including relationship fields
/// * `fk_field_infos` - Extracted ForeignKey field information
///
/// # Returns
///
/// TokenStream containing linkme distributed_slice registrations for all relationships
fn generate_relationship_registrations(
	struct_name: &syn::Ident,
	app_label: &str,
	field_infos: &[FieldInfo],
	fk_field_infos: &[ForeignKeyFieldInfo],
) -> TokenStream {
	let reinhardt = get_reinhardt_crate();
	let _orm_crate = get_reinhardt_orm_crate();
	// Fixes #793: Use dynamic crate path resolution instead of hardcoded ::linkme
	let linkme = get_linkme_crate();
	let mut registrations = Vec::new();
	let model_name = struct_name.to_string();

	// Process ForeignKey and OneToOne fields
	for fk_info in fk_field_infos {
		let field_name = &fk_info.field_name;
		let field_name_str = field_name.to_string();
		let is_one_to_one = fk_info.is_one_to_one;

		// Extract target model name from Type
		let target_model_name = if let Type::Path(type_path) = &fk_info.target_type {
			type_path
				.path
				.segments
				.last()
				.map(|seg| seg.ident.to_string())
				.unwrap_or_else(|| "Unknown".to_string())
		} else {
			"Unknown".to_string()
		};

		// Get related_name from RelAttribute if present
		let related_name_opt = fk_info.rel_attr.related_name.as_ref();
		let related_name = related_name_opt
			.map(|r| quote! { Some(#r) })
			.unwrap_or(quote! { None });

		// Get db_column from RelAttribute if present, otherwise use "{field_name}_id"
		let db_column = fk_info
			.rel_attr
			.db_column
			.as_ref()
			.map(|c| quote! { Some(#c) })
			.unwrap_or_else(|| {
				let default_db_column = format!("{}_id", field_name_str);
				quote! { Some(#default_db_column) }
			});

		// Determine relationship type
		let relationship_type = if is_one_to_one {
			quote! { #reinhardt::apps::registry::RelationshipType::OneToOne }
		} else {
			quote! { #reinhardt::apps::registry::RelationshipType::ForeignKey }
		};

		// Generate unique static variable name for forward relationship
		let static_var_name = syn::Ident::new(
			&format!(
				"__REL_{}_{}_TO_{}",
				model_name.to_uppercase(),
				field_name_str.to_uppercase(),
				target_model_name.to_uppercase()
			),
			struct_name.span(),
		);

		// Generate registration code for forward relationship
		registrations.push(quote! {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#[#linkme::distributed_slice(#reinhardt::apps::registry::RELATIONSHIPS)]
			static #static_var_name: #reinhardt::apps::registry::RelationshipMetadata =
				#reinhardt::apps::registry::RelationshipMetadata {
					from_model: concat!(#app_label, ".", #model_name),
					to_model: #target_model_name,
					relationship_type: #relationship_type,
					field_name: #field_name_str,
					related_name: #related_name,
					db_column: #db_column,
					through_table: None,
				};
		});

		// Generate reverse relationship registration if related_name is present
		if let Some(related_name_str) = related_name_opt {
			// Determine reverse relationship type
			let reverse_relationship_type = if is_one_to_one {
				quote! { #reinhardt::apps::registry::RelationshipType::OneToOne }
			} else {
				// ForeignKey reverse is also ForeignKey (direction determined by from_model/to_model)
				quote! { #reinhardt::apps::registry::RelationshipType::ForeignKey }
			};

			// Generate unique static variable name for reverse relationship
			let reverse_static_var_name = syn::Ident::new(
				&format!(
					"__REL_REVERSE_{}_{}_TO_{}",
					target_model_name.to_uppercase(),
					related_name_str.to_uppercase(),
					model_name.to_uppercase()
				),
				struct_name.span(),
			);

			// Generate registration code for reverse relationship
			registrations.push(quote! {
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				#[#linkme::distributed_slice(#reinhardt::apps::registry::RELATIONSHIPS)]
				static #reverse_static_var_name: #reinhardt::apps::registry::RelationshipMetadata =
					#reinhardt::apps::registry::RelationshipMetadata {
						from_model: #target_model_name,
						to_model: concat!(#app_label, ".", #model_name),
						relationship_type: #reverse_relationship_type,
						field_name: #related_name_str,
						related_name: Some(#field_name_str),
						db_column: None,
						through_table: None,
					};
			});
		}
	}

	// Process ManyToMany fields
	for field_info in field_infos {
		// Check if this is a ManyToMany field
		if !is_many_to_many_field_type(&field_info.ty) {
			continue;
		}

		let field_name = &field_info.name;
		let field_name_str = field_name.to_string();

		// Extract target model name from ManyToManyField<Source, Target>
		let target_model_name = if let Some(target_ty) = extract_m2m_target_type(&field_info.ty) {
			if let Type::Path(type_path) = target_ty {
				type_path
					.path
					.segments
					.last()
					.map(|seg| seg.ident.to_string())
					.unwrap_or_else(|| "Unknown".to_string())
			} else {
				continue; // Skip if cannot extract target type
			}
		} else {
			continue; // Skip if no target type
		};

		// Get relationship attributes from RelAttribute if present
		let (related_name, through_table, related_name_opt) = if let Some(rel) = &field_info.rel {
			let related_name_str = rel.related_name.as_ref();
			let related_name = related_name_str
				.map(|r| quote! { Some(#r) })
				.unwrap_or(quote! { None });

			let through_table = rel
				.through
				.as_ref()
				.map(|t| {
					let through_str = quote! { #t }.to_string();
					quote! { Some(#through_str) }
				})
				.unwrap_or(quote! { None });

			(related_name, through_table, related_name_str)
		} else {
			(quote! { None }, quote! { None }, None)
		};

		// Generate unique static variable name for forward M2M relationship
		let static_var_name = syn::Ident::new(
			&format!(
				"__REL_M2M_{}_{}_TO_{}",
				model_name.to_uppercase(),
				field_name_str.to_uppercase(),
				target_model_name.to_uppercase()
			),
			struct_name.span(),
		);

		// Generate registration code for forward M2M relationship
		registrations.push(quote! {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#[#linkme::distributed_slice(#reinhardt::apps::registry::RELATIONSHIPS)]
			static #static_var_name: #reinhardt::apps::registry::RelationshipMetadata =
				#reinhardt::apps::registry::RelationshipMetadata {
					from_model: concat!(#app_label, ".", #model_name),
					to_model: #target_model_name,
					relationship_type: #reinhardt::apps::registry::RelationshipType::ManyToMany,
					field_name: #field_name_str,
					related_name: #related_name,
					db_column: None,
					through_table: #through_table,
				};
		});

		// Generate reverse M2M relationship registration if related_name is present
		if let Some(related_name_str) = related_name_opt {
			// Generate unique static variable name for reverse M2M relationship
			let reverse_static_var_name = syn::Ident::new(
				&format!(
					"__REL_M2M_REVERSE_{}_{}_TO_{}",
					target_model_name.to_uppercase(),
					related_name_str.to_uppercase(),
					model_name.to_uppercase()
				),
				struct_name.span(),
			);

			// Generate registration code for reverse M2M relationship
			registrations.push(quote! {
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				#[#linkme::distributed_slice(#reinhardt::apps::registry::RELATIONSHIPS)]
				static #reverse_static_var_name: #reinhardt::apps::registry::RelationshipMetadata =
					#reinhardt::apps::registry::RelationshipMetadata {
						from_model: #target_model_name,
						to_model: concat!(#app_label, ".", #model_name),
						relationship_type: #reinhardt::apps::registry::RelationshipType::ManyToMany,
						field_name: #related_name_str,
						related_name: Some(#field_name_str),
						db_column: None,
						through_table: #through_table,
					};
			});
		}
	}

	// Combine all registrations
	quote! {
		#(#registrations)*
	}
}

/// Generate composite primary key implementation
fn generate_composite_pk_impl(pk_fields: &[&FieldInfo]) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();

	let field_name_strings: Vec<String> = pk_fields.iter().map(|f| f.name.to_string()).collect();

	quote! {
		fn composite_primary_key() -> Option<#orm_crate::composite_pk::CompositePrimaryKey> {
			Some(
				#orm_crate::composite_pk::CompositePrimaryKey::new(
					vec![#(#field_name_strings.to_string()),*]
				)
				.expect("Invalid composite primary key")
			)
		}

		fn get_composite_pk_values(&self) -> ::std::collections::HashMap<String, #orm_crate::composite_pk::PkValue> {
			// Use the generated composite PK type's to_pk_values() method
			if let Some(pk) = self.primary_key() {
				pk.to_pk_values()
			} else {
				::std::collections::HashMap::new()
			}
		}
	}
}

/// Generate composite primary key type definition
///
/// Creates a dedicated struct type for composite primary keys with:
/// - Named fields matching the model's PK fields
/// - Derived traits: Debug, Clone, PartialEq, Eq, Hash
/// - From/Into conversions for tuple types
/// - Individual PkValue conversions for each field
fn generate_composite_pk_type(struct_name: &syn::Ident, pk_fields: &[&FieldInfo]) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();

	// Generate composite PK struct name: {ModelName}CompositePk
	let composite_pk_name =
		syn::Ident::new(&format!("{}CompositePk", struct_name), struct_name.span());

	// Extract field names and types
	let field_names: Vec<_> = pk_fields.iter().map(|f| &f.name).collect();
	let field_types: Vec<_> = pk_fields
		.iter()
		.map(|f| {
			let ty = &f.ty;
			let (is_option, inner_ty) = extract_option_type(ty);
			if is_option { inner_ty } else { ty }
		})
		.collect();

	// Generate From<tuple> implementation for easy construction
	let tuple_type = if field_types.len() == 1 {
		quote! { #(#field_types),* }
	} else {
		quote! { (#(#field_types),*) }
	};

	// Generate individual field conversions for PkValue
	let pk_value_conversions: Vec<_> = field_names
		.iter()
		.map(|name| {
			quote! {
				values.insert(
					stringify!(#name).to_string(),
					#orm_crate::composite_pk::PkValue::from(&self.#name)
				);
			}
		})
		.collect();

	quote! {
		/// Composite primary key type for #struct_name
		#[derive(Debug, Clone, PartialEq, Eq, Hash)]
		pub struct #composite_pk_name {
			#(pub #field_names: #field_types),*
		}

		impl #composite_pk_name {
			/// Create a new composite primary key
			pub fn new(#(#field_names: #field_types),*) -> Self {
				Self {
					#(#field_names),*
				}
			}

			/// Convert to a HashMap of PkValues for database operations
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub fn to_pk_values(&self) -> ::std::collections::HashMap<String, #orm_crate::composite_pk::PkValue> {
				let mut values = ::std::collections::HashMap::new();
				#(#pk_value_conversions)*
				values
			}
		}

		// Conversion from tuple type
		impl ::std::convert::From<#tuple_type> for #composite_pk_name {
			fn from(tuple: #tuple_type) -> Self {
				let (#(#field_names),*) = tuple;
				Self {
					#(#field_names),*
				}
			}
		}

		// Conversion to tuple type
		impl ::std::convert::From<#composite_pk_name> for #tuple_type {
			fn from(pk: #composite_pk_name) -> Self {
				(#(pk.#field_names),*)
			}
		}

		// Display implementation for composite primary key
		impl ::std::fmt::Display for #composite_pk_name {
			fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
				write!(f, "(")?;
				let mut first = true;
				#(
					if !first {
						write!(f, ", ")?;
					}
					write!(f, "{}={}", stringify!(#field_names), self.#field_names)?;
					first = false;
				)*
				write!(f, ")")
			}
		}
	}
}

/// Generate relationship metadata code for `#[rel]` attributes
///
/// Generates two methods:
/// - `relationship_metadata()` for Model trait (returns `Vec<RelationInfo>`)
/// - `__migration_relationships()` for migration system (returns `Vec<RelationshipMetadata>`)
fn generate_relationship_metadata(
	rel_fields: &[(Ident, RelAttribute)],
	_app_label: &str,
	_struct_name: &Ident,
) -> TokenStream {
	use crate::rel::RelationType;
	let orm_crate = get_reinhardt_orm_crate();

	if rel_fields.is_empty() {
		return quote! {
			fn relationship_metadata() -> Vec<#orm_crate::inspection::RelationInfo> {
				Vec::new()
			}
		};
	}

	let relation_info_items: Vec<TokenStream> = rel_fields
		.iter()
		.map(|(field_name, rel)| {
			let field_name_str = field_name.to_string();

			// Map RelationType to RelationshipType
			let relationship_type = match rel.rel_type {
				RelationType::ForeignKey => {
					quote! { #orm_crate::relationship::RelationshipType::ManyToOne }
				}
				RelationType::OneToOne => {
					quote! { #orm_crate::relationship::RelationshipType::OneToOne }
				}
				RelationType::OneToMany => {
					quote! { #orm_crate::relationship::RelationshipType::OneToMany }
				}
				RelationType::ManyToMany | RelationType::PolymorphicManyToMany => {
					quote! { #orm_crate::relationship::RelationshipType::ManyToMany }
				}
				RelationType::Polymorphic | RelationType::GenericForeignKey => {
					// Current design: Polymorphic and GenericForeignKey are treated as ManyToOne
					quote! { #orm_crate::relationship::RelationshipType::ManyToOne }
				}
				RelationType::GenericRelation => {
					// GenericRelation is a reverse lookup, similar to OneToMany
					quote! { #orm_crate::relationship::RelationshipType::OneToMany }
				}
			};

			let related_model = rel.to.as_ref().map_or_else(
				|| quote! { "" },
				|path| {
					let path_str = quote! { #path }.to_string();
					quote! { #path_str }
				},
			);

			let back_populates = rel.related_name.as_ref().map_or_else(
				|| quote! { None },
				|name| quote! { Some(#name.to_string()) },
			);

			// For ForeignKey, the foreign key field is the field itself
			let foreign_key = match rel.rel_type {
				RelationType::ForeignKey | RelationType::OneToOne => {
					quote! { Some(#field_name_str.to_string()) }
				}
				RelationType::OneToMany => rel
					.foreign_key
					.as_ref()
					.map_or_else(|| quote! { None }, |fk| quote! { Some(#fk.to_string()) }),
				_ => quote! { None },
			};

			// ManyToMany relationship fields
			let through_table = rel
				.through
				.as_ref()
				.map_or_else(|| quote! { None }, |t| quote! { Some(#t.to_string()) });
			let source_field = rel
				.source_field
				.as_ref()
				.map_or_else(|| quote! { None }, |s| quote! { Some(#s.to_string()) });
			let target_field = rel
				.target_field
				.as_ref()
				.map_or_else(|| quote! { None }, |t| quote! { Some(#t.to_string()) });

			quote! {
				#orm_crate::inspection::RelationInfo {
					name: #field_name_str.to_string(),
					relationship_type: #relationship_type,
					foreign_key: #foreign_key,
					related_model: #related_model.to_string(),
					back_populates: #back_populates,
					through_table: #through_table,
					source_field: #source_field,
					target_field: #target_field,
				}
			}
		})
		.collect();

	quote! {
		fn relationship_metadata() -> Vec<#orm_crate::inspection::RelationInfo> {
			vec![
				#(#relation_info_items),*
			]
		}
	}
}

/// Check if a type is Uuid or `Option<Uuid>`.
///
/// Thin projection of the shared `crate::pk_shape::pk_uuid_shape`
/// helper — see issue #4246 for why the underlying detection lives in
/// one place.
fn is_uuid_type(ty: &Type) -> bool {
	crate::pk_shape::pk_uuid_shape(ty).0
}

/// Check if a type is String or `Option<String>`
fn is_string_type(ty: &Type) -> bool {
	let (_, inner_ty) = extract_option_type(ty);
	if let Type::Path(type_path) = inner_ty
		&& let Some(last_segment) = type_path.path.segments.last()
	{
		return last_segment.ident == "String";
	}
	false
}

/// Check if a type is an integer type suitable for auto-increment primary key
/// Supports i8, i16, i32, i64, isize, u8, u16, u32, u64, usize and their Option<> variants
fn is_integer_primary_key_type(ty: &Type) -> bool {
	let (_, inner_ty) = extract_option_type(ty);
	if let Type::Path(type_path) = inner_ty
		&& let Some(last_segment) = type_path.path.segments.last()
	{
		let ident_str = last_segment.ident.to_string();
		return matches!(
			ident_str.as_str(),
			"i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize"
		);
	}
	false
}

/// Check if a type is DateTime<Utc> or `Option<DateTime<Utc>>`
fn is_datetime_utc_type(ty: &Type) -> bool {
	let (_, inner_ty) = extract_option_type(ty);
	if let Type::Path(type_path) = inner_ty
		&& let Some(last_segment) = type_path.path.segments.last()
	{
		// Check if the type is DateTime
		if last_segment.ident != "DateTime" {
			return false;
		}

		// Check if it has generic argument <Utc>
		if let PathArguments::AngleBracketed(args) = &last_segment.arguments
			&& let Some(GenericArgument::Type(Type::Path(arg_path))) = args.args.first()
			&& let Some(arg_segment) = arg_path.path.segments.last()
		{
			return arg_segment.ident == "Utc";
		}

		// DateTime without generic argument might still be DateTime<Utc> if imported
		// For safety, we treat it as DateTime<Utc>
		return true;
	}
	false
}

/// Check if a type is a ManyToManyField
fn is_many_to_many_field_type(ty: &Type) -> bool {
	if let Type::Path(type_path) = ty
		&& let Some(last_segment) = type_path.path.segments.last()
	{
		return last_segment.ident == "ManyToManyField";
	}
	false
}

/// Check if a type is a ForeignKeyField
fn is_foreign_key_field_type(ty: &Type) -> bool {
	if let Type::Path(type_path) = ty
		&& let Some(last_segment) = type_path.path.segments.last()
	{
		return last_segment.ident == "ForeignKeyField";
	}
	false
}

/// Check if a type is a OneToOneField
fn is_one_to_one_field_type(ty: &Type) -> bool {
	if let Type::Path(type_path) = ty
		&& let Some(last_segment) = type_path.path.segments.last()
	{
		return last_segment.ident == "OneToOneField";
	}
	false
}

/// Extract target type from ForeignKeyField<T> or OneToOneField<T>
fn extract_fk_target_type(ty: &Type) -> Option<&Type> {
	if let Type::Path(type_path) = ty
		&& let Some(last_segment) = type_path.path.segments.last()
		&& (last_segment.ident == "ForeignKeyField" || last_segment.ident == "OneToOneField")
		&& let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
	{
		return Some(inner_ty);
	}
	None
}

/// Extract target type from ManyToManyField<Source, Target>
/// Returns the second generic argument (Target model)
fn extract_m2m_target_type(ty: &Type) -> Option<&Type> {
	if let Type::Path(type_path) = ty
		&& let Some(last_segment) = type_path.path.segments.last()
		&& last_segment.ident == "ManyToManyField"
		&& let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments
		&& args.args.len() >= 2
		&& let Some(syn::GenericArgument::Type(target_ty)) = args.args.iter().nth(1)
	{
		return Some(target_ty);
	}
	None
}

/// Check if a type is a relationship field type (ForeignKeyField or OneToOneField)
fn is_relationship_field_type(ty: &Type) -> bool {
	is_foreign_key_field_type(ty) || is_one_to_one_field_type(ty)
}

/// Check if a field is a timestamp field that should be auto-set to Utc::now()
///
/// A field is considered a timestamp field only when explicitly annotated with:
/// - `#[field(auto_now_add = true)]` - auto-set on record creation
/// - `#[field(auto_now = true)]` - auto-set on every save
/// - `#[field(on_update_current_timestamp = true)]` - auto-set on record update (MySQL only)
fn is_timestamp_field(field: &FieldInfo) -> bool {
	let config = &field.config;

	// Check auto_now_add and auto_now (available on all DB backends)
	let auto_timestamp = config.auto_now_add == Some(true) || config.auto_now == Some(true);

	// Check on_update_current_timestamp (MySQL only)
	#[cfg(feature = "db-mysql")]
	let mysql_timestamp = config.on_update_current_timestamp == Some(true);
	#[cfg(not(feature = "db-mysql"))]
	let mysql_timestamp = false;

	auto_timestamp || mysql_timestamp
}

/// Extract the target model type from ForeignKeyField<T> or OneToOneField<T>
fn extract_foreign_key_target_type(ty: &Type) -> Type {
	// ForeignKeyField<User> -> User
	if let Type::Path(type_path) = ty
		&& let Some(segment) = type_path.path.segments.last()
		&& let PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(GenericArgument::Type(inner_ty)) = args.args.first()
	{
		return inner_ty.clone();
	}
	// Fallback: return the entire type
	ty.clone()
}

/// Check if a type is `Option<T>`
fn is_option_type(ty: &syn::Type) -> bool {
	if let syn::Type::Path(type_path) = ty
		&& let Some(segment) = type_path.path.segments.last()
	{
		return segment.ident == "Option";
	}
	false
}

/// Determine if a field should be auto-generated (excluded from required builder inputs).
fn is_auto_generated_field(field: &FieldInfo) -> bool {
	// Fields with skip = true are always excluded from model construction inputs.
	if field.config.skip {
		return true;
	}
	// FK _id fields are auto-generated (excluded from direct setters).
	if field.is_fk_id_field {
		return true;
	}

	let config = &field.config;

	// If include_in_new is explicitly set to false, exclude from required inputs.
	if config.include_in_new == Some(false) {
		return true;
	}

	// If include_in_new is explicitly set to true, always include in required inputs.
	if config.include_in_new == Some(true) {
		return false;
	}

	// Auto-detect timestamp fields
	if is_timestamp_field(field) {
		return true;
	}

	// Generated columns
	if config.generated.is_some() {
		return true;
	}

	// Database-specific ID auto-generation (PostgreSQL)
	#[cfg(feature = "db-postgres")]
	{
		if config.identity_always == Some(true) || config.identity_by_default == Some(true) {
			return true;
		}
	}

	// Database-specific ID auto-generation (MySQL)
	#[cfg(feature = "db-mysql")]
	{
		if config.auto_increment == Some(true) {
			return true;
		}
	}

	// Database-specific ID auto-generation (SQLite)
	#[cfg(feature = "db-sqlite")]
	{
		if config.autoincrement == Some(true) {
			return true;
		}
	}

	// ManyToManyField - always auto-generated with Default::default()
	if is_many_to_many_field_type(&field.ty) {
		return true;
	}

	// ForeignKeyField/OneToOneField - always auto-generated with Default::default()
	if is_relationship_field_type(&field.ty) {
		return true;
	}

	// ManyToMany relationship via #[rel(many_to_many, ...)]
	if let Some(rel) = &field.rel
		&& matches!(rel.rel_type, crate::rel::RelationType::ManyToMany)
	{
		return true;
	}

	// UUID primary key is auto-generated with Uuid::now_v7()
	if config.primary_key && is_uuid_type(&field.ty) {
		return true;
	}

	// Integer primary key is auto-generated by default (auto_increment behavior)
	// Unless explicitly disabled with auto_increment = false
	if config.primary_key && is_integer_primary_key_type(&field.ty) {
		// If auto_increment is explicitly set to false, include in required inputs.
		if config.auto_increment == Some(false) {
			return false;
		}
		// Otherwise, treat as auto-generated (default auto_increment behavior)
		return true;
	}

	false
}

/// Determine whether an auto-generated field should get an optional builder setter.
fn is_builder_optional_auto_field(field: &FieldInfo) -> bool {
	if !is_auto_generated_field(field) {
		return false;
	}
	if field.config.skip || field.is_fk_id_field {
		return false;
	}
	if is_many_to_many_field_type(&field.ty) || is_relationship_field_type(&field.ty) {
		return false;
	}
	if let Some(rel) = &field.rel
		&& matches!(rel.rel_type, crate::rel::RelationType::ManyToMany)
	{
		return false;
	}
	true
}

/// Get the default value expression for an auto-generated field
fn get_auto_field_default_value(field: &FieldInfo) -> TokenStream {
	let config = &field.config;

	// Fields with skip = true use Default::default()
	if config.skip {
		return quote! { ::std::default::Default::default() };
	}

	// ManyToManyField or ManyToMany relationship
	if is_many_to_many_field_type(&field.ty) {
		return quote! { ::std::default::Default::default() };
	}
	if let Some(rel) = &field.rel
		&& matches!(rel.rel_type, crate::rel::RelationType::ManyToMany)
	{
		return quote! { ::std::default::Default::default() };
	}

	// ForeignKeyField or OneToOneField - use Default::default()
	if is_relationship_field_type(&field.ty) {
		return quote! { ::std::default::Default::default() };
	}

	// Timestamp fields - use Utc::now() ONLY if the field type is DateTime<Utc>
	// This prevents type mismatches when fields named 'created_at' are of type i64
	if is_timestamp_field(field) && is_datetime_utc_type(&field.ty) {
		// Wrap with Some() for Option<DateTime<Utc>>
		if is_option_type(&field.ty) {
			return quote! { ::std::option::Option::Some(::chrono::Utc::now()) };
		}
		// Return as-is for DateTime<Utc>
		return quote! { ::chrono::Utc::now() };
	}

	// UUID primary key - generate new UUID
	if config.primary_key && is_uuid_type(&field.ty) {
		let (is_option, _) = extract_option_type(&field.ty);
		if is_option {
			return quote! { Some(::uuid::Uuid::now_v7()) };
		} else {
			return quote! { ::uuid::Uuid::now_v7() };
		}
	}

	// Integer primary key with auto-increment behavior - use 0 as placeholder
	// The actual value will be set by the database on INSERT
	if config.primary_key && is_integer_primary_key_type(&field.ty) {
		let (is_option, inner_ty) = extract_option_type(&field.ty);
		if is_option {
			return quote! { ::std::option::Option::None };
		} else {
			// Use 0 as the default value for integer primary keys
			// This will be replaced by the database-generated value on INSERT
			return quote! { 0 as #inner_ty };
		}
	}

	// Generated columns, IDENTITY, or auto-increment fields
	// These are set by the database, so use Default::default() (typically None for Option types)
	quote! { ::std::default::Default::default() }
}

/// Generate `new()` as a zero-arg alias of `build()` (#4401).
fn generate_new_alias(
	struct_name: &syn::Ident,
	field_infos: &[FieldInfo],
	fk_id_field_names: &[syn::Ident],
) -> TokenStream {
	let builder_name = syn::Ident::new(&format!("{}Builder", struct_name), struct_name.span());
	let unset_marker = syn::Ident::new(&format!("{}BuilderUnset", struct_name), struct_name.span());

	let user_fields: Vec<_> = field_infos
		.iter()
		.filter(|f| !is_auto_generated_field(f))
		.collect();

	let fk_id_to_fk_field: HashMap<String, String> = fk_id_field_names
		.iter()
		.filter_map(|id_name| {
			let id_str = id_name.to_string();
			if id_str.ends_with("_id") {
				let fk_name = id_str.trim_end_matches("_id").to_string();
				Some((id_str, fk_name))
			} else {
				None
			}
		})
		.collect();

	let fk_field_names: std::collections::HashSet<String> =
		fk_id_to_fk_field.values().cloned().collect();

	let slot_count = user_fields
		.iter()
		.filter(|f| !fk_field_names.contains(&f.name.to_string()))
		.count()
		+ fk_id_field_names
			.iter()
			.filter(|id_name| {
				let id_str = id_name.to_string();
				fk_id_to_fk_field.contains_key(&id_str)
			})
			.count();

	let unset_states: Vec<TokenStream> =
		(0..slot_count).map(|_| quote! { #unset_marker }).collect();
	let return_type = if unset_states.is_empty() {
		quote! { #builder_name }
	} else {
		quote! { #builder_name<#(#unset_states),*> }
	};

	quote! {
		impl #struct_name {
			/// Begin constructing a new instance via the typestate builder.
			///
			/// This is an alias of [`Self::build`]. Each required field is set via
			/// a named setter method, and `finish()` constructs the model.
			///
			/// The positional `Model::new(field1, field2, ...)` constructor was
			/// removed in 0.2.0. Use [`Self::build`] or this zero-argument alias.
			pub fn new() -> #return_type {
				Self::build()
			}
		}
	}
}

/// Generate the typestate `build()` builder for the model.
///
/// Adding a new required field to a model only adds a new builder setter —
/// every existing `build().setter().finish()` call site keeps compiling.
/// `new()` is a zero-arg alias of `build()`. See issues #4400 and #4401.
///
/// # Generated API
///
/// For a model with required fields `f1: T1`, `f2: T2`, …, `fN: TN` this
/// function emits:
///
/// - A marker pair `<StructName>BuilderSet` / `<StructName>BuilderUnset` to
///   track per-field set/unset state at the type level.
/// - A struct `<StructName>Builder<S1, …, SN>` that stores the so-far-supplied
///   values in `Option<Ti>` slots and carries `PhantomData<(S1, …, SN)>`.
/// - One `impl` block per required field that provides the setter, transitioning
///   that field's state from `Unset` to `Set` in the type parameter list.
/// - A single `impl <StructName>Builder<Set, …, Set>` block with `finish()` that
///   constructs `Self`.
/// - A `pub fn build() -> <StructName>Builder<Unset, …, Unset>` entry point on
///   the model.
///
/// FK setters accept any `IntoPrimaryKey<Related>` value — callers can pass
/// `&user` (the FK shortcut from #4398) or a raw primary-key value.
///
/// Auto-generated fields (`auto_now_add`, integer/UUID primary keys, identity
/// columns, generated columns, etc.) and fields with `include_in_new = false`
/// are optional builder inputs: omitting them uses the macro-managed default,
/// while calling their named setter stores the supplied value.
fn generate_build_function(
	struct_name: &syn::Ident,
	field_infos: &[FieldInfo],
	fk_id_field_names: &[syn::Ident],
) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();

	// Partition fields so the builder's finish() body can separate caller
	// inputs from macro-managed defaults.
	let user_fields: Vec<_> = field_infos
		.iter()
		.filter(|f| !is_auto_generated_field(f))
		.collect();

	let auto_fields: Vec<_> = field_infos
		.iter()
		.filter(|f| is_auto_generated_field(f))
		.collect();

	let optional_auto_fields: Vec<_> = auto_fields
		.iter()
		.copied()
		.filter(|f| is_builder_optional_auto_field(f))
		.collect();

	// Map of `*_id` (in field_infos / fk_id_field_names) -> related FK field name
	// (the model-typed field, e.g. `room_id` -> `room`).
	let fk_id_to_fk_field: HashMap<String, String> = fk_id_field_names
		.iter()
		.filter_map(|id_name| {
			let id_str = id_name.to_string();
			if id_str.ends_with("_id") {
				let fk_name = id_str.trim_end_matches("_id").to_string();
				Some((id_str, fk_name))
			} else {
				None
			}
		})
		.collect();

	// Classify each required (user-facing) field into one of three setter shapes.
	// `Type` is large (~240 bytes via `syn`), so the FK variant boxes it to keep
	// `SetterKind` compact and satisfy `clippy::large_enum_variant`.
	enum SetterKind {
		/// FK `*_id` field. Setter name is the related FK field (e.g. `author`)
		/// and accepts `impl IntoPrimaryKey<Related>`.
		ForeignKey {
			related_type: Box<Type>,
			setter_name: syn::Ident,
		},
		/// `String` field. Setter accepts `impl Into<String>` for ergonomics.
		String,
		/// Plain field. Setter accepts the exact declared type.
		Plain,
	}

	struct Required<'a> {
		/// The struct field name as stored in the model itself (e.g. `author_id`
		/// for FKs, `question_text` otherwise).
		storage_name: syn::Ident,
		/// The struct field type as stored in the model itself.
		storage_ty: &'a Type,
		/// Setter shape — controls the setter signature and finish() expression.
		kind: SetterKind,
	}

	let mut required: Vec<Required> =
		Vec::with_capacity(user_fields.len() + fk_id_field_names.len());
	for f in user_fields.iter() {
		let name_str = f.name.to_string();
		if let Some(fk_field_name) = fk_id_to_fk_field.get(&name_str) {
			// FK `*_id` field. Look up the related model type from the FK field.
			let fk_field_info = field_infos.iter().find(|fi| fi.name == *fk_field_name);
			let related_type = match fk_field_info {
				Some(info) => extract_foreign_key_target_type(&info.ty),
				// Defensive fallback: keep the stored type. This branch is not
				// expected because the builder mapping comes from the same field set.
				None => f.ty.clone(),
			};
			let setter_name = syn::Ident::new(fk_field_name, f.name.span());
			required.push(Required {
				storage_name: f.name.clone(),
				storage_ty: &f.ty,
				kind: SetterKind::ForeignKey {
					related_type: Box::new(related_type),
					setter_name,
				},
			});
		} else if is_string_type(&f.ty) && !extract_option_type(&f.ty).0 {
			required.push(Required {
				storage_name: f.name.clone(),
				storage_ty: &f.ty,
				kind: SetterKind::String,
			});
		} else {
			required.push(Required {
				storage_name: f.name.clone(),
				storage_ty: &f.ty,
				kind: SetterKind::Plain,
			});
		}
	}

	// FK `*_id` fields (e.g. `user_id`) are flagged as auto-generated by
	// `is_auto_generated_field` and therefore excluded from `user_fields`,
	// but they still need a user-facing setter on the builder so that callers
	// can supply the related model / primary key.
	for fk_id_name in fk_id_field_names.iter() {
		let fk_id_str = fk_id_name.to_string();
		// `fk_id_to_fk_field` only retains `*_id`-suffixed names (see its
		// construction above); names that don't follow the convention have no
		// implicit related-field name and are intentionally skipped.
		let Some(fk_field_name) = fk_id_to_fk_field.get(&fk_id_str) else {
			continue;
		};
		// `fk_id_field_names` is built from `field_infos`, so the lookup MUST
		// succeed; failure indicates an internal data-structure desync.
		let id_field_info = field_infos
			.iter()
			.find(|fi| fi.name == *fk_id_name)
			.unwrap_or_else(|| {
				panic!(
					"internal macro invariant: `{}` is in fk_id_field_names but missing from field_infos",
					fk_id_str
				)
			});
		let fk_field_info = field_infos.iter().find(|fi| fi.name == *fk_field_name);
		let related_type = match fk_field_info {
			Some(info) => extract_foreign_key_target_type(&info.ty),
			// Defensive fallback: use the `*_id` storage type itself.
			None => id_field_info.ty.clone(),
		};
		// Prefer reusing the existing FK field identifier (preserves raw-ident
		// spelling, hygiene, and span). Fall back to `Ident::new_raw` so the
		// proc-macro never panics if `fk_field_name` happens to be a Rust
		// keyword (e.g. `type`, `match`). Strip a leading `r#` defensively in
		// case the source identifier was a raw ident (`Ident::new_raw`
		// expects the bare name without the prefix). Reserved identifiers
		// that even `new_raw` rejects (`self`, `Self`, `super`, `crate`)
		// are surfaced as a clear macro error rather than the underlying
		// panic from `proc_macro2`. Note: `extern` is a keyword but IS
		// permitted as a raw identifier (`r#extern`), so it is excluded
		// from this set.
		let setter_name = match fk_field_info {
			Some(info) => info.name.clone(),
			None => {
				let bare = fk_field_name
					.strip_prefix("r#")
					.unwrap_or(fk_field_name.as_str());
				if matches!(bare, "self" | "Self" | "super" | "crate") {
					return syn::Error::new(
						fk_id_name.span(),
						format!(
							"cannot derive builder setter for FK field `{fk_id_str}`: \
							 the implied setter name `{bare}` is a reserved identifier; \
							 rename the related model-typed field or the `*_id` field"
						),
					)
					.to_compile_error();
				}
				syn::Ident::new_raw(bare, fk_id_name.span())
			}
		};
		required.push(Required {
			storage_name: id_field_info.name.clone(),
			storage_ty: &id_field_info.ty,
			kind: SetterKind::ForeignKey {
				related_type: Box::new(related_type),
				setter_name,
			},
		});
	}

	// Type names for the per-model builder + markers.
	let builder_name = syn::Ident::new(&format!("{}Builder", struct_name), struct_name.span());
	let set_marker = syn::Ident::new(&format!("{}BuilderSet", struct_name), struct_name.span());
	let unset_marker = syn::Ident::new(&format!("{}BuilderUnset", struct_name), struct_name.span());

	// Per-field type parameter idents `B0, B1, …` used in the builder's signature.
	let state_params: Vec<syn::Ident> = (0..required.len())
		.map(|i| syn::Ident::new(&format!("B{}", i), struct_name.span()))
		.collect();

	// Builder struct fields: one Option<StorageTy> per required field, one
	// Option<StorageTy> per optional auto-generated field, plus the PhantomData
	// state marker.
	let builder_struct_fields: Vec<TokenStream> = required
		.iter()
		.map(|r| {
			let name = &r.storage_name;
			let ty = r.storage_ty;
			quote! { #name: ::std::option::Option<#ty> }
		})
		.collect();
	let optional_builder_struct_fields: Vec<TokenStream> = optional_auto_fields
		.iter()
		.map(|f| {
			let name = &f.name;
			let ty = &f.ty;
			quote! { #name: ::std::option::Option<#ty> }
		})
		.collect();

	// `build()` initializer: every slot starts as `None`, every state slot as `Unset`.
	let init_struct_field_assignments: Vec<TokenStream> = required
		.iter()
		.map(|r| {
			let name = &r.storage_name;
			quote! { #name: ::std::option::Option::None }
		})
		.collect();
	let optional_init_struct_field_assignments: Vec<TokenStream> = optional_auto_fields
		.iter()
		.map(|f| {
			let name = &f.name;
			quote! { #name: ::std::option::Option::None }
		})
		.collect();

	// Per-field setter impl blocks. Each one transitions exactly one type slot
	// from `Unset` to `Set` while leaving the others polymorphic.
	let mut setter_impls: Vec<TokenStream> = Vec::with_capacity(required.len());
	for (idx, r) in required.iter().enumerate() {
		// Generic state parameters EXCLUDING the one being transitioned. The
		// transitioned slot is concretely `Unset` on the input and `Set` on the
		// output.
		let other_params: Vec<&syn::Ident> = state_params
			.iter()
			.enumerate()
			.filter_map(|(i, p)| if i == idx { None } else { Some(p) })
			.collect();

		// Input state list (this slot = Unset, others = generic).
		let input_states: Vec<TokenStream> = state_params
			.iter()
			.enumerate()
			.map(|(i, p)| {
				if i == idx {
					quote! { #unset_marker }
				} else {
					quote! { #p }
				}
			})
			.collect();

		// Output state list (this slot = Set, others = generic).
		let output_states: Vec<TokenStream> = state_params
			.iter()
			.enumerate()
			.map(|(i, p)| {
				if i == idx {
					quote! { #set_marker }
				} else {
					quote! { #p }
				}
			})
			.collect();

		// Field copy expressions for moving non-transitioned slots into the new
		// builder. The transitioned slot is replaced by the supplied value.
		let copy_fields: Vec<TokenStream> = required
			.iter()
			.enumerate()
			.map(|(i, other)| {
				let n = &other.storage_name;
				if i == idx {
					quote! {}
				} else {
					quote! { #n: self.#n, }
				}
			})
			.collect();
		let optional_copy_fields: Vec<TokenStream> = optional_auto_fields
			.iter()
			.map(|f| {
				let name = &f.name;
				quote! { #name: self.#name, }
			})
			.collect();

		let storage_name = &r.storage_name;
		let storage_ty = r.storage_ty;

		// Setter signature + body depend on the field kind.
		let (setter_sig, value_expr): (TokenStream, TokenStream) = match &r.kind {
			SetterKind::ForeignKey {
				related_type,
				setter_name,
			} => {
				// Setter named after the related FK field, accepting any
				// IntoPrimaryKey<Related>. This composes with #4398 — callers
				// can pass `&user` directly without manually extracting the PK.
				let sig = quote! {
					/// Set the foreign-key reference for this required field.
					///
					/// Accepts any `IntoPrimaryKey<Related>` — pass either the
					/// related model (e.g. `&user`) or a raw primary-key value.
					/// Transitions this slot from `Unset` to `Set` in the
					/// builder's type-state.
					pub fn #setter_name<__FkArg>(self, value: __FkArg)
						-> #builder_name<#(#output_states),*>
					where
						__FkArg: #orm_crate::IntoPrimaryKey<#related_type>,
				};
				let expr = quote! { value.into_primary_key() };
				(sig, expr)
			}
			SetterKind::String => {
				// Setter for `String` field, accepting `impl Into<String>`.
				let sig = quote! {
					/// Set this required `String` field.
					///
					/// Accepts any `impl Into<String>` (e.g. `&str`, `String`,
					/// `Cow<'_, str>`). Transitions this slot from `Unset` to
					/// `Set` in the builder's type-state.
					pub fn #storage_name<__StrArg>(self, value: __StrArg)
						-> #builder_name<#(#output_states),*>
					where
						__StrArg: ::std::convert::Into<::std::string::String>,
				};
				let expr = quote! { value.into() };
				(sig, expr)
			}
			SetterKind::Plain => {
				// Plain setter using the declared field type.
				let sig = quote! {
					/// Set this required field.
					///
					/// Transitions this slot from `Unset` to `Set` in the
					/// builder's type-state.
					pub fn #storage_name(self, value: #storage_ty)
						-> #builder_name<#(#output_states),*>
				};
				let expr = quote! { value };
				(sig, expr)
			}
		};

		let other_param_list = if other_params.is_empty() {
			quote! {}
		} else {
			quote! { <#(#other_params),*> }
		};

		setter_impls.push(quote! {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl #other_param_list #builder_name<#(#input_states),*> {
				#setter_sig
				{
					#builder_name {
						#(#copy_fields)*
						#(#optional_copy_fields)*
						#storage_name: ::std::option::Option::Some(#value_expr),
						__state: ::std::marker::PhantomData,
					}
				}
			}
		});
	}

	// finish() body: user fields -> FK `*_id` fields -> FK relation defaults
	// -> auto-generated fields.

	// FK id field names by raw string.
	let fk_id_field_names_set: std::collections::HashSet<String> =
		fk_id_to_fk_field.keys().cloned().collect();
	let fk_field_names: std::collections::HashSet<String> =
		fk_id_to_fk_field.values().cloned().collect();

	// User field assignments (non-FK regular fields). Pull from the builder's
	// `Option` slot — type-state guarantees `Some`.
	let user_field_assignments: Vec<TokenStream> = user_fields
		.iter()
		.filter(|f| {
			!fk_field_names.contains(&f.name.to_string())
				&& !fk_id_field_names_set.contains(&f.name.to_string())
		})
		.map(|f| {
			let name = &f.name;
			quote! {
				#name: self
					.#name
					.expect(concat!(
						"build() typestate guarantees ",
						stringify!(#name),
						" is set before finish() is callable"
					))
			}
		})
		.collect();

	// FK `*_id` assignments. The value was stored under the `*_id` name when
	// the user called the related-field setter.
	let fk_id_assignments: Vec<TokenStream> = fk_id_field_names
		.iter()
		.map(|fk_id_name| {
			let name = fk_id_name.clone();
			quote! {
				#name: self
					.#name
					.expect(concat!(
						"build() typestate guarantees ",
						stringify!(#name),
						" is set before finish() is callable"
					))
			}
		})
		.collect();

	// FK relation fields (the `ForeignKeyField<T>` themselves) are default-initialized.
	let fk_field_assignments: Vec<TokenStream> = fk_id_to_fk_field
		.values()
		.map(|fk_name_str| {
			let fk_name = syn::Ident::new(fk_name_str, proc_macro2::Span::call_site());
			quote! { #fk_name: ::std::default::Default::default() }
		})
		.collect();

	// Auto-generated fields (timestamps, UUID/integer PKs, identity, generated,
	// skipped, etc.) use the macro-managed default expressions.
	let auto_field_assignments: Vec<TokenStream> = auto_fields
		.iter()
		.filter(|f| {
			!fk_field_names.contains(&f.name.to_string())
				&& !fk_id_field_names_set.contains(&f.name.to_string())
		})
		.map(|f| {
			let name = &f.name;
			let default_value = get_auto_field_default_value(f);
			if is_builder_optional_auto_field(f) {
				quote! { #name: self.#name.unwrap_or_else(|| #default_value) }
			} else {
				quote! { #name: #default_value }
			}
		})
		.collect();

	// All-Set state list for the finish() impl bound.
	let all_set_states: Vec<TokenStream> = state_params
		.iter()
		.map(|_| quote! { #set_marker })
		.collect();

	// State parameter list for the builder struct + the initial-Unset list for
	// build().
	let state_param_list = if state_params.is_empty() {
		quote! {}
	} else {
		quote! { <#(#state_params),*> }
	};
	let optional_setter_impls: Vec<TokenStream> = optional_auto_fields
		.iter()
		.map(|f| {
			let name = &f.name;
			let ty = &f.ty;
			quote! {
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				impl #state_param_list #builder_name #state_param_list {
					/// Override this macro-managed field for this builder instance.
					///
					/// If this setter is not called, `finish()` uses the field's
					/// macro-managed default value.
					pub fn #name(mut self, value: #ty) -> Self {
						self.#name = ::std::option::Option::Some(value);
						self
					}
				}
			}
		})
		.collect();
	let initial_unset_states: Vec<TokenStream> = state_params
		.iter()
		.map(|_| quote! { #unset_marker })
		.collect();
	let initial_builder_type = if initial_unset_states.is_empty() {
		quote! { #builder_name }
	} else {
		quote! { #builder_name<#(#initial_unset_states),*> }
	};
	let all_set_builder_type = if all_set_states.is_empty() {
		quote! { #builder_name }
	} else {
		quote! { #builder_name<#(#all_set_states),*> }
	};

	// The PhantomData tuple type and field expression. Unit tuple (`()`) when
	// there are no required fields, so the model still gets a usable builder.
	let phantom_tuple_ty = if state_params.is_empty() {
		quote! { () }
	} else {
		quote! { ( #(#state_params,)* ) }
	};

	// Suppress dead_code warnings for builders generated for models that never
	// gain a required field — the markers and Option slots exist for type-state
	// shape consistency.
	let allow_dead = quote! { #[allow(dead_code)] };

	quote! {
		/// Type-state marker: the corresponding builder slot has been provided.
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#allow_dead
		pub struct #set_marker;

		/// Type-state marker: the corresponding builder slot is still missing.
		///
		/// `finish()` is only implemented when every slot is `#set_marker`, so
		/// calling `finish()` with any remaining `#unset_marker` slot is a
		/// compile error.
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#allow_dead
		pub struct #unset_marker;

		/// Typestate builder for [`#struct_name`] (issue #4400).
		///
		/// Construct via [`#struct_name::build`]; each required-field setter
		/// transitions exactly one `Unset` slot to `Set`. `finish()` is only
		/// available when every required slot is `Set`, so omitting a required
		/// field is a compile-time error.
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#allow_dead
		pub struct #builder_name #state_param_list {
			#(#builder_struct_fields,)*
			#(#optional_builder_struct_fields,)*
			__state: ::std::marker::PhantomData<#phantom_tuple_ty>,
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #struct_name {
			/// Begin constructing a [`#struct_name`] via the typestate builder.
			///
			/// Adding a new required field to this model becomes a non-breaking
			/// change for every caller that uses `build()` — the new field is
			/// surfaced as a new setter rather than a new positional parameter.
			///
			/// [`Self::new`] is a zero-argument alias for this method.
			pub fn build() -> #initial_builder_type {
				#builder_name {
					#(#init_struct_field_assignments,)*
					#(#optional_init_struct_field_assignments,)*
					__state: ::std::marker::PhantomData,
				}
			}
		}

		#(#setter_impls)*
		#(#optional_setter_impls)*

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #all_set_builder_type {
			/// Finalize the builder and construct the model instance.
			///
			/// Auto-generated fields (`auto_now_add` timestamps, UUID / integer
			/// primary keys, identity columns, generated columns, etc.) are
			/// initialized by their macro-managed defaults unless their optional
			/// builder setter supplied an explicit value.
			pub fn finish(self) -> #struct_name {
				#struct_name {
					#(#user_field_assignments,)*
					#(#fk_id_assignments,)*
					#(#fk_field_assignments,)*
					#(#auto_field_assignments,)*
				}
			}
		}
	}
}

/// Generate field selector struct
///
/// For type-safe JOIN/GROUP BY/HAVING operations, generates a field selector
/// struct (e.g., `UserFields`) corresponding to each model.
///
/// # Example
///
/// Generate `UserFields` struct for `User` model, enabling usage like:
///
/// ```ignore
/// QuerySet::<User>::new()
///     .inner_join_as::<User, _>("u1", "u2", |u1, u2| u1.id.lt(u2.id))
///     .group_by(|f| vec![f.user_id, f.category])
/// ```
fn generate_field_selector_struct(
	struct_name: &syn::Ident,
	field_infos: &[FieldInfo],
) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();

	// Exclude skip/FK/M2M/O2O fields (only normal DB columns)
	let regular_fields: Vec<_> = field_infos
		.iter()
		.filter(|f| {
			// Exclude fields marked with #[field(skip = true)]
			if f.config.skip {
				return false;
			}
			// FK _id fields are included (they are actual DB columns)
			// But exclude ForeignKeyField, OneToOneField, ManyToManyField (virtual fields)
			!is_foreign_key_field_type(&f.ty)
				&& !is_one_to_one_field_type(&f.ty)
				&& !is_many_to_many_field_type(&f.ty)
		})
		.collect();

	let field_selector_name =
		syn::Ident::new(&format!("{}Fields", struct_name), struct_name.span());

	// Generate field declarations
	let field_declarations: Vec<_> = regular_fields
		.iter()
		.map(|field| {
			let field_name = &field.name;
			let field_type = &field.ty;
			quote! {
				#field_name: #orm_crate::query_fields::Field<#struct_name, #field_type>
			}
		})
		.collect();

	// Generate field initialization
	let field_initializers: Vec<_> = regular_fields
		.iter()
		.map(|field| {
			let field_name = &field.name;
			let field_name_str = field_name.to_string();
			quote! {
				#field_name: #orm_crate::query_fields::Field::new(vec![#field_name_str])
			}
		})
		.collect();

	// List of field names (used in with_alias method)
	let regular_field_names: Vec<_> = regular_fields.iter().map(|field| &field.name).collect();

	quote! {
		/// Type-safe field selector for #struct_name
		///
		/// Provides type-safe field references in JOIN, GROUP BY, and HAVING clauses.
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#[derive(Debug, Clone)]
		pub struct #field_selector_name {
			#(#field_declarations),*
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #field_selector_name {
			/// Create a new field selector instance
			pub fn new() -> Self {
				Self {
					#(#field_initializers),*
				}
			}
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #orm_crate::FieldSelector for #field_selector_name {
			/// Set table alias for all fields
			///
			/// Used for self-joins where the same table appears multiple times
			/// with different aliases.
			///
			/// # Examples
			///
			/// ```ignore
			/// let u1 = UserFields::new().with_alias("u1");
			/// let u2 = UserFields::new().with_alias("u2");
			/// ```
			fn with_alias(mut self, alias: &str) -> Self {
				// Set alias for all fields
				#(self.#regular_field_names = self.#regular_field_names.with_alias(alias);)*
				self
			}
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl ::std::default::Default for #field_selector_name {
			fn default() -> Self {
				Self::new()
			}
		}
	}
}

#[derive(Clone)]
enum InfoSetterKind {
	Plain,
	String,
	Relation { target_ty: TokenStream },
	ManyToMany { target_ty: TokenStream },
}

#[derive(Clone)]
struct InfoFieldSpec {
	name: Ident,
	ty: TokenStream,
	serde_attrs: Vec<syn::Attribute>,
	validate_attrs: Vec<TokenStream>,
	setter_kind: InfoSetterKind,
	from_model: TokenStream,
}

fn is_plain_serde_skip_attr(attr: &syn::Attribute) -> bool {
	if !attr.path().is_ident("serde") {
		return false;
	}

	let syn::Meta::List(meta_list) = &attr.meta else {
		return false;
	};

	meta_list.tokens.to_string() == "skip"
}

fn relation_info_serde_attrs(field: &FieldInfo) -> Vec<syn::Attribute> {
	if !field.injected_relation_serde_skip {
		return field.serde_attrs.clone();
	}

	field
		.serde_attrs
		.iter()
		.filter(|attr| !is_plain_serde_skip_attr(attr))
		.cloned()
		.collect()
}

/// Generate the `{Model}Info` companion struct with `From` conversions (Issues #4194, #5272).
///
/// Relationship marker fields are represented with target-neutral lightweight
/// value types rather than ORM marker fields or flattened `*_id` fields.
fn generate_info_struct(
	struct_name: &Ident,
	generics: &syn::Generics,
	field_infos: &[FieldInfo],
	fk_field_infos: &[ForeignKeyFieldInfo],
	serde_serialize: bool,
	serde_deserialize: bool,
) -> Result<TokenStream> {
	let orm_crate = get_reinhardt_orm_crate();
	let reinhardt = get_reinhardt_crate();

	let info_name = Ident::new(&format!("{}Info", struct_name), struct_name.span());
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	let normalize_relation_ty = |ty: &Type| -> TokenStream {
		if matches!(ty, Type::Path(path) if path.path.is_ident("Self")) {
			quote! { #struct_name #ty_generics }
		} else {
			quote! { #ty }
		}
	};

	let fk_by_field_name: HashMap<String, &ForeignKeyFieldInfo> = fk_field_infos
		.iter()
		.map(|fk| (fk.field_name.to_string(), fk))
		.collect();
	let fk_id_to_field_name: HashMap<String, Ident> = fk_field_infos
		.iter()
		.map(|fk| (format!("{}_id", fk.field_name), fk.field_name.clone()))
		.collect();

	let mut info_fields = Vec::new();
	for f in field_infos {
		if f.config.skip || f.config.skip_info || f.is_fk_id_field {
			continue;
		}

		let name = f.name.clone();
		if let Some(fk) = fk_by_field_name.get(&name.to_string()) {
			let id_name = Ident::new(&format!("{}_id", name), name.span());
			let target_ty = normalize_relation_ty(&fk.target_type);
			let ty = quote! { #reinhardt::model_info::RelationInfo<#target_ty> };
			info_fields.push(InfoFieldSpec {
				name: name.clone(),
				ty,
				serde_attrs: relation_info_serde_attrs(f),
				validate_attrs: Vec::new(),
				setter_kind: InfoSetterKind::Relation { target_ty },
				from_model: quote! {
					#name: #reinhardt::model_info::RelationInfo::new(model.#id_name),
				},
			});
			continue;
		}

		if is_many_to_many_field_type(&f.ty) {
			let Some(target_ty) = extract_m2m_target_type(&f.ty) else {
				return Err(syn::Error::new_spanned(
					&f.ty,
					"ManyToManyField must specify source and target model types",
				));
			};
			let target_ty = normalize_relation_ty(target_ty);
			let source_ty = quote! { #struct_name #ty_generics };
			let ty = quote! { #reinhardt::model_info::ManyToManyInfo<#source_ty, #target_ty> };
			info_fields.push(InfoFieldSpec {
				name: name.clone(),
				ty,
				serde_attrs: relation_info_serde_attrs(f),
				validate_attrs: Vec::new(),
				setter_kind: InfoSetterKind::ManyToMany { target_ty },
				from_model: quote! {
					#name: #reinhardt::model_info::ManyToManyInfo::empty(),
				},
			});
			continue;
		}

		if is_relationship_field_type(&f.ty) {
			continue;
		}

		let ty = &f.ty;
		let validate_attrs = generate_validate_attrs(&f.config);
		let (is_option, _) = extract_option_type(ty);
		let setter_kind = if is_string_type(ty) && !is_option {
			InfoSetterKind::String
		} else {
			InfoSetterKind::Plain
		};
		info_fields.push(InfoFieldSpec {
			name: name.clone(),
			ty: quote! { #ty },
			serde_attrs: f.serde_attrs.clone(),
			validate_attrs,
			setter_kind,
			from_model: quote! { #name: model.#name, },
		});
	}

	// Generate Info struct fields with optional validate attributes
	let info_field_defs: Vec<TokenStream> = info_fields
		.iter()
		.map(|f| {
			let name = &f.name;
			let ty = &f.ty;
			let serde_attrs = &f.serde_attrs;
			let validate_attrs = &f.validate_attrs;
			quote! {
				#(#serde_attrs)*
				#(#validate_attrs)*
				pub #name: #ty,
			}
		})
		.collect();

	// Propagate serde derives detected by the attribute macro via model_config flags.
	// Derive macros cannot see #[derive()] attributes (stripped by rustc), so the
	// attribute macro detects them and passes serde_serialize/serde_deserialize bare
	// flags through #[model_config(...)].
	let mut extra_derives = Vec::new();
	if serde_serialize {
		extra_derives.push(quote!(serde::Serialize));
	}
	if serde_deserialize {
		extra_derives.push(quote!(serde::Deserialize));
	}

	let extra_derives_tokens = if extra_derives.is_empty() {
		quote! {}
	} else {
		quote! { , #(#extra_derives),* }
	};

	// Conditionally add Validate derive if any field has validation. OpenAPI
	// Schema remains explicit so non-OpenAPI REST users do not pull the OpenAPI
	// feature graph through generated companion structs.
	let has_any_validation = info_fields.iter().any(|f| !f.validate_attrs.is_empty());

	let validate_derive = if has_any_validation {
		quote! {
			#[cfg_attr(native, derive(#reinhardt::Validate))]
		}
	} else {
		quote! {}
	};

	// Generate From<Model> for Info
	let model_to_info_fields: Vec<TokenStream> =
		info_fields.iter().map(|f| f.from_model.clone()).collect();

	// Generate From<Info> for Model — all model fields, with defaults for excluded ones
	let info_to_model_fields: Vec<TokenStream> = field_infos
		.iter()
		.map(|f| {
			let name = &f.name;
			let name_str = name.to_string();
			if let Some(relation_name) = fk_id_to_field_name.get(&name_str)
				&& info_fields.iter().any(|inf| inf.name == *relation_name)
			{
				quote! { #name: info.#relation_name.into_id(), }
			} else if info_fields.iter().any(|inf| inf.name == f.name)
				&& !is_relationship_field_type(&f.ty)
				&& !is_many_to_many_field_type(&f.ty)
			{
				quote! { #name: info.#name, }
			} else {
				quote! { #name: ::std::default::Default::default(), }
			}
		})
		.collect();

	let info_builder =
		generate_info_builder(&info_name, generics, &info_fields, &orm_crate, &reinhardt)?;

	let info_doc = format!("Data-transfer companion for [`{}`].", struct_name);

	Ok(quote! {
		#[doc = #info_doc]
		#[allow(missing_docs)]
		#[derive(Debug, Clone, PartialEq #extra_derives_tokens)]
		#validate_derive
		pub struct #info_name #impl_generics #where_clause {
			#(#info_field_defs)*
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #impl_generics ::std::convert::From<#struct_name #ty_generics> for #info_name #ty_generics #where_clause {
			fn from(model: #struct_name #ty_generics) -> Self {
				Self {
					#(#model_to_info_fields)*
				}
			}
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #impl_generics ::std::convert::From<#info_name #ty_generics> for #struct_name #ty_generics #where_clause {
			fn from(info: #info_name #ty_generics) -> Self {
				Self {
					#(#info_to_model_fields)*
				}
			}
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#info_builder
	})
}

/// Generate `#[validate(...)]` attributes from `FieldConfig` metadata.
fn generate_validate_attrs(config: &FieldConfig) -> Vec<TokenStream> {
	let mut attrs = Vec::new();

	// Length validation: combine min_length and max_length
	let has_length = config.min_length.is_some() || config.max_length.is_some();
	if has_length {
		let mut parts = Vec::new();
		if let Some(min) = config.min_length {
			parts.push(quote!(min = #min));
		}
		if let Some(max) = config.max_length {
			parts.push(quote!(max = #max));
		}
		attrs.push(quote! {
			#[cfg_attr(native, validate(length(#(#parts),*)))]
		});
	}

	// Email validation
	if config.email == Some(true) {
		attrs.push(quote! {
			#[cfg_attr(native, validate(email))]
		});
	}

	// URL validation
	if config.url == Some(true) {
		attrs.push(quote! {
			#[cfg_attr(native, validate(url))]
		});
	}

	// Range validation: combine min_value and max_value
	let has_range = config.min_value.is_some() || config.max_value.is_some();
	if has_range {
		let mut parts = Vec::new();
		if let Some(min) = config.min_value {
			parts.push(quote!(min = #min));
		}
		if let Some(max) = config.max_value {
			parts.push(quote!(max = #max));
		}
		attrs.push(quote! {
			#[cfg_attr(native, validate(range(#(#parts),*)))]
		});
	}

	attrs
}

/// Generate a typestate builder for `{Model}Info`.
fn generate_info_builder(
	info_name: &Ident,
	generics: &syn::Generics,
	info_fields: &[InfoFieldSpec],
	orm_crate: &TokenStream,
	reinhardt: &TokenStream,
) -> Result<TokenStream> {
	let builder_name = Ident::new(&format!("{}Builder", info_name), info_name.span());
	let (_impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	// State marker types: one per required field
	let state_names: Vec<Ident> = info_fields
		.iter()
		.enumerate()
		.map(|(i, _)| Ident::new(&format!("__S{}", i), info_name.span()))
		.collect();

	let field_names: Vec<&Ident> = info_fields.iter().map(|f| &f.name).collect();

	// Marker types
	let unset_marker = quote!(());
	let set_marker = quote!(((),));

	// Initial state: all unset
	let initial_states: Vec<TokenStream> = state_names.iter().map(|_| quote!(())).collect();

	// Final state: all set
	let final_states: Vec<TokenStream> = state_names.iter().map(|_| quote!(((),))).collect();

	// Builder struct fields: Option<T> for each field
	let builder_fields: Vec<TokenStream> = info_fields
		.iter()
		.zip(state_names.iter())
		.map(|(f, _)| {
			let name = &f.name;
			let ty = &f.ty;
			quote! { #name: ::std::option::Option<#ty>, }
		})
		.collect();

	// Generate setter methods
	let setter_methods: Vec<TokenStream> = info_fields
		.iter()
		.enumerate()
		.map(|(idx, f)| {
			let name = &f.name;
			let ty = &f.ty;

			// States for this setter: all same except idx goes unset→set
			let input_states: Vec<TokenStream> = state_names
				.iter()
				.enumerate()
				.map(|(i, s)| {
					if i == idx {
						quote!(#unset_marker)
					} else {
						quote!(#s)
					}
				})
				.collect();

			let output_states: Vec<TokenStream> = state_names
				.iter()
				.enumerate()
				.map(|(i, s)| {
					if i == idx {
						quote!(#set_marker)
					} else {
						quote!(#s)
					}
				})
				.collect();

			// Only include state params that are NOT pinned (exclude the one at idx)
			let free_state_params: Vec<&Ident> = state_names
				.iter()
				.enumerate()
				.filter_map(|(i, s)| if i != idx { Some(s) } else { None })
				.collect();

			match &f.setter_kind {
				InfoSetterKind::Relation { target_ty } => {
					quote! {
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						#[allow(missing_docs)]
						impl<#(#free_state_params),*> #builder_name<#(#input_states),*> {
							pub fn #name<__FkArg>(mut self, value: __FkArg)
								-> #builder_name<#(#output_states),*>
							where
								__FkArg: #orm_crate::IntoPrimaryKey<#target_ty>,
							{
								self.#name = ::std::option::Option::Some(
									#reinhardt::model_info::RelationInfo::new(value.into_primary_key())
								);
								#builder_name {
									#(#field_names: self.#field_names,)*
									_state: ::std::marker::PhantomData,
								}
							}
						}
					}
				}
				InfoSetterKind::ManyToMany { target_ty } => {
					quote! {
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						#[allow(missing_docs)]
						impl<#(#free_state_params),*> #builder_name<#(#input_states),*> {
							pub fn #name<__Ids>(mut self, value: __Ids)
								-> #builder_name<#(#output_states),*>
							where
								__Ids: ::std::iter::IntoIterator<
									Item = <#target_ty as #reinhardt::model_info::InfoModel>::PrimaryKey
								>,
							{
								self.#name = ::std::option::Option::Some(
									#reinhardt::model_info::ManyToManyInfo::new(value)
								);
								#builder_name {
									#(#field_names: self.#field_names,)*
									_state: ::std::marker::PhantomData,
								}
							}
						}
					}
				}
				InfoSetterKind::String => {
					quote! {
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						#[allow(missing_docs)]
						impl<#(#free_state_params),*> #builder_name<#(#input_states),*> {
							pub fn #name(mut self, value: impl ::std::convert::Into<String>)
								-> #builder_name<#(#output_states),*>
							{
								self.#name = ::std::option::Option::Some(value.into());
								#builder_name {
									#(#field_names: self.#field_names,)*
									_state: ::std::marker::PhantomData,
								}
							}
						}
					}
				}
				InfoSetterKind::Plain => {
					quote! {
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						#[allow(missing_docs)]
						impl<#(#free_state_params),*> #builder_name<#(#input_states),*> {
							pub fn #name(mut self, value: #ty)
								-> #builder_name<#(#output_states),*>
							{
								self.#name = ::std::option::Option::Some(value);
								#builder_name {
									#(#field_names: self.#field_names,)*
									_state: ::std::marker::PhantomData,
								}
							}
						}
					}
				}
			}
		})
		.collect();

	Ok(quote! {
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#[allow(missing_docs)]
		pub struct #builder_name<#(#state_names = ()),*> {
			#(#builder_fields)*
			_state: ::std::marker::PhantomData<(#(#state_names),*)>,
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#[allow(missing_docs)]
		impl #info_name #ty_generics #where_clause {
			pub fn build() -> #builder_name<#(#initial_states),*> {
				#builder_name {
					#(#field_names: ::std::option::Option::None,)*
					_state: ::std::marker::PhantomData,
				}
			}
		}

		#(#setter_methods)*

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#[allow(missing_docs)]
		impl #builder_name<#(#final_states),*> {
			pub fn finish(self) -> #info_name #ty_generics #where_clause {
				#info_name {
					#(#field_names: self.#field_names.unwrap(),)*
				}
			}
		}
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_fields_are_private() {
		let input = quote! {
			#[model(app_label = "test", table_name = "test", info = false)]
			pub struct TestModel {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(max_length = 255)]
				pub name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		// Verify that fields are not pub
		assert!(!output_str.contains("pub id"));
		assert!(!output_str.contains("pub name"));
	}

	#[test]
	fn test_getter_methods_generated() {
		let input = quote! {
			#[model(app_label = "test", table_name = "test")]
			pub struct TestModel {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(max_length = 255)]
				pub name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		// Verify that getter methods are generated
		assert!(output_str.contains("pub fn id"));
		assert!(output_str.contains("pub fn name"));
	}

	#[test]
	fn test_setter_methods_exclude_auto_fields() {
		let input = quote! {
			#[model(app_label = "test", table_name = "test")]
			pub struct TestModel {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(max_length = 255)]
				pub name: String,
				#[field(auto_now_add = true)]
				pub created_at: DateTime<Utc>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		// Setter for name is generated
		assert!(output_str.contains("pub fn set_name"));

		// Setters for id and created_at are not generated
		assert!(!output_str.contains("pub fn set_id"));
		assert!(!output_str.contains("pub fn set_created_at"));
	}

	#[test]
	fn test_new_is_zero_arg_builder_alias() {
		let input = quote! {
			#[model(app_label = "test", table_name = "test")]
			pub struct TestModel {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(max_length = 255)]
				pub name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		assert!(output_str.contains("pub fn new () -> TestModelBuilder < TestModelBuilderUnset >"));
		assert!(output_str.contains("Self :: build ()"));
		assert!(!output_str.contains("pub fn new <"));
		assert!(!output_str.contains("pub fn new (name"));
	}

	#[test]
	fn test_new_alias_uses_plain_builder_type_when_no_fields_are_required() {
		let input = quote! {
			#[model(app_label = "test", table_name = "test")]
			pub struct EmptyRequiredModel {
				#[field(primary_key = true)]
				pub id: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		assert!(output_str.contains("pub fn new () -> EmptyRequiredModelBuilder"));
		assert!(output_str.contains("pub fn build () -> EmptyRequiredModelBuilder"));
		assert!(!output_str.contains("EmptyRequiredModelBuilder < >"));
	}

	#[test]
	fn test_builder_optional_auto_field_setters_do_not_affect_typestate() {
		let input = quote! {
			#[model(app_label = "test", table_name = "test")]
			pub struct TestModel {
				#[field(primary_key = true, include_in_new = false)]
				pub id: Uuid,
				#[field(max_length = 255)]
				pub name: String,
				#[field(auto_now_add = true)]
				pub created_at: DateTime<Utc>,
				#[field(include_in_new = false)]
				pub external_state: i32,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		assert!(output_str.contains("id : :: std :: option :: Option < Uuid >"));
		assert!(output_str.contains("pub fn id (mut self , value : Uuid) -> Self"));
		assert!(
			output_str.contains("pub fn created_at (mut self , value : DateTime < Utc >) -> Self")
		);
		assert!(output_str.contains("pub fn external_state (mut self , value : i32) -> Self"));
		assert!(
			output_str
				.contains("id : self . id . unwrap_or_else (|| :: uuid :: Uuid :: now_v7 ())")
		);
		assert!(output_str.contains(
			"created_at : self . created_at . unwrap_or_else (|| :: chrono :: Utc :: now ())"
		));
		assert!(output_str.contains(
			"external_state : self . external_state . unwrap_or_else (|| :: std :: default :: Default :: default ())"
		));

		assert!(
			output_str.contains("pub fn build () -> TestModelBuilder < TestModelBuilderUnset >")
		);
		assert!(
			!output_str
				.contains("TestModelBuilder < TestModelBuilderUnset , TestModelBuilderUnset")
		);
		assert!(!output_str.contains("pub fn set_id"));
		assert!(!output_str.contains("pub fn set_created_at"));
	}
}
