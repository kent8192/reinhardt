//! Email sending module.
//!
//! This module provides email sending with SMTP, console,
//! file, and in-memory backends.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt::mail::{EmailMessage, send_mail};
//! ```

#[cfg(feature = "mail")]
pub use reinhardt_mail::*;
