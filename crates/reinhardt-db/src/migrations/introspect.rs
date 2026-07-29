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
use std::path::{Component, Path, PathBuf};
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
	let mut faults = NoAtomicWriteFaults;
	write_generated_files_atomically_with_faults(output, force, &mut faults)
}

fn write_generated_files_atomically_with_faults<I>(
	output: &GeneratedOutput,
	force: bool,
	faults: &mut I,
) -> Result<()>
where
	I: AtomicWriteFaultInjector,
{
	let (files, directories) = build_atomic_write_plan(output, force)?;
	let mut transaction = AtomicWriteTransaction::new(files);
	let write_result = transaction.execute(directories, faults);

	match write_result {
		Ok(()) => {
			transaction.finished = true;
			Ok(())
		}
		Err(trigger) => {
			let rollback_failures = transaction.rollback(faults);
			transaction.finished = true;
			Err(MigrationError::IoError(aggregate_rollback_failures(
				trigger,
				&rollback_failures,
			)))
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicWritePoint {
	BeforeTemporaryCreate,
	BeforeBackupCreate,
	AfterOriginalMove,
	BeforeInstall,
	BeforeBackupCleanup,
	BeforeRollbackRemoveInstalled,
	BeforeRollbackRestore,
	BeforeRollbackTemporaryCleanup,
	BeforeRollbackBackupCleanup,
	BeforeRollbackDirectoryCleanup,
}

trait AtomicWriteFaultInjector {
	fn check(&mut self, point: AtomicWritePoint, index: usize, path: &Path) -> io::Result<()>;
}

struct NoAtomicWriteFaults;

impl AtomicWriteFaultInjector for NoAtomicWriteFaults {
	fn check(&mut self, _point: AtomicWritePoint, _index: usize, _path: &Path) -> io::Result<()> {
		Ok(())
	}
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
	backup: Option<BackupFile>,
	original_moved: bool,
	installed: bool,
}

#[derive(Debug)]
struct BackupFile {
	directory: PathBuf,
	file: PathBuf,
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

	fn execute<I>(&mut self, directories: Vec<PathBuf>, faults: &mut I) -> io::Result<()>
	where
		I: AtomicWriteFaultInjector,
	{
		for directory in directories {
			fs::create_dir(&directory)?;
			self.created_directories.push(directory);
		}

		for index in 0..self.files.len() {
			self.prepare(index, faults)?;
		}
		for index in 0..self.files.len() {
			self.install(index, faults)?;
		}
		self.cleanup_backups(faults)
	}

	fn prepare<I>(&mut self, index: usize, faults: &mut I) -> io::Result<()>
	where
		I: AtomicWriteFaultInjector,
	{
		let destination = self.files[index].destination.clone();
		let (temporary, mut temporary_file) =
			create_temporary_sibling(&destination, "temporary", index, faults)?;
		self.files[index].temporary = Some(temporary.clone());

		temporary_file.write_all(&self.files[index].bytes)?;
		temporary_file.sync_all()?;
		drop(temporary_file);
		if let Some(original) = &self.files[index].original {
			fs::set_permissions(&temporary, original.permissions.clone())?;
		}
		Ok(())
	}

	fn install<I>(&mut self, index: usize, faults: &mut I) -> io::Result<()>
	where
		I: AtomicWriteFaultInjector,
	{
		let destination = self.files[index].destination.clone();
		if self.files[index].original.is_some() {
			let backup = create_backup_sibling(&destination, index, faults)?;
			self.files[index].backup = Some(backup);
			let backup_file = self.files[index]
				.backup
				.as_ref()
				.expect("the exclusive backup directory was just recorded")
				.file
				.clone();
			fs::rename(&destination, backup_file)?;
			self.files[index].original_moved = true;
			faults.check(AtomicWritePoint::AfterOriginalMove, index, &destination)?;
		}

		let temporary = self.files[index]
			.temporary
			.as_ref()
			.expect("every generated file is prepared before commit")
			.clone();
		faults.check(AtomicWritePoint::BeforeInstall, index, &destination)?;
		if let Some(permissions) = self.files[index]
			.original
			.as_ref()
			.map(|original| original.permissions.clone())
		{
			fs::rename(&temporary, &destination)?;
			self.files[index].temporary = None;
			self.files[index].installed = true;
			fs::set_permissions(&destination, permissions)?;
		} else {
			fs::hard_link(&temporary, &destination)?;
			self.files[index].installed = true;
			fs::remove_file(&temporary)?;
			self.files[index].temporary = None;
		}
		Ok(())
	}

	fn cleanup_backups<I>(&mut self, faults: &mut I) -> io::Result<()>
	where
		I: AtomicWriteFaultInjector,
	{
		for (index, file) in self.files.iter_mut().enumerate() {
			if let Some(backup) = file.backup.as_ref() {
				faults.check(
					AtomicWritePoint::BeforeBackupCleanup,
					index,
					&backup.directory,
				)?;
				remove_backup_if_exists(backup)?;
				file.backup = None;
			}
		}
		for file in &mut self.files {
			file.original_moved = false;
		}
		Ok(())
	}

	fn rollback<I>(&mut self, faults: &mut I) -> Vec<RollbackFailure>
	where
		I: AtomicWriteFaultInjector,
	{
		let mut failures = Vec::new();
		for (index, file) in self.files.iter_mut().enumerate().rev() {
			if file.installed {
				match faults
					.check(
						AtomicWritePoint::BeforeRollbackRemoveInstalled,
						index,
						&file.destination,
					)
					.and_then(|()| fs::remove_file(&file.destination))
				{
					Ok(()) => file.installed = false,
					Err(error) => failures.push(RollbackFailure::new(
						"remove installed output",
						&file.destination,
						error,
					)),
				}
			}

			if file.original_moved && !file.installed {
				match faults
					.check(
						AtomicWritePoint::BeforeRollbackRestore,
						index,
						&file.destination,
					)
					.and_then(|()| restore_original_file(file, index))
				{
					Ok(()) => file.original_moved = false,
					Err(error) => failures.push(RollbackFailure::new(
						"restore original",
						&file.destination,
						error,
					)),
				}
			}

			if let Some(temporary) = file.temporary.as_ref() {
				match faults
					.check(
						AtomicWritePoint::BeforeRollbackTemporaryCleanup,
						index,
						temporary,
					)
					.and_then(|()| fs::remove_file(temporary))
				{
					Ok(()) => file.temporary = None,
					Err(error) => failures.push(RollbackFailure::new(
						"remove temporary output",
						temporary,
						error,
					)),
				}
			}
			if !file.original_moved
				&& let Some(backup) = file.backup.as_ref()
			{
				match faults
					.check(
						AtomicWritePoint::BeforeRollbackBackupCleanup,
						index,
						&backup.directory,
					)
					.and_then(|()| remove_backup_if_exists(backup))
				{
					Ok(()) => file.backup = None,
					Err(error) => failures.push(RollbackFailure::new(
						"remove backup output",
						&backup.directory,
						error,
					)),
				}
			}
		}

		for (index, directory) in self.created_directories.iter().enumerate().rev() {
			if let Err(error) = faults
				.check(
					AtomicWritePoint::BeforeRollbackDirectoryCleanup,
					index,
					directory,
				)
				.and_then(|()| fs::remove_dir(directory))
			{
				failures.push(RollbackFailure::new(
					"remove generated directory",
					directory,
					error,
				));
			}
		}
		self.created_directories.clear();
		failures
	}
}

impl Drop for AtomicWriteTransaction {
	fn drop(&mut self) {
		if !self.finished {
			let mut faults = NoAtomicWriteFaults;
			let _ = self.rollback(&mut faults);
		}
	}
}

#[derive(Debug)]
struct RollbackFailure {
	action: &'static str,
	path: PathBuf,
	error: io::Error,
}

impl RollbackFailure {
	fn new(action: &'static str, path: &Path, error: io::Error) -> Self {
		Self {
			action,
			path: path.to_path_buf(),
			error,
		}
	}
}

fn aggregate_rollback_failures(
	trigger: io::Error,
	rollback_failures: &[RollbackFailure],
) -> io::Error {
	if rollback_failures.is_empty() {
		return trigger;
	}

	let details = rollback_failures
		.iter()
		.map(|failure| {
			format!(
				"{} for {:?}: {}",
				failure.action, failure.path, failure.error
			)
		})
		.collect::<Vec<_>>()
		.join("; ");
	io::Error::new(
		trigger.kind(),
		format!("{trigger}; rollback failures: {details}"),
	)
}

fn build_atomic_write_plan(
	output: &GeneratedOutput,
	force: bool,
) -> io::Result<(Vec<AtomicWriteFile>, Vec<PathBuf>)> {
	let mut destinations = HashSet::new();
	let mut files = Vec::with_capacity(output.files.len());
	let mut directories = HashSet::new();

	for generated in &output.files {
		let destination = normalize_destination_path(&generated.path)?;
		if !destinations.insert(destination.clone()) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				format!("Duplicate generated destination: {destination:?}"),
			));
		}

		let original = match fs::symlink_metadata(&destination) {
			Ok(metadata) => {
				if !force {
					return Err(io::Error::new(
						io::ErrorKind::AlreadyExists,
						format!("File already exists: {destination:?}"),
					));
				}
				if !metadata.file_type().is_file() {
					return Err(io::Error::new(
						io::ErrorKind::InvalidInput,
						format!("Generated destination is not a regular file: {destination:?}"),
					));
				}
				Some(OriginalFile {
					bytes: fs::read(&destination)?,
					permissions: metadata.permissions(),
				})
			}
			Err(error) if error.kind() == io::ErrorKind::NotFound => None,
			Err(error) => return Err(error),
		};

		for directory in missing_parent_directories(&destination)? {
			directories.insert(directory);
		}
		files.push(AtomicWriteFile {
			destination,
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

fn normalize_destination_path(destination: &Path) -> io::Result<PathBuf> {
	if destination.file_name().is_none() {
		return Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			format!("Generated destination has no file name: {destination:?}"),
		));
	}
	if destination
		.components()
		.any(|component| component == Component::ParentDir)
	{
		return Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			format!("Generated destination must not contain '..': {destination:?}"),
		));
	}

	let absolute = if destination.is_absolute() {
		destination.to_path_buf()
	} else {
		std::env::current_dir()?.join(destination)
	};
	let file_name = absolute
		.file_name()
		.expect("the destination file name was validated");
	let parent = absolute.parent().ok_or_else(|| {
		io::Error::new(
			io::ErrorKind::InvalidInput,
			format!("Generated destination has no parent: {destination:?}"),
		)
	})?;
	Ok(canonicalize_parent_with_missing_components(parent)?.join(file_name))
}

fn canonicalize_parent_with_missing_components(parent: &Path) -> io::Result<PathBuf> {
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
				let mut canonical = fs::canonicalize(current)?;
				for component in missing.iter().rev() {
					canonical.push(component);
				}
				return Ok(canonical);
			}
			Err(error) if error.kind() == io::ErrorKind::NotFound => {
				let component = current.file_name().ok_or_else(|| {
					io::Error::new(
						io::ErrorKind::NotFound,
						format!("Generated output parent does not exist: {parent:?}"),
					)
				})?;
				missing.push(component.to_os_string());
				current = current.parent().ok_or_else(|| {
					io::Error::new(
						io::ErrorKind::NotFound,
						format!("Generated output parent does not exist: {parent:?}"),
					)
				})?;
			}
			Err(error) => return Err(error),
		}
	}
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

fn create_temporary_sibling<I>(
	destination: &Path,
	label: &str,
	index: usize,
	faults: &mut I,
) -> io::Result<(PathBuf, File)>
where
	I: AtomicWriteFaultInjector,
{
	for _ in 0..100 {
		let candidate = sibling_path(destination, label)?;
		faults.check(AtomicWritePoint::BeforeTemporaryCreate, index, &candidate)?;
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

fn create_backup_sibling<I>(
	destination: &Path,
	index: usize,
	faults: &mut I,
) -> io::Result<BackupFile>
where
	I: AtomicWriteFaultInjector,
{
	for _ in 0..100 {
		let candidate = sibling_path(destination, "backup")?;
		faults.check(AtomicWritePoint::BeforeBackupCreate, index, &candidate)?;
		match fs::create_dir(&candidate) {
			Ok(()) => {
				return Ok(BackupFile {
					file: candidate.join("original"),
					directory: candidate,
				});
			}
			Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
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

fn restore_original_file(file: &AtomicWriteFile, index: usize) -> io::Result<()> {
	if let Some(backup) = file.backup.as_ref()
		&& backup.file.exists()
	{
		return fs::rename(&backup.file, &file.destination);
	}

	let original = file.original.as_ref().ok_or_else(|| {
		io::Error::new(
			io::ErrorKind::NotFound,
			format!(
				"Original output snapshot is unavailable: {:?}",
				file.destination
			),
		)
	})?;
	let mut faults = NoAtomicWriteFaults;
	let (temporary, mut temporary_file) =
		create_temporary_sibling(&file.destination, "restore", index, &mut faults)?;
	let mut cleanup = TemporaryFileCleanup::new(temporary.clone());
	let restore_result = (|| {
		temporary_file.write_all(&original.bytes)?;
		temporary_file.sync_all()?;
		drop(temporary_file);
		fs::set_permissions(&temporary, original.permissions.clone())?;
		fs::rename(&temporary, &file.destination)
	})();

	match restore_result {
		Ok(()) => {
			cleanup.disarm();
			Ok(())
		}
		Err(trigger) => match cleanup.cleanup() {
			Ok(()) => Err(trigger),
			Err(cleanup_error) => Err(io::Error::new(
				trigger.kind(),
				format!("{trigger}; restore cleanup failure for {temporary:?}: {cleanup_error}"),
			)),
		},
	}
}

fn remove_file_if_exists(path: &Path) -> io::Result<()> {
	match fs::remove_file(path) {
		Ok(()) => Ok(()),
		Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
		Err(error) => Err(error),
	}
}

fn remove_directory_if_exists(path: &Path) -> io::Result<()> {
	match fs::remove_dir(path) {
		Ok(()) => Ok(()),
		Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
		Err(error) => Err(error),
	}
}

fn remove_backup_if_exists(backup: &BackupFile) -> io::Result<()> {
	remove_file_if_exists(&backup.file)?;
	remove_directory_if_exists(&backup.directory)
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

	fn cleanup(&mut self) -> io::Result<()> {
		if let Some(path) = self.path.as_ref() {
			remove_file_if_exists(path)?;
			self.path = None;
		}
		Ok(())
	}
}

impl Drop for TemporaryFileCleanup {
	fn drop(&mut self) {
		if let Some(path) = self.path.take() {
			let _ = fs::remove_file(path);
		}
	}
}

#[cfg(test)]
mod atomic_write_tests {
	use super::*;

	struct CallbackFaults<F>(F);

	impl<F> AtomicWriteFaultInjector for CallbackFaults<F>
	where
		F: FnMut(AtomicWritePoint, usize, &Path) -> io::Result<()>,
	{
		fn check(&mut self, point: AtomicWritePoint, index: usize, path: &Path) -> io::Result<()> {
			(self.0)(point, index, path)
		}
	}

	fn output(path: &Path, content: &str) -> GeneratedOutput {
		GeneratedOutput {
			files: vec![GeneratedFile::new(path, content)],
		}
	}

	fn entries(path: &Path) -> Vec<String> {
		let mut entries: Vec<_> = fs::read_dir(path)
			.expect("test directory should remain readable")
			.map(|entry| {
				entry
					.expect("test entry should remain readable")
					.file_name()
					.into_string()
					.expect("test file names should be UTF-8")
			})
			.collect();
		entries.sort();
		entries
	}

	fn io_error(error: MigrationError) -> io::Error {
		match error {
			MigrationError::IoError(error) => error,
			other => panic!("expected an I/O error, got {other:?}"),
		}
	}

	#[test]
	fn non_force_race_does_not_overwrite_concurrent_destination() {
		let temp_dir = tempfile::Builder::new()
			.prefix("inspectdb-output-")
			.tempdir_in("/tmp")
			.expect("temporary directory should be created");
		let destination = temp_dir.path().join("concurrent.rs");
		let mut faults = CallbackFaults(|point, index, path: &Path| {
			if point == AtomicWritePoint::BeforeInstall && index == 0 {
				fs::write(path, b"concurrent bytes")
					.expect("concurrent destination should be created");
			}
			Ok(())
		});

		let error = write_generated_files_atomically_with_faults(
			&output(&destination, "generated bytes"),
			false,
			&mut faults,
		)
		.expect_err("concurrent creation should make installation fail");
		let error = io_error(error);

		assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
		assert_eq!(
			fs::read(&destination).expect("concurrent destination should remain readable"),
			b"concurrent bytes"
		);
		assert_eq!(entries(temp_dir.path()), vec!["concurrent.rs".to_string()]);
	}

	#[test]
	fn rollback_restores_original_after_failure_after_original_move() {
		let temp_dir = tempfile::Builder::new()
			.prefix("inspectdb-output-")
			.tempdir_in("/tmp")
			.expect("temporary directory should be created");
		let destination = temp_dir.path().join("existing.rs");
		fs::write(&destination, b"original bytes").expect("original file should be created");
		let mut faults = CallbackFaults(|point, index, _path: &Path| {
			if point == AtomicWritePoint::AfterOriginalMove && index == 0 {
				Err(io::Error::other("injected post-move failure"))
			} else {
				Ok(())
			}
		});

		let error = write_generated_files_atomically_with_faults(
			&output(&destination, "replacement bytes"),
			true,
			&mut faults,
		)
		.expect_err("post-move failure should be returned");

		assert_eq!(io_error(error).to_string(), "injected post-move failure");
		assert_eq!(
			fs::read(&destination).expect("original destination should be restored"),
			b"original bytes"
		);
		assert_eq!(entries(temp_dir.path()), vec!["existing.rs".to_string()]);
	}

	#[test]
	fn rollback_removes_new_file_after_install_failure() {
		let temp_dir = tempfile::Builder::new()
			.prefix("inspectdb-output-")
			.tempdir_in("/tmp")
			.expect("temporary directory should be created");
		let destination = temp_dir.path().join("new.rs");
		let mut faults = CallbackFaults(|point, index, _path: &Path| {
			if point == AtomicWritePoint::BeforeInstall && index == 0 {
				Err(io::Error::other("injected install failure"))
			} else {
				Ok(())
			}
		});

		let error = write_generated_files_atomically_with_faults(
			&output(&destination, "generated bytes"),
			false,
			&mut faults,
		)
		.expect_err("install failure should be returned");

		assert_eq!(io_error(error).to_string(), "injected install failure");
		assert!(!destination.exists());
		assert_eq!(entries(temp_dir.path()), Vec::<String>::new());
	}

	#[test]
	fn rollback_restores_original_after_backup_cleanup_failure() {
		let temp_dir = tempfile::Builder::new()
			.prefix("inspectdb-output-")
			.tempdir_in("/tmp")
			.expect("temporary directory should be created");
		let destination = temp_dir.path().join("existing.rs");
		fs::write(&destination, b"original bytes").expect("original file should be created");
		let mut faults = CallbackFaults(|point, index, _path: &Path| {
			if point == AtomicWritePoint::BeforeBackupCleanup && index == 0 {
				Err(io::Error::other("injected backup cleanup failure"))
			} else {
				Ok(())
			}
		});

		let error = write_generated_files_atomically_with_faults(
			&output(&destination, "replacement bytes"),
			true,
			&mut faults,
		)
		.expect_err("backup cleanup failure should be returned");

		assert_eq!(
			io_error(error).to_string(),
			"injected backup cleanup failure"
		);
		assert_eq!(
			fs::read(&destination).expect("original destination should be restored"),
			b"original bytes"
		);
		assert_eq!(entries(temp_dir.path()), vec!["existing.rs".to_string()]);
	}

	#[test]
	fn rollback_failure_is_reported_and_backup_is_retained() {
		let temp_dir = tempfile::Builder::new()
			.prefix("inspectdb-output-")
			.tempdir_in("/tmp")
			.expect("temporary directory should be created");
		let destination = temp_dir.path().join("existing.rs");
		fs::write(&destination, b"original bytes").expect("original file should be created");
		let mut faults = CallbackFaults(|point, index, _path: &Path| match (point, index) {
			(AtomicWritePoint::BeforeInstall, 0) => {
				Err(io::Error::other("injected install failure"))
			}
			(AtomicWritePoint::BeforeRollbackRestore, 0) => {
				Err(io::Error::other("injected rollback restoration failure"))
			}
			_ => Ok(()),
		});

		let error = write_generated_files_atomically_with_faults(
			&output(&destination, "replacement bytes"),
			true,
			&mut faults,
		)
		.expect_err("install and rollback failures should be returned");
		let error = io_error(error);
		let normalized_destination = fs::canonicalize(temp_dir.path())
			.expect("test directory should be canonicalizable")
			.join("existing.rs");
		let expected = format!(
			"injected install failure; rollback failures: restore original for \
			 {normalized_destination:?}: injected rollback restoration failure"
		);

		assert_eq!(error.kind(), io::ErrorKind::Other);
		assert_eq!(error.to_string(), expected);
		assert!(!destination.exists());
		let entries = entries(temp_dir.path());
		assert_eq!(entries.len(), 1);
		assert!(entries[0].starts_with(".existing.rs.reinhardt-backup-"));
		let backup = temp_dir.path().join(&entries[0]).join("original");
		assert_eq!(
			fs::read(backup).expect("retained backup should contain the original"),
			b"original bytes"
		);
	}

	#[test]
	fn temporary_sibling_collision_is_not_overwritten() {
		let temp_dir = tempfile::Builder::new()
			.prefix("inspectdb-output-")
			.tempdir_in("/tmp")
			.expect("temporary directory should be created");
		let destination = temp_dir.path().join("new.rs");
		let mut collision = None;
		let mut faults = CallbackFaults(|point, index, path: &Path| {
			if point == AtomicWritePoint::BeforeTemporaryCreate && index == 0 && collision.is_none()
			{
				fs::write(path, b"concurrent temporary")
					.expect("temporary collision should be created");
				collision = Some(path.to_path_buf());
			}
			Ok(())
		});

		write_generated_files_atomically_with_faults(
			&output(&destination, "generated bytes"),
			false,
			&mut faults,
		)
		.expect("a fresh temporary sibling should be retried");
		let collision = collision.expect("collision path should be captured");

		assert_eq!(
			fs::read(&collision).expect("colliding temporary should remain readable"),
			b"concurrent temporary"
		);
		assert_eq!(
			fs::read(&destination).expect("generated destination should be readable"),
			b"generated bytes"
		);
		let collision_name = collision
			.file_name()
			.expect("collision should have a file name")
			.to_string_lossy()
			.into_owned();
		let mut expected_entries = vec![collision_name, "new.rs".to_string()];
		expected_entries.sort();
		assert_eq!(entries(temp_dir.path()), expected_entries);
	}

	#[test]
	fn backup_sibling_collision_is_not_overwritten() {
		let temp_dir = tempfile::Builder::new()
			.prefix("inspectdb-output-")
			.tempdir_in("/tmp")
			.expect("temporary directory should be created");
		let destination = temp_dir.path().join("existing.rs");
		fs::write(&destination, b"original bytes").expect("original file should be created");
		let mut collision = None;
		let mut faults = CallbackFaults(|point, index, path: &Path| {
			if point == AtomicWritePoint::BeforeBackupCreate && index == 0 && collision.is_none() {
				fs::write(path, b"concurrent backup").expect("backup collision should be created");
				collision = Some(path.to_path_buf());
			}
			Ok(())
		});

		write_generated_files_atomically_with_faults(
			&output(&destination, "replacement bytes"),
			true,
			&mut faults,
		)
		.expect("a fresh backup sibling should be retried");
		let collision = collision.expect("collision path should be captured");

		assert_eq!(
			fs::read(&collision).expect("colliding backup should remain readable"),
			b"concurrent backup"
		);
		assert_eq!(
			fs::read(&destination).expect("generated destination should be readable"),
			b"replacement bytes"
		);
		let collision_name = collision
			.file_name()
			.expect("collision should have a file name")
			.to_string_lossy()
			.into_owned();
		let mut expected_entries = vec![collision_name, "existing.rs".to_string()];
		expected_entries.sort();
		assert_eq!(entries(temp_dir.path()), expected_entries);
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
