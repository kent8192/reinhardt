//! Note request and response types

use serde::{Deserialize, Serialize};

/// Request to create a new note
#[derive(Debug, Deserialize)]
pub struct CreateNoteRequest {
	pub title: String,
	pub content: String,
}

/// Note response
#[derive(Debug, Serialize)]
pub struct NoteResponse {
	pub id: String,
	pub title: String,
	pub content: String,
	pub owner_id: String,
	pub created_at: String,
}

impl From<&crate::apps::notes::models::Note> for NoteResponse {
	fn from(note: &crate::apps::notes::models::Note) -> Self {
		Self {
			id: note.id.to_string(),
			title: note.title.clone(),
			content: note.content.clone(),
			owner_id: note.owner_id.to_string(),
			created_at: note.created_at.to_rfc3339(),
		}
	}
}
