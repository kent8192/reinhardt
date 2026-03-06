//! CLI commands module.
//!
//! This module provides Django-like management commands for database seeding.
//!
//! # Available Commands
//!
//! - [`loaddata`](LoadDataCommand) - Load fixture data from files into the database
//! - [`dumpdata`](DumpDataCommand) - Export database data to fixture files
//! - [`seed`](SeedCommand) - Create model instances using factories
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_seeding::commands::{
//!     LoadDataCommand, LoadDataArgs, LoadDataOptions,
//!     DumpDataCommand, DumpDataArgs, DumpDataOptions,
//!     SeedCommand, SeedOptions,
//! };
//!
//! // Load fixtures
//! let load_cmd = LoadDataCommand::new();
//! let args = LoadDataArgs {
//!     fixture_paths: vec!["fixtures/users.json".into()],
//! };
//! let result = load_cmd.execute(args, LoadDataOptions::new()).await?;
//!
//! // Dump data
//! let dump_cmd = DumpDataCommand::new();
//! let args = DumpDataArgs {
//!     models: vec!["auth.User".to_string()],
//! };
//! let result = dump_cmd.execute(args, DumpDataOptions::new()).await?;
//!
//! // Seed data
//! let seed_cmd = SeedCommand::new();
//! let options = SeedOptions::new("auth.User").with_count(10);
//! let result = seed_cmd.execute(options).await?;
//! ```

mod dumpdata;
mod loaddata;
mod seed;

pub use dumpdata::{DumpDataArgs, DumpDataCommand, DumpDataOptions, DumpResult};
pub use loaddata::{LoadDataArgs, LoadDataCommand, LoadDataOptions};
pub use seed::{SeedCommand, SeedOptions, SeedResult};
