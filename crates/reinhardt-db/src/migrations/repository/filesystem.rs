//! Filesystem-based migration repository
//!
//! Persists migrations as `.rs` files on disk.

use super::{Migration, MigrationError, MigrationRepository, Result};
use crate::migrations::ast_parser;
use crate::migrations::dependency::DependencyCondition;
use async_trait::async_trait;
use cap_std::ambient_authority;
use cap_std::fs::{Dir, File, OpenOptions};
use quote::quote;
use reinhardt_query::prelude::{
	ColumnType as QueryColumnType, GeneratedStorage, SchemaBinOper, SchemaExpr, SchemaFunc, Value,
};
use std::io::Write;
use std::path::{Path, PathBuf};
use syn::parse_quote;

/// Controls optional content in rendered migration source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationRenderOptions {
	/// Include the generated-file header.
	pub include_header: bool,
}

/// Repository that persists migrations as `.rs` files
///
/// This repository writes migrations to disk in the format:
/// ```rust,no_run
/// // <app_label>/migrations/<name>.rs
/// use reinhardt_db::migrations::Migration;
/// // use reinhardt::db::migrations::prelude::*;
/// // use reinhardt::db::migrations::FieldType;
///
/// fn migration() -> Migration {
///     Migration::new("0001_initial", "app")
/// }
/// ```
pub struct FilesystemRepository {
	/// Root directory for migration files
	root_dir: PathBuf,
}

#[derive(Debug, Eq, PartialEq)]
struct DirectoryIdentity(same_file::Handle);

fn directory_identity(directory: &Dir) -> std::io::Result<DirectoryIdentity> {
	let file = directory.try_clone()?.into_std_file();
	same_file::Handle::from_file(file).map(DirectoryIdentity)
}

fn path_directory_identity(path: &Path) -> std::io::Result<DirectoryIdentity> {
	same_file::Handle::from_path(path).map(DirectoryIdentity)
}

struct RootValidationHooks<B, V> {
	before_open: B,
	after_identity_open: V,
}

struct IncompleteFile<C>
where
	C: Fn(&Dir, &str) -> std::io::Result<()>,
{
	path: PathBuf,
	directory: Dir,
	name: String,
	file: Option<File>,
	complete: bool,
	cleanup: C,
}

impl<C> IncompleteFile<C>
where
	C: Fn(&Dir, &str) -> std::io::Result<()>,
{
	fn new(path: PathBuf, directory: Dir, name: String, file: File, cleanup: C) -> Self {
		Self {
			path,
			directory,
			name,
			file: Some(file),
			complete: false,
			cleanup,
		}
	}

	fn file_mut(&mut self) -> &mut File {
		self.file.as_mut().expect("incomplete file must be open")
	}

	fn complete(mut self) -> PathBuf {
		self.complete = true;
		self.path.clone()
	}

	fn cleanup_now(&mut self) -> std::io::Result<()> {
		self.file.take();
		(self.cleanup)(&self.directory, &self.name)?;
		self.complete = true;
		Ok(())
	}
}

impl<C> Drop for IncompleteFile<C>
where
	C: Fn(&Dir, &str) -> std::io::Result<()>,
{
	fn drop(&mut self) {
		self.file.take();
		if !self.complete {
			let _ = (self.cleanup)(&self.directory, &self.name);
		}
	}
}

impl FilesystemRepository {
	fn operation_kind(operation: &crate::migrations::Operation) -> &'static str {
		use crate::migrations::Operation;

		match operation {
			Operation::CreateTable { .. } => "CreateTable",
			Operation::DropTable { .. } => "DropTable",
			Operation::AddColumn { .. } => "AddColumn",
			Operation::DropColumn { .. } => "DropColumn",
			Operation::AlterColumn { .. } => "AlterColumn",
			Operation::RenameTable { .. } => "RenameTable",
			Operation::RenameColumn { .. } => "RenameColumn",
			Operation::AddConstraint { .. } => "AddConstraint",
			Operation::AddConstraintDefinition { .. } => "AddConstraintDefinition",
			Operation::AddConstraintRepair { .. } => "AddConstraintRepair",
			Operation::RestoreConstraintOnRollback { .. } => "RestoreConstraintOnRollback",
			Operation::DropConstraint { .. } => "DropConstraint",
			Operation::DropConstraintDefinition { .. } => "DropConstraintDefinition",
			Operation::CreateIndex { .. } => "CreateIndex",
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex { .. } => "CreateNamedIndex",
			Operation::CreateIndexRepair { .. } => "CreateIndexRepair",
			Operation::RestoreIndexOnRollback { .. } => "RestoreIndexOnRollback",
			Operation::DropIndex { .. } => "DropIndex",
			#[cfg(feature = "pgvector")]
			Operation::DropNamedIndex { .. } => "DropNamedIndex",
			Operation::RunSQL { .. } => "RunSQL",
			Operation::RunRust { .. } => "RunRust",
			Operation::AlterTableComment { .. } => "AlterTableComment",
			Operation::AlterUniqueTogether { .. } => "AlterUniqueTogether",
			Operation::AlterModelOptions { .. } => "AlterModelOptions",
			Operation::CreateInheritedTable { .. } => "CreateInheritedTable",
			Operation::AddDiscriminatorColumn { .. } => "AddDiscriminatorColumn",
			Operation::MoveModel { .. } => "MoveModel",
			Operation::CreateSchema { .. } => "CreateSchema",
			Operation::DropSchema { .. } => "DropSchema",
			Operation::CreateExtension { .. } => "CreateExtension",
			Operation::BulkLoad { .. } => "BulkLoad",
			Operation::SetAutoIncrementValue { .. } => "SetAutoIncrementValue",
			Operation::CreateCompositePrimaryKey { .. } => "CreateCompositePrimaryKey",
		}
	}

	/// Create a new FilesystemRepository
	///
	/// # Arguments
	///
	/// * `root_dir` - Root directory where migration files will be stored
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::FilesystemRepository;
	/// let repo = FilesystemRepository::new("./migrations");
	/// ```
	pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
		Self {
			root_dir: root_dir.as_ref().to_path_buf(),
		}
	}

	/// Validate that a path component does not contain traversal sequences.
	///
	/// Rejects components containing `..`, path separators, or null bytes
	/// to prevent directory traversal attacks that could escape the
	/// migration root directory.
	fn validate_path_component(component: &str, label: &str) -> Result<()> {
		if component.is_empty() {
			return Err(MigrationError::PathTraversal(format!(
				"{} cannot be empty",
				label
			)));
		}

		// Reject path traversal sequences
		if component.contains("..") {
			return Err(MigrationError::PathTraversal(format!(
				"{} contains path traversal sequence '..': {}",
				label, component
			)));
		}

		// Reject path separators (both Unix and Windows)
		if component.contains('/') || component.contains('\\') {
			return Err(MigrationError::PathTraversal(format!(
				"{} contains path separator: {}",
				label, component
			)));
		}

		// Reject null bytes
		if component.contains('\0') {
			return Err(MigrationError::PathTraversal(format!(
				"{} contains null byte: {}",
				label, component
			)));
		}

		let valid = if label == "Migration name" {
			let Some((number, description)) = component.split_once('_') else {
				return Err(MigrationError::PathTraversal(format!(
					"{} is not a numbered migration name: {}",
					label, component
				)));
			};
			number.len() >= 4
				&& number.bytes().all(|byte| byte.is_ascii_digit())
				&& !description.is_empty()
				&& description
					.bytes()
					.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
		} else {
			component
				.as_bytes()
				.first()
				.is_some_and(u8::is_ascii_alphabetic)
				&& component
					.bytes()
					.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
		};
		if !valid {
			return Err(MigrationError::PathTraversal(format!(
				"{} is not a safe ASCII component: {}",
				label, component
			)));
		}

		Ok(())
	}

	/// Get the path for a migration file
	///
	/// Returns: `<root_dir>/<app_label>/<name>.rs`
	///
	/// Validates that `app_label` and `name` do not contain path traversal
	/// sequences before constructing the path.
	fn migration_path(&self, app_label: &str, name: &str) -> Result<PathBuf> {
		Self::validate_path_component(app_label, "App label")?;
		Self::validate_path_component(name, "Migration name")?;

		let path = self.root_dir.join(app_label).join(format!("{}.rs", name));

		// Final safety check: if both paths can be canonicalized (i.e., exist on disk),
		// verify the resolved path stays within root_dir.
		// When directories don't exist yet (e.g., during save), the component-level
		// validation above is sufficient to prevent traversal.
		if let (Ok(canonical_root), Some(parent)) = (self.root_dir.canonicalize(), path.parent())
			&& let Ok(canonical_parent) = parent.canonicalize()
			&& !canonical_parent.starts_with(&canonical_root)
		{
			return Err(MigrationError::PathTraversal(format!(
				"Resolved path escapes migration root directory: {}",
				path.display()
			)));
		}

		Ok(path)
	}

	/// Render a complete Rust migration source file.
	pub fn render(&self, migration: &Migration, options: MigrationRenderOptions) -> Result<String> {
		for (index, operation) in migration.operations.iter().enumerate() {
			Self::validate_renderable_operation(index, operation)?;
		}
		// Build dependencies vector (tuple elements need .to_string() for String type)
		let deps: Vec<_> = migration
			.dependencies
			.iter()
			.map(|(app, name)| {
				quote! { (#app.to_string(), #name.to_string()) }
			})
			.collect();

		// Build replaces vector (tuple elements need .to_string() for String type)
		let replaces: Vec<_> = migration
			.replaces
			.iter()
			.map(|(app, name)| {
				quote! { (#app.to_string(), #name.to_string()) }
			})
			.collect();
		let swappable_dependencies = migration.swappable_dependencies.iter().map(|dependency| {
			let setting_key = &dependency.setting_key;
			let default_app = &dependency.default_app;
			let default_model = &dependency.default_model;
			let migration_name = &dependency.migration_name;
			quote! {
				SwappableDependency::new(
					#setting_key,
					#default_app,
					#default_model,
					#migration_name,
				)
			}
		});
		let optional_dependencies = migration.optional_dependencies.iter().map(|dependency| {
			let app_label = &dependency.app_label;
			let migration_name = &dependency.migration_name;
			let condition = match &dependency.condition {
				DependencyCondition::AppInstalled(value) => {
					quote! { DependencyCondition::AppInstalled(#value.to_string()) }
				}
				DependencyCondition::SettingEnabled(value) => {
					quote! { DependencyCondition::SettingEnabled(#value.to_string()) }
				}
				DependencyCondition::FeatureEnabled(value) => {
					quote! { DependencyCondition::FeatureEnabled(#value.to_string()) }
				}
			};
			quote! {
				OptionalDependency::new(#app_label, #migration_name, #condition)
			}
		});

		let app_label = &migration.app_label;
		let name = &migration.name;
		let atomic = migration.atomic;
		let state_only = migration.state_only;
		let database_only = migration.database_only;

		// Build initial field token
		let initial_tokens = match migration.initial {
			Some(true) => quote! { Some(true) },
			Some(false) => quote! { Some(false) },
			None => quote! { None },
		};

		// Generate operation code
		let ops_tokens = migration.operations.iter();
		let operations_code = quote! { vec![#(#ops_tokens),*] };

		// Generate full migration file
		let file: syn::File = parse_quote! {
			use reinhardt::db::migrations::prelude::*;
			use reinhardt::db::migrations::dependency::{
				DependencyCondition, OptionalDependency, SwappableDependency,
			};
			use reinhardt::db::migrations::FieldType;

			pub(super) fn migration() -> Migration {
				Migration {
					app_label: #app_label.to_string(),
					name: #name.to_string(),
					operations: #operations_code,
					dependencies: vec![#(#deps),*],
					atomic: #atomic,
					replaces: vec![#(#replaces),*],
					initial: #initial_tokens,
					state_only: #state_only,
					database_only: #database_only,
					swappable_dependencies: vec![#(#swappable_dependencies),*],
					optional_dependencies: vec![#(#optional_dependencies),*],
				}
			}
		};

		// Format with prettyplease first, then apply rustfmt
		let prettyplease_output = prettyplease::unparse(&file);
		let formatted = Self::format_with_rustfmt(&prettyplease_output)?;
		let parsed = syn::parse_file(&formatted).map_err(|error| {
			let operations = migration
				.operations
				.iter()
				.map(Self::operation_kind)
				.collect::<Vec<_>>()
				.join(", ");
			MigrationError::UnsupportedMigrationRendering {
				operation: format!("{operations}: {error}"),
			}
		})?;
		let reparsed = ast_parser::extract_migration_metadata_strict(
			&parsed,
			&migration.app_label,
			&migration.name,
		)
		.map_err(|error| {
			let context = match error {
				MigrationError::InvalidMigration(message) if message.starts_with("operations[") => {
					message
				}
				other => format!("Migration.metadata: {other}"),
			};
			MigrationError::UnsupportedMigrationRendering { operation: context }
		})?;
		if let Some((index, (expected, _))) = migration
			.operations
			.iter()
			.zip(&reparsed.operations)
			.enumerate()
			.find(|(_, (expected, actual))| expected != actual)
		{
			return Err(MigrationError::UnsupportedMigrationRendering {
				operation: format!(
					"operations[{}].{}: semantic mismatch",
					index,
					Self::operation_kind(expected)
				),
			});
		}
		if migration.operations.len() != reparsed.operations.len() {
			let index = migration.operations.len().min(reparsed.operations.len());
			let kind = migration
				.operations
				.get(index)
				.map(Self::operation_kind)
				.unwrap_or("unexpected");
			return Err(MigrationError::UnsupportedMigrationRendering {
				operation: format!("operations[{index}].{kind}: operation count mismatch"),
			});
		}
		let metadata_mismatch = [
			(
				migration.dependencies != reparsed.dependencies,
				"Migration.dependencies",
			),
			(
				migration.replaces != reparsed.replaces,
				"Migration.replaces",
			),
			(migration.atomic != reparsed.atomic, "Migration.atomic"),
			(migration.initial != reparsed.initial, "Migration.initial"),
			(
				migration.state_only != reparsed.state_only,
				"Migration.state_only",
			),
			(
				migration.database_only != reparsed.database_only,
				"Migration.database_only",
			),
			(
				migration.swappable_dependencies != reparsed.swappable_dependencies,
				"Migration.swappable_dependencies",
			),
			(
				migration.optional_dependencies != reparsed.optional_dependencies,
				"Migration.optional_dependencies",
			),
		]
		.into_iter()
		.find_map(|(mismatch, context)| mismatch.then_some(context));
		if let Some(context) = metadata_mismatch {
			return Err(MigrationError::UnsupportedMigrationRendering {
				operation: format!("{context}: semantic mismatch"),
			});
		}

		if options.include_header {
			Ok(format!(
				"// Generated by Reinhardt migrations.\n\n{formatted}"
			))
		} else {
			Ok(formatted)
		}
	}

	fn validate_renderable_operation(
		index: usize,
		operation: &crate::migrations::Operation,
	) -> Result<()> {
		use crate::migrations::{Constraint, Operation};

		let kind = Self::operation_kind(operation);
		let context = format!("operations[{index}].{kind}");
		match operation {
			Operation::CreateTable { columns, .. }
			| Operation::CreateInheritedTable { columns, .. } => {
				for column in columns {
					Self::validate_renderable_column(&context, column)?;
				}
			}
			Operation::AddColumn { column, .. } => {
				Self::validate_renderable_column(&context, column)?;
			}
			Operation::DropColumn {
				old_definition: Some(column),
				..
			} => {
				Self::validate_renderable_column(&context, column)?;
			}
			Operation::AlterColumn {
				old_definition,
				new_definition,
				..
			} => {
				if let Some(column) = old_definition {
					Self::validate_renderable_column(&context, column)?;
				}
				Self::validate_renderable_column(&context, new_definition)?;
			}
			_ => {}
		}
		let constraints = match operation {
			Operation::CreateTable { constraints, .. } => Some(constraints.as_slice()),
			Operation::AddConstraintDefinition { constraint, .. }
			| Operation::DropConstraintDefinition { constraint, .. } => {
				Some(std::slice::from_ref(constraint))
			}
			_ => None,
		};
		if constraints.is_some_and(|constraints| {
			constraints
				.iter()
				.any(|constraint| matches!(constraint, Constraint::Exclude { .. }))
		}) {
			return Err(MigrationError::UnsupportedMigrationRendering {
				operation: format!("{context}.Constraint::Exclude"),
			});
		}
		match operation {
			Operation::CreateTable {
				without_rowid,
				interleave_in_parent,
				partition,
				..
			} if without_rowid.is_some()
				|| interleave_in_parent.is_some()
				|| partition.is_some() =>
			{
				Err(MigrationError::UnsupportedMigrationRendering { operation: context })
			}
			Operation::AddColumn {
				mysql_options: Some(_),
				..
			}
			| Operation::AlterColumn {
				old_definition: Some(_),
				..
			}
			| Operation::AlterColumn {
				mysql_options: Some(_),
				..
			}
			| Operation::RunRust { .. }
			| Operation::AlterTableComment { .. }
			| Operation::AlterUniqueTogether { .. }
			| Operation::AlterModelOptions { .. }
			| Operation::CreateInheritedTable { .. }
			| Operation::AddDiscriminatorColumn { .. }
			| Operation::MoveModel { .. }
			| Operation::CreateSchema { .. }
			| Operation::DropSchema { .. }
			| Operation::BulkLoad { .. }
			| Operation::SetAutoIncrementValue { .. }
			| Operation::CreateCompositePrimaryKey { .. } => {
				Err(MigrationError::UnsupportedMigrationRendering { operation: context })
			}
			_ => Ok(()),
		}
	}

	fn validate_renderable_column(
		context: &str,
		column: &crate::migrations::ColumnDefinition,
	) -> Result<()> {
		let Some(generated) = &column.generated else {
			return Ok(());
		};
		let context = format!("{context}.GeneratedColumnDefinition");
		match generated.storage {
			GeneratedStorage::Stored | GeneratedStorage::Virtual => {}
			_ => return Self::unsupported_rendering(format!("{context}.GeneratedStorage")),
		}
		if generated.expr.is_none() && generated.expr_tokens.is_some() {
			let Some(expression) = generated.typed_expr() else {
				return Self::unsupported_rendering(format!("{context}.expr_tokens"));
			};
			return Self::validate_schema_expr(&context, &expression);
		}
		if let Some(expression) = generated.expr.as_deref() {
			Self::validate_schema_expr(&context, expression)?;
		}
		Ok(())
	}

	fn validate_schema_expr(context: &str, expression: &SchemaExpr) -> Result<()> {
		match expression {
			SchemaExpr::Column(_) => Ok(()),
			SchemaExpr::Value(value) => Self::validate_schema_value(context, value),
			SchemaExpr::Binary { left, op, right } => {
				match op {
					SchemaBinOper::Add
					| SchemaBinOper::Sub
					| SchemaBinOper::Mul
					| SchemaBinOper::Div => {}
					_ => {
						return Self::unsupported_rendering(format!(
							"{context}.SchemaExpr.SchemaBinOper"
						));
					}
				}
				Self::validate_schema_expr(context, left)?;
				Self::validate_schema_expr(context, right)
			}
			SchemaExpr::Function { func, args } => {
				match func {
					SchemaFunc::Concat => {}
					SchemaFunc::Coalesce if !args.is_empty() => {}
					SchemaFunc::Coalesce => {
						return Self::unsupported_rendering(format!(
							"{context}.SchemaExpr.SchemaFunc::Coalesce"
						));
					}
					_ => {
						return Self::unsupported_rendering(format!(
							"{context}.SchemaExpr.SchemaFunc"
						));
					}
				}
				for argument in args {
					Self::validate_schema_expr(context, argument)?;
				}
				Ok(())
			}
			SchemaExpr::Cast { expr, ty } => {
				Self::validate_schema_expr(context, expr)?;
				Self::validate_query_column_type(context, ty)
			}
			_ => Self::unsupported_rendering(format!("{context}.SchemaExpr")),
		}
	}

	fn validate_schema_value(context: &str, value: &Value) -> Result<()> {
		match value {
			Value::Bool(_)
			| Value::TinyInt(_)
			| Value::SmallInt(_)
			| Value::Int(_)
			| Value::BigInt(_)
			| Value::TinyUnsigned(_)
			| Value::SmallUnsigned(_)
			| Value::Unsigned(_)
			| Value::BigUnsigned(_)
			| Value::Float(_)
			| Value::Double(_)
			| Value::Char(_)
			| Value::String(_) => Ok(()),
			Value::Bytes(_) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::Bytes"))
			}
			#[cfg(feature = "pgvector")]
			Value::Vector(_) => Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::Vector")),
			Value::ChronoDate(_) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::ChronoDate"))
			}
			Value::ChronoTime(_) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::ChronoTime"))
			}
			Value::ChronoDateTime(_) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::ChronoDateTime"))
			}
			Value::ChronoDateTimeUtc(_) => Self::unsupported_rendering(format!(
				"{context}.SchemaExpr.Value::ChronoDateTimeUtc"
			)),
			Value::ChronoDateTimeLocal(_) => Self::unsupported_rendering(format!(
				"{context}.SchemaExpr.Value::ChronoDateTimeLocal"
			)),
			Value::ChronoDateTimeWithTimeZone(_) => Self::unsupported_rendering(format!(
				"{context}.SchemaExpr.Value::ChronoDateTimeWithTimeZone"
			)),
			Value::Uuid(_) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::Uuid"))
			}
			Value::Json(_) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::Json"))
			}
			Value::Decimal(_) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::Decimal"))
			}
			Value::BigDecimal(_) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::BigDecimal"))
			}
			Value::Array(_, _) => {
				Self::unsupported_rendering(format!("{context}.SchemaExpr.Value::Array"))
			}
		}
	}

	fn validate_query_column_type(context: &str, column_type: &QueryColumnType) -> Result<()> {
		match column_type {
			QueryColumnType::Char(_)
			| QueryColumnType::String(_)
			| QueryColumnType::Text
			| QueryColumnType::TinyInteger
			| QueryColumnType::SmallInteger
			| QueryColumnType::Integer
			| QueryColumnType::BigInteger
			| QueryColumnType::Float
			| QueryColumnType::Double
			| QueryColumnType::Decimal(_)
			| QueryColumnType::Boolean
			| QueryColumnType::Date
			| QueryColumnType::Time
			| QueryColumnType::DateTime
			| QueryColumnType::Timestamp
			| QueryColumnType::TimestampWithTimeZone
			| QueryColumnType::Binary(_)
			| QueryColumnType::VarBinary(_)
			| QueryColumnType::Blob
			| QueryColumnType::Uuid
			| QueryColumnType::Json
			| QueryColumnType::JsonBinary
			| QueryColumnType::Custom(_) => Ok(()),
			QueryColumnType::Array(inner) => Self::validate_query_column_type(context, inner),
			#[cfg(feature = "pgvector")]
			QueryColumnType::Vector(_) => Ok(()),
			_ => Self::unsupported_rendering(format!("{context}.SchemaExpr.ColumnType")),
		}
	}

	fn unsupported_rendering<T>(operation: String) -> Result<T> {
		Err(MigrationError::UnsupportedMigrationRendering { operation })
	}

	/// Create a new migration source file without overwriting an existing file.
	pub fn create_new_source(
		&self,
		app_label: &str,
		migration_name: &str,
		source: &str,
	) -> Result<PathBuf> {
		self.create_new_source_with_hooks(
			app_label,
			migration_name,
			source,
			RootValidationHooks {
				before_open: || Ok(()),
				after_identity_open: || Ok(()),
			},
			|file, formatted| {
				file.write_all(formatted.as_bytes())?;
				file.flush()?;
				file.sync_all()
			},
			|directory, name| directory.remove_file(name),
		)
	}

	fn create_new_source_with_hooks<B, V, W, C>(
		&self,
		app_label: &str,
		migration_name: &str,
		source: &str,
		hooks: RootValidationHooks<B, V>,
		write_source: W,
		cleanup: C,
	) -> Result<PathBuf>
	where
		B: FnOnce() -> std::io::Result<()>,
		V: FnOnce() -> std::io::Result<()>,
		W: FnOnce(&mut File, &str) -> std::io::Result<()>,
		C: Fn(&Dir, &str) -> std::io::Result<()>,
	{
		let RootValidationHooks {
			before_open,
			after_identity_open,
		} = hooks;
		let path = self.migration_path(app_label, migration_name)?;
		syn::parse_file(source).map_err(|error| {
			MigrationError::InvalidMigration(format!("Failed to parse migration source: {error}"))
		})?;
		let formatted = Self::format_with_rustfmt(source)?;
		std::fs::create_dir_all(&self.root_dir)?;
		let root = Dir::open_ambient_dir(&self.root_dir, ambient_authority())?;
		let root_identity = directory_identity(&root)?;
		before_open()?;
		self.validate_root_identity(&root_identity, || Ok(()))?;
		root.create_dir_all(app_label)?;
		let app_directory = root.open_dir(app_label)?;
		let file_name = format!("{migration_name}.rs");
		let mut options = OpenOptions::new();
		options.write(true).create_new(true);
		let file = app_directory.open_with(&file_name, &options)?;
		let mut incomplete = IncompleteFile::new(path, app_directory, file_name, file, cleanup);
		if let Err(write_error) = write_source(incomplete.file_mut(), &formatted) {
			return match incomplete.cleanup_now() {
				Ok(()) => Err(MigrationError::IoError(write_error)),
				Err(cleanup_error) => Err(MigrationError::IoError(std::io::Error::other(format!(
					"Failed to write migration source: {write_error}; failed to remove incomplete \
					 file: {cleanup_error}"
				)))),
			};
		}
		if let Err(identity_error) =
			self.validate_root_identity(&root_identity, after_identity_open)
		{
			return match incomplete.cleanup_now() {
				Ok(()) => Err(identity_error),
				Err(cleanup_error) => Err(MigrationError::IoError(std::io::Error::other(format!(
					"{identity_error}; failed to remove incomplete file after root directory \
					 identity validation: {cleanup_error}"
				)))),
			};
		}
		Ok(incomplete.complete())
	}

	fn validate_root_identity<V>(&self, expected: &DirectoryIdentity, after_open: V) -> Result<()>
	where
		V: FnOnce() -> std::io::Result<()>,
	{
		let before_open = std::fs::symlink_metadata(&self.root_dir).map_err(|error| {
			MigrationError::PathTraversal(format!(
				"Migration root directory identity is unavailable: {error}"
			))
		})?;
		if before_open.file_type().is_symlink() {
			return Err(MigrationError::PathTraversal(
				"Migration root directory identity changed to a symbolic link".to_string(),
			));
		}
		let ambient =
			Dir::open_ambient_dir(&self.root_dir, ambient_authority()).map_err(|error| {
				MigrationError::PathTraversal(format!(
					"Migration root directory identity is unavailable: {error}"
				))
			})?;
		let actual = directory_identity(&ambient).map_err(|error| {
			MigrationError::PathTraversal(format!(
				"Migration root directory identity is unavailable: {error}"
			))
		})?;
		after_open()?;
		let after_open = std::fs::symlink_metadata(&self.root_dir).map_err(|error| {
			MigrationError::PathTraversal(format!(
				"Migration root directory identity is unavailable: {error}"
			))
		})?;
		if after_open.file_type().is_symlink() || &actual != expected {
			return Err(MigrationError::PathTraversal(format!(
				"Migration root directory identity changed during source creation: expected \
				 {expected:?}, found {actual:?}"
			)));
		}
		let after_open_identity = path_directory_identity(&self.root_dir).map_err(|error| {
			MigrationError::PathTraversal(format!(
				"Migration root directory identity is unavailable: {error}"
			))
		})?;
		if &after_open_identity != expected {
			return Err(MigrationError::PathTraversal(format!(
				"Migration root directory identity changed during source creation: expected \
				 {expected:?}, found {after_open_identity:?}"
			)));
		}
		Ok(())
	}

	/// Format code with rustfmt, applying project's rustfmt.toml settings (hard_tabs = true)
	///
	/// Falls back to prettyplease output if rustfmt is not available or fails.
	fn format_with_rustfmt(code: &str) -> Result<String> {
		use std::process::{Command, Stdio};

		// Try to run rustfmt
		let child = Command::new("rustfmt")
			.arg("--edition=2024")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn();

		match child {
			Ok(mut child_process) => {
				// Write code to stdin
				if let Some(stdin) = child_process.stdin.as_mut() {
					stdin.write_all(code.as_bytes()).map_err(|e| {
						MigrationError::IoError(std::io::Error::other(format!(
							"Failed to write to rustfmt stdin: {}",
							e
						)))
					})?;
				}

				// Get formatted output
				let output = child_process.wait_with_output().map_err(|e| {
					MigrationError::IoError(std::io::Error::other(format!(
						"Failed to read rustfmt output: {}",
						e
					)))
				})?;

				if output.status.success() {
					String::from_utf8(output.stdout).map_err(|e| {
						MigrationError::IoError(std::io::Error::other(format!(
							"Invalid UTF-8 from rustfmt: {}",
							e
						)))
					})
				} else {
					// rustfmt failed, fallback to prettyplease output
					eprintln!("Warning: rustfmt failed, using prettyplease output");
					Ok(code.to_string())
				}
			}
			Err(_) => {
				// rustfmt not available, use prettyplease output
				eprintln!("Warning: rustfmt not found, using prettyplease output (space-indented)");
				Ok(code.to_string())
			}
		}
	}

	/// Check if two migrations have identical operations
	///
	/// Returns true if the operations vectors are equal.
	fn has_identical_operations(&self, m1: &Migration, m2: &Migration) -> bool {
		m1.operations == m2.operations
	}
}

#[async_trait]
impl MigrationRepository for FilesystemRepository {
	async fn save(&mut self, migration: &Migration) -> Result<()> {
		let path = self.migration_path(&migration.app_label, &migration.name)?;

		// Check if migration file already exists to prevent overwriting
		if tokio::fs::try_exists(&path).await.unwrap_or(false) {
			return Err(MigrationError::IoError(std::io::Error::other(format!(
				"Migration file already exists: {}. \
				If you want to replace it, please delete the existing file first.",
				path.display()
			))));
		}

		// Check for duplicate operations with existing migrations.
		// Skip for migrations with empty operations (e.g., merge migrations)
		// since empty-vs-empty comparison is never a meaningful duplicate signal.
		// The file-exists check above already prevents actual overwrites.
		if !migration.operations.is_empty() {
			let existing_migrations = self.list(&migration.app_label).await?;
			for existing in &existing_migrations {
				if self.has_identical_operations(existing, migration) {
					return Err(MigrationError::DuplicateOperations(format!(
						"Migration '{}' has identical operations to existing migration '{}'. \
						This usually indicates a problem with from_state construction. \
						The existing migration was created at the same location and performs \
						the same database changes.",
						migration.name, existing.name
					)));
				}
			}
		}

		// Create parent directories
		if let Some(parent) = path.parent() {
			tokio::fs::create_dir_all(parent).await.map_err(|e| {
				MigrationError::IoError(std::io::Error::other(format!(
					"Failed to create directory {}: {}",
					parent.display(),
					e
				)))
			})?;
		}

		let code = self.render(
			migration,
			MigrationRenderOptions {
				include_header: true,
			},
		)?;
		self.create_new_source(&migration.app_label, &migration.name, &code)?;

		Ok(())
	}

	async fn get(&self, app_label: &str, name: &str) -> Result<Migration> {
		let path = self.migration_path(app_label, name)?;

		if !path.exists() {
			return Err(MigrationError::NotFound(format!("{}.{}", app_label, name)));
		}

		// Read and parse file
		let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to read {}: {}",
				path.display(),
				e
			)))
		})?;

		// Parse with syn
		let ast: syn::File = syn::parse_file(&content).map_err(|e| {
			MigrationError::InvalidMigration(format!("Failed to parse {}: {}", path.display(), e))
		})?;

		// Extract migration data from AST using ast_parser utility
		ast_parser::extract_migration_metadata(&ast, app_label, name)
	}

	async fn list(&self, app_label: &str) -> Result<Vec<Migration>> {
		Self::validate_path_component(app_label, "App label")?;
		let migrations_dir = self.root_dir.join(app_label);

		if !migrations_dir.exists() {
			return Ok(vec![]);
		}

		let mut migrations = Vec::new();

		// Read directory
		let mut entries = tokio::fs::read_dir(&migrations_dir).await.map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to read directory {}: {}",
				migrations_dir.display(),
				e
			)))
		})?;

		while let Some(entry) = entries.next_entry().await.map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to read directory entry: {}",
				e
			)))
		})? {
			let path = entry.path();

			// Skip non-.rs files
			if path.extension().and_then(|s| s.to_str()) != Some("rs") {
				continue;
			}

			// Extract name from filename
			if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
				// Get migration
				match self.get(app_label, name).await {
					Ok(migration) => migrations.push(migration),
					Err(e) => {
						eprintln!("Warning: Failed to load migration {}: {}", name, e);
					}
				}
			}
		}

		Ok(migrations)
	}

	async fn delete(&mut self, app_label: &str, name: &str) -> Result<()> {
		let path = self.migration_path(app_label, name)?;

		if !path.exists() {
			return Err(MigrationError::NotFound(format!("{}.{}", app_label, name)));
		}

		tokio::fs::remove_file(&path).await.map_err(|e| {
			MigrationError::IoError(std::io::Error::other(format!(
				"Failed to delete {}: {}",
				path.display(),
				e
			)))
		})?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::fields::FieldType;
	use crate::migrations::operations::{ColumnDefinition, Operation};
	use rstest::rstest;
	use serial_test::serial;
	use tempfile::TempDir;

	/// Creates a test migration with a unique CreateTable operation based on the migration name.
	/// This ensures each migration has distinct operations to avoid duplicate detection errors.
	fn create_test_migration(app_label: &str, name: &str) -> Migration {
		let mut migration = Migration::new(name, app_label);

		// Create a unique table name derived from the migration name
		let table_name = format!("table_{}", name.replace('-', "_"));
		migration.operations.push(Operation::CreateTable {
			name: table_name,
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		});

		migration
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_new() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();

		// Act
		let repo = FilesystemRepository::new(temp_dir.path());

		// Assert
		assert_eq!(repo.root_dir, temp_dir.path());
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("polls", "0001_initial");

		// Act
		repo.save(&migration).await.unwrap();

		// Assert
		let path = repo.migration_path("polls", "0001_initial").unwrap();
		assert!(tokio::fs::try_exists(&path).await.unwrap());

		let content = tokio::fs::read_to_string(&path).await.unwrap();
		assert!(content.contains("pub(super) fn migration() -> Migration"));
		assert!(content.contains("app_label: \"polls\""));
		assert!(content.contains("name: \"0001_initial\""));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_get() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("polls", "0001_initial");
		repo.save(&migration).await.unwrap();

		// Act
		let retrieved = repo.get("polls", "0001_initial").await.unwrap();

		// Assert
		assert_eq!(retrieved.app_label, "polls");
		assert_eq!(retrieved.name, "0001_initial");
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_get_not_found() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.get("polls", "0001_initial").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), MigrationError::NotFound(_)));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_list() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		repo.save(&create_test_migration("polls", "0001_initial"))
			.await
			.unwrap();
		repo.save(&create_test_migration("polls", "0002_add_field"))
			.await
			.unwrap();

		// Act
		let migrations = repo.list("polls").await.unwrap();

		// Assert
		assert_eq!(migrations.len(), 2);
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_list_empty() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let migrations = repo.list("polls").await.unwrap();

		// Assert
		assert_eq!(migrations.len(), 0);
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_delete() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("polls", "0001_initial");
		repo.save(&migration).await.unwrap();
		let path = repo.migration_path("polls", "0001_initial").unwrap();
		assert!(tokio::fs::try_exists(&path).await.unwrap());

		// Act
		repo.delete("polls", "0001_initial").await.unwrap();

		// Assert
		assert!(!tokio::fs::try_exists(&path).await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_delete_not_found() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.delete("polls", "0001_initial").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), MigrationError::NotFound(_)));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save_with_dependencies() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration =
			Migration::new("0002_add_field", "polls").add_dependency("polls", "0001_initial");

		// Act
		repo.save(&migration).await.unwrap();

		// Assert
		let path = repo.migration_path("polls", "0002_add_field").unwrap();
		let content = tokio::fs::read_to_string(&path).await.unwrap();
		assert!(content.contains("dependencies"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save_prevents_overwrite() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("polls", "0001_initial");
		repo.save(&migration).await.unwrap();
		let path = repo.migration_path("polls", "0001_initial").unwrap();
		assert!(tokio::fs::try_exists(&path).await.unwrap());

		// Act
		let duplicate_migration = create_test_migration("polls", "0001_initial");
		let result = repo.save(&duplicate_migration).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, MigrationError::IoError(_)));
		assert!(err.to_string().contains("already exists"));
	}

	#[rstest]
	#[case("../etc", "0001_initial", "App label")]
	#[case("polls", "../secret", "Migration name")]
	#[case("../../root", "0001_initial", "App label")]
	#[case("polls", "../../etc/passwd", "Migration name")]
	fn test_path_traversal_rejected(
		#[case] app_label: &str,
		#[case] name: &str,
		#[case] expected_label: &str,
	) {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path(app_label, name);

		// Assert
		assert!(result.is_err(), "Path traversal should be rejected");
		let err = result.unwrap_err();
		assert!(matches!(err, MigrationError::PathTraversal(_)));
		assert!(
			err.to_string().contains(expected_label),
			"Error should mention '{}', got: {}",
			expected_label,
			err
		);
	}

	#[rstest]
	#[case("polls/subdir", "0001_initial")]
	#[case("polls\\subdir", "0001_initial")]
	#[case("polls", "name/with/slashes")]
	fn test_path_separator_rejected(#[case] app_label: &str, #[case] name: &str) {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path(app_label, name);

		// Assert
		assert!(result.is_err(), "Path separators should be rejected");
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	#[case("polls\0evil", "0001_initial")]
	#[case("polls", "0001\0evil")]
	fn test_null_byte_rejected(#[case] app_label: &str, #[case] name: &str) {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path(app_label, name);

		// Assert
		assert!(result.is_err(), "Null bytes should be rejected");
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	#[case("", "0001_initial")]
	#[case("polls", "")]
	fn test_empty_component_rejected(#[case] app_label: &str, #[case] name: &str) {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path(app_label, name);

		// Assert
		assert!(result.is_err(), "Empty components should be rejected");
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	fn test_valid_path_accepted() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.migration_path("polls", "0001_initial");

		// Assert
		assert!(result.is_ok(), "Valid path should be accepted");
		let path = result.unwrap();
		assert!(path.starts_with(temp_dir.path()));
		assert!(path.ends_with("0001_initial.rs"));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_save_rejects_traversal_in_app_label() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let migration = create_test_migration("../etc", "0001_initial");

		// Act
		let result = repo.save(&migration).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_list_rejects_traversal_in_app_label() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());

		// Act
		let result = repo.list("../etc").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			MigrationError::PathTraversal(_)
		));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save_with_initial_true() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let mut migration = create_test_migration("polls", "0001_initial_true");
		migration.initial = Some(true);

		// Act
		repo.save(&migration).await.unwrap();

		// Assert - verify generated code contains initial: Some(true)
		let path = repo.migration_path("polls", "0001_initial_true").unwrap();
		let content = tokio::fs::read_to_string(&path).await.unwrap();
		assert!(
			content.contains("Some(true)"),
			"Generated code should contain Some(true), got: {}",
			content
		);

		// Assert - round-trip: get() parses back the same initial value
		let retrieved = repo.get("polls", "0001_initial_true").await.unwrap();
		assert_eq!(retrieved.initial, Some(true));
	}

	#[rstest]
	#[tokio::test]
	#[serial(filesystem_repository)]
	async fn test_filesystem_repository_save_with_initial_false() {
		// Arrange
		let temp_dir = TempDir::new().unwrap();
		let mut repo = FilesystemRepository::new(temp_dir.path());
		let mut migration = create_test_migration("polls", "0001_initial_false");
		migration.initial = Some(false);

		// Act
		repo.save(&migration).await.unwrap();

		// Assert - verify generated code contains initial: Some(false)
		let path = repo.migration_path("polls", "0001_initial_false").unwrap();
		let content = tokio::fs::read_to_string(&path).await.unwrap();
		assert!(
			content.contains("Some(false)"),
			"Generated code should contain Some(false), got: {}",
			content
		);

		// Assert - round-trip: get() parses back the same initial value
		let retrieved = repo.get("polls", "0001_initial_false").await.unwrap();
		assert_eq!(retrieved.initial, Some(false));
	}

	#[test]
	fn create_new_source_removes_partial_file_when_injected_write_fails() {
		use std::io::Write;

		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());
		let source = repo
			.render(
				&create_test_migration("polls", "0001_squashed"),
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap();

		let result = repo.create_new_source_with_hooks(
			"polls",
			"0001_squashed",
			&source,
			RootValidationHooks {
				before_open: || Ok(()),
				after_identity_open: || Ok(()),
			},
			|file, formatted| {
				file.write_all(&formatted.as_bytes()[..16])?;
				Err(std::io::Error::other("injected write failure"))
			},
			|directory, name| directory.remove_file(name),
		);

		assert!(matches!(result, Err(MigrationError::IoError(_))));
		assert!(!temp_dir.path().join("polls/0001_squashed.rs").exists());
	}

	#[cfg(unix)]
	#[test]
	fn create_new_source_resists_symlink_swap_after_validation() {
		use std::os::unix::fs::symlink;

		let root = TempDir::new().unwrap();
		let outside = TempDir::new().unwrap();
		std::fs::create_dir(root.path().join("polls")).unwrap();
		let repo = FilesystemRepository::new(root.path());
		let source = repo
			.render(
				&create_test_migration("polls", "0001_squashed"),
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap();

		let result = repo.create_new_source_with_hooks(
			"polls",
			"0001_squashed",
			&source,
			RootValidationHooks {
				before_open: || {
					std::fs::remove_dir(root.path().join("polls"))?;
					symlink(outside.path(), root.path().join("polls"))
				},
				after_identity_open: || Ok(()),
			},
			|file, formatted| {
				file.write_all(formatted.as_bytes())?;
				file.sync_all()
			},
			|directory, name| directory.remove_file(name),
		);

		assert!(result.is_err());
		assert!(!outside.path().join("0001_squashed.rs").exists());
	}

	#[cfg(unix)]
	#[test]
	fn create_new_source_rejects_preexisting_root_symlink_before_file_creation() {
		use std::cell::Cell;
		use std::os::unix::fs::symlink;

		let container = TempDir::new().unwrap();
		let root = container.path().join("migrations");
		let outside = TempDir::new().unwrap();
		symlink(outside.path(), &root).unwrap();
		let repo = FilesystemRepository::new(&root);
		let source = repo
			.render(
				&create_test_migration("polls", "0001_squashed"),
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap();
		let cleanup_called = Cell::new(false);

		let error = repo
			.create_new_source_with_hooks(
				"polls",
				"0001_squashed",
				&source,
				RootValidationHooks {
					before_open: || Ok(()),
					after_identity_open: || Ok(()),
				},
				|file, formatted| {
					file.write_all(formatted.as_bytes())?;
					file.sync_all()
				},
				|_: &Dir, _: &str| {
					cleanup_called.set(true);
					Err(std::io::Error::other("injected cleanup failure"))
				},
			)
			.unwrap_err();

		assert!(matches!(error, MigrationError::PathTraversal(_)));
		assert!(!cleanup_called.get());
		assert!(!outside.path().join("polls/0001_squashed.rs").exists());
	}

	#[cfg(unix)]
	#[test]
	fn create_new_source_anchors_root_before_path_replacement() {
		use std::os::unix::fs::symlink;

		let container = TempDir::new().unwrap();
		let root = container.path().join("migrations");
		let anchored_root = container.path().join("anchored-migrations");
		let outside = TempDir::new().unwrap();
		std::fs::create_dir(&root).unwrap();
		let repo = FilesystemRepository::new(&root);
		let source = repo
			.render(
				&create_test_migration("polls", "0001_squashed"),
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap();

		let result = repo.create_new_source_with_hooks(
			"polls",
			"0001_squashed",
			&source,
			RootValidationHooks {
				before_open: || {
					std::fs::rename(&root, &anchored_root)?;
					symlink(outside.path(), &root)
				},
				after_identity_open: || Ok(()),
			},
			|file, formatted| {
				file.write_all(formatted.as_bytes())?;
				file.sync_all()
			},
			|directory, name| directory.remove_file(name),
		);

		let error = result.unwrap_err();
		assert!(matches!(error, MigrationError::PathTraversal(_)));
		assert!(!anchored_root.join("polls/0001_squashed.rs").exists());
		assert!(!outside.path().join("polls").exists());
	}

	#[cfg(unix)]
	#[test]
	fn create_new_source_rejects_root_swap_before_cleanup_is_needed() {
		use std::cell::Cell;
		use std::os::unix::fs::symlink;

		let container = TempDir::new().unwrap();
		let root = container.path().join("migrations");
		let anchored_root = container.path().join("anchored-migrations");
		let outside = TempDir::new().unwrap();
		std::fs::create_dir(&root).unwrap();
		let repo = FilesystemRepository::new(&root);
		let source = repo
			.render(
				&create_test_migration("polls", "0001_squashed"),
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap();
		let cleanup_called = Cell::new(false);

		let error = repo
			.create_new_source_with_hooks(
				"polls",
				"0001_squashed",
				&source,
				RootValidationHooks {
					before_open: || {
						std::fs::rename(&root, &anchored_root)?;
						symlink(outside.path(), &root)
					},
					after_identity_open: || Ok(()),
				},
				|file, formatted| {
					file.write_all(formatted.as_bytes())?;
					file.sync_all()
				},
				|_: &Dir, _: &str| {
					cleanup_called.set(true);
					Err(std::io::Error::other("injected identity cleanup failure"))
				},
			)
			.unwrap_err();

		assert!(error.to_string().contains("root directory identity"));
		assert!(!cleanup_called.get());
		assert!(!anchored_root.join("polls/0001_squashed.rs").exists());
		assert!(!outside.path().join("polls").exists());
	}

	#[test]
	fn create_new_source_reports_write_and_cleanup_failures() {
		let temp_dir = TempDir::new().unwrap();
		let repo = FilesystemRepository::new(temp_dir.path());
		let source = repo
			.render(
				&create_test_migration("polls", "0001_squashed"),
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap();

		let error = repo
			.create_new_source_with_hooks(
				"polls",
				"0001_squashed",
				&source,
				RootValidationHooks {
					before_open: || Ok(()),
					after_identity_open: || Ok(()),
				},
				|file, _| {
					file.write_all(b"partial")?;
					Err(std::io::Error::other("injected write failure"))
				},
				|_: &Dir, _: &str| Err(std::io::Error::other("injected cleanup failure")),
			)
			.unwrap_err();

		assert!(error.to_string().contains("injected write failure"));
		assert!(error.to_string().contains("injected cleanup failure"));
	}

	#[cfg(unix)]
	#[test]
	fn create_new_source_rejects_root_directory_swap_after_identity_open() {
		let container = TempDir::new().unwrap();
		let root = container.path().join("migrations");
		let anchored_root = container.path().join("anchored-migrations");
		let replacement_root = container.path().join("replacement-migrations");
		std::fs::create_dir(&root).unwrap();
		std::fs::create_dir(&replacement_root).unwrap();
		std::fs::write(replacement_root.join("marker"), b"replacement root").unwrap();
		let repo = FilesystemRepository::new(&root);
		let source = repo
			.render(
				&create_test_migration("polls", "0001_squashed"),
				MigrationRenderOptions {
					include_header: false,
				},
			)
			.unwrap();

		let result = repo.create_new_source_with_hooks(
			"polls",
			"0001_squashed",
			&source,
			RootValidationHooks {
				before_open: || Ok(()),
				after_identity_open: || {
					std::fs::rename(&root, &anchored_root)?;
					std::fs::rename(&replacement_root, &root)
				},
			},
			|file, formatted| {
				file.write_all(formatted.as_bytes())?;
				file.sync_all()
			},
			|directory, name| directory.remove_file(name),
		);

		let anchored_file = anchored_root.join("polls/0001_squashed.rs");
		assert_eq!(
			std::fs::read(root.join("marker")).unwrap(),
			b"replacement root"
		);
		assert!(!root.join("polls/0001_squashed.rs").exists());
		assert!(
			matches!(result, Err(MigrationError::PathTraversal(_))) && !anchored_file.exists(),
			"expected root identity error and anchored cleanup, got {result:?}; anchored file \
			 exists: {}",
			anchored_file.exists()
		);
	}
}
