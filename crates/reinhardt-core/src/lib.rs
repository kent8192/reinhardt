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
//! ## Related Backend Crates
//!
//! Backend integrations live in focused crates rather than in `reinhardt-core`.
//! See `reinhardt-db`, `reinhardt-auth`, `reinhardt-mail`,
//! `reinhardt-tasks`, and `reinhardt-utils` for database, auth, mail,
//! queue, cache, and storage integrations.
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
//! - [`security`]: CSRF, XSS prevention, security headers, HSTS, IP filtering, redirect validation, and resource limits
//! - [`validators`]: Comprehensive input validation (IP, IBAN, phone, credit card)
//! - [`serializers`]: Data serialization and deserialization framework
//! - `pagination`: Cursor, page number, and limit-offset pagination strategies
//! - [`parsers`]: Request body parsing (JSON, form, multipart)
//! - `negotiation`: HTTP content negotiation utilities
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `types` | enabled | Core type definitions |
//! | `exception` | enabled | Error hierarchy and HTTP status mapping |
//! | `signals` | enabled | Async signal/slot system |
//! | `macros` | enabled | Procedural macros re-export |
//! | `security` | enabled | CSRF, XSS prevention, headers, HSTS, IP filtering, redirects, and resource limits |
//! | `validators` | enabled | Comprehensive input validation |
//! | `serializers` | enabled | Data serialization framework |
//! | `parsers` | disabled | Request body parsers |
//! | `pagination` | disabled | Pagination strategies |
//! | `negotiation` | disabled | HTTP content negotiation |
//! | `messages` | disabled | Flash message storage |
//! | `page` | disabled | Server-side page rendering types (enables `security`) |
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
/// Target-neutral schema, policy, and payload contracts for model-backed forms.
pub mod model_form;
/// Target-neutral metadata traits emitted by model macros.
pub mod model_info {
	use std::marker::PhantomData;

	/// Minimal model identity needed by generated `{Model}Info` companion types.
	///
	/// Unlike the ORM `Model` trait, this trait is available on WASM and only
	/// exposes the primary-key type required for generated foreign-key `*_id`
	/// fields.
	pub trait InfoModel {
		/// Primary-key type used by generated DTO companion fields.
		type PrimaryKey;

		/// Database table name used by generated relation-aware client metadata.
		fn table_name() -> &'static str {
			""
		}
	}

	/// Lightweight relationship reference used by generated `{Model}Info` fields (Issue #5272).
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(
		feature = "serde",
		serde(bound(
			serialize = "T::PrimaryKey: serde::Serialize",
			deserialize = "T::PrimaryKey: serde::Deserialize<'de>"
		))
	)]
	pub struct RelationInfo<T: InfoModel> {
		/// Primary key of the related model.
		pub id: T::PrimaryKey,
		#[cfg_attr(feature = "serde", serde(skip))]
		_model: PhantomData<T>,
	}

	impl<T: InfoModel> RelationInfo<T> {
		/// Creates a relationship reference from a related model primary key.
		pub const fn new(id: T::PrimaryKey) -> Self {
			Self {
				id,
				_model: PhantomData,
			}
		}

		/// Returns the related model primary key.
		pub const fn id(&self) -> &T::PrimaryKey {
			&self.id
		}

		/// Converts this relationship reference into its primary key.
		pub fn into_id(self) -> T::PrimaryKey {
			self.id
		}
	}

	impl<T> Default for RelationInfo<T>
	where
		T: InfoModel,
		T::PrimaryKey: Default,
	{
		fn default() -> Self {
			Self::new(T::PrimaryKey::default())
		}
	}

	impl<T> std::fmt::Debug for RelationInfo<T>
	where
		T: InfoModel,
		T::PrimaryKey: std::fmt::Debug,
	{
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			f.debug_struct("RelationInfo")
				.field("id", &self.id)
				.finish()
		}
	}

	impl<T> Clone for RelationInfo<T>
	where
		T: InfoModel,
		T::PrimaryKey: Clone,
	{
		fn clone(&self) -> Self {
			Self::new(self.id.clone())
		}
	}

	impl<T> PartialEq for RelationInfo<T>
	where
		T: InfoModel,
		T::PrimaryKey: PartialEq,
	{
		fn eq(&self, other: &Self) -> bool {
			self.id == other.id
		}
	}

	/// Lightweight many-to-many relationship payload for generated `{Model}Info` (Issue #5272).
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	#[cfg_attr(
		feature = "serde",
		serde(bound(
			serialize = "Target::PrimaryKey: serde::Serialize",
			deserialize = "Target::PrimaryKey: serde::Deserialize<'de>"
		))
	)]
	pub struct ManyToManyInfo<Source, Target: InfoModel> {
		/// Primary keys of related target models.
		pub target_ids: Vec<Target::PrimaryKey>,
		#[cfg_attr(feature = "serde", serde(skip))]
		_source: PhantomData<Source>,
	}

	impl<Source, Target> ManyToManyInfo<Source, Target>
	where
		Target: InfoModel,
	{
		/// Creates a many-to-many payload from target primary keys.
		pub fn new<I>(target_ids: I) -> Self
		where
			I: IntoIterator<Item = Target::PrimaryKey>,
		{
			Self {
				target_ids: target_ids.into_iter().collect(),
				_source: PhantomData,
			}
		}

		/// Creates an empty many-to-many payload.
		pub const fn empty() -> Self {
			Self {
				target_ids: Vec::new(),
				_source: PhantomData,
			}
		}

		/// Returns the target model primary keys.
		pub fn target_ids(&self) -> &[Target::PrimaryKey] {
			&self.target_ids
		}

		/// Converts this payload into the target primary-key list.
		pub fn into_target_ids(self) -> Vec<Target::PrimaryKey> {
			self.target_ids
		}
	}

	impl<Source, Target> Default for ManyToManyInfo<Source, Target>
	where
		Target: InfoModel,
	{
		fn default() -> Self {
			Self::empty()
		}
	}

	impl<Source, Target> std::fmt::Debug for ManyToManyInfo<Source, Target>
	where
		Target: InfoModel,
		Target::PrimaryKey: std::fmt::Debug,
	{
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			f.debug_struct("ManyToManyInfo")
				.field("target_ids", &self.target_ids)
				.finish()
		}
	}

	impl<Source, Target> Clone for ManyToManyInfo<Source, Target>
	where
		Target: InfoModel,
		Target::PrimaryKey: Clone,
	{
		fn clone(&self) -> Self {
			Self::new(self.target_ids.clone())
		}
	}

	impl<Source, Target> PartialEq for ManyToManyInfo<Source, Target>
	where
		Target: InfoModel,
		Target::PrimaryKey: PartialEq,
	{
		fn eq(&self, other: &Self) -> bool {
			self.target_ids == other.target_ids
		}
	}

	#[cfg(test)]
	mod tests {
		use super::{InfoModel, ManyToManyInfo, RelationInfo};

		#[derive(Debug)]
		struct Post;

		impl InfoModel for Post {
			type PrimaryKey = i64;
		}

		#[test]
		fn relation_info_preserves_primary_key() {
			let relation = RelationInfo::<Post>::new(7);
			assert_eq!(*relation.id(), 7);
			assert_eq!(relation.into_id(), 7);
		}

		#[test]
		fn many_to_many_info_preserves_target_ids() {
			let info = ManyToManyInfo::<(), Post>::new([1, 2, 3]);
			assert_eq!(info.target_ids(), &[1, 2, 3]);
		}
	}
}
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
/// Security utilities (CSRF, XSS prevention, headers, HSTS, IP filtering, redirects, and resource limits).
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
/// WebSocket routing primitives shared across reinhardt crates.
#[cfg(native)]
pub mod ws;

// Re-export Page types when page feature is enabled
// This provides Page, PageElement, IntoPage, Head, EventType, etc.
#[cfg(all(feature = "types", feature = "page"))]
pub use crate::types::page;

#[cfg(feature = "macros")]
pub use reinhardt_macros as macros;
#[cfg(feature = "macros")]
pub use reinhardt_macros::Validate;

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
