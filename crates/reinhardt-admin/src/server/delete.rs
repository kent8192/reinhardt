//! Delete operation Server Functions
//!
//! Provides delete operations for admin models (single and bulk).

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{
	AdminDatabase, AdminRecord, AdminSite, BulkDeleteRequest, BulkDeleteResponse,
};
use crate::types::MutationResponse;
use reinhardt_di::Depends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::audit;
#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::limits::MAX_BULK_DELETE_IDS;
#[cfg(server)]
use super::security::require_csrf_token;

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
/// let response = delete_record("User".to_string(), "42".to_string(), "token".to_string()).await?;
/// println!("Deleted: {}", response.message);
/// ```
#[server_fn]
pub async fn delete_record(
	model_name: String,
	id: String,
	csrf_token: String,
	#[inject] site: Depends<AdminSite>,
	#[inject] db: Depends<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<MutationResponse, ServerFnError> {
	// CSRF token validation (double-submit cookie pattern)
	require_csrf_token(&csrf_token, &http_request.inner().headers)?;

	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Delete)
		.await?;

	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	let user_id = auth.user_id().unwrap_or("unknown").to_string();

	let result = db
		.delete::<AdminRecord>(table_name, pk_field, &id)
		.await
		.map_server_fn_error();

	// Check for database errors first, logging failure before returning
	let affected = match result {
		Err(e) => {
			audit::log_delete(&user_id, &model_name, &id, false);
			return Err(e);
		}
		Ok(n) => n,
	};

	// Return 404 error when no record was found with the given ID.
	// Only log success=true after confirming the record was actually deleted.
	if affected == 0 {
		audit::log_delete(&user_id, &model_name, &id, false);
		return Err(ServerFnError::server(
			404,
			format!("{} not found", model_name),
		));
	}

	audit::log_delete(&user_id, &model_name, &id, true);

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
///     csrf_token: "token".to_string(),
///     ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
/// };
/// let response = bulk_delete_records("User".to_string(), request).await?;
/// println!("Deleted {} items", response.deleted);
/// ```
#[server_fn]
pub async fn bulk_delete_records(
	model_name: String,
	request: BulkDeleteRequest,
	#[inject] site: Depends<AdminSite>,
	#[inject] db: Depends<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<BulkDeleteResponse, ServerFnError> {
	// CSRF token validation (double-submit cookie pattern)
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Delete)
		.await?;

	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	let user_id = auth.user_id().unwrap_or("unknown").to_string();

	let ids = request.ids;
	if ids.len() > MAX_BULK_DELETE_IDS {
		return Err(ServerFnError::application(format!(
			"Too many IDs for bulk delete: {} exceeds maximum of {}",
			ids.len(),
			MAX_BULK_DELETE_IDS
		)));
	}

	let result = db
		.bulk_delete::<AdminRecord>(table_name, pk_field, ids.clone())
		.await
		.map_server_fn_error();

	let success = result.is_ok();
	let affected_count = result.as_ref().copied().unwrap_or(0);
	audit::log_bulk_delete(&user_id, &model_name, &ids, affected_count, success);

	let affected = result?;

	Ok(BulkDeleteResponse {
		success: affected > 0,
		deleted: affected,
		message: format!("Deleted {} {} items", affected, model_name),
	})
}
