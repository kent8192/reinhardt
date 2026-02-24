//! Lazy translation tests
//!
//! Tests based on Django's i18n/tests.py - lazy translation functionality

use reinhardt_i18n::{
	MessageCatalog, TranslationContext, gettext_lazy, ngettext_lazy, set_active_translation,
};
use serial_test::serial;
use std::sync::Arc;

/// Creates a translation context with French, German, and Polish catalogs
fn create_test_context(locale: &str) -> TranslationContext {
	let mut ctx = TranslationContext::new(locale, "en-US");

	// Setup French catalog
	let mut fr_catalog = MessageCatalog::new("fr-FR");
	fr_catalog.add("Add %(name)s".to_string(), "Ajouter %(name)s".to_string());
	fr_catalog.add("Hello".to_string(), "Bonjour".to_string());
	fr_catalog.add_plural(
		"%(count)d item".to_string(),
		vec![
			"%(count)d élément".to_string(),
			"%(count)d éléments".to_string(),
		],
	);
	ctx.add_catalog("fr-FR", fr_catalog).unwrap();

	// Setup German catalog
	let mut de_catalog = MessageCatalog::new("de-DE");
	de_catalog.add(
		"Add %(name)s".to_string(),
		"%(name)s hinzufügen".to_string(),
	);
	de_catalog.add("Hello".to_string(), "Hallo".to_string());
	de_catalog.add_plural(
		"%(count)d item".to_string(),
		vec![
			"%(count)d Artikel".to_string(),
			"%(count)d Artikel".to_string(),
		],
	);
	ctx.add_catalog("de-DE", de_catalog).unwrap();

	// Setup Polish catalog
	let mut pl_catalog = MessageCatalog::new("pl-PL");
	pl_catalog.add("Add %(name)s".to_string(), "Dodaj %(name)s".to_string());
	pl_catalog.add("Hello".to_string(), "Witaj".to_string());
	ctx.add_catalog("pl-PL", pl_catalog).unwrap();

	ctx
}

#[test]
#[serial(i18n)]
fn test_lazy_string_basic() {
	let lazy_msg = gettext_lazy("Hello");

	// Initially no context - returns original
	assert_eq!(lazy_msg.to_string(), "Hello");

	// Activate French
	let ctx = create_test_context("fr-FR");
	let _guard = set_active_translation(Arc::new(ctx));
	assert_eq!(lazy_msg.to_string(), "Bonjour");
}

#[test]
#[serial(i18n)]
fn test_lazy_string_locale_switching() {
	let lazy_msg = gettext_lazy("Hello");

	// Test with French
	{
		let ctx = create_test_context("fr-FR");
		let _guard = set_active_translation(Arc::new(ctx));
		assert_eq!(lazy_msg.to_string(), "Bonjour");
	}

	// Test with German
	{
		let ctx = create_test_context("de-DE");
		let _guard = set_active_translation(Arc::new(ctx));
		assert_eq!(lazy_msg.to_string(), "Hallo");
	}

	// Back to no context
	assert_eq!(lazy_msg.to_string(), "Hello");
}

#[test]
#[serial(i18n)]
fn test_lazy_translation_string_display() {
	let lazy_msg = gettext_lazy("Hello");

	let ctx = create_test_context("fr-FR");
	let _guard = set_active_translation(Arc::new(ctx));
	let displayed = format!("{}", lazy_msg);
	assert_eq!(displayed, "Bonjour");
}

#[test]
#[serial(i18n)]
fn test_lazy_string_interpolation() {
	let lazy_msg = gettext_lazy("Add %(name)s");

	// Test in French
	{
		let ctx = create_test_context("fr-FR");
		let _guard = set_active_translation(Arc::new(ctx));
		let msg = lazy_msg.to_string();
		let result = msg.replace("%(name)s", "Ringo");
		assert_eq!(result, "Ajouter Ringo");
	}

	// Test in German
	{
		let ctx = create_test_context("de-DE");
		let _guard = set_active_translation(Arc::new(ctx));
		let msg = lazy_msg.to_string();
		let result = msg.replace("%(name)s", "Ringo");
		assert_eq!(result, "Ringo hinzufügen");
	}

	// Test in Polish
	{
		let ctx = create_test_context("pl-PL");
		let _guard = set_active_translation(Arc::new(ctx));
		let msg = lazy_msg.to_string();
		let result = msg.replace("%(name)s", "Ringo");
		assert_eq!(result, "Dodaj Ringo");
	}
}

#[test]
#[serial(i18n)]
fn test_ngettext_lazy_basic() {
	let lazy_msg = ngettext_lazy("%(count)d item", "%(count)d items", 1);

	// No context - default plural rules
	let msg = lazy_msg.to_string();
	let result = msg.replace("%(count)d", "1");
	assert_eq!(result, "1 item");

	// With French context
	let ctx = create_test_context("fr-FR");
	let _guard = set_active_translation(Arc::new(ctx));
	let msg = lazy_msg.to_string();
	let result = msg.replace("%(count)d", "1");
	assert_eq!(result, "1 élément");
}

#[test]
#[serial(i18n)]
fn test_ngettext_lazy_plural() {
	let lazy_msg = ngettext_lazy("%(count)d item", "%(count)d items", 5);

	// No context - default plural rules
	let msg = lazy_msg.to_string();
	let result = msg.replace("%(count)d", "5");
	assert_eq!(result, "5 items");

	// With French context
	let ctx = create_test_context("fr-FR");
	let _guard = set_active_translation(Arc::new(ctx));
	let msg = lazy_msg.to_string();
	let result = msg.replace("%(count)d", "5");
	assert_eq!(result, "5 éléments");
}

#[test]
#[serial(i18n)]
fn test_lazy_string_clone() {
	let lazy_msg1 = gettext_lazy("Hello");
	let lazy_msg2 = lazy_msg1.clone();

	let ctx = create_test_context("fr-FR");
	let _guard = set_active_translation(Arc::new(ctx));
	assert_eq!(lazy_msg1.to_string(), "Bonjour");
	assert_eq!(lazy_msg2.to_string(), "Bonjour");
}

#[test]
#[serial(i18n)]
fn test_lazy_string_untranslated() {
	let lazy_msg = gettext_lazy("Untranslated message");
	assert_eq!(lazy_msg.to_string(), "Untranslated message");

	// Even with a context, untranslated messages return original
	let ctx = create_test_context("fr-FR");
	let _guard = set_active_translation(Arc::new(ctx));
	assert_eq!(lazy_msg.to_string(), "Untranslated message");
}

#[test]
#[serial(i18n)]
fn test_lazy_string_empty() {
	let lazy_msg = gettext_lazy("");
	assert_eq!(lazy_msg.to_string(), "");

	let ctx = create_test_context("fr-FR");
	let _guard = set_active_translation(Arc::new(ctx));
	assert_eq!(lazy_msg.to_string(), "");
}

#[test]
#[serial(i18n)]
fn test_lazy_evaluation_timing() {
	// Create lazy string before activating locale
	let lazy_msg = gettext_lazy("Hello");

	// Lazy string should not be evaluated yet
	// Activate French and then evaluate
	{
		let ctx = create_test_context("fr-FR");
		let _guard = set_active_translation(Arc::new(ctx));
		assert_eq!(lazy_msg.to_string(), "Bonjour");
	}

	// Change locale and re-evaluate
	{
		let ctx = create_test_context("de-DE");
		let _guard = set_active_translation(Arc::new(ctx));
		assert_eq!(lazy_msg.to_string(), "Hallo");
	}
}

#[test]
#[serial(i18n)]
fn test_ngettext_lazy_zero() {
	let lazy_msg = ngettext_lazy("%(count)d item", "%(count)d items", 0);

	// No context - default plural rules (0 is plural)
	let msg = lazy_msg.to_string();
	let result = msg.replace("%(count)d", "0");
	assert_eq!(result, "0 items");

	// With French context
	let ctx = create_test_context("fr-FR");
	let _guard = set_active_translation(Arc::new(ctx));
	let msg = lazy_msg.to_string();
	let result = msg.replace("%(count)d", "0");
	// In French: 0 uses singular form
	assert_eq!(result, "0 élément");
}

#[test]
#[serial(i18n)]
fn test_lazy_string_debug() {
	let lazy_msg = gettext_lazy("Test message");
	let debug_str = format!("{:?}", lazy_msg);

	// Debug representation should contain the message
	assert!(debug_str.contains("Test message") || debug_str.contains("LazyString"));
}

#[test]
#[serial(i18n)]
fn test_multiple_lazy_strings() {
	let lazy1 = gettext_lazy("Hello");
	let lazy2 = gettext_lazy("Add %(name)s");

	{
		let ctx = create_test_context("fr-FR");
		let _guard = set_active_translation(Arc::new(ctx));
		assert_eq!(lazy1.to_string(), "Bonjour");
		assert_eq!(
			lazy2.to_string().replace("%(name)s", "Test"),
			"Ajouter Test"
		);
	}

	{
		let ctx = create_test_context("de-DE");
		let _guard = set_active_translation(Arc::new(ctx));
		assert_eq!(lazy1.to_string(), "Hallo");
		assert_eq!(
			lazy2.to_string().replace("%(name)s", "Test"),
			"Test hinzufügen"
		);
	}
}
