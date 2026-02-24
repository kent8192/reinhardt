//! Core abstractions for social authentication

pub mod claims;
pub mod client;
pub mod config;
pub mod error;
pub mod provider;
pub mod token;

pub use claims::{IdToken, StandardClaims};
pub use client::OAuth2Client;
pub use config::{OAuth2Config, OIDCConfig, ProviderConfig};
pub use error::SocialAuthError;
pub use provider::OAuthProvider;
pub use token::{OAuthToken, TokenResponse};
