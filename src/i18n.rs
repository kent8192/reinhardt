//! Internationalization module.
//!
//! This module provides Django-style internationalization with
//! message translation, plural forms, and lazy evaluation.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt::i18n::{TranslationContext, MessageCatalog, gettext};
//! ```

#[cfg(feature = "i18n")]
pub use reinhardt_i18n::*;
