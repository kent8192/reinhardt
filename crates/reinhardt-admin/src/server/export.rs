//! Export operation Server Function
//!
//! Provides export operations for admin models.

use crate::adapters::{AdminDatabase, AdminRecord, AdminSite, ExportFormat, ExportResponse};
use reinhardt_pages::server_fn::{ServerFnError, ServerFnRequest, server_fn};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use super::error::{AdminAuth, MapServerFnError};
#[cfg(not(target_arch = "wasm32"))]
use super::limits::MAX_EXPORT_RECORDS;

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
#[server_fn(use_inject = true)]
pub async fn export_data(
	model_name: String,
	format: ExportFormat,
	#[inject] site: Arc<AdminSite>,
	#[inject] db: Arc<AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
) -> Result<ExportResponse, ServerFnError> {
	// Authentication and authorization check
	let auth = AdminAuth::from_request(&http_request);
	auth.require_view_permission(&model_name)?;

	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	let table_name = model_admin.table_name();

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
			let mut wtr = csv::Writer::from_writer(vec![]);

			// Write records to CSV
			for record in results {
				wtr.serialize(record).map_err(|e| {
					ServerFnError::serialization(format!("CSV serialization failed: {}", e))
				})?;
			}

			let data = wtr
				.into_inner()
				.map_err(|e| ServerFnError::serialization(format!("CSV write failed: {}", e)))?;

			(
				data,
				format!("{}.csv", model_name.to_lowercase()),
				"text/csv",
			)
		}
		ExportFormat::TSV => {
			let mut wtr = csv::WriterBuilder::new()
				.delimiter(b'\t')
				.from_writer(vec![]);

			// Write records to TSV
			for record in results {
				wtr.serialize(record).map_err(|e| {
					ServerFnError::serialization(format!("TSV serialization failed: {}", e))
				})?;
			}

			let data = wtr
				.into_inner()
				.map_err(|e| ServerFnError::serialization(format!("TSV write failed: {}", e)))?;

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
	})
}
