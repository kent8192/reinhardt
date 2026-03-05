//! # Reinhardt Utils
//!
//! Utility functions for Reinhardt framework, inspired by Django's utils module.
//!
//! ## Modules
//!
//! - `timezone`: Timezone-aware datetime handling
//! - `dateformat`: Date and time formatting utilities
//! - `html`: HTML escaping and manipulation
//! - `encoding`: Text encoding and URL encoding
//! - `text`: Text manipulation utilities
//! - `humanize`: Human-friendly formatting utilities
//! - `logging`: Logging utilities (feature: `logging`)
//! - `cache`: Caching utilities (feature: `cache`)
//! - `storage`: Storage utilities (feature: `storage`)
//! - `staticfiles`: Static file serving utilities (feature: `staticfiles`)
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_utils::{timezone, dateformat, html, encoding, text};
//!
//! // Timezone
//! let now = timezone::now();
//! let formatted = dateformat::format(&now, "Y-m-d H:i:s");
//!
//! // HTML
//! let escaped = html::escape("<script>alert('XSS')</script>");
//!
//! // Encoding
//! let slug = encoding::slugify("Hello World");
//!
//! // Text
//! let ordinal = text::ordinal(1); // "1st"
//! ```

pub mod cache;
pub mod logging;
pub mod staticfiles;
pub mod storage;
pub mod utils_core;

// Core modules
pub mod humanize;

// Re-export utils-core modules
pub use crate::utils_core::{dateformat, encoding, html, text, timezone};

pub use crate::utils_core::input_validation::{
	IdentifierError, sanitize_log_input, validate_identifier, validate_redirect_url,
};
pub use crate::utils_core::lock_recovery::{
	LockRecoveryError, recover_mutex, recover_rwlock_read, recover_rwlock_write,
};
pub use crate::utils_core::path_safety::{
	PathTraversalError, is_safe_filename_component, safe_path_join,
};
pub use dateformat::format as format_date;
pub use encoding::{escapejs, slugify, truncate_chars, truncate_words, urldecode, urlencode};
pub use html::{SafeString, escape, escape_attr, strip_tags, unescape};
pub use text::{capfirst, floatcomma, intcomma, ordinal, pluralize, title};
pub use timezone::{
	get_timezone_name_local, get_timezone_name_utc, localtime, now, to_local, to_utc,
};
