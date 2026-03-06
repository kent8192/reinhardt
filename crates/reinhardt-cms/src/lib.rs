//! # Reinhardt CMS
//!
//! A full-featured Content Management System for the Reinhardt framework,
//! inspired by Wagtail and django-cms.
//!
//! ## Features
//!
//! - **Hierarchical Page Tree**: Parent-child page relationships with URL routing
//! - **StreamField Content Blocks**: Polymorphic, nestable content blocks (Wagtail StreamField equivalent)
//! - **Image Management**: Upload, resize, crop (renditions), metadata, folder management
//! - **Fine-grained Permissions**: Page-level access control, role-based permissions
//! - **Workflow Engine**: Draft/Review/Publish states, approval flow, version history
//! - **Admin UI Integration**: Integrated with reinhardt-pages for seamless admin experience
//!
//! ## Architecture
//!
//! ```text
//! reinhardt-cms
//! ├── pages     - Hierarchical page tree, URL routing
//! ├── blocks    - StreamField-style content blocks
//! ├── media     - Image/file management, renditions
//! ├── permissions - Page-level access control
//! ├── workflow  - Draft/review/publish state machine
//! └── admin     - Admin UI components
//! ```
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use reinhardt_cms::prelude::*;
//!
//! // Define a custom page type
//! #[derive(Page)]
//! struct BlogPage {
//!     #[page(parent)]
//!     parent: Option<PageId>,
//!
//!     title: String,
//!     slug: String,
//!
//!     #[page(content)]
//!     body: StreamField,
//! }
//!
//! // Use StreamField with content blocks
//! let body = StreamField::new()
//!     .add_block(RichTextBlock::new("Introduction text"))
//!     .add_block(ImageBlock::new(image_id))
//!     .add_block(QuoteBlock::new("Famous quote"));
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

// Re-export for macros
pub use serde;
pub use serde_json;

// Module declarations
pub mod admin;
pub mod blocks;
pub mod media;
pub mod pages;
pub mod permissions;
pub mod workflow;

// Prelude for convenient imports
pub mod prelude {
	//! Convenient re-exports of commonly used items

	// Pages
	pub use crate::pages::{Page, PageNode, PageTree};

	// Blocks
	pub use crate::blocks::{Block, BlockLibrary, StreamField};

	// Media
	pub use crate::media::{ImageRendition, MediaFile, RenditionSpec};

	// Permissions
	pub use crate::permissions::{PagePermission, PermissionChecker};

	// Workflow
	pub use crate::workflow::{PageState, WorkflowEngine};

	// Admin
	pub use crate::admin::{AdminPageRegistry, PageEditor};
}

/// CMS error types
pub mod error {
	use thiserror::Error;

	/// CMS-related errors
	#[derive(Error, Debug)]
	pub enum CmsError {
		/// Page not found
		#[error("Page not found: {0}")]
		PageNotFound(String),

		/// Invalid page hierarchy (e.g., circular reference)
		#[error("Invalid page hierarchy: {0}")]
		InvalidHierarchy(String),

		/// Block type not registered
		#[error("Block type not registered: {0}")]
		UnknownBlockType(String),

		/// Media file not found
		#[error("Media file not found: {0}")]
		MediaNotFound(String),

		/// Permission denied
		#[error("Permission denied: {0}")]
		PermissionDenied(String),

		/// Invalid workflow transition
		#[error("Invalid workflow transition: {0}")]
		InvalidWorkflowTransition(String),

		/// Database error
		#[error("Database error: {0}")]
		Database(String),

		/// Generic error
		#[error("{0}")]
		Generic(String),
	}

	/// Result type for CMS operations
	pub type CmsResult<T> = Result<T, CmsError>;
}
