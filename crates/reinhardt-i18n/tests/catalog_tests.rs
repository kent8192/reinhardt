//! Message catalog tests
//!
//! Tests based on Django's i18n catalog functionality

use reinhardt_i18n::{CatalogLoader, I18nError, MessageCatalog};
use rstest::rstest;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;
use unic_langid::LanguageIdentifier;

#[test]
#[serial(i18n)]
fn test_catalog_simple_message_integration() {
	let locale: LanguageIdentifier = "en-US".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	catalog.add("hello".to_string(), "Hello!".to_string());
	assert_eq!(catalog.get("hello"), Some(&"Hello!".to_string()));
	assert_eq!(catalog.get("nonexistent"), None);
}

#[test]
#[serial(i18n)]
fn test_catalog_plural_english() {
	let locale: LanguageIdentifier = "en-US".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	catalog.add_plural(
		"item".to_string(),
		vec!["One item".to_string(), "Many items".to_string()],
	);

	assert_eq!(catalog.get_plural("item", 1), Some(&"One item".to_string()));
	assert_eq!(
		catalog.get_plural("item", 0),
		Some(&"Many items".to_string())
	);
	assert_eq!(
		catalog.get_plural("item", 5),
		Some(&"Many items".to_string())
	);
	assert_eq!(
		catalog.get_plural("item", 2),
		Some(&"Many items".to_string())
	);
}

#[test]
#[serial(i18n)]
fn test_catalog_plural_french() {
	let locale: LanguageIdentifier = "fr-FR".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	// French: 0 and 1 are singular (index 0), > 1 is plural (index 1)
	catalog.add_plural(
		"jour".to_string(),
		vec!["jour".to_string(), "jours".to_string()],
	);

	assert_eq!(catalog.get_plural("jour", 0), Some(&"jour".to_string()));
	assert_eq!(catalog.get_plural("jour", 1), Some(&"jour".to_string()));
	assert_eq!(catalog.get_plural("jour", 2), Some(&"jours".to_string()));
	assert_eq!(catalog.get_plural("jour", 5), Some(&"jours".to_string()));
}

#[test]
#[serial(i18n)]
fn test_catalog_plural_japanese() {
	let locale: LanguageIdentifier = "ja-JP".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	// Japanese has no plural forms - always uses index 0
	catalog.add_plural("item".to_string(), vec!["アイテム".to_string()]);

	assert_eq!(catalog.get_plural("item", 0), Some(&"アイテム".to_string()));
	assert_eq!(catalog.get_plural("item", 1), Some(&"アイテム".to_string()));
	assert_eq!(
		catalog.get_plural("item", 100),
		Some(&"アイテム".to_string())
	);
}

#[test]
#[serial(i18n)]
fn test_catalog_context_integration() {
	let locale: LanguageIdentifier = "ja-JP".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	catalog.add_context(
		"menu".to_string(),
		"File".to_string(),
		"ファイル".to_string(),
	);
	catalog.add_context(
		"verb".to_string(),
		"File".to_string(),
		"提出する".to_string(),
	);

	assert_eq!(
		catalog.get_context("menu", "File"),
		Some(&"ファイル".to_string())
	);
	assert_eq!(
		catalog.get_context("verb", "File"),
		Some(&"提出する".to_string())
	);
	assert_eq!(catalog.get_context("other", "File"), None);
}

#[test]
#[serial(i18n)]
fn test_catalog_context_plural() {
	let locale: LanguageIdentifier = "de-DE".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	// Use explicit add_context_plural() for context-qualified entries
	catalog.add_context_plural(
		"email",
		"message",
		"messages",
		vec!["Nachricht", "Nachrichten"],
	);

	assert_eq!(
		catalog.get_context_plural("email", "message", 1),
		Some(&"Nachricht".to_string())
	);
	assert_eq!(
		catalog.get_context_plural("email", "message", 5),
		Some(&"Nachrichten".to_string())
	);
}

#[test]
#[serial(i18n)]
fn test_catalog_multiple_contexts() {
	let locale: LanguageIdentifier = "en-US".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	catalog.add_context(
		"food".to_string(),
		"Apple".to_string(),
		"Food apple".to_string(),
	);
	catalog.add_context(
		"company".to_string(),
		"Apple".to_string(),
		"Apple Inc.".to_string(),
	);
	catalog.add_context(
		"color".to_string(),
		"Apple".to_string(),
		"Apple color".to_string(),
	);

	assert_eq!(
		catalog.get_context("food", "Apple"),
		Some(&"Food apple".to_string())
	);
	assert_eq!(
		catalog.get_context("company", "Apple"),
		Some(&"Apple Inc.".to_string())
	);
	assert_eq!(
		catalog.get_context("color", "Apple"),
		Some(&"Apple color".to_string())
	);
}

#[test]
#[serial(i18n)]
fn test_catalog_empty_message() {
	let locale: LanguageIdentifier = "en-US".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	catalog.add("".to_string(), "".to_string());
	assert_eq!(catalog.get(""), Some(&"".to_string()));
}

#[test]
#[serial(i18n)]
fn test_catalog_special_characters() {
	let locale: LanguageIdentifier = "en-US".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	catalog.add("Hello\nWorld".to_string(), "Bonjour\nMonde".to_string());
	catalog.add("Tab\tSeparated".to_string(), "Tab\tSéparé".to_string());
	catalog.add("Quote\"Test".to_string(), "Citation\"Test".to_string());

	assert_eq!(
		catalog.get("Hello\nWorld"),
		Some(&"Bonjour\nMonde".to_string())
	);
	assert_eq!(
		catalog.get("Tab\tSeparated"),
		Some(&"Tab\tSéparé".to_string())
	);
	assert_eq!(
		catalog.get("Quote\"Test"),
		Some(&"Citation\"Test".to_string())
	);
}

#[test]
#[serial(i18n)]
fn test_catalog_locale() {
	let locale: LanguageIdentifier = "fr-FR".parse().unwrap();
	let catalog = MessageCatalog::new(&locale.to_string());

	assert_eq!(catalog.locale(), "fr-FR");
}

#[test]
#[serial(i18n)]
fn test_catalog_loader_loads_po_file() {
	let temp_dir = TempDir::new().unwrap();

	// Create locale directory structure
	let locale_dir = temp_dir.path().join("fr-FR").join("LC_MESSAGES");
	std::fs::create_dir_all(&locale_dir).unwrap();

	// Create a sample .po file
	let po_content = r#"
msgid "Hello"
msgstr "Bonjour"

msgid "Goodbye"
msgstr "Au revoir"

msgid "item"
msgid_plural "items"
msgstr[0] "article"
msgstr[1] "articles"
"#;
	std::fs::write(locale_dir.join("django.po"), po_content).unwrap();

	// Create a catalog loader
	let loader = CatalogLoader::new(temp_dir.path());

	let locale: LanguageIdentifier = "fr-FR".parse().unwrap();

	// Load the catalog from the .po file
	let catalog = loader.load(&locale.to_string()).unwrap();

	// Verify translations were loaded
	assert_eq!(catalog.get("Hello"), Some(&"Bonjour".to_string()));
	assert_eq!(catalog.get("Goodbye"), Some(&"Au revoir".to_string()));

	// Verify plural translations were loaded
	assert_eq!(catalog.get_plural("item", 1), Some(&"article".to_string()));
	assert_eq!(catalog.get_plural("item", 5), Some(&"articles".to_string()));

	// Cleanup is automatic with TempDir
}

#[test]
#[serial(i18n)]
fn test_catalog_loader_loads_messages_po_file() {
	let temp_dir = TempDir::new().unwrap();

	// Create locale directory structure
	let locale_dir = temp_dir.path().join("ja").join("LC_MESSAGES");
	std::fs::create_dir_all(&locale_dir).unwrap();

	// Create a sample messages.po file (fallback when django.po doesn't exist)
	let po_content = r#"
msgid "Welcome"
msgstr "ようこそ"

msgid "Thank you"
msgstr "ありがとう"
"#;
	std::fs::write(locale_dir.join("messages.po"), po_content).unwrap();

	// Create a catalog loader
	let loader = CatalogLoader::new(temp_dir.path());

	// Load the catalog from the messages.po file
	let catalog = loader.load("ja").unwrap();

	// Verify translations were loaded
	assert_eq!(catalog.get("Welcome"), Some(&"ようこそ".to_string()));
	assert_eq!(catalog.get("Thank you"), Some(&"ありがとう".to_string()));

	// Cleanup is automatic with TempDir
}

#[test]
#[serial(i18n)]
fn test_catalog_loader_not_found_returns_error() {
	let temp_dir = TempDir::new().unwrap();

	// Create a catalog loader with a temporary directory as base path
	let loader = CatalogLoader::new(temp_dir.path());

	let locale: LanguageIdentifier = "xx-XX".parse().unwrap();

	// CatalogLoader::load() returns an error when no .po file is found
	let result = loader.load(&locale.to_string());

	assert!(result.is_err());
	assert!(matches!(result, Err(I18nError::CatalogNotFound(_))));

	// Cleanup is automatic with TempDir
}

#[test]
#[serial(i18n)]
fn test_catalog_loader_load_or_empty_returns_empty_catalog() {
	let temp_dir = TempDir::new().unwrap();

	// Create a catalog loader with a temporary directory as base path
	let loader = CatalogLoader::new(temp_dir.path());

	let locale: LanguageIdentifier = "xx-XX".parse().unwrap();

	// load_or_empty() returns an empty catalog when no .po file is found
	let catalog = loader.load_or_empty(&locale.to_string());

	assert_eq!(catalog.locale(), "xx-XX");
	assert_eq!(catalog.get("nonexistent"), None);

	// Cleanup is automatic with TempDir
}

#[test]
#[serial(i18n)]
fn test_catalog_loader_multiple_dirs() {
	let temp_dir1 = TempDir::new().unwrap();
	let temp_dir2 = TempDir::new().unwrap();

	let locale_dir1 = temp_dir1.path().join("locale1");
	let locale_dir2 = temp_dir2.path().join("locale2");

	fs::create_dir_all(&locale_dir1).unwrap();
	fs::create_dir_all(&locale_dir2).unwrap();

	// Create two loaders for different base paths
	let loader1 = CatalogLoader::new(&locale_dir1);
	let loader2 = CatalogLoader::new(&locale_dir2);

	let locale: LanguageIdentifier = "fr-FR".parse().unwrap();

	// Use load_or_empty() since no .po files exist in these directories
	// This test verifies both loaders can be created and used independently
	let catalog1 = loader1.load_or_empty(&locale.to_string());
	let catalog2 = loader2.load_or_empty(&locale.to_string());

	// Verify both catalogs have the correct locale
	assert_eq!(catalog1.locale(), "fr-FR");
	assert_eq!(catalog2.locale(), "fr-FR");

	// Note: In production, loaders would support multiple search paths
	// and priority-based loading from different directories

	// Cleanup is automatic with TempDir
}

#[test]
#[serial(i18n)]
fn test_catalog_plural_nonexistent() {
	let locale: LanguageIdentifier = "en-US".parse().unwrap();
	let catalog = MessageCatalog::new(&locale.to_string());

	assert_eq!(catalog.get_plural("nonexistent", 1), None);
}

#[test]
#[serial(i18n)]
fn test_catalog_overwrite() {
	let locale: LanguageIdentifier = "en-US".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	catalog.add("test".to_string(), "first".to_string());
	assert_eq!(catalog.get("test"), Some(&"first".to_string()));

	// Overwrite
	catalog.add("test".to_string(), "second".to_string());
	assert_eq!(catalog.get("test"), Some(&"second".to_string()));
}

#[test]
#[serial(i18n)]
fn test_catalog_unicode_messages() {
	let locale: LanguageIdentifier = "ja-JP".parse().unwrap();
	let mut catalog = MessageCatalog::new(&locale.to_string());

	catalog.add("こんにちは".to_string(), "Hello".to_string());
	catalog.add("Hello".to_string(), "こんにちは".to_string());
	catalog.add("emoji".to_string(), "😀🎉🚀".to_string());

	assert_eq!(catalog.get("こんにちは"), Some(&"Hello".to_string()));
	assert_eq!(catalog.get("Hello"), Some(&"こんにちは".to_string()));
	assert_eq!(catalog.get("emoji"), Some(&"😀🎉🚀".to_string()));
}

// ===================================================================
// Path traversal prevention tests
// ===================================================================

#[rstest]
#[serial(i18n)]
#[case("../../etc/passwd", "parent directory traversal")]
#[case("../secret_config", "single parent traversal")]
#[case("foo/../../bar", "embedded traversal")]
fn test_catalog_loader_rejects_path_traversal(
	#[case] malicious_locale: &str,
	#[case] _description: &str,
) {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let loader = CatalogLoader::new(temp_dir.path());

	// Act
	let result = loader.load(malicious_locale);

	// Assert
	assert!(result.is_err());
	assert!(
		matches!(
			&result,
			Err(I18nError::InvalidLocale(_)) | Err(I18nError::PathTraversal(_))
		),
		"Expected InvalidLocale or PathTraversal error for '{}', got: {:?}",
		malicious_locale,
		result
	);
}

#[rstest]
#[serial(i18n)]
fn test_catalog_loader_rejects_absolute_path_locale() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let loader = CatalogLoader::new(temp_dir.path());

	// Act
	let result = loader.load("/etc/passwd");

	// Assert
	assert!(result.is_err());
	assert!(matches!(result, Err(I18nError::InvalidLocale(_))));
}

#[rstest]
#[serial(i18n)]
fn test_catalog_loader_rejects_empty_locale() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let loader = CatalogLoader::new(temp_dir.path());

	// Act
	let result = loader.load("");

	// Assert
	assert!(result.is_err());
	assert!(matches!(result, Err(I18nError::InvalidLocale(_))));
}

#[rstest]
#[serial(i18n)]
fn test_catalog_loader_rejects_dot_dot_locale() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let loader = CatalogLoader::new(temp_dir.path());

	// Act
	let result = loader.load("..");

	// Assert
	assert!(result.is_err());
	assert!(matches!(result, Err(I18nError::InvalidLocale(_))));
}

#[rstest]
#[serial(i18n)]
fn test_catalog_loader_accepts_valid_locale() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("en-US").join("LC_MESSAGES");
	fs::create_dir_all(&locale_dir).unwrap();
	let po_content = "msgid \"Hello\"\nmsgstr \"Hello\"\n";
	fs::write(locale_dir.join("django.po"), po_content).unwrap();
	let loader = CatalogLoader::new(temp_dir.path());

	// Act
	let result = loader.load("en-US");

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[serial(i18n)]
fn test_catalog_load_from_file_rejects_traversal() {
	// Arrange
	let temp_dir = TempDir::new().unwrap();
	let loader = CatalogLoader::new(temp_dir.path());

	// Act
	let result = loader.load_from_file("../../etc/passwd", "en");

	// Assert
	assert!(result.is_err());
	let err_msg = result.unwrap_err();
	assert!(
		err_msg.contains("traversal") || err_msg.contains("parent"),
		"Expected path traversal error, got: {}",
		err_msg
	);
}
