//! AST parser utilities for migration files
//!
//! Provides helper functions to extract migration metadata and operations
//! from parsed Rust ASTs.

use super::{Migration, MigrationError, Result};
use quote::ToTokens;
use reinhardt_query::value::Value as QueryValue;
use syn::{Expr, File, Item, ItemFn, Stmt};

/// Extract migration metadata from parsed AST
pub fn extract_migration_metadata(ast: &File, app_label: &str, name: &str) -> Result<Migration> {
	let dependencies = extract_dependencies(ast)?;
	let atomic = extract_atomic(ast).unwrap_or(true);
	let replaces = extract_replaces(ast).unwrap_or_default();
	let operations = extract_operations(ast).unwrap_or_default();
	let initial = extract_initial(ast);

	Ok(Migration {
		app_label: app_label.to_string(),
		name: name.to_string(),
		operations,
		dependencies,
		atomic,
		replaces,
		initial,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	})
}

/// Extract migration metadata without accepting missing or malformed core metadata.
///
/// `app_label` and `name` are authoritative. Filesystem callers derive them from
/// the migration path rather than trusting duplicate identity fields in source.
///
/// Swappable and optional dependencies are not currently representable by the
/// filesystem catalog format. They may be omitted or specified as empty vectors;
/// non-empty values are rejected to prevent semantic loss.
pub fn extract_migration_metadata_strict(
	ast: &File,
	app_label: &str,
	name: &str,
) -> Result<Migration> {
	let migration_expr = ast
		.items
		.iter()
		.find_map(|item| {
			let Item::Fn(function) = item else {
				return None;
			};
			(function.sig.ident == "migration")
				.then(|| function.block.stmts.last())
				.flatten()
				.and_then(|statement| {
					let Stmt::Expr(expression, _) = statement else {
						return None;
					};
					Some(expression)
				})
		})
		.ok_or_else(|| {
			MigrationError::InvalidMigration("Missing migration() entrypoint".to_string())
		})?;

	let Expr::Struct(migration_struct) = migration_expr else {
		return Err(MigrationError::InvalidMigration(
			"migration() must return a Migration struct literal".to_string(),
		));
	};
	if migration_struct
		.path
		.segments
		.last()
		.is_none_or(|segment| segment.ident != "Migration")
	{
		return Err(MigrationError::InvalidMigration(
			"migration() must return a Migration struct literal".to_string(),
		));
	}

	let dependencies_expr = extract_field_from_migration_struct(migration_expr, "dependencies")
		.ok_or_else(|| {
			MigrationError::InvalidMigration(
				"Migration metadata is missing required 'dependencies' field".to_string(),
			)
		})?;
	let operations_expr = extract_field_from_migration_struct(migration_expr, "operations")
		.ok_or_else(|| {
			MigrationError::InvalidMigration(
				"Migration metadata is missing required 'operations' field".to_string(),
			)
		})?;
	let dependencies = parse_tuple_vec_expr_strict(&dependencies_expr, "dependencies")?;
	let operations = parse_operations_vec_strict(&operations_expr)?;
	let replaces = extract_field_from_migration_struct(migration_expr, "replaces")
		.map(|expression| parse_tuple_vec_expr_strict(&expression, "replaces"))
		.transpose()?
		.unwrap_or_default();
	let atomic = parse_optional_bool_field(&migration_struct.fields, "atomic", true)?;
	let initial = parse_optional_initial_field(&migration_struct.fields)?;
	let state_only = parse_optional_bool_field(&migration_struct.fields, "state_only", false)?;
	let database_only =
		parse_optional_bool_field(&migration_struct.fields, "database_only", false)?;
	let swappable_dependencies =
		parse_swappable_dependencies(&migration_struct.fields, "swappable_dependencies")?;
	let optional_dependencies =
		parse_optional_dependencies(&migration_struct.fields, "optional_dependencies")?;

	Ok(Migration {
		app_label: app_label.to_string(),
		name: name.to_string(),
		operations,
		dependencies,
		atomic,
		replaces,
		initial,
		state_only,
		database_only,
		swappable_dependencies,
		optional_dependencies,
	})
}

fn dependency_metadata_expressions(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Result<Vec<Expr>> {
	let Some(field) = fields
		.iter()
		.find(|field| matches!(&field.member, syn::Member::Named(ident) if ident == field_name))
	else {
		return Ok(Vec::new());
	};
	parse_vec_expressions(&field.expr, field_name)
}

fn parse_swappable_dependencies(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Result<Vec<super::dependency::SwappableDependency>> {
	dependency_metadata_expressions(fields, field_name)?
		.iter()
		.map(|expression| {
			let Expr::Call(call) = expression else {
				return Err(malformed_dependency_metadata(field_name));
			};
			if !call_path_is(&call.func, "SwappableDependency", "new") || call.args.len() != 4 {
				return Err(malformed_dependency_metadata(field_name));
			}
			Ok(super::dependency::SwappableDependency::new(
				extract_string_expr(&call.args[0])
					.ok_or_else(|| malformed_dependency_metadata(field_name))?,
				extract_string_expr(&call.args[1])
					.ok_or_else(|| malformed_dependency_metadata(field_name))?,
				extract_string_expr(&call.args[2])
					.ok_or_else(|| malformed_dependency_metadata(field_name))?,
				extract_string_expr(&call.args[3])
					.ok_or_else(|| malformed_dependency_metadata(field_name))?,
			))
		})
		.collect()
}

fn parse_optional_dependencies(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Result<Vec<super::dependency::OptionalDependency>> {
	dependency_metadata_expressions(fields, field_name)?
		.iter()
		.map(|expression| {
			let Expr::Call(call) = expression else {
				return Err(malformed_dependency_metadata(field_name));
			};
			if !call_path_is(&call.func, "OptionalDependency", "new") || call.args.len() != 3 {
				return Err(malformed_dependency_metadata(field_name));
			}
			Ok(super::dependency::OptionalDependency::new(
				extract_string_expr(&call.args[0])
					.ok_or_else(|| malformed_dependency_metadata(field_name))?,
				extract_string_expr(&call.args[1])
					.ok_or_else(|| malformed_dependency_metadata(field_name))?,
				parse_dependency_condition(&call.args[2])
					.ok_or_else(|| malformed_dependency_metadata(field_name))?,
			))
		})
		.collect()
}

fn parse_dependency_condition(expr: &Expr) -> Option<super::dependency::DependencyCondition> {
	let Expr::Call(call) = expr else {
		return None;
	};
	let Expr::Path(path) = &*call.func else {
		return None;
	};
	if call.args.len() != 1 {
		return None;
	}
	let value = extract_string_expr(&call.args[0])?;
	match path.path.segments.last()?.ident.to_string().as_str() {
		"AppInstalled" => Some(super::dependency::DependencyCondition::AppInstalled(value)),
		"SettingEnabled" => Some(super::dependency::DependencyCondition::SettingEnabled(
			value,
		)),
		"FeatureEnabled" => Some(super::dependency::DependencyCondition::FeatureEnabled(
			value,
		)),
		_ => None,
	}
}

fn call_path_is(function: &Expr, type_name: &str, function_name: &str) -> bool {
	let Expr::Path(path) = function else {
		return false;
	};
	let mut segments = path.path.segments.iter().rev();
	segments
		.next()
		.is_some_and(|segment| segment.ident == function_name)
		&& segments
			.next()
			.is_some_and(|segment| segment.ident == type_name)
}

fn malformed_dependency_metadata(field_name: &str) -> MigrationError {
	MigrationError::InvalidMigration(format!("Malformed migration '{}' metadata", field_name))
}

fn parse_optional_bool_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
	default: bool,
) -> Result<bool> {
	let Some(field) = fields
		.iter()
		.find(|field| matches!(&field.member, syn::Member::Named(ident) if ident == field_name))
	else {
		return Ok(default);
	};
	let Expr::Lit(literal) = &field.expr else {
		return Err(MigrationError::InvalidMigration(format!(
			"Malformed migration '{}' metadata",
			field_name
		)));
	};
	let syn::Lit::Bool(value) = &literal.lit else {
		return Err(MigrationError::InvalidMigration(format!(
			"Malformed migration '{}' metadata",
			field_name
		)));
	};
	Ok(value.value)
}

fn parse_optional_initial_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
) -> Result<Option<bool>> {
	let Some(field) = fields
		.iter()
		.find(|field| matches!(&field.member, syn::Member::Named(ident) if ident == "initial"))
	else {
		return Ok(None);
	};
	if let Expr::Path(path) = &field.expr
		&& path.path.is_ident("None")
	{
		return Ok(None);
	}
	if let Expr::Call(call) = &field.expr
		&& let Expr::Path(path) = &*call.func
		&& path.path.is_ident("Some")
		&& call.args.len() == 1
		&& let Expr::Lit(literal) = &call.args[0]
		&& let syn::Lit::Bool(value) = &literal.lit
	{
		return Ok(Some(value.value));
	}
	Err(MigrationError::InvalidMigration(
		"Malformed migration 'initial' metadata".to_string(),
	))
}

/// Extract dependencies from `migration()` function
fn extract_dependencies(ast: &File) -> Result<Vec<(String, String)>> {
	// Find the migration() function
	for item in &ast.items {
		if let Item::Fn(func) = item
			&& func.sig.ident == "migration"
		{
			// Look for the Migration struct literal in the return value
			if let Some(Stmt::Expr(expr, _)) = func.block.stmts.last()
				&& let Some(dependencies) =
					extract_field_from_migration_struct(expr, "dependencies")
			{
				return parse_tuple_vec_expr(&dependencies);
			}
		}
	}
	Ok(vec![])
}

/// Extract atomic flag from `atomic()` function
fn extract_atomic(ast: &File) -> Option<bool> {
	for item in &ast.items {
		if let Item::Fn(func) = item
			&& func.sig.ident == "atomic"
		{
			return parse_bool_return(func);
		}
	}
	None
}

/// Extract replaces from `migration()` function
fn extract_replaces(ast: &File) -> Option<Vec<(String, String)>> {
	// Find the migration() function
	for item in &ast.items {
		if let Item::Fn(func) = item
			&& func.sig.ident == "migration"
		{
			// Look for the Migration struct literal in the return value
			if let Some(Stmt::Expr(expr, _)) = func.block.stmts.last()
				&& let Some(replaces) = extract_field_from_migration_struct(expr, "replaces")
			{
				return parse_tuple_vec_expr(&replaces).ok();
			}
		}
	}
	None
}

/// Extract initial flag from `migration()` function
fn extract_initial(ast: &File) -> Option<bool> {
	for item in &ast.items {
		if let Item::Fn(func) = item
			&& func.sig.ident == "migration"
			&& let Some(Stmt::Expr(expr, _)) = func.block.stmts.last()
			&& let Some(initial_expr) = extract_field_from_migration_struct(expr, "initial")
		{
			return parse_option_bool_expr(&initial_expr);
		}
	}
	None
}

/// Parse an `Option<bool>` expression (`Some(true)`, `Some(false)`, or `None`)
fn parse_option_bool_expr(expr: &Expr) -> Option<bool> {
	match expr {
		Expr::Call(call) => {
			// Some(true) or Some(false)
			if let Expr::Path(path) = &*call.func
				&& path.path.is_ident("Some")
				&& call.args.len() == 1
				&& let Expr::Lit(lit) = &call.args[0]
				&& let syn::Lit::Bool(b) = &lit.lit
			{
				return Some(b.value);
			}
			None
		}
		Expr::Path(path) if path.path.is_ident("None") => None,
		_ => None,
	}
}

/// Extract operations from `migration()` function
fn extract_operations(ast: &File) -> Result<Vec<super::Operation>> {
	let mut operations = Vec::new();

	// Find the migration() function
	for item in &ast.items {
		if let Item::Fn(func) = item
			&& func.sig.ident == "migration"
		{
			// Look for the Migration struct literal in the return value
			if let Some(Stmt::Expr(expr, _)) = func.block.stmts.last()
				&& let Some(ops_expr) = extract_field_from_migration_struct(expr, "operations")
			{
				operations = parse_operations_vec(&ops_expr);
			}
		}
	}

	Ok(operations)
}

/// Parse operations from vec![...] expression
fn parse_operations_vec(expr: &Expr) -> Vec<super::Operation> {
	let mut operations = Vec::new();

	match expr {
		// Handle vec![...] macro
		Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			// The tokens contain the operation expressions separated by commas
			// We need to parse them as expressions
			if let Ok(parsed) = syn::parse2::<syn::ExprArray>(quote::quote! { [#tokens] }) {
				for elem in &parsed.elems {
					if let Some(op) = parse_single_operation(elem) {
						operations.push(op);
					}
				}
			}
		}
		// Handle array literal [...]
		Expr::Array(expr_array) => {
			for elem in &expr_array.elems {
				if let Some(op) = parse_single_operation(elem) {
					operations.push(op);
				}
			}
		}
		_ => {}
	}

	operations
}

fn parse_operations_vec_strict(expr: &Expr) -> Result<Vec<super::Operation>> {
	let expressions = parse_vec_expressions(expr, "operations")?;
	expressions
		.iter()
		.map(|expression| {
			parse_single_operation(expression).ok_or_else(|| {
				let operation_name = match expression {
					Expr::Struct(operation) => operation
						.path
						.segments
						.last()
						.map(|segment| segment.ident.to_string())
						.unwrap_or_else(|| "<expression>".to_string()),
					_ => "<expression>".to_string(),
				};
				MigrationError::InvalidMigration(format!(
					"Unsupported or malformed migration operation '{}'",
					operation_name
				))
			})
		})
		.collect()
}

fn parse_vec_expressions(expr: &Expr, field_name: &str) -> Result<Vec<Expr>> {
	match expr {
		Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			let parsed =
				syn::parse2::<syn::ExprArray>(quote::quote! { [#tokens] }).map_err(|_| {
					MigrationError::InvalidMigration(format!(
						"Malformed migration '{}' metadata",
						field_name
					))
				})?;
			Ok(parsed.elems.into_iter().collect())
		}
		Expr::Array(array) => Ok(array.elems.iter().cloned().collect()),
		_ => Err(MigrationError::InvalidMigration(format!(
			"Malformed migration '{}' metadata",
			field_name
		))),
	}
}

/// Parse a single Operation from an expression
fn parse_single_operation(expr: &Expr) -> Option<super::Operation> {
	// Handle Operation::CreateTable { ... }
	if let Expr::Struct(expr_struct) = expr {
		// Extract the variant name
		let variant_name = expr_struct.path.segments.last()?.ident.to_string();

		match variant_name.as_str() {
			"CreateExtension" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let if_not_exists =
					extract_bool_field(&expr_struct.fields, "if_not_exists").unwrap_or(true);
				let schema = extract_optional_str_field(&expr_struct.fields, "schema");
				return Some(super::Operation::CreateExtension {
					name,
					if_not_exists,
					schema,
				});
			}
			"CreateTable" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let columns = extract_columns_field(&expr_struct.fields)?;
				let constraints = extract_constraints_field(&expr_struct.fields);

				return Some(super::Operation::CreateTable {
					name,
					columns,
					constraints,
					without_rowid: None,
					interleave_in_parent: None,
					partition: None,
				});
			}
			"DropTable" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				return Some(super::Operation::DropTable { name });
			}
			"AddColumn" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let column = extract_column_definition_field(&expr_struct.fields, "column")?;
				return Some(super::Operation::AddColumn {
					table,
					column,
					mysql_options: None,
				});
			}
			"DropColumn" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let column = extract_string_field(&expr_struct.fields, "column")?;
				let old_definition =
					extract_optional_column_definition_field(&expr_struct.fields, "old_definition");
				return Some(super::Operation::DropColumn {
					table,
					column,
					old_definition,
				});
			}
			"AlterColumn" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let column = extract_string_field(&expr_struct.fields, "column")?;
				let new_definition =
					extract_column_definition_field(&expr_struct.fields, "new_definition")?;
				return Some(super::Operation::AlterColumn {
					table,
					column,
					new_definition,
					old_definition: None,
					mysql_options: None,
				});
			}
			"RenameTable" => {
				let old_name = extract_string_field(&expr_struct.fields, "old_name")?;
				let new_name = extract_string_field(&expr_struct.fields, "new_name")?;
				return Some(super::Operation::RenameTable { old_name, new_name });
			}
			"RenameColumn" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let old_name = extract_string_field(&expr_struct.fields, "old_name")?;
				let new_name = extract_string_field(&expr_struct.fields, "new_name")?;
				return Some(super::Operation::RenameColumn {
					table,
					old_name,
					new_name,
				});
			}
			"CreateIndex" | "CreateNamedIndex" | "CreateIndexRepair" | "RestoreIndexOnRollback" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let name = if variant_name == "CreateNamedIndex" {
					Some(extract_string_field(&expr_struct.fields, "name")?)
				} else {
					extract_optional_str_field(&expr_struct.fields, "name")
				};
				let columns = extract_string_vec_field(&expr_struct.fields, "columns");
				let unique = extract_bool_field(&expr_struct.fields, "unique").unwrap_or(false);
				let index_type = extract_index_type_field(&expr_struct.fields, "index_type");
				let where_clause = extract_optional_str_field(&expr_struct.fields, "where_clause");
				let concurrently =
					extract_bool_field(&expr_struct.fields, "concurrently").unwrap_or(false);
				let expressions =
					extract_optional_string_vec_field(&expr_struct.fields, "expressions");
				let mysql_options =
					extract_alter_table_options_field(&expr_struct.fields, "mysql_options");
				let operator_class =
					extract_optional_str_field(&expr_struct.fields, "operator_class");

				return Some(match variant_name.as_str() {
					"CreateIndex" => super::Operation::CreateIndex {
						table,
						columns,
						unique,
						index_type,
						where_clause,
						concurrently,
						expressions,
						mysql_options,
						operator_class,
					},
					#[cfg(feature = "pgvector")]
					"CreateNamedIndex" => super::Operation::CreateNamedIndex {
						table,
						name: name?,
						columns,
						unique,
						index_type,
						where_clause,
						concurrently,
						expressions,
						mysql_options,
						operator_class,
					},
					#[cfg(not(feature = "pgvector"))]
					"CreateNamedIndex" => super::Operation::CreateIndex {
						table,
						columns,
						unique,
						index_type,
						where_clause,
						concurrently,
						expressions,
						mysql_options,
						operator_class,
					},
					"CreateIndexRepair" => super::Operation::CreateIndexRepair {
						table,
						name,
						columns,
						unique,
						index_type,
						where_clause,
						concurrently,
						expressions,
						mysql_options,
						operator_class,
					},
					_ => super::Operation::RestoreIndexOnRollback {
						table,
						name,
						columns,
						unique,
						index_type,
						where_clause,
						concurrently,
						expressions,
						mysql_options,
						operator_class,
					},
				});
			}
			"DropIndex" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let columns = extract_string_vec_field(&expr_struct.fields, "columns");
				return Some(super::Operation::DropIndex { table, columns });
			}
			"DropNamedIndex" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let columns = extract_string_vec_field(&expr_struct.fields, "columns");
				let unique = extract_bool_field(&expr_struct.fields, "unique").unwrap_or(false);
				let index_type = extract_index_type_field(&expr_struct.fields, "index_type");
				let where_clause = extract_optional_str_field(&expr_struct.fields, "where_clause");
				let concurrently =
					extract_bool_field(&expr_struct.fields, "concurrently").unwrap_or(false);
				let expressions =
					extract_optional_string_vec_field(&expr_struct.fields, "expressions");
				let mysql_options =
					extract_alter_table_options_field(&expr_struct.fields, "mysql_options");
				let operator_class =
					extract_optional_str_field(&expr_struct.fields, "operator_class");
				#[cfg(feature = "pgvector")]
				return Some(super::Operation::DropNamedIndex {
					table,
					name,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				});
				#[cfg(not(feature = "pgvector"))]
				{
					let _ = (
						name,
						unique,
						index_type,
						where_clause,
						concurrently,
						expressions,
						mysql_options,
						operator_class,
					);
					return Some(super::Operation::DropIndex { table, columns });
				}
			}
			"AddConstraint" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let constraint_sql = extract_string_field(&expr_struct.fields, "constraint_sql")?;
				return Some(super::Operation::AddConstraint {
					table,
					constraint_sql,
				});
			}
			"AddConstraintDefinition" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let constraint = expr_struct.fields.iter().find_map(|field| {
					matches!(&field.member, syn::Member::Named(ident) if ident == "constraint")
						.then(|| parse_single_constraint(&field.expr))
						.flatten()
				})?;
				return Some(super::Operation::AddConstraintDefinition { table, constraint });
			}
			"AddConstraintRepair" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let constraint_sql = extract_string_field(&expr_struct.fields, "constraint_sql")?;
				return Some(super::Operation::AddConstraintRepair {
					table,
					constraint_sql,
				});
			}
			"RestoreConstraintOnRollback" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let constraint_sql = extract_string_field(&expr_struct.fields, "constraint_sql")?;
				return Some(super::Operation::RestoreConstraintOnRollback {
					table,
					constraint_sql,
				});
			}
			"DropConstraint" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let constraint_name = extract_string_field(&expr_struct.fields, "constraint_name")?;
				return Some(super::Operation::DropConstraint {
					table,
					constraint_name,
				});
			}
			"DropConstraintDefinition" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let constraint = expr_struct
					.fields
					.iter()
					.find(|field| field.member.to_token_stream().to_string() == "constraint")
					.and_then(|field| parse_single_constraint(&field.expr))?;
				return Some(super::Operation::DropConstraintDefinition { table, constraint });
			}
			"RunSQL" => {
				// Use extract_string_field to handle both literal and .to_string() patterns (#1336)
				let sql = extract_string_field(&expr_struct.fields, "sql")?;
				let reverse_sql = extract_optional_str_field(&expr_struct.fields, "reverse_sql");
				return Some(super::Operation::RunSQL { sql, reverse_sql });
			}
			_ => {
				// Log unhandled operation types
				eprintln!(
					"Warning: Unhandled operation type in AST parser: {}",
					variant_name
				);
			}
		}
	}

	None
}

/// Extract a boolean field from struct fields
fn extract_bool_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<bool> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
			&& let Expr::Lit(expr_lit) = &field.expr
			&& let syn::Lit::Bool(lit_bool) = &expr_lit.lit
		{
			return Some(lit_bool.value);
		}
	}
	None
}

/// Extract an optional string field (`Option<&'static str>`)
fn extract_optional_str_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<String> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			// Check for None
			if let Expr::Path(expr_path) = &field.expr
				&& expr_path.path.is_ident("None")
			{
				return None;
			}
			// Check for Some(...)
			if let Expr::Call(expr_call) = &field.expr
				&& let Expr::Path(func_path) = &*expr_call.func
				&& func_path.path.is_ident("Some")
				&& !expr_call.args.is_empty()
			{
				// Handle Some("str".to_string()) pattern
				if let Expr::MethodCall(method_call) = &expr_call.args[0]
					&& method_call.method == "to_string"
				{
					return extract_string_literal(&method_call.receiver);
				}
				return extract_string_literal(&expr_call.args[0]);
			}
		}
	}
	None
}

fn extract_optional_expr_tokens_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<String> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			if let Expr::Path(expr_path) = &field.expr
				&& expr_path.path.is_ident("None")
			{
				return None;
			}
			if let Expr::Call(expr_call) = &field.expr
				&& let Expr::Path(func_path) = &*expr_call.func
				&& func_path.path.is_ident("Some")
				&& expr_call.args.len() == 1
			{
				let expr = unwrap_box_new_expr(&expr_call.args[0]).unwrap_or(&expr_call.args[0]);
				return Some(expr.to_token_stream().to_string());
			}
		}
	}
	None
}

/// Extract a `Vec<&'static str>` field
fn extract_string_vec_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Vec<String> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			return extract_string_vec(&field.expr);
		}
	}
	Vec::new()
}

fn extract_optional_string_vec_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<Vec<String>> {
	fields.iter().find_map(|field| {
		let syn::Member::Named(ident) = &field.member else {
			return None;
		};
		if ident != field_name {
			return None;
		}
		let Expr::Call(call) = &field.expr else {
			return None;
		};
		let Expr::Path(path) = &*call.func else {
			return None;
		};
		path.path
			.is_ident("Some")
			.then(|| call.args.first().map(extract_string_vec))
			.flatten()
	})
}

fn extract_alter_table_options_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<super::AlterTableOptions> {
	use super::{AlterTableOptions, MySqlAlgorithm, MySqlLock};

	let options = fields.iter().find_map(|field| {
		let syn::Member::Named(ident) = &field.member else {
			return None;
		};
		if ident != field_name {
			return None;
		}
		let Expr::Call(call) = &field.expr else {
			return None;
		};
		let Expr::Path(path) = &*call.func else {
			return None;
		};
		if !path.path.is_ident("Some") {
			return None;
		}
		let Expr::Struct(options) = call.args.first()? else {
			return None;
		};
		Some(options)
	})?;

	let algorithm =
		extract_optional_path_variant_field(&options.fields, "algorithm").and_then(|variant| {
			match variant.as_str() {
				"Instant" => Some(MySqlAlgorithm::Instant),
				"Inplace" => Some(MySqlAlgorithm::Inplace),
				"Copy" => Some(MySqlAlgorithm::Copy),
				"Default" => Some(MySqlAlgorithm::Default),
				_ => None,
			}
		});
	let lock = extract_optional_path_variant_field(&options.fields, "lock").and_then(|variant| {
		match variant.as_str() {
			"None" => Some(MySqlLock::None),
			"Shared" => Some(MySqlLock::Shared),
			"Exclusive" => Some(MySqlLock::Exclusive),
			"Default" => Some(MySqlLock::Default),
			_ => None,
		}
	});
	Some(AlterTableOptions { algorithm, lock })
}

fn extract_optional_path_variant_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<String> {
	fields.iter().find_map(|field| {
		let syn::Member::Named(ident) = &field.member else {
			return None;
		};
		if ident != field_name {
			return None;
		}
		let Expr::Call(call) = &field.expr else {
			return None;
		};
		let Expr::Path(some) = &*call.func else {
			return None;
		};
		if !some.path.is_ident("Some") {
			return None;
		}
		let Expr::Path(variant) = call.args.first()? else {
			return None;
		};
		Some(variant.path.segments.last()?.ident.to_string())
	})
}

/// Extract `Vec<String>` from expression
fn extract_string_vec(expr: &Expr) -> Vec<String> {
	let mut result = Vec::new();

	match expr {
		Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			if let Ok(parsed) = syn::parse2::<syn::ExprArray>(quote::quote! { [#tokens] }) {
				for elem in &parsed.elems {
					if let Some(s) = extract_string_literal(elem) {
						result.push(s);
					}
				}
			}
		}
		Expr::Array(expr_array) => {
			for elem in &expr_array.elems {
				if let Some(s) = extract_string_literal(elem) {
					result.push(s);
				}
			}
		}
		_ => {}
	}

	result
}

/// Extract columns (`Vec<ColumnDefinition>`) field from struct
fn extract_columns_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
) -> Option<Vec<super::ColumnDefinition>> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == "columns"
		{
			return Some(parse_columns_vec(&field.expr));
		}
	}
	None
}

/// Extract a string field that may use .to_string() pattern
fn extract_string_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<String> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			// Handle "string".to_string() pattern
			if let Expr::MethodCall(method_call) = &field.expr
				&& method_call.method == "to_string"
			{
				return extract_string_literal(&method_call.receiver);
			}
			// Handle direct string literal
			return extract_string_literal(&field.expr);
		}
	}
	None
}

/// Extract ForeignKeyAction enum from struct field
fn extract_foreign_key_action_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<super::ForeignKeyAction> {
	use super::ForeignKeyAction;

	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
			&& let Expr::Path(expr_path) = &field.expr
			&& let Some(last_segment) = expr_path.path.segments.last()
		{
			let variant = last_segment.ident.to_string();
			return match variant.as_str() {
				"Restrict" => Some(ForeignKeyAction::Restrict),
				"Cascade" => Some(ForeignKeyAction::Cascade),
				"SetNull" => Some(ForeignKeyAction::SetNull),
				"NoAction" => Some(ForeignKeyAction::NoAction),
				"SetDefault" => Some(ForeignKeyAction::SetDefault),
				_ => None,
			};
		}
	}
	None
}

/// Extract IndexType enum from struct field (for CreateIndex operation)
fn extract_index_type_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<super::IndexType> {
	use super::IndexType;

	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			// Check for None
			if let Expr::Path(expr_path) = &field.expr
				&& expr_path.path.is_ident("None")
			{
				return None;
			}

			// Check for Some(IndexType::Variant)
			if let Expr::Call(expr_call) = &field.expr
				&& let Expr::Path(func_path) = &*expr_call.func
				&& func_path.path.is_ident("Some")
				&& !expr_call.args.is_empty()
			{
				match &expr_call.args[0] {
					Expr::Path(variant_path) => {
						let variant = variant_path.path.segments.last()?.ident.to_string();
						return match variant.as_str() {
							"BTree" => Some(IndexType::BTree),
							"Hash" => Some(IndexType::Hash),
							"Gin" => Some(IndexType::Gin),
							"Gist" => Some(IndexType::Gist),
							"Brin" => Some(IndexType::Brin),
							"Fulltext" => Some(IndexType::Fulltext),
							"Spatial" => Some(IndexType::Spatial),
							_ => None,
						};
					}
					Expr::Struct(variant) => {
						let variant_name = variant.path.segments.last()?.ident.to_string();
						return match variant_name.as_str() {
							#[cfg(feature = "pgvector")]
							"Hnsw" => Some(IndexType::Hnsw {
								m: extract_optional_integer_field(&variant.fields, "m")
									.and_then(|value| u16::try_from(value).ok()),
								ef_construction: extract_optional_integer_field(
									&variant.fields,
									"ef_construction",
								)
								.and_then(|value| u16::try_from(value).ok()),
							}),
							#[cfg(feature = "pgvector")]
							"Ivfflat" => Some(IndexType::Ivfflat {
								lists: extract_optional_integer_field(&variant.fields, "lists")
									.and_then(|value| u32::try_from(value).ok()),
							}),
							_ => None,
						};
					}
					_ => {}
				}
			}
		}
	}
	None
}

#[cfg(feature = "pgvector")]
fn extract_optional_integer_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<u64> {
	fields.iter().find_map(|field| {
		let syn::Member::Named(ident) = &field.member else {
			return None;
		};
		if ident != field_name {
			return None;
		}
		let Expr::Call(call) = &field.expr else {
			return None;
		};
		let Expr::Path(path) = &*call.func else {
			return None;
		};
		if !path.path.is_ident("Some") {
			return None;
		}
		let Expr::Lit(value) = call.args.first()? else {
			return None;
		};
		let syn::Lit::Int(value) = &value.lit else {
			return None;
		};
		value.base10_parse().ok()
	})
}

/// Extract `Vec<String>` from vec!["str".to_string(), ...] pattern
fn extract_string_vec_from_to_string(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Vec<String> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			return parse_string_vec_with_to_string(&field.expr);
		}
	}
	Vec::new()
}

/// Parse `Vec<String>` from expression with .to_string() calls
fn parse_string_vec_with_to_string(expr: &Expr) -> Vec<String> {
	let mut result = Vec::new();

	match expr {
		Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			if let Ok(parsed) = syn::parse2::<syn::ExprArray>(quote::quote! { [#tokens] }) {
				for elem in &parsed.elems {
					// Handle "string".to_string() pattern
					if let Expr::MethodCall(method_call) = elem
						&& method_call.method == "to_string"
					{
						if let Some(s) = extract_string_literal(&method_call.receiver) {
							result.push(s);
						}
					}
					// Handle direct string literal
					else if let Some(s) = extract_string_literal(elem) {
						result.push(s);
					}
				}
			}
		}
		Expr::Array(expr_array) => {
			for elem in &expr_array.elems {
				if let Expr::MethodCall(method_call) = elem
					&& method_call.method == "to_string"
				{
					if let Some(s) = extract_string_literal(&method_call.receiver) {
						result.push(s);
					}
				} else if let Some(s) = extract_string_literal(elem) {
					result.push(s);
				}
			}
		}
		_ => {}
	}

	result
}

/// Parse a single Constraint from struct expression
fn parse_single_constraint(expr: &Expr) -> Option<super::Constraint> {
	if let Expr::Struct(expr_struct) = expr {
		let variant_name = expr_struct.path.segments.last()?.ident.to_string();

		match variant_name.as_str() {
			"ForeignKey" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let columns = extract_string_vec_from_to_string(&expr_struct.fields, "columns");
				let referenced_table =
					extract_string_field(&expr_struct.fields, "referenced_table")?;
				let referenced_columns =
					extract_string_vec_from_to_string(&expr_struct.fields, "referenced_columns");
				let on_delete = extract_foreign_key_action_field(&expr_struct.fields, "on_delete")
					.unwrap_or(super::ForeignKeyAction::Restrict);
				let on_update = extract_foreign_key_action_field(&expr_struct.fields, "on_update")
					.unwrap_or(super::ForeignKeyAction::Restrict);

				return Some(super::Constraint::ForeignKey {
					name,
					columns,
					referenced_table,
					referenced_columns,
					on_delete,
					on_update,
					deferrable: None,
				});
			}
			"Unique" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let columns = extract_string_vec_from_to_string(&expr_struct.fields, "columns");

				return Some(super::Constraint::Unique { name, columns });
			}
			"Check" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let expression = extract_string_field(&expr_struct.fields, "expression")?;

				return Some(super::Constraint::Check { name, expression });
			}
			"EnumDomain" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let column = extract_string_field(&expr_struct.fields, "column")?;
				let domain = expr_struct.fields.iter().find_map(|field| {
					matches!(&field.member, syn::Member::Named(ident) if ident == "domain")
						.then(|| parse_field_domain(&field.expr))
						.flatten()
				})?;

				return Some(super::Constraint::EnumDomain {
					name,
					column,
					domain,
				});
			}
			"OneToOne" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let column = extract_string_field(&expr_struct.fields, "column")?;
				let referenced_table =
					extract_string_field(&expr_struct.fields, "referenced_table")?;
				let referenced_column =
					extract_string_field(&expr_struct.fields, "referenced_column")?;
				let on_delete = extract_foreign_key_action_field(&expr_struct.fields, "on_delete")
					.unwrap_or(super::ForeignKeyAction::Restrict);
				let on_update = extract_foreign_key_action_field(&expr_struct.fields, "on_update")
					.unwrap_or(super::ForeignKeyAction::NoAction);

				return Some(super::Constraint::OneToOne {
					name,
					column,
					referenced_table,
					referenced_column,
					on_delete,
					on_update,
					deferrable: None,
				});
			}
			"ManyToMany" => {
				let name = extract_string_field(&expr_struct.fields, "name")?;
				let through_table = extract_string_field(&expr_struct.fields, "through_table")?;
				let source_column = extract_string_field(&expr_struct.fields, "source_column")?;
				let target_column = extract_string_field(&expr_struct.fields, "target_column")?;
				let target_table = extract_string_field(&expr_struct.fields, "target_table")?;

				return Some(super::Constraint::ManyToMany {
					name,
					through_table,
					source_column,
					target_column,
					target_table,
				});
			}
			_ => {
				eprintln!(
					"Warning: Unhandled constraint type in AST parser: {}",
					variant_name
				);
			}
		}
	}

	None
}

/// Parse constraints from vec![...] or array expression
fn parse_constraints_vec(expr: &Expr) -> Vec<super::Constraint> {
	let mut constraints = Vec::new();

	match expr {
		// Handle vec![...] macro
		Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			if let Ok(parsed) = syn::parse2::<syn::ExprArray>(quote::quote! { [#tokens] }) {
				for elem in &parsed.elems {
					if let Some(constraint) = parse_single_constraint(elem) {
						constraints.push(constraint);
					}
				}
			}
		}
		// Handle array literal [...]
		Expr::Array(expr_array) => {
			for elem in &expr_array.elems {
				if let Some(constraint) = parse_single_constraint(elem) {
					constraints.push(constraint);
				}
			}
		}
		_ => {}
	}

	constraints
}

/// Extract constraints from struct
fn extract_constraints_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
) -> Vec<super::Constraint> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == "constraints"
		{
			return parse_constraints_vec(&field.expr);
		}
	}
	Vec::new()
}

/// Extract a single ColumnDefinition field
fn extract_column_definition_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<super::ColumnDefinition> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			return parse_column_definition(&field.expr);
		}
	}
	None
}

/// Extract an `Option<ColumnDefinition>` field.
fn extract_optional_column_definition_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<super::ColumnDefinition> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			return parse_optional_column_definition(&field.expr);
		}
	}
	None
}

fn parse_optional_column_definition(expr: &Expr) -> Option<super::ColumnDefinition> {
	if let Expr::Path(expr_path) = expr
		&& expr_path.path.is_ident("None")
	{
		return None;
	}

	if let Expr::Call(expr_call) = expr
		&& let Expr::Path(func_path) = &*expr_call.func
		&& func_path.path.is_ident("Some")
	{
		return parse_column_definition(expr_call.args.first()?);
	}

	parse_column_definition(expr)
}

/// Parse `Vec<ColumnDefinition>` from expression
fn parse_columns_vec(expr: &Expr) -> Vec<super::ColumnDefinition> {
	let mut columns = Vec::new();

	match expr {
		Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			if let Ok(parsed) = syn::parse2::<syn::ExprArray>(quote::quote! { [#tokens] }) {
				for elem in &parsed.elems {
					if let Some(col) = parse_column_definition(elem) {
						columns.push(col);
					}
				}
			}
		}
		Expr::Array(expr_array) => {
			for elem in &expr_array.elems {
				if let Some(col) = parse_column_definition(elem) {
					columns.push(col);
				}
			}
		}
		_ => {}
	}

	columns
}

/// Parse a single ColumnDefinition from struct expression
fn parse_column_definition(expr: &Expr) -> Option<super::ColumnDefinition> {
	if let Expr::Struct(expr_struct) = expr {
		// Verify it's a ColumnDefinition struct
		let struct_name = expr_struct.path.segments.last()?.ident.to_string();
		if struct_name != "ColumnDefinition" {
			return None;
		}

		let name = extract_string_field(&expr_struct.fields, "name")?;
		let type_definition = extract_field_type(&expr_struct.fields)
			.unwrap_or(super::FieldType::Custom("VARCHAR".to_string()));
		let not_null = extract_bool_field(&expr_struct.fields, "not_null").unwrap_or(false);
		let unique = extract_bool_field(&expr_struct.fields, "unique").unwrap_or(false);
		let primary_key = extract_bool_field(&expr_struct.fields, "primary_key").unwrap_or(false);
		let auto_increment =
			extract_bool_field(&expr_struct.fields, "auto_increment").unwrap_or(false);
		let default = extract_optional_str_field(&expr_struct.fields, "default");
		let generated = extract_generated_column_field(&expr_struct.fields);
		let domain = extract_field_domain(&expr_struct.fields);

		return Some(super::ColumnDefinition {
			name,
			type_definition,
			not_null,
			unique,
			primary_key,
			auto_increment,
			default,
			generated,
			domain,
		});
	}

	None
}

fn extract_field_domain(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
) -> Option<crate::field_domain::FieldDomain> {
	let expr = fields.iter().find_map(|field| {
		matches!(&field.member, syn::Member::Named(ident) if ident == "domain")
			.then_some(&field.expr)
	})?;
	let Expr::Call(call) = expr else {
		return None;
	};
	let Expr::Path(path) = &*call.func else {
		return None;
	};
	if !path.path.is_ident("Some") || call.args.len() != 1 {
		return None;
	}
	parse_field_domain(&call.args[0])
}

fn parse_field_domain(expr: &Expr) -> Option<crate::field_domain::FieldDomain> {
	let Expr::Struct(domain) = expr else {
		return None;
	};
	if domain.path.segments.last()?.ident != "Enum" {
		return None;
	}
	let repr_expr = domain.fields.iter().find_map(|field| {
		matches!(&field.member, syn::Member::Named(ident) if ident == "repr").then_some(&field.expr)
	})?;
	let repr_path = match repr_expr {
		Expr::Path(path) => path.path.segments.last()?.ident.to_string(),
		_ => return None,
	};
	let repr = match repr_path.as_str() {
		"String" => crate::field_domain::ModelEnumRepr::String,
		"I32" => crate::field_domain::ModelEnumRepr::I32,
		_ => return None,
	};
	let values_expr = domain.fields.iter().find_map(|field| {
		matches!(&field.member, syn::Member::Named(ident) if ident == "values")
			.then_some(&field.expr)
	})?;
	let values = parse_model_enum_values(values_expr)?;
	Some(crate::field_domain::FieldDomain::Enum { repr, values }.canonicalized())
}

fn parse_model_enum_values(expr: &Expr) -> Option<Vec<crate::field_domain::ModelEnumValue>> {
	let Expr::Macro(expr_macro) = expr else {
		return None;
	};
	if !expr_macro.mac.path.is_ident("vec") {
		return None;
	}
	let values = expr_macro
		.mac
		.parse_body_with(syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated)
		.ok()?;
	values.iter().map(parse_model_enum_value).collect()
}

fn parse_model_enum_value(expr: &Expr) -> Option<crate::field_domain::ModelEnumValue> {
	let Expr::Call(call) = expr else {
		return None;
	};
	let Expr::Path(path) = &*call.func else {
		return None;
	};
	let variant = path.path.segments.last()?.ident.to_string();
	let value = call.args.first()?;
	match variant.as_str() {
		"String" => extract_string_expr(value).map(crate::field_domain::ModelEnumValue::String),
		"I32" => parse_i32_expr(value).map(crate::field_domain::ModelEnumValue::I32),
		_ => None,
	}
}

fn parse_i32_expr(expr: &Expr) -> Option<i32> {
	i32::try_from(parse_signed_integer_expr(expr)?).ok()
}

fn parse_signed_integer_expr(expr: &Expr) -> Option<i128> {
	match expr {
		Expr::Lit(syn::ExprLit {
			lit: syn::Lit::Int(value),
			..
		}) => value.base10_parse().ok(),
		Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
			parse_signed_integer_expr(&unary.expr)?.checked_neg()
		}
		_ => None,
	}
}

fn extract_string_expr(expr: &Expr) -> Option<String> {
	match expr {
		Expr::MethodCall(call) if call.method == "to_string" => extract_string_expr(&call.receiver),
		Expr::Lit(syn::ExprLit {
			lit: syn::Lit::Str(value),
			..
		}) => Some(value.value()),
		_ => None,
	}
}

fn extract_generated_column_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
) -> Option<super::GeneratedColumnDefinition> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == "generated"
		{
			return parse_optional_generated_column_definition(&field.expr);
		}
	}
	None
}

fn parse_optional_generated_column_definition(
	expr: &Expr,
) -> Option<super::GeneratedColumnDefinition> {
	if let Expr::Path(expr_path) = expr
		&& expr_path.path.is_ident("None")
	{
		return None;
	}

	if let Expr::Call(expr_call) = expr
		&& let Expr::Path(func_path) = &*expr_call.func
		&& func_path.path.is_ident("Some")
		&& expr_call.args.len() == 1
	{
		return parse_generated_column_definition(&expr_call.args[0]);
	}

	None
}

fn parse_generated_column_definition(expr: &Expr) -> Option<super::GeneratedColumnDefinition> {
	if let Expr::Struct(expr_struct) = expr {
		let struct_name = expr_struct.path.segments.last()?.ident.to_string();
		if struct_name != "GeneratedColumnDefinition" {
			return None;
		}

		let expr_tokens = extract_optional_str_field(&expr_struct.fields, "expr_tokens")
			.or_else(|| extract_optional_expr_tokens_field(&expr_struct.fields, "expr"));
		let expr = extract_optional_schema_expr_field(&expr_struct.fields, "expr")
			.or_else(|| expr_tokens.as_deref().and_then(parse_schema_expr_tokens));
		let raw_sql = extract_optional_str_field(&expr_struct.fields, "raw_sql");
		let storage = extract_generated_storage_field(&expr_struct.fields)
			.unwrap_or(super::GeneratedStorage::Stored);

		return Some(super::GeneratedColumnDefinition {
			expr: expr.map(Box::new),
			expr_tokens,
			raw_sql,
			storage,
		});
	}

	if let Expr::Call(expr_call) = expr {
		let Expr::Path(func_path) = &*expr_call.func else {
			return None;
		};
		let constructor = func_path.path.segments.last()?.ident.to_string();
		return match constructor.as_str() {
			"typed" if expr_call.args.len() == 3 => {
				let expr = parse_schema_expr(&expr_call.args[0])?;
				let expr_tokens = extract_string_literal(&expr_call.args[1])?;
				let storage = parse_generated_storage_expr(&expr_call.args[2])?;
				Some(super::GeneratedColumnDefinition {
					expr: Some(Box::new(expr)),
					expr_tokens: Some(expr_tokens),
					raw_sql: None,
					storage,
				})
			}
			"raw_sql" if expr_call.args.len() == 2 => {
				let raw_sql = extract_string_literal(&expr_call.args[0])?;
				let storage = parse_generated_storage_expr(&expr_call.args[1])?;
				Some(super::GeneratedColumnDefinition {
					expr: None,
					expr_tokens: None,
					raw_sql: Some(raw_sql),
					storage,
				})
			}
			_ => None,
		};
	}

	None
}

pub(crate) fn parse_schema_expr_tokens(tokens: &str) -> Option<super::SchemaExpr> {
	syn::parse_str::<Expr>(tokens)
		.ok()
		.and_then(|expr| parse_schema_expr(&expr))
}

fn extract_optional_schema_expr_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
	field_name: &str,
) -> Option<super::SchemaExpr> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == field_name
		{
			return parse_optional_schema_expr(&field.expr);
		}
	}
	None
}

fn parse_optional_schema_expr(expr: &Expr) -> Option<super::SchemaExpr> {
	if let Expr::Path(expr_path) = expr
		&& expr_path.path.is_ident("None")
	{
		return None;
	}

	if let Expr::Call(expr_call) = expr
		&& let Expr::Path(func_path) = &*expr_call.func
		&& func_path.path.is_ident("Some")
		&& expr_call.args.len() == 1
	{
		let expr = unwrap_box_new_expr(&expr_call.args[0]).unwrap_or(&expr_call.args[0]);
		return parse_schema_expr(expr);
	}

	None
}

fn parse_schema_expr(expr: &Expr) -> Option<super::SchemaExpr> {
	match expr {
		Expr::Paren(expr_paren) => parse_schema_expr(&expr_paren.expr),
		Expr::Group(expr_group) => parse_schema_expr(&expr_group.expr),
		Expr::MethodCall(method_call) => {
			let receiver = parse_schema_expr(&method_call.receiver)?;
			match method_call.method.to_string().as_str() {
				"binary" if method_call.args.len() == 2 => {
					let op = parse_schema_bin_oper(&method_call.args[0])?;
					let right = parse_schema_expr(&method_call.args[1])?;
					Some(receiver.binary(op, right))
				}
				"cast" if method_call.args.len() == 1 => {
					let ty = parse_query_column_type(&method_call.args[0])?;
					Some(receiver.cast(ty))
				}
				_ => None,
			}
		}
		Expr::Call(expr_call) => {
			let Expr::Path(func_path) = &*expr_call.func else {
				return None;
			};
			let func = func_path.path.segments.last()?.ident.to_string();
			match func.as_str() {
				"col" if expr_call.args.len() == 1 => {
					let name = extract_string_literal(&expr_call.args[0])?;
					Some(super::SchemaExpr::col(name))
				}
				"val" if expr_call.args.len() == 1 => {
					let value = parse_schema_value(&expr_call.args[0])?;
					Some(super::SchemaExpr::Value(value))
				}
				"concat" if expr_call.args.len() == 1 => {
					let args = parse_schema_expr_items(&expr_call.args[0])?;
					Some(super::SchemaExpr::concat(args))
				}
				"coalesce" if expr_call.args.len() == 1 => {
					let args = parse_schema_expr_items(&expr_call.args[0])?;
					if args.is_empty() {
						return None;
					}
					Some(super::SchemaExpr::coalesce(args))
				}
				_ => None,
			}
		}
		_ => None,
	}
}

fn parse_schema_expr_items(expr: &Expr) -> Option<Vec<super::SchemaExpr>> {
	match expr {
		Expr::Array(expr_array) => expr_array.elems.iter().map(parse_schema_expr).collect(),
		Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			let parsed = syn::parse2::<syn::ExprArray>(quote::quote! { [#tokens] }).ok()?;
			parsed.elems.iter().map(parse_schema_expr).collect()
		}
		_ => None,
	}
}

fn parse_schema_bin_oper(expr: &Expr) -> Option<super::SchemaBinOper> {
	let Expr::Path(expr_path) = expr else {
		return None;
	};
	match expr_path.path.segments.last()?.ident.to_string().as_str() {
		"Add" => Some(super::SchemaBinOper::Add),
		"Sub" => Some(super::SchemaBinOper::Sub),
		"Mul" => Some(super::SchemaBinOper::Mul),
		"Div" => Some(super::SchemaBinOper::Div),
		_ => None,
	}
}

fn parse_schema_value(expr: &Expr) -> Option<QueryValue> {
	match expr {
		Expr::Path(expr_path) => parse_option_none_schema_value(&expr_path.path),
		Expr::Lit(expr_lit) => match &expr_lit.lit {
			syn::Lit::Str(lit_str) => Some(QueryValue::String(Some(Box::new(lit_str.value())))),
			syn::Lit::Bool(lit_bool) => Some(QueryValue::Bool(Some(lit_bool.value))),
			syn::Lit::Char(lit_char) => Some(QueryValue::Char(Some(lit_char.value()))),
			syn::Lit::Int(lit_int) => parse_integer_schema_value(lit_int),
			syn::Lit::Float(lit_float) => parse_float_schema_value(lit_float),
			_ => None,
		},
		Expr::Unary(expr_unary) => {
			if !matches!(expr_unary.op, syn::UnOp::Neg(_)) {
				return None;
			}
			match parse_schema_value(&expr_unary.expr)? {
				QueryValue::Int(Some(value)) => Some(QueryValue::Int(Some(-value))),
				QueryValue::BigInt(Some(value)) => Some(QueryValue::BigInt(Some(-value))),
				QueryValue::Float(Some(value)) => Some(QueryValue::Float(Some(-value))),
				QueryValue::Double(Some(value)) => Some(QueryValue::Double(Some(-value))),
				_ => None,
			}
		}
		_ => None,
	}
}

fn parse_integer_schema_value(lit_int: &syn::LitInt) -> Option<QueryValue> {
	match lit_int.suffix() {
		"i8" => lit_int
			.base10_parse::<i8>()
			.map(|value| QueryValue::TinyInt(Some(value)))
			.ok(),
		"i16" => lit_int
			.base10_parse::<i16>()
			.map(|value| QueryValue::SmallInt(Some(value)))
			.ok(),
		"i32" => lit_int
			.base10_parse::<i32>()
			.map(|value| QueryValue::Int(Some(value)))
			.ok(),
		"i64" => lit_int
			.base10_parse::<i64>()
			.map(|value| QueryValue::BigInt(Some(value)))
			.ok(),
		"u8" => lit_int
			.base10_parse::<u8>()
			.map(|value| QueryValue::TinyUnsigned(Some(value)))
			.ok(),
		"u16" => lit_int
			.base10_parse::<u16>()
			.map(|value| QueryValue::SmallUnsigned(Some(value)))
			.ok(),
		"u32" => lit_int
			.base10_parse::<u32>()
			.map(|value| QueryValue::Unsigned(Some(value)))
			.ok(),
		"u64" => lit_int
			.base10_parse::<u64>()
			.map(|value| QueryValue::BigUnsigned(Some(value)))
			.ok(),
		"" => lit_int
			.base10_parse::<i32>()
			.map(|value| QueryValue::Int(Some(value)))
			.or_else(|_| {
				lit_int
					.base10_parse::<i64>()
					.map(|value| QueryValue::BigInt(Some(value)))
			})
			.ok(),
		_ => None,
	}
}

fn parse_float_schema_value(lit_float: &syn::LitFloat) -> Option<QueryValue> {
	match lit_float.suffix() {
		"f32" => lit_float
			.base10_parse::<f32>()
			.map(|value| QueryValue::Float(Some(value)))
			.ok(),
		"f64" | "" => lit_float
			.base10_parse::<f64>()
			.map(|value| QueryValue::Double(Some(value)))
			.ok(),
		_ => None,
	}
}

fn parse_option_none_schema_value(path: &syn::Path) -> Option<QueryValue> {
	let mut segments = path.segments.iter();
	let option_segment = segments.next()?;
	let none_segment = segments.next()?;
	if segments.next().is_some() || option_segment.ident != "Option" || none_segment.ident != "None"
	{
		return None;
	}

	let syn::PathArguments::AngleBracketed(arguments) = &option_segment.arguments else {
		return None;
	};
	if arguments.args.len() != 1 {
		return None;
	}
	let Some(syn::GenericArgument::Type(syn::Type::Path(type_path))) = arguments.args.first()
	else {
		return None;
	};
	let ident = type_path.path.segments.last()?.ident.to_string();
	match ident.as_str() {
		"bool" => Some(QueryValue::Bool(None)),
		"i8" => Some(QueryValue::TinyInt(None)),
		"i16" => Some(QueryValue::SmallInt(None)),
		"i32" => Some(QueryValue::Int(None)),
		"i64" => Some(QueryValue::BigInt(None)),
		"u8" => Some(QueryValue::TinyUnsigned(None)),
		"u16" => Some(QueryValue::SmallUnsigned(None)),
		"u32" => Some(QueryValue::Unsigned(None)),
		"u64" => Some(QueryValue::BigUnsigned(None)),
		"f32" => Some(QueryValue::Float(None)),
		"f64" => Some(QueryValue::Double(None)),
		"char" => Some(QueryValue::Char(None)),
		"String" => Some(QueryValue::String(None)),
		_ => None,
	}
}

fn parse_query_column_type(expr: &Expr) -> Option<super::ColumnType> {
	match expr {
		Expr::Path(expr_path) => match expr_path.path.segments.last()?.ident.to_string().as_str() {
			"Text" => Some(super::ColumnType::Text),
			"TinyInteger" => Some(super::ColumnType::TinyInteger),
			"SmallInteger" => Some(super::ColumnType::SmallInteger),
			"Integer" => Some(super::ColumnType::Integer),
			"BigInteger" => Some(super::ColumnType::BigInteger),
			"Float" => Some(super::ColumnType::Float),
			"Double" => Some(super::ColumnType::Double),
			"Boolean" => Some(super::ColumnType::Boolean),
			"Date" => Some(super::ColumnType::Date),
			"Time" => Some(super::ColumnType::Time),
			"DateTime" => Some(super::ColumnType::DateTime),
			"Timestamp" => Some(super::ColumnType::Timestamp),
			"TimestampWithTimeZone" => Some(super::ColumnType::TimestampWithTimeZone),
			"Blob" => Some(super::ColumnType::Blob),
			"Uuid" => Some(super::ColumnType::Uuid),
			"Json" => Some(super::ColumnType::Json),
			"JsonBinary" => Some(super::ColumnType::JsonBinary),
			_ => None,
		},
		Expr::Call(expr_call) => {
			let Expr::Path(func_path) = &*expr_call.func else {
				return None;
			};
			let variant = func_path.path.segments.last()?.ident.to_string();
			match variant.as_str() {
				"Char" if expr_call.args.len() == 1 => Some(super::ColumnType::Char(
					parse_optional_u32(&expr_call.args[0])?,
				)),
				"String" if expr_call.args.len() == 1 => Some(super::ColumnType::String(
					parse_optional_u32(&expr_call.args[0])?,
				)),
				"Decimal" if expr_call.args.len() == 1 => Some(super::ColumnType::Decimal(
					parse_optional_u32_pair(&expr_call.args[0])?,
				)),
				"Binary" if expr_call.args.len() == 1 => Some(super::ColumnType::Binary(
					parse_optional_u32(&expr_call.args[0])?,
				)),
				"VarBinary" if expr_call.args.len() == 1 => Some(super::ColumnType::VarBinary(
					parse_u32_literal(&expr_call.args[0])?,
				)),
				"Array" if expr_call.args.len() == 1 => {
					let inner = unwrap_box_new_expr(&expr_call.args[0])?;
					Some(super::ColumnType::Array(Box::new(parse_query_column_type(
						inner,
					)?)))
				}
				#[cfg(feature = "pgvector")]
				"Vector" if expr_call.args.len() == 1 => Some(super::ColumnType::Vector(parse_u32_literal(
					&expr_call.args[0],
				)?)),
				"Custom" if expr_call.args.len() == 1 => Some(super::ColumnType::Custom(
					extract_string_literal(&expr_call.args[0])?,
				)),
				_ => None,
			}
		}
		_ => None,
	}
}

fn parse_optional_u32(expr: &Expr) -> Option<Option<u32>> {
	if let Expr::Path(expr_path) = expr
		&& expr_path.path.is_ident("None")
	{
		return Some(None);
	}
	if let Expr::Call(expr_call) = expr
		&& let Expr::Path(func_path) = &*expr_call.func
		&& func_path.path.is_ident("Some")
		&& expr_call.args.len() == 1
	{
		return Some(Some(parse_u32_literal(&expr_call.args[0])?));
	}
	None
}

fn parse_optional_u32_pair(expr: &Expr) -> Option<Option<(u32, u32)>> {
	if let Expr::Path(expr_path) = expr
		&& expr_path.path.is_ident("None")
	{
		return Some(None);
	}
	if let Expr::Call(expr_call) = expr
		&& let Expr::Path(func_path) = &*expr_call.func
		&& func_path.path.is_ident("Some")
		&& expr_call.args.len() == 1
		&& let Expr::Tuple(tuple) = &expr_call.args[0]
		&& tuple.elems.len() == 2
	{
		return Some(Some((
			parse_u32_literal(&tuple.elems[0])?,
			parse_u32_literal(&tuple.elems[1])?,
		)));
	}
	None
}

fn parse_u32_literal(expr: &Expr) -> Option<u32> {
	let Expr::Lit(expr_lit) = expr else {
		return None;
	};
	let syn::Lit::Int(lit_int) = &expr_lit.lit else {
		return None;
	};
	lit_int.base10_parse::<u32>().ok()
}

fn unwrap_box_new_expr(expr: &Expr) -> Option<&Expr> {
	let Expr::Call(expr_call) = expr else {
		return None;
	};
	let Expr::Path(func_path) = &*expr_call.func else {
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

fn extract_generated_storage_field(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
) -> Option<super::GeneratedStorage> {
	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == "storage"
		{
			return parse_generated_storage_expr(&field.expr);
		}
	}
	None
}

fn parse_generated_storage_expr(expr: &Expr) -> Option<super::GeneratedStorage> {
	if let Expr::Path(expr_path) = expr
		&& let Some(last_segment) = expr_path.path.segments.last()
	{
		return match last_segment.ident.to_string().as_str() {
			"Stored" => Some(super::GeneratedStorage::Stored),
			"Virtual" => Some(super::GeneratedStorage::Virtual),
			_ => None,
		};
	}
	None
}

/// Extract a field value from Migration struct literal
fn extract_field_from_migration_struct(expr: &Expr, field_name: &str) -> Option<Expr> {
	if let Expr::Struct(expr_struct) = expr {
		// Check if this is a Migration struct
		if expr_struct.path.segments.last()?.ident == "Migration" {
			// Find the field we're looking for
			for field in &expr_struct.fields {
				if let syn::Member::Named(ident) = &field.member
					&& ident == field_name
				{
					return Some(field.expr.clone());
				}
			}
		}
	}
	None
}

/// Parse a vec![...] or array expression containing tuples of strings
fn parse_tuple_vec_expr(expr: &Expr) -> Result<Vec<(String, String)>> {
	let mut result = Vec::new();

	match expr {
		// Handle vec![...] macro
		Expr::Macro(expr_macro) if expr_macro.mac.path.is_ident("vec") => {
			let tokens = &expr_macro.mac.tokens;
			// Wrap tokens in array brackets so syn can parse comma-separated items
			if let Ok(parsed) = syn::parse2::<syn::ExprArray>(quote::quote! { [#tokens] }) {
				for item in &parsed.elems {
					if let Some(tuple) = extract_string_tuple(item) {
						result.push(tuple);
					}
				}
			}
		}
		// Handle array literal [...]
		Expr::Array(expr_array) => {
			for item in &expr_array.elems {
				if let Some(tuple) = extract_string_tuple(item) {
					result.push(tuple);
				}
			}
		}
		_ => {}
	}

	Ok(result)
}

fn parse_tuple_vec_expr_strict(expr: &Expr, field_name: &str) -> Result<Vec<(String, String)>> {
	parse_vec_expressions(expr, field_name)?
		.iter()
		.map(|expression| {
			extract_string_tuple(expression).ok_or_else(|| {
				MigrationError::InvalidMigration(format!(
					"Malformed migration '{}' metadata",
					field_name
				))
			})
		})
		.collect()
}

/// Extract a tuple of two strings from an expression like ("app", "name")
fn extract_string_tuple(expr: &Expr) -> Option<(String, String)> {
	if let Expr::Tuple(expr_tuple) = expr
		&& expr_tuple.elems.len() == 2
	{
		let first = extract_string_literal(&expr_tuple.elems[0])?;
		let second = extract_string_literal(&expr_tuple.elems[1])?;
		return Some((first, second));
	}
	None
}

/// Extract string value from a literal expression or `.to_string()` method call
fn extract_string_literal(expr: &Expr) -> Option<String> {
	// Handle direct string literal: "foo"
	if let Expr::Lit(expr_lit) = expr
		&& let syn::Lit::Str(lit_str) = &expr_lit.lit
	{
		return Some(lit_str.value());
	}
	// Handle "foo".to_string() pattern
	if let Expr::MethodCall(method_call) = expr
		&& method_call.method == "to_string"
	{
		return extract_string_literal(&method_call.receiver);
	}
	None
}

/// Helper to parse `true` or `false` return
fn parse_bool_return(func: &ItemFn) -> Option<bool> {
	if let Some(Stmt::Expr(Expr::Lit(expr_lit), _)) = func.block.stmts.last()
		&& let syn::Lit::Bool(lit_bool) = &expr_lit.lit
	{
		return Some(lit_bool.value);
	}
	None
}

#[cfg(test)]
mod tests {
	use quote::ToTokens;
	use rstest::rstest;

	use super::{extract_migration_metadata, extract_migration_metadata_strict};
	use crate::field_domain::{FieldDomain, ModelEnumRepr, ModelEnumValue};
	use crate::migrations::{
		AlterTableOptions, ColumnType, Constraint, FieldType, GeneratedStorage, IndexType,
		MySqlAlgorithm, MySqlLock, Operation, SchemaExpr,
	};

	#[rstest]
	#[case(
		"pub fn not_a_migration() {}",
		"Invalid migration: Missing migration() entrypoint"
	)]
	#[case(
		r#"pub fn migration() -> Migration {
			Migration { operations: vec![], replaces: vec![] }
		}"#,
		"Invalid migration: Migration metadata is missing required 'dependencies' field"
	)]
	#[case(
		r#"pub fn migration() -> Migration {
			Migration { dependencies: vec![], replaces: vec![] }
		}"#,
		"Invalid migration: Migration metadata is missing required 'operations' field"
	)]
	fn strict_metadata_rejects_missing_entrypoint_and_required_fields(
		#[case] source: &str,
		#[case] expected: &str,
	) {
		// Arrange
		let ast = syn::parse_file(source).unwrap();

		// Act
		let error = extract_migration_metadata_strict(&ast, "blog", "0001_initial").unwrap_err();

		// Assert
		assert_eq!(error.to_string(), expected);
	}

	#[rstest]
	#[case(
		"Operation::UnknownOperation { value: 1 }",
		"Invalid migration: Unsupported or malformed migration operation 'UnknownOperation'"
	)]
	#[case(
		r#"Operation::DropTable { wrong_name: "posts".to_string() }"#,
		"Invalid migration: Unsupported or malformed migration operation 'DropTable'"
	)]
	fn strict_metadata_rejects_unknown_and_malformed_operations(
		#[case] operation: &str,
		#[case] expected: &str,
	) {
		// Arrange
		let source = format!(
			r#"pub fn migration() -> Migration {{
				Migration {{
					operations: vec![{operation}],
					dependencies: vec![],
					replaces: vec![],
				}}
			}}"#
		);
		let ast = syn::parse_file(&source).unwrap();

		// Act
		let error = extract_migration_metadata_strict(&ast, "blog", "0001_initial").unwrap_err();

		// Assert
		assert_eq!(error.to_string(), expected);
	}

	#[rstest]
	#[case(
		r#"dependencies: vec![("blog", 1)], replaces: vec![]"#,
		"Invalid migration: Malformed migration 'dependencies' metadata"
	)]
	#[case(
		r#"dependencies: vec![], replaces: vec![("blog",)]"#,
		"Invalid migration: Malformed migration 'replaces' metadata"
	)]
	fn strict_metadata_rejects_malformed_relationship_metadata(
		#[case] metadata: &str,
		#[case] expected: &str,
	) {
		// Arrange
		let source = format!(
			r#"pub fn migration() -> Migration {{
				Migration {{
					operations: vec![],
					{metadata},
				}}
			}}"#
		);
		let ast = syn::parse_file(&source).unwrap();

		// Act
		let error = extract_migration_metadata_strict(&ast, "blog", "0001_initial").unwrap_err();

		// Assert
		assert_eq!(error.to_string(), expected);
	}

	#[rstest]
	fn strict_metadata_preserves_catalog_flags_and_replacements() {
		// Arrange
		let ast = syn::parse_file(
			r#"pub fn migration() -> Migration {
				Migration {
					operations: vec![],
					dependencies: vec![("accounts", "0001_initial")],
					replaces: vec![("blog", "0001_old")],
					atomic: false,
					initial: Some(false),
					state_only: true,
					database_only: true,
				}
			}"#,
		)
		.unwrap();

		// Act
		let migration = extract_migration_metadata_strict(&ast, "blog", "0001_squashed").unwrap();

		// Assert
		assert_eq!(
			migration.dependencies,
			vec![("accounts".to_string(), "0001_initial".to_string())]
		);
		assert_eq!(
			migration.replaces,
			vec![("blog".to_string(), "0001_old".to_string())]
		);
		assert!(!migration.atomic);
		assert_eq!(migration.initial, Some(false));
		assert!(migration.state_only);
		assert!(migration.database_only);
	}

	#[test]
	fn strict_metadata_preserves_conditional_dependencies() {
		// Arrange
		let ast = syn::parse_file(
			r#"pub fn migration() -> Migration {
				Migration {
					operations: vec![],
					dependencies: vec![],
					replaces: vec![],
					swappable_dependencies: vec![
						SwappableDependency::new(
							"AUTH_USER_MODEL", "auth", "User", "0001_initial"
						)
					],
					optional_dependencies: vec![
						OptionalDependency::new(
							"gis",
							"0001_initial",
							DependencyCondition::AppInstalled("gis".to_string()),
						),
						OptionalDependency::new(
							"audit",
							"0002_events",
							DependencyCondition::SettingEnabled("AUDIT_ENABLED".to_string()),
						),
						OptionalDependency::new(
							"search",
							"0003_index",
							DependencyCondition::FeatureEnabled("search".to_string()),
						),
					],
				}
			}"#,
		)
		.unwrap();

		// Act
		let migration = extract_migration_metadata_strict(&ast, "blog", "0001_initial").unwrap();

		// Assert
		assert_eq!(
			migration.swappable_dependencies,
			vec![super::super::dependency::SwappableDependency::new(
				"AUTH_USER_MODEL",
				"auth",
				"User",
				"0001_initial",
			)]
		);
		assert_eq!(migration.optional_dependencies.len(), 3);
		assert!(matches!(
			migration.optional_dependencies[0].condition,
			super::super::dependency::DependencyCondition::AppInstalled(ref value)
				if value == "gis"
		));
		assert!(matches!(
			migration.optional_dependencies[1].condition,
			super::super::dependency::DependencyCondition::SettingEnabled(ref value)
				if value == "AUDIT_ENABLED"
		));
		assert!(matches!(
			migration.optional_dependencies[2].condition,
			super::super::dependency::DependencyCondition::FeatureEnabled(ref value)
				if value == "search"
		));
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn vector_index_tokens_reparse_data_bearing_index_types() {
		let operations = [
			Operation::CreateIndex {
				table: "source".to_string(),
				columns: vec!["embedding".to_string()],
				unique: false,
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				where_clause: Some("tenant_id IS NOT NULL".to_string()),
				concurrently: true,
				expressions: None,
				mysql_options: Some(
					AlterTableOptions::new()
						.with_algorithm(MySqlAlgorithm::Inplace)
						.with_lock(MySqlLock::Shared),
				),
				operator_class: Some("vector_cosine_ops".to_string()),
			},
			Operation::CreateNamedIndex {
				table: "source".to_string(),
				name: "source_embedding_ann".to_string(),
				columns: vec!["embedding".to_string()],
				unique: false,
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: Some("vector_cosine_ops".to_string()),
			},
			Operation::CreateIndexRepair {
				table: "source".to_string(),
				name: Some("source_normalized_embedding_l2".to_string()),
				columns: vec![],
				unique: false,
				index_type: Some(IndexType::Ivfflat { lists: Some(100) }),
				where_clause: Some("active".to_string()),
				concurrently: true,
				expressions: Some(vec!["normalize(embedding)".to_string()]),
				mysql_options: Some(
					AlterTableOptions::new()
						.with_algorithm(MySqlAlgorithm::Copy)
						.with_lock(MySqlLock::Exclusive),
				),
				operator_class: Some("vector_l2_ops".to_string()),
			},
			Operation::RestoreIndexOnRollback {
				table: "source".to_string(),
				name: Some("source_embedding_ip".to_string()),
				columns: vec!["embedding".to_string()],
				unique: false,
				index_type: Some(IndexType::Hnsw {
					m: None,
					ef_construction: None,
				}),
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: Some(AlterTableOptions::default()),
				operator_class: Some("vector_ip_ops".to_string()),
			},
			Operation::DropNamedIndex {
				table: "source".to_string(),
				name: "source_embedding_ann".to_string(),
				columns: vec!["embedding".to_string()],
				unique: false,
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: Some("vector_cosine_ops".to_string()),
			},
		];

		for operation in operations {
			let tokens = operation.to_token_stream().to_string();
			let expression: syn::Expr =
				syn::parse_str(&tokens).expect("generated operation tokens must parse");

			assert_eq!(
				super::parse_single_operation(&expression),
				Some(operation),
				"generated operation tokens must preserve vector index metadata: {tokens}"
			);
		}
	}

	#[test]
	fn drop_constraint_ast_accepts_legacy_and_typed_variants() {
		let legacy: syn::Expr = syn::parse_str(
			r#"Operation::DropConstraint {
				table: "jobs".to_string(),
				constraint_name: "jobs_status_check".to_string(),
			}"#,
		)
		.expect("legacy operation should parse as Rust syntax");
		assert_eq!(
			super::parse_single_operation(&legacy),
			Some(Operation::DropConstraint {
				table: "jobs".to_string(),
				constraint_name: "jobs_status_check".to_string(),
			})
		);

		let typed: syn::Expr = syn::parse_str(
			r#"Operation::DropConstraintDefinition {
				table: "jobs".to_string(),
				constraint: Constraint::EnumDomain {
					name: "jobs_status_check".to_string(),
					column: "status".to_string(),
					domain: FieldDomain::Enum {
						repr: ModelEnumRepr::String,
						values: vec![ModelEnumValue::String("queued".to_string())],
					},
				},
			}"#,
		)
		.expect("typed operation should parse as Rust syntax");
		assert!(matches!(
			super::parse_single_operation(&typed),
			Some(Operation::DropConstraintDefinition {
				constraint: Constraint::EnumDomain { column, .. },
				..
			}) if column == "status"
		));
	}

	#[test]
	fn extract_migration_metadata_restores_model_enum_domain() {
		let source = r#"
use reinhardt_db::migrations::prelude::*;

pub(super) fn migration() -> Migration {
	Migration {
		app_label: "jobs".to_string(),
		name: "0001_initial".to_string(),
		operations: vec![Operation::AddColumn {
			table: "jobs".to_string(),
			column: ColumnDefinition {
				name: "job_status".to_string(),
				type_definition: FieldType::VarChar(32),
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: Some(FieldDomain::Enum {
					repr: ModelEnumRepr::String,
					values: vec![
						ModelEnumValue::String("queued".to_string()),
						ModelEnumValue::String("running".to_string()),
					],
				}),
			},
			mysql_options: None,
		}],
		dependencies: vec![], atomic: true, replaces: vec![], initial: None,
		state_only: false, database_only: false,
		swappable_dependencies: vec![], optional_dependencies: vec![],
	}

}
"#;
		let ast = syn::parse_file(source).expect("migration source must parse");

		let migration = extract_migration_metadata(&ast, "jobs", "0001_initial")
			.expect("migration metadata must parse");
		let Operation::AddColumn { column, .. } = &migration.operations[0] else {
			panic!("expected AddColumn operation");
		};

		assert_eq!(
			column.domain,
			Some(FieldDomain::Enum {
				repr: ModelEnumRepr::String,
				values: vec![
					ModelEnumValue::String("queued".to_string()),
					ModelEnumValue::String("running".to_string()),
				],
			})
		);
	}

	#[test]
	fn extract_migration_metadata_restores_negative_enum_domain_constraint() {
		let source = r#"
use reinhardt_db::migrations::prelude::*;

pub(super) fn migration() -> Migration {
	Migration {
		app_label: "jobs".to_string(), name: "0001_initial".to_string(),
		operations: vec![Operation::CreateTable {
			name: "jobs".to_string(), columns: vec![],
			constraints: vec![Constraint::EnumDomain {
				name: "jobs_status_model_enum_check".to_string(),
				column: "status".to_string(),
				domain: FieldDomain::Enum {
					repr: ModelEnumRepr::I32,
					values: vec![ModelEnumValue::I32(-2147483648i32), ModelEnumValue::I32(1)],
				},
			}],
			without_rowid: None, interleave_in_parent: None, partition: None,
		}],
		dependencies: vec![], atomic: true, replaces: vec![], initial: Some(true),
		state_only: false, database_only: false,
		swappable_dependencies: vec![], optional_dependencies: vec![],
	}
}
"#;
		let ast = syn::parse_file(source).expect("migration source must parse");

		let migration = extract_migration_metadata(&ast, "jobs", "0001_initial")
			.expect("migration metadata must parse");
		let Operation::CreateTable { constraints, .. } = &migration.operations[0] else {
			panic!("expected CreateTable operation");
		};

		assert_eq!(
			constraints,
			&vec![crate::migrations::Constraint::EnumDomain {
				name: "jobs_status_model_enum_check".to_string(),
				column: "status".to_string(),
				domain: FieldDomain::Enum {
					repr: ModelEnumRepr::I32,
					values: vec![ModelEnumValue::I32(i32::MIN), ModelEnumValue::I32(1)],
				},
			}]
		);
	}

	#[test]
	fn extract_migration_metadata_restores_typed_generated_expression() {
		let source = r#"
use reinhardt_db::migrations::prelude::*;

pub(super) fn migration() -> Migration {
	Migration {
		app_label: "accounts".to_string(),
		name: "0002_full_name".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition {
					name: "full_name".to_string(),
					type_definition: FieldType::VarChar(201),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: Some(GeneratedColumnDefinition {
						expr: Some(Box::new(SchemaExpr::concat([
							SchemaExpr::col("first_name"),
							SchemaExpr::val(" "),
							SchemaExpr::col("last_name"),
						]))),
						expr_tokens: Some("SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])".to_string()),
						raw_sql: None,
						storage: GeneratedStorage::Stored,
					}),
					domain: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
"#;
		let ast = syn::parse_file(source).expect("migration source must parse");

		let migration = extract_migration_metadata(&ast, "accounts", "0002_full_name")
			.expect("migration metadata must parse");

		let Operation::AddColumn { column, .. } = &migration.operations[0] else {
			panic!("expected AddColumn operation");
		};
		let generated = column
			.generated
			.as_ref()
			.expect("generated metadata must be restored");
		assert_eq!(generated.storage, GeneratedStorage::Stored);
		assert_eq!(
			generated.expr.as_deref(),
			Some(&SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]))
		);
	}

	#[test]
	fn extract_migration_metadata_restores_generated_constructor_calls() {
		let source = r#"
use reinhardt_db::migrations::prelude::*;

pub(super) fn migration() -> Migration {
	Migration {
		app_label: "accounts".to_string(),
		name: "0002_full_name".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition {
					name: "full_name".to_string(),
					type_definition: FieldType::VarChar(201),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: Some(GeneratedColumnDefinition::typed(
						SchemaExpr::concat([
							SchemaExpr::col("first_name"),
							SchemaExpr::val(" "),
							SchemaExpr::col("last_name"),
						]),
						"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
						GeneratedStorage::Stored,
					)),
					domain: None,
				},
				mysql_options: None,
			},
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition {
					name: "search_name".to_string(),
					type_definition: FieldType::VarChar(201),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: Some(GeneratedColumnDefinition::raw_sql("LOWER(full_name)", GeneratedStorage::Virtual)),
					domain: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![],
		atomic: true,
		replaces: vec![],
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
"#;
		let ast = syn::parse_file(source).expect("migration source must parse");

		let migration = extract_migration_metadata(&ast, "accounts", "0002_full_name")
			.expect("migration metadata must parse");

		let Operation::AddColumn { column, .. } = &migration.operations[0] else {
			panic!("expected AddColumn operation");
		};
		let generated = column
			.generated
			.as_ref()
			.expect("typed generated metadata must be restored");
		assert_eq!(generated.storage, GeneratedStorage::Stored);
		assert_eq!(
			generated.expr.as_deref(),
			Some(&SchemaExpr::concat([
				SchemaExpr::col("first_name"),
				SchemaExpr::val(" "),
				SchemaExpr::col("last_name"),
			]))
		);

		let Operation::AddColumn { column, .. } = &migration.operations[1] else {
			panic!("expected AddColumn operation");
		};
		let generated = column
			.generated
			.as_ref()
			.expect("raw generated metadata must be restored");
		assert_eq!(generated.storage, GeneratedStorage::Virtual);
		assert_eq!(generated.raw_sql.as_deref(), Some("LOWER(full_name)"));
	}

	#[test]
	fn parse_schema_expr_tokens_accepts_custom_cast_to_string_tokens() {
		let parsed = super::parse_schema_expr_tokens(
			r#"SchemaExpr::col("name").cast(ColumnType::Custom("CITEXT".to_string()))"#,
		)
		.expect("custom cast tokens should parse");

		assert_eq!(
			parsed,
			SchemaExpr::col("name").cast(ColumnType::Custom("CITEXT".to_string()))
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn parse_schema_expr_tokens_accepts_vector_casts() {
		let parsed = super::parse_schema_expr_tokens(
			r#"SchemaExpr::col("embedding").cast(ColumnType::Vector(3))"#,
		)
		.expect("vector cast tokens should parse");

		assert_eq!(
			parsed,
			SchemaExpr::col("embedding").cast(ColumnType::Vector(3))
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn extract_field_type_accepts_vector_arrays() {
		let column: syn::ExprStruct = syn::parse_quote! {
			ColumnDefinition {
				type_definition: FieldType::Array(Box::new(FieldType::Vector {
					dimensions: 3,
				}))
			}
		};

		assert_eq!(
			super::extract_field_type(&column.fields),
			Some(FieldType::Array(Box::new(FieldType::Vector {
				dimensions: 3,
			})))
		);
	}
}

/// Extract FieldType from type_definition field
fn extract_field_type(
	fields: &syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
) -> Option<super::FieldType> {
	use super::FieldType;

	for field in fields {
		if let syn::Member::Named(ident) = &field.member
			&& ident == "type_definition"
		{
			// Handle FieldType::Variant or path::to::FieldType::Variant
			if let Expr::Path(expr_path) = &field.expr {
				let segments: Vec<_> = expr_path
					.path
					.segments
					.iter()
					.map(|s| s.ident.to_string())
					.collect();

				// Get the last segment as the variant name
				if let Some(last_segment) = expr_path.path.segments.last() {
					let variant = last_segment.ident.to_string();

					return match variant.as_str() {
						"Integer" => Some(FieldType::Integer),
						"BigInteger" => Some(FieldType::BigInteger),
						"SmallInteger" => Some(FieldType::SmallInteger),
						"TinyInt" => Some(FieldType::TinyInt),
						"MediumInt" => Some(FieldType::MediumInt),
						"Text" => Some(FieldType::Text),
						"TinyText" => Some(FieldType::TinyText),
						"MediumText" => Some(FieldType::MediumText),
						"LongText" => Some(FieldType::LongText),
						"Date" => Some(FieldType::Date),
						"Time" => Some(FieldType::Time),
						"DateTime" => Some(FieldType::DateTime),
						"TimestampTz" => Some(FieldType::TimestampTz),
						"Float" => Some(FieldType::Float),
						"Double" => Some(FieldType::Double),
						"Real" => Some(FieldType::Real),
						"Boolean" => Some(FieldType::Boolean),
						"Binary" => Some(FieldType::Binary),
						"Blob" => Some(FieldType::Blob),
						"TinyBlob" => Some(FieldType::TinyBlob),
						"MediumBlob" => Some(FieldType::MediumBlob),
						"LongBlob" => Some(FieldType::LongBlob),
						"Bytea" => Some(FieldType::Bytea),
						"Json" => Some(FieldType::Json),
						"JsonBinary" => Some(FieldType::JsonBinary),
						"Uuid" => Some(FieldType::Uuid),
						"Year" => Some(FieldType::Year),
						_ => Some(FieldType::Custom(segments.join("::"))),
					};
				}
			}
			// Handle FieldType::VarChar(n) or FieldType::Char(n)
			else if let Expr::Call(expr_call) = &field.expr {
				if let Expr::Path(func_path) = &*expr_call.func
					&& let Some(last_segment) = func_path.path.segments.last()
				{
					let variant = last_segment.ident.to_string();

					#[cfg(feature = "pgvector")]
					if variant == "Array" && expr_call.args.len() == 1 {
						let inner = unwrap_box_new_expr(&expr_call.args[0])?;
						if let Expr::Struct(inner_struct) = inner
							&& inner_struct
								.path
								.segments
								.last()
								.is_some_and(|segment| segment.ident == "Vector")
						{
							for field_value in &inner_struct.fields {
								if let syn::Member::Named(ident) = &field_value.member
									&& ident == "dimensions" && let Expr::Lit(expr_lit) =
									&field_value.expr && let syn::Lit::Int(lit_int) = &expr_lit.lit
									&& let Ok(dimensions) = lit_int.base10_parse::<usize>()
								{
									return Some(FieldType::Array(Box::new(FieldType::Vector {
										dimensions,
									})));
								}
							}
						}
					}

					if !expr_call.args.is_empty()
						&& let Expr::Lit(expr_lit) = &expr_call.args[0]
						&& let syn::Lit::Int(lit_int) = &expr_lit.lit
						&& let Ok(size) = lit_int.base10_parse::<u32>()
					{
						return match variant.as_str() {
							"VarChar" => Some(FieldType::VarChar(size)),
							"Char" => Some(FieldType::Char(size)),
							_ => None,
						};
					}
				}
			}
			// Handle FieldType::Decimal { precision, scale }
			// Handle FieldType::OneToOne { to, on_delete, on_update }
			// Handle FieldType::ManyToMany { to, through }
			else if let Expr::Struct(expr_struct) = &field.expr {
				if let Some(last_segment) = expr_struct.path.segments.last() {
					let variant = last_segment.ident.to_string();

					match variant.as_str() {
						#[cfg(feature = "pgvector")]
						"Vector" => {
							for field_value in &expr_struct.fields {
								if let syn::Member::Named(ident) = &field_value.member
									&& ident == "dimensions" && let Expr::Lit(expr_lit) =
									&field_value.expr && let syn::Lit::Int(lit_int) = &expr_lit.lit
									&& let Ok(dimensions) = lit_int.base10_parse::<usize>()
								{
									return Some(FieldType::Vector { dimensions });
								}
							}
						}
						"Decimal" => {
							let mut precision = 10u32;
							let mut scale = 0u32;

							for field_value in &expr_struct.fields {
								if let syn::Member::Named(field_ident) = &field_value.member
									&& let Expr::Lit(expr_lit) = &field_value.expr
									&& let syn::Lit::Int(lit_int) = &expr_lit.lit
									&& let Ok(val) = lit_int.base10_parse::<u32>()
								{
									if field_ident == "precision" {
										precision = val;
									} else if field_ident == "scale" {
										scale = val;
									}
								}
							}

							return Some(FieldType::Decimal { precision, scale });
						}
						"OneToOne" => {
							// Extract required field: to
							let to = extract_string_field(&expr_struct.fields, "to")?;

							// Extract optional fields with defaults
							let on_delete =
								extract_foreign_key_action_field(&expr_struct.fields, "on_delete")
									.unwrap_or(super::ForeignKeyAction::Restrict);
							let on_update =
								extract_foreign_key_action_field(&expr_struct.fields, "on_update")
									.unwrap_or(super::ForeignKeyAction::NoAction);

							return Some(FieldType::OneToOne {
								to,
								on_delete,
								on_update,
							});
						}
						"ManyToMany" => {
							// Extract required field: to
							let to = extract_string_field(&expr_struct.fields, "to")?;

							// Extract optional field: through
							let through =
								extract_optional_str_field(&expr_struct.fields, "through");

							return Some(FieldType::ManyToMany { to, through });
						}
						_ => {}
					}
				}
			}
			// Handle FieldType::Custom("...")
			else if let Expr::Call(expr_call) = &field.expr
				&& let Expr::Path(func_path) = &*expr_call.func
				&& let Some(last_segment) = func_path.path.segments.last()
				&& last_segment.ident == "Custom"
				&& !expr_call.args.is_empty()
				&& let Some(s) = extract_string_literal(&expr_call.args[0])
			{
				return Some(FieldType::Custom(s));
			}
		}
	}
	None
}
