use std::sync::Arc;

use reinhardt_conf::HasCommonSettings;
use reinhardt_db::orm::{DatabaseConnection, ScopedDatabaseRegistration, install_scoped_database};
use reinhardt_di::{InjectionContext, SingletonScope, global_registry};

use crate::{CommandError, CommandResult};

/// Runtime state owned by one Rust management shell evaluator.
pub struct ShellEnvironment<S> {
	settings: S,
	database: ScopedDatabaseRegistration,
	di: Arc<InjectionContext>,
}

impl<S> ShellEnvironment<S>
where
	S: HasCommonSettings + Send + Sync + 'static,
{
	/// Builds settings, scoped ORM state, and the application singleton DI context.
	pub async fn bootstrap(settings: S) -> CommandResult<Self> {
		let env_database_url = std::env::var("DATABASE_URL").ok();
		let database_url =
			crate::builtin::resolve_database_url(Some(&settings), env_database_url.as_deref())?;
		let database = install_scoped_database(&database_url)
			.await
			.map_err(|error| {
				let category = error
					.database_kind()
					.map(|kind| format!("{kind:?}"))
					.unwrap_or_else(|| "Database".to_string());
				database_initialization_error(&database_url, &category)
			})?;
		let lease = database.lease();
		let connection = database.connection();
		let di = InjectionContext::builder(SingletonScope::new())
			.singleton(lease)
			.singleton(connection)
			.with_registry(Arc::clone(global_registry()))
			.build();

		Ok(Self {
			settings,
			database,
			di: Arc::new(di),
		})
	}

	/// Returns the concrete project settings value.
	pub fn settings(&self) -> &S {
		&self.settings
	}

	/// Returns the shell's copyable ORM database capability.
	pub fn database(&self) -> DatabaseConnection {
		self.database.connection()
	}

	/// Returns the application dependency-injection context.
	pub fn di(&self) -> Arc<InjectionContext> {
		Arc::clone(&self.di)
	}
}

fn database_initialization_error(database_url: &str, category: &str) -> CommandError {
	let scheme = database_url
		.split_once(':')
		.map(|(scheme, _)| scheme)
		.filter(|scheme| {
			!scheme.is_empty()
				&& scheme
					.bytes()
					.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
		})
		.unwrap_or("unknown");
	CommandError::ExecutionError(format!(
		"Failed to initialize shell ORM database (scheme: {scheme}, category: {category})"
	))
}

#[cfg(test)]
mod tests {
	#[test]
	fn database_initialization_error_does_not_expose_url_credentials() {
		let password = "literal-shell-password";
		let database_url = format!("custom://admin:{password}@database.example/app");

		let error = super::database_initialization_error(&database_url, "Unsupported");
		let diagnostic = error.to_string();

		assert_eq!(
			diagnostic,
			"Execution error: Failed to initialize shell ORM database \
			 (scheme: custom, category: Unsupported)"
		);
		assert!(!diagnostic.contains(password));
		assert!(!diagnostic.contains("admin"));
	}
}
