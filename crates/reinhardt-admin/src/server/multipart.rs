//! Dynamic multipart payloads used by admin model forms.

#[cfg(all(server, feature = "file-uploads"))]
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
#[cfg(all(server, feature = "file-uploads"))]
use std::convert::Infallible;
#[cfg(all(server, feature = "file-uploads"))]
use std::sync::Arc;

use reinhardt_core::parsers::UploadedFile;
#[cfg(server)]
use reinhardt_core::parsers::multipart::MultipartPart;
#[cfg(all(server, feature = "file-uploads"))]
use reinhardt_pages::server_fn::server_fn;
#[cfg(server)]
use reinhardt_pages::server_fn::{MultipartArguments, ServerFnError};

#[cfg(all(server, feature = "file-uploads"))]
use super::{
	admin_auth::AdminAuthenticatedUser,
	create::create_record_with_inline_outcomes,
	error::{AdminAuth, MapServerFnError},
	security::{extract_csrf_header, require_csrf_token},
	type_inference::{find_model_by_table_name, infer_required},
	update::update_record_with_previous_values,
};
#[cfg(all(server, feature = "file-uploads"))]
use crate::{
	adapters::{AdminDatabase, AdminSite, ModelAdmin},
	core::{AdminDatabaseKey, AdminSiteKey},
	types::{ModelPermission, MutationRequest, MutationResponse},
};
#[cfg(all(server, feature = "file-uploads"))]
use reinhardt_db::orm::{
	FileCleanupOperation, FileCommit, FileField, FileFieldPolicy, FileMutationError,
	FileValidationPolicy, FileWriteOperation, PendingFileUpload, coordinate_file_mutations,
};
#[cfg(all(server, feature = "file-uploads"))]
use reinhardt_di::KeyedDepends;
#[cfg(all(server, feature = "file-uploads"))]
use reinhardt_pages::server_fn::ServerFnRequest;

/// Reserved multipart part containing the registered model name.
pub(crate) const MODEL_PART: &str = "__reinhardt_model";
/// Reserved multipart part containing the edited record ID.
pub(crate) const ID_PART: &str = "__reinhardt_id";
/// Prefix for optional file-clear controls.
pub(crate) const CLEAR_PREFIX: &str = "__reinhardt_clear.";
/// Prefix for inline model-form controls that continue into inline parsing.
pub(crate) const INLINE_PREFIX: &str = "__reinhardt_inlines.";

/// Parsed dynamic admin form data.
#[derive(Debug)]
pub(crate) struct AdminMultipartPayload {
	/// Registered model name supplied by the form client.
	pub model_name: String,
	/// Edited record ID, or `None` for create requests.
	pub id: Option<String>,
	/// JSON scalar values keyed by logical field name.
	pub data: HashMap<String, serde_json::Value>,
	/// Uploaded files keyed by logical field name.
	pub uploads: HashMap<String, UploadedFile>,
	/// Empty browser file inputs keyed by logical field name.
	pub empty_uploads: HashSet<String>,
	/// Nullable file fields explicitly marked for clearing.
	pub clears: HashSet<String>,
}

/// Parse a dynamic admin multipart request while rejecting unconsumed input.
#[cfg(server)]
pub(crate) async fn parse_admin_multipart(
	request: &reinhardt_http::Request,
	update: bool,
) -> Result<AdminMultipartPayload, ServerFnError> {
	let mut arguments = MultipartArguments::from_request(request).await?;
	let mut model_name = None;
	let mut id = None;
	let mut data = HashMap::new();
	let mut uploads = HashMap::new();
	let mut empty_uploads = HashSet::new();
	let mut clears = HashSet::new();

	for part in arguments.take_parts() {
		match part {
			MultipartPart::Field { name, data: bytes } => {
				if bytes.is_empty() {
					return Err(invalid_request("empty JSON field"));
				}
				let value: serde_json::Value = serde_json::from_slice(&bytes)
					.map_err(|_| invalid_request("malformed JSON field"))?;
				if name == MODEL_PART {
					if model_name.is_some() {
						return Err(invalid_request("duplicate model part"));
					}
					model_name = Some(required_string(value, MODEL_PART)?);
				} else if name == ID_PART {
					if !update || id.is_some() {
						return Err(invalid_request("unexpected record ID part"));
					}
					id = Some(required_string(value, ID_PART)?);
				} else if let Some(field_name) = name.strip_prefix(CLEAR_PREFIX) {
					if field_name.is_empty() {
						return Err(invalid_request("empty clear field name"));
					}
					match value {
						serde_json::Value::Bool(true) => {
							clears.insert(field_name.to_owned());
						}
						serde_json::Value::Bool(false) => {}
						_ => return Err(invalid_request("clear marker must be boolean")),
					}
				} else if name.starts_with(INLINE_PREFIX) {
					if data.insert(name, value).is_some() {
						return Err(invalid_request("duplicate form field"));
					}
				} else if name.starts_with("__reinhardt_") {
					return Err(invalid_request("reserved multipart field name"));
				} else if data.insert(name, value).is_some() {
					return Err(invalid_request("duplicate form field"));
				}
			}
			MultipartPart::File(file) => {
				if file.name.is_empty()
					|| (file.name.starts_with("__reinhardt_")
						&& !file.name.starts_with(INLINE_PREFIX))
				{
					return Err(invalid_request("invalid uploaded file field name"));
				}
				if is_empty_file_input(&file) {
					empty_uploads.insert(file.name.clone());
					continue;
				}
				if uploads.insert(file.name.clone(), file).is_some() {
					return Err(invalid_request("duplicate uploaded file"));
				}
			}
		}
	}
	arguments.finish()?;

	let model_name = model_name.ok_or_else(|| invalid_request("missing model part"))?;
	if update && id.is_none() {
		return Err(invalid_request("missing record ID part"));
	}

	Ok(AdminMultipartPayload {
		model_name,
		id,
		data,
		uploads,
		empty_uploads,
		clears,
	})
}

#[cfg(server)]
fn required_string(value: serde_json::Value, field: &str) -> Result<String, ServerFnError> {
	match value {
		serde_json::Value::String(value) if !value.trim().is_empty() => Ok(value),
		_ => Err(invalid_request(match field {
			MODEL_PART => "model part must be a non-empty string",
			ID_PART => "record ID part must be a non-empty string",
			_ => "reserved part must be a non-empty string",
		})),
	}
}

#[cfg(server)]
fn invalid_request(message: &'static str) -> ServerFnError {
	tracing::warn!(message, "Rejected dynamic admin multipart request");
	ServerFnError::server(400, "Invalid admin multipart request")
}

fn is_empty_file_input(file: &UploadedFile) -> bool {
	file.size == 0 && file.filename.as_deref().is_none_or(str::is_empty)
}

#[cfg(all(server, feature = "file-uploads"))]
#[derive(Clone, Debug)]
struct AdminFileField {
	logical_name: String,
	aliases: Vec<String>,
	required: bool,
	nullable: bool,
	policy: FileFieldPolicy,
}

#[cfg(all(server, feature = "file-uploads"))]
#[derive(Debug)]
struct FileCleanupReference {
	policy: FileFieldPolicy,
	field_name: String,
	operation: FileCleanupOperation,
}

#[cfg(all(server, feature = "file-uploads"))]
struct PreparedFileMutation {
	data: HashMap<String, serde_json::Value>,
	writes: Vec<PendingFileUpload>,
	write_fields: Vec<String>,
	inline_write_targets: Vec<Option<InlineFileTarget>>,
	cleanup: Vec<FileCleanupReference>,
}

#[cfg(all(server, feature = "file-uploads"))]
#[derive(Clone, Debug)]
struct InlineFileTarget {
	path: String,
	inline_key: String,
	submitted_index: usize,
	field_name: String,
	policy: FileFieldPolicy,
}

#[cfg(all(server, feature = "file-uploads"))]
struct FileMutationContext {
	site: KeyedDepends<AdminSiteKey, AdminSite>,
	db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	http_request: ServerFnRequest,
	user: Arc<dyn crate::core::AdminUser>,
	model_admin: Arc<dyn ModelAdmin>,
}

#[cfg(all(server, feature = "file-uploads"))]
fn validation_error(field: &str, message: &str) -> ServerFnError {
	ServerFnError::validation_with_message(
		"Invalid file upload",
		[(field.to_owned(), message.to_owned())],
	)
}

#[cfg(all(server, feature = "file-uploads"))]
fn metadata_error(field: &str, detail: impl std::fmt::Display) -> ServerFnError {
	tracing::error!(field, error = %detail, "Invalid storage-backed file field metadata");
	ServerFnError::server(500, "Invalid file field metadata")
}

#[cfg(all(server, feature = "file-uploads"))]
fn parse_metadata_usize(
	params: &HashMap<String, String>,
	field: &str,
	key: &str,
	default: usize,
) -> Result<usize, ServerFnError> {
	params.get(key).map_or(Ok(default), |value| {
		value
			.parse::<usize>()
			.ok()
			.filter(|parsed| *parsed > 0)
			.ok_or_else(|| metadata_error(field, format!("{key} must be a positive integer")))
	})
}

#[cfg(all(server, feature = "file-uploads"))]
fn parse_metadata_u32(
	params: &HashMap<String, String>,
	field: &str,
	key: &str,
) -> Result<Option<u32>, ServerFnError> {
	params.get(key).map_or(Ok(None), |value| {
		value
			.parse::<u32>()
			.ok()
			.filter(|parsed| *parsed > 0)
			.map(Some)
			.ok_or_else(|| metadata_error(field, format!("{key} must be a positive integer")))
	})
}

#[cfg(all(server, feature = "file-uploads"))]
fn parse_metadata_bool(
	params: &HashMap<String, String>,
	field: &str,
	key: &str,
	default: bool,
) -> Result<bool, ServerFnError> {
	params.get(key).map_or(Ok(default), |value| {
		value
			.parse::<bool>()
			.map_err(|_| metadata_error(field, format!("{key} must be a boolean")))
	})
}

#[cfg(all(server, feature = "file-uploads"))]
fn file_field_policy(
	model_name: &str,
	logical_name: &str,
	metadata: &reinhardt_db::migrations::FieldMetadata,
) -> Result<FileFieldPolicy, ServerFnError> {
	let kind = metadata
		.params
		.get("model_field_type")
		.map(String::as_str)
		.ok_or_else(|| metadata_error(logical_name, "missing model_field_type"))?;
	let upload_to = metadata
		.params
		.get("upload_to")
		.cloned()
		.ok_or_else(|| metadata_error(logical_name, "missing upload_to"))?;
	let storage_alias = metadata
		.params
		.get("file_storage")
		.cloned()
		.unwrap_or_else(|| "default".to_owned());
	let max_length = parse_metadata_usize(&metadata.params, logical_name, "max_length", 255)?;
	let cleanup = parse_metadata_bool(&metadata.params, logical_name, "cleanup", true)?;
	let validation = match kind {
		"file" => FileValidationPolicy::File,
		"image" => FileValidationPolicy::Image {
			max_width: parse_metadata_u32(&metadata.params, logical_name, "max_width")?,
			max_height: parse_metadata_u32(&metadata.params, logical_name, "max_height")?,
		},
		other => {
			return Err(metadata_error(
				logical_name,
				format!("unsupported kind {other}"),
			));
		}
	};

	Ok(FileFieldPolicy {
		model: Cow::Owned(model_name.to_owned()),
		field: Cow::Owned(logical_name.to_owned()),
		upload_to: Cow::Owned(upload_to),
		storage_alias: Cow::Owned(storage_alias),
		max_length,
		cleanup,
		validation,
	})
}

#[cfg(all(server, feature = "file-uploads"))]
fn all_file_fields_for_model(
	model_admin: &dyn ModelAdmin,
) -> Result<Vec<AdminFileField>, ServerFnError> {
	let metadata = find_model_by_table_name(model_admin.table_name())
		.ok_or_else(|| ServerFnError::server(500, "File field metadata is not registered"))?;
	let mut fields = metadata
		.fields
		.into_iter()
		.filter_map(|(registered_name, metadata)| {
			metadata
				.params
				.get("model_field_type")
				.is_some_and(|kind| matches!(kind.as_str(), "file" | "image"))
				.then_some((registered_name, metadata))
		})
		.map(|(registered_name, metadata)| {
			let logical_name = metadata
				.params
				.get("rust_field_name")
				.cloned()
				.unwrap_or_else(|| registered_name.clone());
			let mut aliases = Vec::with_capacity(3);
			for name in [
				Some(registered_name),
				Some(logical_name.clone()),
				metadata.params.get("db_column").cloned(),
			]
			.into_iter()
			.flatten()
			{
				if !aliases.iter().any(|alias| alias == &name) {
					aliases.push(name);
				}
			}
			let policy = file_field_policy(model_admin.model_name(), &logical_name, &metadata)?;
			Ok(AdminFileField {
				logical_name,
				aliases,
				required: infer_required(&metadata),
				nullable: metadata.nullable,
				policy,
			})
		})
		.collect::<Result<Vec<_>, ServerFnError>>()?;
	fields.sort_unstable_by(|left, right| left.logical_name.cmp(&right.logical_name));
	Ok(fields)
}

#[cfg(all(server, feature = "file-uploads"))]
fn file_fields_for_model(
	model_admin: &dyn ModelAdmin,
) -> Result<Vec<AdminFileField>, ServerFnError> {
	let mut fields = all_file_fields_for_model(model_admin)?;
	let (form_fields, _) = crate::core::resolve_form_fields(model_admin)
		.map_err(|_| ServerFnError::server(500, "Invalid admin form configuration"))?;
	let readonly_fields = model_admin.readonly_fields();
	fields.retain(|field| {
		let exposed = form_fields
			.iter()
			.any(|name| field.aliases.iter().any(|alias| alias == name));
		let readonly = readonly_fields
			.iter()
			.any(|name| field.aliases.iter().any(|alias| alias == name));
		exposed && !readonly
	});
	Ok(fields)
}

#[cfg(all(server, feature = "file-uploads"))]
fn find_file_field<'a>(fields: &'a [AdminFileField], name: &str) -> Option<&'a AdminFileField> {
	fields
		.iter()
		.find(|field| field.aliases.iter().any(|alias| alias == name))
}

#[cfg(all(server, feature = "file-uploads"))]
fn inline_file_target(
	name: &str,
	model_admin: &dyn ModelAdmin,
	site: &AdminSite,
) -> Result<InlineFileTarget, ServerFnError> {
	let path = name
		.strip_prefix(INLINE_PREFIX)
		.ok_or_else(|| validation_error(name, "Invalid inline file field"))?;
	let mut parts = path.split('.');
	let inline_key = parts
		.next()
		.filter(|value| !value.is_empty())
		.ok_or_else(|| validation_error(name, "Invalid inline file field"))?;
	let submitted_index = parts
		.next()
		.ok_or_else(|| validation_error(name, "Invalid inline file field"))?
		.parse::<usize>()
		.ok()
		.filter(|index| *index < 100)
		.ok_or_else(|| validation_error(name, "Invalid inline row index"))?;
	let field_name = parts
		.next()
		.filter(|value| !value.is_empty())
		.ok_or_else(|| validation_error(name, "Invalid inline file field"))?;
	if parts.next().is_some() {
		return Err(validation_error(name, "Invalid inline file field"));
	}
	let inline = model_admin
		.inlines()
		.into_iter()
		.find(|inline| inline.key() == inline_key)
		.ok_or_else(|| validation_error(name, "Unknown inline"))?;
	if !inline.fields().iter().any(|field| field == field_name) {
		return Err(validation_error(name, "Inline field is not editable"));
	}
	let child_admin = site
		.get_model_admin(inline.child_model())
		.map_server_fn_error()?;
	let child_fields = all_file_fields_for_model(child_admin.as_ref())?;
	let field = find_file_field(&child_fields, field_name)
		.ok_or_else(|| validation_error(name, "Unknown inline file field"))?;
	if child_admin
		.readonly_fields()
		.iter()
		.any(|configured| field.aliases.iter().any(|alias| alias == configured))
	{
		return Err(validation_error(name, "Inline field is read-only"));
	}

	Ok(InlineFileTarget {
		path: name.to_owned(),
		inline_key: inline_key.to_owned(),
		submitted_index,
		field_name: field.logical_name.clone(),
		policy: field.policy.clone(),
	})
}

#[cfg(all(server, feature = "file-uploads"))]
pub(crate) fn file_field_aliases(
	model_admin: &dyn ModelAdmin,
) -> Result<Vec<(String, String)>, ServerFnError> {
	let mut aliases = Vec::new();
	for field in file_fields_for_model(model_admin)? {
		for alias in field.aliases {
			if alias != field.logical_name {
				aliases.push((field.logical_name.clone(), alias));
			}
		}
	}
	Ok(aliases)
}

#[cfg(all(server, feature = "file-uploads"))]
pub(crate) fn reject_file_field_json_data(
	data: &HashMap<String, serde_json::Value>,
	model_admin: &dyn ModelAdmin,
	site: &AdminSite,
) -> Result<(), ServerFnError> {
	let fields = all_file_fields_for_model(model_admin)?;
	if let Some(name) = data
		.keys()
		.find(|name| find_file_field(&fields, name).is_some())
	{
		return Err(validation_error(
			name,
			"File fields must be submitted through the multipart endpoint",
		));
	}
	if let Some(name) = data.keys().find(|name| {
		name.starts_with(INLINE_PREFIX) && inline_file_target(name, model_admin, site).is_ok()
	}) {
		return Err(validation_error(
			name,
			"File fields must be submitted through the multipart endpoint",
		));
	}
	Ok(())
}

#[cfg(all(server, feature = "file-uploads"))]
fn existing_file_reference(
	values: Option<&HashMap<String, serde_json::Value>>,
	field_name: &str,
	policy: &FileFieldPolicy,
) -> Result<Option<FileField>, ServerFnError> {
	let Some(value) = values.and_then(|values| values.get(field_name)) else {
		return Ok(None);
	};
	match value {
		serde_json::Value::Null => Ok(None),
		serde_json::Value::String(path) if path.is_empty() => Ok(None),
		serde_json::Value::String(path) => {
			match FileField::from_existing(path.clone(), policy.storage_alias.as_ref()) {
				Ok(file) => Ok(Some(file)),
				Err(error) => {
					tracing::warn!(
						field = field_name,
						error = %error,
						"Stored file reference is invalid; cleanup was skipped"
					);
					Ok(None)
				}
			}
		}
		_ => Err(validation_error(
			field_name,
			"The stored file reference is not a string",
		)),
	}
}

#[cfg(all(test, server, feature = "file-uploads"))]
fn prepare_file_mutation(
	payload: AdminMultipartPayload,
	fields: &[AdminFileField],
	update: bool,
) -> Result<PreparedFileMutation, ServerFnError> {
	prepare_file_mutation_with_inlines(payload, fields, None, None, update)
}

#[cfg(all(server, feature = "file-uploads"))]
fn prepare_file_mutation_with_inlines(
	payload: AdminMultipartPayload,
	fields: &[AdminFileField],
	model_admin: Option<&dyn ModelAdmin>,
	site: Option<&AdminSite>,
	update: bool,
) -> Result<PreparedFileMutation, ServerFnError> {
	let mut data = payload.data;
	for name in data.keys() {
		if find_file_field(fields, name).is_some() {
			return Err(validation_error(
				name,
				"File fields must be submitted as file parts",
			));
		}
	}
	if let (Some(model_admin), Some(site)) = (model_admin, site) {
		for name in data.keys() {
			if name.starts_with(INLINE_PREFIX)
				&& inline_file_target(name, model_admin, site).is_ok()
			{
				return Err(validation_error(
					name,
					"File fields must be submitted as file parts",
				));
			}
		}
	}

	let mut uploads = HashMap::new();
	let mut inline_uploads = Vec::new();
	let mut inline_paths = HashSet::new();
	for (name, upload) in payload.uploads {
		if name.starts_with(INLINE_PREFIX) {
			let target = match (model_admin, site) {
				(Some(model_admin), Some(site)) => inline_file_target(&name, model_admin, site)?,
				_ => {
					return Err(validation_error(
						&name,
						"Inline file uploads are not configured",
					));
				}
			};
			if !inline_paths.insert(target.path.clone()) {
				return Err(validation_error(
					&target.path,
					"An inline file field was submitted more than once",
				));
			}
			inline_uploads.push((target, upload));
			continue;
		}
		let field = find_file_field(fields, &name)
			.ok_or_else(|| validation_error(&name, "Unknown file field"))?;
		if uploads
			.insert(field.logical_name.clone(), (name.clone(), upload))
			.is_some()
		{
			return Err(validation_error(
				&field.logical_name,
				"A file field was submitted more than once",
			));
		}
	}

	let mut empty_uploads = HashSet::new();
	for name in payload.empty_uploads {
		if name.starts_with(INLINE_PREFIX) {
			let target = match (model_admin, site) {
				(Some(model_admin), Some(site)) => inline_file_target(&name, model_admin, site)?,
				_ => {
					return Err(validation_error(
						&name,
						"Inline file uploads are not configured",
					));
				}
			};
			if !inline_paths.insert(target.path.clone()) {
				return Err(validation_error(
					&target.path,
					"An inline file field was submitted more than once",
				));
			}
			continue;
		}
		let field = find_file_field(fields, &name)
			.ok_or_else(|| validation_error(&name, "Unknown file field"))?;
		if !empty_uploads.insert(field.logical_name.clone())
			|| uploads.contains_key(&field.logical_name)
		{
			return Err(validation_error(
				&field.logical_name,
				"A file field was submitted more than once",
			));
		}
	}

	let mut clears = HashMap::new();
	for name in payload.clears {
		let field = find_file_field(fields, &name)
			.ok_or_else(|| validation_error(&name, "Unknown file field"))?;
		if !update {
			return Err(validation_error(
				&field.logical_name,
				"Clear markers are only valid when editing",
			));
		}
		if !field.nullable {
			return Err(validation_error(
				&field.logical_name,
				"This file field cannot be cleared",
			));
		}
		if clears
			.insert(field.logical_name.clone(), name.clone())
			.is_some()
			|| uploads.contains_key(&field.logical_name)
			|| empty_uploads.contains(&field.logical_name)
		{
			return Err(validation_error(
				&field.logical_name,
				"A file field cannot be uploaded and cleared together",
			));
		}
	}

	if !update {
		for field in fields.iter().filter(|field| field.required) {
			if !uploads.contains_key(&field.logical_name) {
				return Err(validation_error(&field.logical_name, "A file is required"));
			}
		}
	}

	let mut sorted_uploads = uploads.into_iter().collect::<Vec<_>>();
	sorted_uploads.sort_unstable_by(|left, right| left.0.cmp(&right.0));
	let mut writes = Vec::with_capacity(sorted_uploads.len());
	let mut write_fields = Vec::with_capacity(sorted_uploads.len());
	let mut inline_write_targets = Vec::with_capacity(sorted_uploads.len());
	let mut cleanup = Vec::new();
	for (logical_name, (submitted_name, upload)) in sorted_uploads {
		let field =
			find_file_field(fields, &logical_name).expect("canonical upload field must resolve");
		if update && field.policy.cleanup {
			cleanup.push(FileCleanupReference {
				policy: field.policy.clone(),
				field_name: field.logical_name.clone(),
				operation: FileCleanupOperation::Replace,
			});
		}
		writes.push(PendingFileUpload {
			policy: field.policy.clone(),
			operation: if update {
				FileWriteOperation::Replace
			} else {
				FileWriteOperation::Create
			},
			upload,
		});
		write_fields.push(submitted_name);
		inline_write_targets.push(None);
	}
	inline_uploads.sort_unstable_by(|left, right| left.0.path.cmp(&right.0.path));
	for (target, upload) in inline_uploads {
		writes.push(PendingFileUpload {
			policy: target.policy.clone(),
			operation: if update {
				FileWriteOperation::Replace
			} else {
				FileWriteOperation::Create
			},
			upload,
		});
		write_fields.push(target.path.clone());
		inline_write_targets.push(Some(target));
	}

	for (logical_name, submitted_name) in clears {
		let field =
			find_file_field(fields, &logical_name).expect("canonical clear field must resolve");
		if field.policy.cleanup {
			cleanup.push(FileCleanupReference {
				policy: field.policy.clone(),
				field_name: field.logical_name.clone(),
				operation: FileCleanupOperation::Clear,
			});
		}
		data.insert(submitted_name, serde_json::Value::Null);
	}

	Ok(PreparedFileMutation {
		data,
		writes,
		write_fields,
		inline_write_targets,
		cleanup,
	})
}

#[cfg(all(server, feature = "file-uploads"))]
fn map_file_mutation_error(
	error: FileMutationError<ServerFnError>,
	fallback_field: &str,
) -> ServerFnError {
	match error {
		FileMutationError::Database(error) => error,
		FileMutationError::StorageForField { field, source } => {
			tracing::warn!(
				field = field.as_str(),
				error = %source,
				"Admin file upload failed before database persistence"
			);
			validation_error(&field, "The file could not be stored")
		}
		FileMutationError::Storage(error) => {
			tracing::warn!(
				field = fallback_field,
				error = %error,
				"Admin file upload failed before database persistence"
			);
			validation_error(fallback_field, "The file could not be stored")
		}
	}
}

#[cfg(all(server, feature = "file-uploads"))]
async fn persist_file_mutation(
	payload: AdminMultipartPayload,
	id: Option<String>,
	csrf_token: String,
	context: FileMutationContext,
) -> Result<MutationResponse, ServerFnError> {
	let FileMutationContext {
		site,
		db,
		http_request,
		user,
		model_admin,
	} = context;
	let update = id.is_some();
	let fields = file_fields_for_model(model_admin.as_ref())?;
	let prepared = prepare_file_mutation_with_inlines(
		payload,
		&fields,
		Some(model_admin.as_ref()),
		Some(site.as_ref()),
		update,
	)?;
	let fallback_field = prepared
		.write_fields
		.first()
		.map(String::as_str)
		.unwrap_or("file")
		.to_owned();

	let model_name = model_admin.model_name().to_owned();
	let site_value = (*site).clone();
	let db_value = (*db).clone();
	let write_fields = prepared.write_fields;
	let inline_write_targets = prepared.inline_write_targets;
	let cleanup = prepared.cleanup;
	let data = prepared.data;
	let has_file_lifecycle = !prepared.writes.is_empty() || !cleanup.is_empty();
	let persist = move |stored: Vec<FileField>| async move {
		let mut data = data;
		let mut stored_inline_targets = Vec::new();
		for ((field_name, target), file) in write_fields
			.into_iter()
			.zip(inline_write_targets)
			.zip(stored)
		{
			if let Some(target) = target {
				stored_inline_targets.push(target);
			}
			data.insert(
				field_name,
				serde_json::Value::String(file.path().to_owned()),
			);
		}
		let request = MutationRequest { csrf_token, data };
		let (response, previous, outcomes) = match id {
			Some(id) => {
				let (response, previous, outcomes) = update_record_with_previous_values(
					model_name,
					id,
					request,
					KeyedDepends::from_value(site_value),
					KeyedDepends::from_value(db_value),
					http_request,
					AdminAuthenticatedUser(user),
				)
				.await?;
				(response, Some(previous), outcomes)
			}
			None => {
				let (response, outcomes) = create_record_with_inline_outcomes(
					model_name,
					request,
					KeyedDepends::from_value(site_value),
					KeyedDepends::from_value(db_value),
					http_request,
					AdminAuthenticatedUser(user),
				)
				.await?;
				(response, None, outcomes)
			}
		};
		let mut commit = FileCommit::new(response);
		for cleanup in cleanup {
			if let Some(file) =
				existing_file_reference(previous.as_ref(), &cleanup.field_name, &cleanup.policy)?
			{
				commit = commit.cleanup(cleanup.policy, file, cleanup.operation);
			}
		}
		for target in stored_inline_targets {
			if !target.policy.cleanup {
				continue;
			}
			let Some(outcome) = outcomes.iter().find(|outcome| {
				outcome.inline_key == target.inline_key
					&& outcome.submitted_index == target.submitted_index
			}) else {
				continue;
			};
			if let Some(file) = existing_file_reference(
				Some(&outcome.previous_values),
				&target.field_name,
				&target.policy,
			)? {
				commit = commit.cleanup(target.policy, file, FileCleanupOperation::Replace);
			}
		}
		Ok(commit)
	};
	let response = if has_file_lifecycle {
		coordinate_file_mutations(prepared.writes, persist)
			.await
			.map_err(|error| map_file_mutation_error(error, &fallback_field))?
	} else {
		persist(Vec::new())
			.await
			.map_err(|error| {
				map_file_mutation_error(FileMutationError::Database(error), &fallback_field)
			})?
			.value
	};

	Ok(response)
}

#[cfg(all(server, feature = "file-uploads"))]
fn multipart_csrf_token(request: &ServerFnRequest) -> Result<String, ServerFnError> {
	let token = extract_csrf_header(&request.inner().headers)
		.ok_or_else(|| ServerFnError::server(403, "CSRF token missing from header"))?;
	require_csrf_token(&token, &request.inner().headers)?;
	Ok(token)
}

/// Create an admin record from a multipart form containing file fields.
#[cfg(all(server, feature = "file-uploads"))]
#[server_fn]
pub async fn create_record_multipart(
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<MutationResponse, ServerFnError> {
	let csrf_token = multipart_csrf_token(&http_request)?;
	let payload = parse_admin_multipart(http_request.inner(), false).await?;
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site
		.get_model_admin(&payload.model_name)
		.map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Add)
		.await?;
	persist_file_mutation(
		payload,
		None,
		csrf_token,
		FileMutationContext {
			site,
			db,
			http_request,
			user,
			model_admin,
		},
	)
	.await
}

/// Update an admin record from a multipart form containing file fields.
#[cfg(all(server, feature = "file-uploads"))]
#[server_fn]
pub async fn update_record_multipart(
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<MutationResponse, ServerFnError> {
	let csrf_token = multipart_csrf_token(&http_request)?;
	let payload = parse_admin_multipart(http_request.inner(), true).await?;
	let id = payload
		.id
		.clone()
		.ok_or_else(|| ServerFnError::server(400, "Missing record ID"))?;
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site
		.get_model_admin(&payload.model_name)
		.map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Change)
		.await?;
	persist_file_mutation(
		payload,
		Some(id),
		csrf_token,
		FileMutationContext {
			site,
			db,
			http_request,
			user,
			model_admin,
		},
	)
	.await
}

#[cfg(all(server, feature = "file-uploads"))]
pub(crate) async fn cleanup_deleted_files(
	model_admin: &dyn ModelAdmin,
	values: Option<&HashMap<String, serde_json::Value>>,
) {
	let fields = match file_fields_for_model(model_admin) {
		Ok(fields) => fields,
		Err(error) => {
			tracing::warn!(error = %error, "Deleted file references could not be resolved");
			return;
		}
	};
	let mut commit = FileCommit::new(());
	let mut has_cleanup = false;
	for field in fields {
		if !field.policy.cleanup {
			continue;
		}
		let Some(serde_json::Value::String(path)) =
			values.and_then(|values| values.get(&field.logical_name))
		else {
			continue;
		};
		if path.is_empty() {
			continue;
		}
		let file = match FileField::from_existing(path.clone(), field.policy.storage_alias.as_ref())
		{
			Ok(file) => file,
			Err(error) => {
				tracing::warn!(
					field = field.logical_name.as_str(),
					error = %error,
					"Deleted file reference is invalid; cleanup was skipped"
				);
				continue;
			}
		};
		commit = commit.cleanup(field.policy, file, FileCleanupOperation::Delete);
		has_cleanup = true;
	}
	if !has_cleanup {
		return;
	}

	if let Err(error) =
		coordinate_file_mutations(Vec::new(), |_| async { Ok::<_, Infallible>(commit) }).await
	{
		tracing::warn!(error = %error, "Deleted file cleanup could not be scheduled");
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::*;

	fn multipart_request(parts: &str) -> reinhardt_http::Request {
		reinhardt_http::Request::builder()
			.uri("/api/server_fn/create_record_multipart")
			.header(
				hyper::header::CONTENT_TYPE,
				"multipart/form-data; boundary=boundary",
			)
			.body(parts.as_bytes().to_vec().into())
			.build()
			.expect("multipart request should build")
	}

	#[tokio::test]
	async fn parse_admin_multipart_extracts_fields_uploads_and_clears() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_id\"\r\n\r\n\"42\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n\"Hello\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_clear.thumbnail\"\r\n\r\ntrue\r\n--boundary\r\nContent-Disposition: form-data; name=\"image\"; filename=\"cover.png\"\r\nContent-Type: image/png\r\n\r\npng\r\n--boundary\r\nContent-Disposition: form-data; name=\"attachment\"; filename=\"\"\r\nContent-Type: application/octet-stream\r\n\r\n\r\n--boundary--\r\n",
		);

		let payload = parse_admin_multipart(&request, true)
			.await
			.expect("multipart payload should parse");

		assert_eq!(payload.model_name, "Article");
		assert_eq!(payload.id.as_deref(), Some("42"));
		assert_eq!(payload.data.get("title"), Some(&serde_json::json!("Hello")));
		assert_eq!(
			payload
				.uploads
				.get("image")
				.and_then(|file| file.filename.as_deref()),
			Some("cover.png")
		);
		assert!(payload.empty_uploads.contains("attachment"));
		assert!(payload.clears.contains("thumbnail"));
	}

	#[tokio::test]
	async fn parse_admin_multipart_preserves_inline_controls_and_file_parts() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_inlines.comments.0.__present\"\r\n\r\ntrue\r\n--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_inlines.comments.0.avatar\"; filename=\"avatar.png\"\r\nContent-Type: image/png\r\n\r\nimage\r\n--boundary--\r\n",
		);

		let payload = parse_admin_multipart(&request, false)
			.await
			.expect("inline controls should remain available to inline parsing");

		assert_eq!(
			payload.data.get("__reinhardt_inlines.comments.0.__present"),
			Some(&serde_json::json!(true))
		);
		assert_eq!(
			payload
				.uploads
				.get("__reinhardt_inlines.comments.0.avatar")
				.and_then(|file| file.filename.as_deref()),
			Some("avatar.png")
		);
	}

	#[tokio::test]
	async fn parse_admin_multipart_rejects_duplicate_names() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n\"one\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n\"two\"\r\n--boundary--\r\n",
		);

		let error = parse_admin_multipart(&request, false)
			.await
			.expect_err("duplicate multipart names must fail");

		assert_eq!(error.status(), Some(400));
	}

	#[tokio::test]
	async fn parse_admin_multipart_rejects_malformed_json() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nnot-json\r\n--boundary--\r\n",
		);

		let error = parse_admin_multipart(&request, false)
			.await
			.expect_err("malformed JSON must fail");

		assert_eq!(error.status(), Some(400));
	}

	#[tokio::test]
	async fn parse_admin_multipart_requires_an_id_for_updates() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary--\r\n",
		);

		let error = parse_admin_multipart(&request, true)
			.await
			.expect_err("update multipart payloads require an ID");

		assert_eq!(error.status(), Some(400));
	}

	#[tokio::test]
	async fn parse_admin_multipart_rejects_update_parts_on_creates() {
		let request = multipart_request(
			"--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_model\"\r\n\r\n\"Article\"\r\n--boundary\r\nContent-Disposition: form-data; name=\"__reinhardt_id\"\r\n\r\n\"42\"\r\n--boundary--\r\n",
		);

		let error = parse_admin_multipart(&request, false)
			.await
			.expect_err("create multipart payloads must reject update-only parts");

		assert_eq!(error.status(), Some(400));
	}
}

#[cfg(all(test, server, feature = "file-uploads"))]
mod file_lifecycle_tests {
	use super::*;
	use bytes::Bytes;

	fn field(required: bool, nullable: bool, cleanup: bool) -> AdminFileField {
		AdminFileField {
			logical_name: "avatar".to_owned(),
			aliases: vec!["avatar".to_owned(), "avatar_path".to_owned()],
			required,
			nullable,
			policy: FileFieldPolicy {
				model: Cow::Borrowed("Article"),
				field: Cow::Borrowed("avatar"),
				upload_to: Cow::Borrowed("avatars"),
				storage_alias: Cow::Borrowed("default"),
				max_length: 255,
				cleanup,
				validation: FileValidationPolicy::File,
			},
		}
	}

	fn payload() -> AdminMultipartPayload {
		AdminMultipartPayload {
			model_name: "Article".to_owned(),
			id: None,
			data: HashMap::new(),
			uploads: HashMap::new(),
			empty_uploads: HashSet::new(),
			clears: HashSet::new(),
		}
	}

	#[test]
	fn required_create_requires_and_prepares_a_file_write() {
		let mut payload = payload();
		payload.uploads.insert(
			"avatar_path".to_owned(),
			UploadedFile::new("avatar_path".to_owned(), Bytes::from_static(b"image"))
				.with_filename("avatar.png".to_owned()),
		);

		let prepared = prepare_file_mutation(payload, &[field(true, false, true)], false)
			.expect("required upload should be prepared");

		assert_eq!(prepared.write_fields, vec!["avatar_path"]);
		assert_eq!(prepared.writes.len(), 1);
		assert_eq!(prepared.writes[0].operation, FileWriteOperation::Create);
	}

	#[test]
	fn update_without_a_new_file_preserves_the_existing_reference() {
		let mut payload = payload();
		payload
			.data
			.insert("title".to_owned(), serde_json::json!("Updated"));
		let prepared = prepare_file_mutation(payload, &[field(true, false, true)], true)
			.expect("omitting an update file should preserve it");

		assert!(prepared.writes.is_empty());
		assert!(prepared.cleanup.is_empty());
		assert_eq!(
			prepared.data.get("title"),
			Some(&serde_json::json!("Updated"))
		);
	}

	#[test]
	fn nullable_clear_sets_null_and_schedules_old_file_cleanup() {
		let mut payload = payload();
		payload.clears.insert("avatar_path".to_owned());
		let prepared = prepare_file_mutation(payload, &[field(false, true, true)], true)
			.expect("nullable clear should be prepared");

		assert_eq!(
			prepared.data.get("avatar_path"),
			Some(&serde_json::Value::Null)
		);
		assert_eq!(prepared.cleanup.len(), 1);
		assert_eq!(prepared.cleanup[0].field_name, "avatar");
		assert_eq!(prepared.cleanup[0].operation, FileCleanupOperation::Clear);
	}

	#[test]
	fn cleanup_opt_out_keeps_the_old_reference_without_scheduling_cleanup() {
		let mut payload = payload();
		payload.clears.insert("avatar_path".to_owned());
		let prepared = prepare_file_mutation(payload, &[field(false, true, false)], true)
			.expect("cleanup opt-out should still clear the database value");

		assert_eq!(
			prepared.data.get("avatar_path"),
			Some(&serde_json::Value::Null)
		);
		assert!(prepared.cleanup.is_empty());
	}

	#[test]
	fn image_metadata_builds_dimension_validation_policy() {
		let metadata = reinhardt_db::migrations::FieldMetadata::new(
			reinhardt_db::migrations::FieldType::VarChar(255),
		)
		.with_param("model_field_type", "image")
		.with_param("upload_to", "images")
		.with_param("max_length", "120")
		.with_param("max_width", "800")
		.with_param("max_height", "600");

		let policy = file_field_policy("Article", "cover", &metadata)
			.expect("image metadata should produce a file policy");

		assert_eq!(policy.max_length, 120);
		assert!(matches!(
			policy.validation,
			FileValidationPolicy::Image {
				max_width: Some(800),
				max_height: Some(600)
			}
		));
	}
}
