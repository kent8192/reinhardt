//! Rust code generator for database models.
//!
//! Generates `#[model(...)]` annotated Rust structs from database schema.

use super::config::IntrospectConfig;
use super::naming::{column_to_field_name, sanitize_identifier, table_to_struct_name};
use super::type_mapping::TypeMapper;
use crate::migrations::introspection::{ColumnInfo, DatabaseSchema, TableInfo};
use crate::migrations::{GeneratedColumnDefinition, GeneratedStorage, MigrationError, Result};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use std::path::PathBuf;

/// Generated output containing all model files.
#[derive(Debug, Clone)]
pub struct GeneratedOutput {
	/// Generated files
	pub files: Vec<GeneratedFile>,
}

impl GeneratedOutput {
	/// Create a new empty output.
	pub fn new() -> Self {
		Self { files: Vec::new() }
	}

	/// Add a file to the output.
	pub fn add_file(&mut self, file: GeneratedFile) {
		self.files.push(file);
	}
}

impl Default for GeneratedOutput {
	fn default() -> Self {
		Self::new()
	}
}

/// A single generated file.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
	/// Path where the file should be written
	pub path: PathBuf,
	/// File content
	pub content: String,
}

impl GeneratedFile {
	/// Create a new generated file.
	pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
		Self {
			path: path.into(),
			content: content.into(),
		}
	}
}

/// Code generator for database models.
pub struct SchemaCodeGenerator {
	config: IntrospectConfig,
	type_mapper: TypeMapper,
}

impl SchemaCodeGenerator {
	/// Create a new code generator with the given configuration.
	pub fn new(config: IntrospectConfig) -> Self {
		let type_mapper = TypeMapper::new(config.type_overrides.clone());
		Self {
			config,
			type_mapper,
		}
	}

	/// Generate all model files from the database schema.
	pub fn generate(&self, schema: &DatabaseSchema) -> Result<GeneratedOutput> {
		let mut output = GeneratedOutput::new();

		// Filter tables based on configuration
		let tables: Vec<_> = schema
			.tables
			.values()
			.filter(|t| self.config.should_include_table(&t.name))
			.collect();

		if tables.is_empty() {
			tracing::warn!("No tables found to introspect after applying filters");
			return Ok(output);
		}

		// Build a map of table name -> struct name for FK resolution
		let table_to_struct: HashMap<String, String> = tables
			.iter()
			.map(|t| {
				let struct_name = table_to_struct_name(
					&t.name,
					self.config.generation.struct_naming_convention(),
				);
				(t.name.clone(), struct_name)
			})
			.collect();

		// Generate files
		if self.config.output.single_file {
			// Generate all models in a single file
			let file = self.generate_single_file(&tables, &table_to_struct, schema)?;
			output.add_file(file);
		} else {
			// Generate one file per table beneath the Rust 2024 module directory.
			for table in &tables {
				let file = self.generate_model_file(table, &table_to_struct, schema)?;
				output.add_file(file);
			}

			// Generate the sibling module entry point.
			let module_file = self.generate_module_file(&tables)?;
			output.add_file(module_file);
		}

		Ok(output)
	}

	/// Generate a single file containing all models.
	fn generate_single_file(
		&self,
		tables: &[&TableInfo],
		table_to_struct: &HashMap<String, String>,
		schema: &DatabaseSchema,
	) -> Result<GeneratedFile> {
		let header = self.generate_header();
		let imports = self.generate_imports();

		let mut models = Vec::new();
		for table in tables {
			let model = self.generate_model(table, table_to_struct, schema)?;
			models.push(model);
		}

		let tokens = quote! {
			#header
			#imports

			#(#models)*
		};

		let content = self.format_tokens(tokens)?;

		let path = self
			.config
			.output
			.directory
			.join(&self.config.output.single_file_name);
		Ok(GeneratedFile::new(path, content))
	}

	/// Generate a model file for a single table.
	fn generate_model_file(
		&self,
		table: &TableInfo,
		table_to_struct: &HashMap<String, String>,
		schema: &DatabaseSchema,
	) -> Result<GeneratedFile> {
		let header = self.generate_header();
		let imports = self.generate_imports();
		let model = self.generate_model(table, table_to_struct, schema)?;

		let tokens = quote! {
			#header
			#imports

			#model
		};

		let content = self.format_tokens(tokens)?;

		// Use snake_case for file names
		let file_name = format!("{}.rs", super::naming::to_snake_case(&table.name));
		let path = self.config.output.directory.join("models").join(file_name);

		Ok(GeneratedFile::new(path, content))
	}

	/// Generate `models.rs`, which declares and re-exports all model modules.
	fn generate_module_file(&self, tables: &[&TableInfo]) -> Result<GeneratedFile> {
		let mut module_names = Vec::new();
		let mut struct_names = Vec::new();

		for table in tables {
			let snake_name = super::naming::to_snake_case(&table.name);
			let struct_name = table_to_struct_name(
				&table.name,
				self.config.generation.struct_naming_convention(),
			);

			let module_ident = format_ident!("{}", sanitize_identifier(&snake_name));
			let struct_ident = format_ident!("{}", struct_name);

			module_names.push(module_ident);
			struct_names.push(struct_ident);
		}

		let header = self.generate_header();

		let tokens = quote! {
			#header

			#(pub mod #module_names;)*

			#(pub use #module_names::#struct_names;)*
		};

		let content = self.format_tokens(tokens)?;
		let path = self.config.output.directory.join("models.rs");

		Ok(GeneratedFile::new(path, content))
	}

	/// Generate the file header comment.
	fn generate_header(&self) -> TokenStream {
		// Build header comments as doc attributes
		let comment1 = "Generated by `reinhardt inspectdb` - DO NOT EDIT";
		let comment2 = "";
		let comment3 = "To regenerate, run:";
		let comment4 = "  cargo run --bin manage inspectdb";

		quote! {
			#![doc = #comment1]
			#![doc = #comment2]
			#![doc = #comment3]
			#![doc = #comment4]
		}
	}

	/// Generate import statements.
	fn generate_imports(&self) -> TokenStream {
		let mut imports = vec![
			quote! { use reinhardt::prelude::*; },
			quote! { use serde::{Deserialize, Serialize}; },
		];

		// Add chrono if we have date/time types
		imports.push(quote! { use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc}; });

		// Add additional imports from config
		for import in &self.config.imports.additional {
			if let Ok(import_tokens) = import.parse::<TokenStream>() {
				imports.push(quote! { use #import_tokens; });
			}
		}

		quote! {
			#(#imports)*
		}
	}

	/// Generate a model struct for a table.
	fn generate_model(
		&self,
		table: &TableInfo,
		_table_to_struct: &HashMap<String, String>,
		_schema: &DatabaseSchema,
	) -> Result<TokenStream> {
		let struct_name = table_to_struct_name(
			&table.name,
			self.config.generation.struct_naming_convention(),
		);
		let struct_ident = format_ident!("{}", struct_name);
		let table_name = &table.name;
		let app_label = &self.config.generation.app_label;

		// Generate derives
		let derives: Vec<TokenStream> = self
			.config
			.generation
			.derives
			.iter()
			.filter_map(|d| d.parse().ok())
			.collect();

		// Generate fields
		let mut fields = Vec::new();
		let columns: Vec<_> = table.columns.values().collect();

		for column in &columns {
			let field = self.generate_field(table, column)?;
			fields.push(field);
		}

		// Generate doc comment
		let doc_comment = format!("Represents the `{}` table", table_name);

		Ok(quote! {
			#[doc = #doc_comment]
			#[model(app_label = #app_label, table_name = #table_name)]
			#[derive(#(#derives),*)]
			pub struct #struct_ident {
				#(#fields)*
			}
		})
	}

	/// Generate a field for a column.
	fn generate_field(&self, table: &TableInfo, column: &ColumnInfo) -> Result<TokenStream> {
		let field_name = column_to_field_name(
			&column.name,
			self.config.generation.field_naming_convention(),
		);
		let field_ident = format_ident!("{}", field_name);

		let rust_type = self
			.type_mapper
			.map_column(&table.name, column)
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to map type for {}.{}: {}",
					table.name, column.name, e
				))
			})?;

		// Generate field attributes
		let mut attrs = Vec::new();

		// Primary key attribute
		if table.primary_key.contains(&column.name) {
			if column.auto_increment {
				attrs.push(quote! { #[field(primary_key = true, auto_increment = true)] });
			} else {
				attrs.push(quote! { #[field(primary_key = true)] });
			}
		}

		// Unique attribute
		let is_unique = table
			.unique_constraints
			.iter()
			.any(|c| c.columns.len() == 1 && c.columns.contains(&column.name));
		if is_unique && !table.primary_key.contains(&column.name) {
			attrs.push(quote! { #[field(unique = true)] });
		}

		// Max length for varchar
		if let crate::migrations::fields::FieldType::VarChar(len) = &column.column_type {
			let len = *len;
			attrs.push(quote! { #[field(max_length = #len)] });
		}

		// Default value
		if let Some(ref default) = column.default {
			// Skip auto-generated defaults like NOW() or sequences
			if !is_auto_default(default) {
				let default_str = default.as_str();
				attrs.push(quote! { #[field(default = #default_str)] });
			}
		}

		if let Some(generated) = column.generated.as_ref() {
			attrs.push(Self::generated_field_attr(generated)?);
		}

		// Generate doc comment if enabled
		let doc = if self.config.generation.include_column_comments {
			let comment = format!("Column: `{}`", column.name);
			Some(quote! { #[doc = #comment] })
		} else {
			None
		};

		Ok(quote! {
			#doc
			#(#attrs)*
			pub #field_ident: #rust_type,
		})
	}

	fn generated_field_attr(generated: &GeneratedColumnDefinition) -> Result<TokenStream> {
		let storage_attr = match generated.storage {
			GeneratedStorage::Stored => quote! { generated_stored = true },
			GeneratedStorage::Virtual => quote! { generated_virtual = true },
			_ => {
				return Err(MigrationError::IntrospectionError(
					"Unsupported generated column storage mode".to_string(),
				));
			}
		};

		if let Some(expr_tokens) = generated.expr_tokens.as_deref() {
			let expr = expr_tokens.parse::<TokenStream>().map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to parse generated expression tokens: {}",
					e
				))
			})?;
			return Ok(quote! { #[field(generated = #expr, #storage_attr)] });
		}

		if let Some(raw_sql) = generated.raw_sql.as_deref() {
			return Ok(quote! { #[field(generated_sql = #raw_sql, #storage_attr)] });
		}

		Err(MigrationError::IntrospectionError(
			"Generated column metadata is missing both expression tokens and raw SQL".to_string(),
		))
	}

	/// Format TokenStream to pretty Rust code.
	fn format_tokens(&self, tokens: TokenStream) -> Result<String> {
		let syntax_tree = syn::parse2::<syn::File>(tokens).map_err(|e| {
			MigrationError::IntrospectionError(format!("Failed to parse generated code: {}", e))
		})?;

		Ok(prettyplease::unparse(&syntax_tree))
	}
}

/// Check if a default value is auto-generated (e.g., NOW(), sequences).
fn is_auto_default(default: &str) -> bool {
	let upper = default.to_uppercase();
	upper.contains("NOW()")
		|| upper.contains("CURRENT_TIMESTAMP")
		|| upper.contains("CURRENT_DATE")
		|| upper.contains("CURRENT_TIME")
		|| upper.contains("NEXTVAL")
		|| upper.contains("UUID_GENERATE")
		|| upper.contains("GEN_RANDOM_UUID")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::fields::FieldType;
	use crate::migrations::introspection::{ColumnInfo, TableInfo, UniqueConstraintInfo};
	use crate::migrations::{GeneratedColumnDefinition, GeneratedStorage, SchemaExpr};
	use std::collections::HashMap;

	fn create_test_table() -> TableInfo {
		let mut columns = HashMap::new();

		columns.insert(
			"id".to_string(),
			ColumnInfo {
				name: "id".to_string(),
				column_type: FieldType::BigInteger,
				nullable: false,
				default: None,
				auto_increment: true,
				generated: None,
			},
		);

		columns.insert(
			"name".to_string(),
			ColumnInfo {
				name: "name".to_string(),
				column_type: FieldType::VarChar(255),
				nullable: false,
				default: None,
				auto_increment: false,
				generated: None,
			},
		);

		columns.insert(
			"email".to_string(),
			ColumnInfo {
				name: "email".to_string(),
				column_type: FieldType::VarChar(255),
				nullable: true,
				default: None,
				auto_increment: false,
				generated: None,
			},
		);

		TableInfo {
			name: "users".to_string(),
			columns,
			indexes: HashMap::new(),
			primary_key: vec!["id".to_string()],
			foreign_keys: vec![],
			unique_constraints: vec![UniqueConstraintInfo {
				name: "users_email_unique".to_string(),
				columns: vec!["email".to_string()],
			}],
			check_constraints: vec![],
		}
	}

	#[test]
	fn test_generate_model() {
		let config = IntrospectConfig::default().with_app_label("test");
		let generator = SchemaCodeGenerator::new(config);

		let table = create_test_table();
		let table_to_struct: HashMap<String, String> =
			[("users".to_string(), "Users".to_string())].into();

		let mut schema = DatabaseSchema {
			tables: HashMap::new(),
		};
		schema.tables.insert("users".to_string(), table.clone());

		let result = generator.generate_model(&table, &table_to_struct, &schema);
		assert!(result.is_ok());

		let tokens = result.unwrap();
		let code = generator.format_tokens(tokens).unwrap();

		assert!(code.contains("pub struct Users"));
		assert!(code.contains("pub id: i64"));
		assert!(code.contains("pub name: String"));
		assert!(code.contains("pub email: Option<String>"));
	}

	#[test]
	fn multi_file_generation_uses_rust_2024_module_layout() {
		let mut config = IntrospectConfig::default().with_app_label("test");
		config.output.directory = PathBuf::from("/tmp/reinhardt-inspectdb-layout");
		let generator = SchemaCodeGenerator::new(config);
		let table = create_test_table();
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table)].into(),
		};

		let output = generator
			.generate(&schema)
			.expect("multi-file generation should succeed");
		let files: Vec<_> = output
			.files
			.iter()
			.map(|file| file.path.as_path())
			.collect();

		assert_eq!(
			files,
			vec![
				std::path::Path::new("/tmp/reinhardt-inspectdb-layout/models/users.rs"),
				std::path::Path::new("/tmp/reinhardt-inspectdb-layout/models.rs"),
			],
		);
		assert!(
			output
				.files
				.iter()
				.all(|file| file.path.file_name().is_none_or(|name| name != "mod.rs")),
			"Rust 2024 output must never generate mod.rs",
		);
		let module = output
			.files
			.iter()
			.find(|file| file.path.ends_with("models.rs"))
			.expect("models.rs should be generated");
		syn::parse_file(&module.content).expect("models.rs should be parseable Rust");
		assert!(module.content.contains("pub mod users;"));
		assert!(module.content.contains("pub use users::Users;"));
	}

	#[test]
	fn test_generate_model_emits_generated_field_attributes() {
		let config = IntrospectConfig::default().with_app_label("test");
		let generator = SchemaCodeGenerator::new(config);
		let mut table = create_test_table();
		table.columns.insert(
			"normalized_name".to_string(),
			ColumnInfo {
				name: "normalized_name".to_string(),
				column_type: FieldType::VarChar(255),
				nullable: false,
				default: None,
				auto_increment: false,
				generated: Some(GeneratedColumnDefinition::raw_sql(
					"lower(name)",
					GeneratedStorage::Stored,
				)),
			},
		);
		table.columns.insert(
			"name_copy".to_string(),
			ColumnInfo {
				name: "name_copy".to_string(),
				column_type: FieldType::VarChar(255),
				nullable: false,
				default: None,
				auto_increment: false,
				generated: Some(GeneratedColumnDefinition::typed(
					SchemaExpr::col("name"),
					"SchemaExpr::col(\"name\")",
					GeneratedStorage::Virtual,
				)),
			},
		);
		let table_to_struct: HashMap<String, String> =
			[("users".to_string(), "Users".to_string())].into();
		let mut schema = DatabaseSchema {
			tables: HashMap::new(),
		};
		schema.tables.insert("users".to_string(), table.clone());

		let tokens = generator
			.generate_model(&table, &table_to_struct, &schema)
			.expect("model generation should succeed");
		let code = generator
			.format_tokens(tokens)
			.expect("generated model should format");

		assert!(code.contains(r#"generated_sql = "lower(name)""#));
		assert!(code.contains("generated_stored = true"));
		assert!(code.contains(r#"generated = SchemaExpr::col("name")"#));
		assert!(code.contains("generated_virtual = true"));
	}

	#[test]
	fn test_is_auto_default() {
		assert!(is_auto_default("NOW()"));
		assert!(is_auto_default("CURRENT_TIMESTAMP"));
		assert!(is_auto_default("nextval('seq')"));
		assert!(!is_auto_default("true"));
		assert!(!is_auto_default("'default_value'"));
	}
}
