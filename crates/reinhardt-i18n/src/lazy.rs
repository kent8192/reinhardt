//! Lazy translation strings

use std::fmt;

/// A lazily-evaluated translation string
///
/// The translation is performed when the string is actually used (e.g., displayed or converted to String).
/// This is useful for translations that need to be defined at compile time but evaluated at runtime.
///
/// # Example
///
/// ```
/// use reinhardt_i18n::{TranslationContext, set_active_translation, gettext_lazy, MessageCatalog};
/// use std::sync::Arc;
///
/// let lazy = gettext_lazy("Welcome");
///
/// // Set up translation after creating the lazy string
/// let mut ctx = TranslationContext::new("fr", "en-US");
/// let mut catalog = MessageCatalog::new("fr");
/// catalog.add_translation("Welcome", "Bienvenue");
/// ctx.add_catalog("fr", catalog).unwrap();
///
/// let _guard = set_active_translation(Arc::new(ctx));
///
/// // Translation happens when we use it
/// assert_eq!(lazy.to_string(), "Bienvenue");
/// ```
#[derive(Debug, Clone)]
pub struct LazyString {
	message: String,
	plural_message: Option<String>,
	count: Option<usize>,
	context: Option<String>,
}

impl LazyString {
	/// Create a new lazy translation string
	pub fn new(message: String, context: Option<String>, _is_plural: bool) -> Self {
		Self {
			message,
			plural_message: None,
			count: None,
			context,
		}
	}

	/// Create a new lazy plural translation string
	pub fn new_plural(
		singular: String,
		plural: String,
		count: usize,
		context: Option<String>,
	) -> Self {
		Self {
			message: singular,
			plural_message: Some(plural),
			count: Some(count),
			context,
		}
	}

	/// Evaluate the lazy string to get the actual translation
	fn evaluate(&self) -> String {
		use crate::{gettext, ngettext, npgettext, pgettext};

		match (&self.context, &self.plural_message, self.count) {
			(Some(ctx), Some(plural), Some(count)) => npgettext(ctx, &self.message, plural, count),
			(Some(ctx), None, _) => pgettext(ctx, &self.message),
			(None, Some(plural), Some(count)) => ngettext(&self.message, plural, count),
			_ => gettext(&self.message),
		}
	}
}

impl fmt::Display for LazyString {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.evaluate())
	}
}

impl From<LazyString> for String {
	fn from(lazy: LazyString) -> String {
		lazy.evaluate()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{MessageCatalog, TranslationContext, set_active_translation};
	use serial_test::serial;
	use std::sync::Arc;

	#[test]
	#[serial(i18n)]
	fn test_lazy_string_basic() {
		let lazy = LazyString::new("Hello".to_string(), None, false);

		let mut ctx = TranslationContext::new("zh", "en-US");
		let mut catalog = MessageCatalog::new("zh");
		catalog.add_translation("Hello", "你好");
		ctx.add_catalog("zh", catalog).unwrap();

		let _guard = set_active_translation(Arc::new(ctx));

		assert_eq!(lazy.to_string(), "你好");
	}

	#[test]
	#[serial(i18n)]
	fn test_lazy_string_plural() {
		let lazy = LazyString::new_plural("cat".to_string(), "cats".to_string(), 3, None);

		let mut ctx = TranslationContext::new("ru", "en-US");
		let mut catalog = MessageCatalog::new("ru");
		catalog.add_plural_str("cat", "cats", vec!["кошка", "кошки"]);
		ctx.add_catalog("ru", catalog).unwrap();

		let _guard = set_active_translation(Arc::new(ctx));

		assert_eq!(lazy.to_string(), "кошки");
	}
}
