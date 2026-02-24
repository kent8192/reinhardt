//! Application registry for examples-twitter
//!
//! This file maintains the list of installed apps.
//! New apps created with `startapp` will be automatically added here.

#[path = "apps/auth/lib.rs"]
pub mod auth;

#[path = "apps/profile/lib.rs"]
pub mod profile;

#[path = "apps/relationship/lib.rs"]
pub mod relationship;

#[path = "apps/dm/lib.rs"]
pub mod dm;

#[path = "apps/tweet/lib.rs"]
pub mod tweet;
