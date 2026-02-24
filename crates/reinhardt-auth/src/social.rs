//! Social Authentication Module
//!
//! Provides OAuth2/OIDC-based social login support for third-party identity providers.
//!
//! # Supported Providers
//!
//! - **Google OIDC**: OpenID Connect authentication with Google
//! - **GitHub OAuth2**: OAuth 2.0 authentication with GitHub
//! - **Apple OIDC**: OpenID Connect authentication with Apple (with JWT-based client_secret)
//! - **Microsoft OIDC**: OpenID Connect authentication with Microsoft/Azure AD
//!
//! # Security Features
//!
//! - **PKCE**: Proof Key for Code Exchange (RFC 7636) for all flows
//! - **CSRF Protection**: State parameter validation
//! - **ID Token Validation**: Signature verification with JWKS
//! - **Nonce Validation**: Replay attack prevention for OIDC flows
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_auth::social::{
//!     ProviderConfig,
//!     providers::GoogleProvider,
//! };
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create your provider configuration (client ID, secret, redirect URI, scopes, etc.)
//!     let config = ProviderConfig::google(
//!         "client_id".into(),
//!         "client_secret".into(),
//!         "https://example.com/callback".into(),
//!     );
//!
//!     // Initialize the Google provider using the configuration.
//!     let google = GoogleProvider::new(config).await.unwrap();
//!
//!     // Integrate `google` with your own routing, session/state management,
//!     // and storage to start authorization flows and handle callbacks.
//! }
//! ```

pub mod backend;
pub mod core;
pub mod flow;
pub mod oidc;
pub mod providers;
pub mod storage;
pub mod user_mapping;

// Re-export core types
pub use core::{
	IdToken, OAuth2Client, OAuth2Config, OAuthProvider, OAuthToken, OIDCConfig, ProviderConfig,
	SocialAuthError, StandardClaims, TokenResponse,
};

// Re-export flow types
pub use flow::{
	AuthorizationFlow, PkceFlow, RefreshFlow, StateData, StateStore, TokenExchangeFlow,
};

// Re-export OIDC types
pub use oidc::{
	DiscoveryClient, IdTokenValidator, Jwk, JwkSet, JwksCache, OIDCDiscovery, UserInfoClient,
};

// Re-export providers
pub use providers::{AppleProvider, GitHubProvider, GoogleProvider, MicrosoftProvider};

// Re-export backend
pub use backend::{AuthorizationResult, CallbackResult, SocialAuthBackend};

// Re-export user mapping
pub use user_mapping::{DefaultUserMapper, MappedUser, UserMapper};

// Re-export storage
pub use storage::{InMemorySocialAccountStorage, SocialAccount, SocialAccountStorage};
