//! Core translation functions
//!
//! Provides Django-style translation functions:
//! - `gettext()` - Simple translation
//! - `ngettext()` - Plural translation
//! - `pgettext()` - Contextual translation
//! - `npgettext()` - Contextual plural translation

use crate::{LazyString, get_active_translation};

/// Translate a message
///
/// Uses the active translation context set via `set_active_translation()`.
/// If no context is active, returns the original message.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, gettext, MessageCatalog};
/// use std::sync::Arc;
///
/// // Create and activate a Japanese catalog
/// let mut ctx = TranslationContext::new("ja", "en-US");
/// let mut catalog = MessageCatalog::new("ja");
/// catalog.add_translation("Hello, world!", "こんにちは、世界！");
/// ctx.add_catalog("ja", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
/// let msg = gettext("Hello, world!");
/// assert_eq!(msg, "こんにちは、世界！");
/// ```
pub fn gettext(message: &str) -> String {
	if let Some(ctx) = get_active_translation() {
		ctx.translate(message)
	} else {
		// No active context, return original message
		message.to_string()
	}
}

/// Translate a message with plural support
///
/// Uses the active translation context set via `set_active_translation()`.
/// If no context is active, uses default English plural rules.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, ngettext, MessageCatalog};
/// use std::sync::Arc;
///
/// // Set up German plural translations
/// let mut ctx = TranslationContext::new("de", "en-US");
/// let mut catalog = MessageCatalog::new("de");
/// catalog.add_plural_str("item", "items", vec!["Artikel", "Artikel"]);
/// ctx.add_catalog("de", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
///
/// // Singular form (1 item)
/// let msg_singular = ngettext("item", "items", 1);
/// assert_eq!(msg_singular, "Artikel");
///
/// // Plural form (5 items)
/// let msg_plural = ngettext("item", "items", 5);
/// assert_eq!(msg_plural, "Artikel");
/// ```
pub fn ngettext(singular: &str, plural: &str, count: usize) -> String {
	if let Some(ctx) = get_active_translation() {
		ctx.translate_plural(singular, plural, count)
	} else {
		// Use default English plural rules
		if count == 1 { singular } else { plural }.to_string()
	}
}

/// Translate a message with context
///
/// Context helps disambiguate translations. For example:
/// - pgettext("menu", "File") -> "ファイル"
/// - pgettext("verb", "File") -> "提出する"
///
/// Uses the active translation context set via `set_active_translation()`.
/// If no context is active, returns the original message.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, pgettext, MessageCatalog};
/// use std::sync::Arc;
///
/// // Set up contextual translations for Japanese
/// let mut ctx = TranslationContext::new("ja", "en-US");
/// let mut catalog = MessageCatalog::new("ja");
/// catalog.add_context("menu".to_string(), "File".to_string(), "ファイル".to_string());
/// catalog.add_context("verb".to_string(), "File".to_string(), "提出する".to_string());
/// ctx.add_catalog("ja", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
///
/// // Same word, different meanings based on context
/// let menu_file = pgettext("menu", "File");
/// assert_eq!(menu_file, "ファイル");
///
/// let verb_file = pgettext("verb", "File");
/// assert_eq!(verb_file, "提出する");
/// ```
pub fn pgettext(context: &str, message: &str) -> String {
	if let Some(ctx) = get_active_translation() {
		ctx.translate_context(context, message)
	} else {
		// Return original message if no translation found
		message.to_string()
	}
}

/// Translate a message with context and plural support
///
/// Uses the active translation context set via `set_active_translation()`.
/// If no context is active, uses default English plural rules.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, npgettext, MessageCatalog};
/// use std::sync::Arc;
///
/// // Set up contextual plural translations for Spanish
/// let mut ctx = TranslationContext::new("es", "en-US");
/// let mut catalog = MessageCatalog::new("es");
/// catalog.add_context_plural("email", "message", "messages", vec!["mensaje", "mensajes"]);
/// catalog.add_context_plural("notification", "message", "messages", vec!["notificación", "notificaciones"]);
/// ctx.add_catalog("es", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
///
/// // Email context (1 message)
/// let email_singular = npgettext("email", "message", "messages", 1);
/// assert_eq!(email_singular, "mensaje");
///
/// // Email context (5 messages)
/// let email_plural = npgettext("email", "message", "messages", 5);
/// assert_eq!(email_plural, "mensajes");
///
/// // Notification context (3 messages)
/// let notification_plural = npgettext("notification", "message", "messages", 3);
/// assert_eq!(notification_plural, "notificaciones");
/// ```
pub fn npgettext(context: &str, singular: &str, plural: &str, count: usize) -> String {
	if let Some(ctx) = get_active_translation() {
		ctx.translate_context_plural(context, singular, plural, count)
	} else {
		// Use default English plural rules
		if count == 1 { singular } else { plural }.to_string()
	}
}

/// Create a lazy translation that is evaluated when converted to string
///
/// Uses the active translation context at evaluation time.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, gettext_lazy, MessageCatalog};
/// use std::sync::Arc;
///
/// // Create lazy translation before setting up catalog
/// let lazy_msg = gettext_lazy("Good morning");
///
/// // Set up catalog later
/// let mut ctx = TranslationContext::new("ko", "en-US");
/// let mut catalog = MessageCatalog::new("ko");
/// catalog.add_translation("Good morning", "좋은 아침");
/// ctx.add_catalog("ko", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
///
/// // Translation happens when we use it
/// assert_eq!(lazy_msg.to_string(), "좋은 아침");
/// ```
pub fn gettext_lazy(message: &str) -> LazyString {
	LazyString::new(message.to_string(), None, false)
}

/// Create a lazy plural translation
///
/// Uses the active translation context at evaluation time.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, ngettext_lazy, MessageCatalog};
/// use std::sync::Arc;
///
/// // Create lazy plural translation
/// let lazy_msg = ngettext_lazy("apple", "apples", 7);
///
/// // Set up catalog with plural forms
/// // Polish requires 3 forms: form 0 (n==1), form 1 (n%10 in 2..4), form 2 (other)
/// let mut ctx = TranslationContext::new("pl", "en-US");
/// let mut catalog = MessageCatalog::new("pl");
/// catalog.add_plural_str("apple", "apples", vec!["jabłko", "jabłka", "jabłek"]);
/// ctx.add_catalog("pl", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
///
/// // Translation happens when evaluated
/// assert_eq!(lazy_msg.to_string(), "jabłek");
/// ```
pub fn ngettext_lazy(singular: &str, plural: &str, count: usize) -> LazyString {
	LazyString::new_plural(singular.to_string(), plural.to_string(), count, None)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_gettext_no_translation() {
		let result = gettext("Untranslated message");
		assert_eq!(result, "Untranslated message");
	}

	#[test]
	fn test_ngettext_default_rules_unit() {
		let result_singular = ngettext("There is {} item", "There are {} items", 1);
		assert_eq!(result_singular, "There is {} item");

		let result_plural = ngettext("There is {} item", "There are {} items", 5);
		assert_eq!(result_plural, "There are {} items");
	}

	#[test]
	fn test_pgettext_no_translation() {
		let result = pgettext("menu", "File");
		assert_eq!(result, "File");
	}
}
