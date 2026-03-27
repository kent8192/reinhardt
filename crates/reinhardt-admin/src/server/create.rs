//! Create operation Server Function
//!
//! Provides create operations for admin models.

use super::user::AdminDefaultUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite};
use crate::types::{MutationRequest, MutationResponse};
use reinhardt_auth::AuthUser;
use reinhardt_pages::server_fn::{ServerFnError, ServerFnRequest, server_fn};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use super::audit;
#[cfg(not(target_arch = "wasm32"))]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(not(target_arch = "wasm32"))]
use super::security::{require_csrf_token, sanitize_mutation_values};
#[cfg(not(target_arch = "wasm32"))]
use super::validation::validate_mutation_data;

/// Create a new model instance
///
/// Inserts a new record into the database using the provided field data.
/// Returns the number of affected rows (typically 1) on success.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Authentication
///
/// Requires authentication and add permission for the model.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::create_record;
/// use reinhardt_admin::types::MutationRequest;
/// use std::collections::HashMap;
///
/// // Client-side usage (automatically generates HTTP request)
/// let mut data = HashMap::new();
/// data.insert("username".to_string(), serde_json::json!("alice"));
/// data.insert("email".to_string(), serde_json::json!("alice@example.com"));
///
/// let request = MutationRequest { csrf_token: "token".to_string(), data };
/// let response = create_record("User".to_string(), request).await?;
/// println!("Created: {}", response.message);
/// ```
#[server_fn]
pub async fn create_record(
	model_name: String,
	request: MutationRequest,
	#[inject] site: Arc<AdminSite>,
	#[inject] db: Arc<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AuthUser(user): AuthUser<AdminDefaultUser>,
) -> Result<MutationResponse, ServerFnError> {
	// CSRF token validation (double-submit cookie pattern)
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(
		model_admin.as_ref(),
		&user as &dyn crate::core::AdminUser,
		ModelPermission::Add,
	)
	.await?;
	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	// Validate input data before database operation
	validate_mutation_data(&request.data, model_admin.as_ref(), false).map_server_fn_error()?;

	// Sanitize string values to prevent stored XSS
	let mut sanitized_data = request.data;
	sanitize_mutation_values(&mut sanitized_data);

	let user_id = auth.user_id().unwrap_or("unknown").to_string();

	let result = db
		.create::<AdminRecord>(table_name, Some(pk_field), sanitized_data.clone())
		.await
		.map_server_fn_error();

	let success = result.is_ok();
	audit::log_create(&user_id, &model_name, &sanitized_data, success);

	let affected = result?;

	Ok(MutationResponse {
		success: true,
		message: format!("{} created successfully", model_name),
		affected: Some(affected),
		data: None,
	})
}
