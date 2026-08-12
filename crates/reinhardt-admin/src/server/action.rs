//! Registered admin action Server Function.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use crate::adapters::{AdminDatabase, AdminSite, ModelAdmin};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey};
#[cfg(server)]
use crate::types::{AdminAction, AdminError, AdminResult};
use crate::types::{AdminActionRequest, MutationResponse};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::audit;
#[cfg(server)]
use super::error::{AdminAuth, IntoServerFnError};
#[cfg(server)]
use super::limits::MAX_BULK_DELETE_IDS;
#[cfg(server)]
use super::security::require_csrf_token;
#[cfg(server)]
use super::type_inference::{canonicalize_primary_key_ids, get_field_metadata};
#[cfg(server)]
use std::collections::HashSet;

#[cfg(server)]
pub(crate) fn registered_actions(model_admin: &dyn ModelAdmin) -> AdminResult<Vec<AdminAction>> {
	let actions = model_admin.actions();
	let mut names = HashSet::new();
	for action in &actions {
		if action.name.is_empty() || !names.insert(action.name.as_str()) {
			return Err(AdminError::ValidationError(
				"Admin action names must be nonempty and unique".to_string(),
			));
		}
	}
	Ok(actions)
}

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
		return Err(ServerFnError::server(400, "Action is required"));
	}
	let action = match registered_actions(model_admin.as_ref())
		.map_err(|error| error.into_server_fn_error())?
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
			return Err(ServerFnError::server(400, "Unknown admin action"));
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
		return Err(ServerFnError::server(400, "Select at least one record"));
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
		return Err(ServerFnError::server(400, "Too many records selected"));
	}
	let primary_key_type = get_field_metadata(model_admin.table_name(), model_admin.pk_field())
		.map(|metadata| metadata.field_type)
		.unwrap_or(reinhardt_db::migrations::FieldType::Text);
	let canonical_ids = match canonicalize_primary_key_ids(&primary_key_type, &request.ids) {
		Ok(ids) => ids,
		Err(error) => {
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
	};
	if canonical_ids.iter().collect::<HashSet<_>>().len() != canonical_ids.len() {
		audit::log_action(
			user_id,
			model_admin.model_name(),
			&request.ids,
			&action.name,
			0,
			false,
		);
		return Err(ServerFnError::server(
			400,
			"Duplicate record IDs are not allowed",
		));
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

	let result = db
		.connection()
		.atomic_write(async |transaction| {
			model_admin
				.execute_action(&action.name, &canonical_ids, transaction, user.as_ref())
				.await
		})
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
