//! End-to-end coverage for storage-backed ORM file fields.

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use reinhardt::db::migrations::{
	FieldMetadata, FieldType, MigrationAutodetector, ModelMetadata, Operation, ProjectState,
	SqlDialect, global_registry,
};
use reinhardt::db::orm::manager::{get_connection, reinitialize_database};
use reinhardt::db::orm::{FileField, Model};
use reinhardt::file_storage::{
	FileStorageError, StorageBackend, StorageCapabilities, StorageEntry, StorageError,
	StorageRegistry, StorageSettings, active_storage_registry, initialize,
	initialize_with_model_registry,
};
use reinhardt::model;
use reinhardt_core::parsers::UploadedFile;
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

#[model(app_label = "file_field_tests", table_name = "file_field_profiles")]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct Profile {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(
		upload_to = "avatars/%Y/%m/%d",
		file_storage = "default",
		max_length = 255
	)]
	avatar: FileField,
}

fn sqlite_model_table_sql() -> String {
	let metadata = global_registry()
		.get_model("file_field_tests", "Profile")
		.expect("FileField model metadata should be registered");
	let mut target = ProjectState::new();
	target.add_model(metadata.to_model_state());
	let operation = MigrationAutodetector::new(ProjectState::new(), target)
		.generate_operations()
		.into_iter()
		.find(|operation| matches!(operation, Operation::CreateTable { .. }))
		.expect("FileField model should produce a CREATE TABLE operation");
	operation.to_sql(&SqlDialect::Sqlite)
}

fn local_storage_settings(directory: &TempDir) -> StorageSettings {
	let mut settings = StorageSettings::default();
	settings
		.local
		.as_mut()
		.expect("file-storage-local should expose local settings")
		.base_path = directory.path().display().to_string();
	settings
}

fn missing_alias_model_registry() -> reinhardt::db::migrations::ModelRegistry {
	let registry = reinhardt::db::migrations::ModelRegistry::new();
	let mut model = ModelMetadata::new("file_field_tests", "MissingAlias", "missing_alias");
	let field = FieldMetadata::new(FieldType::VarChar(255))
		.with_param("model_field_type", "file")
		.with_param("file_storage", "missing_alias");
	model.add_field("avatar".to_owned(), field);
	registry.register_model(model);
	registry
}

struct NoExclusiveCreateBackend;

#[async_trait]
impl StorageBackend for NoExclusiveCreateBackend {
	async fn save(&self, _name: &str, _content: &[u8]) -> Result<String, StorageError> {
		Err(StorageError::UnsupportedOperation(
			"test backend".to_owned(),
		))
	}

	async fn open(&self, _name: &str) -> Result<Vec<u8>, StorageError> {
		Err(StorageError::NotFound("test backend".to_owned()))
	}

	async fn delete(&self, _name: &str) -> Result<(), StorageError> {
		Ok(())
	}

	async fn exists(&self, _name: &str) -> Result<bool, StorageError> {
		Ok(false)
	}

	async fn url(&self, _name: &str, _expiry_secs: u64) -> Result<String, StorageError> {
		Err(StorageError::UnsupportedOperation(
			"test backend".to_owned(),
		))
	}

	async fn size(&self, _name: &str) -> Result<u64, StorageError> {
		Err(StorageError::NotFound("test backend".to_owned()))
	}

	async fn get_modified_time(&self, _name: &str) -> Result<DateTime<Utc>, StorageError> {
		Err(StorageError::NotFound("test backend".to_owned()))
	}

	fn capabilities(&self) -> StorageCapabilities {
		StorageCapabilities::default()
	}
}

#[tokio::test]
#[serial(file_storage_registry)]
async fn file_field_foundation_round_trip_and_activation_boundaries() {
	let missing_directory = TempDir::new().expect("missing-alias temp directory should be created");
	let missing_settings = local_storage_settings(&missing_directory);
	let missing =
		initialize_with_model_registry(&missing_settings, &missing_alias_model_registry()).await;
	assert!(matches!(
		missing,
		Err(FileStorageError::UnknownStorageAlias(alias)) if alias == "missing_alias"
	));
	assert!(matches!(
		active_storage_registry(),
		Err(FileStorageError::RegistryUnavailable)
	));

	let unsupported = Arc::new(
		StorageRegistry::from_entries(
			StorageEntry::new(Arc::new(NoExclusiveCreateBackend), Duration::from_secs(60)),
			[] as [(String, StorageEntry); 0],
		)
		.expect("fake default storage entry should be valid"),
	);
	assert!(matches!(
		unsupported.validate_file_field_aliases(["default"]),
		Err(FileStorageError::UnsupportedExclusiveSave(alias)) if alias == "default"
	));
	assert!(matches!(
		active_storage_registry(),
		Err(FileStorageError::RegistryUnavailable)
	));

	let directory = TempDir::new().expect("local storage directory should be created");
	let settings = local_storage_settings(&directory);
	let guard = initialize(&settings)
		.await
		.expect("local file storage should initialize before ORM use");
	let expected_directory = format!("avatars/{}", Utc::now().format("%Y/%m/%d"));

	reinitialize_database("sqlite::memory:")
		.await
		.expect("SQLite ORM connection should initialize");
	let connection = get_connection()
		.await
		.expect("SQLite ORM connection should be available");
	connection
		.execute(&sqlite_model_table_sql(), vec![])
		.await
		.expect("FileField model table should be created");

	let payload = b"avatar payload bytes";
	let stored = Profile::file_avatar()
		.store(UploadedFile {
			name: "avatar".to_owned(),
			filename: Some("avatar.png".to_owned()),
			content_type: Some("image/png".to_owned()),
			size: payload.len(),
			data: Bytes::copy_from_slice(payload),
		})
		.await
		.expect("generated FileField descriptor should store an upload");
	assert_eq!(stored.storage_alias(), "default");
	assert!(stored.path().starts_with(&format!("{expected_directory}/")));
	assert!(stored.path().ends_with("/avatar.png"));

	let profile = Profile::objects()
		.create(&Profile {
			id: None,
			avatar: stored.clone(),
		})
		.await
		.expect("generated ORM API should insert the FileField model");
	let id = profile
		.id
		.expect("SQLite should assign the model primary key");
	let raw = connection
		.query_one(
			"SELECT avatar FROM file_field_profiles WHERE id = ?",
			vec![reinhardt::db::orm::QueryValue::Int(id)],
		)
		.await
		.expect("physical FileField column should be queryable");
	assert_eq!(
		raw.get::<String>("avatar")
			.expect("avatar column should be text"),
		stored.path()
	);

	let hydrated = Profile::objects()
		.get(id)
		.get()
		.await
		.expect("generated ORM API should hydrate the FileField model");
	assert_eq!(hydrated.avatar.storage_alias(), "default");
	assert_eq!(hydrated.avatar.path(), stored.path());
	assert_eq!(hydrated.avatar.open().await.unwrap(), payload);
	assert_eq!(hydrated.avatar.size().await.unwrap(), payload.len() as u64);
	let url = hydrated
		.avatar
		.url()
		.await
		.expect("local URL should resolve");
	assert!(url.starts_with("file://"));
	assert!(url.ends_with(stored.path()));
	let explicit_url = hydrated
		.avatar
		.url_with_expiry(Duration::from_secs(900))
		.await
		.expect("explicit local URL expiry should resolve");
	assert_eq!(explicit_url, url);

	drop(guard);
	assert!(matches!(
		hydrated.avatar.open().await,
		Err(FileStorageError::RegistryUnavailable)
	));
}
