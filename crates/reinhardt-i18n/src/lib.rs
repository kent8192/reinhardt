//! Internationalization (i18n) support for Reinhardt
//!
//! This crate provides Django-style internationalization features including:
//! - Message translation with gettext-style API
//! - Plural forms support
//! - Context-aware translations
//! - Lazy translation evaluation
//! - Message catalog management
//!
//! # Example
//!
//! ```
//! use reinhardt_i18n::{TranslationContext, set_active_translation, gettext, MessageCatalog};
//! use std::sync::Arc;
//!
//! // Create a translation context with Japanese catalog
//! let mut ctx = TranslationContext::new("ja", "en-US");
//! let mut catalog = MessageCatalog::new("ja");
//! catalog.add_translation("Hello", "こんにちは");
//! ctx.add_catalog("ja", catalog).unwrap();
//!
//! // Set as active translation context (scoped)
//! let _guard = set_active_translation(Arc::new(ctx));
//!
//! // Translate messages
//! let greeting = gettext("Hello");
//! assert_eq!(greeting, "こんにちは");
//! // Guard is dropped here, restoring previous context
//! ```
//!
//! # DI Integration
//!
//! When the `di` feature is enabled, `TranslationContext` implements `Injectable`:
//!
//! ```ignore
//! use reinhardt_di::{InjectionContext, SingletonScope, Injectable};
//! use reinhardt_i18n::TranslationContext;
//!
//! async fn handler(ctx: &InjectionContext) {
//!     let translation = TranslationContext::inject(ctx).await.unwrap();
//!     // Use translation...
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use reinhardt_utils::safe_path_join;

/// Error types for i18n operations
#[derive(Debug, thiserror::Error)]
pub enum I18nError {
	#[error("Invalid locale format: {0}")]
	InvalidLocale(String),
	#[error("Catalog not found for locale: {0}")]
	CatalogNotFound(String),
	#[error("Failed to load catalog: {0}")]
	LoadError(String),
	#[error("Path traversal detected: {0}")]
	PathTraversal(String),
}

mod catalog;
mod lazy;
mod locale;
pub mod po_parser;
mod translation;
pub mod utils;

pub use catalog::MessageCatalog;
pub use lazy::LazyString;
use locale::validate_locale;
pub use locale::{activate, activate_with_catalog, deactivate, get_locale};
pub use translation::{gettext, gettext_lazy, ngettext, ngettext_lazy, npgettext, pgettext};

// Re-export get_locale as get_language for compatibility
pub use locale::get_locale as get_language;

// New scoped translation API
// TranslationContext, TranslationGuard, set_active_translation, get_active_translation
// are defined below and exported at module level

/// Catalog loader for loading message catalogs from files or other sources
pub struct CatalogLoader {
	base_path: std::path::PathBuf,
}

impl CatalogLoader {
	/// Create a new catalog loader with the given base path
	///
	/// # Example
	/// ```
	/// use reinhardt_i18n::CatalogLoader;
	///
	/// let loader = CatalogLoader::new("locale");
	/// ```
	pub fn new<P: Into<std::path::PathBuf>>(base_path: P) -> Self {
		Self {
			base_path: base_path.into(),
		}
	}

	/// Load a catalog for the given locale from a .po file
	///
	/// This method looks for .po files in the following locations:
	/// - `{base_path}/{locale}/LC_MESSAGES/django.po`
	/// - `{base_path}/{locale}/LC_MESSAGES/messages.po`
	///
	/// # Errors
	///
	/// Returns `I18nError::CatalogNotFound` if no .po file is found for the locale.
	/// Returns `I18nError::LoadError` if the file cannot be opened or parsed.
	///
	/// # Example
	/// ```no_run
	/// use reinhardt_i18n::CatalogLoader;
	///
	/// let loader = CatalogLoader::new("locale");
	/// let catalog = loader.load("fr").unwrap();
	/// ```
	pub fn load(&self, locale: &str) -> Result<MessageCatalog, I18nError> {
		// Validate locale name to prevent path traversal attacks.
		// Locale names should only contain alphanumeric characters, hyphens, and underscores.
		if locale.is_empty()
			|| !locale
				.chars()
				.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
		{
			return Err(I18nError::InvalidLocale(locale.to_string()));
		}

		// Defense in depth: use safe_path_join to verify the locale path stays
		// within the base directory, even after the character validation above.
		let safe_locale_dir = safe_path_join(&self.base_path, locale).map_err(|e| {
			I18nError::PathTraversal(format!(
				"Locale '{}' failed path safety check: {}",
				locale, e
			))
		})?;

		// Try multiple common .po file locations
		let possible_paths = vec![
			safe_locale_dir.join("LC_MESSAGES").join("django.po"),
			safe_locale_dir.join("LC_MESSAGES").join("messages.po"),
		];

		for path in possible_paths {
			if path.exists() {
				let file = std::fs::File::open(&path).map_err(|e| {
					I18nError::LoadError(format!("Failed to open .po file at {:?}: {}", path, e))
				})?;

				return po_parser::parse_po_file(file, locale)
					.map_err(|e| I18nError::LoadError(format!("Failed to parse .po file: {}", e)));
			}
		}

		// If no .po file found, return an error
		Err(I18nError::CatalogNotFound(locale.to_string()))
	}

	/// Load a catalog for the given locale, returning an empty catalog if not found
	///
	/// This is a convenience method that falls back to an empty catalog when
	/// no .po file is found. Use `load()` instead if you want to handle
	/// missing catalogs explicitly.
	///
	/// # Example
	/// ```
	/// use reinhardt_i18n::CatalogLoader;
	///
	/// let loader = CatalogLoader::new("locale");
	/// // Returns empty catalog if no .po file exists
	/// let catalog = loader.load_or_empty("fr");
	/// ```
	pub fn load_or_empty(&self, locale: &str) -> MessageCatalog {
		self.load(locale)
			.unwrap_or_else(|_| MessageCatalog::new(locale))
	}

	/// Load a catalog from a specific .po file path
	///
	/// # Example
	/// ```no_run
	/// use reinhardt_i18n::CatalogLoader;
	///
	/// let loader = CatalogLoader::new("locale");
	/// let catalog = loader.load_from_file("locale/fr/custom.po", "fr").unwrap();
	/// ```
	pub fn load_from_file<P: AsRef<std::path::Path>>(
		&self,
		path: P,
		locale: &str,
	) -> Result<MessageCatalog, String> {
		// Validate the file path stays within the base directory
		let path_str = path
			.as_ref()
			.to_str()
			.ok_or_else(|| "Invalid path encoding".to_string())?;
		let safe_path = safe_path_join(&self.base_path, path_str).map_err(|e| e.to_string())?;

		let file = std::fs::File::open(&safe_path)
			.map_err(|e| format!("Failed to open .po file: {}", e))?;

		po_parser::parse_po_file(file, locale)
			.map_err(|e| format!("Failed to parse .po file: {}", e))
	}
}

// Thread-local storage for the active translation context
thread_local! {
	static ACTIVE_TRANSLATION: RefCell<Option<Arc<TranslationContext>>> = const { RefCell::new(None) };
}

/// Translation context containing catalogs and locale settings.
///
/// This struct holds all the translation state including:
/// - Message catalogs indexed by locale
/// - Current active locale
/// - Fallback locale for missing translations
///
/// # Usage
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, gettext, MessageCatalog};
/// use std::sync::Arc;
///
/// let mut ctx = TranslationContext::new("ja", "en-US");
/// let mut catalog = MessageCatalog::new("ja");
/// catalog.add_translation("Hello", "こんにちは");
/// ctx.add_catalog("ja", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
/// assert_eq!(gettext("Hello"), "こんにちは");
/// ```
#[derive(Clone, Default)]
pub struct TranslationContext {
	current_locale: String,
	fallback_locale: String,
	catalogs: HashMap<String, MessageCatalog>,
}

impl TranslationContext {
	/// Creates a new translation context with the specified locales.
	///
	/// # Arguments
	///
	/// * `current_locale` - The current locale to use for translations
	/// * `fallback_locale` - The fallback locale when translation is not found
	pub fn new(current_locale: impl Into<String>, fallback_locale: impl Into<String>) -> Self {
		Self {
			current_locale: current_locale.into(),
			fallback_locale: fallback_locale.into(),
			catalogs: HashMap::new(),
		}
	}

	/// Creates a new translation context with English (en-US) as default.
	pub fn english() -> Self {
		Self::new("en-US", "en-US")
	}

	/// Returns the current locale.
	pub fn get_locale(&self) -> &str {
		if self.current_locale.is_empty() {
			"en-US"
		} else {
			&self.current_locale
		}
	}

	/// Returns the fallback locale.
	pub fn get_fallback_locale(&self) -> &str {
		if self.fallback_locale.is_empty() {
			"en-US"
		} else {
			&self.fallback_locale
		}
	}

	/// Returns the catalog for the given locale.
	pub fn get_catalog(&self, locale: &str) -> Option<&MessageCatalog> {
		self.catalogs.get(locale)
	}

	/// Sets the current locale.
	///
	/// # Errors
	///
	/// Returns `I18nError::InvalidLocale` if the locale string format is invalid.
	pub fn set_locale(&mut self, locale: impl Into<String>) -> Result<(), I18nError> {
		let locale = locale.into();
		// Allow empty string for deactivation (reset to default)
		if !locale.is_empty() {
			validate_locale(&locale)?;
		}
		self.current_locale = locale;
		Ok(())
	}

	/// Sets the fallback locale.
	///
	/// # Errors
	///
	/// Returns `I18nError::InvalidLocale` if the locale string format is invalid.
	pub fn set_fallback_locale(&mut self, locale: impl Into<String>) -> Result<(), I18nError> {
		let locale = locale.into();
		if !locale.is_empty() {
			validate_locale(&locale)?;
		}
		self.fallback_locale = locale;
		Ok(())
	}

	/// Adds a message catalog for the given locale.
	///
	/// # Errors
	///
	/// Returns `I18nError::InvalidLocale` if the locale string format is invalid.
	pub fn add_catalog(
		&mut self,
		locale: impl Into<String>,
		catalog: MessageCatalog,
	) -> Result<(), I18nError> {
		let locale = locale.into();
		validate_locale(&locale)?;
		self.catalogs.insert(locale, catalog);
		Ok(())
	}

	/// Translates a message using the current locale.
	///
	/// Falls back to the fallback locale if translation is not found.
	pub fn translate(&self, message: &str) -> String {
		let locale = self.get_locale();

		if let Some(translation) = self.get_catalog(locale).and_then(|c| c.get(message)) {
			return translation.clone();
		}

		// Try fallback locale
		let fallback = self.get_fallback_locale();
		if locale != fallback
			&& let Some(translation) = self.get_catalog(fallback).and_then(|c| c.get(message))
		{
			return translation.clone();
		}

		// Return original message if no translation found
		message.to_string()
	}

	/// Translates a message with plural support.
	pub fn translate_plural(&self, singular: &str, plural: &str, count: usize) -> String {
		let locale = self.get_locale();

		if let Some(translation) = self
			.get_catalog(locale)
			.and_then(|c| c.get_plural(singular, count))
		{
			return translation.clone();
		}

		// Try fallback locale
		let fallback = self.get_fallback_locale();
		if locale != fallback
			&& let Some(translation) = self
				.get_catalog(fallback)
				.and_then(|c| c.get_plural(singular, count))
		{
			return translation.clone();
		}

		// Use default English plural rules
		if count == 1 { singular } else { plural }.to_string()
	}

	/// Translates a message with context.
	pub fn translate_context(&self, context: &str, message: &str) -> String {
		let locale = self.get_locale();

		if let Some(translation) = self
			.get_catalog(locale)
			.and_then(|c| c.get_context(context, message))
		{
			return translation.clone();
		}

		// Try fallback locale
		let fallback = self.get_fallback_locale();
		if locale != fallback
			&& let Some(translation) = self
				.get_catalog(fallback)
				.and_then(|c| c.get_context(context, message))
		{
			return translation.clone();
		}

		// Return original message if no translation found
		message.to_string()
	}

	/// Translates a message with context and plural support.
	pub fn translate_context_plural(
		&self,
		context: &str,
		singular: &str,
		plural: &str,
		count: usize,
	) -> String {
		let locale = self.get_locale();

		if let Some(translation) = self
			.get_catalog(locale)
			.and_then(|c| c.get_context_plural(context, singular, count))
		{
			return translation.clone();
		}

		// Try fallback locale
		let fallback = self.get_fallback_locale();
		if locale != fallback
			&& let Some(translation) = self
				.get_catalog(fallback)
				.and_then(|c| c.get_context_plural(context, singular, count))
		{
			return translation.clone();
		}

		// Use default English plural rules
		if count == 1 { singular } else { plural }.to_string()
	}
}

/// RAII guard for active TranslationContext scope.
///
/// When dropped, restores the previous translation context.
pub struct TranslationGuard {
	prev: Option<Arc<TranslationContext>>,
}

impl Drop for TranslationGuard {
	fn drop(&mut self) {
		ACTIVE_TRANSLATION.with(|t| {
			// Use try_borrow_mut to prevent panic on reentrant drop.
			// If the RefCell is already borrowed (e.g., during a destructor chain),
			// the translation will be cleaned up when the outer borrow is released.
			if let Ok(mut guard) = t.try_borrow_mut() {
				*guard = self.prev.take();
			}
		});
	}
}

/// Sets the active translation context and returns a guard.
///
/// The guard restores the previous context when dropped.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, gettext, MessageCatalog};
/// use std::sync::Arc;
///
/// let mut ctx = TranslationContext::new("de", "en-US");
/// let mut catalog = MessageCatalog::new("de");
/// catalog.add_translation("Hello", "Hallo");
/// ctx.add_catalog("de", catalog).unwrap();
///
/// {
///     let _guard = set_active_translation(Arc::new(ctx));
///     assert_eq!(gettext("Hello"), "Hallo");
/// }
/// // Context restored to previous (or None)
/// assert_eq!(gettext("Hello"), "Hello");
/// ```
pub fn set_active_translation(ctx: Arc<TranslationContext>) -> TranslationGuard {
	let prev = ACTIVE_TRANSLATION.with(|t| t.borrow_mut().replace(ctx));
	TranslationGuard { prev }
}

/// Sets the active translation context permanently without returning a guard.
///
/// Unlike `set_active_translation()`, this function does not provide RAII semantics.
/// The translation context remains active until explicitly changed or until the thread ends.
/// Use this when you need permanent activation without scope-based cleanup.
///
/// # Memory Safety
///
/// This function is memory-safe and does not leak memory like `std::mem::forget` on the guard.
/// The previous translation context (if any) is properly dropped.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation_permanent, gettext, MessageCatalog};
/// use std::sync::Arc;
///
/// let mut ctx = TranslationContext::new("de", "en-US");
/// let mut catalog = MessageCatalog::new("de");
/// catalog.add_translation("Hello", "Hallo");
/// ctx.add_catalog("de", catalog).unwrap();
///
/// set_active_translation_permanent(Arc::new(ctx));
/// assert_eq!(gettext("Hello"), "Hallo");
///
/// // Context remains active (no guard to drop)
/// assert_eq!(gettext("Hello"), "Hallo");
/// ```
pub fn set_active_translation_permanent(ctx: Arc<TranslationContext>) {
	ACTIVE_TRANSLATION.with(|t| {
		*t.borrow_mut() = Some(ctx);
	});
}

/// Returns the currently active translation context, if any.
pub fn get_active_translation() -> Option<Arc<TranslationContext>> {
	ACTIVE_TRANSLATION.with(|t| t.borrow().clone())
}

// DI integration (feature-gated)
#[cfg(feature = "di")]
mod di_integration {
	use super::*;
	use reinhardt_di::{DiResult, Injectable, InjectionContext};

	#[async_trait::async_trait]
	impl Injectable for TranslationContext {
		async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
			// First check thread-local storage
			if let Some(active) = get_active_translation() {
				return Ok((*active).clone());
			}

			// Fall back to singleton scope
			if let Some(singleton) = ctx.get_singleton::<TranslationContext>() {
				return Ok((*singleton).clone());
			}

			// Default to empty English context
			Ok(TranslationContext::english())
		}
	}
}
