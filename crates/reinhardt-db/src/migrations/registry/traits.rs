//! Migration Registry Trait
//!
//! Provides a unified interface for migration registries (global and local).

use crate::migrations::Migration;

/// Common interface for migration registries.
///
/// This trait provides a unified API for both the global registry (using linkme)
/// and local registries (for testing). This allows tests to use isolated registries
/// while production code uses the global registry.
///
/// # Implementations
///
/// - `GlobalRegistry`: Production registry using `linkme::distributed_slice`
/// - `LocalRegistry`: Test registry using dynamic registration
pub trait MigrationRegistry: Send + Sync {
	/// Returns all registered migrations from all apps.
	///
	/// Migrations are collected from all registered providers and returned.
	/// Note that the order may vary between builds for the global registry,
	/// so you should sort migrations by their dependencies before applying them.
	fn all_migrations(&self) -> Vec<Migration>;

	/// Returns migrations for a specific app label.
	///
	/// # Arguments
	///
	/// * `app_label` - The app label to filter by (e.g., "polls", "users")
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::migrations::registry::MigrationRegistry;
	/// # fn example(registry: &dyn MigrationRegistry) {
	/// let polls_migrations = registry.migrations_for_app("polls");
	/// assert!(polls_migrations.iter().all(|m| m.app_label == "polls"));
	/// # }
	/// ```
	fn migrations_for_app(&self, app_label: &str) -> Vec<Migration>;

	/// Returns all unique app labels that have registered migrations.
	///
	/// The returned vector is sorted and contains no duplicates.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::migrations::registry::MigrationRegistry;
	/// # fn example(registry: &dyn MigrationRegistry) {
	/// let apps = registry.registered_app_labels();
	/// for app in apps {
	///     println!("App with migrations: {}", app);
	/// }
	/// # }
	/// ```
	fn registered_app_labels(&self) -> Vec<String>;

	/// Registers a migration at runtime.
	///
	/// This is primarily intended for testing and dynamic registration scenarios.
	/// The global registry supports this but stores runtime registrations separately
	/// from compile-time linkme registrations.
	///
	/// # Arguments
	///
	/// * `migration` - The migration to register
	///
	/// # Returns
	///
	/// `Ok(())` on success, `Err(String)` if registration fails
	fn register(&self, migration: Migration) -> Result<(), String>;

	/// Clears all runtime-registered migrations.
	///
	/// This is primarily intended for testing to ensure test isolation.
	/// For the global registry, this only clears runtime registrations,
	/// not the compile-time linkme registrations.
	fn clear(&self);
}
