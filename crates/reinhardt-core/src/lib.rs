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
//! crate documentation in `reinhardt-contrib`, `reinhardt-tasks`, `reinhardt-core/backends`.

#[cfg(not(target_arch = "wasm32"))]
pub mod endpoint;
pub mod exception;
pub mod messages;
pub mod negotiation;
pub mod pagination;
pub mod parsers;
pub mod rate_limit;
pub mod reactive;
pub mod security;
pub mod serializers;
pub mod signals;
pub mod types;
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

// Re-export serde with json as a submodule
#[cfg(feature = "serde")]
pub mod serde {
	pub use ::serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser};
	pub use ::serde_json as json;
}
