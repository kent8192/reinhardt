//! Database-backed permission model.
//!
//! Provides a structured permission model that maps to the `auth_permission` table,
//! following Django's permission system design.

use reinhardt_core::macros::model;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Database-backed permission model.
///
/// Represents a single permission in the Django-style format: `"app_label.action_model"`.
/// Used with `ManyToManyField` on user models for structured permission management.
///
/// Named `AuthPermission` to avoid conflict with the `Permission` trait in `core::permission`.
///
/// # Table
///
/// Maps to `auth_permission` table with columns:
/// - `id` (UUID, primary key)
/// - `name` (VARCHAR(255), human-readable description)
/// - `codename` (VARCHAR(100), machine-readable identifier)
/// - `app_label` (VARCHAR(100), owning application label)
#[model(app_label = "auth", table_name = "auth_permission")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthPermission {
	/// Unique identifier for the permission.
	#[field(primary_key = true)]
	pub id: Uuid,

	/// Human-readable name (e.g., "Can add article").
	#[field(max_length = 255)]
	pub name: String,

	/// Machine-readable codename (e.g., "add_article").
	#[field(max_length = 100)]
	pub codename: String,

	/// App label this permission belongs to (e.g., "blog").
	#[field(max_length = 100)]
	pub app_label: String,
}
