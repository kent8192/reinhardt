use crate::orm::field_codec::{DatabaseField, FieldCodecContext, FieldCodecError};
use reinhardt_storages::{
	FileStorageError, active_storage_registry, validate_logical_key, validate_storage_alias,
};
use std::time::Duration;

/// A validated logical path bound to one named storage backend.
#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Serialize)]
pub struct FileField {
	path: String,
	#[serde(rename = "storage")]
	storage_alias: String,
}

impl<'de> serde::Deserialize<'de> for FileField {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		#[derive(serde::Deserialize)]
		struct FileFieldObject {
			path: String,
			#[serde(rename = "storage")]
			storage_alias: String,
		}

		let value = <FileFieldObject as serde::Deserialize>::deserialize(deserializer)?;
		Self::from_existing(value.path, value.storage_alias).map_err(serde::de::Error::custom)
	}
}

impl FileField {
	/// Construct a typed reference to an existing logical storage key.
	pub fn from_existing(
		path: impl Into<String>,
		storage_alias: impl Into<String>,
	) -> Result<Self, FileStorageError> {
		let path = path.into();
		let storage_alias = storage_alias.into();
		validate_logical_key(&path)?;
		validate_storage_alias(&storage_alias, true)?;
		Ok(Self {
			path,
			storage_alias,
		})
	}

	/// Return the portable logical storage key.
	#[must_use]
	pub fn path(&self) -> &str {
		&self.path
	}

	/// Return the registry alias used to resolve this value.
	#[must_use]
	pub fn storage_alias(&self) -> &str {
		&self.storage_alias
	}

	/// Read the referenced file from the active storage registry.
	pub async fn open(&self) -> Result<Vec<u8>, FileStorageError> {
		let backend = active_storage_registry()?.backend(&self.storage_alias)?;
		backend.open(&self.path).await.map_err(Into::into)
	}

	/// Return the referenced file size in bytes.
	pub async fn size(&self) -> Result<u64, FileStorageError> {
		let backend = active_storage_registry()?.backend(&self.storage_alias)?;
		backend.size(&self.path).await.map_err(Into::into)
	}

	/// Generate a URL using the configured expiration for this storage alias.
	pub async fn url(&self) -> Result<String, FileStorageError> {
		let registry = active_storage_registry()?;
		let expiry = registry.url_expiry(&self.storage_alias)?;
		let backend = registry.backend(&self.storage_alias)?;
		backend
			.url(&self.path, expiry.as_secs())
			.await
			.map_err(Into::into)
	}

	/// Generate a URL with an explicit expiration.
	pub async fn url_with_expiry(&self, expiry: Duration) -> Result<String, FileStorageError> {
		let backend = active_storage_registry()?.backend(&self.storage_alias)?;
		backend
			.url(&self.path, expiry.as_secs())
			.await
			.map_err(Into::into)
	}
}

impl DatabaseField for FileField {
	type Storage = String;

	fn encode_database(&self) -> Result<Self::Storage, FieldCodecError> {
		Ok(self.path.clone())
	}

	fn decode_database(
		value: Self::Storage,
		context: &FieldCodecContext,
	) -> Result<Self, FieldCodecError> {
		let storage_alias = context.metadata("file_storage").ok_or_else(|| {
			FieldCodecError::MissingFieldMetadata {
				context: context.clone(),
				key: "file_storage".to_owned(),
			}
		})?;
		Self::from_existing(value, storage_alias).map_err(|error| {
			FieldCodecError::Serialization(format!("invalid stored file reference: {error}"))
		})
	}

	fn validate_database_context(
		&self,
		context: &FieldCodecContext,
	) -> Result<(), FieldCodecError> {
		let expected = context.metadata("file_storage").ok_or_else(|| {
			FieldCodecError::MissingFieldMetadata {
				context: context.clone(),
				key: "file_storage".to_owned(),
			}
		})?;
		if expected == self.storage_alias {
			Ok(())
		} else {
			Err(FieldCodecError::FieldPolicyMismatch {
				context: Box::new(context.clone()),
				key: "file_storage".to_owned(),
				expected: expected.to_owned(),
				actual: self.storage_alias.clone(),
			})
		}
	}
}

#[cfg(test)]
mod tests {
	use super::FileField;
	use crate::orm::field_codec::{DatabaseField, FieldCodecContext, FieldCodecError};
	use crate::orm::{DatabaseValue, IntoFieldValue};
	use std::collections::HashSet;

	#[test]
	fn existing_file_preserves_path_alias_and_database_path() {
		let value =
			FileField::from_existing("avatars/2026/08/08/avatar.png", "private_uploads").unwrap();

		assert_eq!(value.path(), "avatars/2026/08/08/avatar.png");
		assert_eq!(value.storage_alias(), "private_uploads");
		assert_eq!(
			value.encode_database().unwrap(),
			"avatars/2026/08/08/avatar.png"
		);
	}

	#[test]
	fn serde_contract_uses_path_and_storage_object_fields() {
		let value = FileField::from_existing("avatars/a.png", "default").unwrap();

		let json = serde_json::to_value(&value).unwrap();
		assert_eq!(
			json,
			serde_json::json!({"path": "avatars/a.png", "storage": "default"})
		);
		assert_eq!(serde_json::from_value::<FileField>(json).unwrap(), value);
	}

	#[test]
	fn serde_rejects_unsafe_logical_keys() {
		for path in ["../avatar.png", "/avatars/a.png", "avatars//a.png"] {
			let error = serde_json::from_value::<FileField>(serde_json::json!({
				"path": path,
				"storage": "default"
			}))
			.unwrap_err();

			assert!(
				error.to_string().contains("unsafe upload filename"),
				"unexpected error for {path}: {error}"
			);
		}
	}

	#[test]
	fn serde_rejects_invalid_storage_aliases() {
		for storage in ["Default", "-private", "default/backup"] {
			let error = serde_json::from_value::<FileField>(serde_json::json!({
				"path": "avatars/a.png",
				"storage": storage
			}))
			.unwrap_err();

			assert!(
				error.to_string().contains("invalid storage alias"),
				"unexpected error for {storage}: {error}"
			);
		}
	}

	#[test]
	fn equality_and_hash_include_the_storage_alias() {
		let default = FileField::from_existing("avatars/a.png", "default").unwrap();
		let private = FileField::from_existing("avatars/a.png", "private_uploads").unwrap();
		let duplicate = FileField::from_existing("avatars/a.png", "default").unwrap();
		let values = HashSet::from([default.clone(), private.clone(), duplicate]);

		assert_ne!(default, private);
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn existing_file_rejects_an_unsafe_logical_key() {
		assert!(FileField::from_existing("../avatar.png", "default").is_err());
	}

	#[test]
	fn decode_reconstructs_alias_from_codec_metadata() {
		let context = FieldCodecContext::new("Profile", "avatar", "avatar_path")
			.with_metadata("file_storage", "private_uploads");

		let value = FileField::decode_database("avatars/a.png".to_owned(), &context).unwrap();

		assert_eq!(value.path(), "avatars/a.png");
		assert_eq!(value.storage_alias(), "private_uploads");
	}

	#[test]
	fn decode_requires_file_storage_metadata() {
		let context = FieldCodecContext::new("Profile", "avatar", "avatar_path");

		let error = FileField::decode_database("avatars/a.png".to_owned(), &context).unwrap_err();

		assert!(matches!(
			error,
			FieldCodecError::MissingFieldMetadata { ref key, .. } if key == "file_storage"
		));
	}

	#[test]
	fn nullable_file_values_preserve_null_without_resolving_policy() {
		let context = FieldCodecContext::new("Profile", "avatar", "avatar_path");

		let decoded = Option::<FileField>::decode_database(None, &context).unwrap();
		let encoded = <Option<FileField> as IntoFieldValue<Option<FileField>>>::
			into_field_value_with_context(None, &context)
			.unwrap();

		assert_eq!(decoded, None);
		assert_eq!(encoded, DatabaseValue::Null);
	}
}
