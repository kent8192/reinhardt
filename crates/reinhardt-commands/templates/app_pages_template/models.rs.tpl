//! Models module for {{ app_name }} app
//!
//! Use the `#[user]` macro to auto-generate `BaseUser`, `FullUser`,
//! `PermissionsMixin`, and `AuthIdentity` trait implementations for user models.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt::macros::user;
//! use reinhardt::Argon2Hasher;
//!
//! #[user(hasher = Argon2Hasher, username_field = "email")]
//! #[model(table_name = "users")]
//! pub struct User {
//!     #[field(primary_key = true)]
//!     pub id: uuid::Uuid,
//!     #[field(max_length = 255, unique = true)]
//!     pub email: String,
//!     #[field(max_length = 255)]
//!     pub password_hash: Option<String>,
//!     #[field]
//!     pub last_login: Option<chrono::DateTime<chrono::Utc>>,
//!     #[field(default = true)]
//!     pub is_active: bool,
//!     #[field(default = false)]
//!     pub is_superuser: bool,
//!     #[field(auto_now_add = true)]
//!     pub created_at: chrono::DateTime<chrono::Utc>,
//! }
//! ```
