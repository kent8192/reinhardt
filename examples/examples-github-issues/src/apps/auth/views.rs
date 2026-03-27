//! GraphQL resolvers for authentication
//!
//! This module contains Query and Mutation resolvers for user authentication operations.

use chrono::Utc;
use reinhardt::graphql::{Context, Error as GqlError, GqlResult, ID, Object};
use reinhardt::{BaseUser, JwtAuth};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::apps::auth::models::User;
use crate::apps::auth::serializers::{AuthPayload, CreateUserInput, LoginInput, UserType};

/// In-memory user storage
#[derive(Clone, Default)]
pub struct UserStorage {
	users: Arc<RwLock<HashMap<String, User>>>,
}

impl UserStorage {
	/// Create a new empty user storage
	pub fn new() -> Self {
		Self {
			users: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Add or update a user
	pub async fn add_user(&self, user: User) {
		self.users.write().await.insert(user.id.to_string(), user);
	}

	/// Get a user by ID
	pub async fn get_user(&self, id: &str) -> Option<User> {
		self.users.read().await.get(id).cloned()
	}

	/// Find a user by email
	pub async fn find_by_email(&self, email: &str) -> Option<User> {
		self.users
			.read()
			.await
			.values()
			.find(|u| u.email == email)
			.cloned()
	}

	/// Find a user by username
	pub async fn find_by_username(&self, username: &str) -> Option<User> {
		self.users
			.read()
			.await
			.values()
			.find(|u| u.username == username)
			.cloned()
	}

	/// List all users
	pub async fn list_users(&self) -> Vec<User> {
		self.users.read().await.values().cloned().collect()
	}
}

/// Authentication Query resolvers
#[derive(Default)]
pub struct AuthQuery;

#[Object]
impl AuthQuery {
	/// Get current authenticated user
	async fn me(&self, ctx: &Context<'_>) -> GqlResult<Option<UserType>> {
		use reinhardt::Claims;
		let claims = ctx
			.data_opt::<Claims>()
			.ok_or_else(|| GqlError::new("Authentication required"))?;
		let storage = ctx.data::<UserStorage>()?;
		let user = storage.get_user(&claims.sub).await;
		Ok(user.map(UserType))
	}

	/// List all users
	async fn users(&self, ctx: &Context<'_>) -> GqlResult<Vec<UserType>> {
		let storage = ctx.data::<UserStorage>()?;
		let users = storage.list_users().await;
		Ok(users.into_iter().map(UserType).collect())
	}

	/// Get a user by ID
	async fn user(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<UserType>> {
		let storage = ctx.data::<UserStorage>()?;
		let user = storage.get_user(id.as_str()).await;
		Ok(user.map(UserType))
	}
}

/// Authentication Mutation resolvers
#[derive(Default)]
pub struct AuthMutation;

#[Object]
impl AuthMutation {
	/// Login with username and password, returns JWT token
	async fn login(&self, ctx: &Context<'_>, input: LoginInput) -> GqlResult<AuthPayload> {
		let storage = ctx.data::<UserStorage>()?;
		let jwt_auth = ctx.data::<JwtAuth>()?;

		let user = storage
			.find_by_username(&input.username)
			.await
			.ok_or_else(|| GqlError::new("Invalid credentials"))?;

		// Verify password
		if !user
			.check_password(&input.password)
			.map_err(|e| GqlError::new(e.to_string()))?
		{
			return Err(GqlError::new("Invalid credentials"));
		}

		// Generate JWT token
		let token = jwt_auth
			.generate_token(user.id.to_string(), user.username.clone())
			.map_err(|e| GqlError::new(e.to_string()))?;

		Ok(AuthPayload { token, user })
	}

	/// Register a new user
	async fn register(&self, ctx: &Context<'_>, input: CreateUserInput) -> GqlResult<AuthPayload> {
		let storage = ctx.data::<UserStorage>()?;
		let jwt_auth = ctx.data::<JwtAuth>()?;

		// Check if username already exists
		if storage.find_by_username(&input.username).await.is_some() {
			return Err(GqlError::new("Username already taken"));
		}

		// Check if email already exists
		if storage.find_by_email(&input.email).await.is_some() {
			return Err(GqlError::new("Email already in use"));
		}

		// Create new user with direct struct initialization
		let mut user = User {
			id: Uuid::new_v4(),
			username: input.username,
			email: input.email,
			password_hash: None,
			first_name: input.first_name.unwrap_or_default(),
			last_name: input.last_name.unwrap_or_default(),
			is_active: true,
			is_staff: false,
			is_superuser: false,
			last_login: None,
			date_joined: Utc::now(),
		};
		user.set_password(&input.password)
			.map_err(|e| GqlError::new(e.to_string()))?;

		storage.add_user(user.clone()).await;

		// Generate JWT token
		let token = jwt_auth
			.generate_token(user.id.to_string(), user.username.clone())
			.map_err(|e| GqlError::new(e.to_string()))?;

		Ok(AuthPayload { token, user })
	}
}
