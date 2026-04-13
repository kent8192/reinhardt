//! Test authentication utilities.
//!
//! Provides a builder-based API for setting up authentication state in tests,
//! replacing the deprecated `force_authenticate` method.
//!
//! # Architecture
//!
//! - **[`ForceLoginUser`]**: Trait for extracting session identity from any user type.
//!   Blanket-implemented for all `AuthIdentity` types (available on native targets).
//! - **[`SessionIdentity`]**: Type-erased identity struct matching `CookieSessionAuthMiddleware` fields.
//! - **[`AuthBuilder`]**: Entry point returned by `APIClient::auth()`.
//! - **[`SecondaryAuth`]**: Open trait for secondary auth layers (MFA, PassKey, etc.).
//!
//! # Platform Support
//!
//! Session/JWT builders, TOTP secondary auth, and `AuthIdentity` blanket impl are
//! available unconditionally on native targets (non-wasm).

mod error;
mod identity;
mod secondary;
mod traits;

pub use error::TestAuthError;
pub use identity::SessionIdentity;
pub use secondary::SecondaryAuth;
pub use traits::ForceLoginUser;

#[cfg(native)]
pub use secondary::TotpSecondaryAuth;

#[cfg(native)]
mod builder;
#[cfg(native)]
pub use builder::{AuthBuilder, JwtAuthBuilder, JwtTestConfig, SessionAuthBuilder};

#[cfg(native)]
mod server_fn_builder;
#[cfg(native)]
pub use server_fn_builder::ServerFnAuthBuilder;
