//! i18n management commands
//!
//! Commands for message extraction and compilation

use crate::{
	BaseCommand, CommandArgument, CommandContext, CommandError, CommandOption, CommandResult,
};
use async_trait::async_trait;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
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
	let mut result = String::new();
	let mut chars = s.chars();

	while let Some(character) = chars.next() {
		if character != '\\' {
			result.push(character);
			continue;
		}

		match chars.next() {
			Some('n') => result.push('\n'),
			Some('r') => result.push('\r'),
			Some('t') => result.push('\t'),
			Some('\\') => result.push('\\'),
			Some('"') => result.push('"'),
			Some(other) => {
				result.push('\\');
				result.push(other);
			}
			None => result.push('\\'),
		}
	}

	result
}

fn parse_po_quoted_value(value: &str) -> Option<String> {
	let value = value.trim();
	let value = value.strip_prefix('"')?.strip_suffix('"')?;
	Some(unescape_po_string(value))
}

fn parse_po_entries(content: &str) -> Vec<(String, String)> {
	#[derive(Clone, Copy)]
	enum Field {
		Msgid,
		Msgstr,
	}

	let mut entries = Vec::new();
	let mut current_msgid = None;
	let mut current_msgstr = String::new();
	let mut current_field = None;

	for line in content.lines() {
		if let Some(value) = line.strip_prefix("msgid ") {
			if let Some(msgid) = current_msgid.take() {
				entries.push((msgid, std::mem::take(&mut current_msgstr)));
			}
			current_msgid = parse_po_quoted_value(value);
			current_field = Some(Field::Msgid);
		} else if let Some(value) = line.strip_prefix("msgstr ") {
			current_msgstr = parse_po_quoted_value(value).unwrap_or_default();
			current_field = Some(Field::Msgstr);
		} else if line.starts_with('"') {
			let Some(value) = parse_po_quoted_value(line) else {
				continue;
			};
			match current_field {
				Some(Field::Msgid) => {
					if let Some(msgid) = &mut current_msgid {
						msgid.push_str(&value);
					}
				}
				Some(Field::Msgstr) => current_msgstr.push_str(&value),
				None => {}
			}
		}
	}

	if let Some(msgid) = current_msgid {
		entries.push((msgid, current_msgstr));
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

		// Process each locale
		for locale in &normalized_locales {
			ctx.info(&format!("Processing locale: {}", locale));

			let locale_dir = PathBuf::from("locale").join(locale).join("LC_MESSAGES");
			std::fs::create_dir_all(&locale_dir).map_err(|e| {
				CommandError::ExecutionError(format!("Failed to create locale directory: {}", e))
			})?;

			let po_file = locale_dir.join("reinhardt.po");

			// Check if PO file exists
			let exists = po_file.exists();

			if exists {
				ctx.verbose(&format!("Updating existing PO file: {}", po_file.display()));
			} else {
				ctx.verbose(&format!("Creating new PO file: {}", po_file.display()));
			}

			// Extract translatable strings from source files
			let mut messages = Self::extract_messages(".", &extensions, ctx)?;

			// Remove duplicates and sort
			messages.sort_by(|a, b| a.msgid.cmp(&b.msgid));
			messages.dedup_by(|a, b| a.msgid == b.msgid);

			ctx.verbose(&format!(
				"Found {} unique translatable strings",
				messages.len()
			));

			// Create or update PO file
			if exists {
				Self::update_po_file(&po_file, &messages, ctx)?;
			} else {
				Self::create_po_file_with_messages(&po_file, locale, &messages)?;
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
		let locale_dir = PathBuf::from(base_path).join("locale");

		if !locale_dir.exists() {
			return Ok(vec![]);
		}

		let mut locales = Vec::new();

		for entry in std::fs::read_dir(locale_dir).map_err(CommandError::IoError)? {
			let entry = entry.map_err(CommandError::IoError)?;
			let path = entry.path();

			if path.is_dir()
				&& let Some(name) = path.file_name()
				&& let Some(name_str) = name.to_str()
			{
				locales.push(name_str.to_string());
			}
		}

		Ok(locales)
	}

	fn extract_messages(
		base_path: &str,
		extensions: &[String],
		ctx: &CommandContext,
	) -> CommandResult<Vec<TranslatableMessage>> {
		let mut messages = Vec::new();
		let mut seen_msgids = HashSet::new();

		// Regex patterns for different gettext functions
		// Matches: gettext!("message"), _("message"), t!("message")
		static I18N_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
			vec![
				Regex::new(r#"gettext!\s*\(\s*"([^"]+)"\s*\)"#).unwrap(),
				Regex::new(r#"_\s*\(\s*"([^"]+)"\s*\)"#).unwrap(),
				Regex::new(r#"t!\s*\(\s*"([^"]+)"\s*\)"#).unwrap(),
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

			// Extract messages using patterns
			for pattern in patterns {
				for cap in pattern.captures_iter(&content) {
					if let Some(msgid) = cap.get(1) {
						let msgid_str = msgid.as_str().to_string();

						if !seen_msgids.contains(&msgid_str) {
							seen_msgids.insert(msgid_str.clone());
							messages.push(TranslatableMessage {
								msgid: msgid_str,
								locations: vec![path.display().to_string()],
							});
						}
					}
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

		// Extract existing translations (simple approach: keep msgstr values)
		let existing_translations: HashMap<String, String> = parse_po_entries(&existing_content)
			.into_iter()
			.filter(|(msgid, _)| !msgid.is_empty())
			.collect();

		ctx.verbose(&format!(
			"Merging {} new messages with {} existing translations",
			messages.len(),
			existing_translations.len()
		));

		// Create new content with merged messages
		let header = Self::extract_po_header(&existing_content);
		let mut new_content = header;

		for msg in messages {
			new_content.push_str(&format!("\nmsgid \"{}\"\n", escape_po_string(&msg.msgid)));

			// Use existing translation if available, otherwise empty
			if let Some(existing_msgstr) = existing_translations.get(&msg.msgid) {
				new_content.push_str(&format!(
					"msgstr \"{}\"\n",
					escape_po_string(existing_msgstr)
				));
			} else {
				new_content.push_str("msgstr \"\"\n");
			}
		}

		std::fs::write(path, new_content)
			.map_err(|e| CommandError::ExecutionError(format!("Failed to write PO file: {}", e)))?;

		Ok(())
	}

	fn extract_po_header(content: &str) -> String {
		if let Some(header_start) = content.find("msgid \"\"") {
			let next_msgid = content[header_start + "msgid \"\"".len()..]
				.find("\nmsgid ")
				.map(|offset| header_start + "msgid \"\"".len() + offset + 1);
			let header_end = next_msgid.unwrap_or(content.len());
			return format!("{}\n", content[..header_end].trim_end_matches('\n'));
		}

		if let Some(pos) = content.find("\nmsgid ")
			&& pos > 0
		{
			return format!("{}\n", content[..pos].trim_end_matches('\n'));
		}

		String::new()
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
			Self::find_all_locales(".")?
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
			let locale_dir = PathBuf::from("locale").join(locale).join("LC_MESSAGES");
			let po_file = locale_dir.join("reinhardt.po");
			let mo_file = locale_dir.join("reinhardt.mo");

			if !po_file.exists() {
				ctx.warning(&format!(
					"PO file not found for locale {}: {}",
					locale,
					po_file.display()
				));
				continue;
			}

			ctx.verbose(&format!("Compiling {}", po_file.display()));

			// Parse .po file and compile to .mo format
			match Self::compile_po_to_mo(&po_file, &mo_file, _use_fuzzy) {
				Ok(count) => {
					ctx.verbose(&format!("Compiled {} messages", count));
				}
				Err(e) => {
					ctx.warning(&format!("Failed to compile {}: {}", po_file.display(), e));
					continue;
				}
			}

			compiled_count += 1;
			ctx.success(&format!("Compiled {}", locale));
		}

		ctx.success(&format!(
			"Successfully compiled {} locale(s)",
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
		Ok(parse_po_entries(content)
			.into_iter()
			.filter(|(msgid, msgstr)| !msgid.is_empty() && !msgstr.is_empty())
			.collect())
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

	fn find_all_locales(base_path: &str) -> CommandResult<Vec<String>> {
		let locale_dir = PathBuf::from(base_path).join("locale");

		if !locale_dir.exists() {
			return Ok(vec![]);
		}

		let mut locales = Vec::new();

		for entry in std::fs::read_dir(locale_dir).map_err(CommandError::IoError)? {
			let entry = entry.map_err(CommandError::IoError)?;
			let path = entry.path();

			if path.is_dir() {
				// Check if LC_MESSAGES/reinhardt.po exists
				let po_file = path.join("LC_MESSAGES").join("reinhardt.po");
				if po_file.exists()
					&& let Some(name) = path.file_name()
					&& let Some(name_str) = name.to_str()
				{
					locales.push(name_str.to_string());
				}
			}
		}

		Ok(locales)
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
			"gettext!(\"Shared\"); _(\"Underscore\"); t!(\"Macro\");",
		)
		.expect("Rust source");
		fs::write(source_dir.join("later.rs"), "gettext!(\"Shared\");").expect("duplicate source");
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
			&["rs".to_string(), "html".to_string()],
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
					msgid: "Underscore".to_string(),
					locations: vec![source_dir.join("first.rs").display().to_string()],
				},
				TranslatableMessage {
					msgid: "Macro".to_string(),
					locations: vec![source_dir.join("first.rs").display().to_string()],
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
				"# Translator note\nmsgid \"\"\nmsgstr \"\"\n\"Language: ja\\n\"\n\n",
				"msgid \"Hello\"\nmsgstr \"こんにちは\"\n\n",
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
