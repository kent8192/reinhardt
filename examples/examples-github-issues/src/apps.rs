//! Application registry for examples-github-issues
//!
//! This file maintains the list of installed apps.
//! New apps created with `startapp` will be automatically added here.

#[path = "apps/auth/lib.rs"]
pub mod auth;
pub use auth::Auth;

#[path = "apps/projects/lib.rs"]
pub mod projects;
pub use projects::Projects;

#[path = "apps/issues/lib.rs"]
pub mod issues;
pub use issues::Issues;
