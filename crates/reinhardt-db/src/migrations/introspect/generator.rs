//! Rust code generator for database models.
//!
//! Generates `#[model(...)]` annotated Rust structs from database schema.

use super::config::IntrospectConfig;
use super::naming::{column_to_field_name, sanitize_identifier, table_to_struct_name};
use super::type_mapping::TypeMapper;
use crate::migrations::introspection::{ColumnInfo, DatabaseSchema, ForeignKeyInfo, TableInfo};
use crate::migrations::{GeneratedColumnDefinition, GeneratedStorage, MigrationError, Result};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use std::collections::HashSet;
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

		let mut names = HashSet::with_capacity(tables.len());
		for table in &tables {
			let struct_name = sanitize_identifier(&table_to_struct_name(
				&table.name,
				self.config.generation.struct_naming_convention(),
			));
			validate_generated_identifier(&table.name, &struct_name)?;
			if !names.insert(struct_name.clone()) {
				return Err(MigrationError::IntrospectionError(format!(
					"Multiple tables normalize to the generated model name `{struct_name}`"
				)));
			}
		}

		// Build a map of table name -> struct name for FK resolution
		let table_to_struct: HashMap<String, String> = tables
			.iter()
			.map(|t| {
				let struct_name = sanitize_identifier(&table_to_struct_name(
					&t.name,
					self.config.generation.struct_naming_convention(),
				));
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
		let relationship_imports =
			self.generate_relationship_imports(table, table_to_struct, schema);
		let model = self.generate_model(table, table_to_struct, schema)?;

		let tokens = quote! {
			#header
			#imports
			#relationship_imports

			#model
		};

		let content = self.format_tokens(tokens)?;

		// Use snake_case for file names
		let file_name = format!("{}.rs", module_file_stem(&table.name));
		let path = self.config.output.directory.join("models").join(file_name);

		Ok(GeneratedFile::new(path, content))
	}

	fn generate_relationship_imports(
		&self,
		table: &TableInfo,
		table_to_struct: &HashMap<String, String>,
		schema: &DatabaseSchema,
	) -> TokenStream {
		if self.config.output.single_file || !self.config.generation.detect_relationships {
			return TokenStream::new();
		}
		let current_module = module_identifier(&table.name);
		let mut imported_tables = HashSet::new();
		let imports: Vec<_> = table
			.foreign_keys
			.iter()
			.filter(|foreign_key| self.is_relationship_foreign_key(table, foreign_key, schema))
			.filter_map(|foreign_key| {
				let target = table_to_struct.get(&foreign_key.referenced_table)?;
				let module = module_identifier(&foreign_key.referenced_table);
				(module != current_module && imported_tables.insert(module.clone())).then(|| {
					let module = format_ident!("{}", module);
					let target = format_ident!("{}", target);
					quote! { use super::#module::#target; }
				})
			})
			.collect();
		quote! { #(#imports)* }
	}

	fn is_relationship_foreign_key(
		&self,
		table: &TableInfo,
		foreign_key: &ForeignKeyInfo,
		schema: &DatabaseSchema,
	) -> bool {
		if !self.config.generation.detect_relationships || foreign_key.columns.len() != 1 {
			return false;
		}
		let Some(source_column) = foreign_key.columns.first() else {
			return false;
		};
		let Some(target) = schema.tables.get(&foreign_key.referenced_table) else {
			return false;
		};
		table.columns.contains_key(source_column)
			&& target.primary_key.len() == 1
			&& foreign_key.referenced_columns == target.primary_key
	}

	/// Generate `models.rs`, which declares and re-exports all model modules.
	fn generate_module_file(&self, tables: &[&TableInfo]) -> Result<GeneratedFile> {
		let mut module_names = Vec::new();
		let mut struct_names = Vec::new();

		for table in tables {
			let module_name = module_identifier(&table.name);
			let struct_name = sanitize_identifier(&table_to_struct_name(
				&table.name,
				self.config.generation.struct_naming_convention(),
			));

			let module_ident = format_ident!("{}", module_name);
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
		table_to_struct: &HashMap<String, String>,
		schema: &DatabaseSchema,
	) -> Result<TokenStream> {
		let struct_name = sanitize_identifier(&table_to_struct_name(
			&table.name,
			self.config.generation.struct_naming_convention(),
		));
		validate_generated_identifier(&table.name, &struct_name)?;
		let struct_ident = format_ident!("{}", struct_name);
		let table_name = &table.name;
		let app_label = &self.config.generation.app_label;
		for index in table.indexes.values() {
			if index.columns.len() != 1 {
				return Err(MigrationError::IntrospectionError(format!(
					"index `{}` on `{}` has {} columns and cannot be represented by a field attribute",
					index.name,
					table.name,
					index.columns.len()
				)));
			}
		}
		if let Some(constraint) = table
			.unique_constraints
			.iter()
			.find(|constraint| constraint.columns.len() != 1)
		{
			return Err(MigrationError::IntrospectionError(format!(
				"unique constraint `{}` on `{}` has {} columns and cannot be represented by a field attribute",
				constraint.name,
				table.name,
				constraint.columns.len()
			)));
		}
		if let Some(foreign_key) = table.foreign_keys.iter().find(|foreign_key| {
			foreign_key.columns.len() != 1 || foreign_key.referenced_columns.len() != 1
		}) {
			return Err(MigrationError::IntrospectionError(format!(
				"foreign key `{}` on `{}` has {} source columns and {} referenced columns; composite foreign keys cannot be represented by a relationship field",
				foreign_key.name,
				table.name,
				foreign_key.columns.len(),
				foreign_key.referenced_columns.len(),
			)));
		}
		if self.config.generation.detect_relationships
			&& let Some(foreign_key) = table.foreign_keys.iter().find(|foreign_key| {
				foreign_key.columns.len() == 1
					&& table.primary_key.contains(&foreign_key.columns[0])
			}) {
			return Err(MigrationError::IntrospectionError(format!(
				"foreign key `{}` on `{}` is also the primary key; shared-primary-key relationships cannot be represented by a relationship field",
				foreign_key.name, table.name
			)));
		}
		if let Some(constraint) = table.check_constraints.first() {
			return Err(MigrationError::IntrospectionError(format!(
				"CHECK constraint `{}` on `{}` cannot be represented by a field attribute without a verified target column",
				constraint.name.as_deref().unwrap_or("<unnamed>"),
				table.name
			)));
		}

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
		let mut field_names = HashSet::new();
		let mut columns: Vec<_> = table.columns.values().collect();
		columns.sort_by_key(|column| {
			table
				.primary_key
				.iter()
				.position(|name| name == &column.name)
				.map(|position| (0, position, String::new()))
				.unwrap_or_else(|| (1, 0, column.name.clone()))
		});

		for column in &columns {
			let field_name = column_to_field_name(
				&column.name,
				self.config.generation.field_naming_convention(),
			);
			if !field_names.insert(field_name.clone()) {
				return Err(MigrationError::IntrospectionError(format!(
					"Columns in `{}` normalize to the same Rust field name `{field_name}`",
					table.name
				)));
			}
			let field = self.generate_field(table, column, table_to_struct, schema)?;
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
	fn generate_field(
		&self,
		table: &TableInfo,
		column: &ColumnInfo,
		table_to_struct: &HashMap<String, String>,
		schema: &DatabaseSchema,
	) -> Result<TokenStream> {
		let field_name = column_to_field_name(
			&column.name,
			self.config.generation.field_naming_convention(),
		);
		let field_ident = format_ident!("{}", field_name);

		let relationship = table.foreign_keys.iter().find(|foreign_key| {
			self.is_relationship_foreign_key(table, foreign_key, schema)
				&& foreign_key.columns.as_slice() == [column.name.as_str()]
		});
		let relationship_target = if let Some(foreign_key) = relationship {
			let target = table_to_struct
				.get(&foreign_key.referenced_table)
				.ok_or_else(|| {
					MigrationError::IntrospectionError(format!(
						"foreign key `{}` on `{}` references filtered-out table `{}`",
						foreign_key.name, table.name, foreign_key.referenced_table
					))
				})?;
			let target_table = schema
				.tables
				.get(&foreign_key.referenced_table)
				.ok_or_else(|| {
					MigrationError::IntrospectionError(format!(
						"foreign key `{}` on `{}` references unavailable table `{}`",
						foreign_key.name, table.name, foreign_key.referenced_table
					))
				})?;
			Some((foreign_key, target, target_table))
		} else {
			None
		};
		let is_one_to_one = relationship_target.as_ref().is_some_and(|_| {
			table
				.unique_constraints
				.iter()
				.any(|constraint| constraint.columns.as_slice() == [column.name.as_str()])
				|| table
					.indexes
					.values()
					.any(|index| index.unique && index.columns.as_slice() == [column.name.as_str()])
		});
		let rust_type = if let Some((_, target, _)) = relationship_target {
			let target_ident = format_ident!("{}", target);
			if is_one_to_one {
				quote! { OneToOneField<#target_ident> }
			} else {
				quote! { ForeignKeyField<#target_ident> }
			}
		} else {
			ensure_field_type_is_representable(table, column)?;
			self.type_mapper
				.map_column(&table.name, column)
				.map_err(|e| {
					MigrationError::IntrospectionError(format!(
						"Failed to map type for {}.{}: {}",
						table.name, column.name, e
					))
				})?
		};

		// Generate field attributes
		let mut attrs = Vec::new();
		let relationship_attr = relationship_target.map(|(foreign_key, _, target_table)| {
			let column_name = column.name.as_str();
			let nullable = column.nullable;
			let referenced_column = &foreign_key.referenced_columns[0];
			let target_field = column_to_field_name(
				referenced_column,
				self.config.generation.field_naming_convention(),
			);
			if !target_table.columns.contains_key(referenced_column) {
				return Err(MigrationError::IntrospectionError(format!(
					"foreign key `{}` on `{}` references missing column `{}.{}`",
					foreign_key.name, table.name, foreign_key.referenced_table, referenced_column
				)));
			}
			let on_delete = relation_action(foreign_key.on_delete.as_deref())?;
			let on_update = relation_action(foreign_key.on_update.as_deref())?;
			let relation_kind = if is_one_to_one { quote! { one_to_one } } else { quote! { foreign_key } };
			Ok(quote! {
				#[rel(#relation_kind, db_column = #column_name, to_field = #target_field, on_delete = #on_delete, on_update = #on_update, null = #nullable)]
			})
		}).transpose()?;
		if relationship_target.is_none() && field_name != column.name {
			let column_name = column.name.as_str();
			attrs.push(quote! { db_column = #column_name });
		}

		// Primary key attribute
		if table.primary_key.contains(&column.name) {
			if let Some(identity_generation) = column.identity_generation.as_deref() {
				match identity_generation {
					"ALWAYS" => attrs.push(quote! { primary_key = true, identity_always = true }),
					"BY DEFAULT" => {
						attrs.push(quote! { primary_key = true, identity_by_default = true })
					}
					other => {
						return Err(MigrationError::IntrospectionError(format!(
							"column `{}.{}` has unsupported PostgreSQL identity generation mode `{other}`",
							table.name, column.name
						)));
					}
				}
			} else if column.auto_increment {
				attrs.push(quote! { primary_key = true, auto_increment = true });
			} else {
				attrs.push(quote! { primary_key = true, auto_increment = false });
			}
		}

		// Unique attribute
		let is_unique = table
			.unique_constraints
			.iter()
			.any(|c| c.columns.len() == 1 && c.columns.contains(&column.name))
			|| table
				.indexes
				.values()
				.any(|index| index.unique && index.columns.as_slice() == [column.name.as_str()]);
		if is_unique && relationship_target.is_none() && !table.primary_key.contains(&column.name) {
			attrs.push(quote! { unique = true });
		}
		let is_indexed = table
			.indexes
			.values()
			.any(|index| index.columns.as_slice() == [column.name.as_str()] && !index.unique);
		if is_indexed && relationship_target.is_none() {
			attrs.push(quote! { index = true });
		}

		// Max length for varchar
		if let crate::migrations::fields::FieldType::VarChar(len) = &column.column_type {
			let len = *len;
			attrs.push(quote! { max_length = #len });
		}
		if let crate::migrations::fields::FieldType::Char(len) = &column.column_type {
			let len = *len;
			let field_type = format!("char({len})");
			attrs.push(quote! { max_length = #len, field_type = #field_type });
		}
		if matches!(
			column.column_type,
			crate::migrations::fields::FieldType::Text
				| crate::migrations::fields::FieldType::TinyText
				| crate::migrations::fields::FieldType::MediumText
				| crate::migrations::fields::FieldType::LongText
		) {
			attrs.push(quote! { field_type = "text" });
		}
		match column.column_type {
			crate::migrations::fields::FieldType::Json => {
				attrs.push(quote! { field_type = "json" });
			}
			crate::migrations::fields::FieldType::JsonBinary => {
				attrs.push(quote! { field_type = "jsonb" });
			}
			_ => {}
		}

		// Default value
		if let Some(ref default) = column.default {
			// Skip auto-generated defaults like NOW() or sequences
			if !is_auto_default(default)
				&& let Some(default_expression) =
					render_default_expression(default, &column.column_type)
			{
				attrs.push(quote! { default = #default_expression });
			}
		}

		if let Some(generated) = column.generated.as_ref() {
			attrs.push(Self::generated_field_args(generated)?);
		}

		// Generate doc comment if enabled
		let doc = if self.config.generation.include_column_comments {
			let comment = format!("Column: `{}`", column.name);
			Some(quote! { #[doc = #comment] })
		} else {
			None
		};
		let field_attr = (!attrs.is_empty()).then(|| quote! { #[field(#(#attrs),*)] });

		Ok(quote! {
			#doc
			#relationship_attr
			#field_attr
			pub #field_ident: #rust_type,
		})
	}

	fn generated_field_args(generated: &GeneratedColumnDefinition) -> Result<TokenStream> {
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
			return Ok(quote! { generated = #expr, #storage_attr });
		}

		if let Some(raw_sql) = generated.raw_sql.as_deref() {
			return Ok(quote! { generated_sql = #raw_sql, #storage_attr });
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

fn module_file_stem(table_name: &str) -> String {
	let stem = super::naming::to_snake_case(table_name);
	if stem == "mod" {
		"mod_model".to_string()
	} else {
		let identifier = sanitize_identifier(&stem);
		identifier
			.strip_prefix("r#")
			.unwrap_or(&identifier)
			.to_string()
	}
}

fn module_identifier(table_name: &str) -> String {
	let stem = super::naming::to_snake_case(table_name);
	if stem == "mod" {
		"mod_model".to_string()
	} else {
		sanitize_identifier(&stem)
	}
}

fn validate_generated_identifier(table_name: &str, identifier: &str) -> Result<()> {
	syn::parse_str::<syn::Ident>(identifier).map_err(|_| {
		MigrationError::IntrospectionError(format!(
			"table `{}` does not normalize to a valid Rust model identifier",
			table_name
		))
	})?;
	Ok(())
}

fn relation_action(action: Option<&str>) -> Result<TokenStream> {
	match action.unwrap_or("NO ACTION") {
		"CASCADE" => Ok(quote! { Cascade }),
		"SET NULL" => Ok(quote! { SetNull }),
		"SET DEFAULT" => Ok(quote! { SetDefault }),
		"RESTRICT" => Ok(quote! { Restrict }),
		"NO ACTION" => Ok(quote! { NoAction }),
		other => Err(MigrationError::IntrospectionError(format!(
			"foreign key action `{other}` cannot be represented by a relationship attribute"
		))),
	}
}

fn ensure_field_type_is_representable(table: &TableInfo, column: &ColumnInfo) -> Result<()> {
	use crate::migrations::fields::FieldType;
	let storage_type = match column.column_type {
		FieldType::SmallInteger => Some("SMALLINT"),
		FieldType::TinyInt => Some("TINYINT"),
		FieldType::MediumInt => Some("MEDIUMINT"),
		FieldType::Char(_) => Some("CHAR"),
		FieldType::TinyText => Some("TINYTEXT"),
		FieldType::MediumText => Some("MEDIUMTEXT"),
		FieldType::LongText => Some("LONGTEXT"),
		FieldType::Blob => Some("BLOB"),
		FieldType::TinyBlob => Some("TINYBLOB"),
		FieldType::MediumBlob => Some("MEDIUMBLOB"),
		FieldType::LongBlob => Some("LONGBLOB"),
		FieldType::Enum { .. } => Some("ENUM"),
		_ => None,
	};
	if let Some(storage_type) = storage_type {
		return Err(MigrationError::IntrospectionError(format!(
			"column `{}.{}` uses {storage_type}, which cannot be preserved by the generated Rust field type",
			table.name, column.name
		)));
	}
	Ok(())
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

fn render_default_expression(
	default: &str,
	field_type: &crate::migrations::fields::FieldType,
) -> Option<TokenStream> {
	use crate::migrations::fields::FieldType;
	let normalized = default
		.trim()
		.split_once("::")
		.map_or(default.trim(), |(literal, _)| literal)
		.trim();
	if let Some(value) = normalized
		.strip_prefix('\'')
		.and_then(|value| value.strip_suffix('\''))
	{
		let value = value.replace("''", "'");
		return Some(quote! { #value });
	}

	if matches!(field_type, FieldType::Boolean) {
		return match normalized {
			"0" | "false" | "FALSE" => Some(quote! { false }),
			"1" | "true" | "TRUE" => Some(quote! { true }),
			value => syn::parse_str::<syn::Expr>(value)
				.ok()
				.map(|expression| quote! { #expression }),
		};
	}

	if matches!(
		field_type,
		FieldType::Char(_)
			| FieldType::VarChar(_)
			| FieldType::Text
			| FieldType::TinyText
			| FieldType::MediumText
			| FieldType::LongText
	) {
		let value = normalized;
		let unquoted = value
			.strip_prefix('\'')
			.and_then(|value| value.strip_suffix('\''))
			.unwrap_or(value)
			.replace("''", "'");
		return Some(quote! { #unquoted });
	}

	syn::parse_str::<syn::Expr>(normalized)
		.ok()
		.map(|expression| quote! { #expression })
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::fields::FieldType;
	use crate::migrations::introspection::{
		CheckConstraintInfo, ColumnInfo, ForeignKeyInfo, IndexInfo, TableInfo, UniqueConstraintInfo,
	};
	use crate::migrations::{GeneratedColumnDefinition, GeneratedStorage, SchemaExpr};
	use rstest::rstest;
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
				identity_generation: None,
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
				identity_generation: None,
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
				identity_generation: None,
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

	#[rstest]
	fn render_default_expression_skips_sql_only_array_defaults() {
		assert!(
			render_default_expression(
				"ARRAY[]::text[]",
				&FieldType::Array(Box::new(FieldType::Text)),
			)
			.is_none()
		);
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
	fn generate_model_rejects_composite_indexes() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut table = create_test_table();
		table.indexes.insert(
			"users_name_email_idx".to_string(),
			IndexInfo {
				name: "users_name_email_idx".to_string(),
				columns: vec!["name".to_string(), "email".to_string()],
				unique: false,
				#[cfg(feature = "pgvector")]
				access_method: None,
				index_type: None,
				#[cfg(feature = "pgvector")]
				expressions: None,
				#[cfg(feature = "pgvector")]
				operator_class: None,
				#[cfg(feature = "pgvector")]
				operator_class_is_default: false,
			},
		);
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table.clone())].into(),
		};

		let error = generator
			.generate_model(&table, &HashMap::new(), &schema)
			.expect_err("composite indexes must not be silently discarded");
		assert_eq!(
			error.to_string(),
			"Introspection error: index `users_name_email_idx` on `users` has 2 columns and cannot be represented by a field attribute"
		);
	}

	#[test]
	fn generate_model_rejects_composite_unique_constraints() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut table = create_test_table();
		table.unique_constraints.push(UniqueConstraintInfo {
			name: "users_name_email_key".to_string(),
			columns: vec!["name".to_string(), "email".to_string()],
		});
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table.clone())].into(),
		};

		let error = generator
			.generate_model(&table, &HashMap::new(), &schema)
			.expect_err("composite unique constraints must not be silently discarded");
		assert_eq!(
			error.to_string(),
			"Introspection error: unique constraint `users_name_email_key` on `users` has 2 columns and cannot be represented by a field attribute"
		);
	}

	#[rstest]
	fn generate_model_emits_typed_literals_for_scalar_defaults() {
		let config = IntrospectConfig::default().with_app_label("test");
		let generator = SchemaCodeGenerator::new(config);
		let mut table = create_test_table();
		table.columns.insert(
			"enabled".to_string(),
			ColumnInfo {
				name: "enabled".to_string(),
				column_type: FieldType::Boolean,
				nullable: false,
				default: Some("0".to_string()),
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		table.columns.insert(
			"retries".to_string(),
			ColumnInfo {
				name: "retries".to_string(),
				column_type: FieldType::Integer,
				nullable: false,
				default: Some("0".to_string()),
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table.clone())].into(),
		};
		let code = generator
			.format_tokens(
				generator
					.generate_model(&table, &HashMap::new(), &schema)
					.expect("model generation should succeed"),
			)
			.expect("generated model should format");

		assert!(code.contains("default = false"));
		assert!(code.contains("default = 0"));
	}

	#[test]
	fn generate_model_preserves_identity_mode_and_postgres_cast_defaults() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut table = create_test_table();
		table
			.columns
			.get_mut("id")
			.expect("id column")
			.identity_generation = Some("ALWAYS".to_string());
		table.columns.insert(
			"metadata".to_string(),
			ColumnInfo {
				name: "metadata".to_string(),
				column_type: FieldType::JsonBinary,
				nullable: false,
				default: Some("'{}'::jsonb".to_string()),
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let code = generator
			.format_tokens(
				generator
					.generate_model(
						&table,
						&HashMap::new(),
						&DatabaseSchema {
							tables: [("users".to_string(), table.clone())].into(),
						},
					)
					.expect("identity and cast defaults should be representable"),
			)
			.expect("model should format");
		assert!(code.contains("identity_always = true"));
		assert!(code.contains("field_type = \"jsonb\""));
		assert!(code.contains("default = \"{}\""));
	}

	#[test]
	fn generate_model_preserves_json_metadata() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut table = create_test_table();
		table.columns.insert(
			"payload".to_string(),
			ColumnInfo {
				name: "payload".to_string(),
				column_type: FieldType::Json,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		table.columns.insert(
			"metadata".to_string(),
			ColumnInfo {
				name: "metadata".to_string(),
				column_type: FieldType::JsonBinary,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let code = generator
			.format_tokens(
				generator
					.generate_model(
						&table,
						&HashMap::new(),
						&DatabaseSchema {
							tables: [("users".to_string(), table.clone())].into(),
						},
					)
					.expect("JSON types should be representable"),
			)
			.expect("generated model should format");
		assert!(code.contains("field_type = \"json\""));
		assert!(code.contains("field_type = \"jsonb\""));
	}

	#[rstest]
	fn generate_model_preserves_primary_key_order_and_text_metadata() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut table = create_test_table();
		table.primary_key = vec!["name".to_string(), "id".to_string()];
		table.columns.insert(
			"description".to_string(),
			ColumnInfo {
				name: "description".to_string(),
				column_type: FieldType::Text,
				nullable: false,
				default: Some("'pending'::character varying".to_string()),
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table.clone())].into(),
		};
		let code = generator
			.format_tokens(
				generator
					.generate_model(&table, &HashMap::new(), &schema)
					.expect("model generation should succeed"),
			)
			.expect("generated model should format");

		assert!(code.contains("field_type = \"text\""));
		assert!(code.contains("default = \"pending\""));
		assert!(
			code.find("pub name").expect("name field") < code.find("pub id").expect("id field")
		);
	}

	#[rstest]
	fn generate_model_emits_enabled_foreign_key_relationships() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut project = create_test_table();
		project.name = "projects".to_string();
		let mut job = create_test_table();
		job.name = "jobs".to_string();
		job.columns.remove("name");
		job.columns.insert(
			"project".to_string(),
			ColumnInfo {
				name: "project".to_string(),
				column_type: FieldType::BigInteger,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		job.foreign_keys = vec![ForeignKeyInfo {
			name: "jobs_project_fk".to_string(),
			columns: vec!["project".to_string()],
			referenced_table: "projects".to_string(),
			referenced_columns: vec!["id".to_string()],
			on_delete: Some("CASCADE".to_string()),
			on_update: Some("SET NULL".to_string()),
		}];
		let schema = DatabaseSchema {
			tables: [
				("projects".to_string(), project),
				("jobs".to_string(), job.clone()),
			]
			.into(),
		};
		let code = generator
			.format_tokens(
				generator
					.generate_model(
						&job,
						&[("projects".to_string(), "Projects".to_string())].into(),
						&schema,
					)
					.expect("model generation should succeed"),
			)
			.expect("generated model should format");

		assert!(code.contains("foreign_key"));
		assert!(code.contains("db_column = \"project\""));
		assert!(code.contains("to_field = \"id\""));
		assert!(code.contains("on_delete = Cascade"));
		assert!(code.contains("on_update = SetNull"));
		assert!(code.contains("pub project: ForeignKeyField<Projects>"));
	}

	#[rstest]
	fn generate_model_keeps_non_primary_foreign_keys_as_scalars() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut accounts = create_test_table();
		accounts.name = "accounts".to_string();
		accounts.columns.insert(
			"external_id".to_string(),
			ColumnInfo {
				name: "external_id".to_string(),
				column_type: FieldType::BigInteger,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let mut invoices = create_test_table();
		invoices.name = "invoices".to_string();
		invoices.columns.insert(
			"account_external_id".to_string(),
			ColumnInfo {
				name: "account_external_id".to_string(),
				column_type: FieldType::BigInteger,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		invoices.foreign_keys = vec![ForeignKeyInfo {
			name: "invoices_account_external_id_fk".to_string(),
			columns: vec!["account_external_id".to_string()],
			referenced_table: "accounts".to_string(),
			referenced_columns: vec!["external_id".to_string()],
			on_delete: Some("CASCADE".to_string()),
			on_update: Some("CASCADE".to_string()),
		}];
		let schema = DatabaseSchema {
			tables: [
				("accounts".to_string(), accounts),
				("invoices".to_string(), invoices.clone()),
			]
			.into(),
		};

		let code = generator
			.format_tokens(
				generator
					.generate_model(
						&invoices,
						&[("accounts".to_string(), "Accounts".to_string())].into(),
						&schema,
					)
					.expect("model generation should succeed"),
			)
			.expect("generated model should format");

		assert!(code.contains("pub account_external_id: i64"));
		assert!(!code.contains("ForeignKeyField<Accounts>"));
	}

	#[rstest]
	fn generate_model_omits_unrepresentable_sql_defaults() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut table = create_test_table();
		table.columns.insert(
			"token".to_string(),
			ColumnInfo {
				name: "token".to_string(),
				column_type: FieldType::Uuid,
				nullable: false,
				default: Some("uuid_generate_v4()".to_string()),
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table.clone())].into(),
		};

		let code = generator
			.format_tokens(
				generator
					.generate_model(&table, &HashMap::new(), &schema)
					.expect("model generation should succeed"),
			)
			.expect("generated model should format");

		assert!(code.contains("pub token: uuid::Uuid"));
		assert!(!code.contains("default = uuid_generate_v4"));
	}

	#[rstest]
	fn generate_model_rejects_normalized_field_name_collisions() {
		let config = IntrospectConfig::default().with_app_label("test");
		let generator = SchemaCodeGenerator::new(config);
		let mut table = create_test_table();
		table.columns.insert(
			"display-name".to_string(),
			ColumnInfo {
				name: "display-name".to_string(),
				column_type: FieldType::Text,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		table.columns.insert(
			"display_name".to_string(),
			ColumnInfo {
				name: "display_name".to_string(),
				column_type: FieldType::Text,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table.clone())].into(),
		};

		let error = generator
			.generate_model(&table, &HashMap::new(), &schema)
			.expect_err("normalized collisions must be rejected");
		assert!(
			error
				.to_string()
				.contains("normalize to the same Rust field name")
		);
	}

	#[rstest]
	fn generate_rejects_normalized_model_name_collisions() {
		let config = IntrospectConfig::default().with_app_label("test");
		let generator = SchemaCodeGenerator::new(config);
		let mut hyphenated = create_test_table();
		hyphenated.name = "user-profile".to_string();
		let mut underscored = create_test_table();
		underscored.name = "user_profile".to_string();
		let schema = DatabaseSchema {
			tables: [
				("user-profile".to_string(), hyphenated),
				("user_profile".to_string(), underscored),
			]
			.into(),
		};

		let error = generator
			.generate(&schema)
			.expect_err("colliding generated model names must be rejected");
		assert!(error.to_string().contains("generated model name"));
	}

	#[rstest]
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
	fn multi_file_generation_omits_scalar_foreign_key_imports() {
		let mut config = IntrospectConfig::default();
		config.output.directory = PathBuf::from("/tmp/reinhardt-inspectdb-scalar-fk-imports");
		let generator = SchemaCodeGenerator::new(config);
		let mut accounts = create_test_table();
		accounts.name = "string".to_string();
		accounts.columns.insert(
			"external_id".to_string(),
			ColumnInfo {
				name: "external_id".to_string(),
				column_type: FieldType::BigInteger,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let mut invoices = create_test_table();
		invoices.name = "invoices".to_string();
		invoices.columns.insert(
			"account_external_id".to_string(),
			ColumnInfo {
				name: "account_external_id".to_string(),
				column_type: FieldType::BigInteger,
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		invoices.foreign_keys = vec![ForeignKeyInfo {
			name: "invoices_account_external_id_fk".to_string(),
			columns: vec!["account_external_id".to_string()],
			referenced_table: "string".to_string(),
			referenced_columns: vec!["external_id".to_string()],
			on_delete: Some("CASCADE".to_string()),
			on_update: Some("CASCADE".to_string()),
		}];

		let output = generator
			.generate(&DatabaseSchema {
				tables: [
					("string".to_string(), accounts),
					("invoices".to_string(), invoices),
				]
				.into(),
			})
			.expect("multi-file generation should succeed");
		let invoice_file = output
			.files
			.iter()
			.find(|file| file.path.ends_with("models/invoices.rs"))
			.expect("invoice model file should be generated");

		assert!(
			invoice_file
				.content
				.contains("pub account_external_id: i64")
		);
		assert!(!invoice_file.content.contains("use super::string::String;"));
	}

	#[test]
	fn multi_file_generation_uses_raw_identifier_only_in_module_declaration() {
		let mut config = IntrospectConfig::default().with_app_label("test");
		config.output.directory = PathBuf::from("/tmp/reinhardt-inspectdb-keyword-layout");
		let generator = SchemaCodeGenerator::new(config);
		let mut table = create_test_table();
		table.name = "type".to_string();
		let output = generator
			.generate(&DatabaseSchema {
				tables: [("type".to_string(), table)].into(),
			})
			.expect("keyword table should generate valid Rust 2024 modules");
		assert!(
			output
				.files
				.iter()
				.any(|file| file.path.ends_with("models/type.rs"))
		);
		let module = output
			.files
			.iter()
			.find(|file| file.path.ends_with("models.rs"))
			.expect("models module should be generated");
		assert!(module.content.contains("pub mod r#type;"));
	}

	#[test]
	fn multi_file_generation_emits_empty_models_module() {
		let mut config = IntrospectConfig::default();
		config.output.directory = PathBuf::from("/tmp/reinhardt-inspectdb-empty-layout");
		let output = SchemaCodeGenerator::new(config)
			.generate(&DatabaseSchema {
				tables: HashMap::new(),
			})
			.expect("empty schema should still provide the module entry point");
		assert_eq!(output.files.len(), 1);
		assert!(output.files[0].path.ends_with("models.rs"));
	}

	fn generate_email_reference(target_uses_unique_index: bool, source_is_unique: bool) -> String {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut users = create_test_table();
		users.name = "users".to_string();
		users.columns.insert(
			"email".to_string(),
			ColumnInfo {
				name: "email".to_string(),
				column_type: FieldType::VarChar(255),
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		if target_uses_unique_index {
			users.unique_constraints.clear();
			users.indexes.insert(
				"users_email_unique_idx".to_string(),
				IndexInfo {
					name: "users_email_unique_idx".to_string(),
					columns: vec!["email".to_string()],
					unique: true,
					#[cfg(feature = "pgvector")]
					access_method: None,
					index_type: None,
					#[cfg(feature = "pgvector")]
					expressions: None,
					#[cfg(feature = "pgvector")]
					operator_class: None,
					#[cfg(feature = "pgvector")]
					operator_class_is_default: false,
				},
			);
		}
		let mut profiles = create_test_table();
		profiles.name = "profiles".to_string();
		profiles.columns.remove("name");
		profiles.columns.insert(
			"user_email".to_string(),
			ColumnInfo {
				name: "user_email".to_string(),
				column_type: FieldType::VarChar(255),
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		profiles.foreign_keys = vec![ForeignKeyInfo {
			name: "profiles_user_email_fkey".to_string(),
			columns: vec!["user_email".to_string()],
			referenced_table: "users".to_string(),
			referenced_columns: vec!["email".to_string()],
			on_delete: Some("RESTRICT".to_string()),
			on_update: Some("SET NULL".to_string()),
		}];
		if source_is_unique {
			profiles.unique_constraints.push(UniqueConstraintInfo {
				name: "profiles_user_email_key".to_string(),
				columns: vec!["user_email".to_string()],
			});
		}
		let schema = DatabaseSchema {
			tables: [
				("users".to_string(), users),
				("profiles".to_string(), profiles.clone()),
			]
			.into(),
		};
		generator
			.format_tokens(
				generator
					.generate_model(
						&profiles,
						&[("users".to_string(), "Users".to_string())].into(),
						&schema,
					)
					.expect("single-column unique foreign key should be representable"),
			)
			.expect("model should format")
	}

	#[rstest::rstest]
	fn generate_model_keeps_foreign_key_to_unique_target_index_scalar() {
		// Arrange
		let code = generate_email_reference(true, false);

		// Act
		let field = code
			.lines()
			.find(|line| line.trim_start().starts_with("pub user_email:"))
			.map(str::trim);
		let relationship_count = code.matches("#[rel(").count();

		// Assert
		assert_eq!(field, Some("pub user_email: String,"));
		assert_eq!(relationship_count, 0);
	}

	#[rstest::rstest]
	fn generate_model_keeps_unique_source_to_non_primary_target_scalar() {
		// Arrange
		let code = generate_email_reference(false, true);

		// Act
		let field = code
			.lines()
			.find(|line| line.trim_start().starts_with("pub user_email:"))
			.map(str::trim);
		let relationship_count = code.matches("#[rel(").count();

		// Assert
		assert_eq!(field, Some("pub user_email: String,"));
		assert_eq!(relationship_count, 0);
	}

	#[test]
	fn generate_model_rejects_shared_primary_key_relationships() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut profile = create_test_table();
		profile.name = "profiles".to_string();
		profile.foreign_keys = vec![ForeignKeyInfo {
			name: "profiles_id_fkey".to_string(),
			columns: vec!["id".to_string()],
			referenced_table: "users".to_string(),
			referenced_columns: vec!["id".to_string()],
			on_delete: None,
			on_update: None,
		}];
		let schema = DatabaseSchema {
			tables: [("profiles".to_string(), profile.clone())].into(),
		};

		let error = generator
			.generate_model(&profile, &HashMap::new(), &schema)
			.expect_err("shared primary-key relationships must not generate marker primary keys");
		assert!(
			error
				.to_string()
				.contains("shared-primary-key relationships")
		);
	}

	#[test]
	fn generate_model_rejects_lossy_constraints_and_storage_types() {
		let generator = SchemaCodeGenerator::new(IntrospectConfig::default());
		let mut table = create_test_table();
		table.check_constraints = vec![CheckConstraintInfo {
			name: Some("users_name_check".to_string()),
			expression: "length(name) > 0".to_string(),
		}];
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table.clone())].into(),
		};
		assert!(
			generator
				.generate_model(&table, &HashMap::new(), &schema)
				.expect_err("table checks must not be silently discarded")
				.to_string()
				.contains("CHECK constraint")
		);
		table.check_constraints.clear();
		for (field_type, storage_type) in [
			(FieldType::TinyBlob, "TINYBLOB"),
			(FieldType::Char(4), "CHAR"),
			(FieldType::TinyText, "TINYTEXT"),
			(FieldType::MediumText, "MEDIUMTEXT"),
			(FieldType::LongText, "LONGTEXT"),
			(
				FieldType::Enum {
					values: vec!["active".to_string(), "inactive".to_string()],
				},
				"ENUM",
			),
		] {
			table
				.columns
				.get_mut("name")
				.expect("name column")
				.column_type = field_type;
			assert!(
				generator
					.generate_model(&table, &HashMap::new(), &schema)
					.expect_err("storage type must not be silently widened")
					.to_string()
					.contains(storage_type)
			);
		}
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
				identity_generation: None,
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
				identity_generation: None,
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
	fn test_generate_model_combines_renamed_field_metadata() {
		let config = IntrospectConfig::default().with_app_label("test");
		let generator = SchemaCodeGenerator::new(config);
		let mut table = create_test_table();
		table.columns.insert(
			"display-name".to_string(),
			ColumnInfo {
				name: "display-name".to_string(),
				column_type: FieldType::VarChar(255),
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: None,
			},
		);
		let table_to_struct: HashMap<String, String> =
			[("users".to_string(), "Users".to_string())].into();
		let schema = DatabaseSchema {
			tables: [("users".to_string(), table.clone())].into(),
		};

		let tokens = generator
			.generate_model(&table, &table_to_struct, &schema)
			.expect("model generation should succeed");
		let code = generator
			.format_tokens(tokens)
			.expect("generated model should format");

		assert!(code.contains(
			"#[field(db_column = \"display-name\", max_length = 255u32)]\n    pub display_name: String,"
		));
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
