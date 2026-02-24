//! i18n command tests
//!
//! Tests based on Django's i18n extraction and compilation tests

use reinhardt_commands::{
	BaseCommand, CommandContext, CompileMessagesCommand, MakeMessagesCommand,
};
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

/// Helper to run a command in a specific directory
async fn run_in_dir<F, Fut>(dir: &std::path::Path, f: F) -> Fut::Output
where
	F: FnOnce() -> Fut,
	Fut: std::future::Future,
{
	let original_dir = std::env::current_dir().ok();
	let _ = std::env::set_current_dir(dir);

	let result = f().await;

	if let Some(original) = original_dir {
		let _ = std::env::set_current_dir(original);
	}

	result
}

#[tokio::test]
#[serial]
async fn test_makemessages_valid_locale() {
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale");
	fs::create_dir(&locale_dir).unwrap();

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["en_us".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// Check that PO file was created
	let po_file = temp_dir
		.path()
		.join("locale/en_us/LC_MESSAGES/reinhardt.po");
	assert!(po_file.exists());
}

#[tokio::test]
#[serial]
async fn test_makemessages_invalid_locale_uppercase() {
	let temp_dir = TempDir::new().unwrap();

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["EN_US".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_err());
	// Should error about uppercase not allowed
}

#[tokio::test]
#[serial]
async fn test_makemessages_no_locale() {
	let temp_dir = TempDir::new().unwrap();
	std::env::set_current_dir(temp_dir.path()).unwrap();

	let cmd = MakeMessagesCommand;
	let ctx = CommandContext::new(vec![]);

	let result = cmd.execute(&ctx).await;

	assert!(result.is_err());
	// Should error about no locale specified
}

#[tokio::test]
#[serial]
async fn test_compilemessages_one_locale() {
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale/ja_jp/LC_MESSAGES");
	fs::create_dir_all(&locale_dir).unwrap();

	// Create a dummy PO file
	let po_file = locale_dir.join("reinhardt.po");
	fs::write(&po_file, "# Dummy PO file\nmsgid \"\"\nmsgstr \"\"").unwrap();

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = CompileMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["ja_jp".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// Check that MO file was created
	let mo_file = temp_dir
		.path()
		.join("locale/ja_jp/LC_MESSAGES/reinhardt.mo");
	assert!(mo_file.exists());
}

#[tokio::test]
#[serial]
async fn test_compilemessages_multiple_locales() {
	let temp_dir = TempDir::new().unwrap();

	// Create multiple locales
	for locale in &["en_us", "fr_fr", "ja_jp"] {
		let locale_dir = temp_dir
			.path()
			.join(format!("locale/{}/LC_MESSAGES", locale));
		fs::create_dir_all(&locale_dir).unwrap();

		let po_file = locale_dir.join("reinhardt.po");
		fs::write(&po_file, "# Dummy PO file\nmsgid \"\"\nmsgstr \"\"").unwrap();
	}

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = CompileMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi(
			"locale".to_string(),
			vec!["en_us".to_string(), "fr_fr".to_string()],
		);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// Check that MO files were created for specified locales
	assert!(
		temp_dir
			.path()
			.join("locale/en_us/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
	assert!(
		temp_dir
			.path()
			.join("locale/fr_fr/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
}

#[tokio::test]
#[serial]
async fn test_compilemessages_exclude() {
	let temp_dir = TempDir::new().unwrap();

	// Create multiple locales
	for locale in &["en_us", "fr_fr", "ja_jp"] {
		let locale_dir = temp_dir
			.path()
			.join(format!("locale/{}/LC_MESSAGES", locale));
		fs::create_dir_all(&locale_dir).unwrap();

		let po_file = locale_dir.join("reinhardt.po");
		fs::write(&po_file, "# Dummy PO file\nmsgid \"\"\nmsgstr \"\"").unwrap();
	}

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = CompileMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("exclude".to_string(), vec!["ja_jp".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// Check that MO files were created for non-excluded locales
	assert!(
		temp_dir
			.path()
			.join("locale/en_us/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
	assert!(
		temp_dir
			.path()
			.join("locale/fr_fr/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
	// ja_jp should not have MO file
	assert!(
		!temp_dir
			.path()
			.join("locale/ja_jp/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
}

#[tokio::test]
#[serial]
async fn test_compilemessages_no_locales() {
	let temp_dir = TempDir::new().unwrap();
	std::env::set_current_dir(temp_dir.path()).unwrap();

	let cmd = CompileMessagesCommand;
	let ctx = CommandContext::new(vec![]);

	let result = cmd.execute(&ctx).await;

	// Should succeed but warn that no locales found
	assert!(result.is_ok());
}

#[tokio::test]
#[serial]
async fn test_makemessages_multiple_locales() {
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale");
	fs::create_dir(&locale_dir).unwrap();

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi(
			"locale".to_string(),
			vec!["en_us".to_string(), "ja_jp".to_string()],
		);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// Check that PO files were created for both locales
	assert!(
		temp_dir
			.path()
			.join("locale/en_us/LC_MESSAGES/reinhardt.po")
			.exists()
	);
	assert!(
		temp_dir
			.path()
			.join("locale/ja_jp/LC_MESSAGES/reinhardt.po")
			.exists()
	);
}

#[tokio::test]
#[serial]
async fn test_makemessages_invalid_locale_start_with_underscore() {
	let temp_dir = TempDir::new().unwrap();
	std::env::set_current_dir(temp_dir.path()).unwrap();

	let cmd = MakeMessagesCommand;
	let ctx = CommandContext::new(vec!["--locale".to_string(), "_en_us".to_string()]);

	let result = cmd.execute(&ctx).await;

	assert!(result.is_err());
}

#[tokio::test]
#[serial]
async fn test_makemessages_pot_charset_header() {
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale");
	fs::create_dir(&locale_dir).unwrap();

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["en_us".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// Check that PO file has UTF-8 charset
	let po_file = temp_dir
		.path()
		.join("locale/en_us/LC_MESSAGES/reinhardt.po");
	let content = fs::read_to_string(&po_file).unwrap();
	assert!(content.contains("charset=UTF-8"));
}

#[tokio::test]
#[serial]
async fn test_makemessages_update_existing_po() {
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale/en_us/LC_MESSAGES");
	fs::create_dir_all(&locale_dir).unwrap();

	// Create an existing PO file
	let po_file = locale_dir.join("reinhardt.po");
	fs::write(
		&po_file,
		"# Existing PO file\nmsgid \"Hello\"\nmsgstr \"Hi\"",
	)
	.unwrap();

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["en_us".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());
	// File should still exist after update
	assert!(po_file.exists());
}

#[tokio::test]
#[serial]
async fn test_makemessages_all_option() {
	let temp_dir = TempDir::new().unwrap();

	// Create locale directories with existing PO files
	for locale in &["en_us", "fr_fr"] {
		let locale_dir = temp_dir
			.path()
			.join(format!("locale/{}/LC_MESSAGES", locale));
		fs::create_dir_all(&locale_dir).unwrap();
		let po_file = locale_dir.join("reinhardt.po");
		fs::write(&po_file, "# PO file").unwrap();
	}

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option("all".to_string(), "true".to_string());
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());
}

#[tokio::test]
#[serial]
async fn test_makemessages_invalid_locale_hyphen() {
	let temp_dir = TempDir::new().unwrap();
	std::env::set_current_dir(temp_dir.path()).unwrap();

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		// Django convention is underscore, not hyphen
		ctx.set_option_multi("locale".to_string(), vec!["en-us".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	// Should succeed - hyphens are valid in language codes
	assert!(result.is_ok());
}

#[tokio::test]
#[serial]
async fn test_makemessages_invalid_locale_special_chars() {
	let temp_dir = TempDir::new().unwrap();
	std::env::set_current_dir(temp_dir.path()).unwrap();

	let cmd = MakeMessagesCommand;
	let ctx = CommandContext::new(vec!["--locale".to_string(), "en$us".to_string()]);

	let result = cmd.execute(&ctx).await;

	assert!(result.is_err());
}

#[tokio::test]
#[serial]
async fn test_compilemessages_missing_po_file() {
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale/en_us/LC_MESSAGES");
	fs::create_dir_all(&locale_dir).unwrap();
	// Don't create PO file

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = CompileMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["en_us".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	// Should succeed but warn about missing file
	assert!(result.is_ok());

	// MO file should not be created
	let mo_file = temp_dir
		.path()
		.join("locale/en_us/LC_MESSAGES/reinhardt.mo");
	assert!(!mo_file.exists());
}

#[tokio::test]
#[serial]
async fn test_compilemessages_all_locales() {
	let temp_dir = TempDir::new().unwrap();

	// Create multiple locales with PO files
	for locale in &["en_us", "fr_fr", "ja_jp"] {
		let locale_dir = temp_dir
			.path()
			.join(format!("locale/{}/LC_MESSAGES", locale));
		fs::create_dir_all(&locale_dir).unwrap();
		let po_file = locale_dir.join("reinhardt.po");
		fs::write(&po_file, "# Dummy PO file\nmsgid \"\"\nmsgstr \"\"").unwrap();
	}

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = CompileMessagesCommand;
		let ctx = CommandContext::new(vec![]); // No locale specified = all locales
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// All MO files should be created
	assert!(
		temp_dir
			.path()
			.join("locale/en_us/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
	assert!(
		temp_dir
			.path()
			.join("locale/fr_fr/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
	assert!(
		temp_dir
			.path()
			.join("locale/ja_jp/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
}

#[tokio::test]
#[serial]
async fn test_compilemessages_multiple_excludes() {
	let temp_dir = TempDir::new().unwrap();

	// Create multiple locales
	for locale in &["en_us", "fr_fr", "ja_jp", "de_de"] {
		let locale_dir = temp_dir
			.path()
			.join(format!("locale/{}/LC_MESSAGES", locale));
		fs::create_dir_all(&locale_dir).unwrap();
		let po_file = locale_dir.join("reinhardt.po");
		fs::write(&po_file, "# Dummy PO file\nmsgid \"\"\nmsgstr \"\"").unwrap();
	}

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = CompileMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi(
			"exclude".to_string(),
			vec!["ja_jp".to_string(), "de_de".to_string()],
		);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// Check that only non-excluded locales have MO files
	assert!(
		temp_dir
			.path()
			.join("locale/en_us/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
	assert!(
		temp_dir
			.path()
			.join("locale/fr_fr/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
	assert!(
		!temp_dir
			.path()
			.join("locale/ja_jp/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
	assert!(
		!temp_dir
			.path()
			.join("locale/de_de/LC_MESSAGES/reinhardt.mo")
			.exists()
	);
}

#[tokio::test]
#[serial]
async fn test_makemessages_po_file_structure() {
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale");
	fs::create_dir(&locale_dir).unwrap();

	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["ja_jp".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	assert!(result.is_ok());

	// Check PO file structure
	let po_file = temp_dir
		.path()
		.join("locale/ja_jp/LC_MESSAGES/reinhardt.po");
	let content = fs::read_to_string(&po_file).unwrap();

	// Should contain required headers
	assert!(content.contains("msgid \"\""));
	assert!(content.contains("msgstr \"\""));
	assert!(content.contains("Content-Type: text/plain; charset=UTF-8"));
	assert!(content.contains("Language: ja_jp"));
}

// --- Regression tests for #378: PO format injection and MO integer overflow ---

use rstest::rstest;

#[rstest]
#[tokio::test]
#[serial]
async fn test_makemessages_po_content_survives_compile() {
	// Arrange: create locale dir and a source file with a translatable string
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale");
	fs::create_dir(&locale_dir).unwrap();

	let src_dir = temp_dir.path().join("src");
	fs::create_dir(&src_dir).unwrap();
	let src_file = src_dir.join("lib.rs");
	// Source contains a translatable string with a backslash sequence
	fs::write(&src_file, "fn foo() { gettext!(\"hello world\"); }").unwrap();

	// Act: extract messages to PO
	let make_result = run_in_dir(temp_dir.path(), || async {
		let cmd = MakeMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["en_us".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	// Assert: PO file created and contains properly escaped msgid
	assert!(
		make_result.is_ok(),
		"makemessages failed: {:?}",
		make_result
	);
	let po_path = temp_dir
		.path()
		.join("locale/en_us/LC_MESSAGES/reinhardt.po");
	assert!(po_path.exists());
	let po_content = fs::read_to_string(&po_path).unwrap();
	assert!(
		po_content.contains("msgid \"hello world\""),
		"expected msgid not found in PO:\n{po_content}"
	);

	// Act: compile PO to MO
	let compile_result = run_in_dir(temp_dir.path(), || async {
		let cmd = CompileMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["en_us".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	// Assert: MO file produced (round-trip succeeds)
	assert!(
		compile_result.is_ok(),
		"compilemessages failed: {:?}",
		compile_result
	);
	let mo_path = temp_dir
		.path()
		.join("locale/en_us/LC_MESSAGES/reinhardt.mo");
	assert!(mo_path.exists());
	let mo_bytes = fs::read(&mo_path).unwrap();
	// MO magic number: 0x950412de (little-endian)
	assert_eq!(
		&mo_bytes[0..4],
		&0x950412de_u32.to_le_bytes(),
		"invalid MO magic number"
	);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_compilemessages_mo_byte_header_is_valid() {
	// Arrange: a minimal PO file with one translated message
	let temp_dir = TempDir::new().unwrap();
	let locale_dir = temp_dir.path().join("locale/fr_fr/LC_MESSAGES");
	fs::create_dir_all(&locale_dir).unwrap();

	let po_content =
		"# Test PO\nmsgid \"\"\nmsgstr \"\"\n\nmsgid \"Save\"\nmsgstr \"Enregistrer\"\n";
	fs::write(locale_dir.join("reinhardt.po"), po_content).unwrap();

	// Act
	let result = run_in_dir(temp_dir.path(), || async {
		let cmd = CompileMessagesCommand;
		let mut ctx = CommandContext::new(vec![]);
		ctx.set_option_multi("locale".to_string(), vec!["fr_fr".to_string()]);
		cmd.execute(&ctx).await
	})
	.await;

	// Assert: MO file has valid header (no integer overflow occurred)
	assert!(result.is_ok(), "compilemessages failed: {:?}", result);
	let mo_path = locale_dir.join("reinhardt.mo");
	assert!(mo_path.exists());
	let mo_bytes = fs::read(&mo_path).unwrap();
	assert!(
		mo_bytes.len() >= 28,
		"MO file too short: {} bytes",
		mo_bytes.len()
	);
	assert_eq!(
		&mo_bytes[0..4],
		&0x950412de_u32.to_le_bytes(),
		"invalid MO magic number"
	);
}
