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
use reinhardt_pages::component::Page;
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

	page!(|| {
		div {
			class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
			{ card_views }
		}
	})()
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
	let header_cells: Vec<Page> = columns
		.iter()
		.map(|col| {
			let label = col.label.clone();
			page!(|| {
				th {
					{ label }
				}
			})()
		})
		.chain(std::iter::once(page!(|| {
			th {
				"Actions"
			}
		})()))
		.collect();

	let thead = page!(|| {
		thead {
			tr {
				{ header_cells }
			}
		}
	})();

	let body_rows: Vec<Page> = records
		.iter()
		.map(|record| table_row(columns, record, model_name))
		.collect();

	let tbody = page!(|| {
		tbody {
			{ body_rows }
		}
	})();

	page!(|| {
		div {
			class: "overflow-x-auto rounded-lg border border-slate-200",
			table {
				class: "admin-table",
				{ thead }
				{ tbody }
			}
		}
	})()
}

/// Generates a table row for a single record
fn table_row(
	columns: &[Column],
	record: &std::collections::HashMap<String, String>,
	model_name: &str,
) -> Page {
	let data_cells: Vec<Page> = columns
		.iter()
		.map(|col| {
			let value = record
				.get(&col.field)
				.cloned()
				.unwrap_or_else(|| "-".to_string());
			page!(|| {
				td {
					{ value }
				}
			})()
		})
		.collect();

	let record_id = record.get("id").cloned().unwrap_or_else(|| "0".to_string());
	let actions = action_buttons(model_name, &record_id);
	let actions_cell = page!(|| {
		td {
			{ actions }
		}
	})();

	page!(|| {
		tr {
			{ data_cells }
			{ actions_cell }
		}
	})()
}

/// Generates action buttons for a record
fn action_buttons(model_name: &str, record_id: &str) -> Page {
	use reinhardt_pages::component::Component;
	use reinhardt_pages::router::Link;

	let encoded_model = encode_path_segment(&model_name.to_lowercase());
	let encoded_id = encode_path_segment(record_id);
	let detail_url = format!("/admin/{}/{}/", encoded_model, encoded_id);
	let edit_url = format!("/admin/{}/{}/change/", encoded_model, encoded_id);

	let view_link = Link::new(detail_url, "View")
		.class("admin-btn admin-btn-outline admin-btn-sm")
		.render();
	let edit_link = Link::new(edit_url, "Edit")
		.class("admin-btn admin-btn-outline admin-btn-sm")
		.render();

	page!(|| {
		div {
			class: "flex gap-1",
			{ view_link }
			{ edit_link }
		}
	})()
}

/// Form field definition for model forms
#[derive(Debug, Clone)]
pub struct FormField {
	/// Field name (corresponds to database column)
	pub name: String,
	/// Field display label
	pub label: String,
	/// Rendering specification (input type, textarea, select, etc.)
	pub spec: crate::types::FormFieldSpec,
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
			let key = key.clone();
			let value = value.clone();
			page!(|| {
				tr {
					th {
						class: "w-1/4 text-left text-sm font-medium text-slate-500 py-3 px-4 bg-slate-50",
						{ key }
					}
					td {
						class: "text-sm text-slate-800 py-3 px-4",
						{ value }
					}
				}
			})()
		})
		.collect();

	page!(|| {
		div {
			class: "overflow-x-auto rounded-lg border border-slate-200",
			table {
				class: "admin-table",
				tbody {
					{ rows }
				}
			}
		}
	})()
}

/// Model form component
///
/// Displays a form for creating or editing a record.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::pages::components::features::{model_form, FormField};
/// use reinhardt_admin::types::FormFieldSpec;
///
/// let fields = vec![
///     FormField {
///         name: "username".to_string(),
///         label: "Username".to_string(),
///         spec: FormFieldSpec::Input { html_type: "text".to_string() },
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

	let form_fields: Vec<Page> = fields.iter().map(form_group).collect();
	let form_groups = page!(|| {
		div {
			class: "admin-card p-6",
			{ form_fields }
		}
	})();
	let cancel_link = Link::new(list_url, "Cancel")
		.class("admin-btn admin-btn-secondary")
		.render();

	page!(|| {
		div {
			class: "model-form max-w-2xl animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-2xl font-bold text-slate-900 mb-6",
				{ form_title }
			}
			form {
				method: "post",
				action: action_url,
				{ form_groups }
				div {
					class: "mt-6 flex gap-2",
					button {
						class: "admin-btn admin-btn-primary",
						type: "submit",
						"Save"
					}
					{ cancel_link }
				}
			}
		}
	})()
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
				for: input_id,
				class: "admin-label",
				{ label }
			}
			{ input }
		}
	})()
}

/// Render `<option>` elements for a list of `(value, label)` choices,
/// marking each option whose value appears in `selected` as `selected`.
///
/// `selected` is a slice so that both single-select (`[current]`) and
/// multi-select (`split` of the `FormField::value` string) can share the
/// same renderer. See `parse_multi_value` for the multi-select wire format.
fn render_option_elements(choices: &[(String, String)], selected: &[&str]) -> Vec<Page> {
	choices
		.iter()
		.map(|(value, label)| {
			let value = value.clone();
			let label = label.clone();
			let is_selected = selected.iter().any(|s| *s == value);
			if is_selected {
				page!(|| {
					option {
						value: value,
						selected: true,
						{ label }
					}
				})()
			} else {
				page!(|| {
					option {
						value: value,
						{ label }
					}
				})()
			}
		})
		.collect()
}

/// Multi-select wire format: `FormField::value` carries the selected values
/// as a comma-separated list (e.g., `"read,write,delete"`). Empty entries
/// are skipped so an empty value yields no selected options.
fn parse_multi_value(raw: &str) -> Vec<&str> {
	raw.split(',')
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.collect()
}

/// Generates an input element for a form field
fn form_element(field: &FormField, input_id: &str) -> Page {
	use crate::types::FormFieldSpec;

	let input_id = input_id.to_string();
	let name = field.name.clone();
	let value = field.value.clone();
	let required = field.required;

	match &field.spec {
		FormFieldSpec::Input { html_type } => {
			render_input(html_type.clone(), input_id, name, value, required)
		}
		FormFieldSpec::File => render_input("file".to_string(), input_id, name, value, required),
		FormFieldSpec::Hidden => {
			render_input("hidden".to_string(), input_id, name, value, required)
		}
		FormFieldSpec::TextArea => {
			if required {
				page!(|| {
					textarea {
						class: "admin-input",
						id: input_id,
						name: name,
						required: true,
						autocomplete: "off",
						{ value }
					}
				})()
			} else {
				page!(|| {
					textarea {
						class: "admin-input",
						id: input_id,
						name: name,
						autocomplete: "off",
						{ value }
					}
				})()
			}
		}
		FormFieldSpec::Select { choices } => {
			let options = render_option_elements(choices, &[value.as_str()]);
			if required {
				page!(|| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						required: true,
						{ options }
					}
				})()
			} else {
				page!(|| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						{ options }
					}
				})()
			}
		}
		FormFieldSpec::MultiSelect { choices } => {
			let selected = parse_multi_value(&value);
			let options = render_option_elements(choices, &selected);
			if required {
				page!(|| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						multiple: true,
						required: true,
						{ options }
					}
				})()
			} else {
				page!(|| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						multiple: true,
						{ options }
					}
				})()
			}
		}
	}
}

/// Render an `<input>` element with the given HTML `type`.
fn render_input(
	html_type: String,
	input_id: String,
	name: String,
	value: String,
	required: bool,
) -> Page {
	if required {
		page!(|| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				value: value,
				required: true,
				autocomplete: "off",
			}
		})()
	} else {
		page!(|| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				value: value,
				autocomplete: "off",
			}
		})()
	}
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
fn create_filter_select(
	field: &str,
	filter_type: &FilterType,
	current_value: Option<&str>,
	filters_signal: Signal<HashMap<String, String>>,
) -> Page {
	let choices = filter_type_to_choices(filter_type);
	let current_val = current_value.unwrap_or("");

	// Generate <option> elements
	let options: Vec<Page> = choices
		.iter()
		.map(|(value, label)| {
			let value = value.clone();
			let label = label.clone();
			if value == current_val {
				page!(|| {
					option {
						value: value,
						selected: true,
						{ label }
					}
				})()
			} else {
				page!(|| {
					option {
						value: value,
						{ label }
					}
				})()
			}
		})
		.collect();
	let options_container = page!(|| {
		span {
			{ options }
		}
	})();
	let field_str = field.to_string();

	page!(|field_str: String, _filters_signal: Signal<HashMap<String, String>>| {
		select {
			class: "admin-select",
			data_filter_field: field_str.clone(),
			@change: move |event| {
						use wasm_bindgen::JsCast;
						if let Some(target) = event.target() {
							if let Ok(select_el) = target.dyn_into::<web_sys::HtmlSelectElement>() {
								let value = select_el.value();
								let field = field_str.clone();
								_filters_signal.update(move |map| {
									if value.is_empty() {
										map.remove(&field);
									} else {
										map.insert(field, value);
									}
								});
							}
						}
					},
			{ options_container }
		}
	})(field_str, filters_signal)
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
		return page!(|| { div {} })();
	}

	let current_filters = filters_signal.get();

	let filter_controls: Vec<Page> = filters_info
		.iter()
		.map(|info| {
			let current_value = current_filters.get(&info.field).map(|s| s.as_str());
			create_filter_control(info, current_value, filters_signal.clone())
		})
		.collect();

	let filter_controls = page!(|| {
		div {
			class: "flex flex-wrap gap-4",
			{ filter_controls }
		}
	})();

	page!(|| {
		div {
			class: "admin-card p-4 mb-4",
			h5 {
				class: "text-xs font-semibold uppercase tracking-wider text-slate-500 mb-3",
				"Filters"
			}
			{ filter_controls }
		}
	})()
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
