//! Update operation Server Function
//!
//! Provides update operations for admin models.

use crate::adapters::{AdminDatabase, AdminRecord, AdminSite};
use crate::types::{MutationRequest, MutationResponse};
use reinhardt_pages::server_fn::{ServerFnError, ServerFnRequest, server_fn};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use super::audit;
#[cfg(not(target_arch = "wasm32"))]
use super::error::{AdminAuth, MapServerFnError};
#[cfg(not(target_arch = "wasm32"))]
use super::security::sanitize_mutation_values;
#[cfg(not(target_arch = "wasm32"))]
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
/// let request = MutationRequest { data };
/// let response = update_record("User".to_string(), "42".to_string(), request).await?;
/// println!("Updated: {}", response.message);
/// ```
#[server_fn(use_inject = true)]
pub async fn update_record(
	model_name: String,
	id: String,
	request: MutationRequest,
	#[inject] site: Arc<AdminSite>,
	#[inject] db: Arc<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
) -> Result<MutationResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	auth.require_change_permission(&model_name)?;

	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	// Validate input data before database operation
	validate_mutation_data(&request.data, model_admin.as_ref(), true).map_server_fn_error()?;

	// Sanitize string values to prevent stored XSS
	let mut sanitized_data = request.data;
	sanitize_mutation_values(&mut sanitized_data);

	let user_id = auth.user_id().unwrap_or("unknown").to_string();

	let result = db
		.update::<AdminRecord>(table_name, pk_field, &id, sanitized_data.clone())
		.await
		.map_server_fn_error();

	let success = result.is_ok();
	audit::log_update(&user_id, &model_name, &id, &sanitized_data, success);

	let affected = result?;

	Ok(MutationResponse {
		success: true,
		message: format!("{} updated successfully", model_name),
		affected: Some(affected),
		data: None,
	})
}
