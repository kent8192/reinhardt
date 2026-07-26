//! Model-derived and migration-backed test database fixtures.
//!
//! [`TestDatabase`] creates a ready-to-use database for tests without requiring
//! hand-written table DDL in each test module. Schema can come from [`Model`]
//! metadata or from application migrations.
//! Model-derived schemas retain field defaults, generated columns,
//! relationships, constraints, and indexes.
//!
//! # Model-derived database
//!
//! ```rust,ignore
//! use reinhardt_testkit::fixtures::{TestDatabase, test_database};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let db: TestDatabase = test_database!(WritingProject, Document).await?;
//! let rows = db
//!     .connection()
//!     .fetch_all("SELECT * FROM writing_project", Vec::new())
//!     .await?;
//! assert!(rows.is_empty());
//! # Ok(())
//! # }
//! ```
//!
//! # Migration-backed database
//!
//! ```rust,no_run
//! use reinhardt_db::migrations::{Migration, MigrationProvider};
//! use reinhardt_testkit::fixtures::TestDatabase;
//!
//! struct AppMigrations;
//!
//! impl MigrationProvider for AppMigrations {
//!     fn migrations() -> Vec<Migration> {
//!         Vec::new()
//!     }
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let db = TestDatabase::builder()
//!     .migrations::<AppMigrations>()
//!     .build()
//!     .await?;
//! let _url = db.url();
//! # Ok(())
//! # }
//! ```
//!
//! [`Model`]: reinhardt_db::orm::Model

use std::path::PathBuf;

use reinhardt_db::backends::types::DatabaseType;
use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
use reinhardt_db::migrations::{FilesystemSource, Migration, MigrationSource};
use thiserror::Error;

use crate::fixtures::schema::ModelSchemaInfo;

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Database backend used by a test database fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TestDatabaseBackend {
	/// Create a temporary SQLite database file.
	SqliteFile,
	/// Create an in-memory SQLite database.
	SqliteMemory,
	/// Create a PostgreSQL database.
	Postgres,
}

/// Error returned while configuring or creating a `TestDatabase`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TestDatabaseError {
	/// The builder was configured with missing or contradictory options.
	#[error("invalid test database configuration: {message}")]
	InvalidConfiguration {
		/// Human-readable configuration error.
		message: &'static str,
	},
	/// Model metadata could not be converted into migration operations.
	#[error("failed to compile model schema into test migration: {source}")]
	SchemaCompilation {
		/// Source schema compilation error.
		#[source]
		source: crate::fixtures::schema::SchemaError,
	},
	/// Migrations could not be loaded from the selected source.
	#[error("failed to load migrations from {source_name}: {source}")]
	MigrationLoad {
		/// Human-readable migration source name.
		source_name: String,
		/// Source migration loading error.
		#[source]
		source: BoxError,
	},
	/// Database backend resource startup failed.
	#[error("failed to start {backend:?} test database: {source}")]
	BackendStartup {
		/// Selected backend.
		backend: TestDatabaseBackend,
		/// Startup source error.
		#[source]
		source: BoxError,
	},
	/// Migration application failed.
	#[error("failed to apply test database migrations: {source}")]
	MigrationApply {
		/// Migration executor error.
		#[source]
		source: BoxError,
	},
	/// ORM global connection initialization failed.
	#[error("failed to initialize ORM global database connection: {source}")]
	OrmGlobalInit {
		/// Source ORM initialization error.
		#[source]
		source: BoxError,
	},
	/// DI context setup failed.
	#[error("failed to initialize test database DI context: {source}")]
	DiContextInit {
		/// Source DI setup error.
		#[source]
		source: BoxError,
	},
}

enum TestDatabaseSchemaSource {
	Models(Vec<ModelSchemaInfo>),
	Provider(fn() -> Vec<Migration>),
	Filesystem(PathBuf),
}

/// Builder for model-derived and migration-backed test database fixtures.
pub struct TestDatabaseBuilder {
	backend: TestDatabaseBackend,
	schema_source: Option<TestDatabaseSchemaSource>,
	schema_source_conflict: bool,
	orm_global: bool,
	di_context: bool,
}

impl TestDatabaseBuilder {
	/// Create a new test database builder.
	pub fn new() -> Self {
		Self {
			backend: TestDatabaseBackend::SqliteFile,
			schema_source: None,
			schema_source_conflict: false,
			orm_global: false,
			di_context: false,
		}
	}

	/// Return the configured database backend.
	pub fn backend(&self) -> TestDatabaseBackend {
		self.backend
	}

	/// Use a temporary SQLite file database.
	pub fn sqlite(mut self) -> Self {
		self.backend = TestDatabaseBackend::SqliteFile;
		self
	}

	/// Use an in-memory SQLite database.
	pub fn sqlite_memory(mut self) -> Self {
		self.backend = TestDatabaseBackend::SqliteMemory;
		self
	}

	/// Use a PostgreSQL testcontainer database.
	#[cfg(feature = "testcontainers")]
	pub fn postgres(mut self) -> Self {
		self.backend = TestDatabaseBackend::Postgres;
		self
	}

	/// Use schema metadata derived from a model type.
	pub fn model<M: reinhardt_db::orm::Model>(self) -> Self {
		self.model_info(ModelSchemaInfo::from_model::<M>())
	}

	/// Add model schema metadata as the schema source.
	pub fn model_info(mut self, model_info: ModelSchemaInfo) -> Self {
		if let Some(TestDatabaseSchemaSource::Models(models)) = &mut self.schema_source {
			models.push(model_info);
			return self;
		}

		self.set_schema_source(TestDatabaseSchemaSource::Models(vec![model_info]))
	}

	/// Use migrations provided by a migration provider type.
	pub fn migrations<P: reinhardt_db::migrations::MigrationProvider + 'static>(self) -> Self {
		self.migrations_from_provider(P::migrations)
	}

	fn migrations_from_provider(self, load: fn() -> Vec<Migration>) -> Self {
		self.set_schema_source(TestDatabaseSchemaSource::Provider(load))
	}

	/// Use migrations loaded from a filesystem directory.
	pub fn migrations_from_dir(self, path: impl Into<PathBuf>) -> Self {
		self.set_schema_source(TestDatabaseSchemaSource::Filesystem(path.into()))
	}

	/// Initialize Reinhardt ORM global database state for this test database.
	///
	/// This mutates process-global ORM state and is intended for tests that need
	/// legacy global ORM access. Prefer [`Self::with_di_context`] for isolated
	/// dependency injection when possible.
	pub fn with_orm_global(mut self) -> Self {
		self.orm_global = true;
		self
	}

	/// Create a DI context with the ORM database connection registered.
	pub fn with_di_context(mut self) -> Self {
		self.di_context = true;
		self
	}

	/// Validate that the builder has exactly one schema source.
	pub fn validate(&self) -> Result<(), TestDatabaseError> {
		let has_schema_source = self.schema_source.is_some();

		if !has_schema_source || self.schema_source_conflict {
			return Err(TestDatabaseError::InvalidConfiguration {
				message: "test database requires exactly one schema source",
			});
		}

		if self.backend == TestDatabaseBackend::SqliteMemory && (self.orm_global || self.di_context)
		{
			return Err(TestDatabaseError::InvalidConfiguration {
				message: "sqlite memory backend cannot initialize ORM global or DI context",
			});
		}

		Ok(())
	}

	pub(crate) async fn resolve_migrations(&self) -> Result<Vec<Migration>, TestDatabaseError> {
		self.validate()?;

		match self
			.schema_source
			.as_ref()
			.expect("validated schema source")
		{
			TestDatabaseSchemaSource::Models(models) => models_to_migration(models),
			TestDatabaseSchemaSource::Provider(load) => Ok(load()),
			TestDatabaseSchemaSource::Filesystem(path) => {
				let source = FilesystemSource::new(path);
				source
					.all_migrations()
					.await
					.map_err(|source| TestDatabaseError::MigrationLoad {
						source_name: path.display().to_string(),
						source: Box::new(source),
					})
			}
		}
	}

	/// Build the configured test database.
	pub async fn build(self) -> Result<TestDatabase, TestDatabaseError> {
		self.validate()?;
		let migrations = self.resolve_migrations().await?;

		let (connection, url, resource) = match self.backend {
			TestDatabaseBackend::SqliteFile => create_sqlite_file_database().await?,
			TestDatabaseBackend::SqliteMemory => create_sqlite_memory_database().await?,
			TestDatabaseBackend::Postgres => {
				#[cfg(feature = "testcontainers")]
				{
					create_postgres_database().await?
				}
				#[cfg(not(feature = "testcontainers"))]
				{
					return Err(TestDatabaseError::InvalidConfiguration {
						message: "postgres backend requires the testcontainers feature",
					});
				}
			}
		};

		apply_migrations(&connection, &migrations).await?;

		let di_context = if self.di_context {
			Some(create_di_context(connection.clone())?)
		} else {
			None
		};

		let orm_global_restore = if self.orm_global {
			let lease = reinhardt_db::orm::connection::DatabaseConnectionLease::register(
				connection.clone(),
			)
			.map_err(|source| TestDatabaseError::OrmGlobalInit {
				source: Box::new(source),
			})?;
			let previous =
				reinhardt_db::orm::manager::replace_database_connection_for_testing(Some(lease))
					.await;

			Some(OrmGlobalRestore { previous })
		} else {
			None
		};

		Ok(TestDatabase {
			database_type: connection.database_type(),
			connection,
			url,
			di_context,
			orm_global_restore,
			_resource: resource,
		})
	}

	#[cfg(test)]
	fn resolve_migrations_for_test(&self) -> Result<Vec<Migration>, TestDatabaseError> {
		self.validate()?;

		match self
			.schema_source
			.as_ref()
			.expect("validated schema source")
		{
			TestDatabaseSchemaSource::Models(models) => models_to_migration(models),
			TestDatabaseSchemaSource::Provider(load) => Ok(load()),
			TestDatabaseSchemaSource::Filesystem(path) => Err(TestDatabaseError::MigrationLoad {
				source_name: path.display().to_string(),
				source: Box::new(std::io::Error::new(
					std::io::ErrorKind::Unsupported,
					"filesystem migration loading is async",
				)),
			}),
		}
	}

	fn set_schema_source(mut self, schema_source: TestDatabaseSchemaSource) -> Self {
		if self.schema_source.is_some() {
			self.schema_source_conflict = true;
		}

		self.schema_source = Some(schema_source);
		self
	}
}

fn models_to_migration(models: &[ModelSchemaInfo]) -> Result<Vec<Migration>, TestDatabaseError> {
	let cloned_models = models
		.iter()
		.map(|model| ModelSchemaInfo {
			name: model.name.clone(),
			table_name: model.table_name.clone(),
			app_label: model.app_label.clone(),
			fields: model.fields.clone(),
			relationships: model.relationships.clone(),
			constraints: model.constraints.clone(),
			indexes: model.indexes.clone(),
		})
		.collect();

	let operations = crate::fixtures::schema::create_table_operations_from_models(cloned_models)
		.map_err(|source| TestDatabaseError::SchemaCompilation { source })?;

	Ok(vec![Migration {
		name: "0001_test_database_schema".to_string(),
		app_label: "test_database".to_string(),
		operations,
		dependencies: Vec::new(),
		replaces: Vec::new(),
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		optional_dependencies: Vec::new(),
		swappable_dependencies: Vec::new(),
	}])
}

async fn create_sqlite_file_database() -> Result<
	(
		reinhardt_db::backends::DatabaseConnection,
		String,
		TestDatabaseResource,
	),
	TestDatabaseError,
> {
	let temp_file =
		tempfile::NamedTempFile::new().map_err(|source| TestDatabaseError::BackendStartup {
			backend: TestDatabaseBackend::SqliteFile,
			source: Box::new(source),
		})?;
	let path = temp_file.path().to_string_lossy().replace('\\', "/");
	let url = format!("sqlite:///{}", path);
	let connection = reinhardt_db::backends::DatabaseConnection::connect_sqlite(&url)
		.await
		.map_err(|source| TestDatabaseError::BackendStartup {
			backend: TestDatabaseBackend::SqliteFile,
			source: Box::new(source),
		})?;

	Ok((connection, url, TestDatabaseResource::SqliteFile(temp_file)))
}

async fn create_sqlite_memory_database() -> Result<
	(
		reinhardt_db::backends::DatabaseConnection,
		String,
		TestDatabaseResource,
	),
	TestDatabaseError,
> {
	let url = "sqlite::memory:".to_string();
	let connection = reinhardt_db::backends::DatabaseConnection::connect_sqlite(&url)
		.await
		.map_err(|source| TestDatabaseError::BackendStartup {
			backend: TestDatabaseBackend::SqliteMemory,
			source: Box::new(source),
		})?;

	Ok((connection, url, TestDatabaseResource::SqliteMemory))
}

#[cfg(feature = "testcontainers")]
async fn create_postgres_database() -> Result<
	(
		reinhardt_db::backends::DatabaseConnection,
		String,
		TestDatabaseResource,
	),
	TestDatabaseError,
> {
	use testcontainers::core::{IntoContainerPort, WaitFor};
	use testcontainers::runners::AsyncRunner;
	use testcontainers::{GenericImage, ImageExt};

	let image = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust");

	let container = image
		.start()
		.await
		.map_err(|source| TestDatabaseError::BackendStartup {
			backend: TestDatabaseBackend::Postgres,
			source: Box::new(source),
		})?;

	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	let mut port_retry = 0;
	let max_port_retries = 7;
	let port = loop {
		match container.get_host_port_ipv4(5432).await {
			Ok(port) => break port,
			Err(source) if port_retry < max_port_retries => {
				port_retry += 1;
				let delay = tokio::time::Duration::from_millis(200 * 2_u64.pow(port_retry));
				eprintln!(
					"PostgreSQL port query attempt {port_retry} of {max_port_retries} failed: {source}"
				);
				tokio::time::sleep(delay).await;
			}
			Err(source) => {
				return Err(TestDatabaseError::BackendStartup {
					backend: TestDatabaseBackend::Postgres,
					source: Box::new(source),
				});
			}
		}
	};

	let url = format!("postgres://postgres@localhost:{port}/postgres?sslmode=disable");
	let connection = connect_postgres_when_ready(&url).await?;

	Ok((
		connection,
		url,
		TestDatabaseResource::Postgres(Box::new(container)),
	))
}

#[cfg(feature = "testcontainers")]
async fn connect_postgres_when_ready(
	url: &str,
) -> Result<reinhardt_db::backends::DatabaseConnection, TestDatabaseError> {
	let mut retry_count = 0;
	let max_retries = 7;

	loop {
		let connection =
			match reinhardt_db::backends::DatabaseConnection::connect_postgres(url).await {
				Ok(connection) => connection,
				Err(source) if retry_count < max_retries => {
					retry_count += 1;
					let delay = tokio::time::Duration::from_millis(200 * 2_u64.pow(retry_count));
					eprintln!(
						"PostgreSQL connection attempt {retry_count} of {max_retries} failed: {source}"
					);
					tokio::time::sleep(delay).await;
					continue;
				}
				Err(source) => {
					return Err(TestDatabaseError::BackendStartup {
						backend: TestDatabaseBackend::Postgres,
						source: Box::new(source),
					});
				}
			};

		match connection.fetch_one("SELECT 1", Vec::new()).await {
			Ok(_) => return Ok(connection),
			Err(source) if retry_count < max_retries => {
				retry_count += 1;
				let delay = tokio::time::Duration::from_millis(200 * 2_u64.pow(retry_count));
				eprintln!(
					"PostgreSQL health check attempt {retry_count} of {max_retries} failed: {source}"
				);
				tokio::time::sleep(delay).await;
			}
			Err(source) => {
				return Err(TestDatabaseError::BackendStartup {
					backend: TestDatabaseBackend::Postgres,
					source: Box::new(source),
				});
			}
		}
	}
}

async fn apply_migrations(
	connection: &reinhardt_db::backends::DatabaseConnection,
	migrations: &[Migration],
) -> Result<(), TestDatabaseError> {
	if migrations.is_empty() {
		return Ok(());
	}

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(migrations)
		.await
		.map_err(|source| TestDatabaseError::MigrationApply {
			source: Box::new(source),
		})?;

	Ok(())
}

fn create_di_context(
	owner: reinhardt_db::backends::DatabaseConnection,
) -> Result<reinhardt_di::InjectionContext, TestDatabaseError> {
	let lease = reinhardt_db::orm::connection::DatabaseConnectionLease::register(owner).map_err(
		|source| TestDatabaseError::DiContextInit {
			source: Box::new(source),
		},
	)?;
	let orm_connection = lease.handle();
	let singleton_scope = std::sync::Arc::new(reinhardt_di::SingletonScope::new());
	singleton_scope.set(lease);
	singleton_scope.set(orm_connection);

	Ok(reinhardt_di::InjectionContext::builder(singleton_scope).build())
}

impl Default for TestDatabaseBuilder {
	fn default() -> Self {
		Self::new()
	}
}

enum TestDatabaseResource {
	#[allow(dead_code)] // The file handle is intentionally held for RAII cleanup.
	SqliteFile(tempfile::NamedTempFile),
	SqliteMemory,
	#[cfg(feature = "testcontainers")]
	#[allow(dead_code)] // The container handle is intentionally held for RAII cleanup.
	Postgres(Box<testcontainers::ContainerAsync<testcontainers::GenericImage>>),
}

struct OrmGlobalRestore {
	previous: Option<reinhardt_db::orm::connection::DatabaseConnectionLease>,
}

impl OrmGlobalRestore {
	fn restore(self) {
		let previous = self.previous;
		let result = std::thread::Builder::new()
			.name("reinhardt-testdb-orm-restore".to_string())
			.spawn(move || {
				match tokio::runtime::Builder::new_current_thread()
					.enable_all()
					.build()
				{
					Ok(runtime) => {
						runtime.block_on(async move {
							reinhardt_db::orm::manager::replace_database_connection_for_testing(
								previous,
							)
							.await;
						});
					}
					Err(error) => {
						eprintln!(
							"Warning: failed to build runtime for ORM global restore: {error}"
						);
					}
				}
			})
			.and_then(|handle| {
				handle
					.join()
					.map_err(|_| std::io::Error::other("ORM global restore thread panicked"))
			});

		if let Err(error) = result {
			eprintln!("Warning: failed to restore ORM global database connection: {error}");
		}
	}
}

/// RAII guard for a test database.
pub struct TestDatabase {
	connection: reinhardt_db::backends::DatabaseConnection,
	database_type: DatabaseType,
	url: String,
	di_context: Option<reinhardt_di::InjectionContext>,
	orm_global_restore: Option<OrmGlobalRestore>,
	_resource: TestDatabaseResource,
}

impl Drop for TestDatabase {
	fn drop(&mut self) {
		if let Some(restore) = self.orm_global_restore.take() {
			restore.restore();
		}
	}
}

impl TestDatabase {
	/// Create a new test database builder.
	pub fn builder() -> TestDatabaseBuilder {
		TestDatabaseBuilder::new()
	}

	/// Return the database connection.
	pub fn connection(&self) -> &reinhardt_db::backends::DatabaseConnection {
		&self.connection
	}

	/// Return the selected database type.
	pub fn database_type(&self) -> DatabaseType {
		self.database_type
	}

	/// Return the database URL.
	///
	/// For [`TestDatabaseBackend::SqliteMemory`], this returns `sqlite::memory:`.
	/// That URL is connection-local: opening a second connection with it creates
	/// a separate empty in-memory database, not another handle to this migrated
	/// database. Use [`Self::connection`] to access the built database.
	pub fn url(&self) -> &str {
		&self.url
	}

	/// Return the optional dependency injection context.
	///
	/// The returned context is intended to be used while the [`TestDatabase`]
	/// guard is alive because the guard owns the backing database resource.
	pub fn di_context(&self) -> Option<&reinhardt_di::InjectionContext> {
		self.di_context.as_ref()
	}
}

/// Create a test database builder fixture.
#[rstest::fixture]
pub fn test_database() -> TestDatabaseBuilder {
	TestDatabase::builder()
}

/// Build a `TestDatabase` from model types or a migration provider.
#[macro_export]
macro_rules! test_database {
	(migrations = $provider:ty, orm_global = true $(,)?) => {{
		$crate::fixtures::TestDatabase::builder()
			.migrations::<$provider>()
			.with_orm_global()
			.build()
	}};
	(migrations = $provider:ty $(,)?) => {{
		$crate::fixtures::TestDatabase::builder()
			.migrations::<$provider>()
			.build()
	}};
	(backend = postgres, migrations = $provider:ty, orm_global = true $(,)?) => {{
		$crate::fixtures::TestDatabase::builder()
			.postgres()
			.migrations::<$provider>()
			.with_orm_global()
			.build()
	}};
	(backend = postgres, migrations = $provider:ty $(,)?) => {{
		$crate::fixtures::TestDatabase::builder()
			.postgres()
			.migrations::<$provider>()
			.build()
	}};
	(backend = postgres, $($model:ty),+ $(,)?) => {{
		let builder = $crate::fixtures::TestDatabase::builder().postgres();
		$(let builder = builder.model::<$model>();)+
		builder.build()
	}};
	($($model:ty),+ $(,)?) => {{
		let builder = $crate::fixtures::TestDatabase::builder();
		$(let builder = builder.model::<$model>();)+
		builder.build()
	}};
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_db::orm::inspection::FieldInfo;
	use reinhardt_db::orm::model::FieldSelector;
	use reinhardt_db::orm::{Manager, Model};
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;

	struct EmptyProvider;

	impl reinhardt_db::migrations::MigrationProvider for EmptyProvider {
		fn migrations() -> Vec<Migration> {
			vec![Migration {
				name: "0001_empty".to_string(),
				app_label: "empty_provider".to_string(),
				operations: Vec::new(),
				dependencies: Vec::new(),
				replaces: Vec::new(),
				atomic: true,
				initial: Some(true),
				state_only: false,
				database_only: false,
				optional_dependencies: Vec::new(),
				swappable_dependencies: Vec::new(),
			}]
		}
	}

	#[derive(Clone)]
	struct TestUserFields;

	impl FieldSelector for TestUserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestUser {
		id: Option<i64>,
		name: String,
	}

	impl Model for TestUser {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"test_users"
		}

		fn app_label() -> &'static str {
			"test_database"
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn field_metadata() -> Vec<FieldInfo> {
			vec![
				FieldInfo {
					name: "id".to_string(),
					field_type: "IntegerField".to_string(),
					storage_kind: None,
					domain: None,
					nullable: false,
					primary_key: true,
					unique: false,
					blank: false,
					editable: true,
					default: None,
					db_default: None,
					db_column: None,
					choices: None,
					attributes: HashMap::new(),
				},
				FieldInfo {
					name: "name".to_string(),
					field_type: "CharField".to_string(),
					storage_kind: None,
					domain: None,
					nullable: false,
					primary_key: false,
					unique: false,
					blank: false,
					editable: true,
					default: None,
					db_default: None,
					db_column: None,
					choices: None,
					attributes: HashMap::from([(
						"max_length".to_string(),
						reinhardt_db::orm::fields::FieldKwarg::Int(100),
					)]),
				},
			]
		}

		fn index_metadata() -> Vec<reinhardt_db::orm::inspection::IndexInfo> {
			vec![reinhardt_db::orm::inspection::IndexInfo::new(
				"test_users_name_idx",
				vec!["name".to_string()],
				false,
				Some("name IS NOT NULL".to_string()),
			)]
		}
	}

	#[rstest]
	fn builder_requires_one_schema_source() {
		let err = TestDatabase::builder().validate().unwrap_err();

		assert!(matches!(
			err,
			TestDatabaseError::InvalidConfiguration { message }
				if message == "test database requires exactly one schema source"
		));
	}

	#[rstest]
	fn builder_rejects_multiple_schema_sources() {
		let builder = TestDatabase::builder()
			.model_info(crate::fixtures::schema::ModelSchemaInfo {
				name: "User".to_string(),
				table_name: "users".to_string(),
				app_label: "test".to_string(),
				fields: Vec::new(),
				relationships: Vec::new(),
				constraints: Vec::new(),
				indexes: Vec::new(),
			})
			.migrations_from_provider(Vec::new);

		let err = builder.validate().unwrap_err();

		assert!(matches!(
			err,
			TestDatabaseError::InvalidConfiguration { message }
				if message == "test database requires exactly one schema source"
		));
	}

	#[rstest]
	fn builder_defaults_to_sqlite_file_backend() {
		let builder = TestDatabase::builder();

		assert_eq!(builder.backend(), TestDatabaseBackend::SqliteFile);
	}

	#[rstest]
	fn model_source_resolves_to_single_synthetic_migration() {
		let model = crate::fixtures::schema::ModelSchemaInfo {
			name: "User".to_string(),
			table_name: "users".to_string(),
			app_label: "accounts".to_string(),
			fields: Vec::new(),
			relationships: Vec::new(),
			constraints: Vec::new(),
			indexes: Vec::new(),
		};
		let builder = TestDatabase::builder().model_info(model);

		let migrations = builder.resolve_migrations_for_test().unwrap();

		assert_eq!(migrations.len(), 1);
		assert_eq!(migrations[0].name, "0001_test_database_schema");
		assert_eq!(migrations[0].app_label, "test_database");
		assert!(migrations[0].initial.unwrap());
	}

	#[rstest]
	fn model_source_preserves_model_indexes() {
		let builder = TestDatabase::builder().model::<TestUser>();

		let migrations = builder.resolve_migrations_for_test().unwrap();

		let operation = migrations[0]
			.operations
			.iter()
			.find(|operation| {
				matches!(
					operation,
					reinhardt_db::migrations::Operation::CreateIndex { .. }
				)
			})
			.expect("model index operation");
		match operation {
			reinhardt_db::migrations::Operation::CreateIndex {
				table,
				columns,
				index_type,
				where_clause,
				expressions,
				operator_class,
				..
			} => {
				assert_eq!(table, "test_users");
				assert_eq!(columns, &["name".to_string()]);
				assert_eq!(where_clause.as_deref(), Some("name IS NOT NULL"));
				assert_eq!(*index_type, None);
				assert_eq!(expressions, &None);
				assert_eq!(operator_class, &None);
			}
			other => panic!("expected CreateIndex, got {other:?}"),
		}
	}

	#[rstest]
	fn provider_source_resolves_provider_migrations() {
		fn provider_migrations() -> Vec<Migration> {
			vec![Migration {
				name: "0001_initial".to_string(),
				app_label: "provider_app".to_string(),
				operations: Vec::new(),
				dependencies: Vec::new(),
				replaces: Vec::new(),
				atomic: true,
				initial: Some(true),
				state_only: false,
				database_only: false,
				optional_dependencies: Vec::new(),
				swappable_dependencies: Vec::new(),
			}]
		}

		let builder = TestDatabase::builder().migrations_from_provider(provider_migrations);

		let migrations = builder.resolve_migrations_for_test().unwrap();

		assert_eq!(migrations.len(), 1);
		assert_eq!(migrations[0].app_label, "provider_app");
		assert_eq!(migrations[0].name, "0001_initial");
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_provider_database_builds() {
		let db = TestDatabase::builder()
			.migrations::<EmptyProvider>()
			.build()
			.await
			.unwrap();

		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Sqlite
		);
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_filesystem_missing_directory_builds_empty_database() {
		let temp_dir = tempfile::tempdir().unwrap();
		let missing_dir = temp_dir.path().join("missing-migrations");

		let db = TestDatabase::builder()
			.migrations_from_dir(&missing_dir)
			.build()
			.await
			.unwrap();

		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Sqlite
		);
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_filesystem_database_builds_from_empty_migration() {
		let temp_dir = tempfile::tempdir().unwrap();
		let app_dir = temp_dir.path().join("filesystem_app");
		std::fs::create_dir_all(&app_dir).unwrap();
		std::fs::write(
			app_dir.join("0001_initial.rs"),
			r#"
use reinhardt_db::migrations::prelude::*;

pub fn migration() -> Migration {
    Migration {
        app_label: "filesystem_app",
        name: "0001_initial",
        operations: Vec::new(),
        dependencies: Vec::new(),
        atomic: true,
        replaces: Vec::new(),
    }
}
"#,
		)
		.unwrap();

		let db = TestDatabase::builder()
			.migrations_from_dir(temp_dir.path())
			.build()
			.await
			.unwrap();

		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Sqlite
		);

		let applied_rows = db
			.connection()
			.fetch_all(
				"SELECT app, name FROM reinhardt_migrations \
				 WHERE app = 'filesystem_app' AND name = '0001_initial'",
				Vec::new(),
			)
			.await
			.unwrap();

		assert_eq!(applied_rows.len(), 1);
		assert_eq!(
			applied_rows[0].get::<String>("app").unwrap(),
			"filesystem_app"
		);
		assert_eq!(
			applied_rows[0].get::<String>("name").unwrap(),
			"0001_initial"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_memory_rejects_orm_global() {
		let err = match TestDatabase::builder()
			.sqlite_memory()
			.migrations::<EmptyProvider>()
			.with_orm_global()
			.build()
			.await
		{
			Ok(_) => panic!("sqlite memory with ORM global should fail"),
			Err(err) => err,
		};

		assert!(matches!(
			err,
			TestDatabaseError::InvalidConfiguration { message }
				if message == "sqlite memory backend cannot initialize ORM global or DI context"
		));
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_memory_rejects_di_context() {
		let err = match TestDatabase::builder()
			.sqlite_memory()
			.migrations::<EmptyProvider>()
			.with_di_context()
			.build()
			.await
		{
			Ok(_) => panic!("sqlite memory with DI context should fail"),
			Err(err) => err,
		};

		assert!(matches!(
			err,
			TestDatabaseError::InvalidConfiguration { message }
				if message == "sqlite memory backend cannot initialize ORM global or DI context"
		));
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_memory_database_uses_connection_local_url() {
		let db = TestDatabase::builder()
			.sqlite_memory()
			.model::<TestUser>()
			.build()
			.await
			.unwrap();

		assert_eq!(db.url(), "sqlite::memory:");

		let rows = db
			.connection()
			.fetch_all("SELECT id, name FROM test_users", Vec::new())
			.await
			.unwrap();

		assert!(rows.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn sqlite_model_database_creates_table() {
		let db = TestDatabase::builder()
			.model::<TestUser>()
			.build()
			.await
			.unwrap();

		let rows = db
			.connection()
			.fetch_all("SELECT id, name FROM test_users", Vec::new())
			.await
			.unwrap();

		assert!(rows.is_empty());
		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Sqlite
		);
		assert!(db.url().starts_with("sqlite:///"));
	}

	#[cfg(feature = "testcontainers")]
	#[rstest]
	#[tokio::test]
	async fn postgres_model_database_creates_table() {
		let db = TestDatabase::builder()
			.postgres()
			.model::<TestUser>()
			.build()
			.await
			.unwrap();

		let rows = db
			.connection()
			.fetch_all("SELECT id, name FROM test_users", Vec::new())
			.await
			.unwrap();

		assert!(rows.is_empty());
		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Postgres
		);
		assert!(db.url().starts_with("postgres://"));
	}

	#[rstest]
	#[tokio::test]
	async fn with_di_context_registers_orm_database_connection() {
		let db = TestDatabase::builder()
			.migrations::<EmptyProvider>()
			.with_di_context()
			.build()
			.await
			.unwrap();

		let ctx = db.di_context().expect("DI context should be present");
		let registered = ctx
			.singleton_scope()
			.get::<reinhardt_db::orm::connection::DatabaseConnection>();

		assert!(registered.is_some());
	}

	#[rstest]
	#[serial_test::serial(test_database_orm_global)]
	#[tokio::test]
	async fn with_orm_global_initializes_global_connection() {
		let db = TestDatabase::builder()
			.migrations::<EmptyProvider>()
			.with_orm_global()
			.build()
			.await
			.unwrap();

		let global = reinhardt_db::orm::get_connection().await.unwrap();
		assert_eq!(
			global.backend(),
			reinhardt_db::orm::connection::DatabaseBackend::Sqlite
		);
		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Sqlite
		);
	}

	#[rstest]
	#[serial_test::serial(test_database_orm_global)]
	#[tokio::test]
	async fn with_orm_global_restores_previous_state_on_drop() {
		let previous =
			reinhardt_db::orm::manager::replace_database_connection_for_testing(None).await;

		{
			let _db = TestDatabase::builder()
				.migrations::<EmptyProvider>()
				.with_orm_global()
				.build()
				.await
				.unwrap();

			assert!(reinhardt_db::orm::get_connection().await.is_ok());
		}

		let after_drop = reinhardt_db::orm::get_connection().await;
		reinhardt_db::orm::manager::replace_database_connection_for_testing(previous).await;

		assert!(after_drop.is_err());
	}

	#[rstest]
	#[serial_test::serial(test_database_orm_global)]
	#[tokio::test]
	async fn with_orm_global_restores_previous_connection_on_drop() {
		let owner = reinhardt_db::backends::DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		let previous =
			reinhardt_db::orm::connection::DatabaseConnectionLease::register(owner).unwrap();
		let previous_handle = previous.handle();
		previous_handle
			.execute(
				"CREATE TABLE previous_marker (id INTEGER PRIMARY KEY)",
				Vec::new(),
			)
			.await
			.unwrap();

		let original =
			reinhardt_db::orm::manager::replace_database_connection_for_testing(Some(previous))
				.await;

		{
			let _db = TestDatabase::builder()
				.migrations::<EmptyProvider>()
				.with_orm_global()
				.build()
				.await
				.unwrap();

			let current = reinhardt_db::orm::get_connection().await.unwrap();
			let rows = current
				.query(
					"SELECT name FROM sqlite_master \
					 WHERE type = 'table' AND name = 'previous_marker'",
					Vec::new(),
				)
				.await
				.unwrap();
			assert!(rows.is_empty());
		}

		let restored = reinhardt_db::orm::get_connection().await.unwrap();
		let rows = restored
			.query(
				"SELECT name FROM sqlite_master \
				 WHERE type = 'table' AND name = 'previous_marker'",
				Vec::new(),
			)
			.await
			.unwrap();

		reinhardt_db::orm::manager::replace_database_connection_for_testing(original).await;

		assert_eq!(rows.len(), 1);
	}

	#[rstest]
	#[serial_test::serial(test_database_orm_global)]
	#[tokio::test]
	async fn init_database_reinitializes_after_orm_global_fixture_drop() {
		let previous =
			reinhardt_db::orm::manager::replace_database_connection_for_testing(None).await;

		{
			let _db = TestDatabase::builder()
				.migrations::<EmptyProvider>()
				.with_orm_global()
				.build()
				.await
				.unwrap();
		}

		assert!(reinhardt_db::orm::get_connection().await.is_err());

		reinhardt_db::orm::manager::init_database("sqlite::memory:")
			.await
			.unwrap();
		let initialized = reinhardt_db::orm::get_connection().await.unwrap();
		let backend = initialized.backend();

		reinhardt_db::orm::manager::replace_database_connection_for_testing(previous).await;

		assert_eq!(
			backend,
			reinhardt_db::orm::connection::DatabaseBackend::Sqlite
		);
	}

	#[rstest]
	#[tokio::test]
	async fn macro_model_mode_builds_sqlite_database() {
		let db = crate::test_database!(TestUser).await.unwrap();

		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Sqlite
		);
	}

	#[rstest]
	#[tokio::test]
	async fn macro_provider_mode_builds_sqlite_database() {
		let db = crate::test_database!(migrations = EmptyProvider)
			.await
			.unwrap();

		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Sqlite
		);
	}

	#[rstest]
	#[serial_test::serial(test_database_orm_global)]
	#[tokio::test]
	async fn macro_provider_mode_accepts_orm_global_flag() {
		let db = crate::test_database!(migrations = EmptyProvider, orm_global = true,)
			.await
			.unwrap();

		assert_eq!(
			db.database_type(),
			reinhardt_db::backends::types::DatabaseType::Sqlite
		);
	}
}
