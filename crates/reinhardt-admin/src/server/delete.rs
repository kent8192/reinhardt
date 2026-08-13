//! Delete operation Server Functions
//!
//! Provides delete operations for admin models (single and bulk).

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminSite, BulkDeleteResponse};
#[cfg(server)]
use crate::core::database::canonicalize_pk_value;
#[cfg(server)]
use crate::core::history::insert_history_event;
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey};
use crate::types::MutationResponse;
#[cfg(server)]
use reinhardt_di::KeyedDepends;
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
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<crate::types::MutationResponse, ServerFnError> {
	// CSRF token validation (double-submit cookie pattern)
	require_csrf_token(&csrf_token, &http_request.inner().headers)?;

	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Delete)
		.await?;

	let model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let object_id = canonicalize_pk_value(&table_name, &pk_field, &id);

	let actor = user.get_username().to_string();
	let audit_user_id = auth.user_id().unwrap_or("unknown").to_string();
	let connection = *db.connection();
	let result: reinhardt_core::exception::Result<_> = async {
		connection
			.atomic_write(async |transaction| {
				let affected = db
					.delete_with_executor(transaction, &table_name, &pk_field, &object_id)
					.await?;
				if affected > 0 {
					let event = audit::new_history_event(
						&actor,
						"DELETE",
						&model_name,
						&table_name,
						&object_id,
						Vec::new(),
						affected,
					);
					insert_history_event(transaction, &event).await?;
				}
				Ok(affected)
			})
			.await
	}
	.await;

	// Check for database errors first, logging failure before returning
	let affected = match result {
		Err(_) => {
			audit::log_delete(&audit_user_id, &model_name, &id, false);
			return Err(ServerFnError::server(500, "Database operation failed"));
		}
		Ok(n) => n,
	};

	// Return 404 error when no record was found with the given ID.
	// Only log success=true after confirming the record was actually deleted.
	if affected == 0 {
		audit::log_delete(&audit_user_id, &model_name, &id, false);
		return Err(ServerFnError::server(
			404,
			format!("{} not found", model_name),
		));
	}

	audit::log_delete(&audit_user_id, &model_name, &id, true);

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
	request: crate::adapters::BulkDeleteRequest,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<crate::adapters::BulkDeleteResponse, ServerFnError> {
	// CSRF token validation (double-submit cookie pattern)
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Delete)
		.await?;

	let model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let actor = user.get_username().to_string();
	let audit_user_id = auth.user_id().unwrap_or("unknown").to_string();

	let ids = request.ids;
	if ids.len() > MAX_BULK_DELETE_IDS {
		return Err(ServerFnError::application(format!(
			"Too many IDs for bulk delete: {} exceeds maximum of {}",
			ids.len(),
			MAX_BULK_DELETE_IDS
		)));
	}

	let connection = *db.connection();
	let result: reinhardt_core::exception::Result<_> = async {
		connection
			.atomic_write(async |transaction| {
				let mut affected = 0;
				for id in &ids {
					let object_id = canonicalize_pk_value(&table_name, &pk_field, id);
					let deleted = db
						.delete_with_executor(transaction, &table_name, &pk_field, &object_id)
						.await?;
					if deleted > 0 {
						let event = audit::new_history_event(
							&actor,
							"BULK_DELETE",
							&model_name,
							&table_name,
							&object_id,
							Vec::new(),
							deleted,
						);
						insert_history_event(transaction, &event).await?;
						affected += deleted;
					}
				}
				Ok(affected)
			})
			.await
	}
	.await;

	let success = result.is_ok();
	let affected_count = result.as_ref().copied().unwrap_or(0);
	audit::log_bulk_delete(&audit_user_id, &model_name, &ids, affected_count, success);

	let affected = result.map_err(|_| ServerFnError::server(500, "Database operation failed"))?;

	Ok(BulkDeleteResponse {
		success: affected > 0,
		deleted: affected,
		message: format!("Deleted {} {} items", affected, model_name),
	})
}
