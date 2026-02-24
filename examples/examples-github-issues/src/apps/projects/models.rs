//! Project and ProjectMember models

use chrono::{DateTime, Utc};
use reinhardt::model;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Project visibility levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectVisibility {
	/// Public project (visible to everyone)
	Public,
	/// Private project (visible only to members)
	Private,
}

/// Project model
///
/// Represents a project that contains issues.
#[model(app_label = "projects", table_name = "projects")]
#[derive(Serialize, Deserialize)]
pub struct Project {
	/// Primary key
	#[field(primary_key = true)]
	id: Uuid,

	/// Project name (unique)
	#[field(max_length = 255, unique = true)]
	name: String,

	/// Project description
	#[field(max_length = 1000)]
	description: String,

	/// Project visibility (public or private)
	/// Stored as string: "public" or "private"
	#[field(max_length = 20, default = "public")]
	visibility: String,

	/// Owner user ID (references User model)
	owner_id: Uuid,

	/// Creation timestamp
	#[field(auto_now_add = true)]
	created_at: DateTime<Utc>,
}

/// ProjectMember role levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
	/// Project owner (full permissions)
	Owner,
	/// Project maintainer (can manage issues and members)
	Maintainer,
	/// Project member (can create and edit issues)
	Member,
	/// Project viewer (read-only access)
	Viewer,
}

/// ProjectMember model
///
/// Represents a user's membership in a project.
/// ManyToMany relationship between Project and User.
#[model(app_label = "projects", table_name = "project_members")]
#[derive(Serialize, Deserialize)]
pub struct ProjectMember {
	/// Primary key
	#[field(primary_key = true)]
	id: Uuid,

	/// Project ID (references Project model)
	project_id: Uuid,

	/// User ID (references User model)
	user_id: Uuid,

	/// Member role in the project
	/// Stored as string: "owner", "maintainer", "member", or "viewer"
	#[field(max_length = 20, default = "member")]
	role: String,

	/// Join timestamp
	#[field(auto_now_add = true)]
	joined_at: DateTime<Utc>,
}
