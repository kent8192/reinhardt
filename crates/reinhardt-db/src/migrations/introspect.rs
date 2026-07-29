//! # Database Schema Introspection and Code Generation
//!
//! This module provides functionality to generate Reinhardt ORM models from existing
//! database schemas. It follows the Database-First approach similar to sqlboiler/ent.
//!
//! ## Features
//!
//! - **Schema Reading**: Uses `DatabaseIntrospector` to read existing database schemas
//! - **Type Mapping**: Maps SQL types to Rust types with proper nullable handling
//! - **Code Generation**: Generates `#[model(...)]` annotated Rust structs
//! - **Relationship Detection**: Automatically detects FK relationships
//! - **Configuration**: TOML-based configuration for customization
//!
//! ## Usage
//!
//! ```bash
//! cargo run --bin manage introspect -d postgres://localhost/mydb -o src/models/
//! ```
//!
//! ## Configuration
//!
//! Create `reinhardt-introspect.toml`:
//!
//! ```toml
//! [database]
//! url = "postgres://user:pass@localhost:5432/myapp"
//!
//! [output]
//! directory = "src/models/generated"
//!
//! [generation]
//! app_label = "myapp"
//! detect_relationships = true
//!
//! [tables]
//! include = [".*"]
//! exclude = ["^pg_", "^reinhardt_migrations"]
//!
//! [type_overrides]
//! "users.status" = "UserStatus"
//! ```

mod config;
mod generator;
mod naming;
mod type_mapping;

pub use config::{CliArgs, GenerationConfig, IntrospectConfig, OutputConfig, TableFilterConfig};
pub use generator::{GeneratedFile, GeneratedOutput, SchemaCodeGenerator};
pub use naming::{
	NamingConvention, escape_rust_keyword, sanitize_identifier, to_pascal_case, to_snake_case,
};
pub use type_mapping::{TypeMapper, TypeMappingError};

use super::introspection::DatabaseSchema;
use super::{MigrationError, Result};
use quote::ToTokens;

/// Introspect a database and generate Rust model code.
///
/// This is the main entry point for the introspection feature.
///
/// # Arguments
///
/// * `config` - Configuration for introspection
/// * `introspector` - Database introspector implementation
///
/// # Returns
///
/// Generated output containing model files
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_db::migrations::introspect::{IntrospectConfig, introspect};
/// use reinhardt_db::migrations::introspection::PostgresIntrospector;
///
/// let config = IntrospectConfig::from_file("reinhardt-introspect.toml")?;
/// let introspector = PostgresIntrospector::new(pool);
/// let schema = introspector.read_schema().await?;
/// let output = generate_models(&config, &schema)?;
/// ```
pub fn generate_models(
	config: &IntrospectConfig,
	schema: &DatabaseSchema,
) -> Result<GeneratedOutput> {
	let generator = SchemaCodeGenerator::new(config.clone());
	generator.generate(schema)
}

/// Render all selected models as one parseable Rust module.
///
/// The existing schema generator remains the source of model syntax. This wrapper forces
/// single-file output and canonicalizes the generated syntax so HashMap-backed metadata does
/// not make inspectdb output depend on hash iteration order.
pub fn render_models_module(config: &IntrospectConfig, schema: &DatabaseSchema) -> Result<String> {
	let mut config = config.clone();
	// inspectdb renders a source artifact, so it must not pass connection details
	// through to the shared generator's header rendering.
	config.database.url.clear();
	config.output.single_file = true;
	config.imports.additional.sort();

	let schema = canonicalize_schema(schema);
	let output = generate_models(&config, &schema)?;
	let Some(file) = output.files.into_iter().next() else {
		return Ok(String::new());
	};

	let mut syntax = syn::parse_file(&file.content).map_err(|error| {
		MigrationError::IntrospectionError(format!("Failed to parse generated code: {error}"))
	})?;
	canonicalize_module(&mut syntax);
	Ok(prettyplease::unparse(&syntax))
}

fn canonicalize_schema(schema: &DatabaseSchema) -> DatabaseSchema {
	let mut schema = schema.clone();
	for table in schema.tables.values_mut() {
		table.primary_key.sort();
		for index in table.indexes.values_mut() {
			index.columns.sort();
		}
		for foreign_key in &mut table.foreign_keys {
			foreign_key.columns.sort();
			foreign_key.referenced_columns.sort();
		}
		table
			.foreign_keys
			.sort_by(|left, right| left.name.cmp(&right.name));
		for constraint in &mut table.unique_constraints {
			constraint.columns.sort();
		}
		table
			.unique_constraints
			.sort_by(|left, right| left.name.cmp(&right.name));
		table.check_constraints.sort_by(|left, right| {
			left.name
				.cmp(&right.name)
				.then_with(|| left.expression.cmp(&right.expression))
		});
	}
	schema
}

fn canonicalize_module(syntax: &mut syn::File) {
	let mut imports = Vec::new();
	let mut items = Vec::new();
	for mut item in std::mem::take(&mut syntax.items) {
		if let syn::Item::Struct(model) = &mut item
			&& let syn::Fields::Named(fields) = &mut model.fields
		{
			let mut named: Vec<_> = std::mem::take(&mut fields.named).into_iter().collect();
			named.sort_by(|left, right| left.ident.cmp(&right.ident));
			fields.named = named.into_iter().collect();
		}

		if matches!(item, syn::Item::Use(_)) {
			imports.push(item);
		} else {
			items.push(item);
		}
	}

	imports.sort_by_key(|item| item.to_token_stream().to_string());
	items.sort_by_key(|item| match item {
		syn::Item::Struct(model) => model.ident.to_string(),
		_ => item.to_token_stream().to_string(),
	});
	imports.extend(items);
	syntax.items = imports;
}

/// Write generated files to disk.
///
/// # Arguments
///
/// * `output` - Generated output from `generate_models`
/// * `force` - Overwrite existing files
///
/// # Errors
///
/// Returns error if files already exist and `force` is false
pub fn write_output(output: &GeneratedOutput, force: bool) -> Result<()> {
	for file in &output.files {
		if file.path.exists() && !force {
			return Err(MigrationError::IoError(std::io::Error::new(
				std::io::ErrorKind::AlreadyExists,
				format!("File already exists: {:?}", file.path),
			)));
		}

		// Create parent directories if needed
		if let Some(parent) = file.path.parent() {
			std::fs::create_dir_all(parent)?;
		}

		std::fs::write(&file.path, &file.content)?;
	}

	Ok(())
}

/// Preview generated code without writing to disk.
///
/// Useful for `--dry-run` mode.
pub fn preview_output(output: &GeneratedOutput) -> String {
	let mut preview = String::new();

	for file in &output.files {
		preview.push_str(&format!("// === {} ===\n", file.path.display()));
		preview.push_str(&file.content);
		preview.push_str("\n\n");
	}

	preview
}
