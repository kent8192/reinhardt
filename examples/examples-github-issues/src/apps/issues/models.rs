//! Issue model

use chrono::{DateTime, Utc};
use reinhardt::model;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Issue state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
	/// Issue is open
	Open,
	/// Issue is closed
	Closed,
}

/// Issue model
///
/// Represents an issue within a project.
/// Each issue has a project-scoped sequential number.
#[model(app_label = "issues", table_name = "issues")]
#[derive(Serialize, Deserialize)]
pub struct Issue {
	/// Primary key
	#[field(primary_key = true)]
	id: Uuid,

	/// Project ID (references Project model)
	project_id: Uuid,

	/// Project-scoped sequential number
	/// This number is unique within a project (e.g., #1, #2, #3)
	number: i32,

	/// Issue title
	#[field(max_length = 255)]
	title: String,

	/// Issue body (supports Markdown)
	#[field(max_length = 10000)]
	body: String,

	/// Issue state (open or closed)
	/// Stored as string: "open" or "closed"
	#[field(max_length = 20, default = "open")]
	state: String,

	/// Author user ID (references User model)
	author_id: Uuid,

	/// Creation timestamp
	#[field(auto_now_add = true)]
	created_at: DateTime<Utc>,

	/// Last update timestamp
	#[field(auto_now = true)]
	updated_at: DateTime<Utc>,
}
