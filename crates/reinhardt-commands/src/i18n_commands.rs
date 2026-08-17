//! i18n management commands
//!
//! Commands for message extraction and compilation. `makemessages` updates the
//! project catalog and catalogs registered through
//! [`reinhardt_apps::register_app_locale!`]. Rust extraction parses translation
//! macros so interpolated, multiline, escaped, and trailing-comma invocations
//! retain their literal message identifiers.

use crate::{
	BaseCommand, CommandArgument, CommandContext, CommandError, CommandOption, CommandResult,
};
use async_trait::async_trait;
use proc_macro2::{TokenStream, TokenTree};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use syn::parse::{Parse, ParseStream};
use syn::visit::Visit;
use walkdir::WalkDir;

/// Escapes a string for use as a PO file field value.
///
/// PO format requires special characters to be escaped:
/// - Backslash → `\\`
/// - Double quote → `\"`
/// - Newline → `\n`
/// - Carriage return → `\r`
/// - Tab → `\t`
fn escape_po_string(s: &str) -> String {
	s.replace('\\', "\\\\")
		.replace('"', "\\\"")
		.replace('\n', "\\n")
		.replace('\r', "\\r")
		.replace('\t', "\\t")
}

fn unescape_po_string(s: &str) -> String {
	let mut value = String::with_capacity(s.len());
	let mut chars = s.chars();
	while let Some(character) = chars.next() {
		if character != '\\' {
			value.push(character);
			continue;
		}

		match chars.next() {
			Some('\\') => value.push('\\'),
			Some('"') => value.push('"'),
			Some('n') => value.push('\n'),
			Some('r') => value.push('\r'),
			Some('t') => value.push('\t'),
			Some(character) => {
				value.push('\\');
				value.push(character);
			}
			None => value.push('\\'),
		}
	}
	value
}

#[derive(Default)]
struct PoEntry {
	context: Option<String>,
	msgid: Option<String>,
	plural: Option<String>,
	translations: Vec<(usize, String)>,
}

#[derive(Clone, Copy)]
enum PoField {
	Context,
	Msgid,
	Plural,
	Translation(usize),
}

fn po_quoted_value(line: &str) -> Option<String> {
	let value = line.trim().strip_prefix('"')?.strip_suffix('"')?;
	Some(unescape_po_string(value))
}

fn parse_po_entries(content: &str) -> Vec<PoEntry> {
	let mut entries = Vec::new();
	let mut entry = PoEntry::default();
	let mut field = None;

	for line in content.lines() {
		let trimmed = line.trim();
		if trimmed.is_empty() {
			if entry.msgid.is_some() {
				entries.push(entry);
				entry = PoEntry::default();
			}
			field = None;
			continue;
		}

		let directive = if let Some(value) = trimmed.strip_prefix("msgctxt ") {
			Some((PoField::Context, value))
		} else if let Some(value) = trimmed.strip_prefix("msgid_plural ") {
			Some((PoField::Plural, value))
		} else if let Some(value) = trimmed.strip_prefix("msgid ") {
			if entry.msgid.is_some() {
				entries.push(entry);
				entry = PoEntry::default();
			}
			Some((PoField::Msgid, value))
		} else if let Some(value) = trimmed.strip_prefix("msgstr ") {
			Some((PoField::Translation(0), value))
		} else if let Some(indexed) = trimmed.strip_prefix("msgstr[") {
			indexed.split_once("] ").and_then(|(index, value)| {
				index
					.parse()
					.ok()
					.map(|index| (PoField::Translation(index), value))
			})
		} else {
			None
		};

		if let Some((next_field, value)) = directive {
			field = Some(next_field);
			if let Some(value) = po_quoted_value(value) {
				match next_field {
					PoField::Context => entry.context = Some(value),
					PoField::Msgid => entry.msgid = Some(value),
					PoField::Plural => entry.plural = Some(value),
					PoField::Translation(index) => entry.translations.push((index, value)),
				}
			}
			continue;
		}

		if let Some(value) = po_quoted_value(trimmed)
			&& let Some(current_field) = field
		{
			match current_field {
				PoField::Context => entry.context.get_or_insert_default().push_str(&value),
				PoField::Msgid => entry.msgid.get_or_insert_default().push_str(&value),
				PoField::Plural => entry.plural.get_or_insert_default().push_str(&value),
				PoField::Translation(index) => {
					if let Some((_, translation)) = entry
						.translations
						.iter_mut()
						.find(|(entry_index, _)| *entry_index == index)
					{
						translation.push_str(&value);
					}
				}
			}
		}
	}

	if entry.msgid.is_some() {
		entries.push(entry);
	}
	entries
}

fn is_excluded_i18n_path(path: &Path) -> bool {
	path.components().any(|component| {
		matches!(
			component.as_os_str().to_str(),
			Some("target" | ".git" | "locale")
		)
	})
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranslatableMessage {
	msgid: String,
	locations: Vec<String>,
}

struct FirstStringArgument {
	value: syn::LitStr,
}

impl Parse for FirstStringArgument {
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
		let value = input.parse()?;
		let _remaining: proc_macro2::TokenStream = input.parse()?;
		Ok(Self { value })
	}
}

#[derive(Default)]
struct RustMessageVisitor {
	messages: Vec<String>,
}

impl<'ast> Visit<'ast> for RustMessageVisitor {
	fn visit_macro(&mut self, node: &'ast syn::Macro) {
		let is_translation_macro =
			node.path.segments.last().is_some_and(|segment| {
				matches!(segment.ident.to_string().as_str(), "t" | "gettext")
			});

		if is_translation_macro
			&& let Ok(argument) = syn::parse2::<FirstStringArgument>(node.tokens.clone())
		{
			self.messages.push(argument.value.value());
		}

		Self::visit_macro_tokens(node.tokens.clone(), &mut self.messages);

		syn::visit::visit_macro(self, node);
	}
}

impl RustMessageVisitor {
	fn visit_macro_tokens(tokens: TokenStream, messages: &mut Vec<String>) {
		let tokens: Vec<_> = tokens.into_iter().collect();
		for window in tokens.windows(3) {
			if let [
				TokenTree::Ident(ident),
				TokenTree::Punct(bang),
				TokenTree::Group(arguments),
			] = window && bang.as_char() == '!'
				&& matches!(ident.to_string().as_str(), "t" | "gettext")
				&& let Ok(argument) = syn::parse2::<FirstStringArgument>(arguments.stream())
			{
				messages.push(argument.value.value());
			}
		}

		for token in tokens {
			if let TokenTree::Group(group) = token {
				Self::visit_macro_tokens(group.stream(), messages);
			}
		}
	}
}

/// Make messages command - extract translatable strings
pub struct MakeMessagesCommand;

#[async_trait]
impl BaseCommand for MakeMessagesCommand {
	fn name(&self) -> &str {
		"makemessages"
	}

	fn description(&self) -> &str {
		"Extract translatable strings from source files and create/update .po files"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::option(
				Some('l'),
				"locale",
				"Locale(s) to create/update (e.g., en_us, ja_jp)",
			)
			.multi(),
			CommandOption::flag(Some('a'), "all", "Update all locale files"),
			CommandOption::option(
				None,
				"extension",
				"File extensions to examine (default: html,txt,py,rs)",
			)
			.multi(),
			CommandOption::option(None, "symlinks", "Follow symlinks"),
			CommandOption::option(None, "ignore", "Patterns to ignore").multi(),
			CommandOption::flag(None, "no-default-ignore", "Don't ignore default patterns"),
			CommandOption::flag(None, "no-wrap", "Don't break long message lines"),
			CommandOption::flag(None, "no-location", "Don't include location comments"),
			CommandOption::option(
				None,
				"add-location",
				"Location comments style (full/file/never)",
			)
			.with_default("full"),
			CommandOption::flag(None, "keep-pot", "Keep .pot file after processing"),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		ctx.info("Extracting translatable strings...");

		// Get locale list
		let locales = if ctx.has_option("all") {
			// Find all existing locale directories
			Self::find_all_locales(".")?
		} else if let Some(locales) = ctx.option_values("locale") {
			locales
		} else {
			return Err(CommandError::InvalidArguments(
				"Please specify --locale or --all".to_string(),
			));
		};

		if locales.is_empty() {
			return Err(CommandError::InvalidArguments(
				"No locales specified".to_string(),
			));
		}

		// Validate and normalize locales
		let mut normalized_locales = Vec::new();
		for locale in &locales {
			Self::validate_locale(locale)?;
			normalized_locales.push(Self::normalize_locale(locale));
		}

		ctx.verbose(&format!(
			"Processing locales: {}",
			normalized_locales.join(", ")
		));

		// Get file extensions
		let extensions = ctx.option_values("extension").unwrap_or_else(|| {
			vec![
				"html".to_string(),
				"txt".to_string(),
				"py".to_string(),
				"rs".to_string(),
			]
		});

		ctx.verbose(&format!("Scanning extensions: {}", extensions.join(", ")));

		// Get ignore patterns
		let _ignore_patterns = if ctx.has_option("no-default-ignore") {
			ctx.option_values("ignore").unwrap_or_default()
		} else {
			let mut patterns = vec![
				".*".to_string(),
				"*~".to_string(),
				"*.pyc".to_string(),
				"target/*".to_string(),
			];
			if let Some(user_patterns) = ctx.option_values("ignore") {
				patterns.extend(user_patterns);
			}
			patterns
		};

		let mut messages = Self::extract_messages(".", &extensions, ctx)?;
		messages.sort_by(|a, b| a.msgid.cmp(&b.msgid));
		messages.dedup_by(|a, b| a.msgid == b.msgid);

		ctx.verbose(&format!(
			"Found {} unique translatable strings",
			messages.len()
		));

		// Process each locale
		for locale in &normalized_locales {
			ctx.info(&format!("Processing locale: {}", locale));

			for po_file in Self::catalog_paths(locale) {
				if let Some(locale_dir) = po_file.parent() {
					std::fs::create_dir_all(locale_dir).map_err(|error| {
						CommandError::ExecutionError(format!(
							"Failed to create locale directory: {error}"
						))
					})?;
				}

				if po_file.exists() {
					ctx.verbose(&format!("Updating existing PO file: {}", po_file.display()));
					Self::update_po_file(&po_file, &messages, ctx)?;
				} else {
					ctx.verbose(&format!("Creating new PO file: {}", po_file.display()));
					Self::create_po_file_with_messages(&po_file, locale, &messages)?;
				}
			}

			ctx.success(&format!("Processed locale: {}", locale));
		}

		ctx.success(&format!(
			"Successfully processed {} locale(s)",
			normalized_locales.len()
		));
		Ok(())
	}
}

impl MakeMessagesCommand {
	fn catalog_paths(locale: &str) -> Vec<PathBuf> {
		let mut paths = vec![
			PathBuf::from("locale")
				.join(locale)
				.join("LC_MESSAGES")
				.join("reinhardt.po"),
		];

		for config in reinhardt_apps::get_app_locales() {
			let messages_dir = PathBuf::from(config.locale_dir)
				.join(locale)
				.join("LC_MESSAGES");
			let django_catalog = messages_dir.join("django.po");
			let messages_catalog = messages_dir.join("messages.po");
			let catalogs = match (django_catalog.exists(), messages_catalog.exists()) {
				(true, true) => vec![django_catalog, messages_catalog],
				(true, false) => vec![django_catalog],
				(false, _) => vec![messages_catalog],
			};
			for catalog in catalogs {
				if !paths.contains(&catalog) {
					paths.push(catalog);
				}
			}
		}

		paths
	}

	fn validate_locale(locale: &str) -> CommandResult<()> {
		// Validate locale format
		if locale.is_empty() {
			return Err(CommandError::InvalidArguments("Empty locale".to_string()));
		}

		// Check for invalid characters (only alphanumeric, underscore, and hyphen allowed)
		if !locale
			.chars()
			.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
		{
			return Err(CommandError::InvalidArguments(format!(
				"Invalid locale format: {}. Only lowercase letters, numbers, underscores, and hyphens are allowed (e.g., en_us, en-US, ja_jp)",
				locale
			)));
		}

		// Check for invalid patterns
		if locale.starts_with('_')
			|| locale.ends_with('_')
			|| locale.starts_with('-')
			|| locale.ends_with('-')
		{
			return Err(CommandError::InvalidArguments(format!(
				"Locale cannot start or end with underscore or hyphen: {}",
				locale
			)));
		}

		// Check if uppercase (Django convention is lowercase)
		if locale.chars().any(|c| c.is_uppercase()) {
			return Err(CommandError::InvalidArguments(format!(
				"Locale should be lowercase (e.g., en_us or en-us, not EN_US or EN-US): {}",
				locale
			)));
		}

		Ok(())
	}

	fn normalize_locale(locale: &str) -> String {
		// Normalize locale: convert hyphens to underscores and to lowercase
		// This ensures filesystem compatibility (e.g., en-US -> en_us)
		locale.replace('-', "_").to_lowercase()
	}

	fn find_all_locales(base_path: &str) -> CommandResult<Vec<String>> {
		let mut locale_roots = vec![PathBuf::from(base_path).join("locale")];
		locale_roots.extend(
			reinhardt_apps::get_app_locales()
				.into_iter()
				.map(|config| PathBuf::from(config.locale_dir)),
		);

		let mut locales = HashSet::new();
		for locale_root in locale_roots {
			if !locale_root.exists() {
				continue;
			}
			for entry in std::fs::read_dir(locale_root).map_err(CommandError::IoError)? {
				let entry = entry.map_err(CommandError::IoError)?;
				let path = entry.path();
				if path.is_dir()
					&& let Some(name) = path.file_name()
					&& let Some(name_str) = name.to_str()
				{
					locales.insert(name_str.to_string());
				}
			}
		}

		let mut locales: Vec<_> = locales.into_iter().collect();
		locales.sort();
		Ok(locales)
	}

	fn extract_messages(
		base_path: &str,
		extensions: &[String],
		ctx: &CommandContext,
	) -> CommandResult<Vec<TranslatableMessage>> {
		let mut messages = Vec::new();
		let mut seen_msgids = HashSet::new();

		// Regex patterns for non-Rust translation syntax.
		static I18N_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
			vec![
				Regex::new(r#"_\s*\(\s*"([^"]+)"\s*\)"#).unwrap(),
				// Template tags: {% trans "message" %}
				Regex::new(r#"\{%\s*trans\s+"([^"]+)"\s*%\}"#).unwrap(),
			]
		});
		let patterns = &*I18N_PATTERNS;

		for entry in WalkDir::new(base_path)
			.sort_by_file_name()
			.into_iter()
			.filter_entry(|entry| !is_excluded_i18n_path(entry.path()))
			.filter_map(|e| e.ok())
		{
			let path = entry.path();

			// Skip if not a file
			if !path.is_file() {
				continue;
			}

			// Check extension
			if let Some(ext) = path.extension() {
				let ext_str = ext.to_string_lossy();
				if !extensions.iter().any(|e| e == &*ext_str) {
					continue;
				}
			} else {
				continue;
			}

			ctx.verbose(&format!("Scanning: {}", path.display()));

			// Read file content
			let content = match std::fs::read_to_string(path) {
				Ok(c) => c,
				Err(_) => continue,
			};

			let extracted = if path.extension().is_some_and(|extension| extension == "rs") {
				let Ok(file) = syn::parse_file(&content) else {
					continue;
				};
				let mut visitor = RustMessageVisitor::default();
				visitor.visit_file(&file);
				visitor.messages
			} else {
				patterns
					.iter()
					.flat_map(|pattern| pattern.captures_iter(&content))
					.filter_map(|capture| capture.get(1).map(|value| value.as_str().to_string()))
					.collect()
			};

			for msgid in extracted {
				if msgid.is_empty() {
					continue;
				}
				if seen_msgids.insert(msgid.clone()) {
					messages.push(TranslatableMessage {
						msgid,
						locations: vec![path.display().to_string()],
					});
				}
			}
		}

		Ok(messages)
	}

	fn update_po_file(
		path: &Path,
		messages: &[TranslatableMessage],
		ctx: &CommandContext,
	) -> CommandResult<()> {
		// Read existing PO file
		let existing_content = std::fs::read_to_string(path)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to read PO file: {}", e)))?;

		let existing_msgids: HashSet<_> = parse_po_entries(&existing_content)
			.into_iter()
			.filter(|entry| entry.context.is_none())
			.filter_map(|entry| entry.msgid)
			.collect();

		ctx.verbose(&format!(
			"Merging {} new messages with {} existing entries",
			messages.len(),
			existing_msgids.len()
		));

		let mut new_content = existing_content.trim_end().to_string();

		for msg in messages {
			if existing_msgids.contains(&msg.msgid) {
				continue;
			}
			new_content.push_str(&format!("\nmsgid \"{}\"\n", escape_po_string(&msg.msgid)));
			new_content.push_str("msgstr \"\"\n");
		}

		std::fs::write(path, new_content)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to write PO file: {}", e)))?;

		Ok(())
	}

	fn create_po_file_with_messages(
		path: &Path,
		locale: &str,
		messages: &[TranslatableMessage],
	) -> CommandResult<()> {
		let mut content = format!(
			r#"# SOME DESCRIPTIVE TITLE.
# Copyright (C) YEAR THE PACKAGE'S COPYRIGHT HOLDER
# This file is distributed under the same license as the PACKAGE package.
# FIRST AUTHOR <EMAIL@ADDRESS>, YEAR.
#
msgid ""
msgstr ""
"Project-Id-Version: PACKAGE VERSION\n"
"Report-Msgid-Bugs-To: \n"
"POT-Creation-Date: 2025-01-01 00:00+0000\n"
"PO-Revision-Date: YEAR-MO-DA HO:MI+ZONE\n"
"Last-Translator: FULL NAME <EMAIL@ADDRESS>\n"
"Language-Team: LANGUAGE <LL@li.org>\n"
"Language: {}\n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Plural-Forms: nplurals=2; plural=(n != 1);\n"
"#,
			locale
		);

		for msg in messages {
			content.push_str(&format!("\nmsgid \"{}\"\n", escape_po_string(&msg.msgid)));
			content.push_str("msgstr \"\"\n");
		}

		std::fs::write(path, content)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to write PO file: {}", e)))?;

		Ok(())
	}
}

/// Compile messages command - compile .po files to .mo files
pub struct CompileMessagesCommand;

#[async_trait]
impl BaseCommand for CompileMessagesCommand {
	fn name(&self) -> &str {
		"compilemessages"
	}

	fn description(&self) -> &str {
		"Compile .po message files to .mo binary format"
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::option(Some('l'), "locale", "Locale(s) to compile").multi(),
			CommandOption::option(None, "exclude", "Locale(s) to exclude from compilation").multi(),
			CommandOption::option(None, "ignore", "Directory patterns to ignore").multi(),
			CommandOption::flag(Some('f'), "use-fuzzy", "Include fuzzy translations"),
		]
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		ctx.info("Compiling message files...");

		// Get locales to compile
		let locales = if let Some(specified) = ctx.option_values("locale") {
			specified
		} else {
			MakeMessagesCommand::find_all_locales(".")?
		};

		if locales.is_empty() {
			ctx.warning("No locales found to compile");
			return Ok(());
		}

		// Get exclusions
		let excluded = ctx.option_values("exclude").unwrap_or_default();

		// Filter out excluded locales
		let locales_to_compile: Vec<String> = locales
			.into_iter()
			.filter(|l| !excluded.contains(l))
			.collect();

		if locales_to_compile.is_empty() {
			ctx.warning("All locales were excluded");
			return Ok(());
		}

		ctx.verbose(&format!(
			"Compiling locales: {}",
			locales_to_compile.join(", ")
		));

		let _use_fuzzy = ctx.has_option("use-fuzzy");

		let mut compiled_count = 0;

		// Compile each locale
		for locale in &locales_to_compile {
			for po_file in MakeMessagesCommand::catalog_paths(locale) {
				let mo_file = po_file.with_extension("mo");

				if !po_file.exists() {
					ctx.warning(&format!(
						"PO file not found for locale {}: {}",
						locale,
						po_file.display()
					));
					continue;
				}

				ctx.verbose(&format!("Compiling {}", po_file.display()));

				match Self::compile_po_to_mo(&po_file, &mo_file, _use_fuzzy) {
					Ok(count) => ctx.verbose(&format!("Compiled {} messages", count)),
					Err(e) => {
						ctx.warning(&format!("Failed to compile {}: {}", po_file.display(), e));
						continue;
					}
				}

				compiled_count += 1;
				ctx.success(&format!("Compiled {}", po_file.display()));
			}
		}

		ctx.success(&format!(
			"Successfully compiled {} catalog(s)",
			compiled_count
		));
		Ok(())
	}
}

impl CompileMessagesCommand {
	fn compile_po_to_mo(po_file: &Path, mo_file: &Path, _use_fuzzy: bool) -> CommandResult<usize> {
		// Read .po file
		let content = std::fs::read_to_string(po_file)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to read PO file: {}", e)))?;

		// Parse messages from PO file
		let messages = Self::parse_po_file(&content)?;

		// Write .mo file (simplified binary format)
		let mo_content = Self::generate_mo_content(&messages)?;

		// Ensure parent directory exists
		if let Some(parent) = mo_file.parent() {
			std::fs::create_dir_all(parent).map_err(|e| {
				CommandError::ExecutionError(format!("Failed to create directory: {}", e))
			})?;
		}

		std::fs::write(mo_file, mo_content)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to write MO file: {}", e)))?;

		Ok(messages.len())
	}

	fn parse_po_file(content: &str) -> CommandResult<Vec<(String, String)>> {
		let mut messages = Vec::new();
		for mut entry in parse_po_entries(content) {
			let Some(mut msgid) = entry.msgid.take() else {
				continue;
			};
			if msgid.is_empty() {
				continue;
			}
			if let Some(context) = entry.context {
				msgid = format!("{context}\u{4}{msgid}");
			}
			if let Some(plural) = entry.plural {
				msgid.push('\0');
				msgid.push_str(&plural);
			}

			entry.translations.sort_by_key(|(index, _)| *index);
			let msgstr = entry
				.translations
				.into_iter()
				.map(|(_, translation)| translation)
				.collect::<Vec<_>>()
				.join("\0");
			if !msgstr.is_empty() {
				messages.push((msgid, msgstr));
			}
		}

		Ok(messages)
	}

	fn generate_mo_content(messages: &[(String, String)]) -> CommandResult<Vec<u8>> {
		// Simplified MO file format based on GNU gettext specification
		let mut content = Vec::new();

		// Magic number for MO files (little-endian)
		content.extend_from_slice(&0x950412de_u32.to_le_bytes());

		// Version (0)
		content.extend_from_slice(&0_u32.to_le_bytes());

		// Number of strings
		let n_strings: u32 = messages.len().try_into().map_err(|_| {
			CommandError::ExecutionError("Too many messages for MO format".to_string())
		})?;
		content.extend_from_slice(&n_strings.to_le_bytes());

		// Offset of table with original strings (after header)
		let orig_table_offset: u32 = 28;
		content.extend_from_slice(&orig_table_offset.to_le_bytes());

		// Offset of table with translated strings
		let trans_table_offset = orig_table_offset
			.checked_add(n_strings.checked_mul(8).ok_or_else(|| {
				CommandError::ExecutionError("Integer overflow in MO table offset".to_string())
			})?)
			.ok_or_else(|| {
				CommandError::ExecutionError("Integer overflow in MO header".to_string())
			})?;
		content.extend_from_slice(&trans_table_offset.to_le_bytes());

		// Hash table size (0 = no hash table in this simplified version)
		content.extend_from_slice(&0_u32.to_le_bytes());

		// Hash table offset (0)
		content.extend_from_slice(&0_u32.to_le_bytes());

		// Calculate string data offset
		let string_data_offset = trans_table_offset
			.checked_add(n_strings.checked_mul(8).ok_or_else(|| {
				CommandError::ExecutionError(
					"Integer overflow in MO string data offset".to_string(),
				)
			})?)
			.ok_or_else(|| {
				CommandError::ExecutionError(
					"Integer overflow computing string data offset".to_string(),
				)
			})?;
		let mut current_offset = string_data_offset;

		// Write original strings table
		let mut orig_strings = Vec::new();
		for (msgid, _) in messages {
			let msgid_bytes = msgid.as_bytes();
			let msgid_len: u32 = msgid_bytes.len().try_into().map_err(|_| {
				CommandError::ExecutionError("msgid too long for MO format".to_string())
			})?;
			content.extend_from_slice(&msgid_len.to_le_bytes());
			content.extend_from_slice(&current_offset.to_le_bytes());
			orig_strings.push(msgid_bytes);
			current_offset = current_offset
				.checked_add(msgid_len)
				.and_then(|v| v.checked_add(1))
				.ok_or_else(|| {
					CommandError::ExecutionError("Integer overflow in MO string offset".to_string())
				})?;
		}

		// Write translated strings table
		let mut trans_strings = Vec::new();
		for (_, msgstr) in messages {
			let msgstr_bytes = msgstr.as_bytes();
			let msgstr_len: u32 = msgstr_bytes.len().try_into().map_err(|_| {
				CommandError::ExecutionError("msgstr too long for MO format".to_string())
			})?;
			content.extend_from_slice(&msgstr_len.to_le_bytes());
			content.extend_from_slice(&current_offset.to_le_bytes());
			trans_strings.push(msgstr_bytes);
			current_offset = current_offset
				.checked_add(msgstr_len)
				.and_then(|v| v.checked_add(1))
				.ok_or_else(|| {
					CommandError::ExecutionError(
						"Integer overflow in MO translated offset".to_string(),
					)
				})?;
		}

		// Write original strings
		for s in orig_strings {
			content.extend_from_slice(s);
			content.push(0); // null terminator
		}

		// Write translated strings
		for s in trans_strings {
			content.extend_from_slice(s);
			content.push(0); // null terminator
		}

		Ok(content)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::fs;
	use tempfile::TempDir;

	fn message(msgid: &str) -> TranslatableMessage {
		TranslatableMessage {
			msgid: msgid.to_string(),
			locations: vec![],
		}
	}

	fn little_endian_u32(bytes: &[u8], offset: usize) -> u32 {
		u32::from_le_bytes(
			bytes[offset..offset + 4]
				.try_into()
				.expect("four-byte field"),
		)
	}

	#[test]
	fn extraction_recognizes_all_syntaxes_keeps_first_location_and_ignores_excluded_inputs() {
		// Arrange
		let temp_dir = TempDir::new().expect("temporary project");
		let source_dir = temp_dir.path().join("src");
		fs::create_dir_all(&source_dir).expect("source directory");
		fs::write(
			source_dir.join("first.rs"),
			"fn messages() { gettext!(\"Shared\"); t!(\"Macro\"); }",
		)
		.expect("Rust source");
		fs::write(source_dir.join("later.rs"), "gettext!(\"Shared\");").expect("duplicate source");
		fs::write(source_dir.join("underscore.py"), "_(\"Underscore\")").expect("Python source");
		let template_dir = temp_dir.path().join("templates");
		fs::create_dir_all(&template_dir).expect("template directory");
		fs::write(template_dir.join("page.html"), "{% trans \"Template\" %}")
			.expect("HTML template");
		fs::create_dir_all(temp_dir.path().join("target")).expect("target directory");
		fs::write(
			temp_dir.path().join("target/ignored.rs"),
			"gettext!(\"Target\");",
		)
		.expect("target source");
		fs::create_dir_all(temp_dir.path().join(".git")).expect("Git directory");
		fs::write(
			temp_dir.path().join(".git/ignored.rs"),
			"gettext!(\"Git\");",
		)
		.expect("Git source");
		fs::create_dir_all(temp_dir.path().join("locale")).expect("locale directory");
		fs::write(
			temp_dir.path().join("locale/ignored.rs"),
			"gettext!(\"Locale\");",
		)
		.expect("locale source");
		fs::write(
			temp_dir.path().join("ignored.md"),
			"gettext!(\"Extension\");",
		)
		.expect("ignored extension");
		fs::write(source_dir.join("invalid.rs"), [0xff, 0xfe]).expect("invalid UTF-8 source");
		let ctx = CommandContext::new(vec![]);

		// Act
		let messages = MakeMessagesCommand::extract_messages(
			temp_dir.path().to_str().expect("UTF-8 temporary path"),
			&["rs".to_string(), "html".to_string(), "py".to_string()],
			&ctx,
		)
		.expect("extraction succeeds");

		// Assert
		assert_eq!(
			messages,
			vec![
				TranslatableMessage {
					msgid: "Shared".to_string(),
					locations: vec![source_dir.join("first.rs").display().to_string()],
				},
				TranslatableMessage {
					msgid: "Macro".to_string(),
					locations: vec![source_dir.join("first.rs").display().to_string()],
				},
				TranslatableMessage {
					msgid: "Underscore".to_string(),
					locations: vec![source_dir.join("underscore.py").display().to_string()],
				},
				TranslatableMessage {
					msgid: "Template".to_string(),
					locations: vec![template_dir.join("page.html").display().to_string()],
				},
			]
		);
	}

	#[test]
	fn extraction_exclusion_uses_platform_path_components() {
		// Arrange and Act
		let target_path = Path::new("workspace").join("target").join("message.rs");
		let git_path = Path::new("workspace").join(".git").join("message.rs");
		let locale_path = Path::new("workspace").join("locale").join("message.rs");
		let source_path = Path::new("workspace").join("src").join("message.rs");

		// Assert
		assert!(is_excluded_i18n_path(&target_path));
		assert!(is_excluded_i18n_path(&git_path));
		assert!(is_excluded_i18n_path(&locale_path));
		assert!(!is_excluded_i18n_path(&source_path));
	}

	#[test]
	fn po_merge_preserves_header_and_translation_while_escaping_new_entries() {
		// Arrange
		let temp_dir = TempDir::new().expect("temporary locale");
		let po_file = temp_dir.path().join("reinhardt.po");
		fs::write(
			&po_file,
			concat!(
				"msgid \"\"\nmsgstr \"\"\n\"Language: ja\\n\"\n\n",
				"msgid \"Hello\"\nmsgstr \"こんにちは\"\n\n",
				"msgid \"Stale\"\nmsgstr \"古い\"\n"
			),
		)
		.expect("existing PO file");
		let ctx = CommandContext::new(vec![]);

		// Act
		MakeMessagesCommand::update_po_file(
			&po_file,
			&[
				message("Hello"),
				message("New \"message\"\\path\nnext\tcolumn\rend"),
			],
			&ctx,
		)
		.expect("PO merge");

		// Assert
		assert_eq!(
			fs::read_to_string(po_file).expect("merged PO file"),
			concat!(
				"msgid \"\"\nmsgstr \"\"\n\"Language: ja\\n\"\n\n",
				"msgid \"Hello\"\nmsgstr \"こんにちは\"\n\n",
				"msgid \"Stale\"\nmsgstr \"古い\"\n",
				"msgid \"New \\\"message\\\"\\\\path\\nnext\\tcolumn\\rend\"\nmsgstr \"\"\n"
			)
		);
	}

	#[test]
	fn po_merge_finds_the_first_non_header_msgid_without_a_blank_separator() {
		// Arrange
		let temp_dir = TempDir::new().expect("temporary locale");
		let po_file = temp_dir.path().join("reinhardt.po");
		fs::write(
			&po_file,
			concat!(
				"# Translator note\nmsgid \"\"\nmsgstr \"\"\n\"Language: ja\\n\"\n",
				"msgid \"Hello\"\nmsgstr \"こんにちは\"\n",
				"msgid \"Stale\"\nmsgstr \"古い\"\n"
			),
		)
		.expect("existing PO file");
		let ctx = CommandContext::new(vec![]);

		// Act
		MakeMessagesCommand::update_po_file(&po_file, &[message("Hello"), message("New")], &ctx)
			.expect("PO merge");

		// Assert
		assert_eq!(
			fs::read_to_string(po_file).expect("merged PO file"),
			concat!(
				"# Translator note\nmsgid \"\"\nmsgstr \"\"\n\"Language: ja\\n\"\n",
				"msgid \"Hello\"\nmsgstr \"こんにちは\"\n",
				"msgid \"Stale\"\nmsgstr \"古い\"\n",
				"msgid \"New\"\nmsgstr \"\"\n"
			)
		);
	}

	#[test]
	fn po_merge_ignores_header_like_text_inside_comments() {
		// Arrange
		let temp_dir = TempDir::new().expect("temporary locale");
		let po_file = temp_dir.path().join("reinhardt.po");
		fs::write(
			&po_file,
			concat!(
				"# See msgid \"\" below for the catalog header.\n",
				"# Translator note\nmsgid \"\"\nmsgstr \"\"\n\"Language: ja\\n\"\n",
				"msgid \"Hello\"\nmsgstr \"こんにちは\"\n",
				"msgid \"Stale\"\nmsgstr \"古い\"\n"
			),
		)
		.expect("existing PO file");
		let ctx = CommandContext::new(vec![]);

		// Act
		MakeMessagesCommand::update_po_file(&po_file, &[message("Hello"), message("New")], &ctx)
			.expect("PO merge");

		// Assert
		assert_eq!(
			fs::read_to_string(po_file).expect("merged PO file"),
			concat!(
				"# See msgid \"\" below for the catalog header.\n",
				"# Translator note\nmsgid \"\"\nmsgstr \"\"\n\"Language: ja\\n\"\n",
				"msgid \"Hello\"\nmsgstr \"こんにちは\"\n",
				"msgid \"Stale\"\nmsgstr \"古い\"\n",
				"msgid \"New\"\nmsgstr \"\"\n"
			)
		);
	}

	#[test]
	fn po_parser_handles_headers_adjacent_entries_and_escaped_fields() {
		// Arrange
		let content = concat!(
			"msgid \"\"\nmsgstr \"\"\n\"Language: fr\\n\"\n",
			"msgid \"Hello\"\nmsgstr \"Bonjour\"\n",
			"msgid \"Untranslated\"\nmsgstr \"\"\n",
			"msgid \"Quote \\\"and\\\" slash \\\\ tab\\tline\\n\"\n",
			"msgstr \"Citation \\\"et\\\" barre \\\\ onglet\\tligne\\n\"\n",
			"msgid \"Multi \"\n\"line\\q\"\n",
			"msgstr \"Translated \"\n\"value\\q\"\n"
		);

		// Act
		let messages = CompileMessagesCommand::parse_po_file(content).expect("parse PO file");

		// Assert
		assert_eq!(
			messages,
			vec![
				("Hello".to_string(), "Bonjour".to_string()),
				(
					"Quote \"and\" slash \\ tab\tline\n".to_string(),
					"Citation \"et\" barre \\ onglet\tligne\n".to_string(),
				),
				(
					"Multi line\\q".to_string(),
					"Translated value\\q".to_string(),
				),
			]
		);
	}

	#[test]
	fn mo_content_contains_complete_header_tables_offsets_and_terminated_strings() {
		// Arrange
		let messages = vec![
			("Hello".to_string(), "Bonjour".to_string()),
			("Save".to_string(), "Enregistrer".to_string()),
		];

		// Act
		let bytes = CompileMessagesCommand::generate_mo_content(&messages).expect("MO content");

		// Assert
		assert_eq!(little_endian_u32(&bytes, 0), 0x9504_12de);
		assert_eq!(little_endian_u32(&bytes, 4), 0);
		assert_eq!(little_endian_u32(&bytes, 8), 2);
		assert_eq!(little_endian_u32(&bytes, 12), 28);
		assert_eq!(little_endian_u32(&bytes, 16), 44);
		assert_eq!(little_endian_u32(&bytes, 20), 0);
		assert_eq!(little_endian_u32(&bytes, 24), 0);

		let originals = [("Hello", 60_u32), ("Save", 66_u32)];
		let translations = [("Bonjour", 71_u32), ("Enregistrer", 79_u32)];
		for (index, (value, offset)) in originals.into_iter().enumerate() {
			let table_offset = 28 + index * 8;
			assert_eq!(little_endian_u32(&bytes, table_offset), value.len() as u32);
			assert_eq!(little_endian_u32(&bytes, table_offset + 4), offset);
			let offset = offset as usize;
			assert_eq!(&bytes[offset..offset + value.len()], value.as_bytes());
			assert_eq!(bytes[offset + value.len()], 0);
		}
		for (index, (value, offset)) in translations.into_iter().enumerate() {
			let table_offset = 44 + index * 8;
			assert_eq!(little_endian_u32(&bytes, table_offset), value.len() as u32);
			assert_eq!(little_endian_u32(&bytes, table_offset + 4), offset);
			let offset = offset as usize;
			assert_eq!(&bytes[offset..offset + value.len()], value.as_bytes());
			assert_eq!(bytes[offset + value.len()], 0);
		}
	}

	#[test]
	fn po_and_mo_filesystem_failures_keep_operation_context() {
		// Arrange
		let temp_dir = TempDir::new().expect("temporary filesystem");
		let blocking_file = temp_dir.path().join("blocking-file");
		fs::write(&blocking_file, "not a directory").expect("blocking file");
		let ctx = CommandContext::new(vec![]);

		// Act and Assert
		let po_read_error = MakeMessagesCommand::update_po_file(
			&blocking_file.join("reinhardt.po"),
			&[message("Hello")],
			&ctx,
		)
		.expect_err("PO read through a file parent fails");
		assert!(
			po_read_error
				.to_string()
				.starts_with("Execution error: Failed to read PO file:")
		);

		let mo_directory_error = CompileMessagesCommand::compile_po_to_mo(
			&temp_dir.path().join("source.po"),
			&blocking_file.join("LC_MESSAGES/reinhardt.mo"),
			false,
		)
		.expect_err("MO directory through a file parent fails");
		assert!(
			mo_directory_error
				.to_string()
				.starts_with("Execution error: Failed to read PO file:")
		);

		fs::write(
			temp_dir.path().join("source.po"),
			"msgid \"Hello\"\nmsgstr \"Bonjour\"\n",
		)
		.expect("source PO file");
		let mo_directory_error = CompileMessagesCommand::compile_po_to_mo(
			&temp_dir.path().join("source.po"),
			&blocking_file.join("LC_MESSAGES/reinhardt.mo"),
			false,
		)
		.expect_err("MO directory through a file parent fails");
		assert!(
			mo_directory_error
				.to_string()
				.starts_with("Execution error: Failed to create directory:")
		);

		let mo_destination = temp_dir.path().join("destination");
		fs::create_dir_all(&mo_destination).expect("MO destination directory");
		let mo_write_error = CompileMessagesCommand::compile_po_to_mo(
			&temp_dir.path().join("source.po"),
			&mo_destination,
			false,
		)
		.expect_err("writing MO over a directory fails");
		assert!(
			mo_write_error
				.to_string()
				.starts_with("Execution error: Failed to write MO file:")
		);
	}

	#[cfg(unix)]
	#[test]
	fn po_write_failure_keeps_operation_context_when_permissions_prevent_writing() {
		use std::os::unix::fs::PermissionsExt;

		struct PermissionsGuard {
			path: PathBuf,
			permissions: std::fs::Permissions,
		}

		impl Drop for PermissionsGuard {
			fn drop(&mut self) {
				let _ = fs::set_permissions(&self.path, self.permissions.clone());
			}
		}

		// Arrange
		let temp_dir = TempDir::new().expect("temporary locale");
		let po_file = temp_dir.path().join("reinhardt.po");
		fs::write(&po_file, "msgid \"Hello\"\nmsgstr \"Bonjour\"\n").expect("existing PO file");
		let permissions = fs::metadata(&po_file).expect("PO metadata").permissions();
		let _permissions_guard = PermissionsGuard {
			path: po_file.clone(),
			permissions,
		};
		fs::set_permissions(&po_file, std::fs::Permissions::from_mode(0o444))
			.expect("read-only PO file");
		let ctx = CommandContext::new(vec![]);

		// Act
		let error = MakeMessagesCommand::update_po_file(&po_file, &[message("Hello")], &ctx)
			.expect_err("read-only PO file rejects writes");

		// Assert
		assert!(
			error
				.to_string()
				.starts_with("Execution error: Failed to write PO file:")
		);
	}

	#[rstest]
	#[case("plain text", "plain text")]
	#[case(r#"He said "hello""#, r#"He said \"hello\""#)]
	#[case("path\\to\\file", "path\\\\to\\\\file")]
	#[case("line1\nline2", "line1\\nline2")]
	#[case("col1\tcol2", "col1\\tcol2")]
	#[case("cr\rend", "cr\\rend")]
	#[case("mixed\n\"value\"", "mixed\\n\\\"value\\\"")]
	fn test_escape_po_string(#[case] input: &str, #[case] expected: &str) {
		// Arrange: input string with special PO format characters
		// Act
		let result = escape_po_string(input);
		// Assert: all special characters are properly escaped
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case("plain text")]
	#[case(r#"He said "hello""#)]
	#[case("path\\to\\file")]
	#[case("line1\nline2")]
	#[case("col1\tcol2")]
	#[case("cr\rend")]
	#[case("mixed\n\"value\"")]
	fn test_po_string_escape_round_trip(#[case] input: &str) {
		assert_eq!(unescape_po_string(&escape_po_string(input)), input);
	}

	#[test]
	fn test_parse_po_file_supports_wrapped_plural_and_contextual_entries() {
		let content = r#"
msgctxt "button"
msgid ""
"Save "
"file"
msgstr ""
"Enregistrer "
"le fichier"

msgid "apple"
msgid_plural "apples"
msgstr[0] "pomme"
msgstr[1] "pommes"
"#;

		let messages = CompileMessagesCommand::parse_po_file(content).unwrap();

		assert_eq!(
			messages,
			vec![
				(
					"button\u{4}Save file".to_string(),
					"Enregistrer le fichier".to_string(),
				),
				("apple\0apples".to_string(), "pomme\0pommes".to_string()),
			]
		);
	}

	#[rstest]
	fn test_generate_mo_content_empty_messages_succeeds() {
		// Arrange
		let messages: Vec<(String, String)> = vec![];
		// Act
		let result = CompileMessagesCommand::generate_mo_content(&messages);
		// Assert: empty message list produces valid MO content (just header)
		assert!(result.is_ok());
		let data = result.unwrap();
		// MO magic number at offset 0 (little-endian 0x950412de)
		assert_eq!(&data[0..4], &0x950412de_u32.to_le_bytes());
	}

	#[rstest]
	fn test_generate_mo_content_single_message_no_overflow() {
		// Arrange: a single translated message
		let messages = vec![("Hello".to_string(), "Bonjour".to_string())];
		// Act
		let result = CompileMessagesCommand::generate_mo_content(&messages);
		// Assert: compiles without arithmetic errors
		assert!(result.is_ok());
	}
}
