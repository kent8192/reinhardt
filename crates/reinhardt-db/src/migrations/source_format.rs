//! Versioned, source-preserving migration-file upgrades.

use super::{MigrationError, Result};
use proc_macro2::{LineColumn, Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Expr, ExprStruct, File, Item, Stmt};

/// Current generated migration source format.
pub const CURRENT_SOURCE_FORMAT_VERSION: u32 = 1;

const MARKER_PREFIX: &str = "// reinhardt-migration-source:";

/// Result of upgrading one migration source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeResult {
	/// Source after conversion and marker insertion.
	pub source: String,
	/// Whether the source bytes changed.
	pub changed: bool,
	/// Source format version before the upgrade, or `None` when unversioned.
	pub from_version: Option<u32>,
	/// Source format version after the upgrade.
	pub to_version: u32,
}

/// Upgrade one migration source file without formatting unrelated text.
///
/// The upgrader intentionally accepts only the generated migration expression
/// and known framework-owned struct literals. Other Rust code is left alone;
/// malformed or ambiguous generated shapes fail closed before any write.
pub fn upgrade_source(source: &str) -> Result<UpgradeResult> {
	let marker = parse_marker(source)?;
	if let Some(version) = marker
		&& version > CURRENT_SOURCE_FORMAT_VERSION
	{
		return Err(MigrationError::InvalidMigration(format!(
			"migration source format {version} requires a newer Reinhardt tool (current: {})",
			CURRENT_SOURCE_FORMAT_VERSION
		)));
	}

	let (mut current, converted) = convert_legacy_source(source)?;

	if marker == Some(CURRENT_SOURCE_FORMAT_VERSION) && converted {
		return Err(invalid_shape(
			"source format marker is current but legacy struct-literal syntax remains",
		));
	}
	if !converted {
		let file = syn::parse_file(&current).map_err(|error| {
			MigrationError::InvalidMigration(format!("failed to parse migration source: {error}"))
		})?;
		super::ast_parser::extract_migration_metadata_strict(&file, "<app>", "<name>")?;
	}

	let needs_marker = marker != Some(CURRENT_SOURCE_FORMAT_VERSION);
	if needs_marker {
		current = add_marker(&current, marker.is_some());
	}

	if converted {
		validate_semantics(source, &current)?;
	}

	Ok(UpgradeResult {
		changed: current != source,
		source: current,
		from_version: marker,
		to_version: CURRENT_SOURCE_FORMAT_VERSION,
	})
}

fn convert_legacy_source(source: &str) -> Result<(String, bool)> {
	let mut current = source.to_string();
	let mut converted = false;
	loop {
		let file = syn::parse_file(&current).map_err(|error| {
			MigrationError::InvalidMigration(format!("failed to parse migration source: {error}"))
		})?;
		let expressions = migration_expressions(&file)?;
		let targets = expressions
			.into_iter()
			.flat_map(outermost_target_structs)
			.collect::<Vec<_>>();
		if targets.is_empty() {
			break;
		}

		let mut edits = Vec::with_capacity(targets.len());
		for target in targets {
			let replacement = convert_struct(&target)?;
			let start_location = target
				.path
				.leading_colon
				.as_ref()
				.map(|colon| colon.span().start())
				.or_else(|| {
					target
						.path
						.segments
						.first()
						.map(|segment| segment.ident.span().start())
				})
				.unwrap_or_else(|| Span::call_site().start());
			let start = span_offset(&current, start_location)
				.ok_or_else(|| invalid_shape("cannot locate generated struct literal in source"))?;
			let end =
				span_offset(&current, target.brace_token.span.close().end()).ok_or_else(|| {
					invalid_shape("cannot locate generated struct literal end in source")
				})?;
			if start >= end {
				return Err(invalid_shape(
					"generated struct literal has an invalid source span",
				));
			}
			edits.push((
				start,
				end,
				preserve_comments(&current[start..end], replacement.to_string()),
			));
		}
		current = apply_edits(&current, edits)?;
		converted = true;
	}
	Ok((current, converted))
}

fn parse_marker(source: &str) -> Result<Option<u32>> {
	let content_start = source_start(source);
	let first_non_empty = source[content_start..]
		.lines()
		.find(|line| !line.trim().is_empty());
	let Some(first_non_empty) = first_non_empty else {
		return Err(invalid_shape("migration source is empty"));
	};
	let syntax_offset = first_syntax_offset(source)?;
	let marker_lines = leading_marker_lines(source, syntax_offset);
	if marker_lines.len() > 1 {
		return Err(invalid_shape(
			"migration source contains duplicate format markers",
		));
	}
	let trimmed = first_non_empty.trim();
	if !trimmed.starts_with(MARKER_PREFIX) {
		if !marker_lines.is_empty() {
			return Err(invalid_shape(
				"migration source format marker must be the first non-empty line",
			));
		}
		return Ok(None);
	}
	if marker_lines.is_empty() {
		return Ok(None);
	}
	let value = trimmed
		.strip_prefix(MARKER_PREFIX)
		.and_then(|value| value.trim().parse::<u32>().ok())
		.ok_or_else(|| invalid_shape("migration source format marker is malformed"))?;
	Ok(Some(value))
}

/// Return whether the leading Rust trivia contains a migration source marker.
///
/// This distinguishes marked migration sources from ordinary Rust helper files
/// without mistaking marker-like text inside strings or later comments.
pub fn has_source_format_marker(source: &str) -> Result<bool> {
	let syntax_offset = first_syntax_offset(source)?;
	Ok(!leading_marker_lines(source, syntax_offset).is_empty())
}

pub(crate) fn validate_source_version(source: &str) -> Result<()> {
	let marker = parse_marker(source)?;
	if let Some(version) = marker
		&& version > CURRENT_SOURCE_FORMAT_VERSION
	{
		return Err(MigrationError::InvalidMigration(format!(
			"migration source format {version} requires a newer Reinhardt tool (current: {})",
			CURRENT_SOURCE_FORMAT_VERSION
		)));
	}
	if marker == Some(CURRENT_SOURCE_FORMAT_VERSION) {
		upgrade_source(source)?;
	}
	Ok(())
}

fn first_syntax_offset(source: &str) -> Result<usize> {
	let source_start = source_start(source);
	let syntax = &source[source_start..];
	let tokens = syntax
		.parse::<TokenStream>()
		.map_err(|error| invalid_shape(&format!("failed to lex migration source: {error}")))?;
	let Some(token) = tokens.into_iter().next() else {
		return Ok(source.len());
	};
	span_offset(syntax, token.span().start())
		.map(|offset| source_start + offset)
		.ok_or_else(|| invalid_shape("cannot locate first Rust syntax token in source"))
}

fn bom_end(source: &str) -> usize {
	if source.starts_with('\u{feff}') {
		'\u{feff}'.len_utf8()
	} else {
		0
	}
}

fn shebang_end(source: &str) -> Option<usize> {
	let bom_end = bom_end(source);
	let line_end = source[bom_end..]
		.find('\n')
		.map_or(source.len(), |offset| bom_end + offset);
	let line = &source[bom_end..line_end];
	(line.starts_with("#!") && !line.starts_with("#!["))
		.then_some(line_end + usize::from(line_end < source.len()))
}

fn source_start(source: &str) -> usize {
	shebang_end(source).unwrap_or_else(|| bom_end(source))
}

fn leading_marker_lines(source: &str, syntax_offset: usize) -> Vec<&str> {
	let end = syntax_offset.min(source.len());
	let bytes = source.as_bytes();
	let mut index = source_start(source).min(end);
	let mut markers = Vec::new();
	while index < end {
		if bytes[index].is_ascii_whitespace() {
			index += 1;
			continue;
		}
		if bytes[index..].starts_with(b"//") {
			let line_end = source[index..end]
				.find('\n')
				.map_or(end, |offset| index + offset);
			let line = &source[index..line_end];
			if line.trim_start().starts_with(MARKER_PREFIX) {
				markers.push(line);
			}
			index = line_end + usize::from(line_end < end);
			continue;
		}
		if bytes[index..].starts_with(b"/*") {
			index = skip_block_comment(source, index, end);
			continue;
		}
		break;
	}
	markers
}

fn skip_block_comment(source: &str, start: usize, limit: usize) -> usize {
	let bytes = source.as_bytes();
	let mut depth = 1;
	let mut index = start + 2;
	while index + 1 < limit {
		if bytes[index..].starts_with(b"/*") {
			depth += 1;
			index += 2;
		} else if bytes[index..].starts_with(b"*/") {
			depth -= 1;
			index += 2;
			if depth == 0 {
				return index;
			}
		} else {
			index += 1;
		}
	}
	limit
}

fn migration_expressions(file: &File) -> Result<Vec<&Expr>> {
	let expressions = file
		.items
		.iter()
		.filter_map(|item| {
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
		.collect::<Vec<_>>();
	if expressions.is_empty() {
		return Err(invalid_shape("missing migration() entrypoint"));
	}
	Ok(expressions)
}

#[derive(Default)]
struct TargetVisitor {
	target_depth: usize,
	targets: Vec<ExprStruct>,
}

impl<'ast> syn::visit::Visit<'ast> for TargetVisitor {
	fn visit_expr_struct(&mut self, expression: &'ast ExprStruct) {
		let target = is_target_struct(expression);
		if target && self.target_depth == 0 {
			self.targets.push(expression.clone());
		}
		if target {
			self.target_depth += 1;
		}
		syn::visit::visit_expr_struct(self, expression);
		if target {
			self.target_depth -= 1;
		}
	}

	fn visit_expr_macro(&mut self, expression: &'ast syn::ExprMacro) {
		// `syn` treats macro bodies as opaque token streams. Generated
		// migration values commonly place framework-owned structs in `vec!`,
		// so parse only that standard macro and visit its expression elements.
		if expression.mac.path.is_ident("vec")
			&& let Ok(elements) = expression.mac.parse_body_with(
				syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated,
			) {
			for element in elements {
				syn::visit::Visit::visit_expr(self, &element);
			}
		}
	}
}

fn outermost_target_structs(expression: &Expr) -> Vec<ExprStruct> {
	let mut visitor = TargetVisitor::default();
	syn::visit::Visit::visit_expr(&mut visitor, expression);
	visitor.targets
}

fn is_target_struct(expression: &ExprStruct) -> bool {
	let Some(name) = expression
		.path
		.segments
		.last()
		.map(|segment| segment.ident.to_string())
	else {
		return false;
	};
	if !matches!(
		name.as_str(),
		"Migration"
			| "PartitionDef"
			| "InterleaveSpec"
			| "ColumnDefinition"
			| "GeneratedColumnDefinition"
			| "BulkLoadOptions"
	) {
		return false;
	}
	let module = if name == "Migration" {
		"migration"
	} else {
		"operations"
	};
	path_matches(&expression.path, &[name.as_str()])
		|| path_matches(
			&expression.path,
			&["reinhardt", "db", "migrations", name.as_str()],
		) || path_matches(
		&expression.path,
		&["reinhardt", "db", "migrations", module, name.as_str()],
	) || path_matches(
		&expression.path,
		&["reinhardt_db", "migrations", name.as_str()],
	) || path_matches(
		&expression.path,
		&["reinhardt_db", "migrations", module, name.as_str()],
	) || path_matches(&expression.path, &["crate", "migrations", name.as_str()])
		|| path_matches(
			&expression.path,
			&["crate", "migrations", module, name.as_str()],
		)
}

fn path_matches(path: &syn::Path, expected: &[&str]) -> bool {
	path.segments.len() == expected.len()
		&& path
			.segments
			.iter()
			.zip(expected)
			.all(|(segment, expected)| segment.ident == *expected)
}

fn convert_struct(expression: &ExprStruct) -> Result<TokenStream> {
	let name = expression
		.path
		.segments
		.last()
		.map(|segment| segment.ident.to_string())
		.ok_or_else(|| invalid_shape("generated struct literal has no type name"))?;
	match name.as_str() {
		"Migration" => convert_migration(expression),
		"PartitionDef" => convert_partition_def(expression),
		"InterleaveSpec" => convert_interleave_spec(expression),
		"ColumnDefinition" => convert_column_definition(expression),
		"GeneratedColumnDefinition" => convert_generated_column(expression),
		"BulkLoadOptions" => convert_bulk_load_options(expression),
		_ => Err(invalid_shape("unsupported generated struct literal")),
	}
}

fn convert_migration(expression: &ExprStruct) -> Result<TokenStream> {
	let uses_default_rest = validate_migration_fields(
		expression,
		&[
			"name",
			"app_label",
			"operations",
			"dependencies",
			"replaces",
			"atomic",
			"initial",
			"state_only",
			"database_only",
			"swappable_dependencies",
			"optional_dependencies",
		],
	)?;
	let name = field_expression(expression, "name")?;
	let app_label = field_expression(expression, "app_label")?;
	let migration_path = &expression.path;
	let mut builder = quote! { #migration_path :: new(#name, #app_label) };
	let operations_expression = field_expression(expression, "operations")?;
	for operation in vec_expressions(&operations_expression, "operations")? {
		if expression_contains_attributes(&operation) {
			return Err(invalid_shape(
				"generated migration field 'operations' contains an attributed operation; conditional entries are unsupported",
			));
		}
		builder.extend(quote! { .add_operation(#operation) });
	}
	let dependencies = field_expression(expression, "dependencies")?;
	for (app, migration) in tuple_pairs(dependencies, "dependencies")? {
		builder.extend(quote! { .add_dependency(#app, #migration) });
	}
	if let Some(replacements) = optional_field_expression(expression, "replaces") {
		for (app, migration) in tuple_pairs(replacements, "replaces")? {
			builder.extend(quote! { .add_replacement(#app, #migration) });
		}
	}
	let atomic = optional_field_expression(expression, "atomic").unwrap_or_else(|| {
		if uses_default_rest {
			syn::parse_quote!(false)
		} else {
			syn::parse_quote!(true)
		}
	});
	let initial =
		optional_field_expression(expression, "initial").unwrap_or_else(|| syn::parse_quote!(None));
	let state_only = optional_field_expression(expression, "state_only")
		.unwrap_or_else(|| syn::parse_quote!(false));
	let database_only = optional_field_expression(expression, "database_only")
		.unwrap_or_else(|| syn::parse_quote!(false));
	builder.extend(quote! {
		.atomic(#atomic)
		.with_initial(#initial)
		.state_only(#state_only)
		.database_only(#database_only)
	});
	for dependency in optional_vec_expressions(expression, "swappable_dependencies")? {
		if expression_contains_attributes(&dependency) {
			return Err(invalid_shape(
				"generated migration field 'swappable_dependencies' contains an attributed dependency; conditional entries are unsupported",
			));
		}
		builder.extend(quote! { .add_swappable_dependency(#dependency) });
	}
	for dependency in optional_vec_expressions(expression, "optional_dependencies")? {
		if expression_contains_attributes(&dependency) {
			return Err(invalid_shape(
				"generated migration field 'optional_dependencies' contains an attributed dependency; conditional entries are unsupported",
			));
		}
		builder.extend(quote! { .add_optional_dependency(#dependency) });
	}
	Ok(builder)
}

fn expression_contains_attributes(expression: &Expr) -> bool {
	#[derive(Default)]
	struct AttributeVisitor {
		found: bool,
	}

	impl<'ast> syn::visit::Visit<'ast> for AttributeVisitor {
		fn visit_attribute(&mut self, _attribute: &'ast syn::Attribute) {
			self.found = true;
		}
	}

	let mut visitor = AttributeVisitor::default();
	syn::visit::Visit::visit_expr(&mut visitor, expression);
	visitor.found
}

fn convert_partition_def(expression: &ExprStruct) -> Result<TokenStream> {
	validate_fields(expression, &["name", "values"])?;
	let name = field_expression(expression, "name")?;
	let values = field_expression(expression, "values")?;
	let path = &expression.path;
	Ok(quote! { #path :: new(#name, #values) })
}

fn convert_interleave_spec(expression: &ExprStruct) -> Result<TokenStream> {
	validate_fields(expression, &["parent_table", "parent_columns"])?;
	let table = field_expression(expression, "parent_table")?;
	let columns = field_expression(expression, "parent_columns")?;
	let path = &expression.path;
	Ok(quote! { #path :: new(#table, #columns) })
}

fn convert_column_definition(expression: &ExprStruct) -> Result<TokenStream> {
	validate_fields(
		expression,
		&[
			"name",
			"type_definition",
			"field_type",
			"not_null",
			"unique",
			"primary_key",
			"auto_increment",
			"default",
			"generated",
			"domain",
		],
	)?;
	validate_aliases(expression, &["type_definition", "field_type"])?;
	let name = field_expression(expression, "name")?;
	let type_definition = field_expression_alias(expression, &["type_definition", "field_type"])?;
	let not_null = optional_field_expression(expression, "not_null")
		.unwrap_or_else(|| syn::parse_quote!(false));
	let unique =
		optional_field_expression(expression, "unique").unwrap_or_else(|| syn::parse_quote!(false));
	let primary_key = optional_field_expression(expression, "primary_key")
		.unwrap_or_else(|| syn::parse_quote!(false));
	let auto_increment = optional_field_expression(expression, "auto_increment")
		.unwrap_or_else(|| syn::parse_quote!(false));
	let default =
		optional_field_expression(expression, "default").unwrap_or_else(|| syn::parse_quote!(None));
	let generated = optional_field_expression(expression, "generated")
		.unwrap_or_else(|| syn::parse_quote!(None));
	let domain =
		optional_field_expression(expression, "domain").unwrap_or_else(|| syn::parse_quote!(None));
	let path = &expression.path;
	Ok(quote! {
		#path :: new(#name, #type_definition)
			.with_not_null(#not_null)
			.with_unique(#unique)
			.with_primary_key(#primary_key)
			.with_auto_increment(#auto_increment)
			.with_default(#default)
			.with_generated(#generated)
			.with_domain_option(#domain)
	})
}

fn convert_generated_column(expression: &ExprStruct) -> Result<TokenStream> {
	validate_fields(expression, &["expr", "expr_tokens", "raw_sql", "storage"])?;
	let expr = optional_field_expression(expression, "expr");
	let expr_tokens = optional_field_expression(expression, "expr_tokens");
	let raw_sql = optional_field_expression(expression, "raw_sql");
	let storage = optional_field_expression(expression, "storage")
		.unwrap_or_else(|| syn::parse_quote!(GeneratedStorage::Stored));
	let path = &expression.path;
	let has_typed_expression = expr.clone().and_then(option_inner).is_some();
	let has_token_expression = expr_tokens.clone().and_then(option_inner).is_some();
	let has_raw_sql = raw_sql.clone().and_then(option_inner).is_some();
	if has_raw_sql && (has_typed_expression || has_token_expression) {
		return Err(invalid_shape(
			"GeneratedColumnDefinition contains conflicting expression representations",
		));
	}
	if let Some(expr) = expr.and_then(option_inner) {
		let expr = unwrap_box_new(expr.clone()).unwrap_or(expr);
		let Some(expr_tokens) = expr_tokens.and_then(option_inner) else {
			return Err(invalid_shape(
				"typed GeneratedColumnDefinition is missing expr_tokens",
			));
		};
		return Ok(quote! {
			#path :: typed(#expr, #expr_tokens, #storage)
		});
	}
	if let Some(raw_sql) = raw_sql.and_then(option_inner) {
		return Ok(quote! { #path :: raw_sql(#raw_sql, #storage) });
	}
	if let Some(expr_tokens) = expr_tokens.and_then(option_inner) {
		return Ok(quote! { #path :: tokens(#expr_tokens, #storage) });
	}
	Err(invalid_shape(
		"GeneratedColumnDefinition has no expression, expression tokens, or raw SQL",
	))
}

fn convert_bulk_load_options(expression: &ExprStruct) -> Result<TokenStream> {
	validate_fields(
		expression,
		&[
			"delimiter",
			"null_string",
			"header",
			"columns",
			"local",
			"quote",
			"escape",
			"line_terminator",
			"encoding",
		],
	)?;
	let delimiter = optional_field_expression(expression, "delimiter")
		.unwrap_or_else(|| syn::parse_quote!(None));
	let null_string = optional_field_expression(expression, "null_string")
		.unwrap_or_else(|| syn::parse_quote!(None));
	let header =
		optional_field_expression(expression, "header").unwrap_or_else(|| syn::parse_quote!(false));
	let columns =
		optional_field_expression(expression, "columns").unwrap_or_else(|| syn::parse_quote!(None));
	let local =
		optional_field_expression(expression, "local").unwrap_or_else(|| syn::parse_quote!(false));
	let quote_character =
		optional_field_expression(expression, "quote").unwrap_or_else(|| syn::parse_quote!(None));
	let escape =
		optional_field_expression(expression, "escape").unwrap_or_else(|| syn::parse_quote!(None));
	let line_terminator = optional_field_expression(expression, "line_terminator")
		.unwrap_or_else(|| syn::parse_quote!(None));
	let encoding = optional_field_expression(expression, "encoding")
		.unwrap_or_else(|| syn::parse_quote!(None));
	let path = &expression.path;
	Ok(quote! {
		#path :: new()
			.with_delimiter_option(#delimiter)
			.with_null_string_option(#null_string)
			.with_header(#header)
			.with_columns_option(#columns)
			.with_local(#local)
			.with_quote_option(#quote_character)
			.with_escape_option(#escape)
			.with_line_terminator_option(#line_terminator)
			.with_encoding_option(#encoding)
	})
}

fn validate_fields(expression: &ExprStruct, allowed: &[&str]) -> Result<()> {
	validate_struct_fields(expression, allowed, false).map(|_| ())
}

fn validate_migration_fields(expression: &ExprStruct, allowed: &[&str]) -> Result<bool> {
	validate_struct_fields(expression, allowed, true)
}

fn validate_struct_fields(
	expression: &ExprStruct,
	allowed: &[&str],
	allow_default_rest: bool,
) -> Result<bool> {
	let uses_default_rest = expression.rest.as_deref().is_some_and(is_default_rest);
	if expression.rest.is_some() && (!allow_default_rest || !uses_default_rest) {
		return Err(invalid_shape(
			"generated struct literal uses a struct-update expression",
		));
	}
	let mut seen = Vec::with_capacity(expression.fields.len());
	for field in &expression.fields {
		let syn::Member::Named(name) = &field.member else {
			return Err(invalid_shape(
				"generated struct literal uses an unnamed field",
			));
		};
		let name = name.to_string();
		if !field.attrs.is_empty() {
			return Err(invalid_shape(&format!(
				"generated struct literal field '{name}' contains attributes; conditional fields are unsupported"
			)));
		}
		if !allowed.iter().any(|allowed| *allowed == name) {
			return Err(invalid_shape(&format!(
				"generated struct literal contains unsupported field '{name}'"
			)));
		}
		if seen.iter().any(|seen: &String| seen == &name) {
			return Err(invalid_shape(&format!(
				"generated struct literal contains duplicate field '{name}'"
			)));
		}
		seen.push(name);
	}
	Ok(uses_default_rest)
}

fn is_default_rest(expression: &Expr) -> bool {
	let Expr::Call(call) = expression else {
		return false;
	};
	if !call.args.is_empty() {
		return false;
	}
	let Expr::Path(path) = &*call.func else {
		return false;
	};
	call.attrs.is_empty()
		&& path.attrs.is_empty()
		&& path.qself.is_none()
		&& path.path.leading_colon.is_none()
		&& path
			.path
			.segments
			.iter()
			.all(|segment| matches!(segment.arguments, syn::PathArguments::None))
		&& path_matches(&path.path, &["Default", "default"])
}

fn validate_aliases(expression: &ExprStruct, aliases: &[&str]) -> Result<()> {
	let present = expression
		.fields
		.iter()
		.filter_map(|field| match &field.member {
			syn::Member::Named(name) if aliases.iter().any(|alias| name == alias) => {
				Some(name.to_string())
			}
			_ => None,
		})
		.collect::<Vec<_>>();
	if present.len() > 1 {
		return Err(invalid_shape(&format!(
			"generated struct literal contains conflicting aliases: {}",
			present.join(", ")
		)));
	}
	Ok(())
}

fn field_expression(expression: &ExprStruct, name: &str) -> Result<Expr> {
	optional_field_expression(expression, name).ok_or_else(|| {
		invalid_shape(&format!(
			"generated struct literal is missing field '{name}'"
		))
	})
}

fn field_expression_alias(expression: &ExprStruct, names: &[&str]) -> Result<Expr> {
	names
		.iter()
		.find_map(|name| optional_field_expression(expression, name))
		.ok_or_else(|| invalid_shape("generated struct literal is missing a required type field"))
}

fn optional_field_expression(expression: &ExprStruct, name: &str) -> Option<Expr> {
	expression.fields.iter().find_map(|field| {
		matches!(&field.member, syn::Member::Named(member) if member == name)
			.then(|| field.expr.clone())
	})
}

fn optional_vec_expressions(expression: &ExprStruct, name: &str) -> Result<Vec<Expr>> {
	optional_field_expression(expression, name).map_or(Ok(Vec::new()), |expression| {
		vec_expressions(&expression, name)
	})
}

fn vec_expressions(expression: &Expr, field_name: &str) -> Result<Vec<Expr>> {
	match expression {
		Expr::Macro(expression) if expression.mac.path.is_ident("vec") => expression
			.mac
			.parse_body_with(syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated)
			.map(|items| items.into_iter().collect())
			.map_err(|error| invalid_shape(&format!("malformed {field_name} vector: {error}"))),
		Expr::Array(expression) => Ok(expression.elems.iter().cloned().collect()),
		Expr::Call(call)
			if call.args.is_empty()
				&& matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "new") && path.path.segments.iter().any(|segment| segment.ident == "Vec")) =>
		{
			Ok(Vec::new())
		}
		_ => Err(invalid_shape(&format!(
			"generated migration field '{field_name}' is not a vector"
		))),
	}
}

fn tuple_pairs(expression: Expr, field_name: &str) -> Result<Vec<(Expr, Expr)>> {
	vec_expressions(&expression, field_name)?
		.into_iter()
		.map(|expression| match expression {
			Expr::Tuple(tuple) if tuple.elems.len() == 2 && tuple.attrs.is_empty() => {
				let mut elems = tuple.elems.into_iter();
				Ok((
					elems.next().expect("tuple length checked"),
					elems.next().expect("tuple length checked"),
				))
			}
			Expr::Tuple(tuple) if !tuple.attrs.is_empty() => Err(invalid_shape(&format!(
				"generated migration field '{field_name}' contains an attributed tuple; conditional entries are unsupported"
			))),
			_ => Err(invalid_shape(&format!(
				"generated migration field '{field_name}' contains a non-pair"
			))),
		})
		.collect()
}

fn option_inner(expression: Expr) -> Option<Expr> {
	match expression {
		Expr::Path(path) if path.path.is_ident("None") => None,
		Expr::Call(call)
			if call.args.len() == 1
				&& matches!(&*call.func, Expr::Path(path) if path.path.is_ident("Some")) =>
		{
			Some((*call.args.first()?).clone())
		}
		_ => Some(expression),
	}
}

fn unwrap_box_new(expression: Expr) -> Option<Expr> {
	let Expr::Call(call) = expression else {
		return None;
	};
	if !matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "new") && path.path.segments.iter().any(|segment| segment.ident == "Box"))
	{
		return None;
	}
	(call.args.len() == 1).then(|| (*call.args.first().expect("argument length checked")).clone())
}

fn span_offset(source: &str, location: LineColumn) -> Option<usize> {
	if location.line == 0 {
		return None;
	}
	let mut offset = 0usize;
	for (line_index, line) in source.split_inclusive('\n').enumerate() {
		if line_index + 1 == location.line {
			let column = location.column + usize::from(location.line == 1) * bom_end(source);
			return (column <= line.len()).then_some(offset + column);
		}
		offset += line.len();
	}
	if location.line == source.lines().count() + usize::from(source.ends_with('\n')) {
		Some(offset + location.column)
	} else {
		None
	}
}

fn apply_edits(source: &str, mut edits: Vec<(usize, usize, String)>) -> Result<String> {
	edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
	let mut result = source.to_string();
	let mut next_start = source.len();
	for (start, end, replacement) in edits {
		if end > next_start || end > result.len() {
			return Err(invalid_shape("overlapping generated source edits"));
		}
		result.replace_range(start..end, &replacement);
		next_start = start;
	}
	Ok(result)
}

fn add_marker(source: &str, replace_existing: bool) -> String {
	let marker = format!("{MARKER_PREFIX} {CURRENT_SOURCE_FORMAT_VERSION}\n");
	let source_start = source_start(source);
	if replace_existing {
		let mut offset = source_start;
		for line in source[source_start..].split_inclusive('\n') {
			if !line.trim().is_empty() {
				let trimmed = line.trim_start();
				if trimmed.starts_with(MARKER_PREFIX) {
					let mut result = String::with_capacity(source.len() + marker.len());
					result.push_str(&source[..offset]);
					result.push_str(&marker);
					result.push_str(&source[offset + line.len()..]);
					return result;
				}
				break;
			}
			offset += line.len();
		}
	}
	format!(
		"{}{marker}{}",
		&source[..source_start],
		&source[source_start..]
	)
}

fn preserve_comments(original: &str, replacement: String) -> String {
	let comments = extract_comments(original);
	if comments.is_empty() {
		return replacement;
	}
	let mut preserved = String::new();
	for comment in comments {
		preserved.push_str(comment.trim_start());
		preserved.push('\n');
	}
	preserved.push_str(&replacement);
	preserved
}

fn extract_comments(source: &str) -> Vec<String> {
	let bytes = source.as_bytes();
	let mut comments = Vec::new();
	let mut index = 0;
	while index < bytes.len() {
		if let Some(end) = raw_string_end(source, index) {
			index = end;
			continue;
		}
		match bytes[index] {
			b'"' => {
				index = skip_quoted(source, index, b'"').unwrap_or(index + 1);
			}
			b'\'' => {
				index = skip_quoted(source, index, b'\'').unwrap_or(index + 1);
			}
			b'/' if bytes[index..].starts_with(b"//") => {
				let start = index;
				index += 2;
				while index < bytes.len() && bytes[index] != b'\n' {
					index += 1;
				}
				comments.push(source[start..index].to_owned());
			}
			b'/' if bytes[index..].starts_with(b"/*") => {
				let start = index;
				index = skip_block_comment(source, index, bytes.len());
				comments.push(source[start..index].to_owned());
			}
			_ => index += 1,
		}
	}
	comments
}

fn skip_quoted(source: &str, start: usize, quote: u8) -> Option<usize> {
	let bytes = source.as_bytes();
	let mut index = start + 1;
	while index < bytes.len() {
		match bytes[index] {
			b'\\' => index += 2,
			value if value == quote => return Some(index + 1),
			_ => index += 1,
		}
	}
	None
}

fn raw_string_end(source: &str, start: usize) -> Option<usize> {
	let bytes = source.as_bytes();
	let raw_start = if bytes[start] == b'b' && bytes.get(start + 1) == Some(&b'r') {
		start + 1
	} else if bytes[start] == b'r' {
		start
	} else {
		return None;
	};
	let mut index = raw_start + 1;
	let mut hashes = 0;
	while bytes.get(index) == Some(&b'#') {
		hashes += 1;
		index += 1;
	}
	if bytes.get(index) != Some(&b'"') {
		return None;
	}
	index += 1;
	while index < bytes.len() {
		if bytes[index] == b'"' {
			let mut close = index + 1;
			let mut matched = 0;
			while matched < hashes && bytes.get(close) == Some(&b'#') {
				matched += 1;
				close += 1;
			}
			if matched == hashes {
				return Some(close);
			}
		}
		index += 1;
	}
	None
}

fn validate_semantics(before: &str, after: &str) -> Result<()> {
	let before = convert_legacy_source(before)?.0;
	let before_file = syn::parse_file(&before).map_err(|error| {
		MigrationError::InvalidMigration(format!(
			"failed to parse legacy migration source: {error}"
		))
	})?;
	let after_file = syn::parse_file(after).map_err(|error| {
		MigrationError::InvalidMigration(format!(
			"failed to parse upgraded migration source: {error}"
		))
	})?;
	let before =
		super::ast_parser::extract_migration_metadata_strict(&before_file, "<app>", "<name>")?;
	let after =
		super::ast_parser::extract_migration_metadata_strict(&after_file, "<app>", "<name>")?;
	if !super::ast_parser::same_migration_semantics(&before, &after) {
		return Err(invalid_shape(
			"migration source upgrade changed migration semantics",
		));
	}
	Ok(())
}

fn invalid_shape(message: &str) -> MigrationError {
	MigrationError::InvalidMigration(message.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn upgrades_nested_literals_and_preserves_unrelated_text() {
		let source = r#"// custom header
use reinhardt::db::migrations::prelude::*;

fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![Operation::CreateTable {
            name: "items".to_string(),
            columns: vec![ColumnDefinition {
                name: "id".to_string(),
                type_definition: FieldType::Integer,
                not_null: true,
                unique: false,
                primary_key: true,
                auto_increment: false,
                default: None,
                generated: None,
                domain: None,
            }],
            constraints: vec![],
            without_rowid: None,
            interleave_in_parent: None,
            partition: None,
        }],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
"#;
		let result = upgrade_source(source).unwrap();
		assert!(result.changed);
		assert!(
			result
				.source
				.starts_with("// reinhardt-migration-source: 1\n")
		);
		assert!(result.source.contains("ColumnDefinition"));
		assert!(result.source.contains("// custom header"));
		assert_eq!(
			upgrade_source(&result.source).unwrap().source,
			result.source
		);
	}

	#[test]
	fn rejects_future_source_format() {
		let error = upgrade_source(
			"// reinhardt-migration-source: 2\nfn migration() -> Migration { Migration::new(\"0001\", \"app\") }\n",
		)
		.unwrap_err();
		assert!(error.to_string().contains("newer Reinhardt tool"));
	}

	#[test]
	fn replaces_legacy_marker_without_duplication() {
		let source = "// reinhardt-migration-source: 0\nfn migration() -> Migration { Migration::new(\"0001\", \"app\") }\n";
		let result = upgrade_source(source).unwrap();
		assert_eq!(result.from_version, Some(0));
		assert_eq!(result.source.matches(MARKER_PREFIX).count(), 1);
		assert!(
			result
				.source
				.starts_with("// reinhardt-migration-source: 1\n")
		);
	}

	#[test]
	fn preserves_shebang_before_source_marker() {
		let source = "#!/usr/bin/env rust-script\nfn migration() -> Migration { Migration::new(\"0001\", \"app\") }\n";
		let result = upgrade_source(source).unwrap();

		assert!(
			result
				.source
				.starts_with("#!/usr/bin/env rust-script\n// reinhardt-migration-source: 1\n")
		);
		assert_eq!(
			upgrade_source(&result.source).unwrap().source,
			result.source
		);
	}

	#[test]
	fn preserves_utf8_bom_shebang_and_legacy_source_offsets() {
		let source = "\u{feff}#!/usr/bin/env rust-script\nfn migration() -> Migration {\n    Migration {\n        name: \"0001_initial\".to_string(),\n        app_label: \"app\".to_string(),\n        operations: vec![],\n        dependencies: vec![],\n        replaces: vec![],\n        atomic: true,\n        initial: None,\n        state_only: false,\n        database_only: false,\n        swappable_dependencies: vec![],\n        optional_dependencies: vec![],\n    }\n}\n";

		let result = upgrade_source(source).unwrap();

		assert!(
			result.source.starts_with(
				"\u{feff}#!/usr/bin/env rust-script\n// reinhardt-migration-source: 1\n"
			)
		);
		assert!(result.source.contains("Migration :: new"));
		assert_eq!(
			upgrade_source(&result.source).unwrap().source,
			result.source
		);
	}

	#[test]
	fn validates_semantics_for_legacy_column_type_aliases() {
		let source = r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![Operation::CreateTable {
            name: "items".to_string(),
            columns: vec![ColumnDefinition {
                name: "id".to_string(),
                field_type: FieldType::Integer,
                not_null: true,
                unique: false,
                primary_key: true,
                auto_increment: false,
                default: None,
                generated: None,
                domain: None,
            }],
            constraints: vec![],
            without_rowid: None,
            interleave_in_parent: None,
            partition: None,
        }],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
"#;

		let result = upgrade_source(source).unwrap();

		assert!(result.changed);
		assert!(result.source.contains("ColumnDefinition :: new"));
	}

	#[test]
	fn rejects_current_marker_with_legacy_struct_literal() {
		let source = r#"// reinhardt-migration-source: 1
fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
"#;
		let error = upgrade_source(source).unwrap_err();
		assert!(
			error
				.to_string()
				.contains("legacy struct-literal syntax remains")
		);
	}

	#[test]
	fn rejects_current_marker_with_unsupported_builder() {
		let source = r#"// reinhardt-migration-source: 1
fn migration() -> Migration {
    Migration::new("0001", "app").customize()
}
"#;
		let error = upgrade_source(source).unwrap_err();
		assert!(error.to_string().contains("builder method 'customize'"));
	}

	#[test]
	fn rejects_unknown_generated_field() {
		let source = r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
        future_field: true,
    }
}
"#;
		let error = upgrade_source(source).unwrap_err();
		assert!(
			error
				.to_string()
				.contains("unsupported field 'future_field'")
		);
	}

	#[test]
	fn rejects_cfg_attributes_on_dependency_tuples() {
		let source = r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![
            #[cfg(feature = "sqlite")]
            ("app", "0000_initial"),
        ],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
"#;
		let error = upgrade_source(source).unwrap_err();

		assert!(error.to_string().contains("attributed tuple"));
	}

	#[test]
	fn rejects_cfg_attributes_on_operations() {
		let source = r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![
            #[cfg(feature = "sqlite")]
            Operation::RunSQL {
                sql: "SELECT 1".to_string(),
                reverse_sql: None,
            },
        ],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
"#;

		let error = upgrade_source(source).unwrap_err();

		assert_eq!(
			error.to_string(),
			"Invalid migration: generated migration field 'operations' contains an attributed operation; conditional entries are unsupported"
		);
	}

	#[test]
	fn rejects_cfg_attributes_on_struct_fields() {
		let source = r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        #[cfg(feature = "atomic")]
        atomic: true,
        ..Default::default()
    }
}
"#;

		let error = upgrade_source(source).unwrap_err();

		assert_eq!(
			error.to_string(),
			"Invalid migration: generated struct literal field 'atomic' contains attributes; conditional fields are unsupported"
		);
	}

	#[rstest]
	#[case::swappable(
		r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        swappable_dependencies: vec![
            #[cfg(feature = "postgres")]
            SwappableDependency::new("AUTH_USER_MODEL", "auth", "User", "0001_initial"),
        ],
        ..Default::default()
    }
}
"#,
		"swappable_dependencies"
	)]
	#[case::optional(
		r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        optional_dependencies: vec![
            #[cfg(feature = "postgres")]
            OptionalDependency::new(
                "gis",
                "0001_initial",
                DependencyCondition::AppInstalled("gis".to_string()),
            ),
        ],
        ..Default::default()
    }
}
"#,
		"optional_dependencies"
	)]
	fn rejects_cfg_attributes_on_conditional_dependency_entries(
		#[case] source: &str,
		#[case] field_name: &str,
	) {
		let error = upgrade_source(source).unwrap_err();

		assert_eq!(
			error.to_string(),
			format!(
				"Invalid migration: generated migration field '{field_name}' contains an attributed dependency; conditional entries are unsupported"
			)
		);
	}

	#[rstest]
	#[case::function_call("migration_defaults()")]
	#[case::qualified_default("std::default::Default::default()")]
	#[case::default_with_argument("Default::default(())")]
	#[case::generic_default("Default::default::<Migration>()")]
	fn rejects_arbitrary_generated_struct_updates(#[case] rest: &str) {
		let source = format!(
			r#"fn migration() -> Migration {{
    Migration {{
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        ..{rest}
    }}
}}
"#
		);
		let error = upgrade_source(&source).unwrap_err();
		assert!(error.to_string().contains("struct-update expression"));
	}

	#[test]
	fn applies_default_semantics_for_omitted_migration_fields() {
		let source = r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        ..Default::default()
    }
}
"#;

		let result = upgrade_source(source).unwrap();
		let file = syn::parse_file(&result.source).unwrap();
		let migration = crate::migrations::ast_parser::extract_migration_metadata_strict(
			&file,
			"app",
			"0001_initial",
		)
		.unwrap();

		assert_eq!(migration.operations, vec![]);
		assert_eq!(migration.dependencies, vec![]);
		assert_eq!(migration.replaces, vec![]);
		assert!(!migration.atomic);
		assert_eq!(migration.initial, None);
		assert!(!migration.state_only);
		assert!(!migration.database_only);
		assert_eq!(migration.swappable_dependencies, vec![]);
		assert_eq!(migration.optional_dependencies, vec![]);
	}

	#[rstest]
	#[case::tweet(
		"tweet",
		1,
		include_str!("../../tests/fixtures/migration_source/v0_1_4/twitter/tweet/0001_initial.rs")
	)]
	#[case::auth(
		"auth",
		4,
		include_str!("../../tests/fixtures/migration_source/v0_1_4/twitter/auth/0001_initial.rs")
	)]
	#[case::dm(
		"dm",
		3,
		include_str!("../../tests/fixtures/migration_source/v0_1_4/twitter/dm/0001_initial.rs")
	)]
	#[case::profile(
		"profile",
		1,
		include_str!("../../tests/fixtures/migration_source/v0_1_4/twitter/profile/0001_initial.rs")
	)]
	fn upgrades_v0_1_4_generated_twitter_migrations(
		#[case] app_label: &str,
		#[case] operation_count: usize,
		#[case] source: &str,
	) {
		let result = upgrade_source(source).unwrap();
		let file = syn::parse_file(&result.source).unwrap();
		let migration = crate::migrations::ast_parser::extract_migration_metadata_strict(
			&file,
			app_label,
			"0001_initial",
		)
		.unwrap();

		assert!(result.changed);
		assert_eq!(result.from_version, None);
		assert_eq!(migration.operations.len(), operation_count);
		assert_eq!(
			upgrade_source(&result.source).unwrap().source,
			result.source
		);
	}

	#[test]
	fn inserts_marker_before_inner_attributes() {
		let source = "#![allow(dead_code)]\nfn migration() -> Migration { Migration::new(\"0001\", \"app\") }\n";
		let result = upgrade_source(source).unwrap();

		assert!(
			result
				.source
				.starts_with("// reinhardt-migration-source: 1\n#![allow(dead_code)]")
		);
	}

	#[test]
	fn ignores_marker_text_inside_raw_strings() {
		let source = r#"fn migration() -> Migration {
    let note = r"// reinhardt-migration-source: 7";
    Migration::new("0001", "app")
}
"#;
		let result = upgrade_source(source).unwrap();

		assert_eq!(result.from_version, None);
		assert!(
			result
				.source
				.starts_with("// reinhardt-migration-source: 1\n")
		);
		assert!(
			result
				.source
				.contains("let note = r\"// reinhardt-migration-source: 7\"")
		);
	}

	#[test]
	fn converts_absolute_framework_paths_without_duplicate_colons() {
		let source = r#"use reinhardt::db::migrations::prelude::*;

fn migration() -> Migration {
    ::reinhardt::db::migrations::Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
"#;
		let result = upgrade_source(source).unwrap();

		assert!(!result.source.contains("::::"));
		assert!(syn::parse_file(&result.source).is_ok());
	}

	#[test]
	fn converts_all_cfg_gated_migration_entrypoints() {
		let source = r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}

#[cfg(feature = "sqlite")]
fn migration() -> Migration {
    Migration {
        name: "0002_backend".to_string(),
        app_label: "app".to_string(),
        operations: vec![],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
"#;
		let result = upgrade_source(source).unwrap();

		assert_eq!(result.source.matches(":: new").count(), 2);
	}

	#[test]
	fn rejects_invalid_later_cfg_gated_migration_entrypoint() {
		let source = r#"// reinhardt-migration-source: 1
#[cfg(feature = "postgres")]
fn migration() -> Migration {
    Migration::new("0001_backend", "app")
}

#[cfg(feature = "sqlite")]
fn migration() -> Migration {
    Migration::new("0001_backend", "app").customize()
}
"#;

		let error = upgrade_source(source).unwrap_err();

		match error {
			MigrationError::InvalidMigration(message) => assert_eq!(
				message,
				"Migration builder method 'customize' is unsupported or malformed"
			),
			other => panic!("expected invalid migration error, got {other}"),
		}
	}

	#[test]
	fn preserves_comments_inside_converted_literals() {
		let source = r#"fn migration() -> Migration {
    Migration {
        name: "0001_initial".to_string(),
        app_label: "app".to_string(),
        operations: vec![
            // Keep this explanation with the migration.
        ],
        dependencies: vec![],
        replaces: vec![],
        atomic: true,
        initial: None,
        state_only: false,
        database_only: false,
        swappable_dependencies: vec![],
        optional_dependencies: vec![],
    }
}
"#;
		let result = upgrade_source(source).unwrap();

		assert!(
			result
				.source
				.contains("// Keep this explanation with the migration.")
		);
	}

	#[test]
	fn ignores_application_types_with_framework_like_names() {
		let application: ExprStruct = syn::parse_str("app::Migration { value: 1 }").unwrap();
		let nested_application: ExprStruct =
			syn::parse_str("crate::migrations::helpers::Migration { value: 1 }").unwrap();
		let framework: ExprStruct =
			syn::parse_str("reinhardt::db::migrations::Migration { value: 1 }").unwrap();
		let framework_module: ExprStruct = syn::parse_str(
			"crate::migrations::operations::InterleaveSpec { parent_table: String::new(), parent_columns: vec![] }",
		)
		.unwrap();

		assert!(!is_target_struct(&application));
		assert!(!is_target_struct(&nested_application));
		assert!(is_target_struct(&framework));
		assert!(is_target_struct(&framework_module));
	}
}
