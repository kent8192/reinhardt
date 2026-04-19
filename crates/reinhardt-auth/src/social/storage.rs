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
	/// Unique identifier for this social account link.
	pub id: Uuid,
	/// The local user ID linked to this social account.
	pub user_id: Uuid,
	/// The provider name (e.g., "google", "github").
	pub provider: String,
	/// The user's ID on the external provider.
	pub provider_user_id: String,
	/// The user's email from the provider, if available.
	pub email: Option<String>,
	/// The user's display name from the provider, if available.
	pub display_name: Option<String>,
	/// The user's profile picture URL from the provider, if available.
	pub picture: Option<String>,
	/// The OAuth2 access token.
	pub access_token: String,
	/// The OAuth2 refresh token, if available.
	pub refresh_token: Option<String>,
	/// When the access token expires.
	pub token_expires_at: DateTime<Utc>,
	/// The OAuth2 scopes granted.
	pub scopes: Vec<String>,
	/// When this social account link was created.
	pub created_at: DateTime<Utc>,
	/// When this social account link was last updated.
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
	use rstest::rstest;

	fn test_account(user_id: Uuid) -> SocialAccount {
		SocialAccount {
			id: Uuid::now_v7(),
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

	#[rstest]
	#[tokio::test]
	async fn test_create_and_find() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::now_v7();
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

	#[rstest]
	#[tokio::test]
	async fn test_find_by_user() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::now_v7();
		let account = test_account(user_id);

		// Act
		storage.create(account).await.unwrap();
		let accounts = storage.find_by_user(user_id).await.unwrap();

		// Assert
		assert_eq!(accounts.len(), 1);
		assert_eq!(accounts[0].user_id, user_id);
	}

	#[rstest]
	#[tokio::test]
	async fn test_update() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::now_v7();
		let mut account = test_account(user_id);
		storage.create(account.clone()).await.unwrap();

		// Act
		account.access_token = "new_token".to_string();
		let updated = storage.update(account).await.unwrap();

		// Assert
		assert_eq!(updated.access_token, "new_token");
		let found = storage.find_by_user(user_id).await.unwrap();
		assert_eq!(found[0].access_token, "new_token");
	}

	#[rstest]
	#[tokio::test]
	async fn test_delete() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::now_v7();
		let account = test_account(user_id);
		let id = account.id;
		storage.create(account).await.unwrap();

		// Act
		storage.delete(id).await.unwrap();
		let accounts = storage.find_by_user(user_id).await.unwrap();

		// Assert
		assert!(accounts.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn test_delete_nonexistent() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();

		// Act
		let result = storage.delete(Uuid::now_v7()).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_update_nonexistent() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let account = test_account(Uuid::now_v7());

		// Act
		let result = storage.update(account).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_find_by_provider_nonexistent_returns_none() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();

		// Act
		let result = storage
			.find_by_provider_and_uid("nonexistent_provider", "unknown_uid")
			.await
			.unwrap();

		// Assert
		assert!(result.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_find_by_user_no_accounts_returns_empty() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let random_user_id = Uuid::now_v7();

		// Act
		let accounts = storage.find_by_user(random_user_id).await.unwrap();

		// Assert
		assert!(accounts.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn test_create_multiple_providers_same_user() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::now_v7();

		let mut github_account = test_account(user_id);
		github_account.provider = "github".to_string();
		github_account.provider_user_id = "gh_user_1".to_string();

		let mut google_account = test_account(user_id);
		google_account.id = Uuid::now_v7();
		google_account.provider = "google".to_string();
		google_account.provider_user_id = "google_user_1".to_string();

		// Act
		storage.create(github_account).await.unwrap();
		storage.create(google_account).await.unwrap();
		let accounts = storage.find_by_user(user_id).await.unwrap();

		// Assert
		assert_eq!(accounts.len(), 2);
		let providers: Vec<&str> = accounts.iter().map(|a| a.provider.as_str()).collect();
		assert!(providers.contains(&"github"));
		assert!(providers.contains(&"google"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_update_token_fields() {
		// Arrange
		let storage = InMemorySocialAccountStorage::new();
		let user_id = Uuid::now_v7();
		let mut account = test_account(user_id);
		let account_id = account.id;
		storage.create(account.clone()).await.unwrap();

		// Act - update refresh token and scopes
		account.refresh_token = Some("new_refresh_token".to_string());
		account.scopes = vec!["user".to_string(), "repo".to_string()];
		storage.update(account).await.unwrap();

		// Assert - verify persisted changes
		let accounts = storage.find_by_user(user_id).await.unwrap();
		let updated = accounts.iter().find(|a| a.id == account_id).unwrap();
		assert_eq!(updated.refresh_token, Some("new_refresh_token".to_string()));
		assert_eq!(updated.scopes, vec!["user", "repo"]);
	}
}
