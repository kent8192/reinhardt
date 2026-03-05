//! Model definitions for the taggit system
//!
//! This module contains the core data models:
//! - `Tag`: Core tag entity with name and slug
//! - `TaggedItem`: Junction table for polymorphic many-to-many relationships
//! - `Taggable`: Trait for models that can be tagged

pub mod tag;
pub mod taggable;
pub mod tagged_item;
// pub mod content_type;

pub use tag::Tag;
pub use taggable::Taggable;
pub use tagged_item::TaggedItem;
// pub use content_type::ContentTypeRegistry;
