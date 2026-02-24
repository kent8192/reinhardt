//! GraphQL types and input objects for authentication
//!
//! This module defines GraphQL types and inputs for user operations.

use async_graphql::{ID, InputObject, Object};
use chrono::{DateTime, Utc};

use crate::apps::auth::models::User;

/// GraphQL representation of User
#[derive(Clone)]
pub struct UserType(pub User);

#[Object]
impl UserType {
	/// User ID
	async fn id(&self) -> ID {
		ID(self.0.id.to_string())
	}

	/// Username
	async fn username(&self) -> &str {
		&self.0.username
	}

	/// Email address
	async fn email(&self) -> &str {
		&self.0.email
	}

	/// First name
	async fn first_name(&self) -> &str {
		&self.0.first_name
	}

	/// Last name
	async fn last_name(&self) -> &str {
		&self.0.last_name
	}

	/// Whether the user account is active
	async fn is_active(&self) -> bool {
		self.0.is_active
	}

	/// Whether the user is a staff member
	async fn is_staff(&self) -> bool {
		self.0.is_staff
	}

	/// Date when the user joined
	async fn date_joined(&self) -> DateTime<Utc> {
		self.0.date_joined
	}
}

/// Input for user registration
#[derive(InputObject)]
pub struct CreateUserInput {
	/// Username (required)
	pub username: String,
	/// Email address (required)
	pub email: String,
	/// Password (required)
	pub password: String,
	/// First name (optional)
	pub first_name: Option<String>,
	/// Last name (optional)
	pub last_name: Option<String>,
}

/// Input for user login
#[derive(InputObject)]
pub struct LoginInput {
	/// Username
	pub username: String,
	/// Password
	pub password: String,
}

/// Authentication payload returned after login/register
pub struct AuthPayload {
	/// JWT token
	pub token: String,
	/// Authenticated user
	pub user: User,
}

#[Object]
impl AuthPayload {
	/// JWT token for authentication
	async fn token(&self) -> &str {
		&self.token
	}

	/// The authenticated user
	async fn user(&self) -> UserType {
		UserType(self.user.clone())
	}
}
