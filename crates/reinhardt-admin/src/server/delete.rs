//! Delete operation Server Functions
//!
//! Provides delete operations for admin models (single and bulk).

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(all(server, feature = "file-uploads"))]
use crate::adapters::ModelAdmin;
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
#[cfg(all(server, feature = "file-uploads"))]
use super::{
	multipart::{cleanup_deleted_file_references, deleted_file_references},
	type_inference::translate_physical_field_names_to_logical,
};
#[cfg(all(server, feature = "file-uploads"))]
use reinhardt_db::orm::OrmExecutor;

#[cfg(all(server, feature = "file-uploads"))]
struct CapturedInlineFileValues {
	table_name: String,
	pk_field: String,
	object_id: String,
	values: std::collections::HashMap<String, serde_json::Value>,
}

#[cfg(all(server, feature = "file-uploads"))]
fn primary_key_string(value: &serde_json::Value) -> Option<String> {
	match value {
		serde_json::Value::String(value) => Some(value.clone()),
		serde_json::Value::Number(value) => Some(value.to_string()),
		_ => None,
	}
}

#[cfg(all(server, feature = "file-uploads"))]
async fn capture_inline_file_values<E>(
	db: &AdminDatabase,
	site: &AdminSite,
	model_admin: &dyn ModelAdmin,
	parent_id: &str,
	transaction: &mut E,
) -> crate::types::AdminResult<Vec<CapturedInlineFileValues>>
where
	E: OrmExecutor,
{
	let mut captured_values = Vec::new();
	for inline in model_admin.inlines() {
		let child_admin = site.get_model_admin_by_table_name(inline.adapter().table_name())?;
		let rows = db
			.get_all_by_field_with_executor_for_update(
				transaction,
				child_admin.table_name(),
				inline.foreign_key(),
				parent_id,
			)
			.await?;
		for mut values in rows {
			translate_physical_field_names_to_logical(child_admin.table_name(), &mut values)?;
			let Some(object_id) = values
				.get(child_admin.pk_field())
				.and_then(primary_key_string)
			else {
				tracing::warn!(
					table = child_admin.table_name(),
					field = child_admin.pk_field(),
					"Inline file references were skipped because the child primary key was unavailable"
				);
				continue;
			};
			captured_values.push(CapturedInlineFileValues {
				table_name: child_admin.table_name().to_owned(),
				pk_field: child_admin.pk_field().to_owned(),
				object_id: canonicalize_pk_value(
					child_admin.table_name(),
					child_admin.pk_field(),
					&object_id,
				),
				values,
			});
		}
	}
	Ok(captured_values)
}

#[cfg(all(server, feature = "file-uploads"))]
async fn confirmed_inline_deleted_values<E>(
	db: &AdminDatabase,
	captured_values: Vec<CapturedInlineFileValues>,
	transaction: &mut E,
) -> crate::types::AdminResult<Vec<(String, std::collections::HashMap<String, serde_json::Value>)>>
where
	E: OrmExecutor,
{
	let mut deleted_values = Vec::new();
	for captured in captured_values {
		let survived = db
			.get_with_executor_for_update(
				transaction,
				&captured.table_name,
				&captured.pk_field,
				&captured.object_id,
			)
			.await?
			.is_some();
		if !survived {
			deleted_values.push((captured.table_name, captured.values));
		}
	}
	Ok(deleted_values)
}

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
				#[cfg(feature = "file-uploads")]
				let parent_exists = db
					.get_with_executor_for_update(transaction, &table_name, &pk_field, &object_id)
					.await?
					.is_some();
				#[cfg(feature = "file-uploads")]
				let captured_inline_values = if parent_exists {
					capture_inline_file_values(
						db.as_ref(),
						site.as_ref(),
						model_admin.as_ref(),
						&object_id,
						transaction,
					)
					.await?
				} else {
					Vec::new()
				};
				#[cfg(feature = "file-uploads")]
				let (affected, mut deleted_values) = db
					.delete_with_executor_returning(transaction, &table_name, &pk_field, &object_id)
					.await?;
				#[cfg(feature = "file-uploads")]
				let inline_deleted_values = if affected > 0 {
					confirmed_inline_deleted_values(
						db.as_ref(),
						captured_inline_values,
						transaction,
					)
					.await?
				} else {
					Vec::new()
				};
				#[cfg(feature = "file-uploads")]
				if let Some(values) = deleted_values.as_mut()
					&& let Err(error) =
						translate_physical_field_names_to_logical(&table_name, values)
				{
					tracing::warn!(error = %error, "Deleted file references could not be translated");
					deleted_values = None;
				}
				#[cfg(not(feature = "file-uploads"))]
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
				#[cfg(feature = "file-uploads")]
				return Ok((affected, deleted_values, inline_deleted_values));
				#[cfg(not(feature = "file-uploads"))]
				Ok(affected)
			})
			.await
	}
	.await;

	// Check for database errors first, logging failure before returning
	#[cfg(feature = "file-uploads")]
	let (affected, deleted_values, inline_deleted_values) = match result {
		Err(_) => {
			audit::log_delete(&audit_user_id, &model_name, &id, false);
			return Err(ServerFnError::server(500, "Database operation failed"));
		}
		Ok(n) => n,
	};
	#[cfg(not(feature = "file-uploads"))]
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

	#[cfg(feature = "file-uploads")]
	let mut file_references = deleted_file_references(model_admin.as_ref(), deleted_values.as_ref());
	#[cfg(feature = "file-uploads")]
	for (table_name, values) in inline_deleted_values {
		match site.get_model_admin_by_table_name(&table_name) {
			Ok(child_admin) => {
				file_references.extend(deleted_file_references(child_admin.as_ref(), Some(&values)))
			}
			Err(error) => tracing::warn!(
				table = table_name.as_str(),
				error = %error,
				"Deleted inline file references could not be resolved"
			),
		}
	}
	#[cfg(feature = "file-uploads")]
	cleanup_deleted_file_references(file_references).await;

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
				#[cfg(feature = "file-uploads")]
				let mut deleted_values = Vec::new();
				#[cfg(feature = "file-uploads")]
				let mut inline_deleted_values = Vec::new();
				let mut affected = 0;
				for id in &ids {
					let object_id = canonicalize_pk_value(&table_name, &pk_field, id);
					#[cfg(feature = "file-uploads")]
					let parent_exists = db
						.get_with_executor_for_update(
							transaction,
							&table_name,
							&pk_field,
							&object_id,
						)
						.await?
						.is_some();
					#[cfg(feature = "file-uploads")]
					if !parent_exists {
						continue;
					}
					#[cfg(feature = "file-uploads")]
					let captured_inline_values = capture_inline_file_values(
						db.as_ref(),
						site.as_ref(),
						model_admin.as_ref(),
						&object_id,
						transaction,
					)
					.await?;
					#[cfg(feature = "file-uploads")]
					let (deleted, mut values) = db
						.delete_with_executor_returning(
							transaction,
							&table_name,
							&pk_field,
							&object_id,
						)
						.await?;
					#[cfg(not(feature = "file-uploads"))]
					let deleted = db.delete_with_executor(transaction, &table_name, &pk_field, &object_id)
						.await?;
					#[cfg(feature = "file-uploads")]
					if let Some(record) = values.as_mut()
						&& let Err(error) =
							translate_physical_field_names_to_logical(&table_name, record)
					{
						tracing::warn!(error = %error, "Bulk-deleted file references could not be translated");
						values = None;
					}
					if deleted > 0 {
						#[cfg(feature = "file-uploads")]
						inline_deleted_values.extend(
							confirmed_inline_deleted_values(
								db.as_ref(),
								captured_inline_values,
								transaction,
							)
							.await?,
						);
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
						#[cfg(feature = "file-uploads")]
						if let Some(values) = values {
							deleted_values.push(values);
						}
						affected += deleted;
					}
				}
				#[cfg(feature = "file-uploads")]
				return Ok((affected, deleted_values, inline_deleted_values));
				#[cfg(not(feature = "file-uploads"))]
				Ok(affected)
			})
			.await
	}
	.await;

	let success = result.is_ok();
	#[cfg(feature = "file-uploads")]
	let affected_count = result
		.as_ref()
		.map(|(affected, _, _)| *affected)
		.unwrap_or(0);
	#[cfg(not(feature = "file-uploads"))]
	let affected_count = result.as_ref().copied().unwrap_or(0);
	audit::log_bulk_delete(&audit_user_id, &model_name, &ids, affected_count, success);

	#[cfg(feature = "file-uploads")]
	let (affected, deleted_values, inline_deleted_values) =
		result.map_err(|_| ServerFnError::server(500, "Database operation failed"))?;
	#[cfg(not(feature = "file-uploads"))]
	let affected = result.map_err(|_| ServerFnError::server(500, "Database operation failed"))?;

	#[cfg(feature = "file-uploads")]
	if affected > 0 {
		let mut file_references = Vec::new();
		for values in &deleted_values {
			file_references.extend(deleted_file_references(model_admin.as_ref(), Some(values)));
		}
		for (table_name, values) in inline_deleted_values {
			match site.get_model_admin_by_table_name(&table_name) {
				Ok(child_admin) => file_references
					.extend(deleted_file_references(child_admin.as_ref(), Some(&values))),
				Err(error) => tracing::warn!(
					table = table_name.as_str(),
					error = %error,
					"Bulk-deleted inline file references could not be resolved"
				),
			}
		}
		cleanup_deleted_file_references(file_references).await;
	}

	Ok(BulkDeleteResponse {
		success: affected > 0,
		deleted: affected,
		message: format!("Deleted {} {} items", affected, model_name),
	})
}
