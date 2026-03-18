//! AST parser utilities for migration files
//!
//! Provides helper functions to extract migration metadata and operations
//! from parsed Rust ASTs.

use super::{Migration, Result};
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

/// Parse a single Operation from an expression
fn parse_single_operation(expr: &Expr) -> Option<super::Operation> {
	// Handle Operation::CreateTable { ... }
	if let Expr::Struct(expr_struct) = expr {
		// Extract the variant name
		let variant_name = expr_struct.path.segments.last()?.ident.to_string();

		match variant_name.as_str() {
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
				return Some(super::Operation::DropColumn { table, column });
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
			"CreateIndex" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let columns = extract_string_vec_field(&expr_struct.fields, "columns");
				let unique = extract_bool_field(&expr_struct.fields, "unique").unwrap_or(false);
				let index_type = extract_index_type_field(&expr_struct.fields, "index_type");
				let where_clause = extract_optional_str_field(&expr_struct.fields, "where_clause");
				let concurrently =
					extract_bool_field(&expr_struct.fields, "concurrently").unwrap_or(false);

				return Some(super::Operation::CreateIndex {
					table,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions: None,
					mysql_options: None,
					operator_class: None,
				});
			}
			"DropIndex" => {
				let table = extract_string_field(&expr_struct.fields, "table")?;
				let columns = extract_string_vec_field(&expr_struct.fields, "columns");
				return Some(super::Operation::DropIndex { table, columns });
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
				&& let Expr::Path(variant_path) = &expr_call.args[0]
				&& let Some(last_segment) = variant_path.path.segments.last()
			{
				let variant = last_segment.ident.to_string();
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
		}
	}
	None
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

		return Some(super::ColumnDefinition {
			name,
			type_definition,
			not_null,
			unique,
			primary_key,
			auto_increment,
			default,
		});
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
			// Parse the tokens inside vec! as an array expression
			let tokens = &expr_macro.mac.tokens;
			// Try to parse as array
			if let Ok(array) = syn::parse2::<Expr>(tokens.clone()) {
				if let Expr::Array(expr_array) = array {
					for item in &expr_array.elems {
						if let Some(tuple) = extract_string_tuple(item) {
							result.push(tuple);
						}
					}
				} else {
					// Try parsing as single tuple
					if let Some(tuple) = extract_string_tuple(&array) {
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
