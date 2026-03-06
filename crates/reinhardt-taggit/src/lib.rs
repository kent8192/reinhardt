//! # reinhardt-taggit
//!
//! Simple tagging system for Reinhardt framework, inspired by django-taggit.
//!
//! ## Current Features
//!
//! - `Tag` model: Core tag entity with normalized name and URL-friendly slug
//! - `TaggedItem` model: Junction table for polymorphic many-to-many relationships
//! - Error types for tag operations
//!
//! ## Planned Features
//!
//! - `TaggableManager`: High-level API for managing tags on model instances
//! - `#[taggable]` attribute macro for zero-boilerplate tagging
//! - Query extensions for tag-based filtering
//! - Tag normalization and validation
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use reinhardt_taggit::Tag;
//!
//! // Create a tag with auto-generated slug
//! let tag = Tag::from_name("Rust Programming");
//! assert_eq!(tag.slug, "rust-programming");
//!
//! // Create a tag with explicit slug
//! let tag = Tag::new("Rust Programming", "rust-prog");
//! assert_eq!(tag.slug, "rust-prog");
//! ```

// Public modules
pub mod error;
pub mod models;
// TODO: Uncomment when implementations are ready
// pub mod config;
// pub mod manager;
// pub mod query;
// pub mod normalizer;
// pub mod validator;

// Re-exports for convenient access
pub use error::{Result, TaggitError};
pub use models::{Tag, Taggable, TaggedItem};
// TODO: Uncomment when implementations are ready
// pub use manager::TagManager;
// pub use normalizer::{DefaultNormalizer, Normalizer};
// pub use config::{TagConfig, TagConfigBuilder};

// TODO: Uncomment when #[taggable] macro is fully implemented
// pub use reinhardt_taggit_macros::taggable;

/// Prelude module for convenient imports
pub mod prelude {
	pub use crate::error::{Result, TaggitError};
	pub use crate::models::{Tag, Taggable, TaggedItem};
	// TODO: Uncomment when implementations are ready
	// pub use crate::manager::TagManager;
	// pub use crate::config::TagConfig;
	// pub use reinhardt_taggit_macros::taggable;
}
