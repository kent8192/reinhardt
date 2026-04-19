#![warn(missing_docs)]
//! # Reinhardt Core
//!
//! Core components for the Reinhardt framework, providing fundamental types,
//! exception handling, signals, macros, security, and validation utilities.
//!
//! ## Available Validators
//!
//! The validators crate provides comprehensive validation utilities:
//! - **IPAddressValidator**: IPv4/IPv6 address validation
//! - **PhoneNumberValidator**: International phone number validation (E.164)
//! - **CreditCardValidator**: Credit card validation with Luhn algorithm
//! - **IBANValidator**: International bank account number validation
//! - **ColorValidator**: Hex, RGB, HSL color validation
//! - **FileTypeValidator**: MIME type and extension validation
//! - **CustomRegexValidator**: User-defined regex pattern validation
//!
//! ## Available Backend Implementations
//!
//! The backends crate provides multiple backend implementations:
//! - **Cache Backends**: Redis (✅), DynamoDB (✅), Memcached (✅)
//! - **Email Backends**: SMTP (✅), SendGrid (✅), AWS SES (✅), Mailgun (✅)
//! - **Queue Backends**: Redis (✅), RabbitMQ (✅), AWS SQS (✅)
//! - **Session Backends**: JWT (✅), Database (✅), Redis (✅), Cookie (✅), File (✅)
//! - **Storage Backends**: S3 (✅), Azure Blob (✅), GCS (✅), FileSystem (✅), Memory (✅)
//!
//! For detailed implementation and usage information, see the individual
//! crate documentation in `reinhardt-db`, `reinhardt-auth`, `reinhardt-mail`,
//! and `reinhardt-tasks`.
//!
//! ## Quick Start
//!
//! ```rust
//! use reinhardt_core::exception::{Error, ErrorKind};
//!
//! // Create a typed application error
//! let err = Error::NotFound("Resource not found".to_string());
//! assert_eq!(err.kind(), ErrorKind::NotFound);
//! ```
//!
//! ## Architecture
//!
//! Key modules in this crate:
//!
//! - [`exception`]: Typed error hierarchy for HTTP and application-level errors
//! - [`types`]: Fundamental types (URL, money, phone number, color, coordinates)
//! - [`signals`]: Django-style signal/slot system for decoupled event handling
//! - [`security`]: Password hashing, CSRF, XSS prevention, and security utilities
//! - [`validators`]: Comprehensive input validation (IP, IBAN, phone, credit card)
//! - [`serializers`]: Data serialization and deserialization framework
//! - [`pagination`]: Cursor, page number, and limit-offset pagination strategies
//! - [`parsers`]: Request body parsing (JSON, form, multipart)
//! - [`negotiation`]: HTTP content negotiation utilities
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `types` | enabled | Core type definitions |
//! | `exception` | enabled | Error hierarchy and HTTP status mapping |
//! | `signals` | enabled | Async signal/slot system |
//! | `macros` | enabled | Procedural macros re-export |
//! | `security` | enabled | Password hashing and security utilities |
//! | `validators` | enabled | Comprehensive input validation |
//! | `serializers` | enabled | Data serialization framework |
//! | `parsers` | disabled | Request body parsers |
//! | `pagination` | disabled | Pagination strategies |
//! | `negotiation` | disabled | HTTP content negotiation |
//! | `messages` | disabled | Flash message storage |
//! | `page` | disabled | Server-side page rendering types |
//! | `reactive` | disabled | Reactive state management |
//! | `serde` | disabled | Serde serialization support |
//! | `json` | disabled | JSON serialization support |
//! | `xml` | disabled | XML serialization support |
//! | `yaml` | disabled | YAML serialization support |
//! | `parallel` | disabled | Parallel processing with Rayon |
//! | `i18n` | disabled | Internationalization with Fluent |

pub mod apply_update;
pub use apply_update::ApplyUpdate;
/// HTTP endpoint routing and handler registration.
#[cfg(native)]
pub mod endpoint;
/// Error types and exception handling.
#[cfg(feature = "exception")]
pub mod exception;
/// Flash message storage framework.
#[cfg(feature = "messages")]
pub mod messages;
/// Content negotiation for request/response formats.
#[cfg(feature = "negotiation")]
pub mod negotiation;
/// Pagination strategies (page-based, cursor, limit-offset).
#[cfg(feature = "pagination")]
pub mod pagination;
/// Request body parsers (JSON, form, multipart, etc.).
#[cfg(feature = "parsers")]
pub mod parsers;
/// Rate limiting strategies.
pub mod rate_limit;
/// Reactive state management primitives.
#[cfg(feature = "reactive")]
pub mod reactive;
/// Security utilities (password hashing, CSRF, etc.).
#[cfg(feature = "security")]
pub mod security;
/// Data serialization framework.
#[cfg(feature = "serializers")]
pub mod serializers;
/// Signal/event dispatch system.
#[cfg(feature = "signals")]
pub mod signals;
/// Core type definitions.
#[cfg(feature = "types")]
pub mod types;
/// Field and data validators.
#[cfg(feature = "validators")]
pub mod validators;

// Re-export Page types when page feature is enabled
// This provides Page, PageElement, IntoPage, Head, EventType, etc.
#[cfg(all(feature = "types", feature = "page"))]
pub use crate::types::page;

#[cfg(feature = "macros")]
pub use reinhardt_macros as macros;

// Re-export rate limiting types
pub use crate::rate_limit::RateLimitStrategy;

// Re-export common external dependencies
pub use async_trait::async_trait;

// Re-export tokio only on non-WASM targets
#[cfg(native)]
pub use tokio;

/// Re-export of serde serialization types and serde_json.
#[cfg(feature = "serde")]
pub mod serde {
	pub use ::serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser};
	pub use ::serde_json as json;
}
