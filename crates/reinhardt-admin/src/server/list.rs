//! List view Server Function
//!
//! Provides list view operations for admin models.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
use crate::adapters::{
	AdminDatabase, AdminQuery, AdminRequestContext, AdminSite, ColumnInfo, DateHierarchyInfo,
	DateHierarchyLevel, DateHierarchyListResponse, DateHierarchySelection, FilterInfo, FilterType,
	ListColumn, ListResponse, ModelAdmin,
};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey};
#[cfg(server)]
use reinhardt_db::migrations::FieldType as DbFieldType;
#[cfg(server)]
use reinhardt_db::orm::{DatabaseValue, Filter, FilterCondition, FilterOperator, FilterValue};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
use std::sync::Arc;

#[cfg(server)]
use super::error::MapServerFnError;
#[cfg(server)]
use super::limits::MAX_PAGE_SIZE;
#[cfg(server)]
use crate::server::type_inference::{
	get_field_metadata, infer_admin_field_type, infer_filter_type, resolve_list_select_related,
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
fn build_columns(columns: &[ListColumn]) -> Vec<ColumnInfo> {
	columns
		.iter()
		.map(|column| match column {
			ListColumn::Field { field, label } => ColumnInfo {
				field: field.clone(),
				label: label.clone(),
				sortable: true,
			},
			ListColumn::Computed {
				key,
				label,
				sort_field,
			} => ColumnInfo {
				field: key.clone(),
				label: label.clone(),
				sortable: sort_field.is_some(),
			},
		})
		.collect()
}

#[cfg(server)]
fn resolve_sort_field(
	columns: &[ListColumn],
	sort_by: Option<&str>,
) -> Result<Option<String>, ServerFnError> {
	let Some(sort_by) = sort_by else {
		return Ok(None);
	};
	let (key, descending) = sort_by
		.strip_prefix('-')
		.map_or((sort_by, false), |key| (key, true));
	let mapped = columns.iter().find_map(|column| match column {
		ListColumn::Field { field, .. } if field == key => Some(Ok(field.as_str())),
		ListColumn::Computed {
			key: column_key,
			sort_field,
			..
		} if column_key == key => Some(sort_field.as_deref().ok_or_else(|| {
			ServerFnError::server(
				400,
				format!("Computed sort field '{key}' has no database mapping"),
			)
		})),
		_ => None,
	});
	let mapped = mapped
		.ok_or_else(|| ServerFnError::server(400, format!("Unknown sort field '{key}'")))??;
	Ok(Some(if descending {
		format!("-{mapped}")
	} else {
		mapped.to_string()
	}))
}

#[cfg(server)]
fn resolve_default_sort_field(
	table_name: &str,
	sort_by: Option<&str>,
) -> Result<Option<String>, ServerFnError> {
	let Some(sort_by) = sort_by else {
		return Ok(None);
	};
	let key = sort_by.strip_prefix('-').unwrap_or(sort_by);
	if get_field_metadata(table_name, key).is_none() {
		return Err(ServerFnError::server(
			400,
			format!("Unknown sort field '{key}'"),
		));
	}
	Ok(Some(sort_by.to_string()))
}

#[cfg(server)]
fn date_hierarchy_interval(
	selection: &DateHierarchySelection,
	field_type: &DbFieldType,
) -> Result<Option<(DatabaseValue, DatabaseValue)>, ServerFnError> {
	use chrono::Days;

	if selection.month.is_some() && selection.year.is_none() {
		return Err(ServerFnError::server(
			400,
			"Date hierarchy month requires a year",
		));
	}
	if selection.day.is_some() && (selection.year.is_none() || selection.month.is_none()) {
		return Err(ServerFnError::server(
			400,
			"Date hierarchy day requires a year and month",
		));
	}
	let Some(year) = selection.year else {
		return Ok(None);
	};

	let (start, end) = match (selection.month, selection.day) {
		(None, None) => {
			let start = chrono::NaiveDate::from_ymd_opt(year, 1, 1);
			let end = year
				.checked_add(1)
				.and_then(|year| chrono::NaiveDate::from_ymd_opt(year, 1, 1));
			(start, end)
		}
		(Some(month), None) => {
			let start = chrono::NaiveDate::from_ymd_opt(year, month, 1);
			let end = if month == 12 {
				year.checked_add(1)
					.and_then(|year| chrono::NaiveDate::from_ymd_opt(year, 1, 1))
			} else {
				month
					.checked_add(1)
					.and_then(|month| chrono::NaiveDate::from_ymd_opt(year, month, 1))
			};
			(start, end)
		}
		(Some(month), Some(day)) => {
			let start = chrono::NaiveDate::from_ymd_opt(year, month, day);
			let end = start.and_then(|date| date.checked_add_days(Days::new(1)));
			(start, end)
		}
		(None, Some(_)) => unreachable!("day dependency is validated above"),
	};
	let (start, end) = start.zip(end).ok_or_else(|| {
		ServerFnError::server(400, "Invalid or unrepresentable date hierarchy selection")
	})?;

	match field_type {
		DbFieldType::Date => Ok(Some((DatabaseValue::Date(start), DatabaseValue::Date(end)))),
		DbFieldType::DateTime => Ok(Some((
			DatabaseValue::NaiveDateTime(start.and_time(chrono::NaiveTime::MIN)),
			DatabaseValue::NaiveDateTime(end.and_time(chrono::NaiveTime::MIN)),
		))),
		DbFieldType::TimestampTz => Ok(Some((
			DatabaseValue::DateTime(start.and_time(chrono::NaiveTime::MIN).and_utc()),
			DatabaseValue::DateTime(end.and_time(chrono::NaiveTime::MIN).and_utc()),
		))),
		_ => Err(ServerFnError::server(
			400,
			"Date hierarchy field must be a date or datetime field",
		)),
	}
}

#[cfg(server)]
fn date_hierarchy_level(selection: &DateHierarchySelection) -> Option<DateHierarchyLevel> {
	match (selection.year, selection.month, selection.day) {
		(None, None, None) => Some(DateHierarchyLevel::Year),
		(Some(_), None, None) => Some(DateHierarchyLevel::Month),
		(Some(_), Some(_), None) => Some(DateHierarchyLevel::Day),
		(Some(_), Some(_), Some(_)) => None,
		_ => None,
	}
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
	params: crate::adapters::ListQueryParams,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] user: AdminAuthenticatedUser,
) -> Result<crate::adapters::ListResponse, ServerFnError> {
	Ok(get_list_impl(
		model_name,
		params.into(),
		site,
		db,
		http_request,
		user,
		false,
	)
	.await?
	.response)
}

/// Get list view data with date hierarchy metadata.
///
/// This versioned endpoint extends the legacy list contract without adding
/// fields to [`crate::types::ListQueryParams`] or [`crate::types::ListResponse`].
#[server_fn]
pub async fn get_list_with_date_hierarchy(
	model_name: String,
	params: crate::adapters::DateHierarchyListQueryParams,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] user: AdminAuthenticatedUser,
) -> Result<crate::adapters::DateHierarchyListResponse, ServerFnError> {
	get_list_impl(model_name, params, site, db, http_request, user, true).await
}

#[cfg(server)]
async fn get_list_impl(
	model_name: String,
	params: crate::adapters::DateHierarchyListQueryParams,
	site: KeyedDepends<AdminSiteKey, AdminSite>,
	db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	http_request: ServerFnRequest,
	user: AdminAuthenticatedUser,
	include_date_hierarchy: bool,
) -> Result<crate::adapters::DateHierarchyListResponse, ServerFnError> {
	// Get model admin and check permission
	let model_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	if !model_admin.has_view_permission(user.0.as_ref()).await {
		return Err(ServerFnError::server(403, "Permission denied"));
	}
	let request_context = AdminRequestContext::new(http_request.into_inner());
	let mut admin_query = model_admin
		.get_queryset(
			user.0.as_ref(),
			&request_context,
			AdminQuery::new(model_admin.table_name()),
		)
		.await
		.map_server_fn_error()?;
	let columns = model_admin.list_columns();
	let related_fields =
		resolve_list_select_related(model_admin.table_name(), &model_admin.list_select_related())
			.map_server_fn_error()?;

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
	let sort_by = if let Some(sort_by) = params.sort_by.as_deref() {
		resolve_sort_field(&columns, Some(sort_by))?
	} else {
		resolve_default_sort_field(
			model_admin.table_name(),
			model_admin.ordering().first().copied(),
		)?
	};

	let hierarchy = if include_date_hierarchy {
		if let Some(field) = model_admin.date_hierarchy() {
			let metadata =
				get_field_metadata(model_admin.table_name(), field).ok_or_else(|| {
					ServerFnError::server(
						400,
						format!("Date hierarchy field '{field}' does not exist"),
					)
				})?;
			if !matches!(
				metadata.field_type,
				DbFieldType::Date | DbFieldType::DateTime | DbFieldType::TimestampTz
			) {
				return Err(ServerFnError::server(
					400,
					format!("Date hierarchy field '{field}' must be a date or datetime field"),
				));
			}
			let selection = params.date_hierarchy.clone().unwrap_or_default();
			let interval = date_hierarchy_interval(&selection, &metadata.field_type)?;
			let db_field = metadata
				.params
				.get("db_column")
				.cloned()
				.unwrap_or_else(|| field.to_string());
			Some((
				field.to_string(),
				db_field,
				metadata.field_type,
				selection,
				interval,
			))
		} else {
			if params.date_hierarchy.is_some() {
				return Err(ServerFnError::server(
					400,
					"Date hierarchy is not configured",
				));
			}
			None
		}
	} else {
		None
	};

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
	if let Some(filter_condition) = filter_condition {
		admin_query = admin_query.filter_condition(filter_condition);
	}
	for filter in additional_filters {
		admin_query = admin_query.filter(filter);
	}
	if let Some((_, db_field, _, _, Some((start, end)))) = &hierarchy {
		admin_query = admin_query
			.filter(Filter::new(
				db_field,
				FilterOperator::Gte,
				FilterValue::Typed(Ok(start.clone())),
			))
			.filter(Filter::new(
				db_field,
				FilterOperator::Lt,
				FilterValue::Typed(Ok(end.clone())),
			));
	}

	// Fetch page data and total count in one query for the common non-empty page path.
	let (mut results, count) = db
		.list_admin_query_with_count(
			&admin_query,
			&related_fields,
			sort_by.as_deref(),
			offset,
			page_size,
		)
		.await
		.map_server_fn_error()?;

	for row in &mut results {
		for column in &columns {
			let ListColumn::Computed { key, .. } = column else {
				continue;
			};
			let value = model_admin.computed_list_value(key, row).map_err(|error| {
				tracing::error!(
					model = model_admin.model_name(),
					column = key,
					error = ?error,
					"Admin computed list column failed"
				);
				ServerFnError::server(500, "Failed to compute list column")
			})?;
			row.insert(key.clone(), value);
		}
	}

	let date_hierarchy = if let Some((field, db_field, field_type, selection, _)) = hierarchy {
		let next_level = date_hierarchy_level(&selection);
		let choices = if let Some(level) = next_level {
			db.date_hierarchy_choices(&admin_query, &db_field, level, &field_type, &related_fields)
				.await
				.map_server_fn_error()?
		} else {
			Vec::new()
		};
		Some(DateHierarchyInfo {
			field,
			selection,
			next_level,
			choices,
		})
	} else {
		None
	};

	// Calculate total pages
	let total_pages = if count > 0 {
		count.div_ceil(page_size)
	} else {
		1
	};

	Ok(DateHierarchyListResponse {
		response: ListResponse {
			model_name,
			count,
			page,
			page_size,
			total_pages,
			results,
			available_filters: Some(build_filters(&model_admin)),
			columns: Some(build_columns(&columns)),
		},
		date_hierarchy,
	})
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::adapters::{DateHierarchySelection, ListColumn};
	use chrono::{NaiveDate, TimeZone, Utc};
	use reinhardt_db::migrations::FieldType as DbFieldType;
	use reinhardt_db::orm::DatabaseValue;

	#[test]
	fn date_hierarchy_date_intervals_are_half_open_and_accept_leap_day() {
		let cases = [
			(
				DateHierarchySelection {
					year: Some(2024),
					month: None,
					day: None,
				},
				NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
				NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
			),
			(
				DateHierarchySelection {
					year: Some(2024),
					month: Some(2),
					day: None,
				},
				NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
				NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
			),
			(
				DateHierarchySelection {
					year: Some(2024),
					month: Some(2),
					day: Some(29),
				},
				NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(),
				NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
			),
		];

		for (selection, expected_start, expected_end) in cases {
			assert_eq!(
				date_hierarchy_interval(&selection, &DbFieldType::Date).unwrap(),
				Some((
					DatabaseValue::Date(expected_start),
					DatabaseValue::Date(expected_end),
				))
			);
		}
	}

	#[test]
	fn date_hierarchy_datetime_intervals_preserve_naive_midnight() {
		let selection = DateHierarchySelection {
			year: Some(2024),
			month: Some(2),
			day: Some(29),
		};

		assert_eq!(
			date_hierarchy_interval(&selection, &DbFieldType::DateTime).unwrap(),
			Some((
				DatabaseValue::NaiveDateTime(
					NaiveDate::from_ymd_opt(2024, 2, 29)
						.unwrap()
						.and_hms_opt(0, 0, 0)
						.unwrap(),
				),
				DatabaseValue::NaiveDateTime(
					NaiveDate::from_ymd_opt(2024, 3, 1)
						.unwrap()
						.and_hms_opt(0, 0, 0)
						.unwrap(),
				),
			))
		);
	}

	#[test]
	fn date_hierarchy_timestamptz_intervals_bind_utc_midnight() {
		let selection = DateHierarchySelection {
			year: Some(2024),
			month: Some(2),
			day: Some(29),
		};

		assert_eq!(
			date_hierarchy_interval(&selection, &DbFieldType::TimestampTz).unwrap(),
			Some((
				DatabaseValue::DateTime(Utc.with_ymd_and_hms(2024, 2, 29, 0, 0, 0).unwrap(),),
				DatabaseValue::DateTime(Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap(),),
			))
		);
	}

	#[test]
	fn date_hierarchy_levels_follow_selection_depth() {
		let cases = [
			(
				DateHierarchySelection::default(),
				Some(DateHierarchyLevel::Year),
			),
			(
				DateHierarchySelection {
					year: Some(2024),
					month: None,
					day: None,
				},
				Some(DateHierarchyLevel::Month),
			),
			(
				DateHierarchySelection {
					year: Some(2024),
					month: Some(2),
					day: None,
				},
				Some(DateHierarchyLevel::Day),
			),
			(
				DateHierarchySelection {
					year: Some(2024),
					month: Some(2),
					day: Some(29),
				},
				None,
			),
		];

		for (selection, expected) in cases {
			assert_eq!(date_hierarchy_level(&selection), expected);
		}
	}

	#[test]
	fn date_hierarchy_rejects_invalid_dependencies_dates_and_boundaries() {
		let cases = [
			DateHierarchySelection {
				year: None,
				month: Some(1),
				day: None,
			},
			DateHierarchySelection {
				year: Some(2023),
				month: Some(2),
				day: Some(29),
			},
			DateHierarchySelection {
				year: Some(262_142),
				month: Some(12),
				day: Some(31),
			},
		];

		for selection in cases {
			let error = date_hierarchy_interval(&selection, &DbFieldType::Date)
				.expect_err("invalid selection must be rejected");
			assert_eq!(error.status(), Some(400));
		}
	}

	#[test]
	fn computed_sort_key_maps_to_real_field_and_requires_mapping() {
		let columns = vec![
			ListColumn::Field {
				field: "id".to_string(),
				label: "ID".to_string(),
			},
			ListColumn::Computed {
				key: "summary".to_string(),
				label: "Summary".to_string(),
				sort_field: Some("created_at".to_string()),
			},
			ListColumn::Computed {
				key: "badge".to_string(),
				label: "Badge".to_string(),
				sort_field: None,
			},
		];

		assert_eq!(
			resolve_sort_field(&columns, Some("-summary")).unwrap(),
			Some("-created_at".to_string())
		);
		assert_eq!(
			resolve_sort_field(&columns, Some("badge"))
				.expect_err("unmapped computed sort must fail")
				.status(),
			Some(400)
		);
	}
}
