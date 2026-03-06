//! Admin views with IsAdminUser-style permission checking
//!
//! These endpoints require the authenticated user to be a superuser or staff member.

use reinhardt::{Request, Response, StatusCode, ViewResult, get};

use crate::apps::users::models::UserStorage;
use crate::apps::users::serializers::UserResponse;
use crate::apps::users::views::extract_authenticated_user;

/// Helper to extract UserStorage from request extensions
fn get_user_storage(req: &Request) -> Result<UserStorage, String> {
	req.extensions
		.get::<UserStorage>()
		.ok_or_else(|| "UserStorage not found in request extensions".to_string())
}

/// List all users (IsAdminUser permission)
///
/// Only accessible to superusers and staff members.
/// Protected with Basic auth for admin access.
#[get("/api/admin/users", name = "admin_list_users")]
pub async fn list_users(req: Request) -> ViewResult<Response> {
	let storage = get_user_storage(&req)?;

	// Authenticate and verify admin status
	let user = extract_authenticated_user(&req, &storage).await?;

	// IsAdminUser permission check
	if !user.is_superuser && !user.is_staff {
		let error = serde_json::json!({"error": "Admin access required"});
		return Ok(Response::new(StatusCode::FORBIDDEN)
			.with_header("Content-Type", "application/json")
			.with_body(serde_json::to_string(&error).unwrap_or_default()));
	}

	let users = storage.list_users().await;
	let responses: Vec<UserResponse> = users.iter().map(UserResponse::from).collect();

	let json = serde_json::to_string(&responses)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Get admin dashboard stats (IsAdminUser permission)
#[get("/api/admin/stats", name = "admin_stats")]
pub async fn stats(req: Request) -> ViewResult<Response> {
	let storage = get_user_storage(&req)?;

	let user = extract_authenticated_user(&req, &storage).await?;

	// IsAdminUser permission check
	if !user.is_superuser && !user.is_staff {
		let error = serde_json::json!({"error": "Admin access required"});
		return Ok(Response::new(StatusCode::FORBIDDEN)
			.with_header("Content-Type", "application/json")
			.with_body(serde_json::to_string(&error).unwrap_or_default()));
	}

	let users = storage.list_users().await;
	let active_count = users.iter().filter(|u| u.is_active).count();
	let staff_count = users.iter().filter(|u| u.is_staff).count();

	let stats = serde_json::json!({
		"total_users": users.len(),
		"active_users": active_count,
		"staff_users": staff_count,
	});

	let json = serde_json::to_string(&stats)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}
