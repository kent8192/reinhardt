//! Procedural macros for reinhardt-seeding.
//!
//! This crate provides the `#[derive(Factory)]` macro for generating
//! factory implementations.

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod factory_derive;
mod faker_attr;

/// Derives a `Factory` implementation for a struct.
///
/// This macro generates factory code that creates model instances
/// with support for faker data generation, sequences, and default values.
///
/// # Attributes
///
/// ## Struct-level attributes
///
/// - `#[factory(model = ModelType)]` - Specifies the model type to create
///
/// ## Field-level attributes
///
/// - `#[factory(faker = "type")]` - Generate fake data of the specified type
/// - `#[factory(sequence = "format")]` - Generate sequential values with `{n}` placeholder
/// - `#[factory(default = value)]` - Use a default value
/// - `#[factory(skip)]` - Skip this field during factory generation
///
/// # Example
///
/// ```ignore
/// use reinhardt_seeding::Factory;
///
/// #[derive(Factory)]
/// #[factory(model = User)]
/// pub struct UserFactory {
///     #[factory(faker = "username")]
///     pub username: String,
///
///     #[factory(faker = "email")]
///     pub email: String,
///
///     #[factory(sequence = "user_{n}")]
///     pub code: String,
///
///     #[factory(default = true)]
///     pub is_active: bool,
/// }
/// ```
///
/// This generates:
///
/// ```ignore
/// impl Default for UserFactory {
///     fn default() -> Self {
///         Self::new()
///     }
/// }
///
/// impl UserFactory {
///     pub fn new() -> Self {
///         Self {
///             username: reinhardt_seeding::factory::FakerType::Username.generate(),
///             email: reinhardt_seeding::factory::FakerType::Email.generate(),
///             code: reinhardt_seeding::factory::sequence("UserFactory_code", "user_{n}"),
///             is_active: true,
///         }
///     }
/// }
///
/// impl reinhardt_seeding::factory::Factory for UserFactory {
///     type Model = User;
///
///     fn build(&self) -> User {
///         User::new(
///             self.username.clone(),
///             self.email.clone(),
///             self.code.clone(),
///             self.is_active,
///         )
///     }
///
///     async fn create(&self) -> reinhardt_seeding::SeedingResult<User> {
///         let model = self.build();
///         // Persist to database
///         Ok(model)
///     }
///
///     async fn create_batch(&self, count: usize) -> reinhardt_seeding::SeedingResult<Vec<User>> {
///         let mut results = Vec::with_capacity(count);
///         for _ in 0..count {
///             results.push(self.create().await?);
///         }
///         Ok(results)
///     }
/// }
/// ```
#[proc_macro_derive(Factory, attributes(factory))]
pub fn derive_factory(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	factory_derive::derive_factory_impl(input)
		.unwrap_or_else(|err| err.to_compile_error())
		.into()
}
