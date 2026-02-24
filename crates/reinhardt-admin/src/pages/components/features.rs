//! Feature-specific components
//!
//! Provides feature-specific UI components:
//! - `Dashboard` - Dashboard view
//! - `ListView` - List view with filters and pagination
//! - `DetailView` - Detail view for a single record
//! - `ModelForm` - Form for creating/editing records
//! - `Filters` - Filter panel
//! - `DataTable` - Data table component

use crate::types::{FilterInfo, FilterType, ModelInfo};
use reinhardt_pages::Signal;
use reinhardt_pages::component::{IntoPage, Page, PageElement};
use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
use reinhardt_pages::dom::EventType;

#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

/// Dashboard component
///
/// Displays the admin dashboard with model cards.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::features::dashboard;
/// use reinhardt_admin::types::ModelInfo;
///
/// let models = vec![
///     ModelInfo { name: "Users".to_string(), list_url: "/admin/users/".to_string() },
///     ModelInfo { name: "Posts".to_string(), list_url: "/admin/posts/".to_string() },
/// ];
/// dashboard("My Admin Panel", &models)
/// ```
pub fn dashboard(site_name: &str, models: &[ModelInfo]) -> Page {
	PageElement::new("div")
		.attr("class", "dashboard")
		.child(
			PageElement::new("h1")
				.attr("class", "mb-4")
				.child(format!("{} Dashboard", site_name)),
		)
		.child(
			PageElement::new("div")
				.attr("class", "row")
				.child(models_grid(models)),
		)
		.into_page()
}

/// Generates a grid of model cards
fn models_grid(models: &[ModelInfo]) -> Page {
	if models.is_empty() {
		return PageElement::new("div")
			.attr("class", "col-12")
			.child(
				PageElement::new("div")
					.attr("class", "alert alert-info")
					.child("No models registered. Add models to AdminSite to see them here."),
			)
			.into_page();
	}

	let card_views: Vec<Page> = models
		.iter()
		.map(|model| {
			PageElement::new("div")
				.attr("class", "col-md-4")
				.child(model_card(&model.name, &model.list_url))
				.into_page()
		})
		.collect();

	PageElement::new("div")
		.attr("class", "col-12")
		.child(
			PageElement::new("div")
				.attr("class", "row g-4")
				.children(card_views),
		)
		.into_page()
}

/// Generates a single model card
fn model_card(name: &str, url: &str) -> Page {
	PageElement::new("div")
		.attr("class", "card h-100")
		.child(
			PageElement::new("div")
				.attr("class", "card-body")
				.child(
					PageElement::new("h5")
						.attr("class", "card-title")
						.child(name.to_string()),
				)
				.child(
					PageElement::new("p")
						.attr("class", "card-text")
						.child(format!("Manage {} records", name)),
				)
				.child(
					PageElement::new("a")
						.attr("class", "btn btn-primary")
						.attr("href", url.to_string())
						.child(format!("View {}", name)),
				),
		)
		.into_page()
}

/// Column definition for list view
#[derive(Debug, Clone)]
pub struct Column {
	/// Column field name
	pub field: String,
	/// Column display label
	pub label: String,
	/// Whether this column is sortable
	pub sortable: bool,
}

/// List view data structure
#[derive(Debug, Clone)]
pub struct ListViewData {
	/// Model name
	pub model_name: String,
	/// Column definitions
	pub columns: Vec<Column>,
	/// Record data (each record is a HashMap of field -> value)
	pub records: Vec<std::collections::HashMap<String, String>>,
	/// Current page number (1-indexed)
	pub current_page: u64,
	/// Total number of pages
	pub total_pages: u64,
	/// Total number of records
	pub total_count: u64,
	/// Filter information
	pub filters: Vec<FilterInfo>,
}

/// List view component
///
/// Displays a paginated list of records with filters and search.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::features::{list_view, ListViewData, Column};
/// use reinhardt_pages::Signal;
/// use std::collections::HashMap;
///
/// let data = ListViewData {
///     model_name: "User".to_string(),
///     columns: vec![
///         Column { field: "id".to_string(), label: "ID".to_string(), sortable: true },
///         Column { field: "username".to_string(), label: "Username".to_string(), sortable: true },
///     ],
///     records: vec![/* ... */],
///     current_page: 1,
///     total_pages: 5,
///     total_count: 42,
///     filters: vec![],
/// };
/// let page_signal = Signal::new(1u64);
/// let filters_signal = Signal::new(HashMap::new());
/// list_view(&data, page_signal, filters_signal)
/// ```
pub fn list_view(
	data: &ListViewData,
	current_page_signal: reinhardt_pages::Signal<u64>,
	filters_signal: Signal<HashMap<String, String>>,
) -> Page {
	PageElement::new("div")
		.attr("class", "list-view")
		.child(
			PageElement::new("h1")
				.attr("class", "mb-4")
				.child(format!("{} List", data.model_name)),
		)
		.child(filters(&data.filters, filters_signal))
		.child(PageElement::new("div").attr("class", "mb-3").child(format!(
			"Showing {} {} (Page {} of {})",
			data.total_count, data.model_name, data.current_page, data.total_pages
		)))
		.child(data_table(&data.columns, &data.records, &data.model_name))
		.child(crate::pages::components::common::pagination(
			current_page_signal,
			data.total_pages,
		))
		.into_page()
}

/// Generates a data table
fn data_table(
	columns: &[Column],
	records: &[std::collections::HashMap<String, String>],
	model_name: &str,
) -> Page {
	// Table header
	let header_cells: Vec<Page> = columns
		.iter()
		.map(|col| PageElement::new("th").child(col.label.clone()).into_page())
		.chain(std::iter::once(
			PageElement::new("th").child("Actions").into_page(),
		))
		.collect();

	let thead = PageElement::new("thead").child(PageElement::new("tr").children(header_cells));

	// Table body
	let body_rows: Vec<Page> = records
		.iter()
		.map(|record| table_row(columns, record, model_name))
		.collect();

	let tbody = PageElement::new("tbody").children(body_rows);

	PageElement::new("div")
		.attr("class", "table-responsive")
		.child(
			PageElement::new("table")
				.attr("class", "table table-striped table-hover")
				.child(thead)
				.child(tbody),
		)
		.into_page()
}

/// Generates a table row for a single record
fn table_row(
	columns: &[Column],
	record: &std::collections::HashMap<String, String>,
	model_name: &str,
) -> Page {
	// Data cells
	let data_cells: Vec<Page> = columns
		.iter()
		.map(|col| {
			let value = record
				.get(&col.field)
				.cloned()
				.unwrap_or_else(|| "-".to_string());
			PageElement::new("td").child(value).into_page()
		})
		.collect();

	// Actions cell
	let record_id = record.get("id").cloned().unwrap_or_else(|| "0".to_string());
	let actions_cell = PageElement::new("td")
		.child(action_buttons(model_name, &record_id))
		.into_page();

	PageElement::new("tr")
		.children(data_cells)
		.child(actions_cell)
		.into_page()
}

/// Generates action buttons for a record
fn action_buttons(model_name: &str, record_id: &str) -> Page {
	use reinhardt_pages::component::Component;
	use reinhardt_pages::router::Link;

	let detail_url = format!("/admin/{}/{}/", model_name.to_lowercase(), record_id);
	let edit_url = format!("/admin/{}/{}/change/", model_name.to_lowercase(), record_id);

	PageElement::new("div")
		.attr("class", "btn-group btn-group-sm")
		.attr("role", "group")
		.child(
			Link::new(detail_url.clone(), "View")
				.class("btn btn-outline-primary")
				.render(),
		)
		.child(
			Link::new(edit_url.clone(), "Edit")
				.class("btn btn-outline-secondary")
				.render(),
		)
		.into_page()
}

/// Form field definition for model forms
#[derive(Debug, Clone)]
pub struct FormField {
	/// Field name (corresponds to database column)
	pub name: String,
	/// Field display label
	pub label: String,
	/// HTML input type (text, email, number, etc.)
	pub field_type: String,
	/// Whether this field is required
	pub required: bool,
	/// Current field value (for edit forms)
	pub value: String,
}

/// Detail view component
///
/// Displays detailed information about a single record.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::features::detail_view;
/// use std::collections::HashMap;
///
/// let mut record = HashMap::new();
/// record.insert("id".to_string(), "1".to_string());
/// record.insert("username".to_string(), "john_doe".to_string());
/// detail_view("User", "1", &record)
/// ```
pub fn detail_view(
	model_name: &str,
	record_id: &str,
	record: &std::collections::HashMap<String, String>,
) -> Page {
	use reinhardt_pages::component::Component;
	use reinhardt_pages::router::Link;

	let edit_url = format!("/admin/{}/{}/change/", model_name.to_lowercase(), record_id);
	let list_url = format!("/admin/{}/", model_name.to_lowercase());

	PageElement::new("div")
		.attr("class", "detail-view")
		.child(
			PageElement::new("h1")
				.attr("class", "mb-4")
				.child(format!("{} Detail", model_name)),
		)
		.child(detail_table(record))
		.child(
			PageElement::new("div")
				.attr("class", "mt-4")
				.child(
					Link::new(edit_url, "Edit")
						.class("btn btn-primary me-2")
						.render(),
				)
				.child(
					Link::new(list_url, "Back to List")
						.class("btn btn-secondary")
						.render(),
				),
		)
		.into_page()
}

/// Generates a detail table for record fields
fn detail_table(record: &std::collections::HashMap<String, String>) -> Page {
	let rows: Vec<Page> = record
		.iter()
		.map(|(key, value)| {
			PageElement::new("tr")
				.child(
					PageElement::new("th")
						.attr("class", "w-25")
						.child(key.clone()),
				)
				.child(PageElement::new("td").child(value.clone()))
				.into_page()
		})
		.collect();

	PageElement::new("div")
		.attr("class", "table-responsive")
		.child(
			PageElement::new("table")
				.attr("class", "table table-bordered")
				.child(PageElement::new("tbody").children(rows)),
		)
		.into_page()
}

/// Model form component
///
/// Displays a form for creating or editing a record.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::features::{model_form, FormField};
///
/// let fields = vec![
///     FormField {
///         name: "username".to_string(),
///         label: "Username".to_string(),
///         field_type: "text".to_string(),
///         required: true,
///         value: "".to_string(),
///     },
/// ];
/// model_form("User", &fields, None)
/// ```
pub fn model_form(model_name: &str, fields: &[FormField], record_id: Option<&str>) -> Page {
	use reinhardt_pages::component::Component;
	use reinhardt_pages::router::Link;

	let form_title = if record_id.is_some() {
		format!("Edit {}", model_name)
	} else {
		format!("Create {}", model_name)
	};

	let list_url = format!("/admin/{}/", model_name.to_lowercase());

	// Add form fields
	let form_groups: Vec<Page> = fields.iter().map(form_group).collect();

	PageElement::new("div")
		.attr("class", "model-form")
		.child(
			PageElement::new("h1")
				.attr("class", "mb-4")
				.child(form_title),
		)
		.child(
			PageElement::new("form")
				.attr("class", "needs-validation")
				.attr("novalidate", "true")
				.children(form_groups)
				.child(
					PageElement::new("div")
						.attr("class", "mt-4")
						.child(
							PageElement::new("button")
								.attr("class", "btn btn-primary me-2")
								.attr("type", "submit")
								.child("Save"),
						)
						.child(
							Link::new(list_url, "Cancel")
								.class("btn btn-secondary")
								.render(),
						),
				),
		)
		.into_page()
}

/// Generates a form group (label + input) for a field
fn form_group(field: &FormField) -> Page {
	let input_id = format!("field-{}", field.name);

	PageElement::new("div")
		.attr("class", "mb-3")
		.child(
			PageElement::new("label")
				.attr("class", "form-label")
				.attr("for", input_id.clone())
				.child(field.label.clone()),
		)
		.child(form_element(field, &input_id))
		.into_page()
}

/// Generates an input element for a form field
fn form_element(field: &FormField, input_id: &str) -> Page {
	let mut input_builder = PageElement::new("input")
		.attr("class", "form-control")
		.attr("type", field.field_type.clone())
		.attr("id", input_id.to_string())
		.attr("name", field.name.clone())
		.attr("value", field.value.clone());

	if field.required {
		input_builder = input_builder.attr("required", "true");
	}

	input_builder.into_page()
}

/// Convert FilterType to choice list
///
/// Generates a list of (value, label) pairs for select options.
/// Always includes an "All" option as the first choice.
fn filter_type_to_choices(filter_type: &FilterType) -> Vec<(String, String)> {
	let mut choices = vec![("".to_string(), "All".to_string())];

	match filter_type {
		FilterType::Boolean => {
			choices.push(("true".to_string(), "Yes".to_string()));
			choices.push(("false".to_string(), "No".to_string()));
		}
		FilterType::Choice {
			choices: filter_choices,
		} => {
			for choice in filter_choices {
				choices.push((choice.value.clone(), choice.label.clone()));
			}
		}
		FilterType::DateRange { ranges } => {
			for range in ranges {
				choices.push((range.value.clone(), range.label.clone()));
			}
		}
		FilterType::NumberRange { ranges } => {
			for range in ranges {
				choices.push((range.value.clone(), range.label.clone()));
			}
		}
	}

	choices
}

/// Create filter select element
///
/// Generates a <select> element for a filter field.
/// Includes SSR/WASM conditional compilation for event handlers.
fn create_filter_select(
	field: &str,
	filter_type: &FilterType,
	current_value: Option<&str>,
	_filters_signal: Signal<HashMap<String, String>>,
) -> Page {
	let choices = filter_type_to_choices(filter_type);
	let current_val = current_value.unwrap_or("");

	// Generate <option> elements
	let options: Vec<Page> = choices
		.iter()
		.map(|(value, label)| {
			let mut opt = PageElement::new("option")
				.attr("value", value.clone())
				.child(label.clone());

			if value == current_val {
				opt = opt.attr("selected", "true");
			}

			opt.into_page()
		})
		.collect();

	// WASM: Add event handler for filter changes
	#[cfg(target_arch = "wasm32")]
	let select_view = {
		use wasm_bindgen::JsCast;
		use web_sys::HtmlSelectElement;

		let field_clone = field.to_string();
		let filters_signal = _filters_signal;

		PageElement::new("select")
			.attr("class", "form-select form-select-sm")
			.attr("data-filter-field", field.to_string())
			.children(options)
			.on(
				EventType::Change,
				Arc::new(move |event: web_sys::Event| {
					if let Some(target) = event.target() {
						if let Ok(select) = target.dyn_into::<HtmlSelectElement>() {
							let value = select.value();
							let field_name = field_clone.clone();

							filters_signal.update(move |map| {
								if value.is_empty() {
									map.remove(&field_name);
								} else {
									map.insert(field_name, value);
								}
							});
						}
					}
				}),
			)
	};

	// SSR: No event handler (will be hydrated on client)
	#[cfg(not(target_arch = "wasm32"))]
	let select_view = {
		PageElement::new("select")
			.attr("class", "form-select form-select-sm")
			.attr("data-filter-field", field.to_string())
			.attr("data-reactive", "true") // Marker for client-side hydration
			.children(options)
	};

	select_view.into_page()
}

/// Create filter control (label + select)
///
/// Generates a complete filter control with label and select element.
fn create_filter_control(
	filter_info: &FilterInfo,
	current_value: Option<&str>,
	filters_signal: Signal<HashMap<String, String>>,
) -> Page {
	PageElement::new("div")
		.attr("class", "col-md-3")
		.child(
			PageElement::new("div")
				.attr("class", "mb-2")
				.child(
					PageElement::new("label")
						.attr("class", "form-label")
						.child(filter_info.title.clone()),
				)
				.child(create_filter_select(
					&filter_info.field,
					&filter_info.filter_type,
					current_value,
					filters_signal,
				)),
		)
		.into_page()
}

/// Filters component
///
/// Displays filter controls for list views.
/// Uses Signal to track current filter values.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::features::filters;
/// use reinhardt_admin::types::{FilterInfo, FilterType};
/// use reinhardt_pages::Signal;
/// use std::collections::HashMap;
///
/// let filters_signal = Signal::new(HashMap::new());
/// let filter_infos = vec![
///     FilterInfo {
///         field: "status".to_string(),
///         title: "Status".to_string(),
///         filter_type: FilterType::Boolean,
///         current_value: None,
///     },
/// ];
/// filters(&filter_infos, filters_signal)
/// ```
pub fn filters(
	filters_info: &[FilterInfo],
	filters_signal: Signal<HashMap<String, String>>,
) -> Page {
	if filters_info.is_empty() {
		return PageElement::new("div").into_page();
	}

	let current_filters = filters_signal.get();

	let filter_controls: Vec<Page> = filters_info
		.iter()
		.map(|info| {
			let current_value = current_filters.get(&info.field).map(|s| s.as_str());
			create_filter_control(info, current_value, filters_signal.clone())
		})
		.collect();

	PageElement::new("div")
		.attr("class", "filters mb-3")
		.child(
			PageElement::new("h5")
				.attr("class", "mb-2")
				.child("Filters"),
		)
		.child(
			PageElement::new("div")
				.attr("class", "row g-2")
				.children(filter_controls),
		)
		.into_page()
}
