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
#[cfg(client)]
use crate::types::AdminActionRequest;
use crate::types::{
	AdminAction, FieldInfo, Fieldset, FilterInfo, FilterType, InlineFormInfo, InlineRowInfo,
	InlineStyle, ModelInfo, MutationResponse,
};
use reinhardt_pages::component::{IntoPage, Page, PageElement};
use reinhardt_pages::event::{ChangeEvent, EventPayload, typed_event_handler};
use reinhardt_pages::page;
use reinhardt_pages::{Action, EventType, IntoEventHandler, Signal};
use std::collections::{BTreeSet, HashMap};

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
	reverse_admin_url(route_name, &[("model", &model), ("id", record_id)])
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

/// Reactive state used by the internal action-enabled list view.
#[doc(hidden)]
pub type ListActionState = (
	Signal<BTreeSet<String>>,
	Signal<String>,
	Action<MutationResponse, String>,
);

type ListSelectionState = (Signal<BTreeSet<String>>, Action<MutationResponse, String>);

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
	list_view_content(data, "id", &[], current_page_signal, filters_signal, None)
}

#[doc(hidden)]
pub fn list_view_with_actions(
	data: &ListViewData,
	pk_field: &str,
	actions: &[AdminAction],
	current_page_signal: Signal<u64>,
	filters_signal: Signal<HashMap<String, String>>,
	action_state: ListActionState,
) -> Page {
	list_view_content(
		data,
		pk_field,
		actions,
		current_page_signal,
		filters_signal,
		Some(action_state),
	)
}

fn record_primary_key(record: &HashMap<String, String>, pk_field: &str) -> Option<String> {
	record.get(pk_field).cloned()
}

fn set_page_selected(selected: &mut BTreeSet<String>, page_ids: &[String], checked: bool) {
	selected.clear();
	if checked {
		selected.extend(page_ids.iter().cloned());
	}
}

fn set_record_selected(selected: &mut BTreeSet<String>, record_id: &str, checked: bool) {
	if checked {
		selected.insert(record_id.to_string());
	} else {
		selected.remove(record_id);
	}
}

fn find_admin_action<'a>(actions: &'a [AdminAction], name: &str) -> Option<&'a AdminAction> {
	actions.iter().find(|action| action.name == name)
}

fn action_can_dispatch(
	pending: bool,
	action: Option<&AdminAction>,
	selected_ids: &BTreeSet<String>,
) -> bool {
	!pending && action.is_some() && !selected_ids.is_empty()
}

#[cfg(client)]
fn dispatch_selected_admin_action(
	actions: &[AdminAction],
	selected_ids: Signal<BTreeSet<String>>,
	selected_action: Signal<String>,
	action: Action<MutationResponse, String>,
) {
	let selected_action_name = selected_action.get();
	let Some(metadata) = find_admin_action(actions, &selected_action_name) else {
		return;
	};
	let ids = selected_ids.get();
	if !action_can_dispatch(action.is_pending(), Some(metadata), &ids) {
		return;
	}

	if metadata.requires_confirmation {
		let message = format!(
			"Run \"{}\" on {} selected records?",
			metadata.label,
			ids.len()
		);
		let confirmed = web_sys::window()
			.and_then(|window| window.confirm_with_message(&message).ok())
			.unwrap_or(false);
		if !confirmed {
			return;
		}
	}

	action.dispatch(AdminActionRequest::new(
		reinhardt_pages::csrf::get_csrf_token().unwrap_or_default(),
		metadata.name.clone(),
		ids.into_iter().collect(),
	));
}

fn list_action_controls(
	actions: &[AdminAction],
	selected_ids: Signal<BTreeSet<String>>,
	selected_action: Signal<String>,
	action: Action<MutationResponse, String>,
) -> Page {
	let placeholder = page!(|selected_action: Signal<String>| {
		option {
			value: "",
			selected: selected_action.get().is_empty(),
			"Choose an action"
		}
	})(selected_action);
	let options: Vec<Page> = actions
		.iter()
		.map(|metadata| {
			let name = metadata.name.clone();
			let name_for_selected = name.clone();
			let label = metadata.label.clone();
			page!(|name: String,
			 name_for_selected: String,
			 label: String,
			 selected_action: Signal<String>| {
				option {
					value: name,
					selected: selected_action.get() == name_for_selected,
					{ label }
				}
			})(name, name_for_selected, label, selected_action)
		})
		.collect();
	let select_action = action;
	let select_change_action = action;
	let select = PageElement::new("select")
		.attr("class", "admin-select")
		.attr("aria-label", "Admin action")
		.reactive_attr("disabled", move || {
			select_action.is_pending().then(|| "disabled".into())
		})
		.on(
			ChangeEvent::EVENT,
			typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
				if select_change_action.is_pending() {
					return;
				}
				if let Ok(value) = event.value() {
					selected_action.set(value);
				}
			}),
		)
		.child(placeholder)
		.children(options)
		.into_page();

	let actions_for_disabled_attr = actions.to_vec();
	#[cfg(client)]
	let actions_for_dispatch = actions.to_vec();
	let selected_ids_for_disabled = selected_ids;
	let selected_action_for_disabled = selected_action;
	let button_action = action;
	let pending_label = Page::reactive(move || {
		if action.is_pending() {
			Page::text("Running...")
		} else {
			Page::text("Run")
		}
	});
	let on_click = move |_event| {
		#[cfg(client)]
		if !action.is_pending() {
			self::dispatch_selected_admin_action(
				&actions_for_dispatch,
				selected_ids,
				selected_action,
				action,
			);
		}
	};
	let button = PageElement::new("button")
		.attr("type", "button")
		.attr("class", "admin-btn admin-btn-primary")
		.reactive_attr("disabled", move || {
			(!self::action_can_dispatch(
				button_action.is_pending(),
				self::find_admin_action(
					&actions_for_disabled_attr,
					&selected_action_for_disabled.get(),
				),
				&selected_ids_for_disabled.get(),
			))
			.then(|| "disabled".into())
		})
		.on(EventType::Click, on_click.into_event_handler())
		.child(pending_label)
		.into_page();
	let error_message = Page::reactive(move || match action.error() {
		Some(error) => page!(|error: String| {
			p {
				class: "text-sm text-red-700",
				role: "alert",
				{ error }
			}
		})(error),
		None => Page::empty(),
	});

	page!(|select: Page, button: Page, error_message: Page| {
		div {
			class: "admin-card p-4 mb-4 flex flex-wrap items-center gap-3",
			{ select }
			{ button }
			{ error_message }
		}
	})(select, button, error_message)
}

fn list_view_content(
	data: &ListViewData,
	pk_field: &str,
	actions: &[AdminAction],
	current_page_signal: Signal<u64>,
	filters_signal: Signal<HashMap<String, String>>,
	action_state: Option<ListActionState>,
) -> Page {
	let title = format!("{} List", data.model_name);
	let summary = format!(
		"Showing {} {} (Page {} of {})",
		data.total_count, data.model_name, data.current_page, data.total_pages
	);
	let filters_page = filters(&data.filters, filters_signal);
	let action_state = action_state.filter(|_| !actions.is_empty());
	let action_controls = action_state
		.map(|(selected_ids, selected_action, action)| {
			list_action_controls(actions, selected_ids, selected_action, action)
		})
		.into_iter()
		.collect::<Vec<_>>();
	let selection = action_state.map(|(selected_ids, _, action)| (selected_ids, action));
	let table_page = data_table(
		&data.columns,
		&data.records,
		&data.model_name,
		pk_field,
		selection,
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
	 action_controls: Vec<Page>,
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
			{ action_controls }
			{ table_page }
			{ pagination_page }
		}
	})(
		title,
		add_link,
		filters_page,
		action_controls,
		summary,
		table_page,
		pagination_page,
	)
}

/// Generates a data table
fn data_table(
	columns: &[Column],
	records: &[std::collections::HashMap<String, String>],
	model_name: &str,
	pk_field: &str,
	selection: Option<ListSelectionState>,
) -> Page {
	let page_ids = records
		.iter()
		.filter_map(|record| record_primary_key(record, pk_field))
		.collect::<Vec<_>>();
	let selection_header = selection.map(|(selected_ids, action)| {
		let ids_for_handler = page_ids.clone();
		let has_records = !page_ids.is_empty();
		let is_checked = has_records && page_ids.iter().all(|id| selected_ids.get().contains(id));
		let checkbox_action = action;
		let checkbox_change_action = action;
		let checkbox = PageElement::new("input")
			.attr("type", "checkbox")
			.attr("aria-label", "Select current page")
			.bool_attr("checked", is_checked)
			.reactive_attr("disabled", move || {
				(checkbox_action.is_pending() || !has_records).then(|| "disabled".into())
			})
			.on(
				ChangeEvent::EVENT,
				typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
					if checkbox_change_action.is_pending() {
						return;
					}
					if let Ok(checked) = event.checked() {
						selected_ids.update(|selected| {
							self::set_page_selected(selected, &ids_for_handler, checked);
						});
					}
				}),
			)
			.into_page();
		page!(|checkbox: Page| {
			th { { checkbox } }
		})(checkbox)
	});
	let header_cells: Vec<Page> = selection_header
		.into_iter()
		.chain(columns.iter().map(|col| {
			let label = col.label.clone();
			page!(|label: String| {
				th { { label } }
			})(label)
		}))
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
		.map(|record| table_row(columns, record, model_name, pk_field, selection))
		.collect();

	let tbody = page!(|body_rows: Vec<Page>| {
		tbody { { body_rows } }
	})(body_rows);

	page!(|thead: Page, tbody: Page| {
		div {
			class: "overflow-x-auto rounded-lg border border-slate-200",
			table {
				class: "admin-table",
				{ thead }
				{ tbody }
			}
		}
	})(thead, tbody)
}

/// Generates a table row for a single record
fn table_row(
	columns: &[Column],
	record: &std::collections::HashMap<String, String>,
	model_name: &str,
	pk_field: &str,
	selection: Option<ListSelectionState>,
) -> Page {
	let record_id = record_primary_key(record, pk_field);
	let selection_cell = selection.map(|(selected_ids, action)| match record_id.clone() {
		Some(record_id) => {
			let label = format!("Select {}", record_id);
			let is_checked = selected_ids.get().contains(&record_id);
			let id_for_handler = record_id.clone();
			let checkbox_action = action;
			let checkbox_change_action = action;
			let checkbox = PageElement::new("input")
				.attr("type", "checkbox")
				.attr("aria-label", label)
				.attr("value", record_id)
				.bool_attr("checked", is_checked)
				.reactive_attr("disabled", move || {
					checkbox_action.is_pending().then(|| "disabled".into())
				})
				.on(
					ChangeEvent::EVENT,
					typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
						if checkbox_change_action.is_pending() {
							return;
						}
						if let Ok(checked) = event.checked() {
							selected_ids.update(|selected| {
								self::set_record_selected(selected, &id_for_handler, checked);
							});
						}
					}),
				)
				.into_page();
			page!(|checkbox: Page| {
				td { { checkbox } }
			})(checkbox)
		}
		None => page!(|| { td {} })(),
	});
	let data_cells: Vec<Page> = columns
		.iter()
		.map(|col| {
			let value = record
				.get(&col.field)
				.cloned()
				.unwrap_or_else(|| "-".to_string());
			page!(|value: String| {
				td { { value } }
			})(value)
		})
		.collect();

	let actions_cell = record_id.map_or_else(
		|| page!(|| { td {} })(),
		|record_id| {
			let actions = action_buttons(model_name, &record_id);
			page!(|actions: Page| {
				td { { actions } }
			})(actions)
		},
	);
	let selection_cells = selection_cell.into_iter().collect::<Vec<_>>();

	page!(|selection_cells: Vec<Page>, data_cells: Vec<Page>, actions_cell: Page| {
		tr {
			{ selection_cells }
			{ data_cells }
			{ actions_cell }
		}
	})(selection_cells, data_cells, actions_cell)
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
	model_form_with_inlines(model_name, fields, &[], &[], record_id)
}

/// Model form component with configured fieldsets.
///
/// Fields are rendered in fieldset declaration order. Each fieldset uses the
/// browser's native disclosure behavior and is expanded unless configured as
/// collapsed. A collapsed group with an empty required field starts expanded so
/// native form validation keeps the invalid control visible.
pub fn model_form_with_fieldsets(
	model_name: &str,
	fields: &[FormField],
	fieldsets: &[Fieldset],
	record_id: Option<&str>,
) -> Page {
	model_form_with_inlines(model_name, fields, fieldsets, &[], record_id)
}

/// Model form component with optional parent fieldsets and related child rows.
pub fn model_form_with_inlines(
	model_name: &str,
	fields: &[FormField],
	fieldsets: &[Fieldset],
	inlines: &[InlineFormInfo],
	record_id: Option<&str>,
) -> Page {
	let parent_groups = parent_form_groups(fields, fieldsets);
	if inlines.is_empty() {
		return model_form_page(model_name, record_id, parent_groups);
	}

	let mut groups = Vec::with_capacity(inlines.len() + 1);
	groups.push(parent_groups);
	groups.extend(inlines.iter().map(inline_form_section));

	model_form_page(model_name, record_id, Page::Fragment(groups))
}

fn parent_form_groups(fields: &[FormField], fieldsets: &[Fieldset]) -> Page {
	if fieldsets.is_empty() {
		return flat_parent_form_groups(fields);
	}

	fieldset_parent_form_groups(fields, fieldsets)
}

fn flat_parent_form_groups(fields: &[FormField]) -> Page {
	let form_fields: Vec<Page> = fields.iter().map(form_group).collect();
	page!(|form_fields: Vec<Page>| {
		div {
			class: "admin-card p-6",
			{ form_fields }
		}
	})(form_fields)
}

fn fieldset_parent_form_groups(fields: &[FormField], fieldsets: &[Fieldset]) -> Page {
	let fieldsets: Vec<Page> = fieldsets
		.iter()
		.map(|fieldset| {
			let summary = fieldset
				.title
				.as_deref()
				.filter(|title| !title.trim().is_empty())
				.unwrap_or("Fields")
				.to_string();
			let form_fields: Vec<&FormField> = fieldset
				.fields
				.iter()
				.filter_map(|name| fields.iter().find(|field| field.name == *name))
				.collect();
			let open = !fieldset.collapsed
				|| form_fields
					.iter()
					.any(|field| field.required && field.value.is_empty());
			let form_fields: Vec<Page> = form_fields.into_iter().map(form_group).collect();

			page!(|summary: String, open: bool, form_fields: Vec<Page>| {
				details {
					class: "admin-fieldset",
					open: open,
					summary { { summary } }
					{ form_fields }
				}
			})(summary, open, form_fields)
		})
		.collect();
	page!(|fieldsets: Vec<Page>| {
		div {
			class: "admin-card p-6",
			{ fieldsets }
		}
	})(fieldsets)
}

#[derive(Clone, Copy)]
enum InlineFieldLayout {
	Tabular,
	Stacked,
}

fn inline_form_section(inline: &InlineFormInfo) -> Page {
	match inline.style {
		InlineStyle::Tabular => tabular_inline_form(inline),
		InlineStyle::Stacked => stacked_inline_form(inline),
	}
}

fn tabular_inline_form(inline: &InlineFormInfo) -> Page {
	let heading = inline.model_name.clone();
	let caption = format!("{} inline rows", inline.model_name);
	let headers: Vec<Page> = inline
		.fields
		.iter()
		.map(|field| {
			let label = field.label.clone();
			page!(|label: String| {
				th {
					scope: "col",
					{ label }
				}
			})(label)
		})
		.collect();
	let show_delete_column = inline.can_delete && inline.rows.iter().any(|row| row.id.is_some());
	let delete_header = if show_delete_column {
		page!(|| {
			th {
				scope: "col",
				"Delete"
			}
		})()
	} else {
		Page::Empty
	};
	let rows: Vec<Page> = inline
		.rows
		.iter()
		.enumerate()
		.map(|(index, row)| tabular_inline_row(inline, row, index, show_delete_column))
		.collect();

	page!(|heading: String,
	 caption: String,
	 headers: Vec<Page>,
	 delete_header: Page,
	 rows: Vec<Page>| {
		section {
			class: "admin-inline-section",
			h2 {
				class: "admin-inline-heading",
				{ heading }
			}
			div {
				class: "admin-inline-table-wrap",
				table {
					class: "admin-inline-table",
					caption {
						class: "sr-only",
						{ caption }
					}
					thead {
						tr {
							{ headers }
							{ delete_header }
							th {
								class: "sr-only",
								scope: "col",
								"Errors"
							}
						}
					}
					tbody { { rows } }
				}
			}
		}
	})(heading, caption, headers, delete_header, rows)
}

fn tabular_inline_row(
	inline: &InlineFormInfo,
	row: &InlineRowInfo,
	index: usize,
	show_delete_column: bool,
) -> Page {
	let fields = inline_row_fields(inline, row, index, InlineFieldLayout::Tabular);
	let identity = inline_row_identity(inline, row, index);
	let delete_cell = if show_delete_column {
		let delete_control = inline_delete_control(inline, row, index);
		page!(|delete_control: Page| {
			td {
				class: "admin-inline-delete-cell",
				{ delete_control }
			}
		})(delete_control)
	} else {
		Page::Empty
	};
	let row_error = inline_row_error(&inline.key, index);

	page!(|fields: Vec<Page>, identity: Page, delete_cell: Page, row_error: Page| {
		tr {
			{ fields }
			{ delete_cell }
			td {
				class: "admin-inline-error-cell",
				{ identity }
				{ row_error }
			}
		}
	})(fields, identity, delete_cell, row_error)
}

fn stacked_inline_form(inline: &InlineFormInfo) -> Page {
	let heading = inline.model_name.clone();
	let rows: Vec<Page> = inline
		.rows
		.iter()
		.enumerate()
		.map(|(index, row)| stacked_inline_row(inline, row, index))
		.collect();

	page!(|heading: String, rows: Vec<Page>| {
		section {
			class: "admin-inline-section",
			h2 {
				class: "admin-inline-heading",
				{ heading }
			}
			{ rows }
		}
	})(heading, rows)
}

fn stacked_inline_row(inline: &InlineFormInfo, row: &InlineRowInfo, index: usize) -> Page {
	let legend = format!("{} {}", inline.model_name, index + 1);
	let identity = inline_row_identity(inline, row, index);
	let fields = inline_row_fields(inline, row, index, InlineFieldLayout::Stacked);
	let delete_control = inline_delete_control(inline, row, index);
	let row_error = inline_row_error(&inline.key, index);

	page!(|legend: String,
	 identity: Page,
	 fields: Vec<Page>,
	 delete_control: Page,
	 row_error: Page| {
		fieldset {
			class: "admin-inline-stacked-row",
			legend { { legend } }
			{ identity }
			{ fields }
			{ delete_control }
			{ row_error }
		}
	})(legend, identity, fields, delete_control, row_error)
}

fn inline_row_fields(
	inline: &InlineFormInfo,
	row: &InlineRowInfo,
	index: usize,
	layout: InlineFieldLayout,
) -> Vec<Page> {
	inline
		.fields
		.iter()
		.map(|field| {
			let input_id = inline_field_id(&inline.key, index, &field.name);
			let label = field.label.clone();
			let value = inline_field_value(row, field);
			if field.readonly {
				return inline_readonly_field(layout, input_id, label, value);
			}
			let form_field = FormField {
				name: inline_field_name(&inline.key, index, &field.name),
				label: label.clone(),
				spec: crate::types::FormFieldSpec::from(&field.field_type),
				required: row.id.is_some() && field.required,
				value,
			};
			let input = form_element(&form_field, &input_id, &label);

			match layout {
				InlineFieldLayout::Tabular => page!(|input: Page| {
					td { { input } }
				})(input),
				InlineFieldLayout::Stacked => {
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
			}
		})
		.collect()
}

fn inline_readonly_field(
	layout: InlineFieldLayout,
	field_id: String,
	label: String,
	value: String,
) -> Page {
	match layout {
		InlineFieldLayout::Tabular => page!(|field_id: String, value: String| {
			td {
				span {
					class: "admin-inline-readonly",
					id: field_id,
					{ value }
				}
			}
		})(field_id, value),
		InlineFieldLayout::Stacked => page!(|field_id: String, label: String, value: String| {
			div {
				class: "mb-4",
				span {
					class: "admin-label",
					{ label }
				}
				span {
					class: "admin-inline-readonly",
					id: field_id,
					{ value }
				}
			}
		})(field_id, label, value),
	}
}

fn inline_row_identity(inline: &InlineFormInfo, row: &InlineRowInfo, index: usize) -> Page {
	let Some(id) = &row.id else {
		return Page::Empty;
	};
	let name = inline_field_name(&inline.key, index, "__id");
	let id = id.clone();

	page!(|name: String, id: String| {
		input {
			type: "hidden",
			name: name,
			value: id,
		}
	})(name, id)
}

fn inline_delete_control(inline: &InlineFormInfo, row: &InlineRowInfo, index: usize) -> Page {
	if !inline.can_delete || row.id.is_none() {
		return Page::Empty;
	}
	let name = inline_field_name(&inline.key, index, "__delete");
	let label = format!("Delete {} {}", inline.model_name, index + 1);

	page!(|name: String, label: String| {
		label {
			class: "admin-inline-delete",
			input {
				type: "checkbox",
				name: name,
				aria_label: label.clone(),
			}
			{ label }
		}
	})(name, label)
}

fn inline_row_error(key: &str, index: usize) -> Page {
	let error_id = inline_row_error_id(key, index);
	page!(|error_id: String| {
		div {
			class: "admin-inline-row-error",
			id: error_id,
			role: "alert",
			aria_live: "polite",
			data_inline_row_error: "true",
		}
	})(error_id)
}

fn inline_field_name(key: &str, index: usize, field: &str) -> String {
	format!("__reinhardt_inlines.{key}.{index}.{field}")
}

fn inline_field_id(key: &str, index: usize, field: &str) -> String {
	format!("inline-field-{key}-{index}-{field}")
}

fn inline_row_error_id(key: &str, index: usize) -> String {
	format!("inline-error-{key}-{index}")
}

fn inline_field_value(row: &InlineRowInfo, field: &FieldInfo) -> String {
	row.values
		.get(&field.name)
		.map(inline_json_value_to_display_string)
		.unwrap_or_default()
}

fn inline_json_value_to_display_string(value: &serde_json::Value) -> String {
	match value {
		serde_json::Value::String(value) => value.clone(),
		serde_json::Value::Number(value) => value.to_string(),
		serde_json::Value::Bool(value) => value.to_string(),
		serde_json::Value::Null => String::new(),
		serde_json::Value::Array(values) => values
			.iter()
			.map(inline_json_value_to_display_string)
			.collect::<Vec<_>>()
			.join(", "),
		serde_json::Value::Object(_) => value.to_string(),
	}
}

fn model_form_page(model_name: &str, record_id: Option<&str>, form_groups: Page) -> Page {
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
	use wasm_bindgen::JsCast;

	let form = event
		.raw()
		.target()
		.or_else(|| event.raw().current_target())
		.and_then(|target| target.dyn_into::<web_sys::HtmlFormElement>().ok());
	if let Some(form) = &form {
		clear_inline_validation_errors(form);
	}
	let request = collect_mutation_request(event.raw());
	reinhardt_pages::platform::spawn_task(async move {
		let result = if let Some(id) = record_id {
			update_record(model_name, id, request).await
		} else {
			create_record(model_name, request).await
		};

		match result {
			Ok(_) => navigate_or_set_href(&return_url),
			Err(error) => {
				let applied = form
					.as_ref()
					.is_some_and(|form| apply_inline_validation_errors(form, &error));
				if !applied {
					report_admin_error(&format!("Save failed: {}", error));
				}
			}
		}
	});
}

#[cfg(client)]
fn clear_inline_validation_errors(form: &web_sys::HtmlFormElement) {
	use wasm_bindgen::JsCast;

	if let Ok(errors) = form.query_selector_all("[data-inline-row-error]") {
		for index in 0..errors.length() {
			if let Some(error) = errors.item(index) {
				error.set_text_content(None);
			}
		}
	}

	if let Ok(fields) = form.query_selector_all(r#"[name^="__reinhardt_inlines."]"#) {
		for index in 0..fields.length() {
			if let Some(field) = fields.item(index)
				&& let Ok(field) = field.dyn_into::<web_sys::Element>()
			{
				let _ = field.remove_attribute("aria-invalid");
				let _ = field.remove_attribute("aria-describedby");
			}
		}
	}
}

#[cfg(client)]
fn apply_inline_validation_errors(
	form: &web_sys::HtmlFormElement,
	error: &reinhardt_pages::server_fn::ServerFnError,
) -> bool {
	use reinhardt_pages::server_fn::ServerFnErrorKind;

	if error.kind() != ServerFnErrorKind::Validation || error.field_errors().is_empty() {
		return false;
	}

	let mut messages_by_row: HashMap<String, Vec<String>> = HashMap::new();
	let mut applied = 0;
	for field_error in error.field_errors() {
		let Some((key, index, field)) = parse_inline_error_path(field_error.field()) else {
			continue;
		};
		let row_error_id = inline_row_error_id(key, index);
		let Ok(Some(_)) = form.query_selector(&format!("#{row_error_id}")) else {
			continue;
		};
		if field == "_all" {
			messages_by_row
				.entry(row_error_id)
				.or_default()
				.push(field_error.message().to_string());
			applied += 1;
			continue;
		}

		let field_id = inline_field_id(key, index, field);
		let Ok(Some(input)) = form.query_selector(&format!("#{field_id}")) else {
			continue;
		};
		let expected_name = inline_field_name(key, index, field);
		if input.get_attribute("name").as_deref() != Some(expected_name.as_str()) {
			continue;
		}

		let _ = input.set_attribute("aria-invalid", "true");
		let _ = input.set_attribute("aria-describedby", &row_error_id);
		messages_by_row
			.entry(row_error_id)
			.or_default()
			.push(field_error.message().to_string());
		applied += 1;
	}

	for (row_error_id, messages) in messages_by_row {
		if let Ok(Some(row_error)) = form.query_selector(&format!("#{row_error_id}")) {
			row_error.set_text_content(Some(&messages.join(" ")));
		}
	}

	applied == error.field_errors().len()
}

#[cfg(client)]
fn parse_inline_error_path(path: &str) -> Option<(&str, usize, &str)> {
	let mut parts = path.split('.');
	let key = parts.next()?;
	let index = parts.next()?.parse().ok()?;
	let field = parts.next()?;
	if key.is_empty() || field.is_empty() || parts.next().is_some() {
		return None;
	}

	Some((key, index, field))
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
	use wasm_bindgen::JsCast;

	if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
		let name = input.name();
		if name.is_empty() {
			return;
		}
		let value = if input.type_() == "checkbox" {
			serde_json::Value::Bool(input.checked())
		} else {
			form_value_to_json(&name, &input.value(), input.type_() == "number")
		};
		data.insert(name, value);
		return;
	}

	if let Some(textarea) = element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
		let name = textarea.name();
		if !name.is_empty() {
			data.insert(name, serde_json::Value::String(textarea.value()));
		}
		return;
	}

	if let Some(select) = element.dyn_ref::<web_sys::HtmlSelectElement>() {
		let name = select.name();
		if !name.is_empty() {
			data.insert(name.clone(), select_value_to_json(select, &name));
		}
	}
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
	if name.starts_with("__reinhardt_inlines.") {
		return serde_json::Value::String(value.to_string());
	}
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
	use crate::types::FormFieldSpec;

	let input_id = input_id.to_string();
	let name = field.name.clone();
	let label = label.to_string();
	let value = field.value.clone();
	let required = field.required;

	match &field.spec {
		FormFieldSpec::Input { html_type } => {
			render_input(html_type.clone(), input_id, name, label, value, required)
		}
		FormFieldSpec::File => {
			render_input("file".to_string(), input_id, name, label, value, required)
		}
		FormFieldSpec::Hidden => {
			render_input("hidden".to_string(), input_id, name, label, value, required)
		}
		FormFieldSpec::TextArea => {
			if required {
				page!(|input_id: String, name: String, label: String, value: String| {
					textarea {
						class: "admin-input",
						id: input_id,
						name: name,
						aria_label: label,
						required: true,
						autocomplete: "off",
						{ value }
					}
				})(input_id, name, label, value)
			} else {
				page!(|input_id: String, name: String, label: String, value: String| {
					textarea {
						class: "admin-input",
						id: input_id,
						name: name,
						aria_label: label,
						autocomplete: "off",
						{ value }
					}
				})(input_id, name, label, value)
			}
		}
		FormFieldSpec::Select { choices } => {
			let options = render_option_elements(choices, &[value.as_str()]);
			if required {
				page!(|input_id: String, name: String, label: String, options: Vec<Page>| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						aria_label: label,
						required: true,
						{ options }
					}
				})(input_id, name, label, options)
			} else {
				page!(|input_id: String, name: String, label: String, options: Vec<Page>| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						aria_label: label,
						{ options }
					}
				})(input_id, name, label, options)
			}
		}
		FormFieldSpec::MultiSelect { choices } => {
			let selected = parse_multi_value(&value);
			let options = render_option_elements(choices, &selected);
			if required {
				page!(|input_id: String, name: String, label: String, options: Vec<Page>| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						aria_label: label,
						multiple: true,
						required: true,
						{ options }
					}
				})(input_id, name, label, options)
			} else {
				page!(|input_id: String, name: String, label: String, options: Vec<Page>| {
					select {
						class: "admin-select",
						id: input_id,
						name: name,
						aria_label: label,
						multiple: true,
						{ options }
					}
				})(input_id, name, label, options)
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
	value: String,
	required: bool,
) -> Page {
	if html_type == "checkbox" {
		let checked = matches!(value.as_str(), "true" | "1" | "on");
		return render_checkbox_input(input_id, name, label, value, checked);
	}
	if html_type == "number" {
		return page!(|input_id: String,
		 name: String,
		 label: String,
		 value: String,
		 step: String,
		 required: bool| {
			input {
				class: "admin-input",
				type: "number",
				id: input_id,
				name: name,
				aria_label: label,
				value: value,
				step: step,
				required: required,
				autocomplete: "off",
			}
		})(input_id, name, label, value, "any".to_owned(), required);
	}

	if required {
		page!(|html_type: String, input_id: String, name: String, label: String, value: String| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				aria_label: label,
				value: value,
				required: true,
				autocomplete: "off",
			}
		})(html_type, input_id, name, label, value)
	} else {
		page!(|html_type: String, input_id: String, name: String, label: String, value: String| {
			input {
				class: "admin-input",
				type: html_type,
				id: input_id,
				name: name,
				aria_label: label,
				value: value,
				autocomplete: "off",
			}
		})(html_type, input_id, name, label, value)
	}
}

fn render_checkbox_input(
	input_id: String,
	name: String,
	label: String,
	value: String,
	checked: bool,
) -> Page {
	page!(|input_id: String, name: String, label: String, value: String, checked: bool| {
		input {
			class: "admin-input",
			type: "checkbox",
			id: input_id,
			name: name,
			aria_label: label,
			value: value,
			checked: checked,
			autocomplete: "off",
		}
	})(input_id, name, label, value, checked)
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
		Column, ListViewData, action_can_dispatch, detail_table, find_admin_action,
		form_value_to_json, form_values_to_json_array, list_view_with_actions, record_primary_key,
		set_page_selected, set_record_selected,
	};
	use crate::types::{AdminAction, AdminActionRequest, ModelPermission, MutationResponse};
	use reinhardt_pages::Signal;
	use reinhardt_pages::reactive::use_action;
	use reinhardt_pages::testing::component::render;
	use rstest::rstest;
	use serde_json::json;
	use std::cell::RefCell;
	use std::collections::{BTreeSet, HashMap};
	use std::rc::Rc;

	#[rstest]
	fn admin_action_primary_key_uses_the_configured_field_without_an_id_fallback() {
		// Arrange
		let record = HashMap::from([
			("id".to_string(), "17".to_string()),
			("slug".to_string(), "release-notes".to_string()),
		]);

		// Act
		let primary_key = record_primary_key(&record, "slug");

		// Assert
		assert_eq!(primary_key, Some("release-notes".to_string()));
		assert_eq!(record_primary_key(&record, "uuid"), None);
	}

	#[rstest]
	fn admin_action_selecting_a_page_replaces_and_clears_the_selection() {
		// Arrange
		let mut selected = BTreeSet::from(["stale-page-id".to_string()]);
		let page_ids = vec!["article-a".to_string(), "article-b".to_string()];

		// Act
		set_page_selected(&mut selected, &page_ids, true);

		// Assert
		assert_eq!(
			selected,
			BTreeSet::from(["article-a".to_string(), "article-b".to_string()])
		);

		// Act
		set_page_selected(&mut selected, &page_ids, false);

		// Assert
		assert!(selected.is_empty());
	}

	#[rstest]
	fn admin_action_selecting_one_record_toggles_only_that_id() {
		// Arrange
		let mut selected = BTreeSet::from(["article-a".to_string()]);

		// Act
		set_record_selected(&mut selected, "article-b", true);
		set_record_selected(&mut selected, "article-a", false);

		// Assert
		assert_eq!(selected, BTreeSet::from(["article-b".to_string()]));
	}

	#[rstest]
	fn admin_action_controls_select_records_by_the_configured_primary_key() {
		// Arrange
		let actions = vec![AdminAction::new(
			"publish",
			"Publish selected",
			ModelPermission::Change,
			false,
		)];
		let data = ListViewData {
			model_name: "Article".to_string(),
			columns: vec![Column {
				field: "title".to_string(),
				label: "Title".to_string(),
				sortable: true,
			}],
			records: vec![
				HashMap::from([
					("id".to_string(), "41".to_string()),
					("slug".to_string(), "article-a".to_string()),
					("title".to_string(), "Article A".to_string()),
				]),
				HashMap::from([
					("id".to_string(), "42".to_string()),
					("slug".to_string(), "article-b".to_string()),
					("title".to_string(), "Article B".to_string()),
				]),
			],
			current_page: 1,
			total_pages: 1,
			total_count: 2,
			filters: Vec::new(),
		};
		let selected_slot = Rc::new(RefCell::new(None));
		let selected_for_view = Rc::clone(&selected_slot);
		let screen = render(move || {
			let selected_ids = Signal::new(BTreeSet::new());
			*selected_for_view.borrow_mut() = Some(selected_ids);
			let selected_action = Signal::new(String::new());
			let action = use_action(|_: AdminActionRequest| async {
				Ok::<MutationResponse, String>(MutationResponse {
					success: true,
					message: "done".to_string(),
					affected: Some(0),
					data: None,
				})
			});
			list_view_with_actions(
				&data,
				"slug",
				&actions,
				Signal::new(1),
				Signal::new(HashMap::new()),
				(selected_ids, selected_action, action),
			)
		});
		assert!(
			selected_slot
				.borrow()
				.expect("the list view must publish its selection signal")
				.get()
				.is_empty()
		);

		// Act
		screen
			.get_by_label("Select current page")
			.change_checked(true);
		screen
			.get_by_label("Select article-a")
			.change_checked(false);

		// Assert
		let selected_ids = selected_slot
			.borrow()
			.expect("the list view must publish its selection signal")
			.get();
		assert_eq!(selected_ids, BTreeSet::from(["article-b".to_string()]));
		assert!(!selected_ids.contains("41"));
	}

	#[rstest]
	fn admin_action_metadata_controls_confirmation_and_pending_dispatch() {
		// Arrange
		let actions = vec![
			AdminAction::new(
				"publish",
				"Publish selected",
				ModelPermission::Change,
				false,
			),
			AdminAction::new("archive", "Archive selected", ModelPermission::Delete, true),
		];
		let selected_ids = BTreeSet::from(["article-a".to_string()]);

		// Act
		let selected = find_admin_action(&actions, "archive")
			.expect("the selected action must be resolved by its exact name");

		// Assert
		assert_eq!(selected.label, "Archive selected");
		assert!(selected.requires_confirmation);
		assert!(action_can_dispatch(false, Some(selected), &selected_ids));
		assert!(!action_can_dispatch(true, Some(selected), &selected_ids));
		assert!(!action_can_dispatch(false, None, &selected_ids));
		assert!(!action_can_dispatch(
			false,
			Some(selected),
			&BTreeSet::new()
		));
	}

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
}
