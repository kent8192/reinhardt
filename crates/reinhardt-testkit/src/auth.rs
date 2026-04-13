//! Test authentication utilities.
//!
//! Provides a builder-based API for setting up authentication state in tests,
//! replacing the deprecated `force_authenticate` method.
//!
//! # Architecture
//!
//! - **[`ForceLoginUser`]**: Trait for extracting session identity from any user type.
//!   Blanket-implemented for all `AuthIdentity` types (requires `auth-testing` feature).
//! - **[`SessionIdentity`]**: Type-erased identity struct matching `CookieSessionAuthMiddleware` fields.
//! - **[`AuthBuilder`]**: Entry point returned by `APIClient::auth()`.
//! - **[`SecondaryAuth`]**: Open trait for secondary auth layers (MFA, PassKey, etc.).
//!
//! # Feature Flags
//!
//! - `auth-testing`: Enables session/JWT builders, TOTP secondary auth, and `AuthIdentity` blanket impl.

mod error;
mod identity;
mod secondary;
mod traits;

pub use error::TestAuthError;
pub use identity::SessionIdentity;
pub use secondary::SecondaryAuth;
pub use traits::ForceLoginUser;

#[cfg(feature = "auth-testing")]
pub use secondary::TotpSecondaryAuth;

#[cfg(feature = "auth-testing")]
mod builder;
#[cfg(feature = "auth-testing")]
pub use builder::{AuthBuilder, JwtAuthBuilder, JwtTestConfig, SessionAuthBuilder};

#[cfg(feature = "auth-testing")]
mod server_fn_builder;
#[cfg(feature = "auth-testing")]
pub use server_fn_builder::ServerFnAuthBuilder;
