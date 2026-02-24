//! Models for todos app
//!
//! Database models for TODO list

use reinhardt::prelude::*;
use serde::{Deserialize, Serialize};

/// Todo model
///
/// Represents a single TODO item
#[derive(Serialize, Deserialize)]
#[model(app_label = "todos", table_name = "todos")]
pub struct Todo {
	/// Primary key (None for auto-increment on insert)
	#[field(primary_key = true)]
	pub id: Option<i64>,

	/// Task title
	#[field(max_length = 255)]
	pub title: String,

	/// Task description
	#[field(max_length = 1000, null = true)]
	pub description: Option<String>,

	/// Completion status
	#[field(default = false)]
	pub completed: bool,

	/// Creation timestamp
	#[field(auto_now_add = true)]
	pub created_at: chrono::DateTime<chrono::Utc>,

	/// Last update timestamp
	#[field(auto_now = true)]
	pub updated_at: chrono::DateTime<chrono::Utc>,
}
