//! Database seeding and fixture management for the Reinhardt framework.
//!
//! This crate provides Django-compatible data seeding capabilities including:
//!
//! - **Fixture System**: Load and dump data using JSON/YAML fixture files
//! - **Factory Pattern**: Programmatically generate test data with derive macros
//! - **CLI Commands**: `loaddata`, `dumpdata`, and `seed` management commands
//!
//! # Features
//!
//! - `json` - JSON fixture format support (enabled by default)
//! - `yaml` - YAML fixture format support
//! - `macros` - Factory derive macro support (enabled by default)
//! - `full` - All features enabled
//!
//! # Quick Start
//!
//! ## Using Fixtures
//!
//! Create a fixture file (`fixtures/users.json`):
//!
//! ```json
//! [
//!   {
//!     "model": "auth.User",
//!     "pk": 1,
//!     "fields": {
//!       "username": "admin",
//!       "email": "admin@example.com",
//!       "is_active": true
//!     }
//!   }
//! ]
//! ```
//!
//! Load fixtures into the database:
//!
//! ```ignore
//! use reinhardt_seeding::prelude::*;
//!
//! let loader = FixtureLoader::new();
//! let result = loader.load_from_path(Path::new("fixtures/users.json")).await?;
//! println!("Loaded {} records", result.records_loaded);
//! ```
//!
//! ## Using Factories
//!
//! Define a factory for your model:
//!
//! ```ignore
//! use reinhardt_seeding::prelude::*;
//!
//! #[derive(Factory)]
//! #[factory(model = User)]
//! pub struct UserFactory {
//!     #[factory(faker = "username")]
//!     pub username: String,
//!
//!     #[factory(faker = "email")]
//!     pub email: String,
//!
//!     #[factory(sequence = "user_{n}")]
//!     pub code: String,
//!
//!     #[factory(default = true)]
//!     pub is_active: bool,
//! }
//!
//! // Create test instances
//! let factory = UserFactory::new();
//! let user = factory.build();                    // In-memory instance
//! let saved_user = factory.create().await?;     // Persisted to database
//! let batch = factory.create_batch(10).await?;  // Create multiple
//! ```
//!
//! ## Using CLI Commands
//!
//! ```ignore
//! use reinhardt_seeding::commands::{LoadDataCommand, LoadDataArgs, LoadDataOptions};
//!
//! // Load fixtures via command
//! let cmd = LoadDataCommand::new();
//! let args = LoadDataArgs {
//!     fixture_paths: vec!["fixtures/initial_data.json".into()],
//! };
//! let options = LoadDataOptions::new()
//!     .with_verbosity(1);
//! let result = cmd.execute(args, options).await?;
//! ```
//!
//! # Architecture
//!
//! ## Fixture System
//!
//! The fixture system is designed to be compatible with Django fixtures:
//!
//! - [`FixtureRecord`](fixtures::FixtureRecord) - Single fixture record with model ID, pk, and fields
//! - [`FixtureFormat`](fixtures::FixtureFormat) - Supported formats (JSON, YAML)
//! - [`FixtureParser`](fixtures::FixtureParser) - Parse fixture files
//! - [`FixtureLoader`](fixtures::FixtureLoader) - Load fixtures into database
//! - [`FixtureSerializer`](fixtures::FixtureSerializer) - Serialize data to fixtures
//!
//! ## Factory System
//!
//! The factory system is inspired by Factory Boy:
//!
//! - [`Factory`](factory::Factory) trait - Core factory interface
//! - [`FakerType`](factory::FakerType) - Fake data generators
//! - [`Sequence`](factory::Sequence) - Auto-incrementing values
//! - [`FactoryBuilder`](factory::FactoryBuilder) - Fluent factory configuration
//!
//! ## Commands
//!
//! Django-like management commands:
//!
//! - [`LoadDataCommand`](commands::LoadDataCommand) - Load fixtures
//! - [`DumpDataCommand`](commands::DumpDataCommand) - Export fixtures
//! - [`SeedCommand`](commands::SeedCommand) - Create data via factories

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod commands;
pub mod error;
pub mod factory;
pub mod fixtures;
pub mod prelude;

// Re-export commonly used types at crate root
pub use error::{SeedingError, SeedingResult};
pub use factory::{Factory, FactoryExt, FakerType, Sequence};
pub use fixtures::{FixtureData, FixtureFormat, FixtureLoader, FixtureParser, FixtureRecord};

// Re-export derive macro when available
#[cfg(feature = "macros")]
pub use reinhardt_seeding_macros::Factory;
