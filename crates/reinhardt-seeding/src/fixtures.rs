//! Fixture management module.
//!
//! This module provides Django-compatible fixture loading and dumping capabilities.
//!
//! # Overview
//!
//! Fixtures are serialized data files (JSON or YAML) that contain database records.
//! They can be used for:
//!
//! - Initial data population
//! - Test data setup
//! - Data migration between environments
//!
//! # Example
//!
//! ```json
//! [
//!   {
//!     "model": "auth.User",
//!     "pk": 1,
//!     "fields": {
//!       "username": "admin",
//!       "email": "admin@example.com"
//!     }
//!   }
//! ]
//! ```

mod format;
mod loader;
mod parser;
mod registry;
mod serializer;

pub use format::{FixtureData, FixtureFormat, FixtureRecord};
pub use loader::{FixtureLoader, LoadOptions, LoadResult};
pub use parser::FixtureParser;
pub use registry::{ModelLoader, ModelRegistry, register_model_loader};
pub use serializer::{FixtureSerializer, ModelSerializer};
