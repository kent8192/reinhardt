//! Update operation Server Function
//!
//! Provides update operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminSite};
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
use super::inline::{
	map_inline_mutation_error, map_inline_transaction_error, parse_inline_mutations,
	preflight_inline_permissions, remove_unchanged_inline_mutations, sanitize_inline_mutations,
	save_inline_mutations,
};
#[cfg(server)]
use super::relation::{relation_field_aliases, validate_relation_values};
#[cfg(server)]
use super::security::{require_csrf_token, sanitize_mutation_values};
#[cfg(server)]
use super::type_inference::translate_logical_field_names;
#[cfg(server)]
use super::validation::validate_mutation_data_with_aliases;

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

	let model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let object_id = canonicalize_pk_value(&table_name, &pk_field, &id);
	let inlines = model_admin.inlines();
	let mut request = request;
	let mut inline_mutations = if inlines.is_empty() {
		Vec::new()
	} else {
		parse_inline_mutations(&mut request.data, &inlines).map_err(map_inline_mutation_error)?
	};

	// Validate input data before database operation
	let mut data = request.data;
	let field_aliases = relation_field_aliases(&site, &model_admin).map_server_fn_error()?;
	validate_mutation_data_with_aliases(&data, model_admin.as_ref(), true, &field_aliases)
		.map_server_fn_error()?;
	let relation_values =
		validate_relation_values(&auth, user.as_ref(), &site, &db, &model_admin, &mut data).await?;

	// Sanitize string values to prevent stored XSS
	let mut sanitized_data = data;
	sanitize_mutation_values(&mut sanitized_data);
	sanitize_inline_mutations(&mut inline_mutations);
	sanitized_data.extend(relation_values);

	// Inject current timestamp for auto_now fields (updated on every save)
	super::create::inject_auto_now_timestamps(&mut sanitized_data, &table_name);
	translate_logical_field_names(&table_name, &mut sanitized_data).map_server_fn_error()?;

	let actor = user.get_username().to_string();
	let audit_user_id = auth.user_id().unwrap_or("unknown").to_string();
	let mut connection = *db.connection();

	if !inlines.is_empty() {
		remove_unchanged_inline_mutations(
			&inlines,
			&object_id,
			&mut inline_mutations,
			&mut connection,
		)
		.await
		.map_err(map_inline_mutation_error)?;
		preflight_inline_permissions(
			&auth,
			site.as_ref(),
			user.as_ref(),
			&inlines,
			&inline_mutations,
		)
		.await?;
	}

	let result: Result<_, super::inline::InlineTransactionError> = async {
		connection
			.atomic_write(async |transaction| {
				let current_data = db
					.get_with_executor(transaction, &table_name, &pk_field, &object_id)
					.await?;
				let Some(current_data) = current_data else {
					return Err(crate::types::AdminError::ModelNotRegistered(format!(
						"{} not found",
						model_name
					))
					.into());
				};
				let mut changed_fields = sanitized_data
					.iter()
					.filter_map(|(field, value)| {
						(current_data.get(field) != Some(value)).then(|| field.clone())
					})
					.collect::<Vec<_>>();
				changed_fields.sort_unstable();
				let affected = if sanitized_data.is_empty() {
					0
				} else {
					db.update_with_executor(
						transaction,
						&table_name,
						&pk_field,
						&object_id,
						sanitized_data.clone(),
					)
					.await?
				};
				let outcomes =
					save_inline_mutations(&inlines, &object_id, inline_mutations, transaction)
						.await?;
				if !changed_fields.is_empty() {
					let event = audit::new_history_event(
						&actor,
						"UPDATE",
						&model_name,
						&table_name,
						&object_id,
						changed_fields.clone(),
						affected,
					);
					insert_history_event(transaction, &event).await?;
				}
				super::inline::insert_inline_history_events(
					site.as_ref(),
					&actor,
					&outcomes,
					transaction,
				)
				.await?;
				Ok((affected, outcomes))
			})
			.await
	}
	.await;

	// Check for database errors first, logging failure before returning
	let (affected, outcomes) = match result {
		Err(error) => {
			audit::log_update(&audit_user_id, &model_name, &id, &sanitized_data, false);
			return Err(map_inline_transaction_error(error));
		}
		Ok(n) => n,
	};

	audit::log_update(&audit_user_id, &model_name, &id, &sanitized_data, true);
	audit::log_inline_outcomes(site.as_ref(), &audit_user_id, &outcomes);

	Ok(MutationResponse {
		success: true,
		message: format!("{} updated successfully", model_name),
		affected: Some(affected),
		data: None,
	})
}
