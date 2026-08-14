//! Settings fragment for storage backends.

#![allow(deprecated)] // Settings conversion targets legacy config during the compatibility window.

use crate::config::{BackendType, StorageConfig};
use crate::{Result, StorageError};
#[cfg(any(feature = "azure", feature = "gcs"))]
use reinhardt_conf::settings::secret_types::SecretString;
use reinhardt_conf::settings::{
	fragment::SettingsValidation,
	profile::Profile,
	validation::{ValidationError, ValidationResult},
};
use reinhardt_core::macros::settings;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DEFAULT_URL_EXPIRY_SECS: u64 = 3_600;

fn default_url_expiry_secs() -> u64 {
	DEFAULT_URL_EXPIRY_SECS
}

fn default_backend() -> BackendType {
	// Pick a backend that is actually compiled in, so `StorageSettings::default()`
	// stays convertible via `to_config()` even when the `local` feature is disabled.
	// The cfg arms are mutually exclusive and collectively exhaustive, so exactly one
	// is the tail expression in any feature combination.
	#[cfg(feature = "local")]
	{
		BackendType::Local
	}
	#[cfg(all(not(feature = "local"), feature = "s3"))]
	{
		BackendType::S3
	}
	#[cfg(all(not(feature = "local"), not(feature = "s3"), feature = "gcs"))]
	{
		BackendType::Gcs
	}
	#[cfg(all(
		not(feature = "local"),
		not(feature = "s3"),
		not(feature = "gcs"),
		feature = "azure"
	))]
	{
		BackendType::Azure
	}
	#[cfg(not(any(feature = "local", feature = "s3", feature = "gcs", feature = "azure")))]
	{
		BackendType::Local
	}
}

/// Storage configuration fragment.
///
/// This fragment maps to the `[storage]` section and can be composed with the
/// `#[settings]` macro from downstream applications.
#[settings(fragment = true, section = "storage", validate = false)]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageSettings {
	/// Selected storage backend.
	#[setting(leaf)]
	#[serde(default = "default_backend")]
	pub backend: BackendType,
	/// Expiration time for generated file URLs, in seconds.
	#[serde(default = "default_url_expiry_secs")]
	pub url_expiry_secs: u64,
	/// Named storage backends available to file fields.
	#[setting(leaf)]
	#[serde(default, deserialize_with = "deserialize_named_storage_settings")]
	pub named: BTreeMap<String, NamedStorageSettings>,
	/// Amazon S3 backend settings.
	#[cfg(feature = "s3")]
	#[setting(node)]
	#[serde(default)]
	pub s3: Option<S3StorageSettings>,
	/// Google Cloud Storage backend settings.
	#[cfg(feature = "gcs")]
	#[setting(node)]
	#[serde(default)]
	pub gcs: Option<GcsStorageSettings>,
	/// Azure Blob Storage backend settings.
	#[cfg(feature = "azure")]
	#[setting(node)]
	#[serde(default)]
	pub azure: Option<AzureStorageSettings>,
	/// Local filesystem backend settings.
	#[cfg(feature = "local")]
	#[setting(node)]
	#[serde(default)]
	pub local: Option<LocalStorageSettings>,
}

/// Settings for a named storage backend.
///
/// Unlike [`StorageSettings`], this embedded settings node cannot contain another
/// named registry. This keeps the storage registry to a single level.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedStorageSettings {
	/// Selected storage backend.
	pub backend: BackendType,
	/// Expiration time for generated file URLs, in seconds.
	#[serde(default = "default_url_expiry_secs")]
	pub url_expiry_secs: u64,
	/// Amazon S3 backend settings.
	#[cfg(feature = "s3")]
	#[serde(default)]
	pub s3: Option<S3StorageSettings>,
	/// Google Cloud Storage backend settings.
	#[cfg(feature = "gcs")]
	#[serde(default)]
	pub gcs: Option<GcsStorageSettings>,
	/// Azure Blob Storage backend settings.
	#[cfg(feature = "azure")]
	#[serde(default)]
	pub azure: Option<AzureStorageSettings>,
	/// Local filesystem backend settings.
	#[cfg(feature = "local")]
	#[serde(default)]
	pub local: Option<LocalStorageSettings>,
}

/// Amazon S3 settings.
#[cfg(feature = "s3")]
#[settings(fragment = true, default_policy = "required")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct S3StorageSettings {
	/// S3 bucket name.
	pub bucket: String,
	/// AWS region.
	#[setting(optional)]
	#[serde(default)]
	pub region: Option<String>,
	/// Custom S3-compatible endpoint.
	#[setting(optional)]
	#[serde(default)]
	pub endpoint: Option<String>,
	/// Object key prefix.
	#[setting(optional)]
	#[serde(default)]
	pub prefix: Option<String>,
}

/// Google Cloud Storage settings.
#[cfg(feature = "gcs")]
#[settings(fragment = true, default_policy = "required")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GcsStorageSettings {
	/// GCS bucket name.
	pub bucket: String,
	/// Object name prefix.
	#[setting(optional)]
	#[serde(default)]
	pub prefix: Option<String>,
	/// Custom endpoint, primarily for fake-gcs-server.
	#[setting(optional)]
	#[serde(default)]
	pub endpoint: Option<String>,
	/// Service account JSON for explicit credentials and signed URLs.
	#[setting(optional)]
	#[serde(default)]
	pub service_account_json: Option<SecretString>,
}

/// Azure Blob Storage settings.
#[cfg(feature = "azure")]
#[settings(fragment = true, default_policy = "required")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AzureStorageSettings {
	/// Storage account name.
	pub account: String,
	/// Blob container name.
	pub container: String,
	/// Blob name prefix.
	#[setting(optional)]
	#[serde(default)]
	pub prefix: Option<String>,
	/// Custom blob endpoint, primarily for Azurite.
	#[setting(optional)]
	#[serde(default)]
	pub endpoint: Option<String>,
	/// Account access key used for Shared Key and SAS signing.
	#[setting(optional)]
	#[serde(default)]
	pub access_key: Option<SecretString>,
	/// Pre-generated SAS token used only for backend operations.
	#[setting(optional)]
	#[serde(default)]
	pub sas_token: Option<SecretString>,
	/// Azure Storage connection string.
	#[setting(optional)]
	#[serde(default)]
	pub connection_string: Option<SecretString>,
}

/// Local filesystem settings.
#[cfg(feature = "local")]
#[settings(fragment = true, default_policy = "required")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalStorageSettings {
	/// Base directory path for stored files.
	pub base_path: String,
}

#[cfg(feature = "local")]
impl Default for LocalStorageSettings {
	fn default() -> Self {
		Self {
			base_path: "media".to_string(),
		}
	}
}

impl Default for StorageSettings {
	fn default() -> Self {
		// Populate the selected backend's settings so the default value is convertible
		// via `to_config()` in every feature configuration. `default_backend()` only
		// returns a backend whose feature (and therefore whose settings field) is
		// compiled in, so the matching field below is always present and gets `Some`.
		let backend = default_backend();
		Self {
			backend,
			url_expiry_secs: default_url_expiry_secs(),
			named: BTreeMap::new(),
			#[cfg(feature = "s3")]
			s3: matches!(backend, BackendType::S3).then(S3StorageSettings::default),
			#[cfg(feature = "gcs")]
			gcs: matches!(backend, BackendType::Gcs).then(GcsStorageSettings::default),
			#[cfg(feature = "azure")]
			azure: matches!(backend, BackendType::Azure).then(AzureStorageSettings::default),
			#[cfg(feature = "local")]
			local: matches!(backend, BackendType::Local).then(LocalStorageSettings::default),
		}
	}
}

impl SettingsValidation for StorageSettings {
	fn validate(&self, _profile: &Profile) -> ValidationResult {
		self.to_config()
			.map(|_| ())
			.map_err(|err| ValidationError::InvalidValue {
				key: "storage.backend".to_string(),
				message: err.to_string(),
			})
	}
}

impl StorageSettings {
	/// Convert settings into the deprecated compatibility config.
	pub fn to_config(&self) -> Result<StorageConfig> {
		storage_config_from_parts(
			self.backend,
			#[cfg(feature = "s3")]
			self.s3.as_ref(),
			#[cfg(feature = "gcs")]
			self.gcs.as_ref(),
			#[cfg(feature = "azure")]
			self.azure.as_ref(),
			#[cfg(feature = "local")]
			self.local.as_ref(),
			"storage",
		)
	}
}

impl NamedStorageSettings {
	/// Builds a named storage settings fragment for the local filesystem backend.
	///
	/// Optional backend-specific fields (`s3`, `gcs`, `azure`) are populated
	/// according to which storage backend features this crate was actually
	/// compiled with. Downstream crates cannot mirror these `#[cfg]` flags
	/// reliably via their own feature flags, because Cargo's workspace-wide
	/// feature unification can enable this crate's default features (`s3`,
	/// `local`) independently of what a dependent crate requests. Building
	/// the fragment here, inside this crate, keeps the `#[cfg]` gates correct
	/// for whatever feature set this crate actually compiled with.
	#[cfg(feature = "local")]
	pub fn local(url_expiry_secs: u64, local: LocalStorageSettings) -> Self {
		Self {
			backend: BackendType::Local,
			url_expiry_secs,
			#[cfg(feature = "s3")]
			s3: None,
			#[cfg(feature = "gcs")]
			gcs: None,
			#[cfg(feature = "azure")]
			azure: None,
			local: Some(local),
		}
	}

	pub(crate) fn to_config_for_alias(&self, alias: &str) -> Result<StorageConfig> {
		storage_config_from_parts(
			self.backend,
			#[cfg(feature = "s3")]
			self.s3.as_ref(),
			#[cfg(feature = "gcs")]
			self.gcs.as_ref(),
			#[cfg(feature = "azure")]
			self.azure.as_ref(),
			#[cfg(feature = "local")]
			self.local.as_ref(),
			&format!("storage.named.{alias}"),
		)
	}
}

pub(crate) fn is_valid_named_storage_alias(alias: &str) -> bool {
	alias != "default"
		&& alias.as_bytes().split_first().is_some_and(|(first, rest)| {
			first.is_ascii_lowercase()
				&& rest.iter().all(|character| {
					character.is_ascii_lowercase()
						|| character.is_ascii_digit()
						|| matches!(character, b'_' | b'-')
				})
		})
}

fn deserialize_named_storage_settings<'de, D>(
	deserializer: D,
) -> std::result::Result<BTreeMap<String, NamedStorageSettings>, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let named: BTreeMap<String, NamedStorageSettings> = BTreeMap::deserialize(deserializer)?;
	for alias in named.keys() {
		if !is_valid_named_storage_alias(alias) {
			return Err(D::Error::custom(format!("invalid storage alias `{alias}`")));
		}
	}

	Ok(named)
}

fn storage_config_from_parts(
	backend: BackendType,
	#[cfg(feature = "s3")] s3: Option<&S3StorageSettings>,
	#[cfg(feature = "gcs")] gcs: Option<&GcsStorageSettings>,
	#[cfg(feature = "azure")] azure: Option<&AzureStorageSettings>,
	#[cfg(feature = "local")] local: Option<&LocalStorageSettings>,
	section_prefix: &str,
) -> Result<StorageConfig> {
	match backend {
		#[cfg(feature = "s3")]
		BackendType::S3 => s3
			.map(|settings| {
				StorageConfig::S3(crate::config::S3Config {
					bucket: settings.bucket.clone(),
					region: settings.region.clone(),
					endpoint: settings.endpoint.clone(),
					prefix: settings.prefix.clone(),
				})
			})
			.ok_or_else(|| missing_section(&format!("{section_prefix}.s3"))),
		#[cfg(feature = "gcs")]
		BackendType::Gcs => gcs
			.map(|settings| {
				StorageConfig::Gcs(crate::config::GcsConfig {
					bucket: settings.bucket.clone(),
					prefix: settings.prefix.clone(),
					endpoint: settings.endpoint.clone(),
					service_account_json: settings.service_account_json.clone(),
				})
			})
			.ok_or_else(|| missing_section(&format!("{section_prefix}.gcs"))),
		#[cfg(feature = "azure")]
		BackendType::Azure => azure
			.map(|settings| {
				StorageConfig::Azure(crate::config::AzureConfig {
					account: settings.account.clone(),
					container: settings.container.clone(),
					prefix: settings.prefix.clone(),
					endpoint: settings.endpoint.clone(),
					access_key: settings.access_key.clone(),
					sas_token: settings.sas_token.clone(),
					connection_string: settings.connection_string.clone(),
				})
			})
			.ok_or_else(|| missing_section(&format!("{section_prefix}.azure"))),
		#[cfg(feature = "local")]
		BackendType::Local => local
			.map(|settings| {
				StorageConfig::Local(crate::config::LocalConfig {
					base_path: settings.base_path.clone(),
				})
			})
			.ok_or_else(|| missing_section(&format!("{section_prefix}.local"))),
		#[allow(unreachable_patterns)]
		backend => Err(StorageError::ConfigError(format!(
			"Backend type not enabled: {backend:?}"
		))),
	}
}

fn missing_section(section: &str) -> StorageError {
	StorageError::ConfigError(format!("Selected backend requires [{section}] settings"))
}

#[cfg(test)]
mod tests {
	use super::*;

	// `StorageSettings::default()` must be convertible via `to_config()` for whichever
	// backend `default_backend()` selects, including non-local builds where `local` is
	// disabled. This exercises the S3/Gcs/Azure default-backend paths under those
	// feature sets, not just the `Local` path.
	#[test]
	#[cfg(any(feature = "s3", feature = "gcs", feature = "azure", feature = "local"))]
	fn default_is_convertible_in_every_backend_feature_set() {
		// Arrange
		let settings = StorageSettings::default();

		// Act
		let result = settings.to_config();

		// Assert
		assert!(
			result.is_ok(),
			"default StorageSettings must convert when a backend feature is enabled, got {:?}",
			result.err()
		);
	}
}
