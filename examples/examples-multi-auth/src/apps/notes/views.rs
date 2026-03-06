//! Note CRUD views with IsAuthenticated-style permission checking
//!
//! All endpoints require authentication (JWT Bearer, API Token, or Basic).

use chrono::Utc;
use reinhardt::{Request, Response, StatusCode, ViewResult, delete, get, post};
use uuid::Uuid;

use crate::apps::notes::models::{Note, NoteStorage};
use crate::apps::notes::serializers::{CreateNoteRequest, NoteResponse};
use crate::apps::users::models::UserStorage;
use crate::apps::users::views::extract_authenticated_user;

/// Helper to extract NoteStorage from request extensions
fn get_note_storage(req: &Request) -> Result<NoteStorage, String> {
	req.extensions
		.get::<NoteStorage>()
		.ok_or_else(|| "NoteStorage not found in request extensions".to_string())
}

/// Helper to extract UserStorage from request extensions
fn get_user_storage(req: &Request) -> Result<UserStorage, String> {
	req.extensions
		.get::<UserStorage>()
		.ok_or_else(|| "UserStorage not found in request extensions".to_string())
}

/// List notes for the authenticated user (IsAuthenticated permission)
#[get("/api/notes", name = "notes_list")]
pub async fn list_notes(req: Request) -> ViewResult<Response> {
	let user_storage = get_user_storage(&req)?;
	let note_storage = get_note_storage(&req)?;

	// Authenticate user (IsAuthenticated check)
	let user = extract_authenticated_user(&req, &user_storage).await?;

	let notes = note_storage.list_by_owner(&user.id).await;
	let responses: Vec<NoteResponse> = notes.iter().map(NoteResponse::from).collect();

	let json = serde_json::to_string(&responses)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Create a new note (IsAuthenticated permission)
#[post("/api/notes", name = "notes_create")]
pub async fn create_note(req: Request) -> ViewResult<Response> {
	let user_storage = get_user_storage(&req)?;
	let note_storage = get_note_storage(&req)?;

	// Authenticate user
	let user = extract_authenticated_user(&req, &user_storage).await?;

	let body: CreateNoteRequest = req
		.json()
		.map_err(|e| format!("Invalid request body: {}", e))?;

	let note = Note {
		id: Uuid::new_v4(),
		title: body.title,
		content: body.content,
		owner_id: user.id,
		created_at: Utc::now(),
	};

	let response = NoteResponse::from(&note);
	note_storage.add_note(note).await;

	let json = serde_json::to_string(&response)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::CREATED)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Get a specific note (IsAuthenticated + ownership check)
#[get("/api/notes/{id}", name = "notes_detail")]
pub async fn get_note(req: Request) -> ViewResult<Response> {
	let user_storage = get_user_storage(&req)?;
	let note_storage = get_note_storage(&req)?;

	let user = extract_authenticated_user(&req, &user_storage).await?;

	let note_id = req
		.path_params
		.get("id")
		.ok_or("Missing note id")?
		.parse::<Uuid>()
		.map_err(|_| "Invalid note id format")?;

	let note = note_storage
		.get_note(&note_id)
		.await
		.ok_or("Note not found")?;

	// Ownership check
	if note.owner_id != user.id {
		return Ok(Response::new(StatusCode::FORBIDDEN)
			.with_header("Content-Type", "application/json")
			.with_body(r#"{"error":"Access denied"}"#));
	}

	let response = NoteResponse::from(&note);
	let json = serde_json::to_string(&response)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Delete a note (IsAuthenticated + ownership check)
#[delete("/api/notes/{id}", name = "notes_delete")]
pub async fn delete_note(req: Request) -> ViewResult<Response> {
	let user_storage = get_user_storage(&req)?;
	let note_storage = get_note_storage(&req)?;

	let user = extract_authenticated_user(&req, &user_storage).await?;

	let note_id = req
		.path_params
		.get("id")
		.ok_or("Missing note id")?
		.parse::<Uuid>()
		.map_err(|_| "Invalid note id format")?;

	// Check existence and ownership before deletion
	let note = note_storage
		.get_note(&note_id)
		.await
		.ok_or("Note not found")?;

	if note.owner_id != user.id {
		return Ok(Response::new(StatusCode::FORBIDDEN)
			.with_header("Content-Type", "application/json")
			.with_body(r#"{"error":"Access denied"}"#));
	}

	note_storage.delete_note(&note_id).await;

	Ok(Response::new(StatusCode::NO_CONTENT))
}
