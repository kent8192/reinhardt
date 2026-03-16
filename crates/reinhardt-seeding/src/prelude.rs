//! Convenience re-exports for common usage.
//!
//! This module provides a single import for the most commonly used items
//! from the reinhardt-seeding crate.
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_seeding::prelude::*;
//!
//! // Now you have access to:
//! // - Factory traits
//! // - Fixture types
//! // - Command types
//! // - Error types
//! ```

// Error types
pub use crate::error::{SeedingError, SeedingResult};

// Fixture types
pub use crate::fixtures::{
	FixtureData, FixtureFormat, FixtureLoader, FixtureParser, FixtureRecord, FixtureSerializer,
	LoadOptions, LoadResult, ModelLoader, ModelRegistry, ModelSerializer,
};

// Factory types
pub use crate::factory::{
	Factory, FactoryBuilder, FactoryExt, FactoryRegistry, FakerType, FieldGenerator, Sequence,
};

// Factory functions
pub use crate::factory::{generate_fake, register_factory, sequence};

// Command types
pub use crate::commands::{
	DumpDataArgs, DumpDataCommand, DumpDataOptions, DumpResult, LoadDataArgs, LoadDataCommand,
	LoadDataOptions, SeedCommand, SeedOptions, SeedResult,
};

// Re-export the Factory derive macro when available
#[cfg(feature = "macros")]
pub use reinhardt_seeding_macros::Factory;
