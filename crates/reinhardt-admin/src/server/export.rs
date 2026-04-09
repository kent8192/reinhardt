//! Export operation Server Function
//!
//! Provides export operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{AdminDatabase, AdminRecord, AdminSite, ExportFormat, ExportResponse};
use reinhardt_di::Depends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::limits::MAX_EXPORT_RECORDS;

/// Serialize records as delimiter-separated values (CSV or TSV).
///
/// Uses `BTreeMap` for consistent column ordering across records and
/// `write_record` for compatibility with the csv crate (which does not
/// support serializing maps via `serialize`).
#[cfg(server)]
fn serialize_delimited(
	results: &[std::collections::HashMap<String, serde_json::Value>],
	delimiter: u8,
) -> Result<Vec<u8>, String> {
	use std::collections::BTreeMap;

	if results.is_empty() {
		return Ok(Vec::new());
	}

	let mut wtr = csv::WriterBuilder::new()
		.delimiter(delimiter)
		.from_writer(vec![]);

	// Convert all records to BTreeMap for consistent column ordering
	let flat_records: Vec<BTreeMap<String, String>> = results
		.iter()
		.map(|record| {
			record
				.iter()
				.map(|(k, v)| {
					let s = match v {
						serde_json::Value::String(s) => s.clone(),
						serde_json::Value::Null => String::new(),
						other => other.to_string(),
					};
					(k.clone(), s)
				})
				.collect()
		})
		.collect();

	// Write header row from first record's keys
	let headers: Vec<&str> = flat_records[0].keys().map(|k| k.as_str()).collect();
	wtr.write_record(&headers)
		.map_err(|e| format!("header serialization failed: {}", e))?;

	// Write each record's values in header order
	for record in &flat_records {
		let values: Vec<&str> = headers
			.iter()
			.map(|h| record.get(*h).map(|v| v.as_str()).unwrap_or(""))
			.collect();
		wtr.write_record(&values)
			.map_err(|e| format!("record serialization failed: {}", e))?;
	}

	wtr.into_inner()
		.map_err(|e| format!("writer flush failed: {}", e))
}

/// Export model data in various formats
///
/// Exports all records from a model table in the specified format (JSON, CSV, TSV).
/// Returns the exported data as binary content with appropriate content type and filename.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Authentication
///
/// Requires staff (admin) permission and view permission for the model.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::export_data;
/// use reinhardt_admin::types::ExportFormat;
///
/// // Client-side usage (automatically generates HTTP request)
/// let response = export_data("User".to_string(), ExportFormat::JSON).await?;
/// println!("Downloaded {}", response.filename);
/// ```
#[server_fn]
pub async fn export_data(
	model_name: String,
	format: ExportFormat,
	#[inject] site: Depends<AdminSite>,
	#[inject] db: Depends<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<ExportResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(model_admin.as_ref(), user.as_ref(), ModelPermission::View)
		.await?;
	let table_name = model_admin.table_name();

	// Query total count to detect truncation
	let total_count = db
		.count::<AdminRecord>(table_name, vec![])
		.await
		.map_server_fn_error()?;
	let truncated = total_count > MAX_EXPORT_RECORDS;

	if truncated {
		tracing::warn!(
			"Export for model '{}' truncated: {} total records, limit is {}",
			model_name,
			total_count,
			MAX_EXPORT_RECORDS
		);
	}

	// Fetch records with export limit to prevent memory exhaustion
	let results = db
		.list::<AdminRecord>(table_name, vec![], 0, MAX_EXPORT_RECORDS)
		.await
		.map_server_fn_error()?;

	// Serialize based on format
	let (data, filename, content_type) = match format {
		ExportFormat::JSON => {
			let json = serde_json::to_vec_pretty(&results).map_err(|e| {
				ServerFnError::serialization(format!("JSON serialization failed: {}", e))
			})?;
			(
				json,
				format!("{}.json", model_name.to_lowercase()),
				"application/json",
			)
		}
		ExportFormat::CSV => {
			let data = serialize_delimited(&results, b',').map_err(|e| {
				ServerFnError::serialization(format!("CSV serialization failed: {}", e))
			})?;

			(
				data,
				format!("{}.csv", model_name.to_lowercase()),
				"text/csv",
			)
		}
		ExportFormat::TSV => {
			let data = serialize_delimited(&results, b'\t').map_err(|e| {
				ServerFnError::serialization(format!("TSV serialization failed: {}", e))
			})?;

			(
				data,
				format!("{}.tsv", model_name.to_lowercase()),
				"text/tab-separated-values",
			)
		}
		ExportFormat::Excel => {
			return Err(ServerFnError::application(
				"Excel export format is not supported",
			));
		}
		ExportFormat::XML => {
			return Err(ServerFnError::application(
				"XML export format is not supported",
			));
		}
	};

	Ok(ExportResponse {
		data,
		filename,
		content_type: content_type.to_string(),
		truncated,
		total_count: Some(total_count),
	})
}
