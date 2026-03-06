//! Test fixtures for reinhardt-taggit
//!
//! Provides reusable fixtures for database setup, Tag models, and TaggedItem models.

mod database_fixture;
mod tag_fixture;
mod tagged_item_fixture;

// Re-export all fixtures
pub use database_fixture::*;
pub use tag_fixture::*;
pub use tagged_item_fixture::*;
