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
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use reinhardt_pages::Signal;
use reinhardt_pages::component::{IntoPage, Page, PageElement};
use reinhardt_pages::page;
use std::collections::HashMap;

/// Characters that must be percent-encoded in URL path segments.
///
/// This set encodes characters that are unsafe or reserved in URL paths,
/// while preserving RFC 3986 unreserved characters (`A-Z`, `a-z`, `0-9`, `-`, `_`, `.`, `~`).
/// Encoded characters: space, `"`, `#`, `%`, `/`, `<`, `>`, `?`, `[`, `]`, `^`, `` ` ``, `{`, `|`, `}`.
const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
	.add(b' ')
	.add(b'"')
	.add(b'#')
	.add(b'%')
	.add(b'/')
	.add(b'<')
	.add(b'>')
	.add(b'?')
	.add(b'[')
	.add(b']')
	.add(b'^')
	.add(b'`')
	.add(b'{')
	.add(b'|')
	.add(b'}');

/// Percent-encode a string for safe use in URL path segments.
///
/// Encodes characters that are unsafe for URL path segments while preserving
/// RFC 3986 unreserved characters (`-`, `_`, `.`, `~`) to avoid unnecessarily
/// mangling valid route segments such as `user-management`.
fn encode_path_segment(s: &str) -> String {
	utf8_percent_encode(s, PATH_SEGMENT_ENCODE_SET).to_string()
}

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
	let site_name = site_name.to_string();
	let grid = models_grid(models);

	page!(|| {
		div {
			class: "dashboard animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-2xl font-bold text-slate-900 mb-6",
				{ format!("{} Dashboard", site_name) }
			}
			{ grid }
		}
	})()
}

/// Generates a grid of model cards
fn models_grid(models: &[ModelInfo]) -> Page {
	if models.is_empty() {
		return page!(|| {
			div {
				class: "admin-alert admin-alert-info",
				"No models registered. Add models to AdminSite to see them here."
			}
		})();
	}

	let card_views: Vec<Page> = models
		.iter()
		.map(|model| model_card(&model.name, &model.list_url))
		.collect();

	PageElement::new("div")
		.attr(
			"class",
			"grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
		)
		.children(card_views)
		.into_page()
}

/// Generates a single model card
fn model_card(name: &str, url: &str) -> Page {
	let name = name.to_string();
	let url = url.to_string();
	let label = format!("View {}", &name);

	page!(|| {
		div {
			class: "admin-card p-5 flex flex-col animate__animated animate__fadeInUp",
			h3 {
				class: "font-display text-lg font-bold text-slate-900 mb-1",
				{ name.clone() }
			}
			p {
				class: "text-sm text-slate-500 mb-4 flex-1",
				{ format!("Manage {} records", name) }
			}
			a {
				class: "admin-btn admin-btn-primary text-center",
				href: url,
				{ label }
			}
		}
	})()
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
	let title = format!("{} List", data.model_name);
	let summary = format!(
		"Showing {} {} (Page {} of {})",
		data.total_count, data.model_name, data.current_page, data.total_pages
	);
	let filters_page = filters(&data.filters, filters_signal);
	let table_page = data_table(&data.columns, &data.records, &data.model_name);
	let pagination_page =
		crate::pages::components::common::pagination(current_page_signal, data.total_pages);

	page!(|| {
		div {
			class: "list-view animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-2xl font-bold text-slate-900 mb-6",
				{ title }
			}
			{ filters_page }
			div {
				class: "text-sm text-slate-500 mb-4",
				{ summary }
			}
			{ table_page }
			{ pagination_page }
		}
	})()
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
		.attr(
			"class",
			"overflow-x-auto rounded-lg border border-slate-200",
		)
		.child(
			PageElement::new("table")
				.attr("class", "admin-table")
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

	let encoded_model = encode_path_segment(&model_name.to_lowercase());
	let encoded_id = encode_path_segment(record_id);
	let detail_url = format!("/admin/{}/{}/", encoded_model, encoded_id);
	let edit_url = format!("/admin/{}/{}/change/", encoded_model, encoded_id);

	PageElement::new("div")
		.attr("class", "flex gap-1")
		.child(
			Link::new(detail_url.clone(), "View")
				.class("admin-btn admin-btn-outline admin-btn-sm")
				.render(),
		)
		.child(
			Link::new(edit_url.clone(), "Edit")
				.class("admin-btn admin-btn-outline admin-btn-sm")
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

	let encoded_model = encode_path_segment(&model_name.to_lowercase());
	let encoded_id = encode_path_segment(record_id);
	let edit_url = format!("/admin/{}/{}/change/", encoded_model, encoded_id);
	let list_url = format!("/admin/{}/", encoded_model);

	let title = format!("{} Detail", model_name);
	let table_page = detail_table(record);
	let edit_link = Link::new(edit_url, "Edit")
		.class("admin-btn admin-btn-primary mr-2")
		.render();
	let back_link = Link::new(list_url, "Back to List")
		.class("admin-btn admin-btn-secondary")
		.render();

	page!(|| {
		div {
			class: "detail-view animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-2xl font-bold text-slate-900 mb-6",
				{ title }
			}
			{ table_page }
			div {
				class: "mt-6 flex gap-2",
				{ edit_link }
				{ back_link }
			}
		}
	})()
}

/// Generates a detail table for record fields
fn detail_table(record: &std::collections::HashMap<String, String>) -> Page {
	// Collect key-value pairs and sort by key for deterministic field display order
	let mut entries: Vec<(&String, &String)> = record.iter().collect();
	entries.sort_by_key(|(k, _)| *k);
	let rows: Vec<Page> = entries
		.into_iter()
		.map(|(key, value)| {
			PageElement::new("tr")
				.child(
					PageElement::new("th")
						.attr(
							"class",
							"w-1/4 text-left text-sm font-medium text-slate-500 py-3 px-4 bg-slate-50",
						)
						.child(key.clone()),
				)
				.child(
					PageElement::new("td")
						.attr("class", "text-sm text-slate-800 py-3 px-4")
						.child(value.clone()),
				)
				.into_page()
		})
		.collect();

	PageElement::new("div")
		.attr(
			"class",
			"overflow-x-auto rounded-lg border border-slate-200",
		)
		.child(
			PageElement::new("table")
				.attr("class", "admin-table")
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

	let action_url = if let Some(rid) = record_id {
		format!(
			"/admin/{}/{}/change/",
			encode_path_segment(&model_name.to_lowercase()),
			encode_path_segment(rid)
		)
	} else {
		format!(
			"/admin/{}/add/",
			encode_path_segment(&model_name.to_lowercase())
		)
	};

	let list_url = format!(
		"/admin/{}/",
		encode_path_segment(&model_name.to_lowercase())
	);

	let form_groups: Vec<Page> = fields.iter().map(form_group).collect();
	let cancel_link = Link::new(list_url, "Cancel")
		.class("admin-btn admin-btn-secondary")
		.render();

	PageElement::new("div")
		.attr(
			"class",
			"model-form max-w-2xl animate__animated animate__fadeIn",
		)
		.child(
			PageElement::new("h1")
				.attr(
					"class",
					"font-display text-2xl font-bold text-slate-900 mb-6",
				)
				.child(form_title),
		)
		.child(
			PageElement::new("form")
				.attr("method", "POST")
				.attr("action", action_url)
				.child(
					PageElement::new("div")
						.attr("class", "admin-card p-6")
						.children(form_groups),
				)
				.child(
					PageElement::new("div")
						.attr("class", "mt-6 flex gap-2")
						.child(
							PageElement::new("button")
								.attr("class", "admin-btn admin-btn-primary")
								.attr("type", "submit")
								.child("Save"),
						)
						.child(cancel_link),
				),
		)
		.into_page()
}

/// Generates a form group (label + input) for a field
fn form_group(field: &FormField) -> Page {
	let input_id = format!("field-{}", field.name);
	let label = field.label.clone();
	let input = form_element(field, &input_id);

	page!(|| {
		div {
			class: "mb-4",
			label {
				r#for: input_id,
				class: "admin-label",
				{ label }
			}
			{ input }
		}
	})()
}

/// Generates an input element for a form field
fn form_element(field: &FormField, input_id: &str) -> Page {
	let mut input_builder = PageElement::new("input")
		.attr("class", "admin-input")
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
			.attr("class", "admin-select")
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
			.attr("class", "admin-select")
			.attr("data-filter-field", field.to_string())
			.attr("data-reactive", "true")
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
	let label = filter_info.title.clone();
	let select = create_filter_select(
		&filter_info.field,
		&filter_info.filter_type,
		current_value,
		filters_signal,
	);

	page!(|| {
		div {
			class: "min-w-48",
			label {
				class: "admin-label",
				{ label }
			}
			{ select }
		}
	})()
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
		.attr("class", "admin-card p-4 mb-4")
		.child(
			PageElement::new("h5")
				.attr(
					"class",
					"text-xs font-semibold uppercase tracking-wider text-slate-500 mb-3",
				)
				.child("Filters"),
		)
		.child(
			PageElement::new("div")
				.attr("class", "flex flex-wrap gap-4")
				.children(filter_controls),
		)
		.into_page()
}

#[cfg(test)]
mod tests {
	use super::detail_table;
	use rstest::rstest;
	use std::collections::HashMap;

	/// Verifies that detail_table renders fields in alphabetical order regardless
	/// of HashMap insertion order.
	#[rstest]
	fn test_detail_table_renders_fields_in_alphabetical_order() {
		// Arrange
		let mut record = HashMap::new();
		record.insert("zebra".to_string(), "z_value".to_string());
		record.insert("alpha".to_string(), "a_value".to_string());
		record.insert("middle".to_string(), "m_value".to_string());

		// Act
		let page = detail_table(&record);
		let html = page.render_to_string();

		// Assert: alpha must appear before middle, and middle before zebra
		let pos_alpha = html.find("alpha").expect("alpha field must be present");
		let pos_middle = html.find("middle").expect("middle field must be present");
		let pos_zebra = html.find("zebra").expect("zebra field must be present");
		assert!(
			pos_alpha < pos_middle,
			"alpha must appear before middle in rendered output"
		);
		assert!(
			pos_middle < pos_zebra,
			"middle must appear before zebra in rendered output"
		);
	}

	/// Verifies that detail_table renders associated values alongside their keys.
	#[rstest]
	fn test_detail_table_renders_key_value_pairs() {
		// Arrange
		let mut record = HashMap::new();
		record.insert("username".to_string(), "john_doe".to_string());
		record.insert("email".to_string(), "john@example.com".to_string());

		// Act
		let page = detail_table(&record);
		let html = page.render_to_string();

		// Assert
		assert!(
			html.contains("username"),
			"key 'username' must appear in output"
		);
		assert!(
			html.contains("john_doe"),
			"value 'john_doe' must appear in output"
		);
		assert!(html.contains("email"), "key 'email' must appear in output");
		assert!(
			html.contains("john@example.com"),
			"value 'john@example.com' must appear in output"
		);
	}
}
