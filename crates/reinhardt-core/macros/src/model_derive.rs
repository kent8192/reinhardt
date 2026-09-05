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

use std::collections::{HashMap, HashSet};

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::Token;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Result, Type, parse_quote};
use syn::{Ident, LitBool, LitStr, bracketed, parenthesized};

use crate::crate_paths::{
	get_linkme_crate, get_reinhardt_apps_crate, get_reinhardt_core_crate, get_reinhardt_crate,
	get_reinhardt_db_crate, get_reinhardt_forms_crate, get_reinhardt_migrations_crate,
	get_reinhardt_orm_crate, get_serde_crate, get_serde_json_crate,
};
use crate::identifier_case::to_snake_case;
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

struct UniqueConstraintMetadata {
	logical_fields: Vec<String>,
	column_names: Vec<String>,
	name: Option<String>,
	condition: Option<String>,
}

/// Parsed model attributes (intermediate representation)
struct ModelAttributesParsed {
	app_label: Option<String>,
	table_name: Option<String>,
	get_latest_by: Option<Vec<String>>,
	constraints: Option<Vec<ConstraintSpec>>,
	unique_together: Vec<Vec<String>>, // Multiple Django-style unique_together constraints
	/// Optional custom manager path: `manager = MyManager` (Issue #3980).
	manager: Option<syn::Path>,
	/// Whether to generate Info companion struct (Issue #4194).
	/// `None` means not specified (defaults to `true` in `ModelConfig`).
	info: Option<bool>,
	/// Whether this model is only available on the native/server side.
	server_only: bool,
	/// Whether to generate target-neutral model-form schema and payload types.
	form: bool,
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
fn contains_blocked_sql_keyword(sql: &str, keyword: &str) -> bool {
	let mut token = String::new();
	let mut quote = None;
	let mut chars = sql.chars().peekable();

	while let Some(character) = chars.next() {
		if let Some(delimiter) = quote {
			if character == delimiter {
				if chars.peek().copied() == Some(delimiter) {
					chars.next();
				} else {
					quote = None;
				}
			}
			continue;
		}

		if character == '\'' || character == '"' {
			if token == keyword {
				return true;
			}
			token.clear();
			quote = Some(character);
		} else if character.is_ascii_alphanumeric() || character == '_' {
			token.push(character.to_ascii_uppercase());
		} else {
			if token == keyword {
				return true;
			}
			token.clear();
		}
	}

	token == keyword
}

fn validate_sql_expression(sql: &str, attr_name: &str) -> Result<()> {
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
		"DROP", "DELETE", "INSERT", "UPDATE", "ALTER", "TRUNCATE", "EXEC", "EXECUTE", "CREATE",
		"GRANT", "REVOKE",
	];
	for keyword in BLOCKED_KEYWORDS {
		if contains_blocked_sql_keyword(sql, keyword) {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				format!(
					"Dangerous SQL keyword {:?} detected in {} expression: {:?}",
					keyword, attr_name, sql
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

fn validate_generated_schema_expr(expr: &syn::Expr) -> Result<()> {
	if generated_schema_expr_is_supported(expr) {
		return Ok(());
	}

	Err(syn::Error::new_spanned(
		expr,
		"generated expects a reconstructable SchemaExpr expression; supported forms are SchemaExpr::col(...), SchemaExpr::val(...), SchemaExpr::concat([...]), SchemaExpr::coalesce([...]), and chained .binary(...) or .cast(...) calls. Use generated_sql = \"...\" for raw SQL or unsupported expression builders.",
	))
}

fn generated_schema_expr_is_supported(expr: &syn::Expr) -> bool {
	match expr {
		syn::Expr::Paren(expr_paren) => generated_schema_expr_is_supported(&expr_paren.expr),
		syn::Expr::Group(expr_group) => generated_schema_expr_is_supported(&expr_group.expr),
		syn::Expr::MethodCall(method_call) => {
			generated_schema_expr_is_supported(&method_call.receiver)
				&& match method_call.method.to_string().as_str() {
					"binary" if method_call.args.len() == 2 => {
						generated_schema_bin_oper_is_supported(&method_call.args[0])
							&& generated_schema_expr_is_supported(&method_call.args[1])
					}
					"cast" if method_call.args.len() == 1 => {
						generated_column_type_is_supported(&method_call.args[0])
					}
					_ => false,
				}
		}
		syn::Expr::Call(expr_call) => {
			let syn::Expr::Path(func_path) = &*expr_call.func else {
				return false;
			};
			let Some(func) = func_path
				.path
				.segments
				.last()
				.map(|segment| segment.ident.to_string())
			else {
				return false;
			};
			match func.as_str() {
				"col" if expr_call.args.len() == 1 => {
					matches!(&expr_call.args[0], syn::Expr::Lit(expr_lit) if matches!(&expr_lit.lit, syn::Lit::Str(_)))
				}
				"val" if expr_call.args.len() == 1 => {
					generated_schema_value_is_supported(&expr_call.args[0])
				}
				"concat" if expr_call.args.len() == 1 => {
					generated_schema_expr_list_is_supported(&expr_call.args[0], true)
				}
				"coalesce" if expr_call.args.len() == 1 => {
					generated_schema_expr_list_is_supported(&expr_call.args[0], false)
				}
				_ => false,
			}
		}
		_ => false,
	}
}

fn generated_schema_expr_list_is_supported(expr: &syn::Expr, allow_empty: bool) -> bool {
	match expr {
		syn::Expr::Array(expr_array) => {
			(allow_empty || !expr_array.elems.is_empty())
				&& expr_array
					.elems
					.iter()
					.all(generated_schema_expr_is_supported)
		}
		syn::Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			let Ok(parsed) = syn::parse2::<syn::ExprArray>(quote! { [#tokens] }) else {
				return false;
			};
			(allow_empty || !parsed.elems.is_empty())
				&& parsed.elems.iter().all(generated_schema_expr_is_supported)
		}
		_ => false,
	}
}

fn generated_schema_bin_oper_is_supported(expr: &syn::Expr) -> bool {
	let syn::Expr::Path(expr_path) = expr else {
		return false;
	};
	expr_path.path.segments.last().is_some_and(|segment| {
		matches!(
			segment.ident.to_string().as_str(),
			"Add" | "Sub" | "Mul" | "Div"
		)
	})
}

fn generated_schema_value_is_supported(expr: &syn::Expr) -> bool {
	match expr {
		syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
			syn::Lit::Str(_) | syn::Lit::Bool(_) | syn::Lit::Char(_) => true,
			syn::Lit::Int(lit_int) => {
				lit_int.base10_parse::<i32>().is_ok() || lit_int.base10_parse::<i64>().is_ok()
			}
			syn::Lit::Float(lit_float) => lit_float.base10_parse::<f64>().is_ok(),
			_ => false,
		},
		syn::Expr::Unary(expr_unary) => {
			matches!(expr_unary.op, syn::UnOp::Neg(_))
				&& generated_schema_value_is_supported(&expr_unary.expr)
		}
		_ => false,
	}
}

fn generated_column_type_is_supported(expr: &syn::Expr) -> bool {
	match expr {
		syn::Expr::Path(expr_path) => expr_path.path.segments.last().is_some_and(|segment| {
			matches!(
				segment.ident.to_string().as_str(),
				"Text"
					| "TinyInteger" | "SmallInteger"
					| "Integer" | "BigInteger"
					| "Float" | "Double"
					| "Boolean" | "Date"
					| "Time" | "DateTime"
					| "Timestamp" | "TimestampWithTimeZone"
					| "Blob" | "Uuid"
					| "Json" | "Jsonb"
			)
		}),
		syn::Expr::Call(expr_call) => {
			let syn::Expr::Path(func_path) = &*expr_call.func else {
				return false;
			};
			let Some(variant) = func_path
				.path
				.segments
				.last()
				.map(|segment| segment.ident.to_string())
			else {
				return false;
			};
			match variant.as_str() {
				"Char" | "String" | "Binary" if expr_call.args.len() == 1 => {
					generated_optional_u32_is_supported(&expr_call.args[0])
				}
				"Decimal" if expr_call.args.len() == 1 => {
					generated_optional_u32_pair_is_supported(&expr_call.args[0])
				}
				"VarBinary" if expr_call.args.len() == 1 => {
					generated_u32_literal_is_supported(&expr_call.args[0])
				}
				"Array" if expr_call.args.len() == 1 => generated_box_new_expr(&expr_call.args[0])
					.is_some_and(generated_column_type_is_supported),
				"Custom" if expr_call.args.len() == 1 => {
					generated_custom_column_type_arg_is_supported(&expr_call.args[0])
				}
				_ => false,
			}
		}
		_ => false,
	}
}

fn generated_custom_column_type_arg_is_supported(expr: &syn::Expr) -> bool {
	if matches!(expr, syn::Expr::Lit(expr_lit) if matches!(&expr_lit.lit, syn::Lit::Str(_))) {
		return true;
	}

	let syn::Expr::MethodCall(method_call) = expr else {
		return false;
	};
	method_call.method == "to_string"
		&& method_call.args.is_empty()
		&& matches!(&*method_call.receiver, syn::Expr::Lit(expr_lit) if matches!(&expr_lit.lit, syn::Lit::Str(_)))
}

fn generated_box_new_expr(expr: &syn::Expr) -> Option<&syn::Expr> {
	let syn::Expr::Call(expr_call) = expr else {
		return None;
	};
	let syn::Expr::Path(func_path) = &*expr_call.func else {
		return None;
	};
	if func_path
		.path
		.segments
		.last()
		.is_some_and(|segment| segment.ident == "new")
		&& func_path
			.path
			.segments
			.iter()
			.any(|segment| segment.ident == "Box")
		&& expr_call.args.len() == 1
	{
		return Some(&expr_call.args[0]);
	}
	None
}

fn generated_optional_u32_is_supported(expr: &syn::Expr) -> bool {
	if let syn::Expr::Path(expr_path) = expr
		&& expr_path.path.is_ident("None")
	{
		return true;
	}
	if let syn::Expr::Call(expr_call) = expr
		&& let syn::Expr::Path(func_path) = &*expr_call.func
		&& func_path.path.is_ident("Some")
		&& expr_call.args.len() == 1
	{
		return generated_u32_literal_is_supported(&expr_call.args[0]);
	}
	false
}

fn generated_optional_u32_pair_is_supported(expr: &syn::Expr) -> bool {
	if let syn::Expr::Path(expr_path) = expr
		&& expr_path.path.is_ident("None")
	{
		return true;
	}
	if let syn::Expr::Call(expr_call) = expr
		&& let syn::Expr::Path(func_path) = &*expr_call.func
		&& func_path.path.is_ident("Some")
		&& expr_call.args.len() == 1
		&& let syn::Expr::Tuple(tuple) = &expr_call.args[0]
		&& tuple.elems.len() == 2
	{
		return tuple.elems.iter().all(generated_u32_literal_is_supported);
	}
	false
}

fn generated_u32_literal_is_supported(expr: &syn::Expr) -> bool {
	let syn::Expr::Lit(expr_lit) = expr else {
		return false;
	};
	let syn::Lit::Int(lit_int) = &expr_lit.lit else {
		return false;
	};
	lit_int.base10_parse::<u32>().is_ok()
}

/// Model configuration from `#[model(...)]` attribute
#[derive(Debug, Clone)]
struct ModelConfig {
	app_label: String,
	table_name: String,
	get_latest_by: Option<Vec<String>>,
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
	/// Whether to generate target-neutral model-form schema and payload types.
	form: bool,
	/// Whether the original model derives `serde::Serialize`.
	serde_serialize: bool,
	/// Whether the original model derives `serde::Deserialize`.
	serde_deserialize: bool,
}

/// Model-form configuration from struct-level `#[form(...)]` attributes.
#[derive(Debug, Clone, Default)]
struct ModelFormConfig {
	validate: Option<syn::Path>,
}

impl ModelFormConfig {
	fn from_attrs(attrs: &[syn::Attribute]) -> Result<Self> {
		let mut config = Self::default();

		for attr in attrs {
			if !attr.path().is_ident("form") {
				continue;
			}

			attr.parse_nested_meta(|meta| {
				if !meta.path.is_ident("validate") {
					return Err(meta.error("unknown model form option; expected `validate`"));
				}
				if config.validate.is_some() {
					return Err(meta.error("duplicate `validate` model form option"));
				}
				config.validate = Some(meta.value()?.parse()?);
				Ok(())
			})?;
		}

		Ok(config)
	}
}

impl ModelConfig {
	/// Parse `#[model(...)]` attribute
	fn from_attrs(attrs: &[syn::Attribute], struct_name: &syn::Ident) -> Result<Self> {
		let mut app_label = None;
		let mut table_name = None;
		let mut get_latest_by = None;
		let mut constraints = Vec::new();
		let mut manager: Option<syn::Path> = None;
		let mut info: Option<bool> = None;
		let mut server_only = false;
		let mut form = false;
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
			if let Some(fields) = model_attr.get_latest_by {
				get_latest_by = Some(fields);
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
			if model_attr.form {
				form = true;
			}
			if model_attr.serde_serialize {
				serde_serialize = true;
			}
			if model_attr.serde_deserialize {
				serde_deserialize = true;
			}
		}

		let app_label = app_label.ok_or_else(|| {
			syn::Error::new_spanned(
				struct_name,
				"app_label attribute is required in #[model(...)]",
			)
		})?;
		let table_name = table_name.unwrap_or_else(|| {
			format!("{}_{}", app_label, to_snake_case(&struct_name.to_string()))
		});

		Ok(Self {
			app_label,
			table_name,
			get_latest_by,
			constraints,
			manager,
			info: info.unwrap_or(true),
			server_only,
			form,
			serde_serialize,
			serde_deserialize,
		})
	}

	/// Parse all model attributes using custom parser
	fn parse_model_attributes(input: syn::parse::ParseStream) -> Result<ModelAttributesParsed> {
		use syn::Token;

		let mut app_label = None;
		let mut table_name = None;
		let mut get_latest_by = None;
		let mut constraints = None;
		let mut unique_together = Vec::new();
		let mut manager: Option<syn::Path> = None;
		let mut info: Option<bool> = None;
		let mut server_only = false;
		let mut form = false;
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
			} else if ident == "get_latest_by" {
				let content;
				parenthesized!(content in input);
				let fields: Punctuated<LitStr, Token![,]> =
					content.call(Punctuated::parse_terminated)?;
				get_latest_by = Some(fields.iter().map(LitStr::value).collect());
			} else if ident == "manager" {
				// Custom object manager type: `manager = MyManager` (Issue #3980).
				let path: syn::Path = input.parse()?;
				manager = Some(path);
			} else if ident == "info" {
				let value: LitBool = input.parse()?;
				info = Some(value.value());
			} else if ident == "form" {
				let value: LitBool = input.parse()?;
				form = value.value();
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
			get_latest_by,
			constraints,
			unique_together,
			manager,
			info,
			server_only,
			form,
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
	/// Bare model name: `#[field(foreign_key = "User")]`
	ModelName(String),
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

#[derive(Debug, Clone, Copy)]
enum StructuredIndexMethod {
	Hnsw,
	Ivfflat,
}

#[derive(Debug, Clone)]
struct StructuredIndexConfig {
	name: String,
	name_span: Span,
	method: StructuredIndexMethod,
	opclass: String,
	m: Option<u16>,
	ef_construction: Option<u16>,
	lists: Option<u32>,
}

fn optional_u16_tokens(value: Option<u16>) -> TokenStream {
	match value {
		Some(value) => quote! { Some(#value) },
		None => quote! { None },
	}
}

fn optional_u32_tokens(value: Option<u32>) -> TokenStream {
	match value {
		Some(value) => quote! { Some(#value) },
		None => quote! { None },
	}
}

impl StructuredIndexConfig {
	fn parse(meta: syn::meta::ParseNestedMeta<'_>) -> Result<Self> {
		let mut name = None;
		let mut method = None;
		let mut opclass = None;
		let mut m = None;
		let mut ef_construction = None;
		let mut lists = None;

		meta.parse_nested_meta(|nested| {
			if nested.path.is_ident("name") {
				if name.is_some() {
					return Err(nested.error("duplicate vector index key `name`"));
				}
				let value = nested.value()?.parse::<syn::LitStr>()?;
				let parsed = value.value();
				if parsed.is_empty() {
					return Err(syn::Error::new(
						value.span(),
						"vector index name must not be empty",
					));
				}
				if parsed.contains('\0') {
					return Err(syn::Error::new(
						value.span(),
						"vector index name must not contain NUL",
					));
				}
				if parsed.len() > 63 {
					return Err(syn::Error::new(
						value.span(),
						"vector index name must not exceed PostgreSQL's 63-byte identifier limit",
					));
				}
				name = Some((parsed, value.span()));
			} else if nested.path.is_ident("method") {
				if method.is_some() {
					return Err(nested.error("duplicate vector index key `method`"));
				}
				let value = nested.value()?.parse::<syn::LitStr>()?;
				method = Some(match value.value().as_str() {
					"hnsw" => StructuredIndexMethod::Hnsw,
					"ivfflat" => StructuredIndexMethod::Ivfflat,
					_ => {
						return Err(syn::Error::new(
							value.span(),
							"vector index method must be exactly `hnsw` or `ivfflat`",
						));
					}
				});
			} else if nested.path.is_ident("opclass") {
				if opclass.is_some() {
					return Err(nested.error("duplicate vector index key `opclass`"));
				}
				let value = nested.value()?.parse::<syn::LitStr>()?;
				let value_string = value.value();
				if !matches!(
					value_string.as_str(),
					"vector_l2_ops" | "vector_ip_ops" | "vector_cosine_ops"
				) {
					return Err(syn::Error::new(
						value.span(),
						"vector index opclass must be one of: vector_l2_ops, vector_ip_ops, vector_cosine_ops",
					));
				}
				opclass = Some(value_string);
			} else if nested.path.is_ident("m") {
				if m.is_some() {
					return Err(nested.error("duplicate vector index key `m`"));
				}
				let value = nested.value()?.parse::<syn::LitInt>()?;
				let parsed = value.base10_parse::<u16>()?;
				if !(2..=100).contains(&parsed) {
					return Err(syn::Error::new(
						value.span(),
						"vector index option `m` must be in the range 2..=100",
					));
				}
				m = Some(parsed);
			} else if nested.path.is_ident("ef_construction") {
				if ef_construction.is_some() {
					return Err(nested.error("duplicate vector index key `ef_construction`"));
				}
				let value = nested.value()?.parse::<syn::LitInt>()?;
				let parsed = value.base10_parse::<u16>()?;
				if !(4..=1000).contains(&parsed) {
					return Err(syn::Error::new(
						value.span(),
						"vector index option `ef_construction` must be in the range 4..=1000",
					));
				}
				ef_construction = Some(parsed);
			} else if nested.path.is_ident("lists") {
				if lists.is_some() {
					return Err(nested.error("duplicate vector index key `lists`"));
				}
				let value = nested.value()?.parse::<syn::LitInt>()?;
				let parsed = value.base10_parse::<u32>()?;
				if !(1..=32768).contains(&parsed) {
					return Err(syn::Error::new(
						value.span(),
						"vector index option `lists` must be in the range 1..=32768",
					));
				}
				lists = Some(parsed);
			} else {
				return Err(nested.error("unknown vector index key"));
			}
			Ok(())
		})?;

		let (name, name_span) = name.ok_or_else(|| meta.error("vector index requires `name`"))?;
		let method = method.ok_or_else(|| meta.error("vector index requires `method`"))?;
		let opclass = opclass.ok_or_else(|| meta.error("vector index requires `opclass`"))?;
		match method {
			StructuredIndexMethod::Hnsw => {
				if lists.is_some() {
					return Err(meta.error("HNSW vector indexes do not accept `lists`"));
				}
				let effective_m = m.unwrap_or(16);
				let effective_ef_construction = ef_construction.unwrap_or(64);
				if effective_ef_construction < 2 * effective_m {
					return Err(meta.error(
						"HNSW vector index option `ef_construction` must be at least twice `m`",
					));
				}
			}
			StructuredIndexMethod::Ivfflat if m.is_some() || ef_construction.is_some() => {
				return Err(
					meta.error("IVFFlat vector indexes do not accept `m` or `ef_construction`")
				);
			}
			_ => {}
		}

		Ok(Self {
			name,
			name_span,
			method,
			opclass,
			m,
			ef_construction,
			lists,
		})
	}
}

/// Field configuration from `#[field(...)]` attribute
#[derive(Debug, Clone, Default)]
struct FieldConfig {
	primary_key: bool,
	max_length: Option<u64>,
	max_length_span: Option<Span>,
	/// Relative UTC upload-directory template for a storage-backed file field.
	upload_to: Option<String>,
	/// Named storage alias for a storage-backed file field.
	file_storage: Option<String>,
	/// Whether old committed objects are cleaned after database success.
	cleanup: Option<bool>,
	/// Inclusive maximum width for an image field.
	max_width: Option<u32>,
	/// Inclusive maximum height for an image field.
	max_height: Option<u32>,
	null: Option<bool>,
	blank: Option<bool>,
	unique: Option<bool>,
	default: Option<syn::Expr>, // Changed from String to Expr to support bool, int, etc.
	db_column: Option<String>,
	editable: Option<bool>,
	index: Option<bool>,
	index_condition: Option<String>,
	structured_index: Option<StructuredIndexConfig>,
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
	generated: Option<syn::Expr>,
	generated_sql: Option<String>,
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

	// Explicit database type metadata. `text` is also used by MySQL and SQLite
	// inspectdb output, while the remaining mappings are PostgreSQL-specific.
	/// Explicit field type specification (e.g., "jsonb", "hstore", "citext")
	/// Takes priority over automatic type inference
	#[cfg(any(feature = "db-postgres", feature = "db-mysql", feature = "db-sqlite"))]
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
					config.max_length_span = Some(value.span());
					Ok(())
				} else if meta.path.is_ident("upload_to") {
					let value: syn::LitStr = meta.value()?.parse()?;
					config.upload_to = Some(value.value());
					Ok(())
				} else if meta.path.is_ident("file_storage") {
					let value: syn::LitStr = meta.value()?.parse()?;
					config.file_storage = Some(value.value());
					Ok(())
				} else if meta.path.is_ident("cleanup") {
					let value: syn::LitBool = meta.value()?.parse()?;
					config.cleanup = Some(value.value);
					Ok(())
				} else if meta.path.is_ident("max_width") {
					let value: syn::LitInt = meta.value()?.parse()?;
					config.max_width = Some(value.base10_parse()?);
					Ok(())
				} else if meta.path.is_ident("max_height") {
					let value: syn::LitInt = meta.value()?.parse()?;
					config.max_height = Some(value.base10_parse()?);
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
					if config.index.is_some() || config.structured_index.is_some() {
						return Err(meta.error("duplicate `index` field attribute"));
					}
					if meta.input.peek(syn::Token![=]) {
						let value: syn::LitBool = meta.value()?.parse()?;
						config.index = Some(value.value);
					} else {
						config.structured_index = Some(StructuredIndexConfig::parse(meta)?);
					}
					Ok(())
				} else if meta.path.is_ident("condition") {
					let value: syn::LitStr = meta.value()?.parse()?;
					let condition = value.value();
					if condition.trim().is_empty() {
						return Err(meta.error("condition must not be blank"));
					}
					validate_sql_expression(&condition, "condition")?;
					config.index_condition = Some(condition);
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
					let value = meta.value()?;
					if let Ok(ty) = value.parse::<syn::Type>() {
						config.foreign_key = Some(ForeignKeySpec::Type(ty));
						return Ok(());
					}

					// Fall back to string specification
					if let Ok(value) = value.parse::<syn::LitStr>() {
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
							config.foreign_key = Some(ForeignKeySpec::ModelName(spec_str));
							return Ok(());
						}
					}

					Err(meta.error("foreign_key must be a type (User) or string (\"users.User\")"))
				}
				// Generated Columns
				else if meta.path.is_ident("generated") {
					let expr: syn::Expr = meta.value()?.parse()?;
					if matches!(
						&expr,
						syn::Expr::Lit(syn::ExprLit {
							lit: syn::Lit::Str(_),
							..
						})
					) {
						return Err(meta.error(
							"generated expects a SchemaExpr expression; use generated_sql = \"...\" for raw SQL",
						));
					}
					validate_generated_schema_expr(&expr)?;
					config.generated = Some(expr);
					Ok(())
				} else if meta.path.is_ident("generated_sql") {
					let value: syn::LitStr = meta.value()?.parse()?;
					let gen_str = value.value();
					validate_sql_expression(&gen_str, "generated_sql")?;
					config.generated_sql = Some(gen_str);
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
					#[cfg(any(
						feature = "db-postgres",
						feature = "db-mysql",
						feature = "db-sqlite"
					))]
					{
						let value: syn::LitStr = meta.value()?.parse()?;
						config.field_type = Some(value.value());
						Ok(())
					}
					#[cfg(not(any(
						feature = "db-postgres",
						feature = "db-mysql",
						feature = "db-sqlite"
					)))]
					{
						Err(meta.error("field_type is only available with a database feature"))
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
		if self
			.index_condition
			.as_ref()
			.is_some_and(|condition| condition.trim().is_empty())
		{
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"condition must not be blank",
			));
		}
		if self.index_condition.is_some() && self.index != Some(true) {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"condition requires index = true",
			));
		}

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

		if self.auto_increment.is_some() {
			auto_increment_count += 1;
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

		let has_generated = self.generated.is_some() || self.generated_sql.is_some();
		#[cfg(feature = "db-postgres")]
		let has_postgres_auto_increment_attribute =
			self.identity_always.is_some() || self.identity_by_default.is_some();
		#[cfg(not(feature = "db-postgres"))]
		let has_postgres_auto_increment_attribute = false;

		#[cfg(feature = "db-sqlite")]
		let has_sqlite_auto_increment_attribute = self.autoincrement.is_some();
		#[cfg(not(feature = "db-sqlite"))]
		let has_sqlite_auto_increment_attribute = false;

		let has_auto_increment_attribute = self.auto_increment.is_some()
			|| has_postgres_auto_increment_attribute
			|| has_sqlite_auto_increment_attribute;

		if self.generated.is_some() && self.generated_sql.is_some() {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"Generated columns must use either generated or generated_sql, not both",
			));
		}

		// Generated columns cannot have default values
		if has_generated && self.default.is_some() {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"Generated columns cannot have default values",
			));
		}

		if has_generated && has_auto_increment_attribute {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"Generated columns cannot be auto-incrementing",
			));
		}

		// Generated columns should have either generated_stored or generated_virtual
		if has_generated {
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

	/// Validate field configuration that depends on the Rust field type.
	fn validate_for_field_type(&self, ty: &Type) -> Result<()> {
		self.validate()?;
		validate_file_field_config(self, ty)?;

		if self.structured_index.is_some() && vector_dimensions(ty)?.is_none() {
			return Err(syn::Error::new_spanned(
				ty,
				"structured vector index metadata is only valid on Vector<N> fields",
			));
		}

		let has_generated = self.generated.is_some() || self.generated_sql.is_some();
		let implicit_integer_pk_auto_increment = self.primary_key
			&& is_integer_primary_key_type(ty)
			&& self.auto_increment.unwrap_or(true);
		if has_generated && implicit_integer_pk_auto_increment {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"Generated columns cannot be auto-incrementing",
			));
		}

		#[cfg(feature = "db-sqlite")]
		if has_generated && self.primary_key {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"SQLite generated columns cannot be primary keys",
			));
		}

		#[cfg(feature = "db-mysql")]
		if has_generated && self.primary_key && self.generated_virtual.unwrap_or(false) {
			return Err(syn::Error::new(
				proc_macro2::Span::call_site(),
				"MySQL virtual generated columns cannot be primary keys",
			));
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
	form: FieldFormConfig,
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

/// Field-level model-form configuration from `#[form(...)]`.
#[derive(Debug, Clone, Default)]
struct FieldFormConfig {
	trim: bool,
	trim_span: Option<Span>,
}

impl FieldFormConfig {
	/// Parse `#[form(...)]` attributes attached to a model field.
	fn from_attrs(attrs: &[syn::Attribute]) -> Result<Self> {
		let mut config = Self::default();

		for attr in attrs {
			if !attr.path().is_ident("form") {
				continue;
			}

			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("trim") {
					if config.trim {
						return Err(meta.error("duplicate `trim` form field option"));
					}
					if meta.input.peek(syn::Token![=]) {
						return Err(meta.error("`trim` does not accept a value"));
					}
					config.trim = true;
					config.trim_span = Some(meta.path.span());
					Ok(())
				} else {
					Err(meta.error("unknown form field option; expected `trim`"))
				}
			})?;
		}

		Ok(config)
	}
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
	/// Whether the generated ID column is excluded from the Info companion struct
	skip_info: bool,
	/// The full RelAttribute for additional options
	rel_attr: RelAttribute,
}

/// Generate field metadata string from Rust type
fn field_type_to_metadata_string(ty: &Type, _config: &FieldConfig) -> Result<TokenStream> {
	let orm_crate = get_reinhardt_orm_crate();
	if vector_dimensions(ty)?.is_some() {
		return Ok(quote! { "reinhardt.orm.models.VectorField" });
	}
	if let Some(kind) = storage_field_kind(ty) {
		return Ok(match kind {
			StorageFieldKind::File => quote! { "reinhardt.orm.models.FileField" },
			StorageFieldKind::Image => quote! { "reinhardt.orm.models.ImageField" },
		});
	}
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
				"Json" => "JsonField",
				"Value" => "JsonField",
				"HashMap" => "JsonField",
				_ => {
					return Ok(quote! {
						#orm_crate::inspection::database_field_type_path(
							<<#inner_ty as #orm_crate::DatabaseField>::Storage as #orm_crate::DatabaseScalar>::STORAGE_KIND
						)
					});
				}
			};

			let field_type_path = format!("reinhardt.orm.models.{}", type_name);
			Ok(quote! { #field_type_path })
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

fn generated_column_registration(
	config: &FieldConfig,
	migrations_crate: &TokenStream,
) -> TokenStream {
	if config.generated.is_none() && config.generated_sql.is_none() {
		return quote! {};
	}

	let storage = if config.generated_stored.unwrap_or(false) {
		quote! { #migrations_crate::GeneratedStorage::Stored }
	} else {
		quote! { #migrations_crate::GeneratedStorage::Virtual }
	};

	if let Some(generated_expr) = &config.generated {
		let expr_tokens = quote! { #generated_expr }.to_string();
		quote! {
			.with_generated(#migrations_crate::GeneratedColumnDefinition::typed(
				#generated_expr,
				#expr_tokens,
				#storage,
			))
		}
	} else if let Some(generated_sql) = &config.generated_sql {
		quote! {
			.with_generated(#migrations_crate::GeneratedColumnDefinition::raw_sql(
				#generated_sql,
				#storage,
			))
		}
	} else {
		quote! {}
	}
}

/// Map Rust type to ORM field type
fn map_type_to_field_type(ty: &Type, config: &FieldConfig) -> Result<TokenStream> {
	let migrations_crate = get_reinhardt_migrations_crate();
	let orm_crate = get_reinhardt_orm_crate();

	if storage_field_kind(ty).is_some() {
		let max_length =
			file_field_max_length(config).expect("validated FileField max_length must fit in u32");
		return Ok(quote! { #migrations_crate::FieldType::VarChar(#max_length) });
	}

	// Check explicit type metadata before Rust-type inference.
	#[cfg(any(feature = "db-postgres", feature = "db-mysql", feature = "db-sqlite"))]
	if let Some(explicit_type) = &config.field_type {
		return map_explicit_field_type(explicit_type, &migrations_crate);
	}

	if let Some(dimensions) = vector_dimensions(ty)? {
		return Ok(quote! {
			#migrations_crate::FieldType::Vector {
				dimensions: #dimensions,
			}
		});
	}

	// Extract the innermost type when the field uses nested Option wrappers.
	let inner_ty = extract_nested_option_type(ty);

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
				"NaiveDateTime" => {
					quote! { #migrations_crate::FieldType::DateTime }
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
					if is_byte_vector(ty) {
						return Ok(quote! { #migrations_crate::FieldType::Binary });
					}
					return map_vec_to_array_type(ty, last_segment, config, &migrations_crate);
				}
				// Json<T> and serde_json::Value -> JSONB on PostgreSQL, JSON/TEXT elsewhere.
				"Json" => {
					quote! { #migrations_crate::FieldType::Jsonb }
				}
				"Value" => {
					quote! { #migrations_crate::FieldType::Jsonb }
				}
				// Hash maps use the JSON field codec on every database backend.
				"HashMap" => {
					quote! { #migrations_crate::FieldType::Jsonb }
				}
				_ => {
					let max_length = config
						.max_length
						.map(|value| {
							let value = value as u32;
							quote! { ::core::option::Option::Some(#value) }
						})
						.unwrap_or_else(|| quote! { ::core::option::Option::None });
					quote! {
						#orm_crate::inspection::database_storage_field_type(
							<<#inner_ty as #orm_crate::DatabaseField>::Storage as #orm_crate::DatabaseScalar>::STORAGE_KIND,
							#max_length,
						)
					}
				}
			}
		}
		_ => {
			return Err(syn::Error::new_spanned(ty, "Unsupported field type"));
		}
	};

	Ok(field_type)
}

#[cfg(feature = "pgvector")]
fn vector_dimensions(ty: &Type) -> Result<Option<usize>> {
	let (_is_option, inner_ty) = extract_option_type(ty);
	let Type::Path(type_path) = inner_ty else {
		return Ok(None);
	};
	let Some(segment) = type_path.path.segments.last() else {
		return Ok(None);
	};
	if segment.ident != "Vector" {
		return Ok(None);
	}

	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(syn::Error::new_spanned(
			ty,
			"Vector fields require exactly one integer-literal dimension, for example Vector<1536>",
		));
	};
	if arguments.args.len() != 1 {
		return Err(syn::Error::new_spanned(
			arguments,
			"Vector fields require exactly one integer-literal dimension, for example Vector<1536>",
		));
	}
	let Some(GenericArgument::Const(syn::Expr::Lit(literal))) = arguments.args.first() else {
		return Err(syn::Error::new_spanned(
			arguments,
			"Vector dimensions must be an integer literal, for example Vector<1536>",
		));
	};
	let syn::Lit::Int(dimensions) = &literal.lit else {
		return Err(syn::Error::new_spanned(
			literal,
			"Vector dimensions must be an integer literal, for example Vector<1536>",
		));
	};
	let dimensions = dimensions.base10_parse::<usize>().map_err(|_| {
		syn::Error::new_spanned(
			dimensions,
			"Vector dimensions must be an integer literal representable as usize",
		)
	})?;
	if !(1..=2000).contains(&dimensions) {
		return Err(syn::Error::new_spanned(
			literal,
			"Vector dimensions must be in the range 1..=2000",
		));
	}

	Ok(Some(dimensions))
}

#[cfg(not(feature = "pgvector"))]
fn vector_dimensions(_ty: &Type) -> Result<Option<usize>> {
	Ok(None)
}

fn is_builtin_model_field_type(ty: &Type) -> bool {
	if vector_dimensions(ty).ok().flatten().is_some() {
		return true;
	}
	let inner_ty = extract_nested_option_type(ty);
	let Type::Path(type_path) = inner_ty else {
		return false;
	};
	let Some(last_segment) = type_path.path.segments.last() else {
		return false;
	};

	matches!(
		last_segment.ident.to_string().as_str(),
		"i32"
			| "i64" | "String"
			| "bool" | "f32"
			| "f64" | "DateTime"
			| "NaiveDateTime"
			| "Date" | "Time"
			| "Decimal"
			| "Uuid" | "Vec"
			| "Json" | "Value"
			| "HashMap"
	)
}

fn builtin_storage_kind(ty: &Type, orm_crate: &TokenStream) -> Option<TokenStream> {
	if let Some(dimensions) = vector_dimensions(ty).ok().flatten() {
		return Some(quote! {
			#orm_crate::DatabaseStorageKind::Vector(#dimensions)
		});
	}
	let inner_ty = extract_nested_option_type(ty);
	let Type::Path(type_path) = inner_ty else {
		return None;
	};
	let last_segment = type_path.path.segments.last()?;
	if last_segment.ident == "Vec" {
		let PathArguments::AngleBracketed(arguments) = &last_segment.arguments else {
			return None;
		};
		if arguments.args.len() != 1 {
			return None;
		}
		let Some(GenericArgument::Type(Type::Path(element))) = arguments.args.first() else {
			return None;
		};
		return Some(if element.path.is_ident("u8") {
			quote! { #orm_crate::DatabaseStorageKind::Bytes }
		} else {
			quote! { #orm_crate::DatabaseStorageKind::Json }
		});
	}
	let kind = match last_segment.ident.to_string().as_str() {
		"bool" => quote! { #orm_crate::DatabaseStorageKind::Bool },
		"i32" => quote! { #orm_crate::DatabaseStorageKind::I32 },
		"i64" => quote! { #orm_crate::DatabaseStorageKind::I64 },
		"f32" => quote! { #orm_crate::DatabaseStorageKind::F32 },
		"f64" => quote! { #orm_crate::DatabaseStorageKind::F64 },
		"Decimal" => quote! { #orm_crate::DatabaseStorageKind::Decimal },
		"String" => quote! { #orm_crate::DatabaseStorageKind::String },
		"Json" | "Value" | "HashMap" => quote! { #orm_crate::DatabaseStorageKind::Json },
		"Uuid" => quote! { #orm_crate::DatabaseStorageKind::Uuid },
		"Date" => quote! { #orm_crate::DatabaseStorageKind::Date },
		"Time" => quote! { #orm_crate::DatabaseStorageKind::Time },
		"DateTime" => quote! { #orm_crate::DatabaseStorageKind::DateTime },
		"NaiveDateTime" => quote! { #orm_crate::DatabaseStorageKind::NaiveDateTime },
		_ => return None,
	};

	Some(kind)
}

fn is_regular_persisted_field(field: &FieldInfo) -> bool {
	!field.config.skip
		&& !field.is_fk_id_field
		&& !field.injected_relation_serde_skip
		&& !is_relationship_field_type(&field.ty)
		&& !field
			.rel
			.as_ref()
			.map(|relation| matches!(relation.rel_type, crate::rel::RelationType::ManyToMany))
			.unwrap_or(false)
}

fn generate_database_field_validations(field_infos: &[FieldInfo]) -> Vec<TokenStream> {
	let orm_crate = get_reinhardt_orm_crate();

	field_infos
		.iter()
		.filter(|field| {
			is_regular_persisted_field(field)
				&& !is_builtin_model_field_type(&field.ty)
				&& storage_field_kind(&field.ty).is_none()
		})
		.map(|field| {
			let (_is_option, inner_ty) = extract_option_type(&field.ty);
			let storage_kind = quote! {
				<<#inner_ty as #orm_crate::DatabaseField>::Storage as #orm_crate::DatabaseScalar>::STORAGE_KIND
			};
			let max_length_validation = if let Some(max_length) = field.config.max_length {
				let max_length = max_length as usize;
				quote! {
					match #storage_kind {
						#orm_crate::DatabaseStorageKind::String => {
							if let ::core::option::Option::Some(required) =
								<#inner_ty as #orm_crate::DatabaseField>::MAX_STRING_VALUE_CHARS
							{
								assert!(required <= #max_length, "model enum value exceeds field max_length");
							}
						}
						#orm_crate::DatabaseStorageKind::I32 => {
							panic!("integer database fields do not accept max_length");
						}
						_ => {}
					}
				}
			} else {
				quote! {
					if let #orm_crate::DatabaseStorageKind::String = #storage_kind {
						panic!("string database fields require max_length attribute");
					}
				}
			};

			quote! {
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				const _: () = {
					#max_length_validation
				};
			}
		})
		.collect()
}

/// Map explicit database field type string to FieldType.
#[cfg(any(feature = "db-postgres", feature = "db-mysql", feature = "db-sqlite"))]
fn map_explicit_field_type(
	field_type_str: &str,
	migrations_crate: &proc_macro2::TokenStream,
) -> Result<TokenStream> {
	let normalized = field_type_str.trim().to_ascii_lowercase();
	if let Some(length) = normalized
		.strip_prefix("char(")
		.and_then(|value| value.strip_suffix(')'))
		.and_then(|value| value.parse::<u32>().ok())
	{
		return Ok(quote! { #migrations_crate::FieldType::Char(#length) });
	}
	let field_type = match normalized.as_str() {
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
					 tsvector, tsquery, uuid, text, char(n)",
					other
				),
			));
		}
	};
	Ok(field_type)
}

#[cfg(feature = "db-postgres")]
fn is_byte_vector(ty: &Type) -> bool {
	let (_is_option, inner_ty) = extract_option_type(ty);
	let Type::Path(type_path) = inner_ty else {
		return false;
	};
	let Some(segment) = type_path.path.segments.last() else {
		return false;
	};
	let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return false;
	};
	matches!(arguments.args.first(), Some(GenericArgument::Type(Type::Path(element))) if element.path.is_ident("u8"))
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

fn extract_nested_option_type(mut ty: &Type) -> &Type {
	while let (true, inner_ty) = extract_option_type(ty) {
		ty = inner_ty;
	}
	ty
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StorageFieldKind {
	File,
	Image,
}

impl StorageFieldKind {
	fn model_field_type(self) -> &'static str {
		match self {
			Self::File => "file",
			Self::Image => "image",
		}
	}

	fn rust_name(self) -> &'static str {
		match self {
			Self::File => "FileField",
			Self::Image => "ImageField",
		}
	}
}

/// Classify storage-backed file values and their nullable forms.
fn storage_field_kind(ty: &Type) -> Option<StorageFieldKind> {
	let (is_option, inner_ty) = extract_option_type(ty);
	if is_option && extract_option_type(inner_ty).0 {
		return None;
	}
	let Type::Path(type_path) = inner_ty else {
		return None;
	};
	let segments = &type_path.path.segments;
	if segments.len() > 1
		&& !segments.iter().rev().nth(1).is_some_and(|segment| {
			matches!(segment.ident.to_string().as_str(), "orm" | "file_fields")
		}) {
		return None;
	}
	segments
		.last()
		.and_then(|segment| match segment.ident.to_string().as_str() {
			"FileField" => Some(StorageFieldKind::File),
			"ImageField" => Some(StorageFieldKind::Image),
			_ => None,
		})
}

fn nested_storage_field_kind(ty: &Type) -> Option<StorageFieldKind> {
	let (is_option, inner_ty) = extract_option_type(ty);
	if !is_option || !extract_option_type(inner_ty).0 {
		return None;
	}
	let innermost = extract_nested_option_type(inner_ty);
	let Type::Path(type_path) = innermost else {
		return None;
	};
	let segments = &type_path.path.segments;
	if segments.len() > 1
		&& !segments.iter().rev().nth(1).is_some_and(|segment| {
			matches!(segment.ident.to_string().as_str(), "orm" | "file_fields")
		}) {
		return None;
	}
	segments
		.last()
		.and_then(|segment| match segment.ident.to_string().as_str() {
			"FileField" => Some(StorageFieldKind::File),
			"ImageField" => Some(StorageFieldKind::Image),
			_ => None,
		})
}

fn file_field_max_length(
	config: &FieldConfig,
) -> std::result::Result<u32, std::num::TryFromIntError> {
	config.max_length.unwrap_or(100).try_into()
}

fn valid_file_storage_alias(alias: &str) -> bool {
	if alias == "default" {
		return true;
	}
	let Some((first, rest)) = alias.as_bytes().split_first() else {
		return false;
	};
	first.is_ascii_lowercase()
		&& rest.iter().all(|character| {
			character.is_ascii_lowercase()
				|| character.is_ascii_digit()
				|| matches!(character, b'_' | b'-')
		})
}

fn file_template_token_length(token: char) -> Option<usize> {
	file_template_token_replacement(token).map(str::len)
}

fn file_template_token_replacement(token: char) -> Option<&'static str> {
	match token {
		'Y' => Some("2000"),
		'm' | 'd' => Some("01"),
		'H' => Some("00"),
		'M' => Some("34"),
		'S' => Some("56"),
		_ => None,
	}
}

fn is_windows_device_basename_for_macro(component: &str) -> bool {
	let basename = component.split('.').next().unwrap_or(component);
	let uppercase = basename.to_ascii_uppercase();
	matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
		|| uppercase
			.strip_prefix("COM")
			.or_else(|| uppercase.strip_prefix("LPT"))
			.is_some_and(|number| {
				matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
			})
}

fn file_template_component_length(component: &str) -> std::result::Result<usize, String> {
	let mut length = 0usize;
	let mut characters = component.chars();
	while let Some(character) = characters.next() {
		if character != '%' {
			length = length.saturating_add(1);
			continue;
		}
		let token = characters
			.next()
			.ok_or_else(|| "incomplete UTC token".to_owned())?;
		length = length.saturating_add(
			file_template_token_length(token)
				.ok_or_else(|| format!("unsupported UTC token `%{token}`"))?,
		);
	}
	Ok(length)
}

/// Substitute supported upload-template tokens with the same representative
/// value used by the runtime validator before checking component structure.
fn file_template_component_structure(component: &str) -> std::result::Result<String, String> {
	let mut structural = String::with_capacity(component.len());
	let mut characters = component.chars();
	while let Some(character) = characters.next() {
		if character != '%' {
			structural.push(character);
			continue;
		}
		let token = characters
			.next()
			.ok_or_else(|| "incomplete UTC token".to_owned())?;
		let replacement = file_template_token_replacement(token)
			.ok_or_else(|| format!("unsupported UTC token `%{token}`"))?;
		structural.push_str(replacement);
	}
	Ok(structural)
}

fn validate_file_upload_template(template: &str) -> std::result::Result<usize, String> {
	if template.is_empty() {
		return Err("template is empty".to_owned());
	}
	if template.starts_with('/') {
		return Err("rooted templates are not allowed".to_owned());
	}
	if template.contains('\\') {
		return Err("backslashes are not allowed".to_owned());
	}
	if template.len() >= 2
		&& template.as_bytes()[0].is_ascii_alphabetic()
		&& template.as_bytes()[1] == b':'
	{
		return Err("drive prefixes are not allowed".to_owned());
	}
	if template.contains('\0') {
		return Err("NUL is not allowed".to_owned());
	}
	if template.chars().any(char::is_control) {
		return Err("control characters are not allowed".to_owned());
	}

	let mut total = template.matches('/').count();
	for component in template.split('/') {
		if component.is_empty() {
			return Err("empty components are not allowed".to_owned());
		}
		if component == "." {
			return Err("dot components are not allowed".to_owned());
		}
		if component == ".." {
			return Err("parent components are not allowed".to_owned());
		}
		let length = file_template_component_length(component)?;
		let structural = file_template_component_structure(component)?;
		if structural.contains(['<', '>', ':', '"', '|', '?', '*']) {
			return Err("template component contains Windows-forbidden characters".to_owned());
		}
		if component.ends_with(['.', ' ']) || is_windows_device_basename_for_macro(&structural) {
			return Err("template component has unsafe trailing or device-name syntax".to_owned());
		}
		total = total.saturating_add(length);
	}
	Ok(total)
}

fn validate_file_field_config(config: &FieldConfig, ty: &Type) -> Result<()> {
	if let Some(storage_kind) = nested_storage_field_kind(ty) {
		return Err(syn::Error::new_spanned(
			ty,
			format!(
				"nested Option wrappers are not supported for {}",
				storage_kind.rust_name()
			),
		));
	}
	let storage_kind = storage_field_kind(ty);
	let has_file_attribute = config.upload_to.is_some() || config.file_storage.is_some();
	let has_image_attribute = config.max_width.is_some() || config.max_height.is_some();
	if storage_kind.is_none() {
		if has_image_attribute {
			return Err(syn::Error::new_spanned(
				ty,
				"max_width and max_height are only valid on ImageField or Option<ImageField>",
			));
		}
		if config.cleanup.is_some() {
			return Err(syn::Error::new_spanned(
				ty,
				"upload_to, file_storage, and cleanup are only valid on FileField, ImageField, or their Option forms",
			));
		}
		if has_file_attribute {
			return Err(syn::Error::new_spanned(
				ty,
				"upload_to and file_storage are only valid on FileField or Option<FileField>",
			));
		}
		return Ok(());
	}
	let storage_kind = storage_kind.expect("storage field kind was checked above");
	if storage_kind == StorageFieldKind::File && has_image_attribute {
		return Err(syn::Error::new_spanned(
			ty,
			"max_width and max_height are only valid on ImageField or Option<ImageField>",
		));
	}
	if storage_kind == StorageFieldKind::Image
		&& (config.max_width == Some(0) || config.max_height == Some(0))
	{
		return Err(syn::Error::new_spanned(
			ty,
			"ImageField max_width and max_height must be positive",
		));
	}

	let Some(upload_to) = config.upload_to.as_deref() else {
		return Err(syn::Error::new_spanned(
			ty,
			format!(
				"{} fields require an `upload_to` attribute",
				storage_kind.rust_name()
			),
		));
	};
	let directory_length = validate_file_upload_template(upload_to).map_err(|reason| {
		syn::Error::new_spanned(ty, format!("invalid upload template: {reason}"))
	})?;
	let storage_alias = config.file_storage.as_deref().unwrap_or("default");
	if !valid_file_storage_alias(storage_alias) {
		return Err(syn::Error::new_spanned(
			ty,
			format!("invalid storage alias `{storage_alias}`"),
		));
	}

	// Reserve one scalar for the client filename stem, a dot and one scalar
	// for the minimum extension, plus the atomic collision suffix allowance.
	let minimum_length = directory_length
		.saturating_add(usize::from(directory_length > 0))
		.saturating_add(1)
		.saturating_add(2)
		.saturating_add(17);
	let max_length = file_field_max_length(config).map_err(|_| {
		syn::Error::new(
			config.max_length_span.unwrap_or_else(|| ty.span()),
			format!(
				"{} max_length must not exceed u32::MAX (4294967295)",
				storage_kind.rust_name()
			),
		)
	})? as usize;
	if max_length < minimum_length {
		return Err(syn::Error::new_spanned(
			ty,
			format!(
				"{} max_length {max_length} is too small for upload_to template; at least {minimum_length} characters are required",
				storage_kind.rust_name()
			),
		));
	}
	Ok(())
}

fn is_model_form_editable(field: &FieldInfo, field_infos: &[FieldInfo]) -> bool {
	if field.config.skip || field.config.editable == Some(false) {
		return false;
	}
	if field.config.primary_key && is_auto_generated_field(field) {
		return false;
	}
	if field.config.auto_now == Some(true)
		|| field.config.auto_now_add == Some(true)
		|| field.config.generated.is_some()
		|| field.config.generated_sql.is_some()
		|| field.config.include_in_new == Some(false)
	{
		return false;
	}

	if field.is_fk_id_field {
		let relation_name = field.name.to_string();
		return relation_name
			.strip_suffix("_id")
			.and_then(|name| field_infos.iter().find(|candidate| candidate.name == name))
			.is_some_and(|relation| {
				relation.config.editable != Some(false)
					&& relation.config.include_in_new != Some(false)
			});
	}

	field.config.editable == Some(true)
		|| (!is_relationship_field_type(&field.ty) && field.rel.is_none())
}

fn model_form_kind(field: &FieldInfo) -> Result<TokenStream> {
	let core_crate = get_reinhardt_core_crate();
	if let Some(kind) = storage_field_kind(&field.ty) {
		return Ok(match kind {
			StorageFieldKind::File => quote!(#core_crate::model_form::ModelFormFieldKind::File),
			StorageFieldKind::Image => quote!(#core_crate::model_form::ModelFormFieldKind::Image),
		});
	}
	let inner_ty = extract_nested_option_type(&field.ty);
	let unsupported = || {
		Err(syn::Error::new_spanned(
			&field.name,
			format!(
				"editable model field `{}` has no supported model-form mapping; set editable = false or use an explicit non-model form",
				field.name
			),
		))
	};

	if is_many_to_many_field_type(inner_ty)
		|| is_relationship_field_type(inner_ty)
		|| field.rel.as_ref().is_some_and(|relation| {
			!matches!(
				relation.rel_type,
				crate::rel::RelationType::ForeignKey | crate::rel::RelationType::OneToOne
			)
		}) {
		return unsupported();
	}

	let Type::Path(type_path) = inner_ty else {
		return unsupported();
	};
	let Some(segment) = type_path.path.segments.last() else {
		return unsupported();
	};

	let kind = match segment.ident.to_string().as_str() {
		"String" => {
			let min_length = field
				.config
				.min_length
				.map(|value| quote!(::core::option::Option::Some(#value as usize)))
				.unwrap_or_else(|| quote!(::core::option::Option::None));
			let max_length = field
				.config
				.max_length
				.map(|value| quote!(::core::option::Option::Some(#value as usize)))
				.unwrap_or_else(|| quote!(::core::option::Option::None));
			if field.config.email == Some(true) {
				quote!(#core_crate::model_form::ModelFormFieldKind::Email { min_length: #min_length, max_length: #max_length })
			} else if field.config.url == Some(true) {
				quote!(#core_crate::model_form::ModelFormFieldKind::Url { min_length: #min_length, max_length: #max_length })
			} else {
				quote!(#core_crate::model_form::ModelFormFieldKind::Text { min_length: #min_length, max_length: #max_length, multiline: false })
			}
		}
		"i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
			let min = field
				.config
				.min_value
				.map(|value| quote!(::core::option::Option::Some(#value)))
				.unwrap_or_else(|| quote!(::core::option::Option::None));
			let max = field
				.config
				.max_value
				.map(|value| quote!(::core::option::Option::Some(#value)))
				.unwrap_or_else(|| quote!(::core::option::Option::None));
			quote!(#core_crate::model_form::ModelFormFieldKind::Integer { min: #min, max: #max })
		}
		"f32" => {
			if field
				.config
				.min_value
				.is_some_and(|value| !(value as f32).is_finite())
				|| field
					.config
					.max_value
					.is_some_and(|value| !(value as f32).is_finite())
			{
				return Err(syn::Error::new_spanned(
					&field.name,
					"f32 model-form bounds must be finite f32 values",
				));
			}
			let min = field
				.config
				.min_value
				.map(|value| quote!(::core::option::Option::Some(#value as f64)))
				.unwrap_or_else(|| {
					quote!(::core::option::Option::Some(
						::core::primitive::f32::MIN as f64
					))
				});
			let max = field
				.config
				.max_value
				.map(|value| quote!(::core::option::Option::Some(#value as f64)))
				.unwrap_or_else(|| {
					quote!(::core::option::Option::Some(
						::core::primitive::f32::MAX as f64
					))
				});
			quote!(#core_crate::model_form::ModelFormFieldKind::Float { min: #min, max: #max })
		}
		"f64" => {
			let min = field
				.config
				.min_value
				.map(|value| quote!(::core::option::Option::Some(#value as f64)))
				.unwrap_or_else(|| quote!(::core::option::Option::None));
			let max = field
				.config
				.max_value
				.map(|value| quote!(::core::option::Option::Some(#value as f64)))
				.unwrap_or_else(|| quote!(::core::option::Option::None));
			quote!(#core_crate::model_form::ModelFormFieldKind::Float { min: #min, max: #max })
		}
		"Decimal" => {
			let min = field
				.config
				.min_value
				.map(|value| {
					let value = LitStr::new(&value.to_string(), field.name.span());
					quote!(::core::option::Option::Some(#value))
				})
				.unwrap_or_else(|| quote!(::core::option::Option::None));
			let max = field
				.config
				.max_value
				.map(|value| {
					let value = LitStr::new(&value.to_string(), field.name.span());
					quote!(::core::option::Option::Some(#value))
				})
				.unwrap_or_else(|| quote!(::core::option::Option::None));
			quote!(#core_crate::model_form::ModelFormFieldKind::Decimal { min: #min, max: #max })
		}
		"bool" => quote!(#core_crate::model_form::ModelFormFieldKind::Boolean),
		"Date" | "NaiveDate" => quote!(#core_crate::model_form::ModelFormFieldKind::Date),
		"Time" | "NaiveTime" => quote!(#core_crate::model_form::ModelFormFieldKind::Time),
		"DateTime" => quote!(#core_crate::model_form::ModelFormFieldKind::DateTime),
		"NaiveDateTime" => quote!(#core_crate::model_form::ModelFormFieldKind::NaiveDateTime),
		"Uuid" => quote!(#core_crate::model_form::ModelFormFieldKind::Uuid),
		"Json" | "Value" | "HashMap" => quote!(#core_crate::model_form::ModelFormFieldKind::Json),
		_ => return unsupported(),
	};

	Ok(kind)
}

fn validate_model_form_trim(field_infos: &[FieldInfo], model_form_enabled: bool) -> Result<()> {
	for field in field_infos.iter().filter(|field| field.form.trim) {
		if !model_form_enabled
			|| !is_model_form_editable(field, field_infos)
			|| !is_string_type(&field.ty)
		{
			return Err(syn::Error::new(
				field.form.trim_span.unwrap_or_else(|| field.name.span()),
				"`trim` is only valid on editable text, email, or URL ModelForm fields",
			));
		}
	}
	Ok(())
}

fn model_form_relation_id_kind(
	field: &FieldInfo,
	field_infos: &[FieldInfo],
) -> Result<TokenStream> {
	let core_crate = get_reinhardt_core_crate();
	let relation = model_form_relation_id_target_type(field, field_infos)?;
	Ok(quote!(<#relation as #core_crate::model_form::ModelFormPrimaryKey>::FIELD_KIND))
}

fn model_form_relation_id_target_type<'a>(
	field: &FieldInfo,
	field_infos: &'a [FieldInfo],
) -> Result<&'a Type> {
	let field_name = field.name.to_string();
	let relation_name = field_name.strip_suffix("_id").ok_or_else(|| {
		syn::Error::new_spanned(
			&field.name,
			"generated relation id field must end with `_id`",
		)
	})?;
	field_infos
		.iter()
		.find(|candidate| candidate.name == relation_name)
		.and_then(|candidate| extract_fk_target_type(&candidate.ty))
		.ok_or_else(|| {
			syn::Error::new_spanned(
				&field.name,
				"generated relation id field has no foreign-key target",
			)
		})
}

fn model_form_relation_id_is_nullable(field: &FieldInfo, field_infos: &[FieldInfo]) -> bool {
	if !field.is_fk_id_field {
		return false;
	}

	let field_name = field.name.to_string();
	let Some(relation_name) = field_name.strip_suffix("_id") else {
		return false;
	};

	field_infos.iter().any(|candidate| {
		candidate.name == relation_name
			&& candidate
				.rel
				.as_ref()
				.is_some_and(|relation| relation.null == Some(true))
	})
}

fn model_form_declared_default(field: &FieldInfo) -> Option<TokenStream> {
	let expression = field.config.default.as_ref()?;
	let (is_optional, inner_ty) = extract_option_type(&field.ty);
	let value = if is_string_type(inner_ty)
		&& matches!(
			expression,
			syn::Expr::Lit(syn::ExprLit {
				lit: syn::Lit::Str(_),
				..
			})
		) {
		quote!((#expression).to_owned())
	} else {
		quote!(#expression)
	};

	Some(if is_optional {
		quote!(::core::option::Option::Some(#value))
	} else {
		value
	})
}

fn generate_model_form_support(
	struct_name: &Ident,
	struct_vis: &syn::Visibility,
	field_infos: &[FieldInfo],
	form_config: &ModelFormConfig,
) -> Result<TokenStream> {
	let core_crate = get_reinhardt_core_crate();
	let forms_crate = get_reinhardt_forms_crate();
	let validation_enabled = forms_crate.is_some();
	let native_form_cfg = if forms_crate.is_some() {
		quote!(#[cfg(not(all(target_family = "wasm", target_os = "unknown")))])
	} else {
		quote!(#[cfg(any())])
	};
	let forms_crate = forms_crate.unwrap_or_else(|| quote!(::reinhardt_forms));
	let orm_crate = get_reinhardt_orm_crate();
	let serde_crate = get_serde_crate();
	let serde_json_crate = get_serde_json_crate();
	let schema_name = Ident::new(&format!("{}FormSchema", struct_name), struct_name.span());
	let payload_name = Ident::new(&format!("{}ModelFormData", struct_name), struct_name.span());
	let cleaned_payload_name = Ident::new(
		&format!("Cleaned{}ModelFormData", struct_name),
		struct_name.span(),
	);
	let visitor_name = Ident::new(
		&format!("{}ModelFormDataVisitor", struct_name),
		struct_name.span(),
	);
	let field_const_name = Ident::new(
		&format!("{}_FORM_FIELDS", struct_name.to_string().to_uppercase()),
		struct_name.span(),
	);
	let editable_fields: Vec<_> = field_infos
		.iter()
		.filter(|field| is_model_form_editable(field, field_infos))
		.collect();
	if let Some(field) = editable_fields
		.iter()
		.find(|field| field.name == "csrfmiddlewaretoken")
	{
		return Err(syn::Error::new_spanned(
			&field.name,
			"model-backed forms reserve `csrfmiddlewaretoken` for the CSRF control",
		));
	}
	let field_count = editable_fields.len();
	let field_kinds: Vec<_> = editable_fields
		.iter()
		.map(|field| {
			if field.is_fk_id_field {
				model_form_relation_id_kind(field, field_infos)
			} else {
				model_form_kind(field)
			}
		})
		.collect::<Result<_>>()?;
	let field_names: Vec<_> = editable_fields.iter().map(|field| &field.name).collect();
	let mut generated_method_names = HashSet::new();
	for field in &editable_fields {
		let field_name = field.name.to_string();
		let setter_name = format!("set_{field_name}");
		let trusted_setter_name = format!("set_trusted_{field_name}");
		let collides_with_reserved_api = [
			"clean_and_validate",
			"clean_and_validate_for_update",
			"clone",
			"default",
			"empty",
			"fields",
			"forbidden_fields",
			"from_validated_raw",
			"supplied_fields",
			"get_json",
			"into_raw",
			"into_model",
			"apply_to",
			"set_json",
			"from_native_form_value",
			"_policy",
		]
		.contains(&field_name.as_str())
			|| field_name.starts_with("__reinhardt_checkbox_")
			|| field_name.starts_with("__reinhardt_color_")
			|| field_name.starts_with("__reinhardt_range_")
			|| field_name.starts_with("__reinhardt_defaulted_")
			|| [
				"empty",
				"forbidden_fields",
				"supplied_fields",
				"get_json",
				"set_json",
				"from_native_form_value",
			]
			.contains(&setter_name.as_str());
		let collides_with_generated_method = !generated_method_names.insert(field_name)
			|| !generated_method_names.insert(setter_name)
			|| !generated_method_names.insert(trusted_setter_name);
		if collides_with_reserved_api || collides_with_generated_method {
			return Err(syn::Error::new_spanned(
				&field.name,
				"editable model field name collides with generated model-form API; rename the field or set editable = false",
			));
		}
	}
	let field_types: Vec<_> = editable_fields.iter().map(|field| &field.ty).collect();
	let trusted_field_kinds: Vec<_> = field_infos
		.iter()
		.filter(|field| field.is_fk_id_field)
		.map(|field| {
			let name = LitStr::new(&field.name.to_string(), field.name.span());
			let kind = if field.is_fk_id_field {
				model_form_relation_id_kind(field, field_infos)
			} else {
				model_form_kind(field)
			}?;
			Ok(quote!(#name => ::core::option::Option::Some(#kind)))
		})
		.collect::<Result<Vec<_>>>()?;
	let trusted_relation_requiredness = field_infos
		.iter()
		.filter(|field| field.is_fk_id_field)
		.map(|field| {
			let name = LitStr::new(&field.name.to_string(), field.name.span());
			let (is_optional, _) = extract_option_type(&field.ty);
			let nullable = field
				.config
				.null
				.unwrap_or(is_optional || model_form_relation_id_is_nullable(field, field_infos));
			let required =
				!nullable && field.config.blank != Some(true) && field.config.default.is_none();
			quote!(#name => #required)
		});
	let trusted_field_assignments = field_infos.iter().map(|field| {
		let name = LitStr::new(&field.name.to_string(), field.name.span());
		let ident = Ident::new(&field.name.to_string(), field.name.span());
		let ty = &field.ty;
		quote! {
			#name => {
				self.#ident = #serde_json_crate::from_value::<#ty>(value).map_err(|error| {
					#forms_crate::model_form::ModelFormError::FieldValidation {
						errors: ::std::collections::HashMap::from([(
							field.to_owned(),
							vec![error.to_string()],
						)]),
					}
				})?;
				::core::result::Result::Ok(())
			}
		}
	});
	let field_literals: Vec<_> = editable_fields
		.iter()
		.map(|field| LitStr::new(&field.name.to_string(), field.name.span()))
		.collect();
	let primary_key_literals: Vec<_> = field_infos
		.iter()
		.filter(|field| field.config.primary_key)
		.map(|field| LitStr::new(&field.name.to_string(), field.name.span()))
		.collect();
	let reject_supplied_primary_keys = if primary_key_literals.is_empty() {
		quote! {}
	} else {
		quote! {
			if let Some(field) = supplied_fields.iter().copied().find(|field| {
				[#(#primary_key_literals),*]
					.iter()
					.any(|primary_key| *primary_key == *field)
			}) {
				let mut errors = #core_crate::validators::ValidationErrors::new();
				errors.add(
					field.to_owned(),
					#core_crate::validators::ValidationError::Custom(
						"model form primary keys cannot be updated".to_owned(),
					),
				);
				return ::core::result::Result::Err(errors);
			}
		}
	};
	let descriptor_entries = editable_fields
		.iter()
		.zip(&field_kinds)
		.map(|(field, kind)| {
			let name = LitStr::new(&field.name.to_string(), field.name.span());
			let (is_optional, _) = extract_option_type(&field.ty);
			let relation_is_nullable = model_form_relation_id_is_nullable(field, field_infos);
			let nullable = field
				.config
				.null
				.unwrap_or(is_optional || relation_is_nullable);
			let required =
				!nullable && field.config.blank != Some(true) && field.config.default.is_none();
			let has_default = field.config.default.is_some();
			let generated_relation_id = field.is_fk_id_field;
			let trim = field.form.trim;
			quote! {
				#core_crate::model_form::ModelFormFieldDescriptor {
					name: #name,
					kind: #kind,
					required: #required,
					has_default: #has_default,
					nullable: #nullable,
					editable: true,
					generated_relation_id: #generated_relation_id,
					trim: #trim,
				}
			}
		});
	let default_true_boolean_arms: Vec<_> = editable_fields
		.iter()
		.filter(|field| {
			let (_, value_type) = extract_option_type(&field.ty);
			value_type.to_token_stream().to_string() == "bool" && field.config.default.is_some()
		})
		.map(|field| {
			let name = LitStr::new(&field.name.to_string(), field.name.span());
			let default = field
				.config
				.default
				.as_ref()
				.expect("boolean default fields always have a default expression");
			quote!(#name => #default)
		})
		.collect();
	let default_true_boolean_body = if default_true_boolean_arms.is_empty() {
		quote!(false)
	} else {
		quote!(match field { #(#default_true_boolean_arms,)* _ => false })
	};
	let relation_target_match_arms = field_infos
		.iter()
		.filter(|field| field.is_fk_id_field)
		.map(|field| {
			let name = LitStr::new(&field.name.to_string(), field.name.span());
			let target = model_form_relation_id_target_type(field, field_infos)?;
			Ok(
				quote!(#name => ::core::any::TypeId::of::<T>() == ::core::any::TypeId::of::<#target>()),
			)
		})
		.collect::<Result<Vec<_>>>()?;
	let descriptor_accessors = field_names.iter().enumerate().map(|(index, field_name)| {
		quote! {
			pub const fn #field_name() -> &'static #core_crate::model_form::ModelFormFieldDescriptor {
				&#field_const_name[#index]
			}
		}
	});
	let getters = field_names
		.iter()
		.zip(&field_types)
		.map(|(field_name, field_ty)| {
			quote! {
				#[doc = "Returns the raw, unvalidated value when this field was supplied."]
				#[doc = "This P2 supplied-value accessor has equivalent semantics on native and WASM targets."]
				pub fn #field_name(&self) -> ::core::option::Option<&#field_ty> {
					self.#field_name.as_ref()
				}
			}
		});
	let setters = field_names
		.iter()
		.zip(&field_types)
		.map(|(field_name, field_ty)| {
			let setter_name = Ident::new(&format!("set_{}", field_name), field_name.span());
			let field_literal = LitStr::new(&field_name.to_string(), field_name.span());
			quote! {
				pub fn #setter_name(
					&mut self,
					value: #field_ty,
				) -> ::core::result::Result<(), #core_crate::model_form::ModelFormPayloadError> {
					if !<P as #core_crate::model_form::ModelFormPolicy>::allows(#field_literal) {
						return ::core::result::Result::Err(
							#core_crate::model_form::ModelFormPayloadError::ForbiddenField {
								field: #field_literal.to_owned(),
							},
						);
					}
					self.#field_name = ::core::option::Option::Some(value);
					::core::result::Result::Ok(())
				}
			}
		});
	let trusted_setters = field_names
		.iter()
		.zip(&field_types)
		.map(|(field_name, field_ty)| {
			let setter_name = Ident::new(&format!("set_trusted_{field_name}"), field_name.span());
			quote! {
				pub fn #setter_name(&mut self, value: #field_ty) {
					self.#field_name = ::core::option::Option::Some(value);
				}
			}
		});
	let empty_fields = field_names
		.iter()
		.map(|field_name| quote!(#field_name: ::core::option::Option::None));
	let supplied_fields =
		field_names
			.iter()
			.zip(&field_literals)
			.map(|(field_name, field_literal)| {
				quote! {
					if self.#field_name.is_some() {
						fields.push(#field_literal);
					}
				}
			});
	let get_json_arms = field_names.iter().zip(&field_literals).map(|(field_name, field_literal)| {
		quote! {
			#field_literal => self.#field_name.as_ref().and_then(|value| #serde_json_crate::to_value(value).ok()),
		}
	});
	let set_json_arms = field_names.iter().zip(&field_literals).zip(&field_types).map(
		|((field_name, field_literal), field_ty)| {
			quote! {
				#field_literal => {
					if !<P as #core_crate::model_form::ModelFormPolicy>::allows(#field_literal) {
						return ::core::result::Result::Err(#core_crate::model_form::ModelFormPayloadError::ForbiddenField {
							field: #field_literal.to_owned(),
						});
					}
					let parsed = #serde_json_crate::from_value::<#field_ty>(value).map_err(|error| {
						#core_crate::model_form::ModelFormPayloadError::InvalidValue {
							field: #field_literal.to_owned(),
							message: error.to_string(),
						}
					})?;
					self.#field_name = ::core::option::Option::Some(parsed);
					::core::result::Result::Ok(())
				}
			}
		},
	);
	let serialize_entries =
		field_names
			.iter()
			.zip(&field_literals)
			.map(|(field_name, field_literal)| {
				quote! {
					if <P as #core_crate::model_form::ModelFormPolicy>::allows(#field_literal) {
						if let ::core::option::Option::Some(value) = &self.#field_name {
							#serde_crate::ser::SerializeMap::serialize_entry(&mut map, #field_literal, value)?;
						}
					}
				}
			});
	let deserialize_arms =
		field_names
			.iter()
			.zip(&field_literals)
			.map(|(field_name, field_literal)| {
				quote! {
					#field_literal => {
						if <P as #core_crate::model_form::ModelFormPolicy>::allows(#field_literal) {
							#field_name = ::core::option::Option::Some(map.next_value()?);
						} else {
							let _: #serde_crate::de::IgnoredAny = map.next_value()?;
							if !forbidden_fields.contains(&#field_literal) {
								forbidden_fields.push(#field_literal);
							}
						}
					}
				}
			});
	let deserialize_initializers = field_names
		.iter()
		.map(|field_name| quote!(let mut #field_name = ::core::option::Option::None;));
	let serialize_bounds: Vec<_> = field_types
		.iter()
		.map(|field_ty| quote!(#field_ty: #serde_crate::Serialize))
		.collect();
	let deserialize_bounds: Vec<_> = field_types
		.iter()
		.map(|field_ty| quote!(#field_ty: #serde_crate::Deserialize<'de>))
		.collect();
	let payload_bounds: Vec<_> = field_types
		.iter()
		.map(
			|field_ty| quote!(#field_ty: #serde_crate::Serialize + #serde_crate::de::DeserializeOwned),
		)
		.collect();
	let server_context_fields: Vec<_> = field_infos
		.iter()
		.filter(|field| {
			if is_model_form_editable(field, field_infos)
				|| field.config.skip
				|| field.config.include_in_new == Some(false)
				|| is_relationship_field_type(&field.ty)
				|| model_form_declared_default(field).is_some()
				|| field.config.auto_now == Some(true)
				|| field.config.auto_now_add == Some(true)
				|| field.config.generated.is_some()
				|| field.config.generated_sql.is_some()
				|| (field.config.primary_key && is_auto_generated_field(field))
			{
				return false;
			}
			let (is_optional, _) = extract_option_type(&field.ty);
			let nullable = is_optional || model_form_relation_id_is_nullable(field, field_infos);
			!nullable && field.config.blank != Some(true)
		})
		.collect();
	if let Some(field) = server_context_fields
		.iter()
		.find(|field| ["new", "_state"].contains(&field.name.to_string().as_str()))
	{
		return Err(syn::Error::new_spanned(
			&field.name,
			"required non-editable model field name collides with generated model-form server context; rename the field or provide a model default",
		));
	}
	let server_context_names: HashSet<_> = server_context_fields
		.iter()
		.map(|field| field.name.to_string())
		.collect();
	let build_from_cleaned_assignments: Vec<_> = field_infos
		.iter()
		.map(|field| {
			let field_name = &field.name;
			let field_literal = LitStr::new(&field.name.to_string(), field.name.span());
			if is_model_form_editable(field, field_infos) {
				let field_ty = &field.ty;
				let (is_optional, _) = extract_option_type(&field.ty);
				let relation_is_nullable = model_form_relation_id_is_nullable(field, field_infos);
				let nullable = field
					.config
					.null
					.unwrap_or(is_optional || relation_is_nullable);
				let required =
					!nullable && field.config.blank != Some(true) && field.config.default.is_none();
				let unresolved = if let Some(default) = model_form_declared_default(field) {
					default
				} else if is_auto_generated_field(field) && !field.is_fk_id_field {
					get_auto_field_default_value(field)
				} else if required || (!nullable && field.config.blank == Some(true)) {
					quote! {
						return ::core::result::Result::Err(
							#forms_crate::model_form::ModelFormError::MissingModelField {
								field: #field_literal,
							},
						)
					}
				} else {
					quote!(::std::default::Default::default())
				};
				quote! {
					#field_name: match data.#field_name.as_ref() {
						::core::option::Option::Some(value) => value.clone(),
						::core::option::Option::None => match server_values.get(#field_literal) {
							::core::option::Option::Some(value) => #serde_json_crate::from_value::<#field_ty>(value.clone())
								.map_err(|error| #forms_crate::model_form::ModelFormError::FieldValidation {
									errors: ::std::collections::HashMap::from([(#field_literal.to_owned(), vec![error.to_string()])]),
								})?,
							::core::option::Option::None => #unresolved,
						},
					}
				}
			} else if server_context_names.contains(&field.name.to_string()) {
				let field_ty = &field.ty;
				quote! {
					#field_name: match server_values.get(#field_literal) {
						::core::option::Option::Some(value) => #serde_json_crate::from_value::<#field_ty>(value.clone())
							.map_err(|error| #forms_crate::model_form::ModelFormError::FieldValidation {
								errors: ::std::collections::HashMap::from([(#field_literal.to_owned(), vec![error.to_string()])]),
							})?,
						::core::option::Option::None => return ::core::result::Result::Err(
							#forms_crate::model_form::ModelFormError::MissingModelField { field: #field_literal },
						),
					}
				}
			} else {
				let default = if let Some(default) = model_form_declared_default(field) {
					default
				} else if is_auto_generated_field(field) && !field.is_fk_id_field {
					get_auto_field_default_value(field)
				} else {
					quote!(::std::default::Default::default())
				};
				quote!(#field_name: #default)
			}
		})
		.collect();
	let build_from_cleaned_body = quote! {
		::core::result::Result::Ok(Self {
			#(#build_from_cleaned_assignments,)*
		})
	};
	let apply_cleaned_fields = editable_fields
		.iter()
		.filter(|field| !field.config.primary_key)
		.map(|field| {
			let field_name = &field.name;
			quote! {
				if let ::core::option::Option::Some(value) = data.#field_name.as_ref() {
					self.#field_name = value.clone();
				}
			}
		});
	let clone_fields: Vec<_> = field_names
		.iter()
		.map(|field_name| quote!(#field_name: self.#field_name.clone()))
		.collect();
	let clone_bounds: Vec<_> = field_types
		.iter()
		.map(|field_ty| quote!(#field_ty: ::core::clone::Clone))
		.collect();
	let cleaned_getters = field_names
		.iter()
		.zip(&field_types)
		.map(|(field_name, field_ty)| {
			quote! {
				#[doc = "Returns the normalized value when this field was supplied."]
				#[doc = "This P2 cleaned-value accessor has equivalent semantics on native and WASM targets."]
				pub fn #field_name(&self) -> ::core::option::Option<&#field_ty> {
					self.#field_name.as_ref()
				}
			}
		});
	let merge_existing_into_cleaned: Vec<_> = field_names
		.iter()
		.map(|field_name| {
			quote! {
				if merged.#field_name.is_none()
					&& !supplied_fields.contains(&::core::stringify!(#field_name))
				{
					merged.#field_name = ::core::option::Option::Some(existing.#field_name.clone());
				}
			}
		})
		.collect();
	let context_name = Ident::new(
		&format!("{}ModelFormServerContext", struct_name),
		struct_name.span(),
	);
	let context_state_params: Vec<_> = server_context_fields
		.iter()
		.enumerate()
		.map(|(index, field)| Ident::new(&format!("S{index}"), field.name.span()))
		.collect();
	let context_missing_markers: Vec<_> = server_context_fields
		.iter()
		.map(|field| {
			Ident::new(
				&format!(
					"{}ModelForm{}Missing",
					struct_name,
					crate::pascal_case::to_pascal_case_with_suffix(&field.name.to_string(), "")
				),
				field.name.span(),
			)
		})
		.collect();
	let context_present_markers: Vec<_> = server_context_fields
		.iter()
		.map(|field| {
			Ident::new(
				&format!(
					"{}ModelForm{}Present",
					struct_name,
					crate::pascal_case::to_pascal_case_with_suffix(&field.name.to_string(), "")
				),
				field.name.span(),
			)
		})
		.collect();
	let context_field_names: Vec<_> = server_context_fields
		.iter()
		.map(|field| &field.name)
		.collect();
	let context_field_types: Vec<_> = server_context_fields
		.iter()
		.map(|field| &field.ty)
		.collect();
	let context_setters: Vec<_> = server_context_fields
		.iter()
		.enumerate()
		.map(|(setter_index, field)| {
			let field_name = &field.name;
			let field_ty = &field.ty;
			let missing_state = &context_missing_markers[setter_index];
			let present_state = &context_present_markers[setter_index];
			let other_params: Vec<_> = context_state_params
				.iter()
				.enumerate()
				.filter(|(index, _)| *index != setter_index)
				.map(|(_, param)| param)
				.collect();
			let impl_generics = if other_params.is_empty() {
				quote!()
			} else {
				quote!(<#(#other_params),*>)
			};
			let source_states: Vec<_> = context_state_params
				.iter()
				.enumerate()
				.map(|(index, param)| {
					if index == setter_index {
						quote!(#missing_state)
					} else {
						quote!(#param)
					}
				})
				.collect();
			let target_states: Vec<_> = context_state_params
				.iter()
				.enumerate()
				.map(|(index, param)| {
					if index == setter_index {
						quote!(#present_state)
					} else {
						quote!(#param)
					}
				})
				.collect();
			let moved_fields = context_field_names.iter().enumerate().map(|(index, name)| {
				if index == setter_index {
					quote!(#name: ::core::option::Option::Some(value))
				} else {
					quote!(#name: self.#name)
				}
			});
			quote! {
				#native_form_cfg
				impl #impl_generics #context_name<#(#source_states),*> {
					#[doc = "Sets one server-owned value and advances the native context typestate."]
					#[doc = "This is a P0 native-only API because server-owned values are unavailable on WASM clients."]
					pub fn #field_name(self, value: #field_ty) -> #context_name<#(#target_states),*> {
						#context_name {
							#(#moved_fields,)*
							_state: ::core::marker::PhantomData,
						}
					}
				}
			}
		})
		.collect();
	let direct_build_assignments: Vec<_> = field_infos
		.iter()
		.map(|field| {
			let field_name = &field.name;
			let field_literal = LitStr::new(&field.name.to_string(), field.name.span());
			if is_model_form_editable(field, field_infos) {
				let (is_optional, _) = extract_option_type(&field.ty);
				let relation_is_nullable = model_form_relation_id_is_nullable(field, field_infos);
				let nullable = field
					.config
					.null
					.unwrap_or(is_optional || relation_is_nullable);
				let required =
					!nullable && field.config.blank != Some(true) && field.config.default.is_none();
				let unresolved = if let Some(default) = model_form_declared_default(field) {
					default
				} else if is_auto_generated_field(field) && !field.is_fk_id_field {
					get_auto_field_default_value(field)
				} else if required || (!nullable && field.config.blank == Some(true)) {
					quote! {
						return ::core::result::Result::Err(
							#forms_crate::model_form::ModelFormError::MissingModelField { field: #field_literal },
						)
					}
				} else {
					quote!(::std::default::Default::default())
				};
				quote! {
					#field_name: match self.#field_name.as_ref() {
						::core::option::Option::Some(value) => value.clone(),
						::core::option::Option::None => #unresolved,
					}
				}
			} else if server_context_names.contains(&field.name.to_string()) {
				quote! {
					#field_name: context.#field_name.expect("complete model-form server context")
				}
			} else {
				let default = if let Some(default) = model_form_declared_default(field) {
					default
				} else if is_auto_generated_field(field) && !field.is_fk_id_field {
					get_auto_field_default_value(field)
				} else {
					quote!(::std::default::Default::default())
				};
				quote!(#field_name: #default)
			}
		})
		.collect();
	let cleaned_construction_output = if server_context_fields.is_empty() {
		quote! {
			#native_form_cfg
			impl<P: #core_crate::model_form::ModelFormPolicy> #cleaned_payload_name<P> {
				#[doc = "Builds a new model from this cleaned create payload."]
				#[doc = "This is a P0 native-only API because ORM model construction is unavailable on WASM clients."]
				pub fn into_model(self) -> ::core::result::Result<#struct_name, #forms_crate::model_form::ModelFormError> {
					<#struct_name as #forms_crate::model_form::FormModel>::build_from_cleaned_compat(
						&self,
						&::std::collections::HashMap::new(),
					)
				}

				#[doc = "Applies supplied cleaned values to an existing model while preserving omissions."]
				#[doc = "This is a P0 native-only API because ORM model mutation is unavailable on WASM clients."]
				pub fn apply_to(self, mut existing: #struct_name) -> ::core::result::Result<#struct_name, #forms_crate::model_form::ModelFormError> {
					<#struct_name as #forms_crate::model_form::FormModel>::apply_cleaned(&mut existing, &self)?;
					::core::result::Result::Ok(existing)
				}
			}
		}
	} else {
		let marker_definitions = context_missing_markers
			.iter()
			.zip(&context_present_markers)
			.map(|(missing, present)| {
				quote! {
					#native_form_cfg
					#[doc(hidden)]
					pub struct #missing;
					#native_form_cfg
					#[doc(hidden)]
					pub struct #present;
				}
			});
		let empty_context_fields = context_field_names
			.iter()
			.map(|name| quote!(#name: ::core::option::Option::None));
		quote! {
			#(#marker_definitions)*

			#native_form_cfg
			#[doc = "Native typestate context for server-owned values required to construct a model."]
			#[doc = "This is a P0 native-only API because server-owned values are unavailable on WASM clients."]
			pub struct #context_name<#(#context_state_params = #context_missing_markers),*> {
				#(#context_field_names: ::core::option::Option<#context_field_types>,)*
				_state: ::core::marker::PhantomData<(#(#context_state_params,)*)>,
			}

			#native_form_cfg
			impl #context_name<#(#context_missing_markers),*> {
				#[doc = "Creates an empty native server context."]
				#[doc = "Every generated setter must be called before the context can construct a model."]
				#[doc = "This is a P0 native-only API because server-owned values are unavailable on WASM clients."]
				pub fn new() -> Self {
					Self {
						#(#empty_context_fields,)*
						_state: ::core::marker::PhantomData,
					}
				}
			}

			#(#context_setters)*

			#native_form_cfg
			impl<P: #core_crate::model_form::ModelFormPolicy> #cleaned_payload_name<P> {
				#[doc = "Builds a new model using a complete native server context."]
				#[doc = "This is a P0 native-only API because ORM construction and server-owned values are unavailable on WASM clients."]
				pub fn into_model(
					self,
					context: #context_name<#(#context_present_markers),*>,
				) -> ::core::result::Result<#struct_name, #forms_crate::model_form::ModelFormError> {
					::core::result::Result::Ok(#struct_name {
						#(#direct_build_assignments,)*
					})
				}

				#[doc = "Applies supplied cleaned values to an existing model while preserving omissions."]
				#[doc = "This is a P0 native-only API because ORM model mutation is unavailable on WASM clients."]
				pub fn apply_to(self, mut existing: #struct_name) -> ::core::result::Result<#struct_name, #forms_crate::model_form::ModelFormError> {
					<#struct_name as #forms_crate::model_form::FormModel>::apply_cleaned(&mut existing, &self)?;
					::core::result::Result::Ok(existing)
				}
			}
		}
	};
	let validator_call = form_config
		.validate
		.as_ref()
		.map(|validator| quote!(#validator(&cleaned)?;))
		.unwrap_or_default();
	let merged_validator_call = form_config
		.validate
		.as_ref()
		.map(|validator| quote!(#validator(&merged)?;))
		.unwrap_or_default();
	let signature_check = form_config.validate.as_ref().map(|validator| {
		let check_name = Ident::new(
			&format!(
				"__reinhardt_check_{}_model_form_validator",
				to_snake_case(&struct_name.to_string())
			),
			struct_name.span(),
		);
		quote! {
			// The helper is intentionally unused because its body provides the compile-time assertion.
			#[allow(dead_code)]
			fn #check_name<P: #core_crate::model_form::ModelFormPolicy>() {
				let _: fn(
					&#cleaned_payload_name<P>,
				) -> ::core::result::Result<(), #core_crate::validators::ValidationErrors> = #validator;
			}
		}
	});
	let wasm_decimal_validation_arms: Vec<_> = editable_fields
		.iter()
		.filter_map(|field| {
			let ty = extract_nested_option_type(&field.ty);
			let Type::Path(type_path) = ty else {
				return None;
			};
			if type_path
				.path
				.segments
				.last()
				.is_none_or(|segment| segment.ident != "Decimal")
			{
				return None;
			}
			let field_literal = LitStr::new(&field.name.to_string(), field.name.span());
			let max_check = field.config.max_value.map(|max| {
				let max_literal = LitStr::new(&max.to_string(), field.name.span());
				quote! {
					let bound = <#ty as ::core::str::FromStr>::from_str(#max_literal)
						.expect("generated decimal maximum is valid");
					if number > bound {
						errors.add(
							descriptor.name,
							#core_crate::validators::ValidationError::Custom(
								::std::format!("Ensure this value is less than or equal to {}", #max),
							),
						);
						continue;
					}
				}
			});
			let min_check = field.config.min_value.map(|min| {
				let min_literal = LitStr::new(&min.to_string(), field.name.span());
				quote! {
					let bound = <#ty as ::core::str::FromStr>::from_str(#min_literal)
						.expect("generated decimal minimum is valid");
					if number < bound {
						errors.add(
							descriptor.name,
							#core_crate::validators::ValidationError::Custom(
								::std::format!("Ensure this value is greater than or equal to {}", #min),
							),
						);
					}
				}
			});
			Some(quote! {
				#field_literal => {
					if let ::core::result::Result::Ok(number) =
						#serde_json_crate::from_value::<#ty>(value.clone())
					{
						#max_check
						#min_check
					}
				}
			})
		})
		.collect();
	let cleaned_payload_output = quote! {
		#[doc = "A normalized generated model-form payload."]
		#[doc = "This P2 type has equivalent payload semantics on native and WASM targets."]
		#struct_vis struct #cleaned_payload_name<P: #core_crate::model_form::ModelFormPolicy> {
			#(#field_names: ::core::option::Option<#field_types>,)*
			forbidden_fields: ::std::vec::Vec<&'static str>,
			_policy: ::core::marker::PhantomData<P>,
		}

		impl<P: #core_crate::model_form::ModelFormPolicy> #cleaned_payload_name<P> {
			fn from_validated_raw(data: #payload_name<P>) -> Self {
				Self {
					#(#field_names: data.#field_names,)*
					forbidden_fields: data.forbidden_fields,
					_policy: ::core::marker::PhantomData,
				}
			}

			#[doc = "Converts this normalized payload back into its generated raw payload."]
			#[doc = "This P2 conversion is available on native and WASM targets."]
			pub fn into_raw(self) -> #payload_name<P> {
				#payload_name {
					#(#field_names: self.#field_names,)*
					forbidden_fields: self.forbidden_fields,
					_policy: ::core::marker::PhantomData,
				}
			}

			#(#cleaned_getters)*
		}

		impl<P> ::core::clone::Clone for #cleaned_payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#clone_bounds,)*
		{
			fn clone(&self) -> Self {
				Self {
					#(#clone_fields,)*
					forbidden_fields: self.forbidden_fields.clone(),
					_policy: ::core::marker::PhantomData,
				}
			}
		}
	};
	let cleaned_payload_trait_cfg = if validation_enabled {
		quote! {}
	} else {
		quote!(#[cfg(all(target_family = "wasm", target_os = "unknown"))])
	};
	let validation_output = quote! {
			#cleaned_payload_trait_cfg
			impl<P> #core_crate::model_form::ModelFormCleanedPayload
				for #cleaned_payload_name<P>
			where
				P: #core_crate::model_form::ModelFormPolicy,
			{
				type Raw = #payload_name<P>;

				fn into_raw(self) -> Self::Raw {
					#cleaned_payload_name::into_raw(self)
				}
			}

			#native_form_cfg
			impl<P> #core_crate::model_form::ModelFormValidatingPayload for #payload_name<P>
			where
				P: #core_crate::model_form::ModelFormPolicy,
				#(#payload_bounds,)*
			{
				type Cleaned = #cleaned_payload_name<P>;

				fn clean_and_validate(
					mut self,
				) -> ::core::result::Result<
					Self::Cleaned,
					#core_crate::validators::ValidationErrors,
				> {
					#forms_crate::model_form::clean_generated_payload::<#schema_name, P, _>(
						&mut self,
					)?;
					let cleaned = #cleaned_payload_name::from_validated_raw(self);
					#validator_call
					::core::result::Result::Ok(cleaned)
				}

				fn clean_and_validate_with_deferred_required_field(
					mut self,
					deferred_field: &str,
				) -> ::core::result::Result<
					Self::Cleaned,
					#core_crate::validators::ValidationErrors,
				> {
					#forms_crate::model_form::clean_generated_payload_with_deferred_required_field::<#schema_name, P, _>(
						&mut self,
						deferred_field,
					)?;
					let cleaned = #cleaned_payload_name::from_validated_raw(self);
					#validator_call
					::core::result::Result::Ok(cleaned)
				}

				fn clean_and_validate_with_deferred_required_fields(
					mut self,
					deferred_fields: &[&str],
				) -> ::core::result::Result<
					Self::Cleaned,
					#core_crate::validators::ValidationErrors,
				> {
					#forms_crate::model_form::clean_generated_payload_with_deferred_required_fields::<#schema_name, P, _>(
						&mut self,
						deferred_fields,
					)?;
					let cleaned = #cleaned_payload_name::from_validated_raw(self);
					#validator_call
					::core::result::Result::Ok(cleaned)
				}
			}

		#native_form_cfg
		impl<P> #core_crate::model_form::ModelFormUpdatingPayload for #payload_name<P>
			where
				P: #core_crate::model_form::ModelFormPolicy,
				#(#payload_bounds,)*
			{
			type Model = #struct_name;

			fn clean_and_validate_for_update(
					mut self,
				existing: &Self::Model,
				) -> ::core::result::Result<
					Self::Cleaned,
					#core_crate::validators::ValidationErrors,
				> {
					let supplied_fields =
						<Self as #core_crate::model_form::ModelFormPayload<P>>::supplied_fields(&self);
					#reject_supplied_primary_keys
					let instance_values = #serde_json_crate::to_value(existing).ok();
					#forms_crate::model_form::clean_generated_partial_payload_with_trusted_values::<#schema_name, P, _>(
						&mut self,
						instance_values.as_ref(),
					)?;
				let cleaned = #cleaned_payload_name::from_validated_raw(self);
				let mut merged = cleaned.clone();
				#(#merge_existing_into_cleaned)*
				#merged_validator_call
				::core::result::Result::Ok(cleaned)
			}
		}

		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		impl<P> #payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#payload_bounds,)*
		{
			fn __reinhardt_clean_and_validate(
				mut self,
				require_all: bool,
				run_validator: bool,
				deferred_required_fields: &[&str],
				trusted_values: ::core::option::Option<&#serde_json_crate::Value>,
			) -> ::core::result::Result<
				#cleaned_payload_name<P>,
				#core_crate::validators::ValidationErrors,
				> {
					fn json_within_depth(
						value: &#serde_json_crate::Value,
						current: usize,
					) -> bool {
						if current > 64 {
							return false;
						}
						match value {
							#serde_json_crate::Value::Array(values) => values
								.iter()
								.all(|value| json_within_depth(value, current + 1)),
							#serde_json_crate::Value::Object(values) => values
								.values()
								.all(|value| json_within_depth(value, current + 1)),
							_ => true,
						}
					}

					fn serialized_year(value: &#serde_json_crate::Value) -> ::core::option::Option<i32> {
						let raw = value.as_str()?;
						let digits_start = usize::from(raw.starts_with('+') || raw.starts_with('-'));
						let digits_end = raw[digits_start..].find('-')? + digits_start;
						raw[..digits_end].parse().ok()
					}

					fn serialized_uuid_is_valid(value: &#serde_json_crate::Value) -> bool {
						let ::core::option::Option::Some(raw) = value.as_str() else {
							return false;
						};
						let lengths = [8, 4, 4, 4, 12];
						let mut parts = raw.split('-');
						lengths.into_iter().all(|length| {
							parts.next().is_some_and(|part| {
								part.len() == length
									&& part.chars().all(|character| character.is_ascii_hexdigit())
							})
						}) && parts.next().is_none()
					}

					let mut errors = #core_crate::validators::ValidationErrors::new();
					let forbidden_fields = <Self as #core_crate::model_form::ModelFormPayload<P>>::forbidden_fields(&self);
					for descriptor in <#schema_name as #core_crate::model_form::ModelFormSchema>::fields() {
						if forbidden_fields.contains(&descriptor.name) {
							errors.add(
								descriptor.name,
								#core_crate::validators::ValidationError::Custom(
									"This field is not allowed.".to_owned(),
								),
							);
						}
					}
					if !errors.is_empty() {
						return ::core::result::Result::Err(errors);
					}

					let mut normalized = ::std::vec::Vec::new();
					for descriptor in <#schema_name as #core_crate::model_form::ModelFormSchema>::fields() {
						if !descriptor.editable || !P::allows(descriptor.name) {
							continue;
						}
						let ::core::option::Option::Some(value) =
							<Self as #core_crate::model_form::ModelFormPayload<P>>::get_json(
								&self,
								descriptor.name,
							)
						else {
						if require_all
							&& descriptor.required
							&& !deferred_required_fields.contains(&descriptor.name)
						{
							errors.add(
								descriptor.name,
								#core_crate::validators::ValidationError::Custom(
									"This field is required.".to_owned(),
								),
							);
						}
							continue;
						};
						if value.is_null() {
							if descriptor.nullable {
								continue;
							}
							if matches!(
								descriptor.kind,
								#core_crate::model_form::ModelFormFieldKind::Json
							) {
								continue;
							}
							if matches!(
								descriptor.kind,
								#core_crate::model_form::ModelFormFieldKind::Boolean
							) {
								normalized.push((descriptor.name, #serde_json_crate::Value::Bool(false)));
								continue;
							}
							if descriptor.required {
								errors.add(
									descriptor.name,
									#core_crate::validators::ValidationError::Custom(
										"This field is required.".to_owned(),
									),
								);
							}
							continue;
						}

						match descriptor.kind {
							#core_crate::model_form::ModelFormFieldKind::Text {
								min_length,
								max_length,
								..
							}
							| #core_crate::model_form::ModelFormFieldKind::Email {
								min_length,
								max_length,
							}
							| #core_crate::model_form::ModelFormFieldKind::Url {
								min_length,
								max_length,
							} => {
								let ::core::option::Option::Some(raw) = value.as_str() else {
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											"Expected string".to_owned(),
										),
									);
									continue;
								};
								let value = if descriptor.trim { raw.trim() } else { raw };
								if value.is_empty() {
									if descriptor.required {
										errors.add(
											descriptor.name,
											#core_crate::validators::ValidationError::Custom(
												"This field is required.".to_owned(),
											),
										);
									}
									if !descriptor.required {
										normalized.push((descriptor.name, #serde_json_crate::Value::String(value.to_owned())));
									}
									continue;
								}

								let length = value.chars().count();
								let mut valid = true;
								if let ::core::option::Option::Some(max) = max_length
									&& length > max
								{
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											::std::format!(
												"Ensure this value has at most {} characters (it has {})",
												max,
												length,
											),
										),
									);
									valid = false;
								} else if let ::core::option::Option::Some(min) = min_length
									&& length < min
								{
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											::std::format!(
												"Ensure this value has at least {} characters (it has {})",
												min,
												length,
											),
										),
									);
									valid = false;
								}
								if valid {
									let format_valid = match descriptor.kind {
										#core_crate::model_form::ModelFormFieldKind::Email { .. } =>
											<#core_crate::validators::EmailValidator as #core_crate::validators::Validator<str>>::validate(
												&#core_crate::validators::EmailValidator::new(),
												value,
											).is_ok(),
										#core_crate::model_form::ModelFormFieldKind::Url { .. } =>
											<#core_crate::validators::UrlValidator as #core_crate::validators::Validator<str>>::validate(
												&#core_crate::validators::UrlValidator::new(),
												value,
											).is_ok(),
										_ => true,
									};
									if !format_valid {
										let message = match descriptor.kind {
											#core_crate::model_form::ModelFormFieldKind::Email { .. } =>
												"Enter a valid email address",
											_ => "Enter a valid URL",
										};
										errors.add(
											descriptor.name,
											#core_crate::validators::ValidationError::Custom(message.to_owned()),
										);
										valid = false;
									}
								}
								if valid {
									normalized.push((descriptor.name, #serde_json_crate::Value::String(value.to_owned())));
								}
							}
							#core_crate::model_form::ModelFormFieldKind::Integer { min, max } => {
								let signed = value.as_i64();
								let unsigned = value.as_u64();
								if let ::core::option::Option::Some(max) = max
									&& (signed.is_some_and(|number| number > max)
										|| unsigned.is_some_and(|number| max < 0 || number > max as u64))
								{
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											::std::format!("Ensure this value is less than or equal to {}", max),
										),
									);
								} else if let ::core::option::Option::Some(min) = min
									&& (signed.is_some_and(|number| number < min)
										|| unsigned.is_some_and(|number| min > 0 && number < min as u64))
								{
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											::std::format!("Ensure this value is greater than or equal to {}", min),
										),
									);
								}
							}
							#core_crate::model_form::ModelFormFieldKind::Float { min, max } => {
								if let ::core::option::Option::Some(number) = value.as_f64() {
									if let ::core::option::Option::Some(max) = max
										&& number > max
									{
										errors.add(
											descriptor.name,
											#core_crate::validators::ValidationError::Custom(
												::std::format!("Ensure this value is less than or equal to {}", max),
											),
										);
									} else if let ::core::option::Option::Some(min) = min
										&& number < min
									{
										errors.add(
											descriptor.name,
											#core_crate::validators::ValidationError::Custom(
												::std::format!("Ensure this value is greater than or equal to {}", min),
											),
										);
									}
								}
							}
							#core_crate::model_form::ModelFormFieldKind::Decimal { .. } => {
								match descriptor.name {
									#(#wasm_decimal_validation_arms,)*
									_ => {}
								}
							}
							#core_crate::model_form::ModelFormFieldKind::Boolean => {
								if !value.is_boolean() {
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											"Cannot convert to boolean".to_owned(),
										),
									);
								}
							}
							#core_crate::model_form::ModelFormFieldKind::Date => {
								if !serialized_year(&value).is_some_and(|year| (1000..=9999).contains(&year)) {
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											"Enter a valid date with a 4-digit year".to_owned(),
										),
									);
								}
							}
							#core_crate::model_form::ModelFormFieldKind::DateTime
							| #core_crate::model_form::ModelFormFieldKind::NaiveDateTime => {
								if !serialized_year(&value).is_some_and(|year| (1000..=9999).contains(&year)) {
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											"Enter a year between 1000 and 9999".to_owned(),
										),
									);
								}
							}
							#core_crate::model_form::ModelFormFieldKind::Time => {
								if value.as_str().is_none() {
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											"Expected string".to_owned(),
										),
									);
								}
							}
							#core_crate::model_form::ModelFormFieldKind::Uuid => {
								if !serialized_uuid_is_valid(&value) {
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											"Enter a valid UUID.".to_owned(),
										),
									);
								}
							}
							#core_crate::model_form::ModelFormFieldKind::Json => {
								if !json_within_depth(&value, 0) {
									errors.add(
										descriptor.name,
										#core_crate::validators::ValidationError::Custom(
											"JSON structure is too deeply nested.".to_owned(),
										),
									);
								}
							}
							#core_crate::model_form::ModelFormFieldKind::File
							| #core_crate::model_form::ModelFormFieldKind::Image => {
								let valid_reference = value.as_object().is_some_and(|reference| {
									reference
										.get("path")
										.and_then(#serde_json_crate::Value::as_str)
										.is_some_and(|path| !path.is_empty())
										&& reference
											.get("storage")
											.and_then(#serde_json_crate::Value::as_str)
											.is_some_and(|storage| !storage.is_empty())
								});
								if valid_reference
									&& trusted_values
										.and_then(|values| values.get(descriptor.name))
										.is_some_and(|trusted_value| trusted_value == &value)
								{
									continue;
								}
								let message = if valid_reference {
									"Stored file references must come from the existing instance"
								} else {
									"Expected storage-backed file reference"
								};
								errors.add(
									descriptor.name,
									#core_crate::validators::ValidationError::Custom(message.to_owned()),
								);
							}
						}
					}
					if !errors.is_empty() {
						return ::core::result::Result::Err(errors);
					}

					for (field, value) in normalized {
						if let ::core::result::Result::Err(error) =
							<Self as #core_crate::model_form::ModelFormPayload<P>>::set_json(
								&mut self,
								field,
								value,
							)
						{
							errors.add(
								field,
								#core_crate::validators::ValidationError::Custom(error.to_string()),
							);
							return ::core::result::Result::Err(errors);
						}
					}
					let cleaned = #cleaned_payload_name::from_validated_raw(self);
				if run_validator {
					#validator_call
				}
				::core::result::Result::Ok(cleaned)
			}
		}

		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		impl<P> #core_crate::model_form::ModelFormValidatingPayload for #payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#payload_bounds,)*
		{
			type Cleaned = #cleaned_payload_name<P>;

			fn clean_and_validate(
				self,
			) -> ::core::result::Result<
				Self::Cleaned,
				#core_crate::validators::ValidationErrors,
				> {
					self.__reinhardt_clean_and_validate(true, true, &[], None)
				}

				fn clean_and_validate_with_deferred_required_fields(
					self,
					deferred_fields: &[&str],
				) -> ::core::result::Result<
					Self::Cleaned,
					#core_crate::validators::ValidationErrors,
				> {
					let mut errors = #core_crate::validators::ValidationErrors::new();
					for &field in deferred_fields {
						if !<#schema_name as #core_crate::model_form::ModelFormSchema>::fields()
							.iter()
							.any(|descriptor| {
								descriptor.name == field
									&& descriptor.required
									&& matches!(
										descriptor.kind,
										#core_crate::model_form::ModelFormFieldKind::File
											| #core_crate::model_form::ModelFormFieldKind::Image
									)
							})
						{
							errors.add(
								field.to_owned(),
								#core_crate::validators::ValidationError::Custom(
									"only required file or image fields may be deferred".to_owned(),
								),
							);
						}
					}
					if !errors.is_empty() {
						return ::core::result::Result::Err(errors);
					}
					self.__reinhardt_clean_and_validate(true, true, deferred_fields, None)
				}
		}

		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		impl<P> #core_crate::model_form::ModelFormUpdatingPayload for #payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#payload_bounds,)*
		{
			type Model = #struct_name;

			fn clean_and_validate_for_update(
				self,
				existing: &Self::Model,
			) -> ::core::result::Result<
				Self::Cleaned,
				#core_crate::validators::ValidationErrors,
				> {
					let supplied_fields =
						<Self as #core_crate::model_form::ModelFormPayload<P>>::supplied_fields(&self);
					#reject_supplied_primary_keys
					let instance_values = #serde_json_crate::to_value(existing).ok();
					let cleaned = self.__reinhardt_clean_and_validate(
						false,
						false,
						&[],
						instance_values.as_ref(),
					)?;
				let mut merged = cleaned.clone();
				#(#merge_existing_into_cleaned)*
				#merged_validator_call
					::core::result::Result::Ok(cleaned)
				}
			}
	};

	Ok(quote! {
		#struct_vis struct #schema_name;

		const #field_const_name: [#core_crate::model_form::ModelFormFieldDescriptor; #field_count] = [
			#(#descriptor_entries),*
		];

		impl #core_crate::model_form::ModelFormSchema for #schema_name {
			type Model = #struct_name;

			fn fields() -> &'static [#core_crate::model_form::ModelFormFieldDescriptor] {
				&#field_const_name
			}

			fn default_boolean_is_true(field: &str) -> bool {
				#default_true_boolean_body
			}

			fn relation_target_matches<T: 'static>(field: &str) -> bool {
				match field {
					#(#relation_target_match_arms,)*
					_ => false,
				}
			}
		}

		impl #schema_name {
			#(#descriptor_accessors)*
		}

		#struct_vis struct #payload_name<P: #core_crate::model_form::ModelFormPolicy> {
			#(#field_names: ::core::option::Option<#field_types>,)*
			forbidden_fields: ::std::vec::Vec<&'static str>,
			_policy: ::core::marker::PhantomData<P>,
		}

		impl<P> ::core::clone::Clone for #payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#clone_bounds,)*
		{
			fn clone(&self) -> Self {
				Self {
					#(#clone_fields,)*
					forbidden_fields: self.forbidden_fields.clone(),
					_policy: ::core::marker::PhantomData,
				}
			}
		}

		impl<P: #core_crate::model_form::ModelFormPolicy> #payload_name<P> {
			pub fn empty() -> Self {
				Self {
					#(#empty_fields,)*
					forbidden_fields: ::std::vec::Vec::new(),
					_policy: ::core::marker::PhantomData,
				}
			}

			#(#getters)*
			#(#setters)*
			#(#trusted_setters)*
		}

		impl<P: #core_crate::model_form::ModelFormPolicy> ::std::default::Default for #payload_name<P> {
			fn default() -> Self {
				Self::empty()
			}
		}

		impl<P> #core_crate::model_form::ModelFormPayload<P> for #payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#payload_bounds,)*
		{
			fn supplied_fields(&self) -> ::std::vec::Vec<&'static str> {
				let mut fields = ::std::vec::Vec::new();
				#(#supplied_fields)*
				fields
			}

			fn forbidden_fields(&self) -> &[&'static str] {
				&self.forbidden_fields
			}

			fn get_json(&self, field: &str) -> ::core::option::Option<#serde_json_crate::Value> {
				match field {
					#(#get_json_arms)*
					_ => ::core::option::Option::None,
				}
			}

			fn set_json(
				&mut self,
				field: &str,
				value: #serde_json_crate::Value,
			) -> ::core::result::Result<(), #core_crate::model_form::ModelFormPayloadError> {
				match field {
					#(#set_json_arms,)*
					_ => ::core::result::Result::Err(#core_crate::model_form::ModelFormPayloadError::UnknownField {
						field: field.to_owned(),
					}),
				}
			}
		}

		impl<P> #core_crate::model_form::NativeModelFormPayload for #payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#payload_bounds,)*
		{
			fn from_native_form_value(
				value: #serde_json_crate::Value,
			) -> ::core::result::Result<Self, #serde_json_crate::Error> {
				let value = #core_crate::model_form::normalize_native_model_form_value::<#schema_name, P>(value)?;
				#serde_json_crate::from_value(value)
			}
		}

		impl<P> #serde_crate::Serialize for #payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#serialize_bounds,)*
		{
			fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
			where
				S: #serde_crate::Serializer,
			{
				let mut map = #serde_crate::Serializer::serialize_map(serializer, ::core::option::Option::None)?;
				#(#serialize_entries)*
				#serde_crate::ser::SerializeMap::end(map)
			}
		}

		struct #visitor_name<P: #core_crate::model_form::ModelFormPolicy>(::core::marker::PhantomData<P>);

		impl<'de, P> #serde_crate::de::Visitor<'de> for #visitor_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#deserialize_bounds,)*
		{
			type Value = #payload_name<P>;

			fn expecting(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
				formatter.write_str("a model form payload object")
			}

			fn visit_map<A>(self, mut map: A) -> ::core::result::Result<Self::Value, A::Error>
			where
				A: #serde_crate::de::MapAccess<'de>,
			{
				#(#deserialize_initializers)*
				let mut forbidden_fields = ::std::vec::Vec::new();
				while let ::core::option::Option::Some(field) = map.next_key::<::std::string::String>()? {
					match field.as_str() {
						#(#deserialize_arms,)*
						_ => return ::core::result::Result::Err(<A::Error as #serde_crate::de::Error>::unknown_field(&field, &[#(#field_literals),*])),
					}
				}
				::core::result::Result::Ok(#payload_name {
					#(#field_names,)*
					forbidden_fields,
					_policy: ::core::marker::PhantomData,
				})
			}
		}

		impl<'de, P> #serde_crate::Deserialize<'de> for #payload_name<P>
		where
			P: #core_crate::model_form::ModelFormPolicy,
			#(#deserialize_bounds,)*
		{
			fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
			where
				D: #serde_crate::Deserializer<'de>,
			{
				#serde_crate::Deserializer::deserialize_map(
					deserializer,
					#visitor_name(::core::marker::PhantomData),
				)
			}
		}

		#cleaned_payload_output
		#cleaned_construction_output
		#signature_check
		#validation_output

		#native_form_cfg
		impl #forms_crate::model_form::FormModel for #struct_name {
			type Schema = #schema_name;
			type Data<P: #core_crate::model_form::ModelFormPolicy> = #payload_name<P>;
			type CleanedData<P: #core_crate::model_form::ModelFormPolicy> = #cleaned_payload_name<P>;

			fn clean_for_update<P: #core_crate::model_form::ModelFormPolicy>(
				data: Self::Data<P>,
				existing: &Self,
			) -> ::core::result::Result<
				Self::CleanedData<P>,
				#core_crate::validators::ValidationErrors,
			> {
				<#payload_name<P> as #core_crate::model_form::ModelFormUpdatingPayload>::clean_and_validate_for_update(
					data,
					existing,
				)
			}

			fn build_from_cleaned_compat<P: #core_crate::model_form::ModelFormPolicy>(
				data: &Self::CleanedData<P>,
				server_values: &::std::collections::HashMap<::std::string::String, #serde_json_crate::Value>,
			) -> ::core::result::Result<Self, #forms_crate::model_form::ModelFormError> {
				#build_from_cleaned_body
			}

			fn apply_cleaned<P: #core_crate::model_form::ModelFormPolicy>(
				&mut self,
				data: &Self::CleanedData<P>,
			) -> ::core::result::Result<(), #forms_crate::model_form::ModelFormError> {
				#(#apply_cleaned_fields)*
				::core::result::Result::Ok(())
			}

			fn set_trusted_field_json(
				&mut self,
				field: &str,
				value: #serde_json_crate::Value,
			) -> ::core::result::Result<(), #forms_crate::model_form::ModelFormError> {
				match field {
					#(#trusted_field_assignments,)*
					_ => ::core::result::Result::Err(#forms_crate::model_form::ModelFormError::FieldValidation {
						errors: ::std::collections::HashMap::from([(field.to_owned(), vec!["unknown trusted model field".to_owned()])]),
					}),
				}
			}

			fn trusted_relation_field_kind(
				field: &str,
			) -> ::core::option::Option<#core_crate::model_form::ModelFormFieldKind> {
				match field {
					#(#trusted_field_kinds,)*
					_ => ::core::option::Option::None,
				}
			}

			fn trusted_relation_field_is_required(field: &str) -> bool {
				match field {
					#(#trusted_relation_requiredness,)*
					_ => false,
				}
			}

			async fn save_with_mode(
				&mut self,
				executor: &mut dyn #orm_crate::connection::OrmExecutor,
				mode: #forms_crate::model_form::ModelFormPersistenceMode,
			) -> ::core::result::Result<(), #forms_crate::model_form::ModelFormError> {
				let manager = <Self as #orm_crate::Model>::objects();
				let result = match mode {
					#forms_crate::model_form::ModelFormPersistenceMode::Create => {
						match #orm_crate::custom_manager::CustomManager::create_with_conn_outcome(
							&manager,
							executor,
							self,
						)
						.await {
							#orm_crate::custom_manager::CreateWithConnOutcome::Created(saved) =>
								::core::result::Result::Ok(saved),
							#orm_crate::custom_manager::CreateWithConnOutcome::FailedBeforeInsert(error) =>
								::core::result::Result::Err((error, false)),
							#orm_crate::custom_manager::CreateWithConnOutcome::FailedAfterInsert(error) =>
								::core::result::Result::Err((error, true)),
						}
					}
					#forms_crate::model_form::ModelFormPersistenceMode::Update => {
						#orm_crate::custom_manager::CustomManager::update_with_conn(
							&manager,
							executor,
							self,
						)
						.await
						.map_err(|error| (error, false))
					}
				};

				match result {
					::core::result::Result::Ok(saved) => {
						*self = saved;
						::core::result::Result::Ok(())
					}
					::core::result::Result::Err((error, persisted_create)) => {
						let source = match error {
							#core_crate::exception::Error::Database(source) => source,
							#core_crate::exception::Error::DatabaseWithSource {
								database_error,
								..
							} => database_error,
							other => #core_crate::exception::DatabaseError::new(
								#core_crate::exception::DatabaseErrorKind::Query,
								other.to_string(),
							),
						};
						if persisted_create {
							::core::result::Result::Err(
								#forms_crate::model_form::ModelFormError::PersistenceAfterCreate { source },
							)
						} else {
							::core::result::Result::Err(
								#forms_crate::model_form::ModelFormError::Persistence { source },
							)
						}
					}
				}
			}
		}
	})
}

/// Resolve `get_latest_by` Rust field names to physical database columns.
fn resolve_latest_by_fields(
	field_names: &[String],
	field_infos: &[FieldInfo],
	struct_name: &syn::Ident,
) -> Result<Vec<String>> {
	if field_names.is_empty() {
		return Err(syn::Error::new_spanned(
			struct_name,
			"get_latest_by must contain at least one field",
		));
	}

	field_names
		.iter()
		.map(|field_name| {
			let (descending, rust_field_name) = field_name
				.strip_prefix('-')
				.map_or((false, field_name.as_str()), |name| (true, name));
			let field = field_infos
				.iter()
				.find(|field| field.name == rust_field_name)
				.ok_or_else(|| {
					syn::Error::new_spanned(
						struct_name,
						format!("get_latest_by references unknown field '{field_name}'"),
					)
				})?;

			if field.config.skip {
				return Err(syn::Error::new_spanned(
					struct_name,
					format!("get_latest_by cannot include skipped field '{field_name}'"),
				));
			}

			if field.rel.is_some() || is_relationship_field_type(&field.ty) {
				let message = if field.rel.as_ref().is_some_and(|relation| {
					relation.rel_type == crate::rel::RelationType::ManyToMany
				}) || is_many_to_many_field_type(&field.ty)
				{
					format!("get_latest_by cannot include many-to-many field '{field_name}'")
				} else {
					format!("get_latest_by cannot include relation field '{field_name}'")
				};
				return Err(syn::Error::new_spanned(struct_name, message));
			}

			let column = field
				.config
				.db_column
				.clone()
				.or_else(|| {
					field
						.is_fk_id_field
						.then(|| {
							let relation_name = rust_field_name
								.strip_suffix("_id")
								.expect("foreign-key ID fields always end with `_id`");
							field_infos
								.iter()
								.find(|candidate| candidate.name == relation_name)
								.and_then(|candidate| candidate.rel.as_ref())
								.and_then(|relation| relation.db_column.clone())
						})
						.flatten()
				})
				.unwrap_or_else(|| rust_field_name.to_owned());
			Ok(if descending {
				format!("-{column}")
			} else {
				column
			})
		})
		.collect()
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
///     pub const fn field_id() -> FieldRef<User, i64> {
///         unsafe { FieldRef::from_generated_model_field_with_names("id", "id") }
///     }
///     pub const fn field_name() -> FieldRef<User, String> {
///         unsafe { FieldRef::from_generated_model_field_with_names("name", "name") }
///     }
/// }
/// ```
fn generate_field_accessors(
	struct_name: &syn::Ident,
	field_infos: &[FieldInfo],
	constraints: &[ConstraintSpec],
) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();
	let primary_key_fields: Vec<_> = field_infos
		.iter()
		.filter(|field| field.config.primary_key)
		.collect();
	let mut unique_field_names: HashSet<String> = field_infos
		.iter()
		.filter(|field| field.config.unique == Some(true))
		.map(|field| field.name.to_string())
		.collect();

	if let [primary_key] = primary_key_fields.as_slice() {
		unique_field_names.insert(primary_key.name.to_string());
	}

	for constraint in constraints {
		match constraint {
			ConstraintSpec::Unique {
				fields,
				condition: None,
				..
			} if fields.len() == 1 => {
				unique_field_names.insert(fields[0].clone());
			}
			_ => {}
		}
	}

	let accessor_methods: Vec<_> = field_infos
		.iter()
		.filter(|field| !field.config.skip)
		.map(|field| {
			let field_name = &field.name;
			let logical_name = field_name.to_string();
			let field_type = &field.ty;
			let method_name = syn::Ident::new(&format!("field_{}", field_name), field_name.span());
			let column_name = field
				.config
				.db_column
				.clone()
				.unwrap_or_else(|| field_name.to_string());
			let field_constructor = if storage_field_kind(&field.ty).is_some() {
				let storage_alias = field.config.file_storage.as_deref().unwrap_or("default");
				let max_length = file_field_max_length(&field.config)
					.expect("validated FileField max_length must fit in u32")
					.to_string();
				quote! {
					#orm_crate::expressions::FieldRef::<
						#struct_name,
						#field_type,
						#orm_crate::expressions::GeneratedModelField,
					>::from_generated_model_field_with_names_and_metadata(
						#logical_name,
						#column_name,
						&[("file_storage", #storage_alias), ("file_max_length", #max_length)],
					)
				}
			} else {
				quote! {
					#orm_crate::expressions::FieldRef::<
						#struct_name,
						#field_type,
						#orm_crate::expressions::GeneratedModelField,
					>::from_generated_model_field_with_names(
						#logical_name,
						#column_name,
					)
				}
			};
			let storage_accessor = if let Some(storage_kind) = storage_field_kind(&field.ty) {
				let upload_to = field
					.config
					.upload_to
					.as_deref()
					.expect("validated storage fields always have upload_to");
				let storage_alias = field.config.file_storage.as_deref().unwrap_or("default");
				let max_length = file_field_max_length(&field.config)
					.expect("validated storage field max_length must fit in u32")
					as usize;
				match storage_kind {
					StorageFieldKind::File => {
						let cleanup = field.config.cleanup.unwrap_or(false);
						let method_name =
							syn::Ident::new(&format!("file_{}", field_name), field_name.span());
						quote! {
							/// Upload policy descriptor for this storage-backed file field.
							pub const fn #method_name() -> #orm_crate::ModelFileField<Self> {
								// SAFETY: this policy is emitted from the validated field declaration.
								unsafe {
									#orm_crate::ModelFileField::from_model_field_with_cleanup(
										stringify!(#struct_name),
										#logical_name,
										#upload_to,
										#storage_alias,
										#max_length,
										#cleanup,
									)
								}
							}
						}
					}
					StorageFieldKind::Image => {
						let method_name =
							syn::Ident::new(&format!("image_{}", field_name), field_name.span());
						let cleanup = field.config.cleanup.unwrap_or(false);
						let max_width = field
							.config
							.max_width
							.map(|value| quote! { ::core::option::Option::Some(#value) })
							.unwrap_or_else(|| quote! { ::core::option::Option::None });
						let max_height = field
							.config
							.max_height
							.map(|value| quote! { ::core::option::Option::Some(#value) })
							.unwrap_or_else(|| quote! { ::core::option::Option::None });
						quote! {
							/// Upload and validation policy for this storage-backed image field.
							pub const fn #method_name() -> #orm_crate::ModelImageField<Self> {
								// SAFETY: this policy is emitted from the validated field declaration.
								unsafe {
									#orm_crate::ModelImageField::from_model_field(
										stringify!(#struct_name),
										#logical_name,
										#upload_to,
										#storage_alias,
										#max_length,
										#cleanup,
										#max_width,
										#max_height,
									)
								}
							}
						}
					}
				}
			} else {
				quote! {}
			};

			quote! {
				/// Field accessor for type-safe field references
				///
				/// Returns a generated `FieldRef<#struct_name, #field_type>` that provides compile-time
				/// type safety for field operations.
				pub const fn #method_name() -> #orm_crate::expressions::FieldRef<
					#struct_name,
					#field_type,
					#orm_crate::expressions::GeneratedModelField,
				> {
					// SAFETY: the model macro derives both names and the Rust field type
					// from the same declared model field.
					unsafe { #field_constructor }
				}
				#storage_accessor
			}
		})
		.collect();
	let declared_field_names: HashSet<_> = field_infos
		.iter()
		.filter(|field| !field.config.skip)
		.map(|field| field.name.to_string())
		.collect();
	let relation_column_accessor_methods: Vec<_> = field_infos
		.iter()
		.filter(|field| !field.config.skip)
		.filter_map(|field| {
			let relation = field.rel.as_ref()?;
			if !matches!(
				relation.rel_type,
				crate::rel::RelationType::ForeignKey | crate::rel::RelationType::OneToOne
			) {
				return None;
			}
			let column_name = relation.db_column.as_ref()?;
			if declared_field_names.contains(column_name) {
				return None;
			}
			let generated_id_field_name = format!("{}_id", field.name);
			let field_type = &field_infos
				.iter()
				.find(|candidate| candidate.name == generated_id_field_name)?
				.ty;
			let method_name = syn::parse_str::<syn::Ident>(&format!("field_{column_name}")).ok()?;
			let doc_comment = format!("Field accessor for the `{column_name}` relation column.");

			Some(quote! {
				#[doc = #doc_comment]
				pub const fn #method_name() -> #orm_crate::expressions::FieldRef<
					#struct_name,
					#field_type,
					#orm_crate::expressions::GeneratedModelField,
				> {
					// SAFETY: the relation declaration and generated ID field provide this persisted column and type.
					unsafe {
						#orm_crate::expressions::FieldRef::<
							#struct_name,
							#field_type,
							#orm_crate::expressions::GeneratedModelField,
						>::from_generated_model_field_with_names(
							#column_name,
							#column_name,
						)
					}
				}
			})
		})
		.collect();
	let ordering_accessor_methods: Vec<_> = field_infos
		.iter()
		.filter(|field| !field.config.skip)
		.filter(|field| field.rel.is_none())
		.filter(|field| !is_relationship_field_type(&field.ty))
		.filter(|field| !is_many_to_many_field_type(&field.ty))
		.map(|field| {
			let field_name = &field.name;
			let method_name =
				syn::Ident::new(&format!("ordering_{}", field_name), field_name.span());
			let column_name = field
				.config
				.db_column
				.clone()
				.unwrap_or_else(|| field_name.to_string());

			quote! {
				/// Ordering proof for a persisted scalar model field.
				pub const fn #method_name() -> #orm_crate::expressions::OrderingField<#struct_name> {
					// SAFETY: `#[model]` emits this accessor only for a persisted scalar model field.
					unsafe { #orm_crate::expressions::OrderingField::from_model_field(#column_name) }
				}
			}
		})
		.collect();
	let unique_fields: Vec<_> = field_infos
		.iter()
		.filter(|field| !field.config.skip)
		.filter(|field| unique_field_names.contains(&field.name.to_string()))
		.collect();
	let unique_accessor_methods: Vec<_> = unique_fields
		.iter()
		.map(|field| {
			let field_name = &field.name;
			let logical_name = field_name.to_string();
			let (is_option, lookup_type) = extract_option_type(&field.ty);
			let field_name_str = field
				.config
				.db_column
				.clone()
				.unwrap_or_else(|| field_name.to_string());
			let method_name = syn::Ident::new(&format!("unique_{}", field_name), field_name.span());
			let getter_name = syn::Ident::new(
				&format!("__reinhardt_unique_get_{}", field_name),
				field_name.span(),
			);
			let getter_body = if is_option {
				quote! { model.#field_name.clone() }
			} else {
				quote! { ::core::option::Option::Some(model.#field_name.clone()) }
			};
			let unique_constructor = if storage_field_kind(&field.ty).is_some() {
				let storage_alias = field.config.file_storage.as_deref().unwrap_or("default");
				let max_length = file_field_max_length(&field.config)
					.expect("validated FileField max_length must fit in u32")
					.to_string();
				quote! {
					#orm_crate::expressions::UniqueFieldRef::from_model_field_with_names_metadata_and_getter(
						#logical_name,
						#field_name_str,
						&[("file_storage", #storage_alias), ("file_max_length", #max_length)],
						Self::#getter_name,
					)
				}
			} else {
				quote! {
					#orm_crate::expressions::UniqueFieldRef::from_model_field_with_names_and_getter(
						#logical_name,
						#field_name_str,
						Self::#getter_name,
					)
				}
			};

			quote! {
				fn #getter_name(model: &#struct_name) -> ::core::option::Option<#lookup_type> {
					#getter_body
				}

				/// Unique-field accessor for type-safe single-row lookups.
				pub const fn #method_name() -> #orm_crate::expressions::UniqueFieldRef<#struct_name, #lookup_type> {
					// SAFETY: This accessor is generated only for fields proven unique by model metadata.
					unsafe {
						#unique_constructor
					}
				}
			}
		})
		.collect();

	quote! {
		impl #struct_name {
			#(#accessor_methods)*
			#(#relation_column_accessor_methods)*
			#(#ordering_accessor_methods)*
			#(#unique_accessor_methods)*
		}
	}
}

fn generate_relation_traversal_accessors(
	struct_name: &syn::Ident,
	struct_vis: &syn::Visibility,
	field_infos: &[FieldInfo],
) -> TokenStream {
	use crate::rel::RelationType;

	let has_composite_primary_key = field_infos
		.iter()
		.filter(|field| field.config.primary_key)
		.take(2)
		.count()
		> 1;
	if has_composite_primary_key
		&& field_infos.iter().any(|field| {
			field.rel.as_ref().is_some_and(|relation| {
				relation.rel_type == RelationType::OneToMany && relation.to_field.is_none()
			})
		}) {
		return quote! {
			compile_error!("typed reverse one_to_many relations on composite primary-key models require #[rel(to_field = \"...\")]");
		};
	}
	if has_composite_primary_key
		&& field_infos.iter().any(|field| {
			field
				.rel
				.as_ref()
				.is_some_and(|relation| relation.rel_type == RelationType::ManyToMany)
		}) {
		return quote! {
			compile_error!("typed relation traversal does not support many_to_many relations on composite primary-key models");
		};
	}

	let db_crate = get_reinhardt_db_crate();
	let orm_crate = get_reinhardt_orm_crate();
	let native_cfg = quote! {
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
	};
	let wrapper_name = syn::Ident::new(&format!("{}RelationPath", struct_name), struct_name.span());

	let wrapper_field_methods: Vec<_> = field_infos
		.iter()
		.filter(|field| !field.config.skip)
		.filter(|field| field.rel.is_none())
		.filter(|field| !is_relationship_field_type(&field.ty))
		.filter(|field| !is_many_to_many_field_type(&field.ty))
		.map(|field| {
			let field_name = &field.name;
			let logical_name = field_name.to_string();
			let column_name = field
				.config
				.db_column
				.clone()
				.unwrap_or_else(|| logical_name.clone());
			let field_type = &field.ty;
			let method_name = syn::Ident::new(&format!("field_{}", field_name), field_name.span());
			let doc_comment =
				format!("Reference the `{logical_name}` field through this relation path.");

			quote! {
				#[doc = #doc_comment]
				pub fn #method_name(self) -> #orm_crate::relations::RelatedFieldRef<
					Root,
					#struct_name,
					#field_type,
					<Origin as #orm_crate::relations::RelationFieldOrigin<
						#orm_crate::expressions::GeneratedModelField,
					>>::RelatedFieldOrigin,
				>
				where
					Origin: #orm_crate::relations::RelationFieldOrigin<
						#orm_crate::expressions::GeneratedModelField,
					>,
				{
					// SAFETY: the model macro derives both names and the Rust field type
					// from the same declared model field.
					self.field(unsafe {
						#orm_crate::expressions::FieldRef::<
							#struct_name,
							#field_type,
							#orm_crate::expressions::GeneratedModelField,
						>::from_generated_model_field_with_names(
							#logical_name,
							#column_name,
						)
					})
				}
			}
		})
		.collect();

	struct RelationAccessorInfo {
		method_name: syn::Ident,
		descriptor_name: syn::Ident,
		target_ty: TokenStream,
		steps: TokenStream,
	}

	let relation_infos: Vec<_> = field_infos
		.iter()
		.filter_map(|field| {
			let rel = field.rel.as_ref()?;
			let field_name = &field.name;
			let field_name_str = field_name.to_string();
			let method_name = syn::Ident::new(&format!("rel_{}", field_name), field_name.span());
			let relation_type_name = crate::pascal_case::to_pascal_case_with_suffix(
				&field_name_str,
				"RelationDescriptor",
			);
			let descriptor_name = syn::Ident::new(
				&format!("{}{}", struct_name, relation_type_name),
				field_name.span(),
			);

			let source_table = quote! { <#struct_name as #orm_crate::Model>::table_name() };
			let source_pk = quote! { <#struct_name as #orm_crate::Model>::primary_key_column() };
			let (target_ty, steps) = match rel.rel_type {
				RelationType::ForeignKey | RelationType::OneToOne => {
					let target_ty = extract_fk_target_type(&field.ty)?;
					let source_column = rel
						.db_column
						.clone()
						.unwrap_or_else(|| format!("{}_id", field_name_str));
					let target_column = rel.to_field.as_ref().map_or_else(
						|| quote! { <#target_ty as #orm_crate::Model>::primary_key_column() },
						|field| {
							quote! {
								<#target_ty as #orm_crate::Model>::field_metadata()
									.into_iter()
									.find_map(|field_info| {
										if field_info.name == #field {
											Some(field_info.db_column.unwrap_or(field_info.name))
										} else {
											None
										}
									})
									.unwrap_or_else(|| #field.to_string())
							}
						},
					);
					let join_kind = if rel.null == Some(true) {
						quote! { #orm_crate::relations::RelationJoinKind::Left }
					} else {
						quote! { #orm_crate::relations::RelationJoinKind::Inner }
					};
					(
						quote! { #target_ty },
						quote! {
							vec![
								#orm_crate::relations::RelationStep {
									name: (#field_name_str).into(),
									source_table: (#source_table).into(),
									target_table: (<#target_ty as #orm_crate::Model>::table_name()).into(),
									source_column: (#source_column).into(),
									target_column: (#target_column).into(),
									default_join_kind: #join_kind,
									multiplicity: #orm_crate::relations::RelationMultiplicity::Single,
								}
							]
						},
					)
				}
				RelationType::OneToMany => {
					let target_path = rel.to.as_ref()?;
					let foreign_key = rel.foreign_key.as_ref()?;
					let source_column = rel.to_field.as_ref().map_or_else(
						|| quote! { <#struct_name as #orm_crate::Model>::primary_key_column() },
						|field| {
							quote! {
								<#struct_name as #orm_crate::Model>::field_metadata()
									.into_iter()
									.find_map(|field_info| {
										if field_info.name == #field {
											Some(field_info.db_column.unwrap_or(field_info.name))
										} else {
											None
										}
									})
									.unwrap_or_else(|| #field.to_string())
							}
						},
					);
					(
						quote! { #target_path },
						quote! {
							vec![
								#orm_crate::relations::RelationStep {
									name: (#field_name_str).into(),
									source_table: (#source_table).into(),
									target_table: (<#target_path as #orm_crate::Model>::table_name()).into(),
									source_column: (#source_column).into(),
									target_column: (#foreign_key).into(),
									default_join_kind: #orm_crate::relations::RelationJoinKind::Left,
									multiplicity: #orm_crate::relations::RelationMultiplicity::Multiple,
								}
							]
						},
					)
				}
				RelationType::ManyToMany => {
					let target_ty = extract_m2m_target_type(&field.ty)?;
					let target_table = quote! { <#target_ty as #orm_crate::Model>::table_name() };
					let target_pk =
						quote! { <#target_ty as #orm_crate::Model>::primary_key_column() };
					let through = rel.through.as_ref().map_or_else(
						|| {
							quote! {
								#db_crate::m2m_naming::default_through_table(
									#source_table,
									#field_name_str
								)
							}
						},
						|through| quote! { #through },
					);
					let source_field = rel.source_field.as_ref().map_or_else(
						|| {
							quote! {
								#db_crate::m2m_naming::default_m2m_columns(
									#source_table,
									#target_table
								).0
							}
						},
						|source| quote! { #source },
					);
					let target_field = rel.target_field.as_ref().map_or_else(
						|| {
							quote! {
								#db_crate::m2m_naming::default_m2m_columns(
									#source_table,
									#target_table
								).1
							}
						},
						|target| quote! { #target },
					);
					let through_name = format!("{}__through", field_name_str);
					(
						quote! { #target_ty },
						quote! {
							vec![
								#orm_crate::relations::RelationStep {
									name: (#through_name).into(),
									source_table: (#source_table).into(),
									target_table: (#through).into(),
									source_column: (#source_pk).into(),
									target_column: (#source_field).into(),
									default_join_kind: #orm_crate::relations::RelationJoinKind::Left,
									multiplicity: #orm_crate::relations::RelationMultiplicity::Multiple,
								},
								#orm_crate::relations::RelationStep {
									name: (#field_name_str).into(),
									source_table: (#through).into(),
									target_table: (#target_table).into(),
									source_column: (#target_field).into(),
									target_column: (#target_pk).into(),
									default_join_kind: #orm_crate::relations::RelationJoinKind::Left,
									multiplicity: #orm_crate::relations::RelationMultiplicity::Single,
								}
							]
						},
					)
				}
				_ => return None,
			};

			Some(RelationAccessorInfo {
				method_name,
				descriptor_name,
				target_ty: quote! { #target_ty },
				steps,
			})
		})
		.collect();

	let relation_methods: Vec<_> = relation_infos
		.iter()
		.map(|info| {
			let method_name = &info.method_name;
			let relation_name = method_name.to_string();
			let descriptor_name = &info.descriptor_name;
			let target_ty = &info.target_ty;
			let steps = &info.steps;
			let doc_comment = format!("Build a typed relation path for `{relation_name}`.");

			quote! {
				#native_cfg
				struct #descriptor_name;

				#native_cfg
				impl #orm_crate::relations::RelationDescriptor for #descriptor_name {
					type Source = #struct_name;
					type Target = #target_ty;

					fn steps() -> Vec<#orm_crate::relations::RelationStep> {
						#steps
					}
				}

				#native_cfg
				impl #struct_name {
					#[doc = #doc_comment]
					#struct_vis fn #method_name() -> #orm_crate::relations::RelationPath<
						#struct_name,
						#target_ty,
						#orm_crate::relations::GeneratedRelationPath,
					> {
						// SAFETY: This descriptor is generated from the relation metadata of this model.
						unsafe {
							#orm_crate::relations::RelationPath::<
								#struct_name,
								#target_ty,
								#orm_crate::relations::GeneratedRelationPath,
							>::from_generated_steps(
								<#descriptor_name as #orm_crate::relations::RelationDescriptor>::steps(),
							)
						}
					}
				}
			}
		})
		.collect();

	let wrapper_relation_methods: Vec<_> = relation_infos
		.iter()
		.map(|info| {
			let method_name = &info.method_name;
			let relation_name = method_name.to_string();
			let descriptor_name = &info.descriptor_name;
			let target_ty = &info.target_ty;
			let doc_comment = format!("Extend this relation path through `{relation_name}`.");

			quote! {
				#[doc = #doc_comment]
				#struct_vis fn #method_name(self) -> #orm_crate::relations::RelationPath<Root, #target_ty, Origin> {
					// SAFETY: This descriptor is generated from the relation metadata of this model.
					unsafe { self.inner.extend_generated_descriptor::<#descriptor_name, #target_ty>() }
				}
			}
		})
		.collect();
	let wrapper_doc = format!("Typed relation path wrapper for [`{struct_name}`].");
	let wrapper_new_doc = format!("Wrap a raw relation path targeting [`{struct_name}`].");
	let wrapper_optional_doc = "Treat this relation path as optional and prefer left joins.";
	let wrapper_field_doc =
		format!("Reference a [`{struct_name}`] field through this relation path.");

	quote! {
		#native_cfg
		#[doc = #wrapper_doc]
		#struct_vis struct #wrapper_name<
			Root: #orm_crate::Model,
			Origin = #orm_crate::relations::UnverifiedRelationPath,
		> {
			inner: #orm_crate::relations::RelationPath<Root, #struct_name, Origin>,
		}

		#native_cfg
		impl<Root: #orm_crate::Model, Origin> #wrapper_name<Root, Origin> {
			#[doc = #wrapper_new_doc]
			pub fn new(inner: #orm_crate::relations::RelationPath<Root, #struct_name, Origin>) -> Self {
				Self { inner }
			}

			#[doc = #wrapper_optional_doc]
			pub fn optional(self) -> Self {
				Self {
					inner: self.inner.optional(),
				}
			}

			#[doc = #wrapper_field_doc]
			pub fn field<Value, FieldOrigin>(
				self,
				field: #orm_crate::expressions::FieldRef<#struct_name, Value, FieldOrigin>,
			) -> #orm_crate::relations::RelatedFieldRef<
				Root,
				#struct_name,
				Value,
				<Origin as #orm_crate::relations::RelationFieldOrigin<FieldOrigin>>::RelatedFieldOrigin,
			>
			where
				Origin: #orm_crate::relations::RelationFieldOrigin<FieldOrigin>,
			{
				self.inner.field(field)
			}

			#(#wrapper_field_methods)*
			#(#wrapper_relation_methods)*
		}

		#native_cfg
		impl<Root: #orm_crate::Model, Origin> #orm_crate::relations::RelationPathLike for #wrapper_name<Root, Origin> {
			type Root = Root;
			type Target = #struct_name;

			fn steps(&self) -> &[#orm_crate::relations::RelationStep] {
				self.inner.steps()
			}

			fn join_kind(&self) -> #orm_crate::relations::RelationJoinKind {
				self.inner.join_kind()
			}

			fn join_kind_override(&self) -> Option<#orm_crate::relations::RelationJoinKind> {
				self.inner.join_kind_override()
			}

			fn leaf_alias(&self) -> &str {
				self.inner.leaf_alias()
			}
		}

		#native_cfg
		impl #orm_crate::relations::RelationTarget for #struct_name {
			type Path<Root: #orm_crate::Model, Origin> = #wrapper_name<Root, Origin>;

			fn wrap_relation_path<Root: #orm_crate::Model, Origin>(
				path: #orm_crate::relations::RelationPath<Root, Self, Origin>,
			) -> Self::Path<Root, Origin> {
				#wrapper_name::new(path)
			}
		}

		#(#relation_methods)*
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
					pub fn #method_name(&self) -> #orm_crate::ManyToManyAccessor<#struct_name, #target_ty> {
						#orm_crate::ManyToManyAccessor::new(
							self,
							#field_name_str
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
			let nullable = field_infos
				.iter()
				.find(|candidate| candidate.name == fk_id_field_name)
				.is_some_and(|candidate| extract_option_type(&candidate.ty).0);
			let target_column = field.rel.as_ref().and_then(|relation| {
				relation.to_field.as_ref().map(|target_field| {
					quote! {
						<#target_ty as #orm_crate::Model>::field_metadata()
							.into_iter()
							.find_map(|field_info| {
								if field_info.name == #target_field {
									Some(field_info.db_column.unwrap_or(field_info.name))
								} else {
									None
								}
							})
							.unwrap_or_else(|| #target_field.to_string())
					}
				})
			});
			let target_column = target_column.unwrap_or_else(|| {
				quote! { <#target_ty as #orm_crate::Model>::primary_key_column() }
			});
			let doc_comment = format!(
				"Load the related '{}' instance from the database",
				field_name_str
			);
			let load_foreign_key = if nullable {
				quote! {
					let Some(fk_id) = self.#fk_id_field_name() else {
						return ::core::result::Result::Ok(::core::option::Option::None);
					};
				}
			} else {
				quote! {
					let fk_id = self.#fk_id_field_name();
				}
			};

				quote! {
					#[doc = #doc_comment]
					pub async fn #method_name<E>(
						&self,
						db: &mut E
					) -> #core_crate::exception::Result<Option<#target_ty>>
					where
						E: #orm_crate::connection::OrmExecutor,
					{
					// Get FK _id value.
					#load_foreign_key

					// Query the target model through the relation's configured target field.
					<#target_ty as #orm_crate::Model>::objects()
						.filter(#orm_crate::Filter::new(
							#target_column,
							#orm_crate::FilterOperator::Eq,
							#orm_crate::FilterValue::Typed(
								<<#target_ty as #orm_crate::Model>::PrimaryKey as #orm_crate::IntoFieldValue<
									<#target_ty as #orm_crate::Model>::PrimaryKey
								>>::into_field_value(fk_id)
							)
						))
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
/// let tweets_accessor = Tweet::user_accessor().reverse(&user);
/// let tweets = tweets_accessor.all_with_conn(&mut db).await?;
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

			let db_column = field
				.rel
				.as_ref()
				.and_then(|rel| rel.db_column.clone())
				.unwrap_or_else(|| format!("{}_id", field_name_str));

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

fn is_chrono_datetime_type(ty: &Type) -> bool {
	let inner_ty = extract_nested_option_type(ty);
	let Type::Path(path) = inner_ty else {
		return false;
	};
	let segments = path.path.segments.iter().collect::<Vec<_>>();
	let [chrono_segment, datetime_segment] = segments.as_slice() else {
		return false;
	};
	if chrono_segment.ident != "chrono" || datetime_segment.ident != "DateTime" {
		return false;
	}

	matches!(
		&datetime_segment.arguments,
		PathArguments::AngleBracketed(arguments)
			if arguments.args.len() == 1
				&& matches!(arguments.args.first(), Some(GenericArgument::Type(_)))
	)
}

/// Generate getter methods for selected fields.
fn generate_getter_methods<F>(
	struct_name: &syn::Ident,
	field_infos: &[FieldInfo],
	include_field: F,
) -> TokenStream
where
	F: Fn(&FieldInfo) -> bool,
{
	let getter_methods: Vec<_> = field_infos
		.iter()
		// Exclude ForeignKey, OneToOne, and skip_getter fields
		.filter(|field| {
			!is_foreign_key_field_type(&field.ty)
				&& !is_one_to_one_field_type(&field.ty)
				&& !field.config.skip_getter
				&& include_field(field)
		})
		.map(|field| {
			let field_name = &field.name;
			let field_type = &field.ty;
			let method_name = field_name;

			// FK id fields use target-model primary-key projections. Return
			// them by value so native and WASM callers share the same API.
			if field.is_fk_id_field {
				quote! {
					#[doc = concat!("Get ", stringify!(#field_name))]
					pub fn #method_name(&self) -> #field_type {
						self.#field_name.clone()
					}
				}
			} else if is_copy_type(field_type) {
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
	let core_crate = get_reinhardt_core_crate();
	let migrations_crate = get_reinhardt_migrations_crate();
	let orm_crate = get_reinhardt_orm_crate();

	// Make all fields module-local (non-pub)
	make_fields_private(&mut input);

	let struct_name = &input.ident;
	let struct_vis = &input.vis;

	let generics = &input.generics;
	let where_clause = &generics.where_clause;

	// Parse model configuration
	let model_config = ModelConfig::from_attrs(&input.attrs, struct_name)?;
	let model_form_config = ModelFormConfig::from_attrs(&input.attrs)?;
	if model_form_config.validate.is_some() && !model_config.form {
		return Err(syn::Error::new_spanned(
			struct_name,
			"`#[form(validate = ...)]` requires `#[model(form = true)]`",
		));
	}
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
		config.validate_for_field_type(&ty)?;
		let form = FieldFormConfig::from_attrs(&field.attrs)?;
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
			form,
			serde_attrs,
			injected_relation_serde_skip,
			rel,
			is_fk_id_field,
		});
	}
	validate_model_form_trim(&field_infos, model_config.form)?;

	let latest_by_fields = model_config
		.get_latest_by
		.as_deref()
		.map(|fields| resolve_latest_by_fields(fields, &field_infos, struct_name))
		.transpose()?
		.unwrap_or_default();

	let mut structured_index_names = HashMap::new();
	for field in &field_infos {
		let Some(config) = field.config.structured_index.as_ref() else {
			continue;
		};
		if structured_index_names
			.insert(config.name.as_str(), config.name_span)
			.is_some()
		{
			return Err(syn::Error::new(
				config.name_span,
				format!(
					"duplicate structured index name `{}` within model",
					config.name
				),
			));
		}
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
					skip_info: field_info.config.skip_info,
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
		.filter(|f| is_indexable_field(f) && f.config.index.unwrap_or(false))
		.map(|field| {
			let column = fk_field_infos
				.iter()
				.find(|fk| fk.field_name == field.name)
				.map(|fk| fk.id_column_name.clone())
				.unwrap_or_else(|| {
					field
						.config
						.db_column
						.clone()
						.unwrap_or_else(|| field.name.to_string())
				});
			(column, field.config.index_condition.clone())
		})
		.collect();
	let indexed_field_columns: Vec<_> = indexed_fields
		.iter()
		.map(|(column, _)| column.clone())
		.collect();
	let indexed_field_conditions: Vec<_> = indexed_fields
		.iter()
		.map(|(_, condition)| match condition {
			Some(condition) => quote! { Some(#condition.to_string()) },
			None => quote! { None },
		})
		.collect();
	let structured_index_metadata_items: Vec<_> = field_infos
		.iter()
		.filter(|field| is_indexable_field(field))
		.filter_map(|field| {
			let config = field.config.structured_index.as_ref()?;
			let name = &config.name;
			let column = field
				.config
				.db_column
				.clone()
				.unwrap_or_else(|| field.name.to_string());
			let opclass = &config.opclass;
			let index_type = match config.method {
				StructuredIndexMethod::Hnsw => {
					let m = optional_u16_tokens(config.m);
					let ef_construction = optional_u16_tokens(config.ef_construction);
					quote! {
						#orm_crate::inspection::IndexMetadataType::Hnsw {
							m: #m,
							ef_construction: #ef_construction,
						}
					}
				}
				StructuredIndexMethod::Ivfflat => {
					let lists = optional_u32_tokens(config.lists);
					quote! {
						#orm_crate::inspection::IndexMetadataType::Ivfflat {
							lists: #lists,
						}
					}
				}
			};
			Some(quote! {
				#orm_crate::inspection::IndexInfo {
					name: #name.to_string(),
					fields: vec![#column.to_string()],
					unique: false,
					condition: None,
					index_type: Some(#index_type),
					operator_class: Some(#opclass.to_string()),
					expressions: None,
				}
			})
		})
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
	let resolve_db_column = |field_name: &str| {
		field_infos
			.iter()
			.find(|field| field.name == field_name)
			.and_then(|field| field.config.db_column.clone())
			.unwrap_or_else(|| field_name.to_string())
	};
	let unique_constraints: Vec<UniqueConstraintMetadata> = model_config
		.constraints
		.iter()
		.map(|c| match c {
			ConstraintSpec::Unique {
				fields,
				name,
				condition,
			} => UniqueConstraintMetadata {
				logical_fields: fields.clone(),
				column_names: fields
					.iter()
					.map(|field| resolve_db_column(field))
					.collect(),
				name: name.clone(),
				condition: condition.clone(),
			},
		})
		.collect();

	// Generate unique constraint names and definitions for code generation
	let unique_constraint_names: Vec<String> = unique_constraints
		.iter()
		.map(|constraint| {
			if let Some(n) = &constraint.name {
				n.clone()
			} else {
				// Auto-generate name: {table_name}_{field1}_{field2}_uniq
				format!("{}_{}_uniq", table_name, constraint.column_names.join("_"))
			}
		})
		.collect();

	let unique_constraint_definitions: Vec<String> = unique_constraints
		.iter()
		.map(|constraint| {
			let fields_str = constraint.column_names.join(", ");
			if let Some(cond) = &constraint.condition {
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
		.map(|constraint| constraint.column_names.clone())
		.collect();
	let unique_constraint_logical_field_lists: Vec<Vec<String>> = unique_constraints
		.iter()
		.map(|constraint| constraint.logical_fields.clone())
		.collect();
	let unique_constraint_conditions: Vec<TokenStream> = unique_constraints
		.iter()
		.map(|constraint| match &constraint.condition {
			Some(condition) => quote! { Some(#condition.to_string()) },
			None => quote! { None },
		})
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
	let pk_column_name = pk_fields[0]
		.config
		.db_column
		.clone()
		.unwrap_or_else(|| pk_fields[0].name.to_string());
	let primary_key_uses_zero_sentinel = !is_composite_pk
		&& !pk_is_option
		&& is_integer_primary_key_type(&pk_fields[0].ty)
		&& pk_fields[0].config.auto_increment.unwrap_or(true);

	// Generate field_metadata implementation
	let field_metadata_items = generate_field_metadata(&field_infos, &fk_field_infos)?;
	let database_field_validations = generate_database_field_validations(&field_infos);

	// Generate auto-registration code
	let registration_code = generate_registration_code(RegistrationCodeInput {
		struct_name,
		generics,
		app_label,
		table_name,
		field_infos: &field_infos,
		fk_field_infos: &fk_field_infos,
		unique_constraint_names: &unique_constraint_names,
		unique_constraint_field_lists: &unique_constraint_field_lists,
		check_constraints: &check_constraints,
	})?;

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
	let pk_filter_value_impl = if !is_composite_pk && is_integer_primary_key_type(pk_type) {
		quote! {
			fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
				#orm_crate::query::FilterValue::from(pk)
			}
		}
	} else if !is_composite_pk && is_fully_qualified_uuid_type(pk_type) {
		quote! {
			fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
				#orm_crate::query::FilterValue::Uuid(pk)
			}
		}
	} else if !is_composite_pk && is_fully_qualified_datetime_utc_type(pk_type) {
		quote! {
			fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
				#orm_crate::query::FilterValue::Timestamp(pk)
			}
		}
	} else if !is_composite_pk && is_string_type(pk_type) {
		quote! {
			fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
				#orm_crate::query::FilterValue::String(pk.to_string())
			}
		}
	} else if !is_composite_pk {
		quote! {
			fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
				#orm_crate::query::FilterValue::Typed(Self::primary_key_database_value(&pk))
			}
		}
	} else {
		quote! {}
	};
	let pk_filter_value_from_str_impl = if !is_composite_pk {
		quote! {
			fn primary_key_filter_value_from_str(
				value: &str,
			) -> #core_crate::exception::Result<#orm_crate::query::FilterValue> {
				let primary_key = match
					#orm_crate::model::deserialize_primary_key_from_database_str::<Self>(value)
				{
					Ok(primary_key) => primary_key,
					Err(_) => #orm_crate::model::deserialize_primary_key_from_str(value)
						.map_err(|_| #core_crate::exception::Error::Validation(
							format!("invalid primary key: {value}")
						))?,
				};
				Ok(Self::primary_key_filter_value(primary_key))
			}
		}
	} else {
		quote! {}
	};

	// Generate field accessor methods
	let field_accessors =
		generate_field_accessors(struct_name, &field_infos, &model_config.constraints);

	// Generate typed relation traversal methods
	let relation_traversal_accessors =
		generate_relation_traversal_accessors(struct_name, struct_vis, &field_infos);

	// Generate ManyToMany accessor methods
	let m2m_accessor_methods = generate_m2m_accessor_methods(struct_name, &field_infos);

	// Generate ForeignKey and OneToOne accessor methods
	let fk_accessor_methods = generate_fk_accessor_methods(struct_name, &field_infos);

	// Generate relationship metadata
	let relationship_metadata = generate_relationship_metadata(
		&rel_fields,
		&field_infos,
		&fk_field_infos,
		app_label,
		struct_name,
	);

	// Generate new() as zero-arg alias of build()
	let new_fn_impl = generate_new_alias(struct_name, &field_infos, &fk_id_field_names);

	// Generate typestate build() builder. new() is a zero-arg alias of build().
	// See issues #4400 and #4401.
	let build_fn_impl = generate_build_function(struct_name, &field_infos, &fk_id_field_names);

	// Generate getter/setter methods
	let shared_fk_id_getters =
		generate_getter_methods(struct_name, &field_infos, |field| field.is_fk_id_field);
	let native_getters =
		generate_getter_methods(struct_name, &field_infos, |field| !field.is_fk_id_field);
	let setters = generate_setter_methods(struct_name, &field_infos);

	// Generate static FK accessor methods for type-safe reverse relationship access
	let fk_static_accessor_methods = generate_fk_static_accessor_methods(struct_name, &field_infos);

	// Generate field selector struct for type-safe JOIN/GROUP BY/HAVING operations
	let field_selector_name =
		syn::Ident::new(&format!("{}Fields", struct_name), struct_name.span());
	let field_selector_struct = generate_field_selector_struct(struct_name, &field_infos);

	let (info_impl_generics, info_ty_generics, info_where_clause) = generics.split_for_impl();
	let model_form_primary_key_impl = if is_composite_pk {
		quote! {}
	} else if let Ok(kind) = model_form_kind(pk_fields[0]) {
		quote! {
			impl #info_impl_generics #core_crate::model_form::ModelFormPrimaryKey for #struct_name #info_ty_generics #info_where_clause {
				const FIELD_KIND: #core_crate::model_form::ModelFormFieldKind = #kind;
			}
		}
	} else {
		quote! {}
	};
	let model_form_primary_key_field_kind = if is_composite_pk {
		quote! { None }
	} else if let Ok(kind) = model_form_kind(pk_fields[0]) {
		quote! { Some(#kind) }
	} else {
		quote! { None }
	};
	let primary_key_field_names = pk_fields
		.iter()
		.map(|field| LitStr::new(&field.name.to_string(), field.name.span()));
	let model_form_primary_key_fields_impl = quote! {
		impl #info_impl_generics #core_crate::model_form::ModelFormPrimaryKeyFields for #struct_name #info_ty_generics #info_where_clause {
			fn primary_key_fields() -> &'static [&'static str] {
				&[#(#primary_key_field_names),*]
			}

			fn primary_key_field_kind() -> Option<#core_crate::model_form::ModelFormFieldKind> {
				#model_form_primary_key_field_kind
			}
		}
	};
	let info_model_impl = if model_config.server_only {
		quote! {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl #info_impl_generics #reinhardt::model_info::InfoModel for #struct_name #info_ty_generics #info_where_clause {
				type PrimaryKey = #pk_type;
				fn table_name() -> &'static str {
					#table_name
				}
			}
		}
	} else {
		quote! {
			impl #info_impl_generics #reinhardt::model_info::InfoModel for #struct_name #info_ty_generics #info_where_clause {
				type PrimaryKey = #pk_type;
				fn table_name() -> &'static str {
					#table_name
				}
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
		#model_form_primary_key_impl
		#model_form_primary_key_fields_impl
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
	let field_is_none_arms = field_infos.iter().filter_map(|field| {
		let (is_option, _) = extract_option_type(&field.ty);
		if !is_option {
			return None;
		}

		let field_name = &field.name;
		Some(quote! {
			stringify!(#field_name) => self.#field_name.is_none(),
		})
	});
	let generated_field_names: Vec<_> = field_infos
		.iter()
		.filter(|field| field.config.generated.is_some() || field.config.generated_sql.is_some())
		.flat_map(|field| {
			let rust_name = LitStr::new(&field.name.to_string(), field.name.span());
			let column_name = field.config.db_column.as_ref().and_then(|name| {
				(name != &field.name.to_string()).then(|| LitStr::new(name, field.name.span()))
			});
			std::iter::once(rust_name).chain(column_name)
		})
		.collect();
	let database_codec_fields: Vec<_> = field_infos
		.iter()
		.filter(|field| is_regular_persisted_field(field) || field.is_fk_id_field)
		.collect();
	let encode_database_fields = database_codec_fields.iter().map(|field| {
		let field_name = &field.name;
		let field_ty = &field.ty;
		let column_name = field
			.config
			.db_column
			.clone()
			.unwrap_or_else(|| field_name.to_string());
		let context_metadata = if storage_field_kind(&field.ty).is_some() {
			let storage_alias = field.config.file_storage.as_deref().unwrap_or("default");
			let max_length = file_field_max_length(&field.config)
				.expect("validated FileField max_length must fit in u32")
				.to_string();
			quote! { .with_metadata("file_storage", #storage_alias).with_metadata("file_max_length", #max_length) }
		} else {
			quote! {}
		};
		quote! {
			let context = #orm_crate::FieldCodecContext::new(
				stringify!(#struct_name),
				stringify!(#field_name),
				#column_name,
			)#context_metadata;
			<#field_ty as #orm_crate::DatabaseField>::validate_database_context(
				&self.#field_name,
				&context,
			)?;
			fields.insert(
				stringify!(#field_name).to_string(),
				<<#field_ty as #orm_crate::DatabaseField>::Storage as #orm_crate::DatabaseScalar>::into_database_value(
					<#field_ty as #orm_crate::DatabaseField>::encode_database(&self.#field_name)?
				),
			);
		}
	});
	let primary_key_database_value = if is_composite_pk {
		quote! {}
	} else {
		quote! {
			fn primary_key_database_value(
				pk: &Self::PrimaryKey,
			) -> ::core::result::Result<
				#orm_crate::DatabaseValue,
				#orm_crate::FieldCodecError,
			> {
				<#pk_type as #orm_crate::DatabaseField>::encode_database(pk).map(
					<<#pk_type as #orm_crate::DatabaseField>::Storage as #orm_crate::DatabaseScalar>::into_database_value
				)
			}
		}
	};
	let decode_database_fields = database_codec_fields.iter().map(|field| {
		let field_name = &field.name;
		let field_ty = &field.ty;
		let column_name = field
			.config
			.db_column
			.clone()
			.unwrap_or_else(|| field_name.to_string());
		let context_metadata = if storage_field_kind(&field.ty).is_some() {
			let storage_alias = field.config.file_storage.as_deref().unwrap_or("default");
			let max_length = file_field_max_length(&field.config)
				.expect("validated FileField max_length must fit in u32")
				.to_string();
			quote! { .with_metadata("file_storage", #storage_alias).with_metadata("file_max_length", #max_length) }
		} else {
			quote! {}
		};
		quote! {
			stringify!(#field_name) => {
				let storage = <<#field_ty as #orm_crate::DatabaseField>::Storage as #orm_crate::DatabaseScalar>::from_database_value(value)?;
				let context = #orm_crate::FieldCodecContext::new(
					stringify!(#struct_name),
					stringify!(#field_name),
					#column_name,
				)#context_metadata;
				let decoded = <#field_ty as #orm_crate::DatabaseField>::decode_database(storage, &context)?;
				#orm_crate::model::serialize_decoded_database_field(decoded)
			}
		}
	});
	let fixture_validation =
		generate_fixture_validation(struct_name, generics, &field_infos, &fk_field_infos);
	let model_form_output = if model_config.form {
		generate_model_form_support(struct_name, struct_vis, &field_infos, &model_form_config)?
	} else {
		quote! {}
	};
	let model_form_table_name_output = quote! {
		impl #generics #core_crate::model_form::ModelFormTableName
			for #struct_name #generics #where_clause
		{
			fn table_name() -> &'static str {
				#table_name
			}
		}
	};
	let check_constraint_field_arms = check_constraint_names.iter().map(|name| {
		quote! { #name => return Some(Vec::new()), }
	});
	let declared_unique_constraint_field_arms = unique_constraint_names
		.iter()
		.zip(unique_constraint_logical_field_lists.iter())
		.map(|(name, fields)| {
			quote! { #name => return Some(vec![#(#fields),*]), }
		});
	let declared_constraint_names = check_constraint_names
		.iter()
		.chain(unique_constraint_names.iter())
		.collect::<Vec<_>>();
	let generated_foreign_key_names = fk_field_infos.iter().map(|fk_info| {
		let column = &fk_info.id_column_name;
		quote! {
			#orm_crate::naming::foreign_key_constraint_name(Self::table_name(), #column)
		}
	});
	let foreign_key_constraint_lookups = fk_field_infos.iter().map(|fk_info| {
		let column = &fk_info.id_column_name;
		let logical_field = format!("{}_id", fk_info.field_name);
		quote! {
			if constraint == #orm_crate::naming::foreign_key_constraint_name(Self::table_name(), #column) {
				return Some(vec![#logical_field]);
			}
		}
	});
	let generated_unique_fields = field_infos
		.iter()
		.filter(|field| is_regular_persisted_field(field) && field.config.unique == Some(true))
		.map(|field| {
			(
				field
					.config
					.db_column
					.clone()
					.unwrap_or_else(|| field.name.to_string()),
				field.name.to_string(),
			)
		})
		.chain(
			fk_field_infos
				.iter()
				.filter(|fk_info| fk_info.is_one_to_one)
				.map(|fk_info| {
					(
						fk_info.id_column_name.clone(),
						format!("{}_id", fk_info.field_name),
					)
				}),
		)
		.filter(|(column, _)| {
			!unique_constraints.iter().any(|constraint| {
				constraint.column_names.len() == 1 && constraint.column_names[0] == *column
			})
		})
		.collect::<Vec<_>>();
	let generated_unique_physical_columns =
		generated_unique_fields.iter().map(|(column, _)| column);
	let generated_unique_constraint_lookups =
		generated_unique_fields.iter().map(|(column, field)| {
			quote! {
				if generated.iter().any(|(name, generated_column)| {
					name == constraint && generated_column == #column
				}) {
					return Some(vec![#field]);
				}
			}
		});

	// Generate the Model implementation
	let expanded = quote! {
			// Generate composite PK type definition if needed
			#composite_pk_type_def

			#shared_info_output

			#model_form_output

			#model_form_table_name_output

			#(
				#database_field_validations
			)*

			// Generate new() as a zero-arg alias of build()
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#new_fn_impl

			// Generate typestate build() builder (see #4400)
			#build_fn_impl

			// Generate FK id getter methods for shared native/WASM code.
			#shared_fk_id_getters

			// Generate getter methods for native-only ORM model fields.
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#native_getters

			// Generate setter methods for user-defined fields
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#setters

			// Generate field accessor methods for type-safe field references
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#field_accessors

			// Generate typed relation traversal accessors
			#relation_traversal_accessors

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

			fn primary_key_column() -> &'static str {
				#pk_column_name
			}

			fn latest_by_fields() -> &'static [&'static str] {
				&[#(#latest_by_fields),*]
			}

			fn primary_key_uses_zero_sentinel() -> bool {
				#primary_key_uses_zero_sentinel
			}

			#primary_key_database_value

			fn field_is_none(&self, field_name: &str) -> bool {
				match field_name {
					#(#field_is_none_arms)*
					_ => false,
				}
			}

			fn encode_database_fields(
				&self,
			) -> ::core::result::Result<
				::std::collections::BTreeMap<::std::string::String, #orm_crate::DatabaseValue>,
				#orm_crate::FieldCodecError,
			> {
				let mut fields = ::std::collections::BTreeMap::new();
				#(#encode_database_fields)*
				::core::result::Result::Ok(fields)
			}

			fn decode_database_field(
				field_name: &str,
				value: #orm_crate::DatabaseValue,
			) -> ::core::result::Result<#orm_crate::model::ModelFieldJsonValue, #orm_crate::FieldCodecError> {
				match field_name {
					#(#decode_database_fields,)*
					_ => value.into_json_value(),
				}
			}
			#fixture_validation

			#pk_impl

			#set_pk_impl

			#pk_filter_value_impl

			#pk_filter_value_from_str_impl

			#composite_pk_impl

			fn field_metadata() -> Vec<#orm_crate::inspection::FieldInfo> {
				vec![
					#(#field_metadata_items),*
				]
			}

			fn index_metadata() -> Vec<#orm_crate::inspection::IndexInfo> {
				vec![
					#(
						#orm_crate::inspection::IndexInfo::new(
							#migrations_crate::operations::default_index_name(
								<Self as #orm_crate::Model>::table_name(),
								#indexed_field_columns,
							),
							vec![#indexed_field_columns.to_string()],
							false,
							#indexed_field_conditions,
						),
					)*
					#(
						#structured_index_metadata_items,
					)*
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
						fields: Vec::new(),
						condition: None,
						deferrable: false,
						nulls_distinct: None,
					});
				)*
				// Unique constraints
				#(
					constraints.push(#orm_crate::inspection::ConstraintInfo {
						name: #unique_constraint_names.to_string(),
						constraint_type: #orm_crate::inspection::ConstraintType::Unique,
						definition: #unique_constraint_definitions.to_string(),
						fields: vec![#(#unique_constraint_logical_field_lists.to_string()),*],
						condition: #unique_constraint_conditions,
						deferrable: false,
						nulls_distinct: None,
					});
				)*
				constraints
			}

			fn constraint_fields(constraint: &str) -> Option<Vec<&'static str>> {
				match constraint {
					#(#check_constraint_field_arms)*
					#(#declared_unique_constraint_field_arms)*
					_ => {}
				}

				#(#foreign_key_constraint_lookups)*

				let unique_columns = vec![#(#generated_unique_physical_columns.to_string()),*];
				let mut reserved = vec![#(#declared_constraint_names.to_string()),*];
				reserved.extend([#(#generated_foreign_key_names),*]);
				reserved.extend(
					Self::field_metadata()
						.into_iter()
						.filter(|field| field.domain.is_some())
						.map(|field| #orm_crate::naming::enum_domain_constraint_name(
							Self::table_name(),
							field.db_column.as_deref().unwrap_or(&field.name),
						)),
				);
				let generated = #orm_crate::naming::generated_unique_constraint_names(
					Self::table_name(),
					&unique_columns,
					&reserved,
				);
				#(#generated_unique_constraint_lookups)*

				None
			}

			fn generated_field_names() -> &'static [&'static str] {
				&[#(#generated_field_names),*]
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

fn fixture_projection_serde_meta_is_deserialization_adapter(meta: &syn::Meta) -> bool {
	meta.path().is_ident("with")
		|| meta.path().is_ident("deserialize_with")
		|| meta.path().is_ident("bound")
}

fn fixture_projection_serde_attr(attr: &syn::Attribute) -> Option<syn::Attribute> {
	if !attr.path().is_ident("serde") {
		return None;
	}

	let syn::Meta::List(meta_list) = &attr.meta else {
		return None;
	};
	let kept = meta_list
		.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)
		.ok()?
		.into_iter()
		.filter(fixture_projection_serde_meta_is_deserialization_adapter)
		.collect::<Vec<_>>();
	if kept.is_empty() {
		return None;
	}

	Some(parse_quote! {
		#[serde(#(#kept),*)]
	})
}

fn fixture_projection_serde_attrs(field: &FieldInfo) -> Vec<syn::Attribute> {
	field
		.serde_attrs
		.iter()
		.filter_map(fixture_projection_serde_attr)
		.collect()
}

fn fixture_projection_serde_bounds(field: &FieldInfo) -> Vec<syn::Attribute> {
	field
		.serde_attrs
		.iter()
		.filter_map(|attr| {
			if !attr.path().is_ident("serde") {
				return None;
			}
			let syn::Meta::List(meta_list) = &attr.meta else {
				return None;
			};
			let bounds = meta_list
				.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)
				.ok()?
				.into_iter()
				.filter(|meta| meta.path().is_ident("bound"))
				.collect::<Vec<_>>();
			(!bounds.is_empty()).then(|| parse_quote!(#[serde(#(#bounds),*)]))
		})
		.collect()
}

fn fixture_projection_serde_deserializer(field: &FieldInfo) -> Option<TokenStream> {
	field.serde_attrs.iter().find_map(|attr| {
		if !attr.path().is_ident("serde") {
			return None;
		}

		let syn::Meta::List(meta_list) = &attr.meta else {
			return None;
		};
		meta_list
			.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)
			.ok()?
			.iter()
			.find_map(fixture_projection_serde_meta_deserializer)
	})
}

fn fixture_projection_serde_meta_deserializer(meta: &syn::Meta) -> Option<TokenStream> {
	let syn::Meta::NameValue(meta) = meta else {
		return None;
	};
	let syn::Expr::Lit(value) = &meta.value else {
		return None;
	};
	let syn::Lit::Str(path) = &value.lit else {
		return None;
	};
	let path = path.parse::<syn::Path>().ok()?;
	if meta.path.is_ident("with") {
		return Some(quote! { #path::deserialize });
	}
	if meta.path.is_ident("deserialize_with") {
		return Some(quote! { #path });
	}
	None
}

fn generate_fixture_validation(
	struct_name: &Ident,
	generics: &syn::Generics,
	field_infos: &[FieldInfo],
	fk_field_infos: &[ForeignKeyFieldInfo],
) -> TokenStream {
	let orm_crate = get_reinhardt_orm_crate();
	let mut projection_fields = Vec::new();
	let mut projection_field_names = Vec::new();
	let mut has_defaulted_fixture_field = false;
	let mut defaulted_fixture_field_validators = Vec::new();
	let mut has_required_fixture_foreign_key = false;
	let mut has_nullable_fixture_foreign_key = false;

	for field in field_infos {
		if field.config.skip
			|| field.is_fk_id_field
			|| is_relationship_field_type(&field.ty)
			|| is_many_to_many_field_type(&field.ty)
			|| is_fixture_computed_field(field)
		{
			continue;
		}

		let field_name = &field.name;
		let field_type = &field.ty;
		let is_database_generated = is_fixture_generated_field(field);
		let has_sql_default = field
			.config
			.default
			.as_ref()
			.and_then(serialize_field_default)
			.is_some();
		if storage_field_kind(field_type).is_some() {
			let validator_name = Ident::new(
				&format!("__reinhardt_validate_fixture_file_field_{field_name}"),
				field_name.span(),
			);
			let validator = LitStr::new(&validator_name.to_string(), field_name.span());
			let (is_option, _) = extract_option_type(field_type);
			let fixture_validation_type = if is_option {
				quote! { ::std::option::Option<::std::string::String> }
			} else {
				quote! { ::std::string::String }
			};
			let storage_alias = field.config.file_storage.as_deref().unwrap_or("default");
			let max_length = file_field_max_length(&field.config)
				.expect("validated FileField max_length must fit in u32")
				.to_string();
			let model_name = struct_name.to_string();
			let logical_name = field_name.to_string();
			let column_name = field
				.config
				.db_column
				.as_deref()
				.unwrap_or(logical_name.as_str());
			let validate_path = quote! {
				let file = #orm_crate::FileField::from_existing(path.as_str(), #storage_alias)
					.map_err(<D::Error as #orm_crate::serde::de::Error>::custom)?;
				let context = #orm_crate::FieldCodecContext::new(
					#model_name,
					#logical_name,
					#column_name,
				)
				.with_metadata("file_storage", #storage_alias)
				.with_metadata("file_max_length", #max_length);
				<#orm_crate::FileField as #orm_crate::DatabaseField>::validate_database_context(
					&file,
					&context,
				)
				.map_err(<D::Error as #orm_crate::serde::de::Error>::custom)?;
			};
			let validation = if is_option {
				quote! {
					for path in value.iter() {
						#validate_path
					}
				}
			} else {
				quote! {
					let path = &value;
					#validate_path
				}
			};
			defaulted_fixture_field_validators.push(quote! {
				fn #validator_name<'de, D>(
					deserializer: D,
				) -> ::std::result::Result<#fixture_validation_type, D::Error>
				where
					D: #orm_crate::serde::Deserializer<'de>,
				{
					let value = <#fixture_validation_type as #orm_crate::serde::Deserialize>::deserialize(deserializer)?;
					#validation
					Ok(value)
				}
			});
			let serde_default = if has_sql_default || is_database_generated {
				quote! { #[serde(default, deserialize_with = #validator)] }
			} else {
				quote! { #[serde(deserialize_with = #validator)] }
			};
			projection_fields.push(quote! {
				#serde_default
				#field_name: #fixture_validation_type
			});
			projection_field_names.push(field_name.clone());
			continue;
		}
		if has_sql_default {
			let serde_bounds = fixture_projection_serde_bounds(field);
			let (is_option, inner_type) = extract_option_type(field_type);
			let custom_deserializer = fixture_projection_serde_deserializer(field);
			let fixture_validation_type = if custom_deserializer.is_some() {
				quote! { #field_type }
			} else if is_option && field.config.null == Some(false) {
				quote! { #inner_type }
			} else {
				quote! { #field_type }
			};
			let validator = if let Some(deserializer) = custom_deserializer {
				let validator_name = Ident::new(
					&format!("__reinhardt_validate_defaulted_fixture_field_{field_name}"),
					field_name.span(),
				);
				let null_error_message = LitStr::new(
					&format!("fixture field '{field_name}' cannot be null"),
					field_name.span(),
				);
				let validation = if is_option && field.config.null == Some(false) {
					quote! {
						let value: #field_type = #deserializer(deserializer)?;
						if value.is_none() {
							return Err(<D::Error as #orm_crate::serde::de::Error>::custom(
								#null_error_message,
							));
						}
					}
				} else {
					quote! {
						let _: #field_type = #deserializer(deserializer)?;
					}
				};
				defaulted_fixture_field_validators.push(quote! {
					fn #validator_name<'de, D>(
						deserializer: D,
					) -> ::std::result::Result<::std::marker::PhantomData<#fixture_validation_type>, D::Error>
					where
						D: #orm_crate::serde::Deserializer<'de>,
					{
						#validation
						Ok(::std::marker::PhantomData::<#fixture_validation_type>)
					}
				});
				LitStr::new(&validator_name.to_string(), field_name.span())
			} else {
				has_defaulted_fixture_field = true;
				LitStr::new(
					"__reinhardt_validate_defaulted_fixture_field",
					field_name.span(),
				)
			};
			projection_fields.push(quote! {
				#(#serde_bounds)*
				#[serde(default, deserialize_with = #validator)]
				#field_name: ::std::marker::PhantomData<#fixture_validation_type>
			});
		} else {
			let serde_attrs = fixture_projection_serde_attrs(field);
			let serde_bounds = fixture_projection_serde_bounds(field);
			let custom_deserializer = fixture_projection_serde_deserializer(field);
			let (is_option, inner_type) = extract_option_type(field_type);
			let fixture_validation_type = if storage_field_kind(field_type).is_some() {
				if is_option {
					quote! { ::std::option::Option<::std::string::String> }
				} else {
					quote! { ::std::string::String }
				}
			} else if is_database_generated {
				quote! { ::std::option::Option<#field_type> }
			} else if is_option
				&& (field.config.null == Some(false)
					|| (field.config.primary_key && !is_fixture_generated_field(field)))
			{
				quote! { #inner_type }
			} else {
				quote! { #field_type }
			};
			let type_is_rewritten = is_database_generated
				|| (is_option
					&& (field.config.null == Some(false)
						|| (field.config.primary_key && !is_fixture_generated_field(field))));
			if let Some(deserializer) = custom_deserializer.filter(|_| type_is_rewritten) {
				let validator_name = Ident::new(
					&format!("__reinhardt_validate_fixture_field_{field_name}"),
					field_name.span(),
				);
				let validator = LitStr::new(&validator_name.to_string(), field_name.span());
				let null_error_message = LitStr::new(
					&format!("fixture field '{field_name}' cannot be null"),
					field_name.span(),
				);
				let rejects_null = is_option
					&& (field.config.null == Some(false)
						|| (field.config.primary_key && !is_fixture_generated_field(field)));
				let validation = if rejects_null {
					quote! {
						let value: #field_type = #deserializer(deserializer)?;
						if value.is_none() {
							return Err(<D::Error as #orm_crate::serde::de::Error>::custom(
								#null_error_message,
							));
						}
					}
				} else {
					quote! {
						let _: #field_type = #deserializer(deserializer)?;
					}
				};
				defaulted_fixture_field_validators.push(quote! {
					fn #validator_name<'de, D>(
						deserializer: D,
					) -> ::std::result::Result<::std::marker::PhantomData<#fixture_validation_type>, D::Error>
					where
						D: #orm_crate::serde::Deserializer<'de>,
					{
						#validation
						Ok(::std::marker::PhantomData::<#fixture_validation_type>)
					}
				});
				let serde_default = if is_database_generated {
					quote! { #[serde(default, deserialize_with = #validator)] }
				} else {
					quote! { #[serde(deserialize_with = #validator)] }
				};
				projection_fields.push(quote! {
					#(#serde_bounds)*
					#serde_default
					#field_name: ::std::marker::PhantomData<#fixture_validation_type>
				});
			} else {
				projection_fields.push(quote! {
					#(#serde_attrs)*
					#field_name: #fixture_validation_type
				});
			}
		}
		projection_field_names.push(field_name.clone());
	}

	for (index, foreign_key) in fk_field_infos.iter().enumerate() {
		let field_name = Ident::new(
			&format!("__reinhardt_fixture_foreign_key_{index}"),
			foreign_key.field_name.span(),
		);
		let column_name = LitStr::new(&foreign_key.id_column_name, foreign_key.field_name.span());
		let is_nullable = foreign_key.rel_attr.null.unwrap_or(false);
		let field_type = if is_nullable {
			quote! { ::std::option::Option<#orm_crate::FixtureValue> }
		} else {
			has_required_fixture_foreign_key = true;
			quote! { #orm_crate::FixtureValue }
		};
		let deserialize_with = if is_nullable {
			has_nullable_fixture_foreign_key = true;
			quote! { #[serde(default, deserialize_with = "__reinhardt_validate_nullable_fixture_foreign_key")] }
		} else {
			quote! { #[serde(deserialize_with = "__reinhardt_validate_required_fixture_foreign_key")] }
		};
		projection_fields.push(quote! {
			#[serde(rename = #column_name)]
			#deserialize_with
			#field_name: #field_type
		});
		projection_field_names.push(field_name);
	}

	let (_, ty_generics, where_clause) = generics.split_for_impl();
	let marker_field = if generics.params.is_empty() {
		quote! {}
	} else {
		quote! {
			#[serde(skip)]
			__reinhardt_fixture_projection_marker: ::std::marker::PhantomData<#struct_name #ty_generics>,
		}
	};
	let marker_pattern = if generics.params.is_empty() {
		quote! {}
	} else {
		quote! {
			__reinhardt_fixture_projection_marker: _,
		}
	};
	let defaulted_fixture_field_validator = if has_defaulted_fixture_field {
		quote! {
			fn __reinhardt_validate_defaulted_fixture_field<'de, D, T>(
				deserializer: D,
			) -> ::std::result::Result<::std::marker::PhantomData<T>, D::Error>
			where
				D: #orm_crate::serde::Deserializer<'de>,
				T: #orm_crate::serde::Deserialize<'de>,
			{
				let _ = <T as #orm_crate::serde::Deserialize>::deserialize(deserializer)?;
				Ok(::std::marker::PhantomData)
			}
		}
	} else {
		quote! {}
	};
	let required_fixture_foreign_key_validator = if has_required_fixture_foreign_key {
		quote! {
			fn __reinhardt_validate_required_fixture_foreign_key<'de, D>(
				deserializer: D,
			) -> ::std::result::Result<#orm_crate::FixtureValue, D::Error>
			where
				D: #orm_crate::serde::Deserializer<'de>,
			{
				let value = <#orm_crate::FixtureValue as #orm_crate::serde::Deserialize>::deserialize(deserializer)?;
				if value.is_null() || value.is_object() || value.is_array() {
					return Err(<D::Error as #orm_crate::serde::de::Error>::custom(
						"required foreign key fixture fields must be scalar identifiers",
					));
				}
				Ok(value)
			}
		}
	} else {
		quote! {}
	};
	let nullable_fixture_foreign_key_validator = if has_nullable_fixture_foreign_key {
		quote! {
			fn __reinhardt_validate_nullable_fixture_foreign_key<'de, D>(
				deserializer: D,
			) -> ::std::result::Result<::std::option::Option<#orm_crate::FixtureValue>, D::Error>
			where
				D: #orm_crate::serde::Deserializer<'de>,
			{
				let value = <::std::option::Option<#orm_crate::FixtureValue> as #orm_crate::serde::Deserialize>::deserialize(deserializer)?;
				if value.as_ref().is_some_and(|value| value.is_object() || value.is_array()) {
					return Err(<D::Error as #orm_crate::serde::de::Error>::custom(
						"nullable foreign key fixture fields must be scalar identifiers or null",
					));
				}
				Ok(value)
			}
		}
	} else {
		quote! {}
	};

	quote! {
	fn validate_fixture_fields(
			fields: &#orm_crate::FixtureFields,
		) -> ::std::result::Result<(), ::std::string::String> {
			#(#defaulted_fixture_field_validators)*
			#defaulted_fixture_field_validator
			#required_fixture_foreign_key_validator
			#nullable_fixture_foreign_key_validator

			// This projection is deserialized only to validate fixture input.
			#[allow(dead_code)]
			#[derive(#orm_crate::serde::Deserialize)]
			struct __ReinhardtFixtureProjection #generics #where_clause {
				#(#projection_fields,)*
				#marker_field
			}

			let __ReinhardtFixtureProjection {
				#(#projection_field_names: _,)*
				#marker_pattern
			} = #orm_crate::fixtures::__deserialize_fixture_projection::<
				__ReinhardtFixtureProjection #ty_generics,
			>(fields)?;
			Ok(())
		}
	}
}

/// Determine whether a field can be omitted from fixture validation because the database generates it.
fn is_fixture_generated_field(field: &FieldInfo) -> bool {
	if field.config.generated.is_some() || field.config.generated_sql.is_some() {
		return true;
	}

	if field.config.auto_increment == Some(true)
		|| (field.config.primary_key
			&& is_integer_primary_key_type(&field.ty)
			&& field.config.auto_increment.unwrap_or(true))
	{
		return true;
	}

	#[cfg(feature = "db-sqlite")]
	if field.config.autoincrement == Some(true) {
		return true;
	}

	// PostgreSQL identity metadata is only available when the macro is compiled
	// with PostgreSQL support, matching attribute parsing and model metadata generation.
	#[cfg(feature = "db-postgres")]
	{
		field.config.identity_always == Some(true) || field.config.identity_by_default == Some(true)
	}

	#[cfg(not(feature = "db-postgres"))]
	false
}

/// Determine whether a database-computed field cannot be supplied by a fixture.
fn is_fixture_computed_field(field: &FieldInfo) -> bool {
	field.config.generated.is_some() || field.config.generated_sql.is_some()
}

/// Generate FieldInfo construction for field_metadata()
fn generate_field_metadata(
	field_infos: &[FieldInfo],
	fk_field_infos: &[ForeignKeyFieldInfo],
) -> Result<Vec<TokenStream>> {
	let mut items = Vec::new();

	// Filter out non-persisted and relation-managed fields.
	let regular_fields: Vec<_> = field_infos
		.iter()
		.filter(|field| is_regular_persisted_field(field))
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
		let (_is_option, inner_ty) = extract_option_type(&field_info.ty);
		let (storage_kind, domain) = if is_builtin_model_field_type(&field_info.ty) {
			let storage_kind = builtin_storage_kind(&field_info.ty, &orm_crate)
				.map(|kind| quote! { ::core::option::Option::Some(#kind) })
				.unwrap_or_else(|| quote! { ::core::option::Option::None });
			(storage_kind, quote! { ::core::option::Option::None })
		} else {
			(
				quote! {
					::core::option::Option::Some(
						<<#inner_ty as #orm_crate::DatabaseField>::Storage as #orm_crate::DatabaseScalar>::STORAGE_KIND
					)
				},
				quote! { <#inner_ty as #orm_crate::DatabaseField>::domain() },
			)
		};

		let (is_option, _) = extract_option_type(&field_info.ty);
		let nullable = config.null.unwrap_or(is_option);
		let primary_key = config.primary_key;
		let unique = config.unique.unwrap_or(false);
		let blank = config.blank.unwrap_or(false);
		let editable = config.editable.unwrap_or(true);

		// Build attributes map
		let mut attrs = Vec::new();
		let effective_max_length = if storage_field_kind(&field_info.ty).is_some() {
			Some(u64::from(
				file_field_max_length(config)
					.expect("validated FileField max_length must fit in u32"),
			))
		} else {
			config.max_length
		};
		if let Some(max_length) = effective_max_length {
			attrs.push(quote! {
				attributes.insert(
					"max_length".to_string(),
					#orm_crate::fields::FieldKwarg::Uint(#max_length)
				);
			});
		}
		if let Some(upload_to) = config.upload_to.as_deref() {
			attrs.push(quote! {
				attributes.insert(
					"upload_to".to_string(),
					#orm_crate::fields::FieldKwarg::String(#upload_to.to_string())
				);
			});
		}
		if let Some(storage_kind) = storage_field_kind(&field_info.ty) {
			let storage_alias = config.file_storage.as_deref().unwrap_or("default");
			let cleanup = config.cleanup.unwrap_or(false);
			attrs.push(quote! {
				attributes.insert(
					"file_storage".to_string(),
					#orm_crate::fields::FieldKwarg::String(#storage_alias.to_string())
				);
			});
			attrs.push(quote! {
				attributes.insert(
					"cleanup".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(#cleanup)
				);
			});
			if storage_kind == StorageFieldKind::Image {
				if let Some(max_width) = config.max_width {
					attrs.push(quote! {
						attributes.insert(
							"max_width".to_string(),
							#orm_crate::fields::FieldKwarg::Uint(#max_width as u64)
						);
					});
				}
				if let Some(max_height) = config.max_height {
					attrs.push(quote! {
						attributes.insert(
							"max_height".to_string(),
							#orm_crate::fields::FieldKwarg::Uint(#max_height as u64)
						);
					});
				}
			}
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
		if config.auto_now == Some(true) {
			attrs.push(quote! {
				attributes.insert(
					"auto_now".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(true)
				);
			});
		}
		if config.auto_now_add == Some(true) {
			attrs.push(quote! {
				attributes.insert(
					"auto_now_add".to_string(),
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
			let generated_expr = quote! { #generated_expr }.to_string();
			attrs.push(quote! {
				attributes.insert(
					"generated".to_string(),
					#orm_crate::fields::FieldKwarg::String(#generated_expr.to_string())
				);
			});
		}
		if let Some(ref generated_sql) = config.generated_sql {
			attrs.push(quote! {
				attributes.insert(
					"generated_sql".to_string(),
					#orm_crate::fields::FieldKwarg::String(#generated_sql.to_string())
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

		let default = config
			.default
			.as_ref()
			.and_then(|value| field_default_to_metadata(value, &orm_crate))
			.map_or_else(|| quote! { None }, |value| quote! { Some(#value) });
		let db_default = default.clone();

		let item = quote! {
			{
				let mut attributes = ::std::collections::HashMap::new();
				#(#attrs)*

				#orm_crate::inspection::FieldInfo {
					name: #name.to_string(),
					field_type: #field_type_path.to_string(),
					storage_kind: #storage_kind,
					domain: #domain,
					nullable: #nullable,
					primary_key: #primary_key,
					unique: #unique,
					blank: #blank,
					editable: #editable,
					default: #default,
					db_default: #db_default,
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
		let name = format!("{}_id", fk_info.field_name);
		let db_column = if name == fk_info.id_column_name {
			quote! { None }
		} else {
			let column = &fk_info.id_column_name;
			quote! { Some(#column.to_string()) }
		};
		let target_type = &fk_info.target_type;
		let nullable = fk_info.rel_attr.null.unwrap_or(false);
		let unique = fk_info.is_one_to_one; // OneToOne fields have UNIQUE constraint
		let db_index = fk_info.rel_attr.db_index.unwrap_or(true); // FK fields are indexed by default

		// Derive both the field type and storage kind from the target primary key.
		let storage_kind = quote! {
			<<<#target_type as #orm_crate::Model>::PrimaryKey as #orm_crate::DatabaseField>::Storage as #orm_crate::DatabaseScalar>::STORAGE_KIND
		};
		let field_type_storage_kind = storage_kind.clone();

		let item = quote! {
			{
				let mut attributes = ::std::collections::HashMap::new();
				if #db_index {
					attributes.insert(
						"db_index".to_string(),
						#orm_crate::fields::FieldKwarg::Bool(true)
					);
				}
				attributes.insert(
					"relation_managed".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(true)
				);
				attributes.insert(
					"fk_id_field".to_string(),
					#orm_crate::fields::FieldKwarg::Bool(true)
				);

				#orm_crate::inspection::FieldInfo {
					name: #name.to_string(),
					field_type: #orm_crate::inspection::database_field_type_path(#field_type_storage_kind).to_string(),
					storage_kind: ::core::option::Option::Some(#storage_kind),
					domain: ::core::option::Option::None,
					nullable: #nullable,
					primary_key: false,
					unique: #unique,
					blank: false,
					editable: true,
					default: None,
					db_default: None,
					db_column: #db_column,
					choices: None,
					attributes,
				}
			}
		};

		items.push(item);
	}

	Ok(items)
}

/// Convert a literal `#[field(default = ...)]` value into inspection metadata.
///
/// The migration registration path already supports the same literal set. Keeping
/// this representation typed lets model-derived test schemas preserve SQL defaults.
fn field_default_to_metadata(expr: &syn::Expr, orm_crate: &TokenStream) -> Option<TokenStream> {
	let field_kwarg = quote! { #orm_crate::fields::FieldKwarg };

	if let syn::Expr::Unary(unary) = expr
		&& matches!(unary.op, syn::UnOp::Neg(_))
		&& let syn::Expr::Lit(literal) = unary.expr.as_ref()
	{
		match &literal.lit {
			syn::Lit::Int(value) => {
				let value = -value.base10_parse::<i64>().ok()?;
				return Some(quote! { #field_kwarg::Int(#value) });
			}
			syn::Lit::Float(value) => {
				let value = -value.base10_parse::<f64>().ok()?;
				return Some(quote! { #field_kwarg::Float(#value) });
			}
			_ => {}
		}
	}

	let syn::Expr::Lit(literal) = expr else {
		return None;
	};

	match &literal.lit {
		syn::Lit::Bool(value) => {
			let value = value.value;
			Some(quote! { #field_kwarg::Bool(#value) })
		}
		syn::Lit::Int(value) => {
			let value = value.base10_parse::<i64>().ok()?;
			Some(quote! { #field_kwarg::Int(#value) })
		}
		syn::Lit::Float(value) => {
			let value = value.base10_parse::<f64>().ok()?;
			Some(quote! { #field_kwarg::Float(#value) })
		}
		syn::Lit::Str(value) => {
			let value = value.value();
			Some(quote! { #field_kwarg::String(#value.to_string()) })
		}
		_ => None,
	}
}

struct RegistrationCodeInput<'a> {
	struct_name: &'a syn::Ident,
	generics: &'a syn::Generics,
	app_label: &'a str,
	table_name: &'a str,
	field_infos: &'a [FieldInfo],
	fk_field_infos: &'a [ForeignKeyFieldInfo],
	unique_constraint_names: &'a [String],
	unique_constraint_field_lists: &'a [Vec<String>],
	check_constraints: &'a [(String, String)],
}

/// Generate automatic registration code using ctor.
fn generate_registration_code(input: RegistrationCodeInput<'_>) -> Result<TokenStream> {
	let RegistrationCodeInput {
		struct_name,
		generics,
		app_label,
		table_name,
		field_infos,
		fk_field_infos,
		unique_constraint_names,
		unique_constraint_field_lists,
		check_constraints,
	} = input;
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
	let fixture_registration = if generics.params.is_empty() {
		quote! {
			// Register type-erased fixture handlers for dumpdata/loaddata.
			#orm_crate::fixtures::global_fixture_registry().register_model::<#struct_name>();
		}
	} else {
		quote! {}
	};

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
		let field_ty = &field_info.ty;
		let field_type = map_type_to_field_type(&field_info.ty, &field_info.config)?;
		let config = &field_info.config;
		let resolved_column = config
			.db_column
			.clone()
			.unwrap_or_else(|| field_name.clone());

		let mut params = Vec::new();
		#[cfg(feature = "db-mysql")]
		if let Some(unsigned) = config.unsigned {
			params.push(quote! { .with_param("unsigned", #unsigned.to_string()) });
		}
		if let Some(storage_kind) = storage_field_kind(&field_info.ty) {
			let upload_to = config
				.upload_to
				.as_deref()
				.expect("validated storage fields always have upload_to");
			let storage_alias = config.file_storage.as_deref().unwrap_or("default");
			let max_length = file_field_max_length(config)
				.expect("validated storage field max_length must fit in u32")
				.to_string();
			let model_field_type = storage_kind.model_field_type();
			let cleanup = config.cleanup.unwrap_or(false).to_string();
			params.push(quote! { .with_param("model_field_type", #model_field_type) });
			params.push(quote! { .with_param("upload_to", #upload_to) });
			params.push(quote! { .with_param("file_storage", #storage_alias) });
			params.push(quote! { .with_param("max_length", #max_length) });
			params.push(quote! { .with_param("cleanup", #cleanup) });
			if storage_kind == StorageFieldKind::Image {
				if let Some(max_width) = config.max_width {
					let max_width = max_width.to_string();
					params.push(quote! { .with_param("max_width", #max_width) });
				}
				if let Some(max_height) = config.max_height {
					let max_height = max_height.to_string();
					params.push(quote! { .with_param("max_height", #max_height) });
				}
			}
		}
		// Keep PostgreSQL's physical TOAST storage strategy in the migration
		// registry as its own parameter. It is intentionally separate from the
		// logical `file_storage` backend alias used by FileField.
		#[cfg(feature = "db-postgres")]
		if let Some(ref storage) = config.storage {
			let storage_str = match storage {
				StorageStrategy::Plain => "plain",
				StorageStrategy::Extended => "extended",
				StorageStrategy::External => "external",
				StorageStrategy::Main => "main",
			};
			params.push(quote! { .with_param("storage", #storage_str) });
		}
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
		if config.skip_info {
			params.push(quote! { .with_param("skip_info", "true") });
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
					// Preserve the registry identity using the final Rust path segment.
					let type_name = if let Type::Path(type_path) = ty {
						type_path
							.path
							.segments
							.last()
							.map(|segment| segment.ident.to_string())
							.unwrap_or_else(|| quote! { #ty }.to_string())
					} else {
						quote! { #ty }.to_string()
					};
					quote! {
						.with_foreign_key({
							let referenced_table = #migrations_crate::to_snake_case(#type_name);

							#migrations_crate::ForeignKeyInfo {
								referenced_table,
								referenced_column: "id".to_string(),
								on_delete: #migrations_crate::ForeignKeyAction::Cascade,
								on_update: #migrations_crate::ForeignKeyAction::Cascade,
							}
						})
						// Obtain the app label from the target's Model implementation so
						// qualified and imported types resolve to their registered app.
						.with_param("fk_target_app", <#ty as #orm_crate::Model>::app_label())
						.with_param("fk_target_model", #type_name)
					}
				}
				ForeignKeySpec::ModelName(model_name) => {
					quote! {
						.with_param("fk_target_app", #app_label)
						.with_param("fk_target_model", #model_name)
						.with_foreign_key(#migrations_crate::ForeignKeyInfo {
							referenced_table: #migrations_crate::to_snake_case(#model_name),
							referenced_column: "id".to_string(),
							on_delete: #migrations_crate::ForeignKeyAction::Cascade,
							on_update: #migrations_crate::ForeignKeyAction::Cascade,
						})
					}
				}
				ForeignKeySpec::AppModel {
					app_label,
					model_name,
				} => {
					quote! {
						.with_param("fk_target_app", #app_label)
						.with_param("fk_target_model", #model_name)
						.with_foreign_key(#migrations_crate::ForeignKeyInfo {
							referenced_table: #migrations_crate::to_snake_case(#model_name),
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

		let generated_registration = generated_column_registration(config, &migrations_crate);

		field_registrations.push(quote! {
			let field_domain = <#field_ty as #orm_crate::DatabaseField>::domain();
			metadata.add_field(
				#resolved_column.to_string(),
				#migrations_crate::model_registry::FieldMetadata::new(#field_type)
					#(#params)*
					.with_param("field_name", #field_name)
					.with_param("logical_name", #field_name)
					.with_param("rust_field_name", #field_name)
					.with_param("db_column", #resolved_column)
					.with_domain_opt(field_domain.clone())
					#generated_registration
					#fk_registration
			);
			if let Some(field_domain) = field_domain {
				metadata.add_enum_domain_constraint(#resolved_column, field_domain);
			}
		});
	}

	// Generate ManyToMany field registration code
	let mut m2m_registrations = Vec::new();
	for field_info in &m2m_fields {
		let field_name = field_info.name.to_string();
		let target_ty = extract_m2m_target_type(&field_info.ty)
			.cloned()
			.or_else(|| {
				field_info.rel.as_ref().and_then(|rel| {
					rel.to
						.clone()
						.map(|path| Type::Path(syn::TypePath { qself: None, path }))
				})
			});
		let Some(target_ty) = target_ty else {
			continue;
		};
		let target_model_name = relation_target_model_name(&target_ty);
		let target_model_label = quote! {
			format!(
				"{}.{}",
				<#target_ty as #orm_crate::Model>::app_label(),
				#target_model_name,
			)
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
					to_model: #target_model_label,
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
		let rust_field_name = fk_info.field_name.to_string();
		let logical_field_name = format!("{}_id", fk_info.field_name);
		let nullable = fk_info.rel_attr.null.unwrap_or(false);
		let unique = fk_info.is_one_to_one; // OneToOne fields have UNIQUE constraint
		let db_index = fk_info.rel_attr.db_index.unwrap_or(true); // FK fields are indexed by default
		let skip_info = if fk_info.skip_info {
			quote! { .with_param("skip_info", "true") }
		} else {
			quote! {}
		};
		let not_null_str = (!nullable).to_string();
		let unique_str = unique.to_string();
		let db_index_str = db_index.to_string();
		let target_ty = &fk_info.target_type;
		let fk_target_column = fk_info.rel_attr.to_field.as_ref().map_or_else(
			|| quote! { <#target_ty as #orm_crate::Model>::primary_key_column() },
			|target_field| {
				quote! {
					<#target_ty as #orm_crate::Model>::field_metadata()
						.into_iter()
						.find_map(|field_info| {
							if field_info.name == #target_field {
								Some(field_info.db_column.unwrap_or(field_info.name))
							} else {
								None
							}
						})
						.unwrap_or_else(|| #target_field.to_string())
				}
			},
		);
		let target_type = &fk_info.target_type;
		let referenced_column = fk_info.rel_attr.to_field.as_ref().map_or_else(
			|| quote! { <#target_type as #orm_crate::Model>::primary_key_column().to_string() },
			|target_field| {
				quote! {
					<#target_type as #orm_crate::Model>::field_metadata()
						.into_iter()
						.find_map(|field_info| {
							if field_info.name == #target_field {
								Some(field_info.db_column.unwrap_or(field_info.name))
							} else {
								None
							}
						})
						.unwrap_or_else(|| #target_field.to_string())
				}
			},
		);
		let foreign_key_action = |action| match action {
			crate::rel::CascadeAction::Cascade => {
				quote! { #migrations_crate::ForeignKeyAction::Cascade }
			}
			crate::rel::CascadeAction::SetNull => {
				quote! { #migrations_crate::ForeignKeyAction::SetNull }
			}
			crate::rel::CascadeAction::SetDefault => {
				quote! { #migrations_crate::ForeignKeyAction::SetDefault }
			}
			crate::rel::CascadeAction::Restrict => {
				quote! { #migrations_crate::ForeignKeyAction::Restrict }
			}
			crate::rel::CascadeAction::NoAction => {
				quote! { #migrations_crate::ForeignKeyAction::NoAction }
			}
		};
		let on_delete = foreign_key_action(fk_info.rel_attr.on_delete);
		let on_update = foreign_key_action(fk_info.rel_attr.on_update);

		// Extract "User" from ForeignKeyField<User>
		let target_model_name = if let Type::Path(type_path) = target_ty {
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
			quote! {
				.with_param(
					"fk_target_app",
					<#target_ty as #orm_crate::Model>::app_label(),
				)
			}
		} else {
			quote! {}
		};
		let fk_target_field_chain = fk_info.rel_attr.to_field.as_ref().map_or_else(
			|| quote! {},
			|target_field| {
				quote! {
					.with_param("fk_target_field", #target_field)
				}
			},
		);

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
					.with_param("rust_field_name", #rust_field_name)
					.with_param("not_null", #not_null_str)
					.with_param("unique", #unique_str)
					.with_param("db_index", #db_index_str)
					.with_param("logical_name", #logical_field_name)
					.with_param("db_column", #id_column_name)
					#skip_info
					.with_param("fk_target", #target_model_name)
					.with_param("fk_target_column", #fk_target_column)
					#fk_target_app_chain
					#fk_target_field_chain
					.with_foreign_key(#migrations_crate::ForeignKeyInfo {
						referenced_table: <#target_type as #orm_crate::Model>::table_name()
							.to_string(),
						referenced_column: #referenced_column,
						on_delete: #on_delete,
						on_update: #on_update,
					})
			);
		});
	}

	// Build per-constraint registration blocks for ModelMetadata.
	// Field-level CHECK constraints and model-level UNIQUE constraints both
	// feed ModelState, which is consumed by the migration autodetector.
	let mut constraint_registrations: Vec<TokenStream> = check_constraints
		.iter()
		.map(|(field_name, expression)| {
			let name = format!("{field_name}_check");
			quote! {
				metadata.add_constraint(
					#migrations_crate::ConstraintDefinition {
						name: #name.to_string(),
						constraint_type: "check".to_string(),
						fields: Vec::new(),
						expression: Some(#expression.to_string()),
						foreign_key_info: None,
					}
				);
			}
		})
		.collect();
	constraint_registrations.extend(
		unique_constraint_names
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
			}),
	);
	let index_registrations: Vec<TokenStream> = field_infos
		.iter()
		.filter(|field| is_indexable_field(field))
		.filter_map(|field| {
			let config = field.config.structured_index.as_ref()?;
			let name = &config.name;
			let column = field
				.config
				.db_column
				.clone()
				.unwrap_or_else(|| field.name.to_string());
			let opclass = &config.opclass;
			let index_type = match config.method {
				StructuredIndexMethod::Hnsw => {
					let m = optional_u16_tokens(config.m);
					let ef_construction = optional_u16_tokens(config.ef_construction);
					quote! {
						#migrations_crate::operations::IndexType::Hnsw {
							m: #m,
							ef_construction: #ef_construction,
						}
					}
				}
				StructuredIndexMethod::Ivfflat => {
					let lists = optional_u32_tokens(config.lists);
					quote! {
						#migrations_crate::operations::IndexType::Ivfflat {
							lists: #lists,
						}
					}
				}
			};
			Some(quote! {
				metadata.add_index(#migrations_crate::IndexDefinition {
					name: #name.to_string(),
					fields: vec![#column.to_string()],
					unique: false,
					where_clause: None,
					index_type: Some(#index_type),
					operator_class: Some(#opclass.to_string()),
					expressions: None,
				});
			})
		})
		.collect();
	let ordinary_index_registrations: Vec<TokenStream> = field_infos
		.iter()
		.filter(|field| is_indexable_field(field) && field.config.index == Some(true))
		.map(|field| {
			let column = fk_field_infos
				.iter()
				.find(|fk| fk.field_name == field.name)
				.map(|fk| fk.id_column_name.clone())
				.unwrap_or_else(|| {
					field
						.config
						.db_column
						.clone()
						.unwrap_or_else(|| field.name.to_string())
				});
			let name = format!("idx_{}_{}", table_name, column);
			let condition = match &field.config.index_condition {
				Some(condition) => quote! { Some(#condition.to_string()) },
				None => quote! { None },
			};
			quote! {
				let mut index = #migrations_crate::IndexDefinition::new(
					#name,
					vec![#column.to_string()],
					false,
				);
				index.where_clause = #condition;
				metadata.add_index(index);
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
			#(#ordinary_index_registrations)*
			#(#index_registrations)*

			#migrations_crate::model_registry::global_registry().register_model(metadata);

			// Register in global model registry for foreign_key resolution
			#orm_crate::registry::global_model_registry().register(
				#orm_crate::registry::ModelInfo {
					app_label: #app_label.to_string(),
					model_name: #model_name.to_string(),
					type_path: concat!(module_path!(), "::", stringify!(#struct_name)).to_string(),
					table_name: #table_name.to_string(),
				}
			);

			#fixture_registration
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
	let apps = get_reinhardt_apps_crate();
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
			quote! { #apps::registry::RelationshipType::OneToOne }
		} else {
			quote! { #apps::registry::RelationshipType::ForeignKey }
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
			#[#linkme::distributed_slice(#apps::registry::RELATIONSHIPS)]
			static #static_var_name: #apps::registry::RelationshipMetadata =
				#apps::registry::RelationshipMetadata {
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
				quote! { #apps::registry::RelationshipType::OneToOne }
			} else {
				// ForeignKey reverse is also ForeignKey (direction determined by from_model/to_model)
				quote! { #apps::registry::RelationshipType::ForeignKey }
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
				#[#linkme::distributed_slice(#apps::registry::RELATIONSHIPS)]
				static #reverse_static_var_name: #apps::registry::RelationshipMetadata =
					#apps::registry::RelationshipMetadata {
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
			#[#linkme::distributed_slice(#apps::registry::RELATIONSHIPS)]
			static #static_var_name: #apps::registry::RelationshipMetadata =
				#apps::registry::RelationshipMetadata {
					from_model: concat!(#app_label, ".", #model_name),
					to_model: #target_model_name,
					relationship_type: #apps::registry::RelationshipType::ManyToMany,
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
				#[#linkme::distributed_slice(#apps::registry::RELATIONSHIPS)]
				static #reverse_static_var_name: #apps::registry::RelationshipMetadata =
					#apps::registry::RelationshipMetadata {
						from_model: #target_model_name,
						to_model: concat!(#app_label, ".", #model_name),
						relationship_type: #apps::registry::RelationshipType::ManyToMany,
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
	let display_values: Vec<_> = pk_fields
		.iter()
		.map(|field| {
			let name = &field.name;
			let field_type = &field.ty;
			if is_chrono_datetime_type(field_type) {
				quote! { self.#name.to_rfc3339() }
			} else {
				quote! { self.#name.to_string() }
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
				write!(f, "(v2;")?;
				let mut first = true;
				#(
					if !first {
						write!(f, ", ")?;
					}
					let value = #display_values;
					write!(f, "{}={}:{}", stringify!(#field_names), value.len(), value)?;
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
///
/// Many-to-many targets are inferred from `ManyToManyField<Source, Target>` because
/// the corresponding `#[rel(many_to_many)]` attribute does not accept `to`.
fn generate_relationship_metadata(
	rel_fields: &[(Ident, RelAttribute)],
	field_infos: &[FieldInfo],
	fk_field_infos: &[ForeignKeyFieldInfo],
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

	let foreign_keys: HashMap<String, &ForeignKeyFieldInfo> = fk_field_infos
		.iter()
		.map(|field| (field.field_name.to_string(), field))
		.collect();
	let fields: HashMap<String, &FieldInfo> = field_infos
		.iter()
		.map(|field| (field.name.to_string(), field))
		.collect();

	let relation_info_items: Vec<TokenStream> = rel_fields
		.iter()
		.map(|(field_name, rel)| {
			let field_name_str = field_name.to_string();
			let foreign_key_info = foreign_keys.get(&field_name_str);
			let field_info = fields.get(&field_name_str);

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

			let related_model = foreign_key_info.map_or_else(
				|| {
					rel.to.as_ref().map_or_else(
						|| {
							field_info
								.and_then(|field| extract_m2m_target_type(&field.ty))
								.map_or_else(
									|| quote! { "" },
									|target| {
										let target = relation_target_model_name(target);
										quote! { #target }
									},
								)
						},
						|path| {
							let path_str = quote! { #path }.to_string();
							quote! { #path_str }
						},
					)
				},
				|field| {
					let target = relation_target_model_name(&field.target_type);
					let target_ty = &field.target_type;
					quote! { format!("{}.{}", <#target_ty as #orm_crate::Model>::app_label(), #target) }
				},
			);

			let back_populates = rel.related_name.as_ref().map_or_else(
				|| quote! { None },
				|name| quote! { Some(#name.to_string()) },
			);

			// For ForeignKey, the foreign key field is the field itself
			let foreign_key = match rel.rel_type {
				RelationType::ForeignKey | RelationType::OneToOne => {
					let column = foreign_key_info.map_or_else(
						|| format!("{}_id", field_name_str),
						|field| field.id_column_name.clone(),
					);
					quote! { Some(#column.to_string()) }
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

fn relation_target_model_name(ty: &Type) -> String {
	if let Type::Path(type_path) = ty
		&& let Some(segment) = type_path.path.segments.last()
	{
		return segment.ident.to_string();
	}

	quote! { #ty }.to_string()
}

/// Check if a type is Uuid or `Option<Uuid>`.
///
/// Thin projection of the shared `crate::pk_shape::pk_uuid_shape`
/// helper — see issue #4246 for why the underlying detection lives in
/// one place.
fn is_uuid_type(ty: &Type) -> bool {
	crate::pk_shape::pk_uuid_shape(ty).0
}

/// Check whether a type is explicitly `uuid::Uuid`.
fn is_fully_qualified_uuid_type(ty: &Type) -> bool {
	let (_, inner_ty) = extract_option_type(ty);
	matches!(
		inner_ty,
		Type::Path(type_path)
			if matches!(
				type_path.path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>().as_slice(),
				[uuid, uuid_type] if uuid == "uuid" && uuid_type == "Uuid"
			)
	)
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

/// Check whether a type is explicitly `chrono::DateTime<chrono::Utc>`.
///
/// The model macro cannot resolve imports or type aliases, so typed filter
/// conversion must only be generated for the unambiguous chrono spelling.
fn is_fully_qualified_datetime_utc_type(ty: &Type) -> bool {
	let (_, inner_ty) = extract_option_type(ty);
	let Type::Path(datetime_path) = inner_ty else {
		return false;
	};
	let [chrono_segment, datetime_segment] =
		datetime_path.path.segments.iter().collect::<Vec<_>>()[..]
	else {
		return false;
	};
	if chrono_segment.ident != "chrono" || datetime_segment.ident != "DateTime" {
		return false;
	}
	let PathArguments::AngleBracketed(arguments) = &datetime_segment.arguments else {
		return false;
	};
	let Some(GenericArgument::Type(Type::Path(utc_path))) = arguments.args.first() else {
		return false;
	};
	matches!(
		utc_path.path.segments.iter().collect::<Vec<_>>().as_slice(),
		[chrono_segment, utc_segment]
			if chrono_segment.ident == "chrono" && utc_segment.ident == "Utc"
	)
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

fn is_indexable_field(field: &FieldInfo) -> bool {
	!field.config.skip
		&& !field.is_fk_id_field
		&& !is_many_to_many_field_type(&field.ty)
		&& !field
			.rel
			.as_ref()
			.is_some_and(|rel| matches!(rel.rel_type, crate::rel::RelationType::ManyToMany))
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
	if config.generated.is_some() || config.generated_sql.is_some() {
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
			nullable: bool,
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
					nullable: extract_option_type(&f.ty).0,
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
				nullable: extract_option_type(&id_field_info.ty).0,
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
				nullable,
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
				let expr = if *nullable {
					quote! { ::core::option::Option::Some(value.into_primary_key()) }
				} else {
					quote! { value.into_primary_key() }
				};
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
			let field_name_str = field
				.config
				.db_column
				.clone()
				.unwrap_or_else(|| field_name.to_string());
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
	NullableRelation { target_ty: TokenStream },
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

fn relation_info_serde_meta_is_safe(meta: &syn::Meta) -> bool {
	let Some(name) = meta.path().get_ident().map(ToString::to_string) else {
		return false;
	};

	matches!(
		name.as_str(),
		"skip_serializing" | "rename" | "alias" | "flatten"
	)
}

fn relation_info_serde_meta_name(meta: &syn::Meta) -> Option<String> {
	meta.path().get_ident().map(ToString::to_string)
}

fn relation_info_serde_attr(
	attr: &syn::Attribute,
	injected_relation_serde_skip: bool,
) -> Option<syn::Attribute> {
	if !attr.path().is_ident("serde") {
		return Some(attr.clone());
	}

	let syn::Meta::List(meta_list) = &attr.meta else {
		return Some(attr.clone());
	};

	let nested = meta_list
		.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)
		.ok()?;
	let mut keeps_skip_serializing = false;
	let mut needs_info_default = false;
	let mut kept = Vec::new();
	for meta in nested {
		if injected_relation_serde_skip
			&& matches!(&meta, syn::Meta::Path(path) if path.is_ident("skip"))
		{
			continue;
		}

		let name = relation_info_serde_meta_name(&meta);
		if matches!(name.as_deref(), Some("skip_deserializing" | "default")) {
			needs_info_default = true;
			continue;
		}

		if relation_info_serde_meta_is_safe(&meta) {
			keeps_skip_serializing |= matches!(name.as_deref(), Some("skip_serializing"));
			kept.push(meta);
		}
	}

	if keeps_skip_serializing && needs_info_default {
		kept.push(parse_quote!(default));
	}

	if kept.is_empty() {
		return None;
	}

	Some(parse_quote! {
		#[serde(#(#kept),*)]
	})
}

fn relation_info_serde_attrs(field: &FieldInfo) -> Vec<syn::Attribute> {
	field
		.serde_attrs
		.iter()
		.filter_map(|attr| relation_info_serde_attr(attr, field.injected_relation_serde_skip))
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
	let fk_id_to_field: HashMap<String, &ForeignKeyFieldInfo> = fk_field_infos
		.iter()
		.map(|fk| (format!("{}_id", fk.field_name), fk))
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
			let nullable = field_infos
				.iter()
				.find(|candidate| candidate.name == id_name)
				.is_some_and(|candidate| extract_option_type(&candidate.ty).0);
			let ty = if nullable {
				quote! { ::core::option::Option<#reinhardt::model_info::RelationInfo<#target_ty>> }
			} else {
				quote! { #reinhardt::model_info::RelationInfo<#target_ty> }
			};
			let setter_kind = if nullable {
				InfoSetterKind::NullableRelation {
					target_ty: target_ty.clone(),
				}
			} else {
				InfoSetterKind::Relation {
					target_ty: target_ty.clone(),
				}
			};
			let from_model = if nullable {
				quote! {
					#name: model.#id_name.map(#reinhardt::model_info::RelationInfo::new),
				}
			} else {
				quote! {
					#name: #reinhardt::model_info::RelationInfo::new(model.#id_name),
				}
			};
			info_fields.push(InfoFieldSpec {
				name: name.clone(),
				ty,
				serde_attrs: relation_info_serde_attrs(f),
				validate_attrs: Vec::new(),
				setter_kind,
				from_model,
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
			if let Some(fk) = fk_id_to_field.get(&name_str)
				&& info_fields.iter().any(|inf| inf.name == fk.field_name)
			{
				let relation_name = &fk.field_name;
				if extract_option_type(&f.ty).0 {
					quote! { #name: info.#relation_name.map(#reinhardt::model_info::RelationInfo::into_id), }
				} else {
					quote! { #name: info.#relation_name.into_id(), }
				}
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
				InfoSetterKind::NullableRelation { target_ty } => {
					quote! {
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						#[allow(missing_docs)]
						impl<#(#free_state_params),*> #builder_name<#(#input_states),*> {
							pub fn #name(
								mut self,
								value: ::core::option::Option<<#target_ty as #reinhardt::model_info::InfoModel>::PrimaryKey>,
							) -> #builder_name<#(#output_states),*> {
								self.#name = ::core::option::Option::Some(
									value.map(#reinhardt::model_info::RelationInfo::new)
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
	use rstest::rstest;

	fn generated_composite_display(output: TokenStream, composite_name: &str) -> String {
		let file: syn::File = syn::parse2(output).expect("generated model output should parse");
		file.items
			.into_iter()
			.find_map(|item| {
				let syn::Item::Impl(item) = item else {
					return None;
				};
				let self_name = match item.self_ty.as_ref() {
					syn::Type::Path(path) => path.path.segments.last()?.ident.to_string(),
					_ => return None,
				};
				let trait_name = item
					.trait_
					.as_ref()
					.and_then(|(_, path, _)| path.segments.last())
					.map(|segment| segment.ident.to_string());
				(self_name == composite_name && trait_name.as_deref() == Some("Display"))
					.then(|| item.to_token_stream().to_string())
			})
			.expect("generated composite primary keys should implement Display")
	}

	fn generated_primary_key_filter_value(output: &TokenStream) -> String {
		let output = output.to_string();
		let start = output
			.find("fn primary_key_filter_value")
			.expect("typed primary keys should override the filter conversion");
		let function = &output[start..];
		let end = function
			.find('}')
			.expect("generated filter conversion should have a function body")
			+ 1;
		function[..end].to_string()
	}

	#[cfg(any(feature = "db-postgres", feature = "db-mysql", feature = "db-sqlite"))]
	#[rstest::rstest]
	fn explicit_char_field_type_preserves_length() {
		let migrations_crate = quote! { reinhardt_db::migrations };

		let field_type = map_explicit_field_type("char(2)", &migrations_crate)
			.expect("CHAR field type should parse");

		assert_eq!(
			field_type.to_string(),
			"reinhardt_db :: migrations :: FieldType :: Char (2u32)"
		);
	}

	#[test]
	#[cfg(not(feature = "pgvector"))]
	fn vector_named_custom_fields_are_not_claimed_without_pgvector() {
		let custom_vector: Type = parse_quote! { Vector<3> };

		assert_eq!(vector_dimensions(&custom_vector).unwrap(), None);
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn vector_dimensions_require_the_pgvector_feature() {
		let vector: Type = parse_quote! { Vector<3> };

		assert_eq!(vector_dimensions(&vector).unwrap(), Some(3));
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn hnsw_validation_uses_pgvector_defaults_for_omitted_options() {
		let attrs = vec![parse_quote! {
			#[field(index(
				name = "documents_embedding_hnsw",
				method = "hnsw",
				opclass = "vector_l2_ops",
				m = 100
			))]
		}];
		let error = FieldConfig::from_attrs(&attrs)
			.expect_err("the default ef_construction must constrain explicit m");

		assert_eq!(
			error.to_string(),
			"HNSW vector index option `ef_construction` must be at least twice `m`"
		);
	}

	#[test]
	fn builtin_storage_kind_distinguishes_byte_and_array_vectors() {
		let orm_crate = quote! { orm };
		let bytes: Type = parse_quote! { Vec<u8> };
		let strings: Type = parse_quote! { Vec<String> };

		assert_eq!(
			builtin_storage_kind(&bytes, &orm_crate)
				.expect("Vec<u8> should have byte storage")
				.to_string(),
			quote! { orm::DatabaseStorageKind::Bytes }.to_string()
		);
		assert_eq!(
			builtin_storage_kind(&strings, &orm_crate)
				.expect("Vec<String> should retain JSON row metadata")
				.to_string(),
			quote! { orm::DatabaseStorageKind::Json }.to_string()
		);
	}

	#[test]
	fn builtin_storage_kind_recognizes_decimal_fields() {
		let orm_crate = quote! { orm };
		let decimal: Type = parse_quote! { rust_decimal::Decimal };

		assert_eq!(
			builtin_storage_kind(&decimal, &orm_crate)
				.expect("Decimal should have decimal storage")
				.to_string(),
			quote! { orm::DatabaseStorageKind::Decimal }.to_string()
		);
	}

	#[test]
	fn hash_map_fields_use_json_storage_consistently() {
		let orm_crate = quote! { orm };
		let migrations_crate = get_reinhardt_migrations_crate();
		let hash_map: Type = parse_quote! { std::collections::HashMap<String, String> };
		let config = FieldConfig::default();

		assert_eq!(
			field_type_to_metadata_string(&hash_map, &config)
				.expect("HashMap metadata should generate")
				.to_string(),
			quote! { "reinhardt.orm.models.JsonField" }.to_string()
		);
		assert_eq!(
			map_type_to_field_type(&hash_map, &config)
				.expect("HashMap migration type should generate")
				.to_string(),
			quote! { #migrations_crate::FieldType::Jsonb }.to_string()
		);

		assert_eq!(
			builtin_storage_kind(&hash_map, &orm_crate)
				.expect("HashMap should retain JSON row metadata")
				.to_string(),
			quote! { orm::DatabaseStorageKind::Json }.to_string()
		);
	}

	#[test]
	fn test_generated_schema_expr_validation_accepts_reconstructable_expr() {
		let expr: syn::Expr = parse_quote! {
			SchemaExpr::concat([SchemaExpr::col("first_name"), SchemaExpr::val(" "), SchemaExpr::col("last_name")])
				.cast(ColumnType::String(Some(201)))
		};

		assert!(validate_generated_schema_expr(&expr).is_ok());
	}

	#[test]
	fn test_generated_schema_expr_validation_accepts_custom_cast_type_string() {
		let expr: syn::Expr = parse_quote! {
			SchemaExpr::col("email").cast(ColumnType::Custom("CITEXT".to_string()))
		};

		assert!(validate_generated_schema_expr(&expr).is_ok());
	}

	#[test]
	fn test_generated_schema_expr_validation_rejects_empty_coalesce() {
		let array_expr: syn::Expr = parse_quote! {
			SchemaExpr::coalesce([])
		};
		let vec_expr: syn::Expr = parse_quote! {
			SchemaExpr::coalesce(vec![])
		};

		assert!(validate_generated_schema_expr(&array_expr).is_err());
		assert!(validate_generated_schema_expr(&vec_expr).is_err());
	}

	#[test]
	fn test_generated_schema_expr_validation_rejects_unsupported_builder() {
		let expr: syn::Expr = parse_quote! {
			build_full_name_expr()
		};

		let error = validate_generated_schema_expr(&expr)
			.expect_err("unsupported builders must be rejected");

		assert_eq!(
			error.to_string(),
			"generated expects a reconstructable SchemaExpr expression; supported forms are SchemaExpr::col(...), SchemaExpr::val(...), SchemaExpr::concat([...]), SchemaExpr::coalesce([...]), and chained .binary(...) or .cast(...) calls. Use generated_sql = \"...\" for raw SQL or unsupported expression builders."
		);
	}

	#[test]
	fn test_generated_field_validation_rejects_auto_increment() {
		let attrs = vec![parse_quote! {
			#[field(
				generated = SchemaExpr::col("name"),
				generated_stored = true,
				auto_increment = true
			)]
		}];

		let config = FieldConfig::from_attrs(&attrs).expect("field config should parse");
		let error = config
			.validate()
			.expect_err("generated auto-increment must be rejected");

		assert_eq!(
			error.to_string(),
			"Generated columns cannot be auto-incrementing"
		);
	}

	#[test]
	fn test_generated_integer_primary_key_rejects_implicit_auto_increment() {
		let attrs = vec![parse_quote! {
			#[field(
				primary_key = true,
				generated = SchemaExpr::col("name"),
				generated_stored = true
			)]
		}];
		let ty: Type = parse_quote! { i64 };

		let config = FieldConfig::from_attrs(&attrs).expect("field config should parse");
		let error = config
			.validate_for_field_type(&ty)
			.expect_err("generated integer primary keys imply auto-increment by default");

		assert_eq!(
			error.to_string(),
			"Generated columns cannot be auto-incrementing"
		);
	}

	#[test]
	fn test_model_marks_scalar_integer_auto_increment_primary_key_zero_sentinel() {
		let input = quote! {
			#[model(app_label = "test", table_name = "scalar_users", info = false)]
			struct ScalarUser {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 120)]
				name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("scalar integer primary key model must generate")
			.to_string();

		assert!(output.contains("fn primary_key_uses_zero_sentinel () -> bool { true }"));
	}

	#[rstest::rstest]
	fn test_model_routes_primary_key_values_through_database_field_codec() {
		let input = quote! {
			#[model(app_label = "test", table_name = "external_users", info = false)]
			struct ExternalUser {
				#[field(primary_key = true, max_length = 64)]
				external_id: String,
				#[field(max_length = 120)]
				name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("string primary key model must generate")
			.to_string();
		let compact = output.split_whitespace().collect::<String>();

		assert!(compact.contains("fnprimary_key_database_value(pk:&Self::PrimaryKey"));
		assert!(compact.contains("<Stringas"));
		assert!(compact.contains("DatabaseField>::encode_database(pk)"));
		assert!(compact.contains("DatabaseScalar>::into_database_value"));
	}

	#[test]
	fn test_model_disables_zero_sentinel_for_non_auto_increment_primary_key() {
		let input = quote! {
			#[model(app_label = "test", table_name = "manual_users", info = false)]
			struct ManualUser {
				#[field(primary_key = true, auto_increment = false)]
				id: i64,
				#[field(max_length = 120)]
				name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("manual integer primary key model must generate")
			.to_string();

		assert!(output.contains("fn primary_key_uses_zero_sentinel () -> bool { false }"));
	}

	#[cfg(feature = "db-sqlite")]
	#[test]
	fn test_generated_non_integer_primary_key_rejects_on_sqlite() {
		let attrs = vec![parse_quote! {
			#[field(
				primary_key = true,
				generated_sql = "lower(name)",
				generated_stored = true
			)]
		}];
		let ty: Type = parse_quote! { String };

		let config = FieldConfig::from_attrs(&attrs).expect("field config should parse");
		let error = config
			.validate_for_field_type(&ty)
			.expect_err("SQLite generated primary keys must be rejected");

		assert_eq!(
			error.to_string(),
			"SQLite generated columns cannot be primary keys"
		);
	}

	#[cfg(all(feature = "db-mysql", not(feature = "db-sqlite")))]
	#[test]
	fn test_generated_virtual_primary_key_rejects_on_mysql() {
		let attrs = vec![parse_quote! {
			#[field(
				primary_key = true,
				generated_sql = "lower(name)",
				generated_virtual = true
			)]
		}];
		let ty: Type = parse_quote! { String };

		let config = FieldConfig::from_attrs(&attrs).expect("field config should parse");
		let error = config
			.validate_for_field_type(&ty)
			.expect_err("MySQL virtual generated primary keys must be rejected");

		assert_eq!(
			error.to_string(),
			"MySQL virtual generated columns cannot be primary keys"
		);
	}

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
	fn test_partial_index_metadata_uses_relation_column_and_condition() {
		let input = quote! {
			#[model(app_label = "auth", table_name = "auth_tokens", info = false)]
			struct Token {
				#[field(primary_key = true)]
				id: i64,
				#[field(index = true, condition = "consumed_at IS NULL")]
				#[rel(foreign_key, db_index = false)]
				user: ForeignKeyField<User>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("partial index metadata must generate")
			.to_string()
			.split_whitespace()
			.collect::<String>();

		let extract = |start: &str, end: &str| {
			let fragment = output
				.get(output.find(start).expect("generated fragment must exist")..)
				.expect("generated fragment bounds must be valid");
			let end_offset = fragment
				.find(end)
				.expect("generated fragment terminator must exist")
				+ end.len();
			&fragment[..end_offset]
		};

		assert_eq!(
			extract(
				"IndexInfo::new(",
				"Some(\"consumed_atISNULL\".to_string()),),"
			),
			"IndexInfo::new(::reinhardt::db::migrations::operations::default_index_name(<Selfas::reinhardt::db::orm::Model>::table_name(),\"user_id\",),vec![\"user_id\".to_string()],false,Some(\"consumed_atISNULL\".to_string()),),"
		);
		assert_eq!(
			extract("letmutindex=", "metadata.add_index(index);"),
			"letmutindex=::reinhardt::db::migrations::IndexDefinition::new(\"idx_auth_tokens_user_id\",vec![\"user_id\".to_string()],false,);index.where_clause=Some(\"consumed_atISNULL\".to_string());metadata.add_index(index);"
		);
	}

	#[test]
	fn test_partial_index_condition_requires_index() {
		let attrs = vec![parse_quote! {
			#[field(condition = "consumed_at IS NULL")]
		}];

		let config = FieldConfig::from_attrs(&attrs).expect("condition must parse");
		let error = config
			.validate()
			.expect_err("condition without index must be rejected");

		assert_eq!(error.to_string(), "condition requires index = true");
	}

	#[test]
	fn sql_expression_keyword_check_uses_token_boundaries() {
		assert!(validate_sql_expression("LAST_UPDATE IS NULL", "condition").is_ok());
		assert!(validate_sql_expression("UPDATE users", "condition").is_err());
		assert!(validate_sql_expression("operation = 'DELETE'", "condition").is_ok());
		assert!(validate_sql_expression("status = 'UPDATE'", "condition").is_ok());
	}

	#[test]
	fn partial_index_condition_rejects_blank_values() {
		let attrs = vec![parse_quote! {
			#[field(index = true, condition = "   ")]
		}];

		let error = FieldConfig::from_attrs(&attrs).expect_err("blank condition must be rejected");
		assert_eq!(error.to_string(), "condition must not be blank");
	}

	#[test]
	fn test_file_field_policy_defaults_and_metadata() {
		let attrs = vec![parse_quote! {
			#[field(upload_to = "avatars/%Y/%m/%d")]
		}];
		let config = FieldConfig::from_attrs(&attrs).expect("file field config should parse");
		let ty: Type = parse_quote! { Option<db::orm::FileField> };

		config
			.validate_for_field_type(&ty)
			.expect("default storage alias and max length should be accepted");
		assert_eq!(file_field_max_length(&config).unwrap(), 100);
		assert_eq!(storage_field_kind(&ty), Some(StorageFieldKind::File));
		assert_eq!(
			validate_file_upload_template("avatars/%Y/%m/%d").unwrap(),
			18
		);
		assert_eq!(file_template_component_structure("CO%M").unwrap(), "CO34");
		assert_eq!(validate_file_upload_template("CO%M").unwrap(), 4);
		assert!(validate_file_upload_template("COM1").is_err());
		assert!(validate_file_upload_template("avatars:daily").is_err());
	}

	#[test]
	fn test_file_field_descriptor_preserves_disabled_cleanup() {
		let input = quote! {
			#[model(app_label = "media", table_name = "media_assets")]
			struct Asset {
				#[field(primary_key = true)]
				id: i64,
				#[field(upload_to = "files", cleanup = false)]
				file: db::orm::FileField,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("file-field model registration must generate")
			.to_string()
			.replace(' ', "");

		assert!(
			output.contains(
				"from_model_field_with_cleanup(stringify!(Asset),\"file\",\"files\",\"default\",100usize,false,)"
			),
			"generated FileField descriptor must retain cleanup=false: {output}"
		);
	}

	#[test]
	fn test_image_field_policy_metadata_and_descriptor() {
		let input = quote! {
			#[model(app_label = "media", table_name = "media_assets")]
			struct Asset {
				#[field(primary_key = true)]
				id: i64,
				#[field(
					upload_to = "images/%Y/%m/%d",
					file_storage = "media",
					cleanup = false,
					max_width = 800,
					max_height = 600
				)]
				image: db::orm::ImageField,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("image-field model registration must generate")
			.to_string()
			.replace(' ', "");

		for expected in [
			"pubconstfnimage_image()->",
			"ModelImageField<Self>",
			"with_param(\"model_field_type\",\"image\")",
			"with_param(\"upload_to\",\"images/%Y/%m/%d\")",
			"with_param(\"file_storage\",\"media\")",
			"with_param(\"max_length\",\"100\")",
			"with_param(\"cleanup\",\"false\")",
			"with_param(\"max_width\",\"800\")",
			"with_param(\"max_height\",\"600\")",
		] {
			assert!(
				output.contains(expected),
				"image registration must contain `{expected}`: {output}"
			);
		}
	}

	#[test]
	fn test_image_field_classification_covers_direct_and_option_forms() {
		assert_eq!(
			storage_field_kind(&parse_quote! { db::orm::ImageField }),
			Some(StorageFieldKind::Image)
		);
		assert_eq!(
			storage_field_kind(&parse_quote! { Option<ImageField> }),
			Some(StorageFieldKind::Image)
		);
		assert_eq!(
			storage_field_kind(&parse_quote! { FileField }),
			Some(StorageFieldKind::File)
		);
		assert_eq!(
			storage_field_kind(&parse_quote! { my_fields::ImageField }),
			None
		);
	}

	#[rstest::rstest]
	#[case(
		parse_quote! { Option<Option<FileField>> },
		"nested Option wrappers are not supported for FileField"
	)]
	#[case(
		parse_quote! { Option<Option<ImageField>> },
		"nested Option wrappers are not supported for ImageField"
	)]
	fn test_storage_fields_reject_nested_option_wrappers(#[case] ty: Type, #[case] expected: &str) {
		let attrs = vec![parse_quote! { #[field(upload_to = "uploads")] }];
		let config = FieldConfig::from_attrs(&attrs).unwrap();

		let error = config
			.validate_for_field_type(&ty)
			.expect_err("nested storage-field options must be rejected");

		assert_eq!(error.to_string(), expected);
	}

	#[test]
	fn test_image_field_policy_rejects_zero_dimensions() {
		for attrs in [
			vec![parse_quote! { #[field(upload_to = "images", max_width = 0)] }],
			vec![parse_quote! { #[field(upload_to = "images", max_height = 0)] }],
		] {
			let config = FieldConfig::from_attrs(&attrs).unwrap();
			let error = config
				.validate_for_field_type(&parse_quote! { ImageField })
				.expect_err("zero image dimensions must be rejected");
			assert_eq!(
				error.to_string(),
				"ImageField max_width and max_height must be positive"
			);
		}
	}

	#[test]
	fn test_image_only_attributes_are_rejected_on_file_fields() {
		let attrs = vec![parse_quote! {
			#[field(upload_to = "files", max_width = 800, max_height = 600)]
		}];
		let config = FieldConfig::from_attrs(&attrs).unwrap();
		let error = config
			.validate_for_field_type(&parse_quote! { FileField })
			.expect_err("image dimensions must be rejected on FileField");

		assert_eq!(
			error.to_string(),
			"max_width and max_height are only valid on ImageField or Option<ImageField>"
		);
	}

	#[test]
	fn test_storage_attributes_are_rejected_on_non_storage_fields() {
		let attrs = vec![parse_quote! {
			#[field(upload_to = "files", file_storage = "media", cleanup = false)]
		}];
		let config = FieldConfig::from_attrs(&attrs).unwrap();
		let error = config
			.validate_for_field_type(&parse_quote! { String })
			.expect_err("storage policy must be rejected on String");

		assert_eq!(
			error.to_string(),
			"upload_to, file_storage, and cleanup are only valid on FileField, ImageField, or their Option forms"
		);
	}

	#[test]
	fn test_file_field_policy_rejects_non_file_attributes_and_short_paths() {
		let non_file_attrs = vec![parse_quote! {
			#[field(upload_to = "avatars")]
		}];
		let non_file = FieldConfig::from_attrs(&non_file_attrs).unwrap();
		let error = non_file
			.validate_for_field_type(&parse_quote! { String })
			.expect_err("upload_to must be rejected on non-file fields");
		assert!(
			error
				.to_string()
				.contains("only valid on FileField or Option<FileField>")
		);

		let short_attrs = vec![parse_quote! {
			#[field(upload_to = "avatars/%Y/%m/%d", max_length = 20)]
		}];
		let short = FieldConfig::from_attrs(&short_attrs).unwrap();
		let error = short
			.validate_for_field_type(&parse_quote! { db::orm::FileField })
			.expect_err("short file paths must be rejected");
		assert!(error.to_string().contains("too small"));
	}

	#[test]
	fn test_file_field_policy_rejects_max_length_over_u32() {
		let attrs = vec![parse_quote! {
			#[field(upload_to = "avatars", max_length = 4294967296)]
		}];
		let config = FieldConfig::from_attrs(&attrs).expect("file field config should parse");
		assert!(file_field_max_length(&config).is_err());
		let error = config
			.validate_for_field_type(&parse_quote! { db::orm::FileField })
			.expect_err("FileField max_length must fit the migration u32 type");
		assert!(error.to_string().contains("u32::MAX"));
	}

	#[test]
	fn test_file_field_migration_registration_preserves_semantic_params() {
		let input = quote! {
			#[model(app_label = "media", table_name = "media_assets")]
			struct Asset {
				#[field(primary_key = true)]
				id: i64,
				#[field(
					upload_to = "avatars/%Y/%m/%d",
					file_storage = "private_uploads",
					max_length = 255
				)]
				avatar: db::orm::FileField,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("file-field model registration must generate")
			.to_string()
			.replace(' ', "");

		for expected in [
			"with_param(\"model_field_type\",\"file\")",
			"with_param(\"upload_to\",\"avatars/%Y/%m/%d\")",
			"with_param(\"file_storage\",\"private_uploads\")",
			"with_param(\"max_length\",\"255\")",
		] {
			assert!(
				output.contains(expected),
				"migration registration must contain `{expected}`: {output}"
			);
		}
	}

	#[cfg(feature = "db-postgres")]
	#[test]
	fn test_file_field_registration_keeps_postgres_storage_separate() {
		let input = quote! {
			#[model(app_label = "media", table_name = "media_assets")]
			struct Asset {
				#[field(primary_key = true)]
				id: i64,
				#[field(
					upload_to = "avatars/%Y/%m/%d",
					file_storage = "private_uploads",
					max_length = 255,
					storage = "external"
				)]
				avatar: db::orm::FileField,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("PostgreSQL file-field model registration must generate")
			.to_string()
			.replace(' ', "");

		assert!(output.contains("with_param(\"file_storage\",\"private_uploads\")"));
		assert!(output.contains("attributes.insert(\"storage\""));
		assert!(output.contains("attributes.insert(\"file_storage\""));
		assert!(
			output.contains("FieldKwarg::String(\"external\".to_string())"),
			"PostgreSQL physical storage must remain its own metadata parameter: {output}"
		);
	}

	#[test]
	fn test_full_expansion_keeps_foreign_key_primary_key_type() {
		let input = quote! {
			#[model(app_label = "test", table_name = "audits", info = false)]
			pub struct Audit {
				#[field(primary_key = true)]
				pub id: i64,
				#[rel(foreign_key)]
				pub owner: db::associations::ForeignKeyField<Account>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let compact = output.to_string().replace(' ', "");

		assert!(compact.contains("IntoFieldValue"));
		assert!(compact.contains("into_field_value(fk_id)"));
		assert!(compact.contains("primary_key_column()"));
		let target_column_registration = compact
			.split("\"fk_target_column\"")
			.nth(1)
			.and_then(|registration| registration.split("\"fk_target_app\"").next())
			.expect("foreign-key registration must include the target column");
		assert_eq!(
			target_column_registration
				.matches("primary_key_column()")
				.count(),
			1
		);
		assert!(!compact.contains("fk_id.to_string()"));
	}

	#[test]
	fn test_foreign_key_to_field_uses_physical_column_in_accessors_and_registration() {
		let input = quote! {
			#[model(app_label = "test", table_name = "audits", info = false)]
			pub struct Audit {
				#[field(primary_key = true)]
				pub id: i64,
				#[rel(foreign_key, to_field = "external_key")]
				pub owner: db::associations::ForeignKeyField<Account>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output = output.to_string();
		let accessor = output
			.split("pub async fn owner")
			.nth(1)
			.and_then(|output| output.split("pub fn owner_accessor").next())
			.expect("foreign-key accessor must be generated");

		assert!(accessor.contains("field_info . name == \"external_key\""));
		assert!(accessor.contains("field_info . db_column . unwrap_or (field_info . name)"));
		assert!(!accessor.contains("primary_key_column"));
		let target_column_registration = output
			.split("\"fk_target_column\"")
			.nth(1)
			.and_then(|registration| registration.split("\"fk_target_app\"").next())
			.expect("foreign-key registration must include the target column");
		assert_eq!(
			target_column_registration
				.matches("field_info . name == \"external_key\"")
				.count(),
			1
		);
		assert_eq!(
			target_column_registration
				.matches("field_info . db_column . unwrap_or (field_info . name)")
				.count(),
			1
		);
	}

	#[test]
	fn test_db_column_expansion_preserves_write_filters_constraints_and_selectors() {
		let input = quote! {
			#[model(
				app_label = "test",
				table_name = "records",
				unique_together = ("email", "full_name")
			)]
			pub struct Record {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(db_column = "email_addr", max_length = 255)]
				pub email: String,
				#[field(
					db_column = "display_name",
					generated_sql = "lower(email_addr)",
					max_length = 255,
					generated_stored = true
				)]
				pub full_name: String,
				#[rel(foreign_key)]
				pub owner: db::associations::ForeignKeyField<Account>,
				#[serde(default)]
				owner_id: <Account as reinhardt::model_info::InfoModel>::PrimaryKey,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let compact = output.to_string().replace(' ', "");
		let file: syn::File = syn::parse2(output).expect("model expansion should parse as a file");
		let constraint_field_lists = file
			.items
			.iter()
			.filter_map(|item| {
				let syn::Item::Impl(item_impl) = item else {
					return None;
				};
				item_impl.items.iter().find_map(|item| {
					let syn::ImplItem::Fn(method) = item else {
						return None;
					};
					(method.sig.ident == "constraint_metadata").then_some(method)
				})
			})
			.flat_map(|method| method.block.stmts.iter())
			.filter_map(|statement| {
				let syn::Stmt::Expr(syn::Expr::MethodCall(call), _) = statement else {
					return None;
				};
				if call.method != "push" || call.args.len() != 1 {
					return None;
				}
				let syn::Expr::Struct(constraint) = &call.args[0] else {
					return None;
				};
				constraint.fields.iter().find_map(|field| {
					let syn::Member::Named(name) = &field.member else {
						return None;
					};
					(name == "fields").then(|| field.expr.to_token_stream().to_string())
				})
			})
			.collect::<Vec<_>>();

		assert!(compact.contains("stringify!(owner_id).to_string()"));
		assert!(compact.contains("\"full_name\",\"display_name\""));
		assert!(
			compact
				.contains("fields:vec![\"email_addr\".to_string(),\"display_name\".to_string()]")
		);
		assert_eq!(
			constraint_field_lists,
			vec!["vec ! [\"email\" . to_string () , \"full_name\" . to_string ()]"]
		);
		assert!(compact.contains("Field::new(vec![\"email_addr\"])"));
	}

	#[test]
	fn test_database_field_validation_is_native_gated() {
		let input = quote! {
			#[model(app_label = "test", table_name = "test")]
			pub struct TestModel {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(max_length = 1)]
				pub status: Status,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_string = output.to_string();
		let file: syn::File = syn::parse2(output).expect("model expansion should parse as a file");
		let validation = file
			.items
			.iter()
			.find_map(|item| {
				let syn::Item::Const(item_const) = item else {
					return None;
				};
				quote!(#item_const)
					.to_string()
					.contains("model enum value exceeds field max_length")
					.then_some(item_const)
			})
			.unwrap_or_else(|| {
				panic!(
					"custom database field validation const should be generated: {output_string}"
				)
			});

		assert_eq!(
			validation.attrs.len(),
			1,
			"database field schema validation must have one native cfg gate"
		);
		let cfg_attribute = validation
			.attrs
			.first()
			.expect("database field schema validation must have a cfg attribute");
		assert!(cfg_attribute.path().is_ident("cfg"));
		let syn::Meta::List(cfg) = &cfg_attribute.meta else {
			panic!("database field schema validation cfg must contain a condition");
		};
		let condition: String = cfg
			.tokens
			.to_string()
			.chars()
			.filter(|character| !character.is_whitespace())
			.collect();
		assert_eq!(
			condition,
			"not(all(target_family=\"wasm\",target_os=\"unknown\"))"
		);
	}

	#[test]
	fn test_foreign_key_metadata_uses_target_primary_key_storage_kind() {
		let field_info = ForeignKeyFieldInfo {
			field_name: parse_quote! { owner },
			target_type: parse_quote! { User },
			id_column_name: "owner_id".to_string(),
			related_name: None,
			is_one_to_one: false,
			skip_info: false,
			rel_attr: RelAttribute::default(),
		};

		let metadata = generate_field_metadata(&[], &[field_info])
			.expect("foreign key metadata should generate")
			.into_iter()
			.next()
			.expect("foreign key metadata item should exist")
			.to_string();

		assert!(metadata.contains("storage_kind : :: core :: option :: Option :: Some"));
		assert!(metadata.contains("User as"));
		assert!(metadata.contains("fk_id_field"));
		assert!(metadata.contains("domain : :: core :: option :: Option :: None"));
		assert!(metadata.contains("database_field_type_path"));
	}

	#[test]
	fn test_foreign_key_id_registration_propagates_skip_info() {
		let input = quote! {
			#[model(app_label = "test", table_name = "audits", info = false)]
			pub struct Audit {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(skip_info = true)]
				#[rel(foreign_key)]
				pub owner: db::associations::ForeignKeyField<Account>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("foreign-key model should generate")
			.to_string();

		assert!(output.contains("with_param (\"skip_info\" , \"true\")"));
	}

	#[test]
	fn test_foreign_key_registration_preserves_to_field() {
		let field_info = ForeignKeyFieldInfo {
			field_name: parse_quote! { owner },
			target_type: parse_quote! { User },
			id_column_name: "owner_id".to_string(),
			related_name: None,
			is_one_to_one: false,
			skip_info: false,
			rel_attr: RelAttribute {
				to_field: Some("external_key".to_string()),
				..RelAttribute::default()
			},
		};
		let struct_name: syn::Ident = parse_quote! { Comment };
		let generics = syn::Generics::default();
		let output = generate_registration_code(RegistrationCodeInput {
			struct_name: &struct_name,
			generics: &generics,
			app_label: "comments",
			table_name: "comments",
			field_infos: &[],
			fk_field_infos: &[field_info],
			unique_constraint_names: &[],
			unique_constraint_field_lists: &[],
			check_constraints: &[],
		})
		.expect("foreign-key registration should generate")
		.to_string();

		let compact = output.replace(' ', "");
		assert_eq!(
			compact
				.matches("with_param(\"fk_target_field\",\"external_key\")")
				.count(),
			1,
			"generated foreign-key metadata must preserve the to_field association: {output}"
		);
	}

	#[cfg(feature = "db-mysql")]
	#[rstest::rstest]
	fn test_registration_preserves_unsigned_metadata() {
		// Arrange
		let field_info = FieldInfo {
			name: parse_quote! { id },
			ty: parse_quote! { i64 },
			config: FieldConfig {
				unsigned: Some(true),
				..FieldConfig::default()
			},
			form: FieldFormConfig::default(),
			serde_attrs: Vec::new(),
			injected_relation_serde_skip: false,
			rel: None,
			is_fk_id_field: false,
		};
		let struct_name: syn::Ident = parse_quote! { Counter };
		let generics = syn::Generics::default();

		// Act
		let output = generate_registration_code(RegistrationCodeInput {
			struct_name: &struct_name,
			generics: &generics,
			app_label: "test",
			table_name: "counters",
			field_infos: &[field_info],
			fk_field_infos: &[],
			unique_constraint_names: &[],
			unique_constraint_field_lists: &[],
			check_constraints: &[],
		})
		.expect("unsigned registration should generate")
		.to_string();

		// Assert
		assert!(
			output
				.replace(' ', "")
				.contains("with_param(\"unsigned\",true.to_string())")
		);
	}

	fn test_table_name_defaults_to_app_label_and_struct_name_in_snake_case() {
		let cases = [
			("User", "test_user"),
			("BlogPost", "test_blog_post"),
			("Person", "test_person"),
			("HTTPRoute", "test_http_route"),
		];

		for (struct_name, expected_table_name) in cases {
			let struct_name = syn::Ident::new(struct_name, proc_macro2::Span::call_site());
			let attrs = vec![parse_quote! { #[model(app_label = "test")] }];

			let config = ModelConfig::from_attrs(&attrs, &struct_name)
				.expect("table name should be derived from the struct name");

			assert_eq!(config.table_name, expected_table_name);
		}
	}

	#[test]
	fn test_app_label_is_required() {
		let struct_name = parse_quote! { User };
		let attrs = vec![parse_quote! { #[model(table_name = "users")] }];

		let error = ModelConfig::from_attrs(&attrs, &struct_name)
			.expect_err("models without an app_label should be rejected");

		assert_eq!(
			error.to_string(),
			"app_label attribute is required in #[model(...)]"
		);
	}

	#[test]
	fn test_explicit_table_name_overrides_convention() {
		let struct_name = parse_quote! { User };
		let attrs = vec![parse_quote! {
			#[model(app_label = "users", table_name = "users_v2")]
		}];

		let config = ModelConfig::from_attrs(&attrs, &struct_name)
			.expect("explicit table names should remain supported");

		assert_eq!(config.table_name, "users_v2");
	}

	#[test]
	fn test_get_latest_by_parses_tuple_fields() {
		let struct_name = parse_quote! { Event };
		let attrs = vec![parse_quote! {
			#[model(app_label = "events", get_latest_by = ("created_at", "id"))]
		}];

		let config = ModelConfig::from_attrs(&attrs, &struct_name)
			.expect("get_latest_by should parse as field names");

		assert_eq!(
			config.get_latest_by.as_deref(),
			Some(vec!["created_at".to_string(), "id".to_string()].as_slice())
		);
	}

	#[test]
	fn test_get_latest_by_uses_physical_database_columns() {
		let input = quote! {
			#[model(
				app_label = "events",
				table_name = "events",
				get_latest_by = ("created_at", "id")
			)]
			pub struct Event {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(db_column = "created_on")]
				pub created_at: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("get_latest_by fields should resolve");
		let compact = output.to_string().replace(' ', "");

		assert!(
			compact
				.contains("fnlatest_by_fields()->&'static[&'staticstr]{&[\"created_on\",\"id\"]}")
		);
	}

	#[test]
	fn test_get_latest_by_uses_custom_column_for_relationship_named_with_id_suffix() {
		let input = quote! {
			#[model(
				app_label = "events",
				table_name = "events",
				get_latest_by = ("user_id_id",)
			)]
			pub struct Event {
				#[field(primary_key = true)]
				pub id: i64,
				#[rel(foreign_key, db_column = "user_fk")]
				pub user_id: db::associations::ForeignKeyField<User>,
				#[serde(default)]
				user_id_id: <User as reinhardt::model_info::InfoModel>::PrimaryKey,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("get_latest_by fields should resolve");
		let compact = output.to_string().replace(' ', "");

		assert!(compact.contains("fnlatest_by_fields()->&'static[&'staticstr]{&[\"user_fk\"]}"));
	}

	#[test]
	fn test_get_latest_by_preserves_descending_direction() {
		let input = quote! {
			#[model(
				app_label = "events",
				table_name = "events",
				get_latest_by = ("-priority", "created_at")
			)]
			pub struct Event {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(db_column = "event_priority")]
				pub priority: i64,
				pub created_at: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("descending get_latest_by fields should resolve");
		let compact = output.to_string().replace(' ', "");

		assert!(compact.contains(
			"fnlatest_by_fields()->&'static[&'staticstr]{&[\"-event_priority\",\"created_at\"]}"
		));
	}

	#[test]
	fn test_get_latest_by_rejects_empty_and_unknown_fields() {
		let empty_input = quote! {
			#[model(app_label = "events", get_latest_by = ())]
			pub struct Event {
				#[field(primary_key = true)]
				pub id: i64,
			}
		};
		let empty_error = model_derive_impl(syn::parse2(empty_input).unwrap())
			.expect_err("an empty get_latest_by tuple must fail");
		assert_eq!(
			empty_error.to_string(),
			"get_latest_by must contain at least one field"
		);

		let unknown_input = quote! {
			#[model(app_label = "events", get_latest_by = ("missing",))]
			pub struct Event {
				#[field(primary_key = true)]
				pub id: i64,
			}
		};
		let unknown_error = model_derive_impl(syn::parse2(unknown_input).unwrap())
			.expect_err("unknown get_latest_by fields must fail");
		assert_eq!(
			unknown_error.to_string(),
			"get_latest_by references unknown field 'missing'"
		);
	}

	#[test]
	fn test_get_latest_by_rejects_relation_fields() {
		let input = quote! {
			#[model(app_label = "events", get_latest_by = ("owner",))]
			pub struct Event {
				#[field(primary_key = true)]
				pub id: i64,
				#[rel(foreign_key)]
				pub owner: db::associations::ForeignKeyField<User>,
			}
		};

		let error = model_derive_impl(syn::parse2(input).unwrap())
			.expect_err("relation fields cannot define latest ordering");
		assert_eq!(
			error.to_string(),
			"get_latest_by cannot include relation field 'owner'"
		);
	}

	#[test]
	fn test_get_latest_by_rejects_many_to_many_fields() {
		let input = quote! {
			#[model(app_label = "events", get_latest_by = ("tags",))]
			pub struct Event {
				#[field(primary_key = true)]
				pub id: i64,
				#[rel(many_to_many)]
				pub tags: db::associations::ManyToManyField<Event, Tag>,
			}
		};

		let error = model_derive_impl(syn::parse2(input).unwrap())
			.expect_err("many-to-many fields cannot define latest ordering");
		assert_eq!(
			error.to_string(),
			"get_latest_by cannot include many-to-many field 'tags'"
		);
	}

	#[test]
	fn test_qualified_foreign_key_registration_preserves_target_identity() {
		let input = quote! {
			#[model(app_label = "comments", table_name = "comments")]
			pub struct Comment {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(foreign_key = "blog.Post")]
				pub post: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		assert!(output_str.contains("fk_target_app"));
		assert!(output_str.contains("fk_target_model"));
		assert!(output_str.contains("to_snake_case"));
	}

	#[test]
	fn test_bare_string_foreign_key_registration_uses_the_source_app() {
		let input = quote! {
			#[model(app_label = "comments")]
			pub struct Comment {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(foreign_key = "User")]
				pub user_id: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.unwrap()
			.to_string();

		assert!(output.contains("fk_target_model"));
		assert!(
			output.contains("with_param (\"fk_target_app\" , \"comments\")"),
			"bare string foreign keys must carry their source app for table resolution: {output}"
		);
		assert!(output.contains("User"));
		assert!(!output.contains("< User as"));
	}

	#[test]
	fn test_direct_qualified_foreign_key_registration_uses_target_identity() {
		let input = quote! {
			#[model(app_label = "comments")]
			pub struct Comment {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(foreign_key = crate::models::User)]
				pub user_id: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.unwrap()
			.to_string();

		assert!(output.contains("fk_target_model"));
		assert!(output.contains("User"));
		assert!(output.contains("crate :: models :: User as"));
		assert!(!output.contains("crate :: models :: User \""));
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
	fn test_relationship_metadata_infers_many_to_many_target_model() {
		// Arrange
		let input = quote! {
			#[model(app_label = "test", table_name = "articles")]
			pub struct Article {
				#[field(primary_key = true)]
				pub id: i64,
				#[rel(many_to_many)]
				pub tags: ManyToManyField<Article, Tag>,
			}
		};

		// Act
		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		// Assert
		assert!(output_str.contains("related_model : \"Tag\" . to_string ()"));
		assert!(!output_str.contains("related_model : \"\" . to_string ()"));
		assert!(output_str.contains("to_model : format !"));
		assert!(output_str.contains("< Tag as"));
		assert!(output_str.contains(":: app_label ()"));
	}

	#[test]
	fn test_one_to_many_traversal_uses_to_field_as_source_column() {
		let input = quote! {
			#[model(app_label = "test", table_name = "projects")]
			pub struct Project {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(max_length = 120, db_column = "project_slug")]
				pub slug: String,
				#[field(skip = true)]
				#[rel(one_to_many, to = Document, foreign_key = "project_slug", to_field = "slug")]
				pub documents: Vec<Document>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		assert!(output_str.contains("field_info . name == \"slug\""));
		assert!(output_str.contains("source_column"));
	}

	#[test]
	fn test_composite_primary_key_rejects_many_to_many_traversal() {
		let input = quote! {
			#[model(app_label = "test", table_name = "memberships")]
			pub struct Membership {
				#[field(primary_key = true)]
				pub user_id: i64,
				#[field(primary_key = true)]
				pub role_id: i64,
				#[rel(many_to_many)]
				pub tags: ManyToManyField<Membership, Tag>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();

		assert!(output.to_string().contains(
			"typed relation traversal does not support many_to_many relations on composite primary-key models"
		));
	}

	#[rstest]
	fn composite_primary_key_display_includes_parser_version_marker() {
		let input = quote! {
			#[model(app_label = "test", table_name = "memberships")]
			pub struct Membership {
				#[field(primary_key = true)]
				pub organization_id: i64,
				#[field(primary_key = true)]
				pub member_id: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();

		assert_eq!(
			generated_composite_display(output, "MembershipCompositePk"),
			quote! {
				impl ::std::fmt::Display for MembershipCompositePk {
					fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
						write!(f, "(v2;")?;
						let mut first = true;
						if !first {
							write!(f, ", ")?;
						}
						let value = self.organization_id.to_string();
						write!(f, "{}={}:{}", stringify!(organization_id), value.len(), value)?;
						first = false;
						if !first {
							write!(f, ", ")?;
						}
						let value = self.member_id.to_string();
						write!(f, "{}={}:{}", stringify!(member_id), value.len(), value)?;
						first = false;
						write!(f, ")")
					}
				}
			}
			.to_string()
		);
	}

	#[rstest]
	fn composite_primary_key_display_uses_rfc3339_for_datetime_fields() {
		let input = quote! {
			#[model(app_label = "test", table_name = "events")]
			pub struct Event {
				#[field(primary_key = true)]
				pub occurred_at: chrono::DateTime<chrono::Utc>,
				#[field(primary_key = true)]
				pub sequence: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();

		assert_eq!(
			generated_composite_display(output, "EventCompositePk"),
			quote! {
				impl ::std::fmt::Display for EventCompositePk {
					fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
						write!(f, "(v2;")?;
						let mut first = true;
						if !first {
							write!(f, ", ")?;
						}
						let value = self.occurred_at.to_rfc3339();
						write!(f, "{}={}:{}", stringify!(occurred_at), value.len(), value)?;
						first = false;
						if !first {
							write!(f, ", ")?;
						}
						let value = self.sequence.to_string();
						write!(f, "{}={}:{}", stringify!(sequence), value.len(), value)?;
						first = false;
						write!(f, ")")
					}
				}
			}
			.to_string()
		);
	}

	#[rstest]
	fn composite_primary_key_display_keeps_custom_datetime_named_fields_on_display() {
		let input = quote! {
			#[model(app_label = "test", table_name = "custom_keys")]
			pub struct CustomKeyModel {
				#[field(primary_key = true)]
				pub business_datetime_id: BusinessDateTimeId,
				#[field(primary_key = true)]
				pub sequence: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();

		assert_eq!(
			generated_composite_display(output, "CustomKeyModelCompositePk"),
			quote! {
				impl ::std::fmt::Display for CustomKeyModelCompositePk {
					fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
						write!(f, "(v2;")?;
						let mut first = true;
						if !first {
							write!(f, ", ")?;
						}
						let value = self.business_datetime_id.to_string();
						write!(f, "{}={}:{}", stringify!(business_datetime_id), value.len(), value)?;
						first = false;
						if !first {
							write!(f, ", ")?;
						}
						let value = self.sequence.to_string();
						write!(f, "{}={}:{}", stringify!(sequence), value.len(), value)?;
						first = false;
						write!(f, ")")
					}
				}
			}
			.to_string()
		);
	}

	#[rstest]
	fn composite_primary_key_display_rejects_non_chrono_datetime_paths() {
		let input = quote! {
			#[model(app_label = "test", table_name = "domain_keys")]
			pub struct DomainKeyModel {
				#[field(primary_key = true)]
				pub occurred_at: domain::DateTime<chrono::Utc>,
				#[field(primary_key = true)]
				pub sequence: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();

		assert_eq!(
			generated_composite_display(output, "DomainKeyModelCompositePk"),
			quote! {
				impl ::std::fmt::Display for DomainKeyModelCompositePk {
					fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
						write!(f, "(v2;")?;
						let mut first = true;
						if !first {
							write!(f, ", ")?;
						}
						let value = self.occurred_at.to_string();
						write!(f, "{}={}:{}", stringify!(occurred_at), value.len(), value)?;
						first = false;
						if !first {
							write!(f, ", ")?;
						}
						let value = self.sequence.to_string();
						write!(f, "{}={}:{}", stringify!(sequence), value.len(), value)?;
						first = false;
						write!(f, ")")
					}
				}
			}
			.to_string()
		);
	}

	#[test]
	fn test_relation_traversal_field_accessors_preserve_logical_and_physical_names() {
		let input = quote! {
			#[model(app_label = "test", table_name = "projects")]
			pub struct Project {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(max_length = 120, db_column = "email")]
				pub slug: String,
				#[field(max_length = 120, db_column = "email_addr")]
				pub email: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();
		let slug_accessor = output_str
			.split("pub fn field_slug")
			.nth(1)
			.and_then(|output| output.split("pub fn field_email").next())
			.expect("generated relation traversal slug accessor");

		assert!(
			slug_accessor
				.contains("from_generated_model_field_with_names (\"slug\" , \"email\" ,)")
		);
		assert!(
			!slug_accessor
				.contains("from_generated_model_field_with_names (\"slug\" , \"slug\" ,)")
		);
	}

	#[test]
	fn test_unique_accessors_use_the_physical_database_column() {
		let input = quote! {
			#[model(app_label = "test", table_name = "users")]
			pub struct User {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(unique = true, max_length = 120, db_column = "email_addr")]
				pub email: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();
		let unique_accessor = output_str
			.split("pub const fn unique_email")
			.nth(1)
			.expect("generated unique email accessor");

		assert!(
			unique_accessor.contains("UniqueFieldRef :: from_model_field_with_names_and_getter")
		);
		assert!(unique_accessor.contains("\"email\""));
		assert!(unique_accessor.contains("\"email_addr\""));
		assert!(unique_accessor.contains("Self :: __reinhardt_unique_get_email"));
		assert!(output_str.contains(
			"fn __reinhardt_unique_get_email (model : & User) -> :: core :: option :: Option < String >"
		));
	}

	#[test]
	fn test_unique_file_accessors_preserve_codec_policy_metadata() {
		let input = quote! {
			#[model(app_label = "test", table_name = "assets")]
			pub struct Asset {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(unique = true, upload_to = "assets", file_storage = "private", max_length = 80)]
				pub file: FileField,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.unwrap()
			.to_string();
		let unique_accessor = output
			.split("pub const fn unique_file")
			.nth(1)
			.expect("generated unique FileField accessor");

		assert!(
			unique_accessor
				.contains("UniqueFieldRef :: from_model_field_with_names_metadata_and_getter")
		);
		assert!(unique_accessor.contains("\"file_storage\" , \"private\""));
		assert!(unique_accessor.contains("\"file_max_length\" , \"80\""));
	}

	#[test]
	fn test_file_field_fixture_projection_accepts_database_paths() {
		let input = quote! {
			#[model(app_label = "test", table_name = "assets")]
			pub struct Asset {
				#[field(primary_key = true)]
				pub id: i64,
				#[field(upload_to = "assets", max_length = 80)]
				pub file: FileField,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.unwrap()
			.to_string();
		let fixture_projection = output
			.split("struct __ReinhardtFixtureProjection")
			.nth(1)
			.expect("generated fixture projection");

		assert!(fixture_projection.contains("file : :: std :: string :: String"));
		assert!(output.contains("__reinhardt_validate_fixture_file_field_file"));
		assert!(output.contains("FileField :: from_existing"));
		assert!(output.contains("file_max_length"));
	}

	#[test]
	fn test_ordering_accessors_are_separate_from_compatible_field_refs() {
		let input = quote! {
			#[model(app_label = "test", table_name = "events")]
			pub struct Event {
				#[field(primary_key = true, db_column = "event_id")]
				pub id: i64,
				#[rel(foreign_key)]
				pub owner: ForeignKeyField<User>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let output_str = output.to_string();

		assert!(output_str.contains("pub const fn field_id"));
		assert!(output_str.contains("from_generated_model_field_with_names"));
		assert!(output_str.contains("\"id\" , \"event_id\""));
		assert!(output_str.contains("pub const fn ordering_id"));
		assert!(output_str.contains("OrderingField :: from_model_field (\"event_id\")"));
		assert!(!output_str.contains("pub const fn ordering_owner"));
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

	#[test]
	fn test_fixture_handler_registration_does_not_depend_on_serde_config_flags() {
		let input = quote! {
			#[model_config(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();

		assert!(
			output
				.to_string()
				.contains("register_model :: < FixtureModel >"),
			"fixture handler registration must not depend on serde flags forwarded by #[model]"
		);
	}

	#[test]
	fn test_generic_models_skip_fixture_handler_registration() {
		let input = quote! {
			#[model_config(app_label = "fixture_tests", table_name = "fixture_models")]
			struct GenericFixtureModel<T> {
				#[field(primary_key = true)]
				id: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();

		assert!(
			!output
				.to_string()
				.contains("register_model :: < GenericFixtureModel >"),
			"generic model registration must not require an unspecified type parameter"
		);
	}

	#[test]
	fn test_fixture_projection_rejects_omitted_non_sql_defaults() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 255, default = default_title())]
				title: String,
			}
		};

		let output =
			model_derive_impl(syn::parse2(input).unwrap()).expect("fixture model must generate");

		assert!(
			!output
				.to_string()
				.contains("__reinhardt_validate_defaulted_fixture_field"),
			"fixture projections must not make fields with non-SQL defaults optional"
		);
	}

	#[test]
	fn test_fixture_projection_allows_omitted_sql_defaults() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 255, default = "draft")]
				title: String,
			}
		};

		let output =
			model_derive_impl(syn::parse2(input).unwrap()).expect("fixture model must generate");

		assert!(
			output
				.to_string()
				.contains("__reinhardt_validate_defaulted_fixture_field"),
			"fixture projections must allow fields with serialized SQL defaults to be omitted"
		);
	}

	#[test]
	fn test_fixture_sql_defaults_are_reflected_in_database_default_metadata() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 255, default = "draft")]
				status: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();

		assert!(
			output.contains("db_default : Some"),
			"serialized SQL defaults must be exposed as database defaults"
		);
	}

	#[test]
	fn test_fixture_projection_rejects_null_for_non_null_defaulted_option() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 255, null = false, default = "draft")]
				status: Option<String>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();

		assert!(
			output.contains("PhantomData < String >"),
			"non-null defaulted Option fields must deserialize their inner type"
		);
	}

	#[test]
	fn test_fixture_projection_rejects_null_for_required_foreign_keys() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[rel(foreign_key)]
				author: ForeignKeyField<Author>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();

		assert!(
			output.contains("validate_required_fixture_foreign_key"),
			"required foreign keys must use a null-rejecting fixture deserializer"
		);
	}

	#[test]
	fn test_fixture_projection_allows_omitted_nullable_foreign_keys() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[rel(foreign_key, null = true)]
				author: ForeignKeyField<Author>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();

		assert!(
			output.contains(
				"default , deserialize_with = \"__reinhardt_validate_nullable_fixture_foreign_key\""
			),
			"nullable foreign keys must permit omitted fixture relation values"
		);
	}

	#[test]
	fn test_model_form_nullable_relation_id_descriptor_is_not_required() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models", form = true)]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[rel(foreign_key, null = true)]
				author: ForeignKeyField<Author>,
				#[serde(default)]
				author_id: <Author as InfoModel>::PrimaryKey,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();

		assert!(
			output.contains("required : false") && output.contains("nullable : true"),
			"nullable relation ID descriptors must accept an omitted relation value"
		);
	}

	#[test]
	fn test_model_form_explicit_non_null_overrides_option_type() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models", form = true)]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 64, null = false)]
				name: Option<String>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("explicit non-null form field should derive")
			.to_string();

		assert!(
			output.contains("name : \"name\"")
				&& output.contains("nullable : false")
				&& output.contains("required : true")
				&& output.contains("MissingModelField"),
			"an explicit null = false annotation must control the generated descriptor: {output}"
		);
	}

	#[rstest]
	fn test_model_form_server_context_only_tracks_required_server_fields() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models", form = true)]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: Option<i64>,
				#[field(editable = false)]
				organization_id: i64,
				#[field(max_length = 200)]
				note: Option<String>,
				#[field(default = "system", editable = false, max_length = 200)]
				audit_token: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("server-owned form fields should derive");
		let file: syn::File = syn::parse2(output).expect("generated model output should parse");

		let mut context_types: Vec<_> = file
			.items
			.iter()
			.filter_map(|item| {
				let syn::Item::Struct(item) = item else {
					return None;
				};
				let name = item.ident.to_string();
				(name.starts_with("FixtureModelModelForm")
					&& (name.ends_with("Missing")
						|| name.ends_with("Present")
						|| name.ends_with("ServerContext")))
				.then_some(name)
			})
			.collect();
		context_types.sort();
		assert_eq!(
			context_types,
			vec![
				"FixtureModelModelFormOrganizationIdMissing",
				"FixtureModelModelFormOrganizationIdPresent",
				"FixtureModelModelFormServerContext",
			]
		);

		let mut context_method_signatures: Vec<_> = file
			.items
			.iter()
			.filter_map(|item| {
				let syn::Item::Impl(item) = item else {
					return None;
				};
				let syn::Type::Path(self_ty) = item.self_ty.as_ref() else {
					return None;
				};
				(self_ty.path.segments.last()?.ident == "FixtureModelModelFormServerContext")
					.then_some(item)
			})
			.flat_map(|item| item.items.iter())
			.filter_map(|item| {
				let syn::ImplItem::Fn(item) = item else {
					return None;
				};
				Some(format!(
					"{} {}",
					item.vis.to_token_stream(),
					item.sig.to_token_stream()
				))
			})
			.collect();
		context_method_signatures.sort();
		assert_eq!(
			context_method_signatures,
			vec![
				"pub fn new () -> Self",
				"pub fn organization_id (self , value : i64) -> FixtureModelModelFormServerContext < FixtureModelModelFormOrganizationIdPresent >",
			]
		);

		let form_model_methods = file
			.items
			.iter()
			.find_map(|item| {
				let syn::Item::Impl(item) = item else {
					return None;
				};
				let trait_name = item
					.trait_
					.as_ref()
					.and_then(|(_, path, _)| path.segments.last())?
					.ident
					.to_string();
				(trait_name == "FormModel").then(|| {
					item.items
						.iter()
						.filter_map(|item| {
							let syn::ImplItem::Fn(item) = item else {
								return None;
							};
							Some(item.sig.ident.to_string())
						})
						.collect::<Vec<_>>()
				})
			})
			.expect("generated models should implement FormModel");
		assert_eq!(
			form_model_methods,
			vec![
				"clean_for_update",
				"build_from_cleaned_compat",
				"apply_cleaned",
				"set_trusted_field_json",
				"trusted_relation_field_kind",
				"trusted_relation_field_is_required",
				"save_with_mode",
			]
		);
	}

	#[test]
	fn test_model_form_f32_bounds_are_representable() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models", form = true)]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				ratio: f32,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("f32 form field should derive")
			.to_string();

		assert!(
			output.contains("f32 :: MIN as f64") && output.contains("f32 :: MAX as f64"),
			"f32 descriptors must bound values to the finite f32 domain: {output}"
		);
	}

	#[test]
	fn test_model_form_storage_fields_emit_file_and_image_kinds() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models", form = true)]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(upload_to = "documents")]
				document: FileField,
				#[field(upload_to = "avatars")]
				avatar: Option<ImageField>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("storage-backed model form fields should derive");
		let output = syn::parse2::<syn::File>(output).expect("model expansion should parse");
		let fields = output
			.items
			.iter()
			.find_map(|item| match item {
				syn::Item::Const(item) if item.ident == "FIXTUREMODEL_FORM_FIELDS" => {
					Some(&item.expr)
				}
				_ => None,
			})
			.expect("generated model form field table");
		let syn::Expr::Array(fields) = fields.as_ref() else {
			panic!("model form fields must be generated as an array");
		};
		let descriptors = fields
			.elems
			.iter()
			.map(|field| {
				let syn::Expr::Struct(field) = field else {
					panic!("model form field entries must be struct expressions");
				};
				let name = field
					.fields
					.iter()
					.find(|field| field.member == syn::Member::Named(parse_quote!(name)))
					.and_then(|field| match &field.expr {
						syn::Expr::Lit(syn::ExprLit {
							lit: syn::Lit::Str(name),
							..
						}) => Some(name.value()),
						_ => None,
					})
					.expect("field descriptor name");
				let kind = field
					.fields
					.iter()
					.find(|field| field.member == syn::Member::Named(parse_quote!(kind)))
					.and_then(|field| match &field.expr {
						syn::Expr::Path(path) => path
							.path
							.segments
							.last()
							.map(|segment| segment.ident.to_string()),
						_ => None,
					})
					.expect("field descriptor kind");
				(name, kind)
			})
			.collect::<Vec<_>>();

		assert_eq!(
			descriptors,
			[
				("document".to_owned(), "File".to_owned()),
				("avatar".to_owned(), "Image".to_owned())
			]
		);
	}

	#[test]
	fn test_model_form_blank_non_null_field_requires_explicit_value() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models", form = true)]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(blank = true)]
				quantity: i64,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("blank non-null field should derive")
			.to_string();
		let quantity_assignment = output
			.split("quantity : match data . quantity")
			.nth(1)
			.expect("generated form must initialize the blank field")
			.split("} ,")
			.next()
			.expect("quantity assignment must terminate");

		assert!(
			quantity_assignment.contains("MissingModelField"),
			"omitting a non-null blank field must not synthesize a default: {quantity_assignment}"
		);
	}

	#[test]
	fn test_model_form_emits_relation_target_and_primary_key_metadata() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models", form = true)]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[rel(foreign_key)]
				author: ForeignKeyField<Author>,
				#[serde(default)]
				author_id: <Author as InfoModel>::PrimaryKey,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("relation-backed form should derive")
			.to_string();

		assert!(
			output.contains("relation_target_matches")
				&& output.contains("TypeId :: of :: < Author >")
				&& output.contains("ModelFormPrimaryKeyFields"),
			"generated schemas must expose target-aware relation and primary-key metadata: {output}"
		);
	}

	#[test]
	fn test_model_form_rejects_payload_api_accessor_collisions() {
		for field_name in [
			"clean_and_validate",
			"clone",
			"default",
			"fields",
			"from_validated_raw",
			"json",
			"get_json",
			"into_raw",
			"set_json",
			"supplied_fields",
			"__reinhardt_checkbox_enabled",
			"__reinhardt_color_accent",
			"__reinhardt_defaulted_summary",
		] {
			let field_name = Ident::new(field_name, proc_macro2::Span::call_site());
			let input = quote! {
				#[model(app_label = "fixture_tests", table_name = "fixture_models", form = true)]
				struct FixtureModel {
					#[field(primary_key = true)]
					id: i64,
					#[field(max_length = 64)]
					#field_name: String,
				}
			};

			let error = model_derive_impl(syn::parse2(input).unwrap())
				.expect_err("payload API names must not be shadowed by generated accessors");
			assert!(
				error
					.to_string()
					.contains("collides with generated model-form API"),
				"field `{field_name}` must be rejected: {error}",
			);
		}
	}

	#[test]
	fn test_fixture_projection_allows_omitted_implicit_auto_increment_primary_keys() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 255)]
				name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();
		let fixture_projection = output
			.split("struct __ReinhardtFixtureProjection")
			.nth(1)
			.expect("fixture validation must generate a projection")
			.split('}')
			.next()
			.expect("fixture projection must have a body");

		assert!(
			!fixture_projection.contains("id : i64"),
			"implicit integer primary keys must be omitted from fixture projections"
		);
	}

	#[test]
	fn test_fixture_projection_validates_supplied_implicit_auto_increment_primary_keys() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 255)]
				name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();
		let fixture_projection = output
			.split("struct __ReinhardtFixtureProjection")
			.nth(1)
			.expect("fixture validation must generate a projection")
			.split('}')
			.next()
			.expect("fixture projection must have a body");

		assert!(
			fixture_projection.contains("id : :: std :: option :: Option < i64 >"),
			"generated primary keys must remain optional while validating supplied values"
		);
	}

	#[test]
	fn test_fixture_projection_requires_non_generated_option_primary_keys() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true, auto_increment = false)]
				id: Option<i64>,
				#[field(max_length = 255)]
				name: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();
		let fixture_projection = output
			.split("struct __ReinhardtFixtureProjection")
			.nth(1)
			.expect("fixture validation must generate a projection")
			.split('}')
			.next()
			.expect("fixture projection must have a body");

		assert!(
			fixture_projection.contains("id : i64"),
			"non-generated primary keys must reject omitted and null fixture values"
		);
	}

	#[cfg(feature = "db-postgres")]
	#[test]
	fn test_fixture_projection_allows_omitted_identity_columns() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(identity_always = true)]
				sequence: i64,
			}
		};

		let output =
			model_derive_impl(syn::parse2(input).unwrap()).expect("fixture model must generate");
		let output = output.to_string();
		let fixture_projection = output
			.split("struct __ReinhardtFixtureProjection")
			.nth(1)
			.expect("fixture validation must generate a projection")
			.split('}')
			.next()
			.expect("fixture projection must have a body");

		assert!(
			!fixture_projection.contains("sequence : i64"),
			"fixture projections must not require database-generated identity columns"
		);
	}

	#[test]
	fn test_fixture_projection_preserves_deserialize_bounds() {
		let attr: syn::Attribute = parse_quote! {
			#[serde(
				bound(deserialize = "T: serde::Deserialize<'de>"),
				skip_serializing_if = "Option::is_none"
			)]
		};

		let projected = fixture_projection_serde_attr(&attr)
			.expect("deserialize bounds must be retained")
			.meta;
		let projected = quote! { #projected }.to_string();

		assert!(projected.contains("bound"));
		assert!(projected.contains("deserialize"));
		assert!(!projected.contains("skip_serializing_if"));
	}

	#[test]
	fn test_fixture_projection_preserves_custom_deserializers() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[serde(deserialize_with = "deserialize_uuid")]
				payload: Uuid,
				#[field(null = false)]
				#[serde(with = "uuid_serde")]
				optional_payload: Option<Uuid>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();

		assert!(
			output.contains("deserialize_uuid"),
			"fixture projections must retain direct custom deserializers"
		);
		assert!(
			output.contains("uuid_serde :: deserialize"),
			"fixture projections must adapt serde(with) to its deserialize function"
		);
		assert!(
			output.contains("__reinhardt_validate_fixture_field_optional_payload"),
			"rewritten optional fixture fields must validate through their custom deserializer"
		);
	}

	#[test]
	fn test_defaulted_fixture_projection_preserves_deserialize_bounds() {
		let input = quote! {
			#[model(app_label = "fixture_tests", table_name = "fixture_models")]
			struct FixtureModel {
				#[field(primary_key = true)]
				id: i64,
				#[field(max_length = 255, default = "draft")]
				#[serde(bound(deserialize = "String: serde::Deserialize<'de>"))]
				title: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap())
			.expect("fixture model must generate")
			.to_string();

		assert!(output.contains("bound"));
		assert!(output.contains("__reinhardt_validate_defaulted_fixture_field"));
	}

	#[rstest]
	fn uuid_primary_key_uses_uuid_filter_value() {
		let input = quote! {
			#[model(app_label = "test", table_name = "uuid_models")]
			pub struct UuidModel {
				#[field(primary_key = true)]
				pub id: uuid::Uuid,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let orm_crate = get_reinhardt_orm_crate();

		assert_eq!(
			generated_primary_key_filter_value(&output),
			quote! {
				fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
					#orm_crate::query::FilterValue::Uuid(pk)
				}
			}
			.to_string()
		);
	}

	#[rstest]
	#[case(quote!(i32))]
	#[case(quote!(i64))]
	fn integer_primary_key_uses_integer_filter_value(#[case] primary_key_type: TokenStream) {
		let input = quote! {
			#[model(app_label = "test", table_name = "integer_models")]
			pub struct IntegerModel {
				#[field(primary_key = true)]
				pub id: #primary_key_type,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let orm_crate = get_reinhardt_orm_crate();

		assert_eq!(
			generated_primary_key_filter_value(&output),
			quote! {
				fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
					#orm_crate::query::FilterValue::from(pk)
				}
			}
			.to_string()
		);
	}

	#[rstest]
	fn uuid_named_primary_key_uses_the_database_codec_filter_value() {
		let input = quote! {
			#[model(app_label = "test", table_name = "custom_uuid_models")]
			pub struct CustomUuidModel {
				#[field(primary_key = true)]
				pub id: Uuid,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let orm_crate = get_reinhardt_orm_crate();

		assert_eq!(
			generated_primary_key_filter_value(&output),
			quote! {
				fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
					#orm_crate::query::FilterValue::Typed(Self::primary_key_database_value(&pk))
				}
			}
			.to_string()
		);
	}

	#[rstest]
	fn timestamp_primary_key_uses_timestamp_filter_value() {
		let input = quote! {
			#[model(app_label = "test", table_name = "timestamp_models")]
			pub struct TimestampModel {
				#[field(primary_key = true)]
				pub id: chrono::DateTime<chrono::Utc>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let orm_crate = get_reinhardt_orm_crate();

		assert_eq!(
			generated_primary_key_filter_value(&output),
			quote! {
				fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
					#orm_crate::query::FilterValue::Timestamp(pk)
				}
			}
			.to_string()
		);
	}

	#[rstest]
	fn string_primary_key_uses_string_filter_value() {
		let input = quote! {
			#[model(app_label = "test", table_name = "string_models")]
			pub struct StringModel {
				#[field(primary_key = true, max_length = 255)]
				pub id: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let orm_crate = get_reinhardt_orm_crate();

		assert_eq!(
			generated_primary_key_filter_value(&output),
			quote! {
				fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
					#orm_crate::query::FilterValue::String(pk.to_string())
				}
			}
			.to_string()
		);
	}

	#[rstest]
	fn string_named_custom_primary_key_uses_display_string_filter_value() {
		let input = quote! {
			#[model(app_label = "test", table_name = "custom_string_models")]
			pub struct CustomStringModel {
				#[field(primary_key = true, max_length = 255)]
				pub id: String,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let orm_crate = get_reinhardt_orm_crate();

		assert_eq!(
			generated_primary_key_filter_value(&output),
			quote! {
				fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
					#orm_crate::query::FilterValue::String(pk.to_string())
				}
			}
			.to_string()
		);
	}

	#[rstest]
	fn datetime_like_primary_key_uses_the_database_codec_filter_value() {
		let input = quote! {
			#[model(app_label = "test", table_name = "custom_datetime_models")]
			pub struct CustomDateTimeModel {
				#[field(primary_key = true)]
				pub id: DateTime<Utc>,
			}
		};

		let output = model_derive_impl(syn::parse2(input).unwrap()).unwrap();
		let orm_crate = get_reinhardt_orm_crate();

		assert_eq!(
			generated_primary_key_filter_value(&output),
			quote! {
				fn primary_key_filter_value(pk: Self::PrimaryKey) -> #orm_crate::query::FilterValue {
					#orm_crate::query::FilterValue::Typed(Self::primary_key_database_value(&pk))
				}
			}
			.to_string()
		);
	}
}
