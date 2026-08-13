//! Create operation Server Function
//!
//! Provides create operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use crate::adapters::{AdminDatabase, AdminSite};
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
	created_parent_identity, map_inline_mutation_error, map_inline_transaction_error,
	parse_inline_mutations, preflight_inline_permissions, sanitize_inline_mutations,
	save_inline_mutations,
};
#[cfg(server)]
use super::relation::{
	relation_field_aliases, relation_value, resolve_relations, split_relation_values,
	sync_relation_ids, validate_relation_ids, validate_relation_values,
};
#[cfg(server)]
use super::security::{require_csrf_token, sanitize_mutation_values};
#[cfg(server)]
use super::type_inference::translate_logical_field_names;
#[cfg(server)]
use super::validation::validate_mutation_data_with_aliases;

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
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Add)
		.await?;
	let model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let inlines = model_admin.inlines();
	let mut request = request;
	let mut inline_mutations = if inlines.is_empty() {
		Vec::new()
	} else {
		parse_inline_mutations(&mut request.data, &inlines).map_err(map_inline_mutation_error)?
	};

	// Validate input data before database operation
	let data = request.data;
	let field_aliases = relation_field_aliases(&site, &model_admin).map_server_fn_error()?;
	validate_mutation_data_with_aliases(&data, model_admin.as_ref(), false, &field_aliases)
		.map_server_fn_error()?;
	let descriptors = resolve_relations(&site, model_admin.as_ref()).map_server_fn_error()?;
	let (mut data, selections) = split_relation_values(data, &descriptors).map_server_fn_error()?;
	for selection in &selections {
		auth.require_model_permission(
			selection.descriptor.target_admin.as_ref(),
			user.as_ref(),
			ModelPermission::View,
		)
		.await?;
	}
	let relation_values =
		validate_relation_values(&auth, user.as_ref(), &site, &db, &model_admin, &mut data).await?;

	// Sanitize string values to prevent stored XSS
	let mut sanitized_data = data;
	sanitize_mutation_values(&mut sanitized_data);
	sanitize_inline_mutations(&mut inline_mutations);
	sanitized_data.extend(relation_values);
	let mut audit_data = sanitized_data.clone();
	for selection in &selections {
		audit_data.insert(
			selection.descriptor.field_name.clone(),
			serde_json::Value::Null,
		);
	}

	// Inject current timestamp for auto_now and auto_now_add fields.
	// These fields are typically readonly in the admin form, so the client
	// does not submit values for them. Without this injection the database
	// would raise a NOT NULL violation.
	inject_auto_timestamps(&mut sanitized_data, &table_name);
	translate_logical_field_names(&table_name, &mut sanitized_data).map_server_fn_error()?;

	if !inlines.is_empty() {
		preflight_inline_permissions(
			&auth,
			site.as_ref(),
			user.as_ref(),
			&inlines,
			&inline_mutations,
		)
		.await?;
	}

	let actor = user.get_username().to_string();
	let audit_user_id = auth.user_id().unwrap_or("unknown").to_string();
	let connection = *db.connection();
	let result: Result<_, super::inline::InlineTransactionError> = async {
		connection
			.atomic_write(async |transaction| {
				let created = db
					.create_with_executor(
						transaction,
						&table_name,
						Some(&pk_field),
						sanitized_data.clone(),
					)
					.await?;
				let (object_id, _) = created_parent_identity(&created, &table_name, &pk_field)?;
				for selection in &selections {
					let source_pk = relation_value(
						&selection.descriptor.source_metadata,
						&selection.descriptor.source_pk_field,
						&object_id,
					)
					.map_err(reinhardt_core::exception::Error::from)?;
					validate_relation_ids(transaction, &selection.descriptor, &selection.ids)
						.await
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
				let outcomes =
					save_inline_mutations(&inlines, &object_id, inline_mutations, transaction)
						.await?;
				if created.affected > 0 {
					let event = audit::new_history_event(
						&actor,
						"CREATE",
						&model_name,
						&table_name,
						&object_id,
						sanitized_data.keys().cloned().collect(),
						created.affected,
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
				Ok(created)
			})
			.await
	}
	.await;

	let success = result.is_ok();
	audit::log_create(&audit_user_id, &model_name, &audit_data, success);

	let created = result.map_err(map_inline_transaction_error)?;
	let affected = created.primary_key.as_u64().unwrap_or(created.affected);

	Ok(MutationResponse {
		success: true,
		message: format!("{} created successfully", model_name),
		affected: Some(affected),
		data: None,
	})
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
