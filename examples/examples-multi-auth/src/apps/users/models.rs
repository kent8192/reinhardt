//! Custom user model implementing BaseUser and FullUser traits
//!
//! Demonstrates explicit trait implementation for a custom user type
//! with in-memory storage, similar to examples-github-issues pattern.

use chrono::{DateTime, Utc};
use reinhardt::{Argon2Hasher, BaseUser, FullUser};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Custom application user with all authentication-related fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUser {
	pub id: Uuid,
	pub username: String,
	pub email: String,
	pub password_hash: Option<String>,
	pub first_name: String,
	pub last_name: String,
	pub is_active: bool,
	pub is_staff: bool,
	pub is_superuser: bool,
	pub date_joined: DateTime<Utc>,
	pub last_login: Option<DateTime<Utc>>,
}

impl Default for AppUser {
	fn default() -> Self {
		Self {
			id: Uuid::new_v4(),
			username: String::new(),
			email: String::new(),
			password_hash: None,
			first_name: String::new(),
			last_name: String::new(),
			is_active: true,
			is_staff: false,
			is_superuser: false,
			date_joined: Utc::now(),
			last_login: None,
		}
	}
}

impl BaseUser for AppUser {
	type PrimaryKey = Uuid;
	type Hasher = Argon2Hasher;

	fn get_username_field() -> &'static str {
		"username"
	}

	fn get_username(&self) -> &str {
		&self.username
	}

	fn password_hash(&self) -> Option<&str> {
		self.password_hash.as_deref()
	}

	fn set_password_hash(&mut self, hash: String) {
		self.password_hash = Some(hash);
	}

	fn last_login(&self) -> Option<DateTime<Utc>> {
		self.last_login
	}

	fn set_last_login(&mut self, time: DateTime<Utc>) {
		self.last_login = Some(time);
	}

	fn is_active(&self) -> bool {
		self.is_active
	}
}

impl FullUser for AppUser {
	fn username(&self) -> &str {
		&self.username
	}

	fn email(&self) -> &str {
		&self.email
	}

	fn first_name(&self) -> &str {
		&self.first_name
	}

	fn last_name(&self) -> &str {
		&self.last_name
	}

	fn is_staff(&self) -> bool {
		self.is_staff
	}

	fn is_superuser(&self) -> bool {
		self.is_superuser
	}

	fn date_joined(&self) -> DateTime<Utc> {
		self.date_joined
	}
}

/// In-memory user storage for demonstration purposes
#[derive(Clone, Default)]
pub struct UserStorage {
	users: Arc<RwLock<HashMap<Uuid, AppUser>>>,
	/// Token storage: API token -> user ID
	api_tokens: Arc<RwLock<HashMap<String, Uuid>>>,
}

impl UserStorage {
	/// Create a new empty user storage
	pub fn new() -> Self {
		Self {
			users: Arc::new(RwLock::new(HashMap::new())),
			api_tokens: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Add or update a user
	pub async fn add_user(&self, user: AppUser) {
		self.users.write().await.insert(user.id, user);
	}

	/// Get a user by ID
	pub async fn get_user(&self, id: &Uuid) -> Option<AppUser> {
		self.users.read().await.get(id).cloned()
	}

	/// Find a user by username
	pub async fn find_by_username(&self, username: &str) -> Option<AppUser> {
		self.users
			.read()
			.await
			.values()
			.find(|u| u.username == username)
			.cloned()
	}

	/// List all users
	pub async fn list_users(&self) -> Vec<AppUser> {
		self.users.read().await.values().cloned().collect()
	}

	/// Store an API token for a user
	pub async fn store_api_token(&self, token: String, user_id: Uuid) {
		self.api_tokens.write().await.insert(token, user_id);
	}

	/// Look up a user by API token
	pub async fn get_user_by_token(&self, token: &str) -> Option<AppUser> {
		let user_id = self.api_tokens.read().await.get(token).copied();
		if let Some(id) = user_id {
			self.get_user(&id).await
		} else {
			None
		}
	}
}
