//! Registered admin action Server Function.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use crate::adapters::{AdminDatabase, AdminSite};
#[cfg(server)]
use crate::core::history::insert_history_event;
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey, canonicalize_admin_primary_key};
#[cfg(server)]
use crate::types::AdminError;
use crate::types::{AdminActionRequest, MutationResponse};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use std::collections::HashSet;

#[cfg(server)]
use super::audit;
#[cfg(server)]
use super::error::{AdminAuth, IntoServerFnError};
#[cfg(server)]
use super::limits::MAX_BULK_DELETE_IDS;
#[cfg(server)]
use super::security::require_csrf_token;
#[cfg(server)]
use super::type_inference::{get_field_metadata, validate_primary_key_ids};

/// Executes a registered action for the selected model records.
#[server_fn]
pub async fn execute_admin_action(
	model_name: String,
	request: AdminActionRequest,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<MutationResponse, ServerFnError> {
	require_csrf_token(&request.csrf_token, &http_request.inner().headers)?;

	let auth = AdminAuth::from_request(&http_request);
	let user_id = auth.user_id().unwrap_or("unknown");
	let model_admin = match site.get_model_admin(&model_name) {
		Ok(model_admin) => model_admin,
		Err(error) => {
			audit::log_action(
				user_id,
				&model_name,
				&request.ids,
				&request.action,
				0,
				false,
			);
			return Err(error.into_server_fn_error());
		}
	};
	if request.action.is_empty() {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&request.action,
			0,
			false,
		);
		return Err(ServerFnError::application("Action is required"));
	}
	let action = match model_admin
		.actions()
		.into_iter()
		.find(|action| action.name == request.action)
	{
		Some(action) => action,
		None => {
			audit::log_action(
				user_id,
				model_admin.model_name(),
				&request.ids,
				&request.action,
				0,
				false,
			);
			return Err(ServerFnError::application("Unknown admin action"));
		}
	};

	if request.ids.is_empty() {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(ServerFnError::application("Select at least one record"));
	}
	if request.ids.iter().collect::<HashSet<_>>().len() != request.ids.len() {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&request.action,
			0,
			false,
		);
		return Err(ServerFnError::application(
			"Duplicate record IDs are not allowed",
		));
	}
	if request.ids.len() > MAX_BULK_DELETE_IDS {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(ServerFnError::application("Too many records selected"));
	}

	let primary_key_type = get_field_metadata(model_admin.table_name(), model_admin.pk_field())
		.map(|metadata| metadata.field_type)
		.unwrap_or(reinhardt_db::migrations::FieldType::Text);
	if let Err(error) = validate_primary_key_ids(&primary_key_type, &request.ids) {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(error.into_server_fn_error());
	}

	if let Err(error) = auth
		.require_model_permission(model_admin.as_ref(), user.as_ref(), action.permission)
		.await
	{
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(error);
	}

	let canonical_model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let actor = user.get_username().to_string();
	let connection = *db.connection();
	let result: Result<_, AdminError> = async {
		connection
			.atomic_write(async |transaction| {
				let outcome = model_admin
					.execute_action(&action.name, &request.ids, transaction, user.as_ref())
					.await?;
				let mut successful_objects = HashSet::new();
				for successful_id in &outcome.successful_ids {
					let (object_id, _) =
						canonicalize_admin_primary_key(&table_name, &pk_field, successful_id)?;
					if !successful_objects.insert(object_id.clone()) {
						return Err(AdminError::ValidationError(format!(
							"Action returned duplicate successful ID '{object_id}'"
						)));
					}
					let event = audit::new_history_event(
						&actor,
						&action.name,
						&canonical_model_name,
						&table_name,
						&object_id,
						Vec::new(),
						1,
					);
					insert_history_event(transaction, &event).await?;
				}
				Ok(outcome)
			})
			.await
	}
	.await;

	match result {
		Ok(outcome) => {
			audit::log_action(
				user_id,
				model_admin.model_name(),
				&outcome.successful_ids,
				&action.name,
				outcome.affected,
				true,
			);
			Ok(MutationResponse {
				success: true,
				message: "Action completed successfully".to_string(),
				affected: Some(outcome.affected),
				data: None,
			})
		}
		Err(error) => {
			audit::log_action(
				user_id,
				model_admin.model_name(),
				&request.ids,
				&action.name,
				0,
				false,
			);
			Err(error.into_server_fn_error())
		}
	}
}
