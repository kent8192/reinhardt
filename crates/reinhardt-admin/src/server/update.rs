//! Update operation Server Function
//!
//! Provides update operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use crate::adapters::{AdminDatabase, AdminSite};
#[cfg(server)]
use crate::core::database::canonicalize_pk_value;
#[cfg(server)]
use crate::core::history::insert_history_event;
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminQuery, AdminRequestContext, AdminSiteKey};
use crate::types::MutationResponse;
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use std::collections::{HashMap, HashSet};

#[cfg(server)]
use super::audit;
#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::form::prepare_parent_form_data;
#[cfg(server)]
use super::inline::{
	map_inline_mutation_error, map_inline_transaction_error, parse_inline_mutations,
	preflight_inline_permissions, remove_unchanged_inline_mutations,
	sanitize_inline_mutations_with_trusted_fields, save_inline_mutations,
};
#[cfg(server)]
use super::limits::MAX_RELATION_SELECTIONS;
#[cfg(server)]
use super::relation::{
	lock_relation_source, relation_selection_is_unchanged, relation_value, resolve_relations,
	split_relation_values, sync_relation_ids, validate_relation_ids, validate_relation_values,
};
#[cfg(server)]
use super::security::require_csrf_token;
#[cfg(all(server, not(feature = "file-uploads")))]
use super::security::sanitize_mutation_values;
#[cfg(server)]
use super::type_inference::{
	translate_logical_field_names, translate_physical_field_names_to_logical,
};

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
	#[cfg(feature = "file-uploads")]
	let cleanup_site = site.as_ref().clone();
	let (response, _, outcomes) = update_record_with_previous_values(
		model_name,
		id,
		request,
		site,
		db,
		http_request,
		AdminAuthenticatedUser(user),
	)
	.await?;
	#[cfg(feature = "file-uploads")]
	super::multipart::schedule_inline_delete_cleanups(cleanup_site, outcomes).await;
	#[cfg(not(feature = "file-uploads"))]
	let _ = outcomes;
	Ok(response)
}

#[cfg(server)]
pub(crate) async fn update_record_with_previous_values(
	model_name: String,
	id: String,
	request: crate::types::MutationRequest,
	site: KeyedDepends<AdminSiteKey, AdminSite>,
	db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	http_request: ServerFnRequest,
	user: AdminAuthenticatedUser,
) -> Result<
	(
		crate::types::MutationResponse,
		HashMap<String, serde_json::Value>,
		Vec<super::inline::InlineSaveOutcome>,
	),
	ServerFnError,
> {
	update_record_with_trusted_file_fields(
		model_name,
		id,
		request,
		site,
		db,
		http_request,
		user,
		&HashSet::new(),
	)
	.await
}

#[cfg(server)]
// The server-function dependencies remain separate to preserve the existing internal call contract.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn update_record_with_trusted_file_fields(
	model_name: String,
	id: String,
	request: crate::types::MutationRequest,
	site: KeyedDepends<AdminSiteKey, AdminSite>,
	db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	http_request: ServerFnRequest,
	AdminAuthenticatedUser(user): AdminAuthenticatedUser,
	trusted_file_fields: &HashSet<String>,
) -> Result<
	(
		crate::types::MutationResponse,
		HashMap<String, serde_json::Value>,
		Vec<super::inline::InlineSaveOutcome>,
	),
	ServerFnError,
> {
	// CSRF token validation (double-submit cookie pattern)
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Change)
		.await?;
	#[cfg(feature = "file-uploads")]
	super::multipart::reject_file_field_json_data_with_trusted_fields(
		&request.data,
		model_admin.as_ref(),
		site.as_ref(),
		trusted_file_fields,
	)?;

	let model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let object_id = canonicalize_pk_value(&table_name, &pk_field, &id);
	let request_context = AdminRequestContext::new(http_request.into_inner());
	let admin_query = model_admin
		.get_queryset(
			user.as_ref(),
			&request_context,
			AdminQuery::new(table_name.as_str()),
		)
		.await
		.map_server_fn_error()?;
	let inlines = model_admin.inlines();
	let mut request = request;
	let mut inline_mutations = if inlines.is_empty() {
		Vec::new()
	} else {
		parse_inline_mutations(&mut request.data, &inlines).map_err(map_inline_mutation_error)?
	};

	// Normalize and validate parent data before relation processing.
	let data = prepare_parent_form_data(
		&site,
		model_admin.as_ref(),
		crate::core::AdminFormMode::Update,
		request.data,
	)?;
	let descriptors = resolve_relations(&site, model_admin.as_ref()).map_server_fn_error()?;
	let (mut data, selections) = split_relation_values(data, &descriptors).map_server_fn_error()?;
	let mut relation_queries = Vec::with_capacity(selections.len());
	for selection in &selections {
		auth.require_model_permission(
			selection.descriptor.target_admin.as_ref(),
			user.as_ref(),
			ModelPermission::View,
		)
		.await?;
		relation_queries.push(
			selection
				.descriptor
				.target_admin
				.get_queryset(
					user.as_ref(),
					&request_context,
					AdminQuery::new(selection.descriptor.target_admin.table_name()),
				)
				.await
				.map_server_fn_error()?,
		);
	}
	let relation_values = validate_relation_values(
		&auth,
		user.as_ref(),
		&request_context,
		&site,
		&db,
		&model_admin,
		&mut data,
	)
	.await?;

	// Sanitize string values to prevent stored XSS
	let mut sanitized_data = data;
	#[cfg(feature = "file-uploads")]
	super::security::sanitize_mutation_values_with_trusted_fields(
		&mut sanitized_data,
		trusted_file_fields,
	);
	#[cfg(not(feature = "file-uploads"))]
	sanitize_mutation_values(&mut sanitized_data);
	sanitize_inline_mutations_with_trusted_fields(&mut inline_mutations, trusted_file_fields);
	sanitized_data.extend(relation_values);
	let mut audit_data = sanitized_data.clone();
	for selection in &selections {
		audit_data.insert(
			selection.descriptor.field_name.clone(),
			serde_json::Value::Null,
		);
	}

	// Inject current timestamp for auto_now fields (updated on every save)
	super::create::inject_auto_now_timestamps(&mut sanitized_data, &table_name);
	translate_logical_field_names(&table_name, &mut sanitized_data).map_server_fn_error()?;

	let actor = user.get_username().to_string();
	let audit_user_id = auth.user_id().unwrap_or("unknown").to_string();
	let mut connection = *db.connection();

	let inline_scopes = if inlines.is_empty() {
		HashMap::new()
	} else {
		let mut unchanged_scope_queries = HashMap::new();
		for inline in &inlines {
			let child_admin = site
				.get_model_admin_by_table_name(inline.adapter().table_name())
				.map_server_fn_error()?;
			inline
				.validate_child_table(child_admin.table_name())
				.map_server_fn_error()?;
			auth.require_model_permission(
				child_admin.as_ref(),
				user.as_ref(),
				ModelPermission::View,
			)
			.await?;
			let query = child_admin
				.get_queryset(
					user.as_ref(),
					&request_context,
					AdminQuery::new(child_admin.table_name()),
				)
				.await
				.map_server_fn_error()?;
			unchanged_scope_queries.insert(inline.key().to_owned(), query);
		}
		remove_unchanged_inline_mutations(
			&inlines,
			&object_id,
			&mut inline_mutations,
			&unchanged_scope_queries,
			&mut connection,
		)
		.await
		.map_err(map_inline_mutation_error)?;
		preflight_inline_permissions(
			&auth,
			site.as_ref(),
			user.as_ref(),
			&request_context,
			&inlines,
			&inline_mutations,
		)
		.await?
	};

	let result: Result<_, super::inline::InlineTransactionError> = async {
		connection
			.atomic_write(async |transaction| {
				let current_data = db
					.get_admin_query_with_executor_for_update(
						transaction,
						&admin_query,
						&pk_field,
						&object_id,
					)
					.await?;
				let Some(current_data) = current_data else {
					return Err(crate::types::AdminError::ModelNotRegistered(format!(
						"{} not found",
						model_name
					))
					.into());
				};
				if let Some(selection) = selections.first() {
					lock_relation_source(transaction, &selection.descriptor, &object_id)
						.await
						.map_err(reinhardt_core::exception::Error::from)?;
				}
				let mut relation_changed_fields = Vec::new();
				for (selection, relation_query) in selections.iter().zip(&relation_queries) {
					let source_pk = relation_value(
						&selection.descriptor.source_metadata,
						&selection.descriptor.source_pk_field,
						&object_id,
					)
					.map_err(reinhardt_core::exception::Error::from)?;
					validate_relation_ids(
						transaction,
						&selection.descriptor,
						&selection.ids,
						relation_query,
					)
					.await
					.map_err(reinhardt_core::exception::Error::from)?;
					if selection.ids.len() > MAX_RELATION_SELECTIONS {
						let unchanged = relation_selection_is_unchanged(
							transaction,
							&selection.descriptor,
							&source_pk,
							&selection.ids,
						)
						.await
						.map_err(reinhardt_core::exception::Error::from)?;
						if !unchanged {
							return Err(crate::types::AdminError::ValidationError(format!(
								"Field '{}' relation selection too large: {} elements (max {})",
								selection.descriptor.field_name,
								selection.ids.len(),
								MAX_RELATION_SELECTIONS
							))
							.into());
						}
						continue;
					}
					if sync_relation_ids(
						transaction,
						&selection.descriptor,
						source_pk,
						&selection.ids,
					)
					.await
					.map_err(reinhardt_core::exception::Error::from)?
					{
						relation_changed_fields.push(selection.descriptor.field_name.clone());
					}
				}
				let mut changed_fields = sanitized_data
					.iter()
					.filter(|&(field, value)| current_data.get(field) != Some(value))
					.map(|(field, _value)| field.clone())
					.collect::<Vec<_>>();
				changed_fields.extend(relation_changed_fields);
				changed_fields.sort_unstable();
				changed_fields.dedup();
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
				let outcomes = save_inline_mutations(
					&db,
					&inlines,
					&inline_scopes,
					&object_id,
					inline_mutations,
					transaction,
				)
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
				Ok((affected, current_data, outcomes))
			})
			.await
	}
	.await;

	// Check for database errors first, logging failure before returning
	let (affected, mut previous_data, outcomes) = match result {
		Err(error) => {
			audit::log_update(&audit_user_id, &model_name, &id, &audit_data, false);
			return Err(map_inline_transaction_error(error));
		}
		Ok((affected, previous_data, outcomes)) => (affected, previous_data, outcomes),
	};
	audit::log_update(&audit_user_id, &model_name, &id, &audit_data, true);
	audit::log_inline_outcomes(site.as_ref(), &audit_user_id, &outcomes);

	translate_physical_field_names_to_logical(&table_name, &mut previous_data)
		.map_server_fn_error()?;
	let response = MutationResponse {
		success: true,
		message: format!("{} updated successfully", model_name),
		affected: Some(affected),
		data: None,
	};
	Ok((response, previous_data, outcomes))
}
