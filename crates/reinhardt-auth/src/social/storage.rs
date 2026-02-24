//! Social account storage

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::social::core::SocialAuthError;

/// Social account linking user to provider
#[derive(Debug, Clone)]
pub struct SocialAccount {
	pub id: Uuid,
	pub user_id: Uuid,
	pub provider: String,
	pub provider_user_id: String,
	pub email: Option<String>,
	pub display_name: Option<String>,
	pub picture: Option<String>,
	pub access_token: String,
	pub refresh_token: Option<String>,
	pub token_expires_at: DateTime<Utc>,
	pub scopes: Vec<String>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Social account storage trait
#[async_trait]
pub trait SocialAccountStorage: Send + Sync {
	/// Finds a social account by provider name and provider-specific user ID
	async fn find_by_provider_and_uid(
		&self,
		provider: &str,
		provider_user_id: &str,
	) -> Result<Option<SocialAccount>, SocialAuthError>;

	/// Finds all social accounts for a given user
	async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<SocialAccount>, SocialAuthError>;

	/// Creates a new social account
	async fn create(&self, account: SocialAccount) -> Result<SocialAccount, SocialAuthError>;

	/// Updates an existing social account
	async fn update(&self, account: SocialAccount) -> Result<SocialAccount, SocialAuthError>;

	/// Deletes a social account by its ID
	async fn delete(&self, id: Uuid) -> Result<(), SocialAuthError>;
}

/// In-memory social account storage for development and testing
///
/// This implementation is NOT suitable for production use.
/// For production, implement `SocialAccountStorage` backed by a database.
#[derive(Debug, Default)]
pub struct InMemorySocialAccountStorage {
	store: RwLock<HashMap<Uuid, SocialAccount>>,
}

impl InMemorySocialAccountStorage {
	/// Creates a new in-memory storage
	pub fn new() -> Self {
		Self {
			store: RwLock::new(HashMap::new()),
		}
	}
}

#[async_trait]
impl SocialAccountStorage for InMemorySocialAccountStorage {
	async fn find_by_provider_and_uid(
		&self,
		provider: &str,
		provider_user_id: &str,
	) -> Result<Option<SocialAccount>, SocialAuthError> {
		let store = self.store.read().await;
		let account = store
			.values()
			.find(|a| a.provider == provider && a.provider_user_id == provider_user_id)
			.cloned();
		Ok(account)
	}

	async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<SocialAccount>, SocialAuthError> {
		let store = self.store.read().await;
		let accounts: Vec<SocialAccount> = store
			.values()
			.filter(|a| a.user_id == user_id)
			.cloned()
			.collect();
		Ok(accounts)
	}

	async fn create(&self, account: SocialAccount) -> Result<SocialAccount, SocialAuthError> {
		let mut store = self.store.write().await;
		let id = account.id;
		store.insert(id, account.clone());
		Ok(account)
	}

	async fn update(&self, account: SocialAccount) -> Result<SocialAccount, SocialAuthError> {
		let mut store = self.store.write().await;
		if !store.contains_key(&account.id) {
			return Err(SocialAuthError::Storage(format!(
				"Social account not found: {}",
				account.id
			)));
		}
		store.insert(account.id, account.clone());
		Ok(account)
	}

	async fn delete(&self, id: Uuid) -> Result<(), SocialAuthError> {
		let mut store = self.store.write().await;
		store
			.remove(&id)
			.ok_or_else(|| SocialAuthError::Storage(format!("Social account not found: {}", id)))?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Duration;

	fn test_account(user_id: Uuid) -> SocialAccount {
		SocialAccount {
			id: Uuid::new_v4(),
			user_id,
			provider: "github".to_string(),
			provider_user_id: "gh_123".to_string(),
			email: Some("user@example.com".to_string()),
			display_name: Some("Test User".to_string()),
			picture: None,
			access_token: "access_token".to_string(),
			refresh_token: None,
			token_expires_at: Utc::now() + Duration::hours(1),
			scopes: vec!["user".to_string()],
			created_at: Utc::now(),
			updated_at: Utc::now(),
		}
	}

	#[tokio::test]
	async fn test_create_and_find() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::new_v4();
		let account = test_account(user_id);
		let provider_uid = account.provider_user_id.clone();

		// Act
		storage.create(account).await.unwrap();
		let found = storage
			.find_by_provider_and_uid("github", &provider_uid)
			.await
			.unwrap();

		// Assert
		assert!(found.is_some());
		assert_eq!(found.unwrap().provider_user_id, provider_uid);
	}

	#[tokio::test]
	async fn test_find_by_user() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::new_v4();
		let account = test_account(user_id);

		// Act
		storage.create(account).await.unwrap();
		let accounts = storage.find_by_user(user_id).await.unwrap();

		// Assert
		assert_eq!(accounts.len(), 1);
		assert_eq!(accounts[0].user_id, user_id);
	}

	#[tokio::test]
	async fn test_update() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::new_v4();
		let mut account = test_account(user_id);
		let id = account.id;
		storage.create(account.clone()).await.unwrap();

		// Act
		account.access_token = "new_token".to_string();
		let updated = storage.update(account).await.unwrap();

		// Assert
		assert_eq!(updated.access_token, "new_token");
		let found = storage.find_by_user(user_id).await.unwrap();
		assert_eq!(found[0].access_token, "new_token");
	}

	#[tokio::test]
	async fn test_delete() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::new_v4();
		let account = test_account(user_id);
		let id = account.id;
		storage.create(account).await.unwrap();

		// Act
		storage.delete(id).await.unwrap();
		let accounts = storage.find_by_user(user_id).await.unwrap();

		// Assert
		assert!(accounts.is_empty());
	}

	#[tokio::test]
	async fn test_delete_nonexistent() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();

		// Act
		let result = storage.delete(Uuid::new_v4()).await;

		// Assert
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_update_nonexistent() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let account = test_account(Uuid::new_v4());

		// Act
		let result = storage.update(account).await;

		// Assert
		assert!(result.is_err());
	}
}
