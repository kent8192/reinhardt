//! Import operation Server Function
//!
//! Provides import operations for admin models from various formats (JSON, CSV, TSV).

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite, ImportFormat, ImportResponse};
#[cfg(server)]
use crate::core::history::{ensure_history_schema, insert_history_event};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use std::collections::HashMap;

#[cfg(server)]
use super::audit;
#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::limits::{MAX_IMPORT_FILE_SIZE, MAX_IMPORT_RECORDS};

/// Import model data from various formats
///
/// Imports records from uploaded data in the specified format (JSON, CSV, TSV).
/// Each record is inserted as a new entry. Returns statistics about the import operation.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Authentication
///
/// Requires staff (admin) permission and add permission for the model.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::import_data;
/// use reinhardt_admin::types::ImportFormat;
///
/// // Client-side usage (automatically generates HTTP request)
/// let file_data = vec![/* binary data */];
/// let response = import_data(
///     "User".to_string(),
///     ImportFormat::JSON,
///     file_data
/// ).await?;
/// println!("Imported {} records", response.imported);
/// ```
#[server_fn]
pub async fn import_data(
	model_name: String,
	format: crate::adapters::ImportFormat,
	data: Vec<u8>,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<crate::adapters::ImportResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::Add)
		.await?;

	// Validate import file size to prevent memory exhaustion
	if data.len() > MAX_IMPORT_FILE_SIZE {
		return Err(ServerFnError::application(format!(
			"Import file size ({} bytes) exceeds maximum allowed size ({} bytes)",
			data.len(),
			MAX_IMPORT_FILE_SIZE
		)));
	}

	let model_name = model_admin.model_name().to_string();
	let table_name = model_admin.table_name().to_string();
	let pk_field = model_admin.pk_field().to_string();
	let actor = user.get_username().to_string();

	// Parse data based on format
	// Sanitize error messages to avoid exposing internal details (schema, SQL, etc.)
	let records: Vec<HashMap<String, serde_json::Value>> = match format {
		ImportFormat::JSON => serde_json::from_slice(&data)
			.map_err(|_| ServerFnError::deserialization("Invalid JSON format in import data"))?,
		ImportFormat::CSV => {
			let mut rdr = csv::Reader::from_reader(&data[..]);
			rdr.deserialize()
				.collect::<Result<Vec<_>, _>>()
				.map_err(|_| ServerFnError::deserialization("Invalid CSV format in import data"))?
		}
		ImportFormat::TSV => {
			let mut rdr = csv::ReaderBuilder::new()
				.delimiter(b'\t')
				.from_reader(&data[..]);
			rdr.deserialize()
				.collect::<Result<Vec<_>, _>>()
				.map_err(|_| ServerFnError::deserialization("Invalid TSV format in import data"))?
		}
	};

	// Validate record count to prevent database overload
	if records.len() > MAX_IMPORT_RECORDS {
		return Err(ServerFnError::application(format!(
			"Import record count ({}) exceeds maximum allowed count ({})",
			records.len(),
			MAX_IMPORT_RECORDS
		)));
	}

	// Import records
	let mut imported = 0;
	let mut failed = 0;
	let mut errors = Vec::new();
	let mut connection = *db.connection();
	ensure_history_schema(&mut connection)
		.await
		.map_err(|_| ServerFnError::server(500, "History storage is unavailable"))?;

	for (index, record) in records.into_iter().enumerate() {
		let changed_fields = record.keys().cloned().collect();
		let result: reinhardt_core::exception::Result<_> = connection
			.atomic_write(async |transaction| {
				let created = db
					.create_with_executor::<AdminRecord, _>(
						transaction,
						&table_name,
						Some(&pk_field),
						record,
					)
					.await?;
				let object_id = created
					.primary_key
					.as_str()
					.map(ToOwned::to_owned)
					.unwrap_or_else(|| created.primary_key.to_string());
				let event = audit::new_history_event(
					&actor,
					"IMPORT",
					&model_name,
					&table_name,
					&object_id,
					changed_fields,
					created.affected,
					true,
				);
				insert_history_event(transaction, &event).await?;
				Ok(())
			})
			.await;
		match result {
			Ok(_) => imported += 1,
			Err(_) => {
				// Hide internal error details (SQL fragments, table structures, column names)
				// to prevent information disclosure aiding reconnaissance attacks
				failed += 1;
				errors.push(format!("Record {}: import failed", index + 1));
			}
		}
	}

	Ok(ImportResponse {
		success: failed == 0,
		imported,
		updated: 0, // Not supporting updates in basic import
		skipped: 0,
		failed,
		message: if failed == 0 {
			format!("Successfully imported {} {} records", imported, model_name)
		} else {
			format!(
				"Imported {} {} records, {} failed",
				imported, model_name, failed
			)
		},
		errors: if errors.is_empty() {
			None
		} else {
			Some(errors)
		},
	})
}
