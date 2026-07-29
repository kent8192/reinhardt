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
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ATOMIC_OUTPUT_ID: AtomicU64 = AtomicU64::new(0);

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

/// Write all generated files as one rollback-safe operation.
///
/// Every destination and generated byte buffer is validated before directories or files are
/// created. Files are then fully written to temporary siblings before any destination is
/// replaced. If a later replacement fails, previously replaced files are restored.
pub fn write_generated_files_atomically(output: &GeneratedOutput, force: bool) -> Result<()> {
	write_generated_files_atomically_with_commit_hook(output, force, |_, _| Ok(()))
}

/// Write generated files atomically while invoking a hook before each destination replacement.
///
/// This is exposed for deterministic failure testing of rollback behavior. Application code
/// should use [`write_generated_files_atomically`].
#[doc(hidden)]
pub fn write_generated_files_atomically_with_commit_hook<F>(
	output: &GeneratedOutput,
	force: bool,
	mut before_commit: F,
) -> Result<()>
where
	F: FnMut(usize, &Path) -> io::Result<()>,
{
	let (files, directories) = build_atomic_write_plan(output, force)?;
	let mut transaction = AtomicWriteTransaction::new(files);

	for directory in directories {
		fs::create_dir(&directory)?;
		transaction.created_directories.push(directory);
	}

	for index in 0..transaction.files.len() {
		transaction.prepare(index)?;
	}

	for index in 0..transaction.files.len() {
		let destination = transaction.files[index].destination.clone();
		before_commit(index, &destination)?;
		transaction.install(index)?;
	}

	transaction.finish()?;
	Ok(())
}

#[derive(Debug)]
struct OriginalFile {
	bytes: Vec<u8>,
	permissions: Permissions,
}

#[derive(Debug)]
struct AtomicWriteFile {
	destination: PathBuf,
	bytes: Vec<u8>,
	original: Option<OriginalFile>,
	temporary: Option<PathBuf>,
	backup: Option<PathBuf>,
	original_moved: bool,
	installed: bool,
}

#[derive(Debug)]
struct AtomicWriteTransaction {
	files: Vec<AtomicWriteFile>,
	created_directories: Vec<PathBuf>,
	finished: bool,
}

impl AtomicWriteTransaction {
	fn new(files: Vec<AtomicWriteFile>) -> Self {
		Self {
			files,
			created_directories: Vec::new(),
			finished: false,
		}
	}

	fn prepare(&mut self, index: usize) -> io::Result<()> {
		let destination = self.files[index].destination.clone();
		let (temporary, mut temporary_file) = create_temporary_sibling(&destination, "temporary")?;
		self.files[index].temporary = Some(temporary);

		if let Some(original) = &self.files[index].original {
			temporary_file.set_permissions(original.permissions.clone())?;
		}
		temporary_file.write_all(&self.files[index].bytes)?;
		temporary_file.sync_all()
	}

	fn install(&mut self, index: usize) -> io::Result<()> {
		let destination = self.files[index].destination.clone();
		if self.files[index].original.is_some() {
			let backup = unused_sibling_path(&destination, "backup")?;
			self.files[index].backup = Some(backup.clone());
			fs::rename(&destination, &backup)?;
			self.files[index].original_moved = true;
		}

		let temporary = self.files[index]
			.temporary
			.as_ref()
			.expect("every generated file is prepared before commit")
			.clone();
		fs::rename(&temporary, &destination)?;
		self.files[index].temporary = None;
		self.files[index].installed = true;
		Ok(())
	}

	fn finish(&mut self) -> io::Result<()> {
		for file in &self.files {
			if let Some(backup) = &file.backup {
				fs::remove_file(backup)?;
			}
		}
		for file in &mut self.files {
			file.backup = None;
			file.original_moved = false;
		}
		self.finished = true;
		Ok(())
	}

	fn rollback(&mut self) {
		for file in self.files.iter_mut().rev() {
			if file.installed {
				let _ = fs::remove_file(&file.destination);
				file.installed = false;
			}

			if file.original_moved {
				let restored_from_backup = file
					.backup
					.as_ref()
					.is_some_and(|backup| fs::rename(backup, &file.destination).is_ok());
				let restored = restored_from_backup
					|| file.original.as_ref().is_some_and(|original| {
						restore_original_file(&file.destination, original).is_ok()
					});
				if restored {
					file.original_moved = false;
				}
			}

			if let Some(temporary) = file.temporary.take() {
				let _ = fs::remove_file(temporary);
			}
			if !file.original_moved
				&& let Some(backup) = file.backup.take()
				&& backup.exists()
			{
				let _ = fs::remove_file(&backup);
			}
		}

		for directory in self.created_directories.iter().rev() {
			let _ = fs::remove_dir(directory);
		}
		self.created_directories.clear();
	}
}

impl Drop for AtomicWriteTransaction {
	fn drop(&mut self) {
		if !self.finished {
			self.rollback();
		}
	}
}

fn build_atomic_write_plan(
	output: &GeneratedOutput,
	force: bool,
) -> io::Result<(Vec<AtomicWriteFile>, Vec<PathBuf>)> {
	let mut destinations = HashSet::new();
	let mut files = Vec::with_capacity(output.files.len());
	let mut directories = HashSet::new();

	for generated in &output.files {
		validate_destination_path(&generated.path)?;
		if !destinations.insert(generated.path.clone()) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				format!("Duplicate generated destination: {:?}", generated.path),
			));
		}

		let original = match fs::symlink_metadata(&generated.path) {
			Ok(metadata) => {
				if !force {
					return Err(io::Error::new(
						io::ErrorKind::AlreadyExists,
						format!("File already exists: {:?}", generated.path),
					));
				}
				if !metadata.file_type().is_file() {
					return Err(io::Error::new(
						io::ErrorKind::InvalidInput,
						format!(
							"Generated destination is not a regular file: {:?}",
							generated.path
						),
					));
				}
				Some(OriginalFile {
					bytes: fs::read(&generated.path)?,
					permissions: metadata.permissions(),
				})
			}
			Err(error) if error.kind() == io::ErrorKind::NotFound => None,
			Err(error) => return Err(error),
		};

		for directory in missing_parent_directories(&generated.path)? {
			directories.insert(directory);
		}
		files.push(AtomicWriteFile {
			destination: generated.path.clone(),
			bytes: generated.content.as_bytes().to_vec(),
			original,
			temporary: None,
			backup: None,
			original_moved: false,
			installed: false,
		});
	}

	for destination in &destinations {
		if destinations
			.iter()
			.any(|other| other != destination && other.starts_with(destination))
		{
			return Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				format!(
					"Generated destination is an ancestor of another destination: {destination:?}"
				),
			));
		}
	}

	let mut directories: Vec<_> = directories.into_iter().collect();
	directories.sort_by_key(|path| path.components().count());
	Ok((files, directories))
}

fn validate_destination_path(destination: &Path) -> io::Result<()> {
	if destination.file_name().is_none() {
		return Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			format!("Generated destination has no file name: {destination:?}"),
		));
	}
	Ok(())
}

fn missing_parent_directories(destination: &Path) -> io::Result<Vec<PathBuf>> {
	let parent = destination
		.parent()
		.filter(|path| !path.as_os_str().is_empty())
		.unwrap_or_else(|| Path::new("."));
	let mut current = parent;
	let mut missing = Vec::new();

	loop {
		match fs::metadata(current) {
			Ok(metadata) => {
				if !metadata.is_dir() {
					return Err(io::Error::new(
						io::ErrorKind::NotADirectory,
						format!("Generated output parent is not a directory: {current:?}"),
					));
				}
				break;
			}
			Err(error) if error.kind() == io::ErrorKind::NotFound => {
				missing.push(current.to_path_buf());
				current = current
					.parent()
					.filter(|path| !path.as_os_str().is_empty())
					.unwrap_or_else(|| Path::new("."));
			}
			Err(error) => return Err(error),
		}
	}

	missing.reverse();
	Ok(missing)
}

fn create_temporary_sibling(destination: &Path, label: &str) -> io::Result<(PathBuf, File)> {
	for _ in 0..100 {
		let candidate = sibling_path(destination, label)?;
		match OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&candidate)
		{
			Ok(file) => return Ok((candidate, file)),
			Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
			Err(error) => return Err(error),
		}
	}

	Err(io::Error::new(
		io::ErrorKind::AlreadyExists,
		format!("Could not allocate temporary output beside {destination:?}"),
	))
}

fn unused_sibling_path(destination: &Path, label: &str) -> io::Result<PathBuf> {
	for _ in 0..100 {
		let candidate = sibling_path(destination, label)?;
		match fs::symlink_metadata(&candidate) {
			Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
			Ok(_) => {}
			Err(error) => return Err(error),
		}
	}

	Err(io::Error::new(
		io::ErrorKind::AlreadyExists,
		format!("Could not allocate backup output beside {destination:?}"),
	))
}

fn sibling_path(destination: &Path, label: &str) -> io::Result<PathBuf> {
	let parent = destination
		.parent()
		.filter(|path| !path.as_os_str().is_empty())
		.unwrap_or_else(|| Path::new("."));
	let file_name = destination.file_name().ok_or_else(|| {
		io::Error::new(
			io::ErrorKind::InvalidInput,
			format!("Generated destination has no file name: {destination:?}"),
		)
	})?;
	let id = NEXT_ATOMIC_OUTPUT_ID.fetch_add(1, Ordering::Relaxed);
	Ok(parent.join(format!(
		".{}.reinhardt-{label}-{}-{id}",
		file_name.to_string_lossy(),
		std::process::id()
	)))
}

fn restore_original_file(destination: &Path, original: &OriginalFile) -> io::Result<()> {
	let (temporary, mut temporary_file) = create_temporary_sibling(destination, "restore")?;
	let mut cleanup = TemporaryFileCleanup::new(temporary.clone());
	temporary_file.write_all(&original.bytes)?;
	temporary_file.set_permissions(original.permissions.clone())?;
	temporary_file.sync_all()?;
	fs::rename(&temporary, destination)?;
	cleanup.disarm();
	Ok(())
}

#[derive(Debug)]
struct TemporaryFileCleanup {
	path: Option<PathBuf>,
}

impl TemporaryFileCleanup {
	fn new(path: PathBuf) -> Self {
		Self { path: Some(path) }
	}

	fn disarm(&mut self) {
		self.path = None;
	}
}

impl Drop for TemporaryFileCleanup {
	fn drop(&mut self) {
		if let Some(path) = self.path.take() {
			let _ = fs::remove_file(path);
		}
	}
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
