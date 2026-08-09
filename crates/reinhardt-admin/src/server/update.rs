//! Update operation Server Function
//!
//! Provides update operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminSite};
#[cfg(server)]
use crate::core::database::update_with_executor;
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
use super::relation::{
	relation_value, resolve_relations, split_relation_values, sync_relation_ids,
	validate_relation_ids,
};
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
	request: crate::types::MutationRequest,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<crate::types::MutationResponse, ServerFnError> {
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

	let descriptors = resolve_relations(&site, model_admin.as_ref()).map_server_fn_error()?;
	let (mut scalar_data, selections) =
		split_relation_values(request.data, &descriptors).map_server_fn_error()?;
	for selection in &selections {
		auth.require_model_permission(
			selection.descriptor.target_admin.as_ref(),
			user.as_ref(),
			ModelPermission::View,
		)
		.await?;
	}

	// Sanitize string values to prevent stored XSS
	sanitize_mutation_values(&mut scalar_data);

	// Inject current timestamp for auto_now fields (updated on every save)
	super::create::inject_auto_now_timestamps(&mut scalar_data, table_name);

	let user_id = auth.user_id().unwrap_or("unknown").to_string();
	let audit_data = super::create::audit_changed_fields(
		&scalar_data,
		selections
			.iter()
			.map(|selection| selection.descriptor.field_name.as_str()),
	);
	let result: Result<u64, reinhardt_core::exception::Error> = db
		.connection()
		.atomic_write(async |transaction| {
			let affected =
				update_with_executor(table_name, pk_field, &id, scalar_data.clone(), transaction)
					.await
					.map_err(reinhardt_core::exception::Error::from)?;
			if affected == 0 {
				return Err(reinhardt_core::exception::Error::NotFound(format!(
					"{} not found",
					model_name
				)));
			}
			for selection in &selections {
				validate_relation_ids(transaction, &selection.descriptor, &selection.ids)
					.await
					.map_err(reinhardt_core::exception::Error::from)?;
				let source_pk = relation_value(
					&selection.descriptor.source_metadata,
					&selection.descriptor.source_pk_field,
					&id,
				)
				.map_err(reinhardt_core::exception::Error::from)?;
				sync_relation_ids(
					transaction,
					&selection.descriptor,
					source_pk,
					&selection.ids,
				)
				.await
				.map_err(reinhardt_core::exception::Error::from)?;
			}
			Ok(affected)
		})
		.await;

	let success = result.is_ok();
	audit::log_update(&user_id, &model_name, &id, &audit_data, success);
	let affected = result.map_err(super::create::atomic_server_error)?;

	Ok(MutationResponse {
		success: true,
		message: format!("{} updated successfully", model_name),
		affected: Some(affected),
		data: None,
	})
}
