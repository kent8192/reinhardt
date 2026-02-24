//! Integration tests for social authentication module

#![cfg(feature = "social")]

#[path = "social/claims_test.rs"]
mod claims_test;
#[path = "social/config_test.rs"]
mod config_test;
#[path = "social/error_test.rs"]
mod error_test;
#[path = "social/jwks_cache_test.rs"]
mod jwks_cache_test;
#[path = "social/oidc_discovery_test.rs"]
mod oidc_discovery_test;
#[path = "social/pkce_flow_test.rs"]
mod pkce_flow_test;
#[path = "social/state_management_test.rs"]
mod state_management_test;
#[path = "social/token_test.rs"]
mod token_test;

#[path = "social/e2e.rs"]
pub mod e2e;
#[path = "social/flows.rs"]
pub mod flows;
#[path = "social/oidc.rs"]
pub mod oidc;
#[path = "social/providers.rs"]
pub mod providers;
