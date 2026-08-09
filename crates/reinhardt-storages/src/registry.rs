//! Named storage backend registry for file fields.

use crate::factory::{create_storage_from_named_settings, create_storage_from_settings};
use crate::settings::is_valid_named_storage_alias;
use crate::{FileStorageError, StorageBackend, StorageSettings};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const DEFAULT_STORAGE_ALIAS: &str = "default";

struct ActiveRegistry {
	generation: u64,
	registry: Arc<StorageRegistry>,
}

static ACTIVE_REGISTRY: OnceLock<Mutex<Option<ActiveRegistry>>> = OnceLock::new();
static NEXT_GENERATION: OnceLock<Mutex<u64>> = OnceLock::new();

/// A configured storage backend and its generated URL expiration.
#[derive(Clone)]
pub struct StorageEntry {
	backend: Arc<dyn StorageBackend>,
	url_expiry: Duration,
}

impl StorageEntry {
	/// Create a storage registry entry.
	#[must_use]
	pub fn new(backend: Arc<dyn StorageBackend>, url_expiry: Duration) -> Self {
		Self {
			backend,
			url_expiry,
		}
	}
}

/// A validated collection of default and named storage backends.
pub struct StorageRegistry {
	entries: BTreeMap<String, StorageEntry>,
}

/// RAII lease for the active process-wide storage registry.
#[must_use = "retain this guard for as long as file storage must remain active"]
pub struct ActiveStorageRegistryGuard {
	generation: u64,
}

impl Drop for ActiveStorageRegistryGuard {
	fn drop(&mut self) {
		let Ok(mut active) = active_registry_slot().lock() else {
			return;
		};

		if active
			.as_ref()
			.is_some_and(|registry| registry.generation == self.generation)
		{
			*active = None;
		}
	}
}

impl StorageRegistry {
	/// Build a registry from the storage settings fragment.
	pub async fn from_settings(
		settings: &StorageSettings,
	) -> std::result::Result<Self, FileStorageError> {
		for alias in settings.named.keys() {
			validate_named_alias(alias)?;
		}

		let default = StorageEntry::new(
			create_storage_from_settings(settings).await?,
			Duration::from_secs(settings.url_expiry_secs),
		);
		let mut named = Vec::with_capacity(settings.named.len());
		for (alias, settings) in &settings.named {
			let entry = StorageEntry::new(
				create_storage_from_named_settings(settings, alias).await?,
				Duration::from_secs(settings.url_expiry_secs),
			);
			named.push((alias.clone(), entry));
		}

		Self::from_entries(default, named)
	}

	/// Build a registry from already-created backends.
	pub fn from_entries(
		default: StorageEntry,
		named: impl IntoIterator<Item = (String, StorageEntry)>,
	) -> std::result::Result<Self, FileStorageError> {
		let mut entries = BTreeMap::new();
		entries.insert(DEFAULT_STORAGE_ALIAS.to_string(), default);

		for (alias, entry) in named {
			validate_named_alias(&alias)?;
			if entries.insert(alias.clone(), entry).is_some() {
				return Err(FileStorageError::UnknownStorageAlias(alias));
			}
		}

		Ok(Self { entries })
	}

	/// Resolve a backend by alias.
	pub fn backend(
		&self,
		alias: &str,
	) -> std::result::Result<Arc<dyn StorageBackend>, FileStorageError> {
		self.entries
			.get(alias)
			.map(|entry| Arc::clone(&entry.backend))
			.ok_or_else(|| FileStorageError::UnknownStorageAlias(alias.to_string()))
	}

	/// Resolve a generated URL expiration by alias.
	pub fn url_expiry(&self, alias: &str) -> std::result::Result<Duration, FileStorageError> {
		self.entries
			.get(alias)
			.map(|entry| entry.url_expiry)
			.ok_or_else(|| FileStorageError::UnknownStorageAlias(alias.to_string()))
	}

	/// Ensure that every file-field alias references a supported backend.
	pub fn validate_file_field_aliases<'a>(
		&self,
		aliases: impl IntoIterator<Item = &'a str>,
	) -> std::result::Result<(), FileStorageError> {
		for alias in aliases {
			let backend = self.backend(alias)?;
			if !backend.capabilities().exclusive_create {
				return Err(FileStorageError::UnsupportedExclusiveSave(
					alias.to_string(),
				));
			}
		}

		Ok(())
	}

	/// Activate this registry until the returned guard is dropped.
	pub fn activate(
		self: Arc<Self>,
	) -> std::result::Result<ActiveStorageRegistryGuard, FileStorageError> {
		let mut active = active_registry_slot()
			.lock()
			.map_err(|_| FileStorageError::RegistryUnavailable)?;
		if active.is_some() {
			return Err(FileStorageError::RegistryUnavailable);
		}

		let generation = next_generation()?;
		*active = Some(ActiveRegistry {
			generation,
			registry: self,
		});
		Ok(ActiveStorageRegistryGuard { generation })
	}
}

/// Return the process-wide active storage registry.
pub fn active_storage_registry() -> std::result::Result<Arc<StorageRegistry>, FileStorageError> {
	let active = active_registry_slot()
		.lock()
		.map_err(|_| FileStorageError::RegistryUnavailable)?;
	active
		.as_ref()
		.map(|registry| Arc::clone(&registry.registry))
		.ok_or(FileStorageError::RegistryUnavailable)
}

fn active_registry_slot() -> &'static Mutex<Option<ActiveRegistry>> {
	ACTIVE_REGISTRY.get_or_init(|| Mutex::new(None))
}

fn next_generation() -> std::result::Result<u64, FileStorageError> {
	let mut generation = NEXT_GENERATION
		.get_or_init(|| Mutex::new(0))
		.lock()
		.map_err(|_| FileStorageError::RegistryUnavailable)?;
	*generation = generation
		.checked_add(1)
		.ok_or(FileStorageError::RegistryUnavailable)?;
	Ok(*generation)
}

fn validate_named_alias(alias: &str) -> std::result::Result<(), FileStorageError> {
	if is_valid_named_storage_alias(alias) {
		Ok(())
	} else {
		Err(FileStorageError::UnknownStorageAlias(alias.to_string()))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serial_test::serial;

	#[rstest]
	#[serial(file_storage_registry)]
	fn stale_lease_does_not_unregister_newer_generation() {
		// Arrange
		let first = Arc::new(StorageRegistry {
			entries: BTreeMap::new(),
		});
		let stale_lease = Arc::clone(&first).activate().unwrap();
		*active_registry_slot().lock().unwrap() = None;
		let second = Arc::new(StorageRegistry {
			entries: BTreeMap::new(),
		});
		let current_lease = Arc::clone(&second).activate().unwrap();

		// Act
		drop(stale_lease);

		// Assert
		assert!(Arc::ptr_eq(&active_storage_registry().unwrap(), &second));
		drop(current_lease);
		assert!(matches!(
			active_storage_registry(),
			Err(FileStorageError::RegistryUnavailable)
		));
	}
}
