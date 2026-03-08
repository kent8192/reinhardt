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
//! | `serializers` | enabled | Data serialization framework |
//! | `parsers` | enabled | Request body parsers |
//! | `pagination` | enabled | Pagination strategies |
//! | `messages` | disabled | Flash message storage |
//! | `page` | disabled | Server-side page rendering types |

pub mod apply_update;
pub use apply_update::ApplyUpdate;
/// HTTP endpoint routing and handler registration.
#[cfg(not(target_arch = "wasm32"))]
pub mod endpoint;
/// Error types and exception handling.
pub mod exception;
/// Flash message storage framework.
pub mod messages;
/// Content negotiation for request/response formats.
pub mod negotiation;
/// Pagination strategies (page-based, cursor, limit-offset).
pub mod pagination;
/// Request body parsers (JSON, form, multipart, etc.).
pub mod parsers;
/// Rate limiting strategies.
pub mod rate_limit;
/// Reactive state management primitives.
pub mod reactive;
/// Security utilities (password hashing, CSRF, etc.).
pub mod security;
/// Data serialization framework.
pub mod serializers;
/// Signal/event dispatch system.
pub mod signals;
/// Core type definitions.
pub mod types;
/// Field and data validators.
pub mod validators;

// Re-export Page types when page feature is enabled
// This provides Page, PageElement, IntoPage, Head, EventType, etc.
#[cfg(feature = "page")]
pub use crate::types::page;

#[cfg(feature = "macros")]
pub use reinhardt_macros as macros;

// Re-export rate limiting types
pub use crate::rate_limit::RateLimitStrategy;

// Re-export common external dependencies
pub use async_trait::async_trait;

// Re-export tokio only on non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
pub use tokio;

/// Re-export of serde serialization types and serde_json.
#[cfg(feature = "serde")]
pub mod serde {
	pub use ::serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser};
	pub use ::serde_json as json;
}
