//! Note model with in-memory storage

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// A simple note owned by a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
	pub id: Uuid,
	pub title: String,
	pub content: String,
	pub owner_id: Uuid,
	pub created_at: DateTime<Utc>,
}

/// In-memory note storage
#[derive(Clone, Default)]
pub struct NoteStorage {
	notes: Arc<RwLock<HashMap<Uuid, Note>>>,
}

impl NoteStorage {
	/// Create a new empty note storage
	pub fn new() -> Self {
		Self {
			notes: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Add a note
	pub async fn add_note(&self, note: Note) {
		self.notes.write().await.insert(note.id, note);
	}

	/// Get a note by ID
	pub async fn get_note(&self, id: &Uuid) -> Option<Note> {
		self.notes.read().await.get(id).cloned()
	}

	/// List notes owned by a specific user
	pub async fn list_by_owner(&self, owner_id: &Uuid) -> Vec<Note> {
		self.notes
			.read()
			.await
			.values()
			.filter(|n| n.owner_id == *owner_id)
			.cloned()
			.collect()
	}

	/// Delete a note by ID, returning the deleted note if it existed
	pub async fn delete_note(&self, id: &Uuid) -> Option<Note> {
		self.notes.write().await.remove(id)
	}
}
