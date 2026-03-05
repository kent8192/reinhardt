//! Gettext .po file parser
//!
//! This module provides functionality to parse gettext .po files and convert them
//! into MessageCatalog structures.

use crate::MessageCatalog;
use std::io::{BufRead, BufReader};

/// Maximum number of plural forms supported
/// This covers all known natural languages according to CLDR plural rules
const MAX_PLURAL_FORMS: usize = 6;

/// Maximum number of translation entries allowed in a single PO file
const MAX_PO_ENTRIES: usize = 100_000;

/// Maximum PO file size in bytes (10 MB)
const MAX_PO_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Errors that can occur during .po file parsing
#[derive(Debug, thiserror::Error)]
pub enum PoParseError {
	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),
	#[error("Parse error at line {line}: {message}")]
	ParseError { line: usize, message: String },
	#[error("Invalid format: {0}")]
	InvalidFormat(String),
	#[error("Invalid plural index: {0} (maximum is 5)")]
	InvalidPluralIndex(usize),
	#[error("PO file too large: {0} bytes (maximum is {MAX_PO_FILE_SIZE} bytes)")]
	FileTooLarge(u64),
	#[error("Too many entries: {0} (maximum is {MAX_PO_ENTRIES})")]
	TooManyEntries(usize),
}

/// Entry in a .po file
#[derive(Debug, Clone, Default)]
struct PoEntry {
	msgctxt: Option<String>,
	msgid: String,
	msgid_plural: Option<String>,
	msgstr: Vec<String>,
}

impl PoEntry {
	fn new() -> Self {
		Self::default()
	}

	fn is_empty(&self) -> bool {
		self.msgid.is_empty()
	}
}

/// Parse a .po file from a reader
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use reinhardt_i18n::po_parser::parse_po_file;
///
/// let file = File::open("locale/fr/LC_MESSAGES/messages.po").unwrap();
/// let catalog = parse_po_file(file, "fr").unwrap();
/// ```
pub fn parse_po_file<R: std::io::Read>(
	reader: R,
	locale: &str,
) -> Result<MessageCatalog, PoParseError> {
	// Limit reading to MAX_PO_FILE_SIZE to prevent resource exhaustion
	let limited_reader = reader.take(MAX_PO_FILE_SIZE + 1);
	let buf_reader = BufReader::new(limited_reader);
	let mut catalog = MessageCatalog::new(locale);
	let mut current_entry = PoEntry::new();
	let mut current_msgstr_index: Option<usize> = None;
	let mut entry_count: usize = 0;
	let mut bytes_read: u64 = 0;

	for line in buf_reader.lines() {
		let line = line?;
		// Track total bytes read (line length + newline)
		bytes_read += line.len() as u64 + 1;
		if bytes_read > MAX_PO_FILE_SIZE {
			return Err(PoParseError::FileTooLarge(bytes_read));
		}

		let trimmed = line.trim();

		// Skip empty lines and comments
		if trimmed.is_empty() || trimmed.starts_with('#') {
			continue;
		}

		// Parse msgctxt
		if let Some(value) = parse_keyword(trimmed, "msgctxt") {
			if !current_entry.is_empty() {
				entry_count += 1;
				if entry_count > MAX_PO_ENTRIES {
					return Err(PoParseError::TooManyEntries(entry_count));
				}
				add_entry_to_catalog(&mut catalog, &current_entry);
				current_entry = PoEntry::new();
			}
			current_entry.msgctxt = Some(unescape_string(&value));
			current_msgstr_index = None;
		}
		// Parse msgid
		else if let Some(value) = parse_keyword(trimmed, "msgid") {
			if !current_entry.is_empty() {
				entry_count += 1;
				if entry_count > MAX_PO_ENTRIES {
					return Err(PoParseError::TooManyEntries(entry_count));
				}
				add_entry_to_catalog(&mut catalog, &current_entry);
				current_entry = PoEntry::new();
			}
			current_entry.msgid = unescape_string(&value);
			current_msgstr_index = None;
		}
		// Parse msgid_plural
		else if let Some(value) = parse_keyword(trimmed, "msgid_plural") {
			current_entry.msgid_plural = Some(unescape_string(&value));
			current_msgstr_index = None;
		}
		// Parse msgstr[n]
		else if let Some((index, value)) = parse_indexed_msgstr(trimmed) {
			// Validate plural index to prevent memory exhaustion attacks
			if index >= MAX_PLURAL_FORMS {
				return Err(PoParseError::InvalidPluralIndex(index));
			}
			let value = unescape_string(&value);
			// Ensure we have enough space in the msgstr vector
			while current_entry.msgstr.len() <= index {
				current_entry.msgstr.push(String::new());
			}
			current_entry.msgstr[index] = value;
			current_msgstr_index = Some(index);
		}
		// Parse msgstr
		else if let Some(value) = parse_keyword(trimmed, "msgstr") {
			current_entry.msgstr = vec![unescape_string(&value)];
			current_msgstr_index = Some(0);
		}
		// Parse continuation string (quoted string on its own line)
		else if trimmed.starts_with('"') && trimmed.ends_with('"') {
			let value = unescape_string(&trimmed[1..trimmed.len() - 1]);
			if let Some(index) = current_msgstr_index {
				if let Some(existing) = current_entry.msgstr.get_mut(index) {
					existing.push_str(&value);
				}
			} else {
				// Continuation of msgid or msgid_plural
				if current_entry.msgid_plural.is_some() {
					if let Some(plural) = &mut current_entry.msgid_plural {
						plural.push_str(&value);
					}
				} else if !current_entry.msgid.is_empty() {
					current_entry.msgid.push_str(&value);
				}
			}
		}
	}

	// Add the last entry
	if !current_entry.is_empty() {
		entry_count += 1;
		if entry_count > MAX_PO_ENTRIES {
			return Err(PoParseError::TooManyEntries(entry_count));
		}
		add_entry_to_catalog(&mut catalog, &current_entry);
	}

	Ok(catalog)
}

/// Parse a keyword and its value from a line
fn parse_keyword(line: &str, keyword: &str) -> Option<String> {
	if !line.starts_with(keyword) {
		return None;
	}

	let rest = line[keyword.len()..].trim();
	if !rest.starts_with('"') || !rest.ends_with('"') {
		return None;
	}

	Some(rest[1..rest.len() - 1].to_string())
}

/// Parse indexed msgstr (e.g., `msgstr[0]`, `msgstr[1]`)
fn parse_indexed_msgstr(line: &str) -> Option<(usize, String)> {
	if !line.starts_with("msgstr[") {
		return None;
	}

	let close_bracket = line.find(']')?;
	let index_str = &line[7..close_bracket];
	let index: usize = index_str.parse().ok()?;

	let rest = line[close_bracket + 1..].trim();
	if !rest.starts_with('"') || !rest.ends_with('"') {
		return None;
	}

	Some((index, rest[1..rest.len() - 1].to_string()))
}

/// Unescape a string (handle \n, \t, \", \\)
fn unescape_string(s: &str) -> String {
	let mut result = String::new();
	let mut chars = s.chars();

	while let Some(ch) = chars.next() {
		if ch == '\\' {
			if let Some(next_ch) = chars.next() {
				match next_ch {
					'n' => result.push('\n'),
					't' => result.push('\t'),
					'r' => result.push('\r'),
					'"' => result.push('"'),
					'\\' => result.push('\\'),
					_ => {
						result.push('\\');
						result.push(next_ch);
					}
				}
			} else {
				result.push('\\');
			}
		} else {
			result.push(ch);
		}
	}

	result
}

/// Add a parsed entry to the catalog
fn add_entry_to_catalog(catalog: &mut MessageCatalog, entry: &PoEntry) {
	// Skip header entry (empty msgid)
	if entry.msgid.is_empty() {
		return;
	}

	// Handle contextual translations
	if let Some(context) = &entry.msgctxt {
		if entry.msgid_plural.is_some() {
			// Contextual plural
			catalog.add_context_plural(
				context,
				&entry.msgid,
				"",
				entry.msgstr.iter().map(|s| s.as_str()).collect(),
			);
		} else {
			// Contextual simple translation
			if let Some(translation) = entry.msgstr.first() {
				catalog.add_context_str(context, &entry.msgid, translation);
			}
		}
	}
	// Handle regular translations
	else if entry.msgid_plural.is_some() {
		// Plural translation
		catalog.add_plural(entry.msgid.clone(), entry.msgstr.clone());
	} else {
		// Simple translation
		if let Some(translation) = entry.msgstr.first() {
			catalog.add_translation(&entry.msgid, translation);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_parse_simple_translation() {
		let po_content = r#"
msgid "Hello"
msgstr "Bonjour"

msgid "Goodbye"
msgstr "Au revoir"
"#;

		let catalog = parse_po_file(po_content.as_bytes(), "fr").unwrap();
		assert_eq!(catalog.get("Hello"), Some(&"Bonjour".to_string()));
		assert_eq!(catalog.get("Goodbye"), Some(&"Au revoir".to_string()));
	}

	#[test]
	fn test_parse_plural_translation() {
		let po_content = r#"
msgid "item"
msgid_plural "items"
msgstr[0] "article"
msgstr[1] "articles"
"#;

		let catalog = parse_po_file(po_content.as_bytes(), "fr").unwrap();
		assert_eq!(catalog.get_plural("item", 1), Some(&"article".to_string()));
		assert_eq!(catalog.get_plural("item", 5), Some(&"articles".to_string()));
	}

	#[test]
	fn test_parse_contextual_translation() {
		let po_content = r#"
msgctxt "menu"
msgid "File"
msgstr "Fichier"

msgctxt "verb"
msgid "File"
msgstr "Classer"
"#;

		let catalog = parse_po_file(po_content.as_bytes(), "fr").unwrap();
		assert_eq!(
			catalog.get_context("menu", "File"),
			Some(&"Fichier".to_string())
		);
		assert_eq!(
			catalog.get_context("verb", "File"),
			Some(&"Classer".to_string())
		);
	}

	#[test]
	fn test_parse_multiline_string() {
		let po_content = r#"
msgid "This is a long "
"message that spans "
"multiple lines"
msgstr "Ceci est un long "
"message qui s'étend "
"sur plusieurs lignes"
"#;

		let catalog = parse_po_file(po_content.as_bytes(), "fr").unwrap();
		assert_eq!(
			catalog.get("This is a long message that spans multiple lines"),
			Some(&"Ceci est un long message qui s'étend sur plusieurs lignes".to_string())
		);
	}

	#[test]
	fn test_parse_escape_sequences() {
		let po_content = r#"
msgid "Line 1\nLine 2\tTabbed"
msgstr "Ligne 1\nLigne 2\tTabulée"
"#;

		let catalog = parse_po_file(po_content.as_bytes(), "fr").unwrap();
		assert_eq!(
			catalog.get("Line 1\nLine 2\tTabbed"),
			Some(&"Ligne 1\nLigne 2\tTabulée".to_string())
		);
	}

	#[test]
	fn test_parse_with_comments() {
		let po_content = r#"
# Translator comment
#. Extracted comment
#: reference.py:10
#, fuzzy
msgid "Hello"
msgstr "Bonjour"
"#;

		let catalog = parse_po_file(po_content.as_bytes(), "fr").unwrap();
		assert_eq!(catalog.get("Hello"), Some(&"Bonjour".to_string()));
	}

	#[test]
	fn test_parse_empty_file() {
		let po_content = "";
		let catalog = parse_po_file(po_content.as_bytes(), "fr").unwrap();
		assert_eq!(catalog.get("Hello"), None);
	}

	#[test]
	fn test_unescape_string() {
		assert_eq!(unescape_string("Hello\\nWorld"), "Hello\nWorld");
		assert_eq!(unescape_string("Tab\\there"), "Tab\there");
		assert_eq!(unescape_string("Quote\\\"here"), "Quote\"here");
		assert_eq!(unescape_string("Backslash\\\\here"), "Backslash\\here");
	}

	/// Test that huge plural index is rejected to prevent memory exhaustion
	#[rstest]
	#[case(4294967295_usize)] // usize::MAX on 32-bit, large on 64-bit
	#[case(1000000)] // Large but not maximum
	#[case(100)] // Exceeds MAX_PLURAL_FORMS (6)
	#[case(6)] // Exactly MAX_PLURAL_FORMS (boundary)
	fn test_parse_rejects_huge_plural_index(#[case] index: usize) {
		let po_content = format!(
			r#"
msgid "test"
msgstr[{}] "value"
"#,
			index
		);

		let result = parse_po_file(po_content.as_bytes(), "fr");
		assert!(result.is_err());

		// Verify the error type
		match result {
			Err(PoParseError::InvalidPluralIndex(err_index)) => {
				assert_eq!(err_index, index);
			}
			_ => panic!("Expected InvalidPluralIndex error"),
		}
	}

	/// Test that too many entries are rejected
	#[rstest]
	fn test_parse_rejects_too_many_entries() {
		// Arrange: generate a PO file with more than MAX_PO_ENTRIES entries
		let mut po_content = String::new();
		for i in 0..=MAX_PO_ENTRIES {
			po_content.push_str(&format!(
				"msgid \"msg{}\"\nmsgstr \"translation{}\"\n\n",
				i, i
			));
		}

		// Act
		let result = parse_po_file(po_content.as_bytes(), "fr");

		// Assert
		assert!(result.is_err());
		match result {
			Err(PoParseError::TooManyEntries(_)) => {}
			_ => panic!("Expected TooManyEntries error"),
		}
	}

	/// Test that valid plural indices are accepted (within MAX_PLURAL_FORMS)
	#[rstest]
	#[case(0)]
	#[case(1)]
	#[case(2)]
	#[case(5)] // MAX_PLURAL_FORMS - 1
	fn test_parse_accepts_valid_plural_index(#[case] index: usize) {
		let po_content = format!(
			r#"
msgid "item"
msgid_plural "items"
msgstr[{}] "value"
"#,
			index
		);

		let result = parse_po_file(po_content.as_bytes(), "fr");
		assert!(result.is_ok());
	}
}
