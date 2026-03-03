//! Author model definition.

use serde::{Deserialize, Serialize};

/// Author model representing a content creator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
	pub id: i64,
	pub name: String,
	pub bio: String,
	pub is_active: bool,
}
