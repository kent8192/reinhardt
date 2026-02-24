//! Delete operation Server Functions
//!
//! Provides delete operations for admin models (single and bulk).

use crate::adapters::{
	AdminDatabase, AdminRecord, AdminSite, BulkDeleteRequest, BulkDeleteResponse,
};
use crate::types::MutationResponse;
use reinhardt_pages::server_fn::{ServerFnError, ServerFnRequest, server_fn};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use super::audit;
#[cfg(not(target_arch = "wasm32"))]
use super::error::{AdminAuth, MapServerFnError};

/// Delete a single model instance by ID
///
/// Removes a record from the database by its primary key.
/// Returns the number of affected rows (typically 1) on success.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Authentication
///
/// Requires staff (admin) permission and delete permission for the model.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::delete_record;
///
/// // Client-side usage (automatically generates HTTP request)
/// let response = delete_record("User".to_string(), "42".to_string()).await?;
/// println!("Deleted: {}", response.message);
/// ```
#[server_fn(use_inject = true)]
pub async fn delete_record(
	model_name: String,
	id: String,
	#[inject] site: Arc<AdminSite>,
	#[inject] db: Arc<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
) -> Result<MutationResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	auth.require_delete_permission(&model_name)?;

	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	let user_id = auth.user_id().unwrap_or("unknown").to_string();

	let result = db
		.delete::<AdminRecord>(table_name, pk_field, &id)
		.await
		.map_server_fn_error();

	let success = result.is_ok();
	audit::log_delete(&user_id, &model_name, &id, success);

	let affected = result?;

	Ok(MutationResponse {
		success: true,
		message: format!("{} deleted successfully", model_name),
		affected: Some(affected),
		data: None,
	})
}

/// Delete multiple model instances by IDs (bulk delete)
///
/// Removes multiple records from the database using their primary keys.
/// Returns the total number of deleted rows.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Authentication
///
/// Requires staff (admin) permission and delete permission for the model.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::bulk_delete_records;
/// use reinhardt_admin::types::BulkDeleteRequest;
///
/// // Client-side usage (automatically generates HTTP request)
/// let request = BulkDeleteRequest {
///     ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
/// };
/// let response = bulk_delete_records("User".to_string(), request).await?;
/// println!("Deleted {} items", response.deleted);
/// ```
#[server_fn(use_inject = true)]
pub async fn bulk_delete_records(
	model_name: String,
	request: BulkDeleteRequest,
	#[inject] site: Arc<AdminSite>,
	#[inject] db: Arc<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
) -> Result<BulkDeleteResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	auth.require_delete_permission(&model_name)?;

	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	let user_id = auth.user_id().unwrap_or("unknown").to_string();

	let ids = request.ids;
	let result = db
		.bulk_delete::<AdminRecord>(table_name, pk_field, ids.clone())
		.await
		.map_server_fn_error();

	let success = result.is_ok();
	let affected_count = result.as_ref().copied().unwrap_or(0);
	audit::log_bulk_delete(&user_id, &model_name, &ids, affected_count, success);

	let affected = result?;

	Ok(BulkDeleteResponse {
		success: true,
		deleted: affected,
		message: format!("Deleted {} {} items", affected, model_name),
	})
}
