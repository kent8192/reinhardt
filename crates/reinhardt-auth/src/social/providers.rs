//! OAuth2/OIDC provider implementations

pub mod apple;
pub mod generic_oidc;
pub mod github;
pub mod google;
pub mod microsoft;

pub use apple::AppleProvider;
pub use generic_oidc::{GenericOidcConfig, GenericOidcProvider, UserInfoMapper};
pub use github::GitHubProvider;
pub use google::GoogleProvider;
pub use microsoft::MicrosoftProvider;
