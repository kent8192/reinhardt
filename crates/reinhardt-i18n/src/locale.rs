//! Locale management functions
//!
//! These functions provide backward-compatible locale management.
//! For new code, prefer using `TranslationContext` with `set_active_translation()`.

use crate::{
	I18nError, MessageCatalog, TranslationContext, get_active_translation,
	set_active_translation_permanent,
};
use std::sync::Arc;

/// Maximum length for locale strings.
/// BCP 47 / ISO 639 locale identifiers are at most 35 characters;
/// 64 provides a generous upper bound.
const MAX_LOCALE_LEN: usize = 64;

/// Validate locale string format
pub(crate) fn validate_locale(locale: &str) -> Result<(), I18nError> {
	// Basic validation: locale should contain only alphanumeric characters, hyphens, and underscores
	if locale.is_empty() {
		return Err(I18nError::InvalidLocale(
			"Locale cannot be empty".to_string(),
		));
	}

	if locale.len() > MAX_LOCALE_LEN {
		return Err(I18nError::InvalidLocale(format!(
			"Locale too long ({} bytes, maximum is {})",
			locale.len(),
			MAX_LOCALE_LEN
		)));
	}

	if !locale
		.chars()
		.all(|c| c.is_alphanumeric() || c == '-' || c == '_')
	{
		return Err(I18nError::InvalidLocale(locale.to_string()));
	}

	Ok(())
}

/// Activate a locale by creating a new translation context.
///
/// **Note**: This function creates a new context with only the specified locale.
/// For full control, use `TranslationContext` with `set_active_translation()`.
///
/// **Warning**: The returned guard must be kept in scope for translations to work.
/// This is a change from the previous global state behavior.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, gettext, MessageCatalog};
/// use std::sync::Arc;
///
/// // Preferred approach: use TranslationContext directly
/// let mut ctx = TranslationContext::new("es", "en-US");
/// let mut catalog = MessageCatalog::new("es");
/// catalog.add_translation("Welcome", "Bienvenido");
/// ctx.add_catalog("es", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
/// assert_eq!(gettext("Welcome"), "Bienvenido");
/// ```
pub fn activate(locale: &str) -> Result<(), I18nError> {
	validate_locale(locale)?;

	// Get current context or create new one
	let mut ctx = get_active_translation()
		.map(|arc| (*arc).clone())
		.unwrap_or_else(|| TranslationContext::new("en-US", "en-US"));

	// Already validated above, safe to unwrap
	ctx.set_locale(locale).expect("locale already validated");

	// Set the new context permanently (no guard, no memory leak)
	// In new code, users should use set_active_translation() directly
	set_active_translation_permanent(Arc::new(ctx));

	Ok(())
}

/// Activate a locale with its message catalog directly
///
/// This creates a new translation context with the given locale and catalog.
///
/// **Warning**: The returned guard must be kept in scope for translations to work.
/// This is a change from the previous global state behavior.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, gettext, MessageCatalog};
/// use std::sync::Arc;
///
/// // Preferred approach: use TranslationContext directly
/// let mut ctx = TranslationContext::new("es", "en-US");
/// let mut catalog = MessageCatalog::new("es");
/// catalog.add_translation("Welcome", "Bienvenido");
/// ctx.add_catalog("es", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
/// assert_eq!(gettext("Welcome"), "Bienvenido");
/// ```
pub fn activate_with_catalog(locale: &str, catalog: MessageCatalog) -> Result<(), I18nError> {
	validate_locale(locale)?;

	// Get current context or create new one
	let mut ctx = get_active_translation()
		.map(|arc| (*arc).clone())
		.unwrap_or_else(|| TranslationContext::new("en-US", "en-US"));

	// Already validated above, safe to unwrap
	ctx.set_locale(locale).expect("locale already validated");
	ctx.add_catalog(locale, catalog)
		.expect("locale already validated");

	// Set the new context permanently (no guard, no memory leak)
	set_active_translation_permanent(Arc::new(ctx));

	Ok(())
}

/// Deactivate the current locale and revert to English
///
/// This sets the current locale to English (en-US).
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, deactivate, gettext, MessageCatalog};
/// use std::sync::Arc;
///
/// let mut ctx = TranslationContext::new("de", "en-US");
/// let mut catalog = MessageCatalog::new("de");
/// catalog.add_translation("Hello", "Hallo");
/// ctx.add_catalog("de", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
/// assert_eq!(gettext("Hello"), "Hallo");
///
/// deactivate();
/// assert_eq!(gettext("Hello"), "Hello");
/// ```
pub fn deactivate() {
	// Get current context and reset locale to empty
	if let Some(arc) = get_active_translation() {
		let mut ctx = (*arc).clone();
		// Empty string is allowed for deactivation (reset to default)
		ctx.set_locale("")
			.expect("empty string is always valid for deactivation");

		// Set the new context permanently (no guard, no memory leak)
		set_active_translation_permanent(Arc::new(ctx));
	}
}

/// Get the currently active locale
///
/// Returns "en-US" if no translation context is active.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, get_locale, MessageCatalog};
/// use std::sync::Arc;
///
/// // No active context
/// assert_eq!(get_locale(), "en-US");
///
/// let ctx = TranslationContext::new("it", "en-US");
/// let _guard = set_active_translation(Arc::new(ctx));
/// assert_eq!(get_locale(), "it");
/// ```
pub fn get_locale() -> String {
	get_active_translation()
		.map(|ctx| ctx.get_locale().to_string())
		.unwrap_or_else(|| "en-US".to_string())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::set_active_translation;
	use rstest::rstest;
	use serial_test::serial;

	#[rstest]
	#[serial(i18n)]
	fn test_locale_activation() {
		// Arrange
		let mut ctx = TranslationContext::new("pt", "en-US");
		let catalog = MessageCatalog::new("pt");
		ctx.add_catalog("pt", catalog).unwrap();
		let _guard = set_active_translation(Arc::new(ctx));

		// Act
		let locale = get_locale();

		// Assert
		assert_eq!(locale, "pt");
	}

	#[rstest]
	#[serial(i18n)]
	fn test_deactivate() {
		// Arrange
		let mut ctx = TranslationContext::new("fr", "en-US");
		let catalog = MessageCatalog::new("fr");
		ctx.add_catalog("fr", catalog).unwrap();
		let _guard = set_active_translation(Arc::new(ctx));
		assert_eq!(get_locale(), "fr");

		// Act
		deactivate();

		// Assert
		assert_eq!(get_locale(), "en-US");
	}

	#[rstest]
	#[serial(i18n)]
	fn test_activate_does_not_leak_arc() {
		// Arrange
		let ctx = Arc::new(TranslationContext::new("en-US", "en-US"));
		set_active_translation_permanent(Arc::clone(&ctx));

		// Act: activate multiple times; each call replaces the previous Arc
		activate("ja").unwrap();
		activate("de").unwrap();
		activate("fr").unwrap();

		// Assert: only one strong reference remains from this scope
		// (the thread-local holds a different Arc after activate calls)
		assert_eq!(Arc::strong_count(&ctx), 1);
	}

	#[rstest]
	#[serial(i18n)]
	fn test_activate_with_catalog_does_not_leak_arc() {
		// Arrange
		let ctx = Arc::new(TranslationContext::new("en-US", "en-US"));
		set_active_translation_permanent(Arc::clone(&ctx));

		// Act: activate_with_catalog replaces the context without leaking
		let catalog = MessageCatalog::new("es");
		activate_with_catalog("es", catalog).unwrap();

		// Assert: original Arc has only one strong reference (this scope)
		assert_eq!(Arc::strong_count(&ctx), 1);
	}

	#[rstest]
	#[serial(i18n)]
	fn test_deactivate_does_not_leak_arc() {
		// Arrange
		let mut ctx = TranslationContext::new("ko", "en-US");
		let catalog = MessageCatalog::new("ko");
		ctx.add_catalog("ko", catalog).unwrap();
		let shared = Arc::new(ctx);
		set_active_translation_permanent(Arc::clone(&shared));

		// Act
		deactivate();

		// Assert: original Arc has only one strong reference (this scope)
		assert_eq!(Arc::strong_count(&shared), 1);
	}

	#[rstest]
	#[serial(i18n)]
	fn test_activate_validates_locale() {
		// Act & Assert
		assert!(activate("").is_err());
		assert!(activate("en/US").is_err());
		assert!(activate("en US").is_err());
		assert!(activate("en-US").is_ok());
		assert!(activate("ja").is_ok());
	}

	#[rstest]
	fn test_validate_locale_rejects_too_long_string() {
		// Arrange: locale string exceeding MAX_LOCALE_LEN (64)
		let long_locale = "a".repeat(65);

		// Act
		let result = validate_locale(&long_locale);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_locale_accepts_max_length_string() {
		// Arrange: locale string exactly at MAX_LOCALE_LEN (64)
		let max_locale = "a".repeat(64);

		// Act
		let result = validate_locale(&max_locale);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[serial(i18n)]
	fn test_activate_with_catalog_validates_locale() {
		// Arrange
		let catalog = MessageCatalog::new("valid");

		// Act & Assert: invalid locales are rejected
		assert!(activate_with_catalog("", catalog).is_err());

		let catalog = MessageCatalog::new("valid");
		assert!(activate_with_catalog("en/US", catalog).is_err());

		let catalog = MessageCatalog::new("valid");
		assert!(activate_with_catalog("en US", catalog).is_err());

		// Act & Assert: valid locales are accepted
		let catalog = MessageCatalog::new("es");
		assert!(activate_with_catalog("es", catalog).is_ok());
	}

	#[rstest]
	fn test_set_locale_validates_locale() {
		// Arrange
		let mut ctx = TranslationContext::new("en-US", "en-US");

		// Act & Assert: invalid locales are rejected
		assert!(ctx.set_locale("en/US").is_err());
		assert!(ctx.set_locale("en US").is_err());
		assert!(ctx.set_locale("../etc/passwd").is_err());

		// Act & Assert: valid locales are accepted
		assert!(ctx.set_locale("ja").is_ok());
		assert!(ctx.set_locale("en-US").is_ok());

		// Act & Assert: empty string is allowed for deactivation
		assert!(ctx.set_locale("").is_ok());
	}

	#[rstest]
	fn test_set_fallback_locale_validates_locale() {
		// Arrange
		let mut ctx = TranslationContext::new("en-US", "en-US");

		// Act & Assert: invalid locales are rejected
		assert!(ctx.set_fallback_locale("en/US").is_err());
		assert!(ctx.set_fallback_locale("../etc/passwd").is_err());

		// Act & Assert: valid locales are accepted
		assert!(ctx.set_fallback_locale("fr").is_ok());
		assert!(ctx.set_fallback_locale("en-US").is_ok());
	}

	#[rstest]
	fn test_add_catalog_validates_locale() {
		// Arrange
		let mut ctx = TranslationContext::new("en-US", "en-US");

		// Act & Assert: invalid locales are rejected
		let catalog = MessageCatalog::new("test");
		assert!(ctx.add_catalog("", catalog).is_err());

		let catalog = MessageCatalog::new("test");
		assert!(ctx.add_catalog("en/US", catalog).is_err());

		let catalog = MessageCatalog::new("test");
		assert!(ctx.add_catalog("../etc", catalog).is_err());

		// Act & Assert: valid locales are accepted
		let catalog = MessageCatalog::new("ja");
		assert!(ctx.add_catalog("ja", catalog).is_ok());
	}
}
