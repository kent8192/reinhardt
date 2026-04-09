//! Update operation Server Function
//!
//! Provides update operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite};
use crate::types::{MutationRequest, MutationResponse};
use reinhardt_di::Depends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::audit;
#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::security::{require_csrf_token, sanitize_mutation_values};
#[cfg(server)]
use super::validation::validate_mutation_data;

/// Update an existing model instance
///
/// Updates a record in the database by ID using the provided field data.
/// Returns the number of affected rows (typically 1) on success.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Authentication
///
/// Requires staff (admin) permission and change permission for the model.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::update_record;
/// use reinhardt_admin::types::MutationRequest;
/// use std::collections::HashMap;
///
/// // Client-side usage (automatically generates HTTP request)
/// let mut data = HashMap::new();
/// data.insert("email".to_string(), serde_json::json!("alice.new@example.com"));
///
/// let request = MutationRequest { csrf_token: "token".to_string(), data };
/// let response = update_record("User".to_string(), "42".to_string(), request).await?;
/// println!("Updated: {}", response.message);
/// ```
#[server_fn]
pub async fn update_record(
	model_name: String,
	id: String,
	request: MutationRequest,
	#[inject] site: Depends<AdminSite>,
	#[inject] db: Depends<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<MutationResponse, ServerFnError> {
	// CSRF token validation (double-submit cookie pattern)
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Change)
		.await?;

	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	// Validate input data before database operation
	validate_mutation_data(&request.data, model_admin.as_ref(), true).map_server_fn_error()?;

	// Sanitize string values to prevent stored XSS
	let mut sanitized_data = request.data;
	sanitize_mutation_values(&mut sanitized_data);

	// Inject current timestamp for auto_now fields (updated on every save)
	super::create::inject_auto_now_timestamps(&mut sanitized_data, table_name);

	let user_id = auth.user_id().unwrap_or("unknown").to_string();

	let result = db
		.update::<AdminRecord>(table_name, pk_field, &id, sanitized_data.clone())
		.await
		.map_server_fn_error();

	// Check for database errors first, logging failure before returning
	let affected = match result {
		Err(e) => {
			audit::log_update(&user_id, &model_name, &id, &sanitized_data, false);
			return Err(e);
		}
		Ok(n) => n,
	};

	// Return 404 error when no record was found with the given ID.
	// Only log success=true after confirming the record was actually updated.
	if affected == 0 {
		audit::log_update(&user_id, &model_name, &id, &sanitized_data, false);
		return Err(ServerFnError::server(
			404,
			format!("{} not found", model_name),
		));
	}

	audit::log_update(&user_id, &model_name, &id, &sanitized_data, true);

	Ok(MutationResponse {
		success: true,
		message: format!("{} updated successfully", model_name),
		affected: Some(affected),
		data: None,
	})
}
