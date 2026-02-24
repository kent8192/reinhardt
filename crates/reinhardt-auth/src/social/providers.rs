//! OAuth2/OIDC provider implementations

pub mod apple;
pub mod github;
pub mod google;
pub mod microsoft;

pub use apple::AppleProvider;
pub use github::GitHubProvider;
pub use google::GoogleProvider;
pub use microsoft::MicrosoftProvider;
