//! Create operation Server Function
//!
//! Provides create operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey, ModelAdmin};
#[cfg(server)]
use crate::types::AdminResult;
use crate::types::MutationResponse;
#[cfg(server)]
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
	request: crate::types::MutationRequest,
	#[inject] site: Depends<AdminSiteKey, AdminSite>,
	#[inject] db: Depends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<crate::types::MutationResponse, ServerFnError> {
	// CSRF token validation (double-submit cookie pattern)
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Add)
		.await?;
	let table_name = model_admin.table_name();
	let pk_field = model_admin.pk_field();

	let sanitized_data = prepare_create_data(request.data, model_admin.as_ref(), table_name)
		.map_server_fn_error()?;

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

#[cfg(server)]
pub(crate) fn prepare_create_data(
	mut data: std::collections::HashMap<String, serde_json::Value>,
	model_admin: &dyn ModelAdmin,
	table_name: &str,
) -> AdminResult<std::collections::HashMap<String, serde_json::Value>> {
	validate_mutation_data(&data, model_admin, false)?;
	sanitize_mutation_values(&mut data);
	inject_auto_timestamps(&mut data, table_name);
	Ok(data)
}

/// Injects the current UTC timestamp for fields with `auto_now` or `auto_now_add`.
///
/// This mirrors Django's behavior: `auto_now_add` sets the timestamp on creation,
/// and `auto_now` sets it on every save (both apply during creation). Any existing
/// value for these fields is overwritten — they are always server-controlled.
///
/// For updates, call [`inject_auto_now_timestamps`] instead, which only handles
/// `auto_now` fields.
#[cfg(server)]
pub(crate) fn inject_auto_timestamps(
	data: &mut std::collections::HashMap<String, serde_json::Value>,
	table_name: &str,
) {
	use crate::server::type_inference::find_model_by_table_name;

	let Some(model) = find_model_by_table_name(table_name) else {
		return;
	};

	let now = chrono::Utc::now();

	for (field_name, meta) in &model.fields {
		let is_auto_now = meta
			.params
			.get("auto_now")
			.is_some_and(|v| v == "true" || v == "True");
		let is_auto_now_add = meta
			.params
			.get("auto_now_add")
			.is_some_and(|v| v == "true" || v == "True");

		if is_auto_now || is_auto_now_add {
			// Format based on field type: Date, Time, or DateTime
			let value = match &meta.field_type {
				reinhardt_db::migrations::FieldType::Date => {
					serde_json::Value::String(now.format("%Y-%m-%d").to_string())
				}
				reinhardt_db::migrations::FieldType::Time => {
					serde_json::Value::String(now.format("%H:%M:%S").to_string())
				}
				_ => {
					// DateTime and other types: ISO 8601 format
					serde_json::Value::String(now.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
				}
			};
			data.insert(field_name.clone(), value);
		}
	}
}

/// Injects the current UTC timestamp for fields with `auto_now` only.
///
/// Used during updates — `auto_now_add` fields are not touched because they
/// should only be set on initial creation.
#[cfg(server)]
pub(crate) fn inject_auto_now_timestamps(
	data: &mut std::collections::HashMap<String, serde_json::Value>,
	table_name: &str,
) {
	use crate::server::type_inference::find_model_by_table_name;

	let Some(model) = find_model_by_table_name(table_name) else {
		return;
	};

	let now = chrono::Utc::now();

	for (field_name, meta) in &model.fields {
		let is_auto_now = meta
			.params
			.get("auto_now")
			.is_some_and(|v| v == "true" || v == "True");

		if is_auto_now {
			let value = match &meta.field_type {
				reinhardt_db::migrations::FieldType::Date => {
					serde_json::Value::String(now.format("%Y-%m-%d").to_string())
				}
				reinhardt_db::migrations::FieldType::Time => {
					serde_json::Value::String(now.format("%H:%M:%S").to_string())
				}
				_ => serde_json::Value::String(now.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()),
			};
			data.insert(field_name.clone(), value);
		}
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::core::ModelAdminConfig;

	#[test]
	fn prepare_create_data_rejects_readonly_import_fields() {
		let admin = ModelAdminConfig::builder()
			.model_name("Record")
			.list_display(vec!["id", "name"])
			.fields(vec!["id", "name", "owner_id"])
			.readonly_fields(vec!["owner_id"])
			.build()
			.expect("test admin should build");
		let data = std::collections::HashMap::from([
			("name".to_string(), serde_json::json!("safe")),
			("owner_id".to_string(), serde_json::json!(42)),
		]);

		let result = prepare_create_data(data, &admin, "records");

		assert!(matches!(
			result,
			Err(crate::types::AdminError::ValidationError(message))
				if message == "Field 'owner_id' is read-only and cannot be modified"
		));
	}
}
