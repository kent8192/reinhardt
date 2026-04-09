//! List view Server Function
//!
//! Provides list view operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{
	AdminDatabase, AdminRecord, AdminSite, ColumnInfo, FilterInfo, FilterType, ListQueryParams,
	ListResponse, ModelAdmin,
};
#[cfg(server)]
use reinhardt_db::orm::{Filter, FilterCondition, FilterOperator, FilterValue};
use reinhardt_di::Depends;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
use std::sync::Arc;

#[cfg(server)]
use super::error::MapServerFnError;
#[cfg(server)]
use super::limits::MAX_PAGE_SIZE;
#[cfg(server)]
use crate::server::type_inference::{
	get_field_metadata, infer_admin_field_type, infer_filter_type,
};
#[cfg(server)]
use reinhardt_utils::utils_core::text::humanize_field_name;

#[cfg(server)]
fn build_filters(model_admin: &Arc<dyn ModelAdmin>) -> Vec<FilterInfo> {
	let table_name = model_admin.table_name();
	model_admin
		.list_filter()
		.iter()
		.map(|field| {
			// Infer filter type from field metadata in global registry
			let filter_type = get_field_metadata(table_name, field)
				.map(|meta| {
					let admin_type = infer_admin_field_type(&meta.field_type);
					infer_filter_type(&admin_type)
				})
				.unwrap_or(FilterType::Boolean);

			FilterInfo {
				field: field.to_string(),
				title: humanize_field_name(field),
				filter_type,
				current_value: None,
			}
		})
		.collect()
}

#[cfg(server)]
fn build_columns(model_admin: &Arc<dyn ModelAdmin>) -> Vec<ColumnInfo> {
	model_admin
		.list_display()
		.iter()
		.map(|field| ColumnInfo {
			field: field.to_string(),
			label: humanize_field_name(field),
			sortable: true,
		})
		.collect()
}

/// Get list view data with search, filters, sorting, and pagination
///
/// Retrieves a paginated list of records with optional search across multiple fields,
/// field-specific filters, and custom ordering. Returns the records along with
/// pagination metadata and available filter/column information.
///
/// # Server Function
///
/// This function is automatically exposed as an HTTP endpoint by the `#[server_fn]` macro.
/// AdminSite and AdminDatabase dependencies are automatically injected via the DI system.
///
/// # Authentication
///
/// Requires authentication and view permission for the model.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::get_list;
/// use reinhardt_admin::types::ListQueryParams;
/// use std::collections::HashMap;
///
/// // Client-side usage (automatically generates HTTP request)
/// let params = ListQueryParams {
///     search: Some("alice".to_string()),
///     filters: HashMap::new(),
///     sort_by: Some("created_at".to_string()),
///     page: Some(1),
///     page_size: Some(25),
/// };
/// let response = get_list("User".to_string(), params).await?;
/// println!("Found {} users", response.count);
/// ```
#[server_fn]
pub async fn get_list(
	model_name: String,
	params: ListQueryParams,
	#[inject] site: Depends<AdminSite>,
	#[inject] db: Depends<AdminDatabase>,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<ListResponse, ServerFnError> {
	// Get model admin and check permission
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	if !model_admin.has_view_permission(user.as_ref()).await {
		return Err(ServerFnError::server(403, "Permission denied"));
	}

	// Build search condition (OR across search fields)
	let mut filter_condition: Option<FilterCondition> = None;
	if let Some(search) = params.search.as_ref() {
		let search_fields = model_admin.search_fields();
		if !search_fields.is_empty() && !search.is_empty() {
			let search_filters: Vec<FilterCondition> = search_fields
				.iter()
				.map(|field| {
					FilterCondition::Single(Filter::new(
						field.to_string(),
						FilterOperator::Contains,
						FilterValue::String(search.clone()),
					))
				})
				.collect();

			if !search_filters.is_empty() {
				filter_condition = Some(FilterCondition::Or(search_filters));
			}
		}
	}

	// Build additional filters (AND logic)
	// Only accept filter fields that are explicitly defined in model_admin.list_filter()
	let allowed_filter_fields = model_admin.list_filter();
	let mut additional_filters = Vec::new();
	for (field, value) in params.filters.iter() {
		if !allowed_filter_fields.contains(&field.as_str()) {
			return Err(ServerFnError::server(
				400,
				format!(
					"Unknown filter field '{}'. Allowed filter fields: {:?}",
					field, allowed_filter_fields
				),
			));
		}
		additional_filters.push(Filter::new(
			field.clone(),
			FilterOperator::Eq,
			FilterValue::String(value.clone()),
		));
	}

	// Determine sort field
	let sort_by = params
		.sort_by
		.as_deref()
		.or_else(|| model_admin.ordering().first().copied());

	// Validate sort_by against allowed fields to prevent arbitrary column access
	if let Some(sort_field) = sort_by {
		let raw_field = sort_field.strip_prefix('-').unwrap_or(sort_field);
		let allowed_sort_fields = model_admin.list_display();
		if !allowed_sort_fields.contains(&raw_field) {
			return Err(ServerFnError::server(
				400,
				format!(
					"Unknown sort field '{}'. Allowed sort fields: {:?}",
					raw_field, allowed_sort_fields
				),
			));
		}
	}

	// Calculate pagination with upper bound enforcement
	let page = params.page.unwrap_or(1).max(1); // Ensure page is at least 1
	let page_size = params
		.page_size
		.unwrap_or_else(|| {
			let admin_settings = crate::settings::get_admin_settings();
			model_admin
				.list_per_page()
				.unwrap_or(admin_settings.list_per_page) as u64
		})
		.min(MAX_PAGE_SIZE); // Enforce maximum page size to prevent memory exhaustion
	let offset = (page - 1) * page_size;

	// Fetch data
	let results = db
		.list_with_condition::<AdminRecord>(
			model_admin.table_name(),
			filter_condition.as_ref(),
			additional_filters.clone(),
			sort_by,
			offset,
			page_size,
		)
		.await
		.map_server_fn_error()?;

	// Count total records
	let count = db
		.count_with_condition::<AdminRecord>(
			model_admin.table_name(),
			filter_condition.as_ref(),
			additional_filters,
		)
		.await
		.map_server_fn_error()?;

	// Calculate total pages
	let total_pages = if count > 0 {
		count.div_ceil(page_size)
	} else {
		1
	};

	Ok(ListResponse {
		model_name,
		count,
		page,
		page_size,
		total_pages,
		results,
		available_filters: Some(build_filters(&model_admin)),
		columns: Some(build_columns(&model_admin)),
	})
}
