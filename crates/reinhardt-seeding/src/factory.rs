//! Factory module for programmatic data generation.
//!
//! This module provides a Factory Boy-like pattern for generating test data
//! in Rust. It supports:
//!
//! - Factory traits for building and creating model instances
//! - Faker integration for generating realistic fake data
//! - Sequence generators for unique values
//! - Builder pattern for configuring factory behavior
//! - Global registry for factory discovery
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_seeding::factory::{Factory, FakerType, Sequence};
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
//! // Usage
//! let factory = UserFactory::new();
//! let user = factory.build();
//! let saved_user = factory.create().await?;
//! let batch = factory.create_batch(10).await?;
//! ```

mod builder;
mod faker;
mod registry;
mod sequence;
mod traits;

pub use builder::{BuildConfig, FactoryBuilder, FieldGenerator};
pub use faker::{FakerType, generate_fake};
pub use registry::{
	AnyFactory, FactoryRegistry, clear_factories, factory_count, factory_model_ids, get_factory,
	get_factory_for_type, has_factory, register_factory, register_factory_for_type,
};
pub use sequence::{
	Sequence, clear_sequences, remove_sequence, reset_all_sequences, reset_sequence, sequence,
	sequence_names,
};
pub use traits::{Factory, FactoryExt, LazyEvaluator, LazyFactory, RelatedFactory, SubFactory};
