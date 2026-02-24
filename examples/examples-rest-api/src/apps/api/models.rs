//! Models for api app
//!
//! Database models for REST API example

use reinhardt::prelude::*;
use serde::{Deserialize, Serialize};

/// Article model
///
/// Represents a blog article or post
#[derive(Serialize, Deserialize)]
#[model(app_label = "api", table_name = "articles")]
pub struct Article {
	/// Primary key
	#[field(primary_key = true)]
	pub id: i64,

	/// Article title
	#[field(max_length = 255)]
	pub title: String,

	/// Article content
	#[field(max_length = 65535)]
	pub content: String,

	/// Author name
	#[field(max_length = 100)]
	pub author: String,

	/// Publication status
	#[field(default = false)]
	pub published: bool,

	/// Creation timestamp
	#[field(auto_now_add = true)]
	pub created_at: chrono::DateTime<chrono::Utc>,

	/// Last update timestamp
	#[field(auto_now = true)]
	pub updated_at: chrono::DateTime<chrono::Utc>,
}
