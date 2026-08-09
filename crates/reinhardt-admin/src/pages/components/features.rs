//! Feature-specific components
//!
//! Provides feature-specific UI components:
//! - `Dashboard` - Dashboard view
//! - `ListView` - List view with filters and pagination
//! - `DetailView` - Detail view for a single record
//! - `ModelForm` - Form for creating/editing records
//! - `Filters` - Filter panel
//! - `DataTable` - Data table component

#[cfg(client)]
use crate::server::{create_record, delete_record, update_record};
use crate::types::{FilterInfo, FilterType, InlineEditResponse, ModelInfo};
use reinhardt_pages::Signal;
use reinhardt_pages::component::Page;
use reinhardt_pages::page;
use std::collections::HashMap;

const INLINE_EDIT_FORM_ID: &str = "admin-inline-edit-form";
const ADMIN_PATH_SEGMENT_ENCODE_SET: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
	.add(b' ')
	.add(b'"')
	.add(b'#')
	.add(b'%')
	.add(b'/')
	.add(b'<')
	.add(b'>')
	.add(b'?')
	.add(b'[')
	.add(b'\\')
	.add(b']')
	.add(b'^')
	.add(b'`')
	.add(b'{')
	.add(b'|')
	.add(b'}');

fn reverse_admin_url(route_name: &str, params: &[(&str, &str)]) -> String {
	crate::pages::router::try_with_router(|router| router.reverse(route_name, params))
		.unwrap_or_else(|| crate::pages::router::init_router().reverse(route_name, params))
		.unwrap_or_else(|err| panic!("failed to reverse admin route `{}`: {}", route_name, err))
}

fn admin_model_url(route_name: &str, model_name: &str) -> String {
	let model = model_name.to_lowercase();
	reverse_admin_url(route_name, &[("model", &model)])
}

fn admin_record_url(route_name: &str, model_name: &str, record_id: &str) -> String {
	let model = model_name.to_lowercase();
	let record_id =
		percent_encoding::utf8_percent_encode(record_id, ADMIN_PATH_SEGMENT_ENCODE_SET).to_string();
	reverse_admin_url(route_name, &[("model", &model), ("id", &record_id)])
}

pub(crate) fn decode_admin_path_segment(value: &str) -> String {
	percent_encoding::percent_decode_str(value)
		.decode_utf8_lossy()
		.into_owned()
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

	page!(|site_name: String, grid: Page| {
		div {
			class: "dashboard animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-2xl font-bold text-slate-900 mb-6",
				{ format!("{} Dashboard", site_name) }
			}
			{ grid }
		}
	})(site_name, grid)
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

	page!(|card_views: Vec<Page>| {
		div {
			class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
			{ card_views }
		}
	})(card_views)
}

/// Generates a single model card
fn model_card(name: &str, url: &str) -> Page {
	let name = name.to_string();
	let url = url.to_string();
	let label = format!("View {}", &name);
	let manage_text = format!("Manage {} records", &name);

	page!(|name: String, url: String, label: String, manage_text: String| {
		div {
			class: "admin-card p-5 flex flex-col animate__animated animate__fadeInUp",
			h3 {
				class: "font-display text-lg font-bold text-slate-900 mb-1",
				{ name }
			}
			p {
				class: "text-sm text-slate-500 mb-4 flex-1",
				{ manage_text }
			}
			a {
				class: "admin-btn admin-btn-primary text-center",
				href: url,
				{ label }
			}
		}
	})(name, url, label, manage_text)
}

/// Column definition for list view
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
	/// Column field name
	pub field: String,
	/// Column display label
	pub label: String,
	/// Whether this column is sortable
	pub sortable: bool,
	/// Whether this column can be edited directly in the list.
	pub editable: bool,
	/// Whether this column links to the row detail view.
	pub linked: bool,
	/// Whether an editable value is required.
	pub required: bool,
	/// Input rendering specification for editable cells.
	pub form_spec: Option<crate::types::FormFieldSpec>,
}

/// List view data structure
#[derive(Debug, Clone)]
pub struct ListViewData {
	/// Model name
	pub model_name: String,
	/// Column definitions
	pub columns: Vec<Column>,
	/// Primary key field used for row links and mutations.
	pub pk_field: String,
	/// Record data with the JSON value types returned by the server.
	pub records: Vec<std::collections::HashMap<String, serde_json::Value>>,
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
///         Column {
///             field: "id".to_string(),
///             label: "ID".to_string(),
///             sortable: true,
///             editable: false,
///             linked: true,
///             required: true,
///             form_spec: None,
///         },
///     ],
///     pk_field: "id".to_string(),
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
	render_list_view(data, current_page_signal, filters_signal, None)
}

#[cfg(client)]
pub(crate) fn list_view_with_action(
	data: &ListViewData,
	current_page_signal: reinhardt_pages::Signal<u64>,
	filters_signal: Signal<HashMap<String, String>>,
	save_action: reinhardt_pages::Action<InlineEditResponse, String>,
) -> Page {
	render_list_view(data, current_page_signal, filters_signal, Some(save_action))
}

fn render_list_view(
	data: &ListViewData,
	current_page_signal: reinhardt_pages::Signal<u64>,
	filters_signal: Signal<HashMap<String, String>>,
	save_action: Option<reinhardt_pages::Action<InlineEditResponse, String>>,
) -> Page {
	let title = format!("{} List", data.model_name);
	let summary = format!(
		"Showing {} {} (Page {} of {})",
		data.total_count, data.model_name, data.current_page, data.total_pages
	);
	let filters_page = filters(&data.filters, filters_signal);
	let table_page = data_table(
		&data.columns,
		&data.records,
		&data.model_name,
		&data.pk_field,
		save_action,
	);
	let pagination_page =
		crate::pages::components::common::pagination(current_page_signal, data.total_pages);
	let add_url = admin_model_url("create", &data.model_name);
	let add_label = format!("Add {}", data.model_name);
	let add_link = {
		use reinhardt_pages::component::Component;
		use reinhardt_pages::router::Link;
		Link::new(add_url, add_label)
			.class("admin-btn admin-btn-primary")
			.render()
	};

	page!(|title: String,
	 add_link: Page,
	 filters_page: Page,
	 summary: String,
	 table_page: Page,
	 pagination_page: Page| {
		div {
			class: "list-view animate__animated animate__fadeIn",
			div {
				class: "mb-6 flex items-center justify-between gap-4",
				h1 {
					class: "font-display text-2xl font-bold text-slate-900",
					{ title }
				}
				{ add_link }
			}
			{ filters_page }
			div {
				class: "text-sm text-slate-500 mb-4",
				{ summary }
			}
			{ table_page }
			{ pagination_page }
		}
	})(
		title,
		add_link,
		filters_page,
		summary,
		table_page,
		pagination_page,
	)
}

/// Generates a data table
fn data_table(
	columns: &[Column],
	records: &[std::collections::HashMap<String, serde_json::Value>],
	model_name: &str,
	pk_field: &str,
	save_action: Option<reinhardt_pages::Action<InlineEditResponse, String>>,
) -> Page {
	let header_cells: Vec<Page> = columns
		.iter()
		.map(|col| {
			let label = col.label.clone();
			page!(|label: String| {
				th { { label } }
			})(label)
		})
		.chain(std::iter::once(page!(|| {
			th { "Actions" }
		})()))
		.collect();

	let thead = page!(|header_cells: Vec<Page>| {
		thead {
			tr { { header_cells } }
		}
	})(header_cells);

	let body_rows: Vec<Page> = records
		.iter()
		.map(|record| table_row(columns, record, model_name, pk_field, save_action))
		.collect();

	let tbody = page!(|body_rows: Vec<Page>| {
		tbody { { body_rows } }
	})(body_rows);

	let table = page!(|thead: Page, tbody: Page| {
		div {
			class: "overflow-x-auto rounded-lg border border-slate-200",
			table {
				class: "admin-table",
				{ thead }
				{ tbody }
			}
		}
	})(thead, tbody);

	if save_action.is_none()
		|| !columns
			.iter()
			.any(|column| column.editable && !column.linked && column.form_spec.is_some())
	{
		return table;
	}

	inline_edit_form(
		table,
		save_action.expect("editable tables require a save action"),
	)
}

/// Generates a table row for a single record
fn table_row(
	columns: &[Column],
	record: &std::collections::HashMap<String, serde_json::Value>,
	model_name: &str,
	pk_field: &str,
	save_action: Option<reinhardt_pages::Action<InlineEditResponse, String>>,
) -> Page {
	let record_id = scalar_object_id(record.get(pk_field));
	let row_key = record_id.clone().unwrap_or_default();
	let data_cells: Vec<Page> = columns
		.iter()
		.map(|col| {
			let value = record.get(&col.field).cloned().unwrap_or_default();
			let display = json_value_to_display_string(&value);
			if col.linked
				&& let Some(object_id) = record_id.as_deref()
			{
				let link = {
					use reinhardt_pages::component::Component;
					use reinhardt_pages::router::Link;
					Link::new(admin_record_url("detail", model_name, object_id), display).render()
				};
				return page!(|link: Page| {
					td { { link } }
				})(link);
			}
			if let Some(save_action) = save_action
				&& col.editable
				&& !col.linked
				&& col.form_spec.is_some()
				&& let Some(object_id) = record_id.as_deref()
			{
				return editable_table_cell(col, &value, object_id, save_action);
			}

			page!(|display: String| {
				td { { display } }
			})(display)
		})
		.collect();

	let actions_cell = if let Some(record_id) = record_id {
		let actions = action_buttons(model_name, &record_id);
		let row_error = save_action
			.map(|action| inline_error_page(action, &record_id, None))
			.unwrap_or_else(|| page!(|| { span {} })());
		page!(|actions: Page, row_error: Page| {
			td {
				{ actions }
				{ row_error }
			}
		})(actions, row_error)
	} else {
		page!(|| {
			td { "Unavailable" }
		})()
	};

	page!(|data_cells: Vec<Page>, actions_cell: Page, row_key: String| {
		tr {
			data_row_pk: row_key,
			{ data_cells }
			{ actions_cell }
		}
	})(data_cells, actions_cell, row_key)
}

pub(crate) fn json_value_to_display_string(value: &serde_json::Value) -> String {
	match value {
		serde_json::Value::String(value) => value.clone(),
		serde_json::Value::Number(value) => value.to_string(),
		serde_json::Value::Bool(value) => value.to_string(),
		serde_json::Value::Null => String::new(),
		serde_json::Value::Array(values) => values
			.iter()
			.map(json_value_to_display_string)
			.collect::<Vec<_>>()
			.join(", "),
		serde_json::Value::Object(_) => value.to_string(),
	}
}

fn scalar_object_id(value: Option<&serde_json::Value>) -> Option<String> {
	match value? {
		serde_json::Value::String(value) if !value.is_empty() => Some(value.clone()),
		serde_json::Value::Number(value) => Some(value.to_string()),
		serde_json::Value::Bool(value) => Some(value.to_string()),
		_ => None,
	}
}

fn html_id_segment(value: &str) -> String {
	use std::fmt::Write;

	let mut segment = String::with_capacity(value.len() * 2);
	for byte in value.as_bytes() {
		write!(segment, "{byte:02x}").expect("writing to a String cannot fail");
	}
	segment
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineValueKind {
	String,
	Number,
	Boolean,
	Array,
	Time,
	DateTime,
}

impl InlineValueKind {
	fn as_str(self) -> &'static str {
		match self {
			Self::String => "string",
			Self::Number => "number",
			Self::Boolean => "boolean",
			Self::Array => "array",
			Self::Time => "time",
			Self::DateTime => "datetime",
		}
	}

	#[cfg(client)]
	fn parse(value: &str) -> Option<Self> {
		match value {
			"string" => Some(Self::String),
			"number" => Some(Self::Number),
			"boolean" => Some(Self::Boolean),
			"array" => Some(Self::Array),
			"time" => Some(Self::Time),
			"datetime" => Some(Self::DateTime),
			_ => None,
		}
	}
}

fn inline_value_kind(
	spec: &crate::types::FormFieldSpec,
	value: &serde_json::Value,
) -> InlineValueKind {
	match spec {
		crate::types::FormFieldSpec::Input { html_type } if html_type == "checkbox" => {
			InlineValueKind::Boolean
		}
		crate::types::FormFieldSpec::Input { html_type } if html_type == "number" => {
			InlineValueKind::Number
		}
		crate::types::FormFieldSpec::Input { html_type } if html_type == "time" => {
			InlineValueKind::Time
		}
		crate::types::FormFieldSpec::Input { html_type } if html_type == "datetime-local" => {
			InlineValueKind::DateTime
		}
		crate::types::FormFieldSpec::MultiSelect { .. } => InlineValueKind::Array,
		_ if value.is_number() => InlineValueKind::Number,
		_ if value.is_boolean() => InlineValueKind::Boolean,
		_ if value.is_array() => InlineValueKind::Array,
		_ => InlineValueKind::String,
	}
}

fn normalized_inline_original(
	value: &serde_json::Value,
	kind: InlineValueKind,
) -> serde_json::Value {
	if let Some(value) = value.as_str() {
		let normalized = match kind {
			InlineValueKind::Time => normalized_time_input_value(value),
			InlineValueKind::DateTime => normalized_datetime_input_value(value),
			_ => None,
		};
		if let Some(normalized) = normalized {
			return serde_json::Value::String(normalized);
		}
	}
	if !value.is_null() {
		return value.clone();
	}
	match kind {
		InlineValueKind::String | InlineValueKind::Time | InlineValueKind::DateTime => {
			serde_json::Value::String(String::new())
		}
		InlineValueKind::Number => serde_json::Value::Null,
		InlineValueKind::Boolean => serde_json::Value::Bool(false),
		InlineValueKind::Array => serde_json::Value::Array(Vec::new()),
	}
}

fn normalized_time_input_value(value: &str) -> Option<String> {
	use chrono::Timelike;

	let value = chrono::NaiveTime::parse_from_str(value, "%H:%M:%S%.f")
		.or_else(|_| chrono::NaiveTime::parse_from_str(value, "%H:%M"))
		.ok()?;
	Some(if value.nanosecond() == 0 && value.second() == 0 {
		value.format("%H:%M").to_string()
	} else if value.nanosecond() == 0 {
		value.format("%H:%M:%S").to_string()
	} else {
		value.format("%H:%M:%S%.3f").to_string()
	})
}

fn normalized_datetime_input_value(value: &str) -> Option<String> {
	let value = chrono::DateTime::parse_from_rfc3339(value)
		.map(|value| value.naive_utc())
		.or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f"))
		.or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"))
		.ok()?;
	Some(format!(
		"{}T{}",
		value.date().format("%Y-%m-%d"),
		normalized_time_input_value(&value.time().to_string())?
	))
}

fn editable_table_cell(
	column: &Column,
	value: &serde_json::Value,
	object_id: &str,
	save_action: reinhardt_pages::Action<InlineEditResponse, String>,
) -> Page {
	let input_id = format!(
		"inline-{}-{}",
		html_id_segment(object_id),
		html_id_segment(&column.field)
	);
	let error_id = format!("{}-error", input_id);
	let label = format!("{} for {}", column.label, object_id);
	let spec = column
		.form_spec
		.clone()
		.expect("editable columns require a form specification");
	let value_kind = inline_value_kind(&spec, value);
	let control_value = normalized_inline_original(value, value_kind);
	let field = FormField {
		name: column.field.clone(),
		label: column.label.clone(),
		spec,
		required: column.required,
		value: json_value_to_display_string(&control_value),
	};
	let input = form_element_with_description(&field, &input_id, &label, &error_id);
	let original = control_value.to_string();
	let value_kind = value_kind.as_str().to_string();
	let object_id = object_id.to_string();
	let field_name = column.field.clone();
	let error = inline_error_page(save_action, &object_id, Some(&field_name));

	page!(|input_id: String,
	 error_id: String,
	 label: String,
	 input: Page,
	 original: String,
	 value_kind: String,
	 object_id: String,
	 field_name: String,
	 error: Page| {
		td {
			data_inline_editable: "true",
			data_object_id: object_id,
			data_field: field_name,
			data_original_json: original,
			data_inline_value_kind: value_kind,
			label {
				class: "sr-only",
				for: input_id,
				{ label }
			}
			{ input }
			span {
				id: error_id,
				{ error }
			}
		}
	})(
		input_id, error_id, label, input, original, value_kind, object_id, field_name, error,
	)
}

fn inline_error_page(
	save_action: reinhardt_pages::Action<InlineEditResponse, String>,
	object_id: &str,
	field: Option<&str>,
) -> Page {
	let object_id = object_id.to_string();
	let field = field.map(str::to_string);
	Page::reactive(move || {
		let message = save_action
			.result()
			.and_then(|response| inline_error_message(&response, &object_id, field.as_deref()))
			.unwrap_or_default();
		page!(|message: String| {
			span {
				class: "text-sm text-red-600",
				role: "alert",
				{ message }
			}
		})(message)
	})
}

fn inline_error_message(
	response: &InlineEditResponse,
	object_id: &str,
	field: Option<&str>,
) -> Option<String> {
	response
		.errors
		.iter()
		.find(|error| error.object_id == object_id && error.field.as_deref() == field)
		.map(|error| error.message.clone())
}

fn inline_edit_form(
	table: Page,
	save_action: reinhardt_pages::Action<InlineEditResponse, String>,
) -> Page {
	let save_button = inline_save_button(save_action);
	let global_error = inline_error_page(save_action, "", None);
	let form_id = INLINE_EDIT_FORM_ID.to_string();
	let status = Page::reactive(move || {
		let message = if save_action.is_pending() {
			"Saving changes..."
		} else if save_action.is_error() {
			"Save failed. Your changes are still in the form."
		} else {
			match save_action.result() {
				Some(response) if response.errors.is_empty() => "Changes saved.",
				Some(_) => "Correct the reported changes and save again.",
				None => "Edit fields, then select Save.",
			}
		};
		page!(|message: String| {
			span { { message } }
		})(message.to_string())
	});

	page!(|table: Page,
	 save_action: reinhardt_pages::Action<InlineEditResponse, String>,
	 save_button: Page,
	 status: Page,
	 global_error: Page,
	 form_id: String| {
		form {
			id: form_id,
			method: "post",
			@submit: move |event| {
				event.prevent_default();
				#[cfg(client)]
				crate::pages::components::features::submit_inline_edit_form(event, save_action);
			},
			{ table }
			div {
				class: "mt-4 flex items-center gap-3",
				{ save_button }
				span {
					class: "text-sm text-slate-600",
					role: "status",
					aria_live: "polite",
					{ status }
				}
				{ global_error }
			}
		}
	})(
		table,
		save_action,
		save_button,
		status,
		global_error,
		form_id,
	)
}

fn inline_save_button(save_action: reinhardt_pages::Action<InlineEditResponse, String>) -> Page {
	use reinhardt_pages::component::{IntoPage, PageElement};

	let button = PageElement::new("button")
		.attr("class", "admin-btn admin-btn-primary")
		.attr("type", "submit")
		.child(Page::text("Save"));
	let busy_action = save_action;
	button
		.reactive_attr("disabled", move || {
			save_action.is_pending().then(|| "disabled".into())
		})
		.reactive_attr("aria-busy", move || {
			busy_action.is_pending().then(|| "true".into())
		})
		.into_page()
}

#[cfg(any(client, test))]
#[derive(Debug, Clone, PartialEq)]
struct InlineControlSnapshot {
	object_id: Option<String>,
	field: Option<String>,
	original: Option<serde_json::Value>,
	current: serde_json::Value,
}

#[cfg(any(client, test))]
fn inline_edit_updates(
	snapshots: impl IntoIterator<Item = InlineControlSnapshot>,
) -> Vec<crate::types::InlineEditMutation> {
	let mut updates = Vec::<crate::types::InlineEditMutation>::new();
	let mut positions = HashMap::<String, usize>::new();
	for snapshot in snapshots {
		let (Some(object_id), Some(field), Some(original)) =
			(snapshot.object_id, snapshot.field, snapshot.original)
		else {
			continue;
		};
		if object_id.is_empty() || field.is_empty() || original == snapshot.current {
			continue;
		}

		let position = *positions.entry(object_id.clone()).or_insert_with(|| {
			updates.push(crate::types::InlineEditMutation {
				object_id,
				changes: HashMap::new(),
			});
			updates.len() - 1
		});
		updates[position].changes.insert(field, snapshot.current);
	}
	updates
}

#[cfg(any(client, test))]
fn inline_edit_request(
	csrf_token: String,
	snapshots: impl IntoIterator<Item = InlineControlSnapshot>,
) -> Option<crate::types::InlineEditRequest> {
	let updates = inline_edit_updates(snapshots);
	(!updates.is_empty()).then_some(crate::types::InlineEditRequest {
		csrf_token,
		updates,
	})
}

#[cfg(client)]
fn submit_inline_edit_form(
	event: reinhardt_pages::event::SubmitEvent,
	save_action: reinhardt_pages::Action<InlineEditResponse, String>,
) {
	if save_action.is_pending() {
		return;
	}
	let Some(request) = inline_edit_request(
		reinhardt_pages::csrf::get_csrf_token().unwrap_or_default(),
		collect_inline_control_snapshots(event.raw()),
	) else {
		save_action.reset();
		return;
	};

	set_inline_edit_controls_disabled(true);
	save_action.dispatch(request);
}

#[cfg(client)]
pub(crate) fn set_inline_edit_controls_disabled(disabled: bool) {
	use wasm_bindgen::JsCast;

	let Some(form) = web_sys::window()
		.and_then(|window| window.document())
		.and_then(|document| document.get_element_by_id(INLINE_EDIT_FORM_ID))
		.and_then(|element| element.dyn_into::<web_sys::HtmlFormElement>().ok())
	else {
		return;
	};
	let elements = form.elements();
	for index in 0..elements.length() {
		let Some(element) = elements.item(index) else {
			continue;
		};
		let tagged = element.parent_element().is_some_and(|parent| {
			parent.get_attribute("data-inline-editable").as_deref() == Some("true")
		});
		if !tagged {
			continue;
		}
		if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
			input.set_disabled(disabled);
		} else if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
			textarea.set_disabled(disabled);
		} else if let Some(select) = element.dyn_ref::<web_sys::HtmlSelectElement>() {
			select.set_disabled(disabled);
		}
	}
}

#[cfg(client)]
fn collect_inline_control_snapshots(event: &web_sys::Event) -> Vec<InlineControlSnapshot> {
	use wasm_bindgen::JsCast;

	let target = event.target().or_else(|| event.current_target());
	let Some(form) = target.and_then(|target| target.dyn_into::<web_sys::HtmlFormElement>().ok())
	else {
		return Vec::new();
	};

	let elements = form.elements();
	(0..elements.length())
		.filter_map(|index| {
			let element = elements.item(index)?;
			let cell = element.parent_element()?;
			if cell.get_attribute("data-inline-editable").as_deref() != Some("true") {
				return None;
			}
			let value_kind = cell
				.get_attribute("data-inline-value-kind")
				.and_then(|value| InlineValueKind::parse(&value))?;
			Some(InlineControlSnapshot {
				object_id: cell.get_attribute("data-object-id"),
				field: cell.get_attribute("data-field"),
				original: cell
					.get_attribute("data-original-json")
					.and_then(|value| serde_json::from_str(&value).ok()),
				current: inline_form_control_value(&element, value_kind)?,
			})
		})
		.collect()
}

#[cfg(client)]
fn inline_form_control_value(
	element: &web_sys::Element,
	kind: InlineValueKind,
) -> Option<serde_json::Value> {
	use wasm_bindgen::JsCast;

	if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
		return Some(if kind == InlineValueKind::Boolean {
			serde_json::Value::Bool(input.checked())
		} else {
			inline_scalar_value(&input.value(), kind)
		});
	}
	if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
		return Some(inline_scalar_value(&textarea.value(), kind));
	}
	let select = element.dyn_ref::<web_sys::HtmlSelectElement>()?;
	if kind != InlineValueKind::Array {
		return Some(inline_scalar_value(&select.value(), kind));
	}
	let options = select.options();
	Some(serde_json::Value::Array(
		(0..options.length())
			.filter_map(|index| {
				let option = options
					.item(index)?
					.dyn_into::<web_sys::HtmlOptionElement>()
					.ok()?;
				option
					.selected()
					.then(|| serde_json::Value::String(option.value()))
			})
			.collect(),
	))
}

#[cfg(any(client, test))]
fn inline_scalar_value(value: &str, kind: InlineValueKind) -> serde_json::Value {
	if kind != InlineValueKind::Number {
		return serde_json::Value::String(value.to_string());
	}
	if value.trim().is_empty() {
		return serde_json::Value::Null;
	}
	if let Ok(value) = value.parse::<i64>() {
		return serde_json::Value::Number(value.into());
	}
	value
		.parse::<f64>()
		.ok()
		.and_then(serde_json::Number::from_f64)
		.map(serde_json::Value::Number)
		.unwrap_or_else(|| serde_json::Value::String(value.to_string()))
}

/// Generates action buttons for a record
fn action_buttons(model_name: &str, record_id: &str) -> Page {
	use reinhardt_pages::component::Component;
	use reinhardt_pages::router::Link;

	let detail_url = admin_record_url("detail", model_name, record_id);
	let edit_url = admin_record_url("edit", model_name, record_id);

	let view_link = Link::new(detail_url, "View")
		.class("admin-btn admin-btn-outline admin-btn-sm")
		.render();
	let edit_link = Link::new(edit_url, "Edit")
		.class("admin-btn admin-btn-outline admin-btn-sm")
		.render();

	page!(|view_link: Page, edit_link: Page| {
		div {
			class: "flex gap-1",
			{ view_link }
			{ edit_link }
		}
	})(view_link, edit_link)
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

	let edit_url = admin_record_url("edit", model_name, record_id);
	let list_url = admin_model_url("list", model_name);

	let title = format!("{} Detail", model_name);
	let table_page = detail_table(record);
	let edit_link = Link::new(edit_url, "Edit")
		.class("admin-btn admin-btn-primary mr-2")
		.render();
	let back_link = Link::new(list_url.clone(), "Back to List")
		.class("admin-btn admin-btn-secondary")
		.render();
	let delete_model = model_name.to_string();
	let delete_id = record_id.to_string();
	let delete_return_url = list_url.clone();

	page!(|title: String,
	 table_page: Page,
	 edit_link: Page,
	 back_link: Page,
	 delete_model: String,
	 delete_id: String,
	 delete_return_url: String| {
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
				button {
					type: "button",
					class: "admin-btn admin-btn-danger",
					@click: move |_| {
						#[cfg(client)]
						crate::pages::components::features::delete_model_record(
							delete_model.clone(),
							delete_id.clone(),
							delete_return_url.clone(),
						);
					},
					"Delete"
				}
			}
		}
	})(
		title,
		table_page,
		edit_link,
		back_link,
		delete_model,
		delete_id,
		delete_return_url,
	)
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
			page!(|key: String, value: String| {
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
			})(key, value)
		})
		.collect();

	page!(|rows: Vec<Page>| {
		div {
			class: "overflow-x-auto rounded-lg border border-slate-200",
			table {
				class: "admin-table",
				tbody { { rows } }
			}
		}
	})(rows)
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
		admin_record_url("edit", model_name, rid)
	} else {
		admin_model_url("create", model_name)
	};

	let list_url = admin_model_url("list", model_name);

	let form_fields: Vec<Page> = fields.iter().map(form_group).collect();
	let form_groups = page!(|form_fields: Vec<Page>| {
		div {
			class: "admin-card p-6",
			{ form_fields }
		}
	})(form_fields);
	let cancel_link = Link::new(list_url.clone(), "Cancel")
		.class("admin-btn admin-btn-secondary")
		.render();
	let submit_model = model_name.to_string();
	let submit_record_id = record_id.map(str::to_string);
	let submit_return_url = list_url.clone();

	page!(|form_title: String,
	 action_url: String,
	 form_groups: Page,
	 cancel_link: Page,
	 submit_model: String,
	 submit_record_id: Option<String>,
	 submit_return_url: String| {
		div {
			class: "model-form max-w-2xl animate__animated animate__fadeIn",
			h1 {
				class: "font-display text-2xl font-bold text-slate-900 mb-6",
				{ form_title }
			}
			form {
				method: "post",
				action: action_url,
				@submit: move |event| {
					event.prevent_default();
					#[cfg(client)]
					crate::pages::components::features::submit_model_form(
						event,
						submit_model.clone(),
						submit_record_id.clone(),
						submit_return_url.clone(),
					);
				},
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
	})(
		form_title,
		action_url,
		form_groups,
		cancel_link,
		submit_model,
		submit_record_id,
		submit_return_url,
	)
}

#[cfg(client)]
fn submit_model_form(
	event: reinhardt_pages::event::SubmitEvent,
	model_name: String,
	record_id: Option<String>,
	return_url: String,
) {
	let request = collect_mutation_request(event.raw());
	reinhardt_pages::platform::spawn_task(async move {
		let result = if let Some(id) = record_id {
			update_record(model_name, id, request).await
		} else {
			create_record(model_name, request).await
		};

		match result {
			Ok(_) => navigate_or_set_href(&return_url),
			Err(e) => report_admin_error(&format!("Save failed: {}", e)),
		}
	});
}

#[cfg(client)]
fn delete_model_record(model_name: String, record_id: String, return_url: String) {
	let confirmed = web_sys::window()
		.and_then(|w| w.confirm_with_message("Delete this record?").ok())
		.unwrap_or(false);
	if !confirmed {
		return;
	}

	let csrf_token = reinhardt_pages::csrf::get_csrf_token().unwrap_or_default();
	reinhardt_pages::platform::spawn_task(async move {
		match delete_record(model_name, record_id, csrf_token).await {
			Ok(_) => navigate_or_set_href(&return_url),
			Err(e) => report_admin_error(&format!("Delete failed: {}", e)),
		}
	});
}

#[cfg(client)]
fn collect_mutation_request(event: &web_sys::Event) -> crate::types::MutationRequest {
	use wasm_bindgen::JsCast;

	let mut data = HashMap::new();
	let target = event.target().or_else(|| event.current_target());
	if let Some(target) = target
		&& let Ok(form) = target.dyn_into::<web_sys::HtmlFormElement>()
	{
		let elements = form.elements();
		for index in 0..elements.length() {
			let Some(element) = elements.item(index) else {
				continue;
			};
			collect_form_control_value(&element, &mut data);
		}
	}

	crate::types::MutationRequest {
		csrf_token: reinhardt_pages::csrf::get_csrf_token().unwrap_or_default(),
		data,
	}
}

#[cfg(client)]
fn collect_form_control_value(
	element: &web_sys::Element,
	data: &mut HashMap<String, serde_json::Value>,
) {
	if let Some((name, value)) = form_control_name_value(element) {
		data.insert(name, value);
	}
}

#[cfg(client)]
fn form_control_name_value(element: &web_sys::Element) -> Option<(String, serde_json::Value)> {
	use wasm_bindgen::JsCast;

	if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
		let name = input.name();
		if name.is_empty() {
			return None;
		}
		let value = if input.type_() == "checkbox" {
			serde_json::Value::Bool(input.checked())
		} else {
			form_value_to_json(&name, &input.value(), input.type_() == "number")
		};
		return Some((name, value));
	}

	if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
		let name = textarea.name();
		return (!name.is_empty()).then(|| (name, serde_json::Value::String(textarea.value())));
	}

	if let Some(select) = element.dyn_ref::<web_sys::HtmlSelectElement>() {
		let name = select.name();
		return (!name.is_empty()).then(|| {
			let value = select_value_to_json(select, &name);
			(name, value)
		});
	}

	None
}

#[cfg(client)]
fn select_value_to_json(select: &web_sys::HtmlSelectElement, name: &str) -> serde_json::Value {
	use wasm_bindgen::JsCast;

	if !select.multiple() {
		return form_value_to_json(name, &select.value(), false);
	}

	let options = select.options();
	let values: Vec<String> = (0..options.length())
		.filter_map(|index| {
			let option = options.item(index)?;
			let option = option.dyn_into::<web_sys::HtmlOptionElement>().ok()?;
			option.selected().then(|| option.value())
		})
		.collect();

	form_values_to_json_array(name, &values)
}

#[cfg(any(client, test))]
fn form_values_to_json_array(name: &str, values: &[String]) -> serde_json::Value {
	serde_json::Value::Array(
		values
			.iter()
			.map(|value| form_value_to_json(name, value, false))
			.collect(),
	)
}

#[cfg(any(client, test))]
fn form_value_to_json(name: &str, value: &str, prefer_number: bool) -> serde_json::Value {
	if prefer_number || name.ends_with("_id") {
		if value.trim().is_empty() {
			return serde_json::Value::Null;
		}
		if let Ok(value) = value.parse::<i64>() {
			return serde_json::Value::Number(value.into());
		}
		if let Ok(value) = value.parse::<f64>()
			&& let Some(number) = serde_json::Number::from_f64(value)
		{
			return serde_json::Value::Number(number);
		}
	}
	serde_json::Value::String(value.to_string())
}

#[cfg(client)]
fn navigate_or_set_href(url: &str) {
	if reinhardt_pages::navigate(url.to_string(), reinhardt_pages::NavigationType::Push).is_err()
		&& let Some(window) = web_sys::window()
	{
		let _ = window.location().set_href(url);
	}
}

#[cfg(client)]
fn report_admin_error(message: &str) {
	web_sys::console::error_1(&message.into());
	if let Some(window) = web_sys::window() {
		let _ = window.alert_with_message(message);
	}
}

/// Generates a form group (label + input) for a field
fn form_group(field: &FormField) -> Page {
	let input_id = format!("field-{}", field.name);
	let label = field.label.clone();
	let input = form_element(field, &input_id, &label);

	page!(|input_id: String, label: String, input: Page| {
		div {
			class: "mb-4",
			label {
				for: input_id,
				class: "admin-label",
				{ label }
			}
			{ input }
		}
	})(input_id, label, input)
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
				page!(|value: String, label: String| {
					option {
						value: value,
						selected: true,
						{ label }
					}
				})(value, label)
			} else {
				page!(|value: String, label: String| {
					option {
						value: value,
						{ label }
					}
				})(value, label)
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
fn form_element(field: &FormField, input_id: &str, label: &str) -> Page {
	form_element_with_description(field, input_id, label, "")
}

fn form_element_with_description(
	field: &FormField,
	input_id: &str,
	label: &str,
	described_by: &str,
) -> Page {
	use crate::types::FormFieldSpec;

	let input_id = input_id.to_string();
	let name = field.name.clone();
	let label = label.to_string();
	let described_by = described_by.to_string();
	let value = field.value.clone();
	let required = field.required;

	match &field.spec {
		FormFieldSpec::Input { html_type } => render_input(
			html_type.clone(),
			input_id,
			name,
			label,
			described_by,
			value,
			required,
		),
		FormFieldSpec::File => render_input(
			"file".to_string(),
			input_id,
			name,
			label,
			described_by,
			value,
			required,
		),
		FormFieldSpec::Hidden => render_input(
			"hidden".to_string(),
			input_id,
			name,
			label,
			described_by,
			value,
			required,
		),
		FormFieldSpec::TextArea => {
			if required {
				page!(|input_id: String, name: String, label: String, described_by: String, value: String| {
					textarea {
						class: "admin-input",
						id: input_id,
						name: name,
						aria_label: label,
						aria_describedby: described_by,
						required: true,
						autocomplete: "off",
						{ value }
					}
				})(input_id, name, label, described_by, value)
			} else {
				page!(|input_id: String, name: String, label: String, described_by: String, value: String| {
					textarea {
						class: "admin-input",
						id: input_id,
						name: name,
						aria_label: label,
						aria_describedby: described_by,
						autocomplete: "off",
						{ value }
					}
				})(input_id, name, label, described_by, value)
			}
		}
		FormFieldSpec::Select { choices } => {
			let mut choices = choices.clone();
			if !required {
				choices.insert(0, (String::new(), "---------".to_string()));
			}
			let options = render_option_elements(&choices, &[value.as_str()]);
			if required {
				page!(|input_id: String,
				 name: String,
				 label: String,
				 described_by: String,
				 options: Vec<Page>| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						aria_label: label,
						aria_describedby: described_by,
						required: true,
						{ options }
					}
				})(input_id, name, label, described_by, options)
			} else {
				page!(|input_id: String,
				 name: String,
				 label: String,
				 described_by: String,
				 options: Vec<Page>| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						aria_label: label,
						aria_describedby: described_by,
						{ options }
					}
				})(input_id, name, label, described_by, options)
			}
		}
		FormFieldSpec::MultiSelect { choices } => {
			let selected = parse_multi_value(&value);
			let options = render_option_elements(choices, &selected);
			if required {
				page!(|input_id: String,
				 name: String,
				 label: String,
				 described_by: String,
				 options: Vec<Page>| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						aria_label: label,
						aria_describedby: described_by,
						multiple: true,
						required: true,
						{ options }
					}
				})(input_id, name, label, described_by, options)
			} else {
				page!(|input_id: String,
				 name: String,
				 label: String,
				 described_by: String,
				 options: Vec<Page>| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						aria_label: label,
						aria_describedby: described_by,
						multiple: true,
						{ options }
					}
				})(input_id, name, label, described_by, options)
			}
		}
	}
}

/// Render an `<input>` element with the given HTML `type`.
fn render_input(
	html_type: String,
	input_id: String,
	name: String,
	label: String,
	described_by: String,
	value: String,
	required: bool,
) -> Page {
	if html_type == "checkbox" {
		let checked = value == "true";
		return page!(|html_type: String,
		 input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 checked: bool| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				aria_label: label,
				aria_describedby: described_by,
				checked: checked,
				autocomplete: "off",
			}
		})(html_type, input_id, name, label, described_by, checked);
	}
	if matches!(html_type.as_str(), "time" | "datetime-local") {
		return page!(|html_type: String,
		 input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 value: String,
		 step: String,
		 required: bool| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				aria_label: label,
				aria_describedby: described_by,
				value: value,
				step: step,
				required: required,
				autocomplete: "off",
			}
		})(
			html_type,
			input_id,
			name,
			label,
			described_by,
			value,
			"any".to_string(),
			required,
		);
	}

	if required {
		page!(|html_type: String,
		 input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 value: String| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				aria_label: label,
				aria_describedby: described_by,
				value: value,
				required: true,
				autocomplete: "off",
			}
		})(html_type, input_id, name, label, described_by, value)
	} else {
		page!(|html_type: String,
		 input_id: String,
		 name: String,
		 label: String,
		 described_by: String,
		 value: String| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				aria_label: label,
				aria_describedby: described_by,
				value: value,
				autocomplete: "off",
			}
		})(html_type, input_id, name, label, described_by, value)
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
	label: &str,
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
				page!(|value: String, label: String| {
					option {
						value: value,
						selected: true,
						{ label }
					}
				})(value, label)
			} else {
				page!(|value: String, label: String| {
					option {
						value: value,
						{ label }
					}
				})(value, label)
			}
		})
		.collect();
	let options_container = page!(|options: Vec<Page>| {
		span { { options } }
	})(options);
	let field_str = field.to_string();
	let label = label.to_string();

	page!(|field_str: String,
	 label: String,
	 _filters_signal: Signal<HashMap<String, String>>,
	 options_container: Page| {
		select {
			class: "admin-select",
			aria_label: label,
			data_filter_field: field_str.clone(),
			@change: move |event| {
				if let Ok(value) = event.value() {
					let field = field_str.clone();
					_filters_signal.update(move |map| {
						if value.is_empty() {
							map.remove(&field);
						} else {
							map.insert(field, value);
						}
					});
				}
			},
			{ options_container }
		}
	})(field_str, label, filters_signal, options_container)
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
		&label,
		&filter_info.filter_type,
		current_value,
		filters_signal,
	);

	page!(|label: String, select: Page| {
		div {
			class: "min-w-48",
			label {
				class: "admin-label",
				{ label }
			}
			{ select }
		}
	})(label, select)
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
			create_filter_control(info, current_value, filters_signal)
		})
		.collect();

	let filter_controls = page!(|filter_controls: Vec<Page>| {
		div {
			class: "flex flex-wrap gap-4",
			{ filter_controls }
		}
	})(filter_controls);

	page!(|filter_controls: Page| {
		div {
			class: "admin-card p-4 mb-4",
			h5 {
				class: "text-xs font-semibold uppercase tracking-wider text-slate-500 mb-3",
				"Filters"
			}
			{ filter_controls }
		}
	})(filter_controls)
}

#[cfg(all(test, server))]
mod tests {
	use super::{
		Column, FormField, InlineControlSnapshot, InlineValueKind, admin_record_url, data_table,
		decode_admin_path_segment, detail_table, form_element_with_description, form_value_to_json,
		form_values_to_json_array, html_id_segment, inline_edit_request, inline_edit_updates,
		inline_error_message, inline_scalar_value, inline_value_kind, normalized_inline_original,
		scalar_object_id,
	};
	use crate::types::{FormFieldSpec, InlineEditError, InlineEditResponse};
	use reinhardt_core::reactive::ReactiveScope;
	use rstest::rstest;
	use serde_json::json;
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

	#[rstest]
	fn test_form_value_to_json_converts_id_values() {
		assert_eq!(form_value_to_json("owner_id", "42", false), json!(42));
		assert_eq!(
			form_value_to_json("owner_id", "", false),
			serde_json::Value::Null
		);
		assert_eq!(form_value_to_json("title", "42", false), json!("42"));
	}

	#[rstest]
	fn test_form_values_to_json_array_preserves_all_values() {
		let values = vec![
			"read".to_string(),
			"write".to_string(),
			"delete".to_string(),
		];

		assert_eq!(
			form_values_to_json_array("permissions", &values),
			json!(["read", "write", "delete"])
		);
	}

	#[rstest]
	#[case(json!("user-7"), Some("user-7"))]
	#[case(json!(42), Some("42"))]
	#[case(json!(true), Some("true"))]
	#[case(serde_json::Value::Null, None)]
	#[case(json!([42]), None)]
	#[case(json!({ "id": 42 }), None)]
	fn scalar_object_id_accepts_only_stable_scalars(
		#[case] value: serde_json::Value,
		#[case] expected: Option<&str>,
	) {
		assert_eq!(scalar_object_id(Some(&value)).as_deref(), expected);
	}

	#[rstest]
	fn inline_edit_updates_groups_dirty_fields_and_ignores_untagged_controls() {
		// Arrange
		let snapshots = vec![
			InlineControlSnapshot {
				object_id: Some("user-7".to_string()),
				field: Some("active".to_string()),
				original: Some(json!(false)),
				current: json!(true),
			},
			InlineControlSnapshot {
				object_id: Some("user-7".to_string()),
				field: Some("name".to_string()),
				original: Some(json!("Alice")),
				current: json!("Alice"),
			},
			InlineControlSnapshot {
				object_id: Some("user-8".to_string()),
				field: Some("name".to_string()),
				original: Some(json!("Bob")),
				current: json!("Robert"),
			},
			InlineControlSnapshot {
				object_id: None,
				field: Some("selected_action".to_string()),
				original: None,
				current: json!(true),
			},
		];

		// Act
		let updates = inline_edit_updates(snapshots);

		// Assert
		assert_eq!(updates.len(), 2);
		assert_eq!(updates[0].object_id, "user-7");
		assert_eq!(
			updates[0].changes,
			HashMap::from([("active".to_string(), json!(true))])
		);
		assert_eq!(updates[1].object_id, "user-8");
		assert_eq!(
			updates[1].changes,
			HashMap::from([("name".to_string(), json!("Robert"))])
		);
	}

	#[rstest]
	fn dirty_controls_build_one_batch_request() {
		// Arrange
		let snapshots = [
			InlineControlSnapshot {
				object_id: Some("user-7".to_string()),
				field: Some("name".to_string()),
				original: Some(json!("Alice")),
				current: json!("Alicia"),
			},
			InlineControlSnapshot {
				object_id: Some("user-8".to_string()),
				field: Some("name".to_string()),
				original: Some(json!("Bob")),
				current: json!("Robert"),
			},
		];

		// Act
		let request = inline_edit_request("csrf".to_string(), snapshots)
			.expect("dirty controls should create a request");

		// Assert
		assert_eq!(request.csrf_token, "csrf");
		assert_eq!(request.updates.len(), 2);
	}

	#[rstest]
	fn inline_errors_are_matched_by_typed_object_and_field() {
		// Arrange
		let response = InlineEditResponse {
			updated: 0,
			outcomes: vec![],
			errors: vec![InlineEditError {
				object_id: "user-7".to_string(),
				field: Some("name".to_string()),
				message: "Name is required".to_string(),
			}],
		};

		// Act
		let matching = inline_error_message(&response, "user-7", Some("name"));
		let other_row = inline_error_message(&response, "user-8", Some("name"));
		let global = inline_error_message(
			&InlineEditResponse {
				updated: 0,
				outcomes: vec![],
				errors: vec![InlineEditError {
					object_id: String::new(),
					field: None,
					message: "Payload too large".to_string(),
				}],
			},
			"",
			None,
		);

		// Assert
		assert_eq!(matching.as_deref(), Some("Name is required"));
		assert_eq!(other_row, None);
		assert_eq!(global.as_deref(), Some("Payload too large"));
	}

	#[rstest]
	fn inline_value_normalization_preserves_noop_values() {
		// Arrange
		let text = FormFieldSpec::Input {
			html_type: "text".to_string(),
		};
		let number = FormFieldSpec::Input {
			html_type: "number".to_string(),
		};
		let checkbox = FormFieldSpec::Input {
			html_type: "checkbox".to_string(),
		};
		let time = FormFieldSpec::Input {
			html_type: "time".to_string(),
		};
		let datetime = FormFieldSpec::Input {
			html_type: "datetime-local".to_string(),
		};

		// Act
		let text_kind = inline_value_kind(&text, &serde_json::Value::Null);
		let number_kind = inline_value_kind(&number, &serde_json::Value::Null);
		let checkbox_kind = inline_value_kind(&checkbox, &serde_json::Value::Null);
		let time_kind = inline_value_kind(&time, &json!("09:08:00"));
		let datetime_kind = inline_value_kind(&datetime, &json!("2026-08-10T09:08:07+09:00"));
		let updates = inline_edit_updates([
			InlineControlSnapshot {
				object_id: Some("1".to_string()),
				field: Some("nickname".to_string()),
				original: Some(normalized_inline_original(
					&serde_json::Value::Null,
					text_kind,
				)),
				current: json!(""),
			},
			InlineControlSnapshot {
				object_id: Some("1".to_string()),
				field: Some("score".to_string()),
				original: Some(normalized_inline_original(
					&serde_json::Value::Null,
					number_kind,
				)),
				current: serde_json::Value::Null,
			},
			InlineControlSnapshot {
				object_id: Some("1".to_string()),
				field: Some("active".to_string()),
				original: Some(normalized_inline_original(
					&serde_json::Value::Null,
					checkbox_kind,
				)),
				current: json!(false),
			},
			InlineControlSnapshot {
				object_id: Some("1".to_string()),
				field: Some("reminder_at".to_string()),
				original: Some(normalized_inline_original(
					&serde_json::Value::Null,
					time_kind,
				)),
				current: json!(""),
			},
		]);

		// Assert
		assert_eq!(text_kind, InlineValueKind::String);
		assert_eq!(number_kind, InlineValueKind::Number);
		assert_eq!(checkbox_kind, InlineValueKind::Boolean);
		assert_eq!(time_kind, InlineValueKind::Time);
		assert_eq!(datetime_kind, InlineValueKind::DateTime);
		assert_eq!(
			normalized_inline_original(&json!("09:08:00"), time_kind),
			json!("09:08")
		);
		assert_eq!(
			normalized_inline_original(&json!("09:08:07.123456"), time_kind),
			json!("09:08:07.123")
		);
		assert_eq!(
			normalized_inline_original(&json!("2026-08-10T09:08:07+09:00"), datetime_kind,),
			json!("2026-08-10T00:08:07")
		);
		assert_eq!(
			inline_scalar_value("42", InlineValueKind::String),
			json!("42")
		);
		assert_eq!(
			inline_scalar_value("42", InlineValueKind::Number),
			json!(42)
		);
		assert!(updates.is_empty());
	}

	#[rstest]
	fn temporal_inputs_allow_seconds() {
		let field = FormField {
			name: "starts_at".to_string(),
			label: "Starts at".to_string(),
			spec: FormFieldSpec::Input {
				html_type: "datetime-local".to_string(),
			},
			required: false,
			value: normalized_inline_original(
				&json!("2026-08-10T09:08:07.123456Z"),
				InlineValueKind::DateTime,
			)
			.as_str()
			.expect("normalized date-time is a string")
			.to_string(),
		};

		let html =
			form_element_with_description(&field, "starts-at", "Starts at", "starts-at-error")
				.render_to_string();

		assert!(html.contains(r#"type="datetime-local""#));
		assert!(html.contains(r#"step="any""#));
		assert!(html.contains(r#"value="2026-08-10T09:08:07.123""#));
	}

	#[rstest]
	fn inline_ids_are_injective_and_record_urls_round_trip() {
		// Arrange
		let first = "a/b";
		let second = "a_2f_b";

		// Act
		let first_html_id = html_id_segment(first);
		let second_html_id = html_id_segment(second);
		let url = admin_record_url("detail", "User", first);
		let encoded = url
			.trim_end_matches('/')
			.rsplit('/')
			.next()
			.expect("record URL contains an ID segment");

		// Assert
		assert_ne!(first_html_id, second_html_id);
		assert_eq!(decode_admin_path_segment(encoded), first);
		assert!(url.contains("a%2Fb"));
	}

	#[rstest]
	fn nullable_select_renders_an_explicit_empty_option() {
		// Arrange
		let field = FormField {
			name: "status".to_string(),
			label: "Status".to_string(),
			spec: FormFieldSpec::Select {
				choices: vec![("active".to_string(), "Active".to_string())],
			},
			required: false,
			value: String::new(),
		};

		// Act
		let html = form_element_with_description(&field, "status", "Status", "status-error")
			.render_to_string();

		// Assert
		assert!(html.contains("---------"));
		assert!(html.contains(r#"value="""#));
		assert!(html.contains("selected"));
	}

	#[rstest]
	fn data_table_renders_only_configured_editable_cells() {
		// Arrange
		let columns = vec![
			Column {
				field: "slug".to_string(),
				label: "Slug".to_string(),
				sortable: true,
				editable: true,
				linked: true,
				required: true,
				form_spec: Some(FormFieldSpec::Input {
					html_type: "text".to_string(),
				}),
			},
			Column {
				field: "active".to_string(),
				label: "Active".to_string(),
				sortable: false,
				editable: true,
				linked: false,
				required: true,
				form_spec: Some(FormFieldSpec::Input {
					html_type: "checkbox".to_string(),
				}),
			},
			Column {
				field: "created_at".to_string(),
				label: "Created".to_string(),
				sortable: true,
				editable: false,
				linked: false,
				required: false,
				form_spec: None,
			},
		];
		let records = vec![HashMap::from([
			("slug".to_string(), json!("alice")),
			("active".to_string(), json!(true)),
			("created_at".to_string(), json!("2026-08-10")),
		])];

		// Act
		let html = ReactiveScope::run(|| {
			let action = reinhardt_pages::use_action(|_: crate::types::InlineEditRequest| async {
				Ok::<InlineEditResponse, String>(InlineEditResponse {
					updated: 0,
					outcomes: vec![],
					errors: vec![],
				})
			});
			data_table(&columns, &records, "User", "slug", Some(action)).render_to_string()
		});

		// Assert
		assert_eq!(html.matches("data-inline-editable").count(), 1);
		assert!(html.contains(r#"type="checkbox""#));
		assert!(html.contains("checked"));
		assert!(!html.contains("required"));
		assert!(html.contains(r#"data-row-pk="alice""#));
		assert!(html.contains("/admin/user/alice/"));
		assert!(html.contains("2026-08-10"));
		assert!(!html.contains("/admin/user/0/"));
	}

	#[rstest]
	fn data_table_without_save_action_remains_read_only() {
		// Arrange
		let columns = vec![Column {
			field: "name".to_string(),
			label: "Name".to_string(),
			sortable: true,
			editable: true,
			linked: false,
			required: true,
			form_spec: Some(FormFieldSpec::Input {
				html_type: "text".to_string(),
			}),
		}];
		let records = vec![HashMap::from([
			("id".to_string(), json!(1)),
			("name".to_string(), json!("Alice")),
		])];

		// Act
		let html = ReactiveScope::run(|| {
			data_table(&columns, &records, "User", "id", None).render_to_string()
		});

		// Assert
		assert_eq!(html.matches("data-inline-editable").count(), 0);
		assert!(!html.contains(r#"<form"#));
		assert!(!html.contains(r#"<input"#));
		assert!(html.contains("Alice"));
	}

	#[rstest]
	fn invalid_primary_key_disables_row_links_and_inline_controls() {
		// Arrange
		let columns = vec![Column {
			field: "name".to_string(),
			label: "Name".to_string(),
			sortable: true,
			editable: true,
			linked: false,
			required: true,
			form_spec: Some(FormFieldSpec::Input {
				html_type: "text".to_string(),
			}),
		}];
		let records = vec![HashMap::from([
			("uuid".to_string(), serde_json::Value::Null),
			("name".to_string(), json!("Alice")),
		])];

		// Act
		let html = ReactiveScope::run(|| {
			data_table(&columns, &records, "User", "uuid", None).render_to_string()
		});

		// Assert
		assert_eq!(html.matches("data-inline-editable").count(), 0);
		assert!(!html.contains("/admin/user/0/"));
	}

	#[rstest]
	fn read_only_table_has_no_save_control() {
		// Arrange
		let columns = vec![Column {
			field: "name".to_string(),
			label: "Name".to_string(),
			sortable: true,
			editable: false,
			linked: false,
			required: false,
			form_spec: None,
		}];
		let records = vec![HashMap::from([
			("id".to_string(), json!(1)),
			("name".to_string(), json!("Alice")),
		])];

		// Act
		let html = ReactiveScope::run(|| {
			data_table(&columns, &records, "User", "id", None).render_to_string()
		});

		// Assert
		assert!(!html.contains(">Save<"));
		assert_eq!(html.matches("data-inline-editable").count(), 0);
	}
}
