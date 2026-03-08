//! # Reinhardt ContentTypes
//!
//! Django-style content types framework for polymorphic relationships.
//!
//! This crate provides a content type system that enables generic relations
//! similar to Django's contenttypes framework.
//!
//! ## Features
//!
//! - **Multi-database support**: Manage content types across multiple databases
//! - **ORM integration**: Seamless integration with reinhardt-orm
//! - **Generic relations**: Type-safe polymorphic relationships
//! - **Database persistence**: Store content types in database with caching
//!
//! ## Planned Features
//!
//! The following features are planned for future releases:
//!
//! ### Permission System Integration
//!
//! - Associate permissions with content types
//! - Permission checking utilities
//! - Content type-based authorization
//!
//! ### Advanced Features
//!
//! - Content type shortcuts (URL resolution for generic objects)
//! - Content type view mixins
//! - Admin interface integration for generic relations
//! - Automatic content type cleanup on model deletion
//! - Content type renaming and migration support
//!
//! ### Management Commands
//!
//! - `dumpdata`/`loaddata` support for content types
//! - Content type synchronization commands
//! - Content type inspection utilities

pub mod cleanup;
// Allow module_inception: Re-exporting contenttypes submodule from contenttypes.rs
// is intentional for compatibility with existing imports (`reinhardt_db::contenttypes::ContentType`)
#[allow(clippy::module_inception)]
/// Contenttypes module.
pub mod contenttypes;
pub mod generic_fk;
pub mod inspect;
pub mod migration;
pub mod permissions;
pub mod persistence;
pub mod serialization;
pub mod shortcuts;
pub mod sync;

#[cfg(feature = "database")]
pub mod multi_db;

#[cfg(feature = "database")]
pub mod orm_integration;

pub use contenttypes::{
	CONTENT_TYPE_REGISTRY, ContentType, ContentTypeRegistry, GenericForeignKey, GenericRelatable,
	GenericRelationQuery, ModelType,
};

pub use generic_fk::GenericForeignKeyField;

pub use permissions::{ContentTypePermission, PermissionAction, PermissionContext};

#[cfg(feature = "database")]
pub use generic_fk::constraints;

#[cfg(feature = "database")]
pub use persistence::{
	ContentTypeModel, ContentTypePersistence, ContentTypePersistenceBackend, PersistenceError,
};

#[cfg(not(feature = "database"))]
pub use persistence::PersistenceError;

#[cfg(feature = "database")]
pub use multi_db::MultiDbContentTypeManager;

#[cfg(feature = "database")]
pub use orm_integration::{ContentTypeQuery, ContentTypeTransaction};
